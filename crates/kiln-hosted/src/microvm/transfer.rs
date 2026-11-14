//! Bounded durable checkpoint objects. This is local storage, not placement or
//! writer authority. Callers must hold the provider's paused-volume/fencing
//! reservation for the whole capture. Network acknowledgement is not durability.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use kiln_core::types::ContentId;
use kiln_microvm::handoff::{CheckpointExpectation, CheckpointManifest, admit_checkpoint};
use serde::{Deserialize, Serialize};

const COPY_BUFFER_BYTES: usize = 65_536;
const MAX_CHECKPOINT_OBJECT_BYTES: u64 = 1 << 40;
const MAX_MANIFEST_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableObject {
    pub identity: ContentId,
    pub bytes: u64,
}

/// A private, service-owned object directory. It never accepts a caller-selected
/// object pathname, follows a symlink, truncates an existing object, or reuses a
/// partially consumed operation. No automatic garbage collection is performed.
#[derive(Debug)]
pub struct CheckpointStore {
    root: PathBuf,
    directory: File,
}

impl CheckpointStore {
    pub fn open(root: &Path) -> Result<Self, String> {
        super::verify_pool_directory(root)?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let directory = File::open(&root).map_err(|e| e.to_string())?;
        Ok(Self { root, directory })
    }

    fn name(identity: &ContentId) -> Result<&str, String> {
        identity.validate().map_err(|e| format!("{e:?}"))?;
        Ok(identity.as_str())
    }

    fn object_path(&self, identity: &ContentId) -> Result<PathBuf, String> {
        Ok(self.root.join(format!("{}.object", Self::name(identity)?)))
    }

