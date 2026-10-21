use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use bounded_exec::ExecutionLimits;
use fs2::FileExt;
use kiln_adapter_nix_eval::{
    BoundedNixEvalRuntime, NixEvalProcessOutput, NixEvalRuntime, ProcessInvocation,
};
use kiln_adapter_radicle::source::RadicleSourceShell;
use kiln_core::source::{
    GitEntryKind, GitSnapshotEntry, GitSnapshotLimits, build_git_snapshot_manifest,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_radicle_ci::project::{
    PROJECT_RESULT_FILE, PROJECT_RESULT_SCHEMA, ProjectSourceIntent, ProjectWorkerProfile,
    ProjectWorkerResult, RESOLVED_JOB_FILE, ResolvedWorkerJob, SOURCE_INTENT_FILE, SourceContext,
    SourceImportObservation, admit_result, admit_source_intent, identity, native_store_path,
    resolved_job,
};

const IMPORT_FILE: &str = "source-import.json";
const PROJECT_INTENT_FILE: &str = "project-run-intent.json";
const SOURCE_ROOT: &str = "source-root";

// r[impl kiln.radicle_ci.runtime] r[impl kiln.radicle_ci.recovery]
pub(super) fn execute(arguments: &[String]) -> Result<bool, String> {
    let (command, profile_path, profile_hash, source) = match arguments {
        [command, path, hash] if command == "project-run" => (
            command, path, hash, SourceContext {
                provider: std::env::var("LATTICE_SOURCE_PROVIDER").map_err(crate::text)?,
                repository: std::env::var("LATTICE_SOURCE_REPOSITORY").map_err(crate::text)?,
                revision: std::env::var("LATTICE_SOURCE_REVISION").map_err(crate::text)?,
            }),
        [command, path, hash, context] if command == "project-release" => (
            command, path, hash,
            serde_json::from_slice(&crate::read_bounded(Path::new(context))?).map_err(crate::text)?),
        _ => return Err("usage: project-run PROFILE_JSON PROFILE_BLAKE3 | project-release PROFILE_JSON PROFILE_BLAKE3 SOURCE_CONTEXT_JSON".into()),
    };
    let bytes = crate::read_bounded(Path::new(profile_path))?;
    let profile_identity = ContentId::from_blake3_hex(profile_hash).map_err(crate::text)?;
    if blake3::hash(&bytes).to_hex().as_str() != profile_hash {
        return Err("project profile bytes differ from the fixed reviewed identity".into());
    }
    let profile: ProjectWorkerProfile = serde_json::from_slice(&bytes).map_err(crate::text)?;
    let bindings = profile.bindings(&profile_identity, &source)?;
    // The controller creates this root and pins source before native dispatch.
    if !bindings.root.is_dir() {
        return Err("controller source reservation is absent".into());
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(bindings.root.join("project.lock"))
        .map_err(crate::text)?;
    lock.try_lock_exclusive()
        .map_err(|_| "project candidate is already in use".to_string())?;
    let intent: ProjectSourceIntent = load(&bindings.root.join(SOURCE_INTENT_FILE))?;
    admit_source_intent(&profile, &profile_identity, &intent)?;
    if intent.source != source {
        return Err("controller source intent names a different event".into());
    }
    let result_path = bindings.root.join(PROJECT_RESULT_FILE);
    let worker_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(bindings.root.join("worker.lock"))
        .map_err(crate::text)?;
    worker_lock
        .try_lock_exclusive()
        .map_err(|_| "worker state is already in use".to_string())?;
    if command == "project-release" {
        let result: ProjectWorkerResult = load(&result_path)?;
        admit_result(
            &profile,
            &profile_identity,
            &profile.trigger(&source)?,
            &result,
        )?;
        if result.source_intent != intent {
            return Err("release result changed its controller-reserved source intent".into());
        }
        let job: ResolvedWorkerJob = load(&bindings.root.join(RESOLVED_JOB_FILE))?;
        let expected = resolved_job(
            &profile,
            &profile_identity,
            &intent,
            &result.source_store_path,
            &result.source_import,
        )?;
        if job != expected {
            return Err("release resolved job substitution".into());
        }
        let job_identity = job_identity(&job)?;
        if result.worker.build_observation.is_some() {
            crate::release(&job, &job_identity)?;
        }
        release_source_root(&bindings.root.join(SOURCE_ROOT), &result.source_store_path)?;
        // Discovery roots retain the existing adapter's durable-task handoff policy;
        // this command releases only owned source and actual build-output roots.
        let released =
            serde_json::json!({"source_root_released": true, "build_output_roots_released": true});
        let release_path = bindings.root.join("project-root-release.json");
        if release_path.try_exists().map_err(crate::text)? {
            let prior: serde_json::Value = load(&release_path)?;
            if prior != released {
                return Err("project root release receipt changed".into());
            }
        } else {
            crate::store(&release_path, &released)?;
        }
        println!("{}", serde_json::to_string(&released).map_err(crate::text)?);
        return Ok(true);
    }
    if result_path.exists() {
        let result: ProjectWorkerResult = load(&result_path)?;
        let (outcome, _) = admit_result(
            &profile,
            &profile_identity,
            &profile.trigger(&source)?,
            &result,
        )?;
        if result.source_intent != intent {
            return Err("completed result changed its reserved source intent".into());
        }
        println!("{}", serde_json::to_string(&result).map_err(crate::text)?);
        return Ok(outcome == kiln_core::domain::Outcome::Success);
    }
    let run_intent = bindings.root.join(PROJECT_INTENT_FILE);
    let job_path = bindings.root.join(RESOLVED_JOB_FILE);
    let (job, imported, store_path) = if run_intent.exists() {
        let prior: ContentId = load(&run_intent)?;
        if prior != identity(&intent)? {
            return Err("project reservation changed pinned source".into());
        }
        if !job_path.exists() {
            return Err(
                "interrupted project source/import intent remains uncertain; no redispatch".into(),
            );
        }
        let imported: SourceImportObservation = load(&bindings.root.join(IMPORT_FILE))?;
        let store_path = import_path(&profile, &imported.stdout)?;
        let job: ResolvedWorkerJob = load(&job_path)?;
        let expected = resolved_job(&profile, &profile_identity, &intent, &store_path, &imported)?;
        if job != expected {
            return Err("retained resolved job differs from source/profile".into());
        }
        (job, imported, store_path)
    } else {
        if job_path.exists() {
            return Err("resolved project job exists without its intent".into());
        }
        crate::store(&run_intent, &identity(&intent)?)?;
        let checkout = bindings.root.join("source-checkout");
        let acquired = RadicleSourceShell::new(intent.pinned_profile.clone())
            .map_err(crate::text)?
            .materialize(&checkout)
            .map_err(crate::text)?;
        if acquired != intent.resolution {
            return Err("materialized source differs from controller-pinned acquisition".into());
        }
        let (tree, flake) = measure_source(&profile.source.limits, &checkout)?;
        if tree != acquired.resolution.canonical_tree {
            return Err("materialized source tree differs from its source receipt".into());
        }
        let home = bindings.root.join("import-home");
        crate::private_directory(&home)?;
        let output = invoke(
            &profile,
            &home,
            false,
            vec![
                "--extra-experimental-features".into(),
                "nix-command".into(),
                "store".into(),
                "add-path".into(),
                "--name".into(),
                "radicle-source".into(),
                checkout.into_os_string(),
            ],
        )?;
        let store_path = match &output {
            NixEvalProcessOutput::Exited {
                process_status: Some(0),
                signal: None,
                stdout,
                ..
            } => import_path(&profile, stdout),
            _ => Err("bounded immutable source import failed".into()),
        };
        let store_path = match store_path {
            Ok(path) => path,
            Err(error) => {
                exited(output, &bindings.root, "source-import")?;
                return Err(error);
            }
        };
        // Register the root before log fsyncs and the full imported-tree read.
        let root_output = invoke(
            &profile,
            &home,
            true,
            vec![
                "--add-root".into(),
                bindings.root.join(SOURCE_ROOT).into_os_string(),
                "--indirect".into(),
                "--realise".into(),
                store_path.clone().into_os_string(),
            ],
        );
        let (status, stdout, stderr) = exited(output, &bindings.root, "source-import")?;
        let imported = SourceImportObservation {
            executable_identity: profile.build_profile.executable_identity.clone(),
            process_status: status,
            stdout,
            stderr,
            canonical_tree: tree,
            flake_identity: flake,
        };
        crate::store(&bindings.root.join(IMPORT_FILE), &imported)?;
        let (status, _, _) = exited(root_output?, &bindings.root, "source-root")?;
        if status != 0
            || fs::read_link(bindings.root.join(SOURCE_ROOT)).map_err(crate::text)? != store_path
        {
            return Err("source import root handoff did not name exact imported source".into());
        }
        let (stored_tree, stored_flake) = measure_source(&profile.source.limits, &store_path)?;
        if stored_tree != imported.canonical_tree || stored_flake != imported.flake_identity {
            return Err("immutable Nix source differs from the unchanged acquired source".into());
        }
        let job = resolved_job(&profile, &profile_identity, &intent, &store_path, &imported)?;
        crate::store(&job_path, &job)?;
        (job, imported, store_path)
    };
    crate::admit_job(&job)?;
    crate::run(&job, job_identity(&job)?)?;
    let worker = load(&bindings.root.join(crate::RESULT_FILE))?;
    let result = ProjectWorkerResult {
        schema: PROJECT_RESULT_SCHEMA.into(),
        profile_identity,
        source,
        source_intent: intent,
        source_store_path: store_path,
        source_import: imported,
        worker,
    };
    let (outcome, _) = admit_result(
        &profile,
        &result.profile_identity,
        &profile.trigger(&result.source)?,
        &result,
    )?;
    crate::store(&result_path, &result)?;
    println!("{}", serde_json::to_string(&result).map_err(crate::text)?);
    Ok(outcome == kiln_core::domain::Outcome::Success)
}

fn job_identity(job: &ResolvedWorkerJob) -> Result<ContentId, String> {
    ContentId::from_blake3_hex(
        blake3::hash(&serde_json::to_vec_pretty(job).map_err(crate::text)?)
            .to_hex()
            .to_string(),
    )
    .map_err(crate::text)
}
fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&crate::read_bounded(path)?).map_err(crate::text)
}
fn import_path(profile: &ProjectWorkerProfile, stdout: &[u8]) -> Result<PathBuf, String> {
    let line = std::str::from_utf8(stdout).map_err(crate::text)?;
    let line = line
        .strip_suffix('\n')
        .ok_or("source import omitted exact path newline")?;
    let path = PathBuf::from(line);
    if !native_store_path(&profile.build_profile.store_path, &path) {
        return Err("source import returned an unexpected native store path".into());
    }
    Ok(path)
}

