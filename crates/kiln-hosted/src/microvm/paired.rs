//! Local CH paused snapshot pairing. The provider reservation must quiesce
//! backend writes and guest effects before capture. Cross-host path rewriting
//! is deliberately unsupported until a durable handoff owner admits it.
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use kiln_core::types::ContentId;
use kiln_microvm::retained::{Placement, SessionIdentity};
use serde::{Deserialize, Serialize};

use super::retained::Operation;
use super::transfer::DurableObject;

const PAIR_SCHEMA: &str = "kiln.local-paused-checkpoint.v1";
const PAIR_FILE: &str = "kiln-paired-checkpoint.json";
const MAX_PAIR_BYTES: u64 = 65_536;
const MAX_OBJECT_BYTES: u64 = 1 << 40;
const CH_FILES: [&str; 3] = ["config.json", "state.json", "memory-ranges"];

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    placement: Placement,
    workspace: std::path::PathBuf,
    schema: String,
    session: SessionIdentity,
    profile: ContentId,
    disk_generation: u64,
    deadline_unix_ms: u64,
    objects: [DurableObject; 4],
}

fn digest(path: &Path) -> Result<DurableObject, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_OBJECT_BYTES {
        return Err("paired checkpoint object is not a bounded nonempty regular file".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let mut left = metadata.len();
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0_u8; 65_536];
    while left > 0 {
        let bound = usize::try_from(left.min(buffer.len() as u64)).map_err(|e| e.to_string())?;
        let count = file
            .read(buffer.get_mut(..bound).ok_or("pair copy bound")?)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("paired checkpoint changed while reading".into());
        }
        hash.update(buffer.get(..count).ok_or("pair hash bound")?);
        left -= count as u64;
    }
    if file.metadata().map_err(|e| e.to_string())?.len() != metadata.len() {
        return Err("paired checkpoint object length changed".into());
    }
    Ok(DurableObject {
        identity: ContentId::from_blake3_hex(hash.finalize().to_hex().to_string())
            .map_err(|e| format!("{e:?}"))?,
        bytes: metadata.len(),
    })
}

fn observe(operation: &Operation, directory: &Path) -> Result<Pair, String> {
    if !directory.is_absolute()
        || !directory.starts_with(&operation.binding.directory)
        || directory.components().any(|part| {
            !matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err("checkpoint escapes the exact owned VM directory".into());
    }
    super::verify_pool_directory(directory)?;
    let [config, state, memory] = CH_FILES;
    let objects = [
        digest(&directory.join(config))?,
        digest(&directory.join(state))?,
        digest(&directory.join(memory))?,
        digest(&operation.binding.workspace)?,
    ];
    Ok(Pair {
        schema: PAIR_SCHEMA.into(),
        session: operation.session.identity.clone(),
        placement: operation.session.placement.clone(),
        workspace: operation.binding.workspace.clone(),
        profile: operation.binding.profile.clone(),
        disk_generation: operation.binding.disk_generation,
        deadline_unix_ms: operation.binding.deadline_unix_ms,
        objects,
    })
}

pub(super) fn publish(operation: &Operation, directory: &Path) -> Result<(), String> {
    if operation.session.state != kiln_microvm::retained::RetainedState::Paused {
        return Err("paired checkpoint publication requires an observed paused source".into());
    }
    let pair = observe(operation, directory)?;
    let bytes = serde_json::to_vec(&pair).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory.join(PAIR_FILE))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}

pub(super) fn verify(operation: &Operation, directory: &Path) -> Result<(), String> {
    let mut bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory.join(PAIR_FILE))
        .map_err(|e| e.to_string())?
        .take(MAX_PAIR_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PAIR_BYTES {
        return Err("paired checkpoint descriptor exceeds bound".into());
    }
    let expected: Pair = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if expected != observe(operation, directory)? {
        return Err("paired memory/device/config/workspace or profile/generation mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::retained::{Action, Binding};
    use super::*;
    use kiln_microvm::retained::{Placement, RetainedResources, RetainedSession, RetainedState};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn local_restore_rejects_changed_disk_generation_memory_and_incomplete_capture() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let checkpoint = root.path().join("checkpoint");
        std::fs::create_dir(&checkpoint).unwrap_or_else(|_| unreachable!());
        std::fs::set_permissions(&checkpoint, std::fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let id = |c: char| {
            ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        let mut session = RetainedSession::admit(
            SessionIdentity {
                session: id('a'),
                execution: id('f'),
                vm: id('1'),
                work: id('b'),
                work_revision: 1,
                attempt: id('c'),
                fence: 1,
            },
            Placement {
                host: "host-a".into(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 1,
                memory_mib: 512,
                slot: 1,
            },
        )
        .unwrap_or_else(|_| unreachable!());
        let mut operation = Operation {
            operation: id('d'),
            session: session.clone(),
            binding: Binding {
                directory: root.path().into(),
                unit: "kiln-retained-a".into(),
                workspace: root.path().join("workspace.raw"),
                disk_generation: 1,
                deadline_unix_ms: u64::MAX,
                profile: id('e'),
            },
            action: Action::Snapshot {
                checkpoint: checkpoint.clone(),
            },
        };
        std::fs::write(&operation.binding.workspace, b"counter=41\n")
            .unwrap_or_else(|_| unreachable!());
        for name in CH_FILES {
            std::fs::write(checkpoint.join(name), name.as_bytes())
                .unwrap_or_else(|_| unreachable!());
        }
        assert!(publish(&operation, &checkpoint).is_err());
        session.state = RetainedState::Paused;
        operation.session = session;
        publish(&operation, &checkpoint).unwrap_or_else(|_| unreachable!());
        assert!(verify(&operation, &checkpoint).is_ok());
        operation.binding.disk_generation += 1;
        assert!(verify(&operation, &checkpoint).is_err());
        operation.binding.disk_generation -= 1;
        std::fs::write(&operation.binding.workspace, b"counter=42\n")
            .unwrap_or_else(|_| unreachable!());
        assert!(verify(&operation, &checkpoint).is_err());
        std::fs::write(&operation.binding.workspace, b"counter=41\n")
            .unwrap_or_else(|_| unreachable!());
        std::fs::write(checkpoint.join("memory-ranges"), b"other memory")
            .unwrap_or_else(|_| unreachable!());
        assert!(verify(&operation, &checkpoint).is_err());
        std::fs::remove_file(checkpoint.join("state.json")).unwrap_or_else(|_| unreachable!());
        assert!(verify(&operation, &checkpoint).is_err());
    }
}