    /// Stream one exactly sized object. The input owner must enforce a bounded
    /// read timeout; this loop checks cancellation/deadline between reads. On any
    /// error the `.partial` evidence stays present and cannot be replayed under
    /// the same operation ID. Socket errors do not delete it or authorize retry.
    // r[impl kiln.hosted.mobility.runtime.scenario.durable]
    pub fn receive(
        &self,
        operation: &ContentId,
        expected: &DurableObject,
        input: &mut impl Read,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<DurableObject, String> {
        if expected.bytes == 0 || expected.bytes > MAX_CHECKPOINT_OBJECT_BYTES {
            return Err("checkpoint object size exceeds admitted bound".into());
        }
        let partial = self.root.join(format!(
            "{}-{}.partial",
            Self::name(operation)?,
            Self::name(&expected.identity)?
        ));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&partial)
            .map_err(|e| e.to_string())?;
        // Persist consumption before the first input byte. A controller restart
        // must reconcile this operation rather than replaying its old stream.
        self.directory.sync_all().map_err(|e| e.to_string())?;
        let mut hasher = blake3::Hasher::new();
        let mut remaining = expected.bytes;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        while remaining != 0 {
            if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
                return Err("checkpoint transfer cancelled or expired; reconcile operation".into());
            }
            let bound = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
                .map_err(|e| e.to_string())?;
            let count = input
                .read(buffer.get_mut(..bound).ok_or("copy buffer bound")?)
                .map_err(|e| e.to_string())?;
            if count == 0 {
                return Err("checkpoint object ended before its exact length".into());
            }
            let bytes = buffer.get(..count).ok_or("copy input bound")?;
            output.write_all(bytes).map_err(|e| e.to_string())?;
            hasher.update(bytes);
            remaining -= count as u64;
        }
        let actual = ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
            .map_err(|e| format!("{e:?}"))?;
        if actual != expected.identity {
            return Err("checkpoint object digest mismatch".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        let final_path = self.object_path(&actual)?;
        match fs::hard_link(&partial, &final_path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => self.verify(expected)?,
            Err(e) => return Err(e.to_string()),
        }
        self.directory.sync_all().map_err(|e| e.to_string())?;
        // Retain the consumption marker, including after success, until explicit
        // operator/provider retirement; it is not a distributed lock or fence.
        Ok(expected.clone())
    }

    pub fn verify(&self, expected: &DurableObject) -> Result<(), String> {
        if expected.bytes == 0 || expected.bytes > MAX_CHECKPOINT_OBJECT_BYTES {
            return Err("checkpoint object size exceeds bound".into());
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.object_path(&expected.identity)?)
            .map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() != expected.bytes || metadata.mode() & 0o077 != 0 {
            return Err("checkpoint object type, permissions or exact size mismatch".into());
        }
        let mut hash = blake3::Hasher::new();
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        let mut left = expected.bytes;
        while left > 0 {
            let bound =
                usize::try_from(left.min(COPY_BUFFER_BYTES as u64)).map_err(|e| e.to_string())?;
            let count = file
                .read(buffer.get_mut(..bound).ok_or("verify buffer bound")?)
                .map_err(|e| e.to_string())?;
            if count == 0 {
                return Err("checkpoint object shortened while verifying".into());
            }
            hash.update(buffer.get(..count).ok_or("verify input bound")?);
            left -= count as u64;
        }
        let actual = ContentId::from_blake3_hex(hash.finalize().to_hex().to_string())
            .map_err(|e| format!("{e:?}"))?;
        if actual != expected.identity {
            return Err("checkpoint object digest mismatch".into());
        }
        file.sync_all().map_err(|e| e.to_string())
    }

    /// Publish only after re-reading every distinct manifest object from local
    /// durable storage. A caller's receipt or network ACK is never enough.
    pub fn publish_checkpoint(
        &self,
        manifest: &CheckpointManifest,
        expected: &CheckpointExpectation,
        objects: &[DurableObject; 4],
    ) -> Result<ContentId, String> {
        let identity = admit_checkpoint(manifest, expected).map_err(|e| format!("{e:?}"))?;
        for (identity, object) in [
            &manifest.objects.ram,
            &manifest.objects.device_state,
            &manifest.objects.disk,
            &manifest.objects.workspace,
        ]
        .into_iter()
        .zip(objects)
        {
            if identity != &object.identity {
                return Err("checkpoint object receipt binding mismatch".into());
            }
            self.verify(object)?;
        }
        let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("checkpoint manifest exceeds bound".into());
        }
        let partial = self
            .root
            .join(format!("{}.manifest.partial", Self::name(&identity)?));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&partial)
            .map_err(|e| e.to_string())?;
        output
            .write_all(&bytes)
            .and_then(|()| output.sync_all())
            .map_err(|e| e.to_string())?;
        fs::hard_link(
            &partial,
            self.root
                .join(format!("{}.manifest", Self::name(&identity)?)),
        )
        .map_err(|e| e.to_string())?;
        self.directory.sync_all().map_err(|e| e.to_string())?;
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    // r[verify kiln.hosted.mobility.runtime.scenario.durable]
    #[test]
    fn streamed_objects_are_verified_durable_and_single_use() {
        let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let store = CheckpointStore::open(temp.path()).unwrap_or_else(|_| unreachable!());
        let data = vec![0x5a; COPY_BUFFER_BYTES * 3 + 7];
        let identity = ContentId::from_blake3_hex(blake3::hash(&data).to_hex().to_string())
            .unwrap_or_else(|_| unreachable!());
        let object = DurableObject {
            identity: identity.clone(),
            bytes: data.len() as u64,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(10);
        assert_eq!(
            store.receive(
                &identity,
                &object,
                &mut data.as_slice(),
                deadline,
                &cancelled
            ),
            Ok(object.clone())
        );
        assert!(store.verify(&object).is_ok());
        assert!(
            store
                .receive(
                    &identity,
                    &object,
                    &mut data.as_slice(),
                    deadline,
                    &cancelled
                )
                .is_err()
        );
        fs::write(
            store
                .object_path(&identity)
                .unwrap_or_else(|_| unreachable!()),
            b"corrupt",
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(store.verify(&object).is_err());
    }

    #[test]
    fn incomplete_digest_mismatch_cancel_and_symlink_never_publish() {
        let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let store = CheckpointStore::open(temp.path()).unwrap_or_else(|_| unreachable!());
        let identity =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!());
        let object = DurableObject {
            identity: identity.clone(),
            bytes: 5,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(10);
        assert!(
            store
                .receive(
                    &identity,
                    &object,
                    &mut b"abc".as_slice(),
                    deadline,
                    &cancelled
                )
                .is_err()
        );
        assert!(
            !store
                .object_path(&identity)
                .unwrap_or_else(|_| unreachable!())
                .exists()
        );
        assert!(
            store
                .receive(
                    &identity,
                    &object,
                    &mut b"abcde".as_slice(),
                    deadline,
                    &cancelled
                )
                .is_err()
        );
        let other = ContentId::from_blake3_hex("b".repeat(64)).unwrap_or_else(|_| unreachable!());
        assert!(
            store
                .receive(
                    &other,
                    &object,
                    &mut b"abcde".as_slice(),
                    deadline,
                    &cancelled
                )
                .is_err()
        );
        std::os::unix::fs::symlink(
            "/etc/passwd",
            store
                .object_path(&identity)
                .unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(store.verify(&object).is_err());
        cancelled.store(true, Ordering::Release);
        let third = ContentId::from_blake3_hex("c".repeat(64)).unwrap_or_else(|_| unreachable!());
        assert!(
            store
                .receive(
                    &third,
                    &object,
                    &mut b"abcde".as_slice(),
                    deadline,
                    &cancelled
                )
                .is_err()
        );
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::*;
    use kiln_microvm::handoff::{CHECKPOINT_SCHEMA, CheckpointObjects, CheckpointProfile};
    use kiln_microvm::retained::Placement;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn manifest_requires_every_durable_object_and_independent_profile() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let store = CheckpointStore::open(root.path()).unwrap_or_else(|_| unreachable!());
        let id = |c: char| {
            ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        let payloads = [
            b"RAM".as_slice(),
            b"DEVICES".as_slice(),
            b"DISK".as_slice(),
            b"WORKSPACE".as_slice(),
        ];
        let objects = payloads.map(|bytes| DurableObject {
            identity: ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string())
                .unwrap_or_else(|_| unreachable!()),
            bytes: bytes.len() as u64,
        });
        let [ram, device_state, disk, workspace_object] = &objects;
        let profile = CheckpointProfile {
            image: id('1'),
            kernel: id('2'),
            hypervisor: id('3'),
            cpu: id('4'),
            initrd: id('5'),
            devices: id('6'),
            memory_backing: id('7'),
        };
        let manifest = CheckpointManifest {
            schema: CHECKPOINT_SCHEMA.into(),
            session: id('8'),
            vm: id('9'),
            work: id('a'),
            work_revision: 1,
            attempt: id('b'),
            fence: 1,
            execution: id('c'),
            source: Placement {
                host: "a".into(),
                generation: 1,
            },
            target: Placement {
                host: "b".into(),
                generation: 2,
            },
            operation: id('d'),
            profile: profile.clone(),
            disk_generation: 1,
            objects: CheckpointObjects {
                ram: ram.identity.clone(),
                device_state: device_state.identity.clone(),
                disk: disk.identity.clone(),
                workspace: workspace_object.identity.clone(),
            },
        };
        let mut expected = CheckpointExpectation {
            session: manifest.session.clone(),
            vm: manifest.vm.clone(),
            work: manifest.work.clone(),
            work_revision: 1,
            attempt: manifest.attempt.clone(),
            fence: 1,
            execution: manifest.execution.clone(),
            source: manifest.source.clone(),
            target: manifest.target.clone(),
            operation: manifest.operation.clone(),
            disk: manifest.objects.disk.clone(),
            disk_generation: 1,
            profile,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let cancelled = AtomicBool::new(false);
        for (object, bytes) in objects.iter().zip(payloads).take(3) {
            let mut input = bytes;
            store
                .receive(
                    &manifest.operation,
                    object,
                    &mut input,
                    deadline,
                    &cancelled,
                )
                .unwrap_or_else(|_| unreachable!());
        }
        assert!(
            store
                .publish_checkpoint(&manifest, &expected, &objects)
                .is_err()
        );
        let [_, _, _, mut workspace] = payloads;
        store
            .receive(
                &manifest.operation,
                workspace_object,
                &mut workspace,
                deadline,
                &cancelled,
            )
            .unwrap_or_else(|_| unreachable!());
        expected.profile.memory_backing = id('f');
        assert!(
            store
                .publish_checkpoint(&manifest, &expected, &objects)
                .is_err()
        );
        expected.profile.memory_backing = id('7');
        assert!(
            store
                .publish_checkpoint(&manifest, &expected, &objects)
                .is_ok()
        );
    }
}