fn invoke(
    profile: &ProjectWorkerProfile,
    home: &Path,
    store_tool: bool,
    args: Vec<OsString>,
) -> Result<NixEvalProcessOutput, String> {
    let build = &profile.build_profile;
    let (program, expected) = if store_tool {
        (
            &build.nix_store_executable_path,
            &build.nix_store_executable_identity,
        )
    } else {
        (&build.executable_path, &build.executable_identity)
    };
    let mut runtime = BoundedNixEvalRuntime;
    if runtime
        .file_identity(program, build.limits.max_executable_bytes)
        .map_err(crate::text)?
        != *expected
    {
        return Err("source import executable differs from measured reviewed cohort".into());
    }
    runtime
        .run(&ProcessInvocation {
            program: program.clone(),
            args,
            current_dir: home.into(),
            environment: vec![
                ("HOME".into(), home.as_os_str().into()),
                ("NIX_CONF_DIR".into(), home.as_os_str().into()),
                (
                    "NIX_CONFIG".into(),
                    "accept-flake-config = false\nflake-registry =\nsubstituters =\n".into(),
                ),
            ],
            limits: ExecutionLimits {
                timeout_ms: build.limits.timeout_milliseconds,
                stdin_max_bytes: build.limits.stdin_max_bytes,
                stdout_max_bytes: build.limits.max_result_bytes,
                stderr_max_bytes: build.limits.max_stderr_bytes,
                poll_interval_ms: build.limits.poll_interval_milliseconds,
                teardown_timeout_ms: build.limits.teardown_timeout_milliseconds,
            },
        })
        .map_err(crate::text)
}

fn exited(
    output: NixEvalProcessOutput,
    root: &Path,
    name: &str,
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let (stdout, stderr) = match &output {
        NixEvalProcessOutput::Exited { stdout, stderr, .. }
        | NixEvalProcessOutput::TimedOut { stdout, stderr }
        | NixEvalProcessOutput::Cancelled { stdout, stderr }
        | NixEvalProcessOutput::OutputLimitExceeded { stdout, stderr } => {
            (stdout.as_slice(), stderr.as_slice())
        }
        NixEvalProcessOutput::Unknown => (&[][..], &[][..]),
    };
    crate::store_bytes(&root.join(format!("{name}.stdout.log")), stdout)?;
    crate::store_bytes(&root.join(format!("{name}.stderr.log")), stderr)?;
    match output {
        NixEvalProcessOutput::Exited { process_status: Some(status), signal: None, stdout, stderr } => Ok((status, stdout, stderr)),
        _ => Err("source import/root operation did not produce an admitted bounded exit; intent remains uncertain".into()),
    }
}

fn release_source_root(root: &Path, source: &Path) -> Result<(), String> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(root).map_err(crate::text)? != source {
                return Err("source root target substitution".into());
            }
            fs::remove_file(root).map_err(crate::text)
        }
        Ok(_) => Err("source root is not the owned indirect root symlink".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(crate::text(error)),
    }
}

/// Rebuild the existing canonical Git snapshot over unchanged checkout/store files.
fn measure_source(
    limits: &kiln_radicle_source_contract::SnapshotLimits,
    root: &Path,
) -> Result<(ContentId, ContentId), String> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    let mut total = 0_u64;
    let mut visited = 0_u64;
    while let Some(directory) = pending.pop() {
        for item in fs::read_dir(&directory).map_err(crate::text)? {
            let item = item.map_err(crate::text)?;
            visited = visited
                .checked_add(1)
                .ok_or("source entry count overflow")?;
            if visited
                > limits
                    .max_entries
                    .saturating_mul(limits.max_path_bytes.max(1))
            {
                return Err("source traversal exceeds bounded member budget".into());
            }
            let path = item.path();
            let relative = path
                .strip_prefix(root)
                .map_err(crate::text)?
                .to_str()
                .ok_or("source path is not UTF-8")?
                .to_string();
            if relative.len() as u64 > limits.max_path_bytes {
                return Err("source path exceeds acquisition bound".into());
            }
            let metadata = fs::symlink_metadata(&path).map_err(crate::text)?;
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > limits.max_file_bytes
            {
                return Err("source contains a non-regular or oversized member".into());
            }
            total = total
                .checked_add(metadata.len())
                .ok_or("source size overflow")?;
            if total > limits.max_total_bytes || entries.len() as u64 >= limits.max_entries {
                return Err("source exceeds acquired snapshot bounds".into());
            }
            let bytes = BoundedNixEvalRuntime
                .read_file_no_follow(&path, limits.max_file_bytes)
                .map_err(crate::text)?;
            entries.push(GitSnapshotEntry {
                path: relative,
                kind: if metadata.permissions().mode() & 0o111 != 0 {
                    GitEntryKind::Executable
                } else {
                    GitEntryKind::Regular
                },
                // Native object IDs are not part of canonical snapshot bytes.
                native_object: ExternalRef::new("git", "blob", "measured-source-file")
                    .map_err(crate::text)?,
                bytes,
            });
        }
    }
    let flake = entries
        .iter()
        .find(|entry| entry.path == "flake.nix")
        .ok_or("acquired project has no flake.nix")?;
    let flake_identity =
        ContentId::from_blake3_hex(blake3::hash(&flake.bytes).to_hex().to_string())
            .map_err(crate::text)?;
    let convert = |n| usize::try_from(n).map_err(crate::text);
    let manifest = build_git_snapshot_manifest(
        &entries,
        &GitSnapshotLimits {
            max_entries: convert(limits.max_entries)?,
            max_path_bytes: convert(limits.max_path_bytes)?,
            max_file_bytes: convert(limits.max_file_bytes)?,
            max_total_bytes: convert(limits.max_total_bytes)?,
        },
    )
    .map_err(crate::text)?;
    let tree = ContentId::from_blake3_hex(blake3::hash(&manifest).to_hex().to_string())
        .map_err(crate::text)?;
    Ok((tree, flake_identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kiln-project-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|_| unreachable!());
        path
    }

    // r[verify kiln.radicle_ci.source] r[verify kiln.radicle_ci.runtime]
    #[test]
    fn frozen_tree_preserves_bytes_modes_and_rejects_unsafe_or_oversized_members() {
        let root = directory();
        let mut limits = kiln_radicle_source_contract::fixture_request().limits;
        fs::write(root.join("flake.nix"), b"{ outputs = _: {}; }")
            .unwrap_or_else(|_| unreachable!());
        fs::write(root.join("build.sh"), b"#!/bin/sh\nexit 0\n").unwrap_or_else(|_| unreachable!());
        let before = measure_source(&limits, &root).unwrap_or_else(|_| unreachable!());
        fs::set_permissions(root.join("build.sh"), fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|_| unreachable!());
        let executable = measure_source(&limits, &root).unwrap_or_else(|_| unreachable!());
        assert_ne!(before.0, executable.0);
        assert_eq!(before.1, executable.1);
        fs::write(
            root.join("flake.nix"),
            b"{ outputs = _: { changed = true; }; }",
        )
        .unwrap_or_else(|_| unreachable!());
        let changed = measure_source(&limits, &root).unwrap_or_else(|_| unreachable!());
        assert_ne!(changed.0, executable.0);
        assert_ne!(changed.1, executable.1);
        symlink("flake.nix", root.join("unsafe")).unwrap_or_else(|_| unreachable!());
        assert!(measure_source(&limits, &root).is_err());
        fs::remove_file(root.join("unsafe")).unwrap_or_else(|_| unreachable!());
        limits.max_file_bytes = 1;
        assert!(measure_source(&limits, &root).is_err());
        fs::remove_dir_all(root).unwrap_or_else(|_| unreachable!());
    }

    // r[verify kiln.radicle_ci.runtime] r[verify kiln.radicle_ci.recovery]
    #[test]
    fn explicit_source_root_release_is_idempotent_and_never_removes_another_target() {
        let directory = directory();
        let root = directory.join("source-root");
        let source = Path::new("/nix/store/00000000000000000000000000000000-source");
        symlink(source, &root).unwrap_or_else(|_| unreachable!());
        assert!(
            release_source_root(
                &root,
                Path::new("/nix/store/11111111111111111111111111111111-other")
            )
            .is_err()
        );
        assert_eq!(
            fs::read_link(&root).unwrap_or_else(|_| unreachable!()),
            source
        );
        release_source_root(&root, source).unwrap_or_else(|_| unreachable!());
        release_source_root(&root, source).unwrap_or_else(|_| unreachable!());
        fs::write(&root, b"not an owned symlink").unwrap_or_else(|_| unreachable!());
        assert!(release_source_root(&root, source).is_err());
        assert_eq!(
            fs::read(&root).unwrap_or_else(|_| unreachable!()),
            b"not an owned symlink"
        );
        fs::remove_dir_all(directory).unwrap_or_else(|_| unreachable!());
    }
}
