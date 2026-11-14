use super::{VerifierAuthority, process, text};
use kiln_core::implementation::{CandidateHandoff, ImplementationWork};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const GIT_TIMEOUT_MS: u64 = 120_000;

pub(super) struct SourceFacts {
    pub tree: String,
    pub paths: BTreeSet<String>,
}

pub(super) fn admit_authority(
    authority: &VerifierAuthority,
    work: &ImplementationWork,
) -> Result<(), String> {
    if authority.repository != work.repository
        || !authority.repository_root.is_absolute()
        || !authority.scratch_root.is_absolute()
        || fs::canonicalize(&authority.repository_root).map_err(text)? != authority.repository_root
        || fs::canonicalize(&authority.scratch_root).map_err(text)? != authority.scratch_root
        || authority
            .scratch_root
            .starts_with(&authority.repository_root)
    {
        return Err("repository enrollment or isolated scratch path differs".into());
    }
    admit_owned_directory(&authority.repository_root, authority.worker_uid)?;
    admit_owned_directory(&authority.scratch_root, authority.worker_uid)?;
    process::admit_program(&authority.git)
}

#[cfg(target_os = "linux")]
fn admit_owned_directory(path: &Path, worker_uid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    // Read kernel-reported effective/filesystem credentials, not an operator
    // claim or proc directory ownership (which can change with dumpability).
    let status = fs::read_to_string("/proc/self/status").map_err(text)?;
    let mut ids = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or("kernel did not expose verifier credentials")?
        .split_whitespace();
    let _real = ids
        .next()
        .ok_or("real UID missing")?
        .parse::<u32>()
        .map_err(text)?;
    let verifier_uid = ids
        .next()
        .ok_or("effective UID missing")?
        .parse::<u32>()
        .map_err(text)?;
    let _saved = ids
        .next()
        .ok_or("saved UID missing")?
        .parse::<u32>()
        .map_err(text)?;
    let filesystem_uid = ids
        .next()
        .ok_or("filesystem UID missing")?
        .parse::<u32>()
        .map_err(text)?;
    if ids.next().is_some() || verifier_uid != filesystem_uid {
        return Err("ambiguous verifier filesystem credentials".into());
    }
    let mut cursor = Some(path);
    while let Some(directory) = cursor {
        let metadata = fs::symlink_metadata(directory).map_err(text)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() == worker_uid
            || metadata.mode() & 0o022 != 0
            || (metadata.uid() != verifier_uid && metadata.uid() != 0)
        {
            return Err("verifier path is writable by another authority".into());
        }
        cursor = directory.parent();
    }
    if worker_uid == verifier_uid {
        return Err("worker and verifier must use separate effective UIDs".into());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn admit_owned_directory(_path: &Path, _worker_uid: u32) -> Result<(), String> {
    Err("independent verifier credential admission supports Linux only".into())
}

pub(super) fn resolve(
    authority: &VerifierAuthority,
    handoff: &CandidateHandoff,
) -> Result<SourceFacts, String> {
    let base = handoff
        .base
        .as_ref()
        .ok_or("independent verification requires frozen baseline")?;
    for reference in [&handoff.candidate, base] {
        if reference.provider_namespace.as_str() != "git"
            || reference.native_kind.as_str() != "commit"
            || !oid(reference.native_id.as_str())
        {
            return Err("independent verifier requires exact Git commits".into());
        }
        let resolved = git_text(
            authority,
            &authority.repository_root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", reference.native_id.as_str()),
            ],
        )?;
        if resolved != reference.native_id.as_str() {
            return Err("candidate/base resolves to substituted commit".into());
        }
    }
    if handoff.tree.provider_namespace.as_str() != "git"
        || handoff.tree.native_kind.as_str() != "tree"
    {
        return Err("candidate tree is not an exact Git tree".into());
    }
    let tree = git_text(
        authority,
        &authority.repository_root,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{tree}}", handoff.candidate.native_id.as_str()),
        ],
    )?;
    let output = git(
        authority,
        &authority.repository_root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            base.native_id.as_str(),
            handoff.candidate.native_id.as_str(),
            "--",
        ],
    )?;
    let mut paths = BTreeSet::new();
    for bytes in output
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty())
    {
        let path = std::str::from_utf8(bytes).map_err(text)?;
        if !super::safe_relative(path) || !paths.insert(path.into()) {
            return Err("unsafe, non-UTF8 or duplicate source path".into());
        }
    }
    Ok(SourceFacts { tree, paths })
}

pub(super) fn protect(
    authority: &VerifierAuthority,
    handoff: &CandidateHandoff,
    protected: &[String],
    prepared_lifecycle_tree: Option<&str>,
) -> Result<(), String> {
    let base = handoff
        .base
        .as_ref()
        .ok_or("protected requirements need baseline")?;
    for path in protected {
        let baseline = git_text(
            authority,
            &authority.repository_root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}:{path}", base.native_id.as_str()),
            ],
        )?;
        let candidate = git_text(
            authority,
            &authority.repository_root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}:{path}", handoff.candidate.native_id.as_str()),
            ],
        )?;
        let expected = if path == ".cairn" {
            match prepared_lifecycle_tree {
                Some(tree) if oid(tree) => tree,
                Some(_) => return Err("invalid frozen prepared lifecycle tree".into()),
                None => &baseline,
            }
        } else {
            &baseline
        };
        if expected != candidate {
            return Err(format!(
                "candidate changed protected verifier/acceptance path {path}"
            ));
        }
    }
    Ok(())
}

pub(super) fn snapshot(
    authority: &VerifierAuthority,
    commit: &str,
    destination: &Path,
) -> Result<(), String> {
    // A private clone has no shared index, hooks, alternates or worker writable
    // files. Submodules are deliberately not initialized under candidate control.
    git(
        authority,
        &authority.scratch_root,
        &[
            "clone",
            "--no-local",
            "--no-checkout",
            "--no-hardlinks",
            "--",
            authority
                .repository_root
                .to_str()
                .ok_or("non UTF8 repository path")?,
            destination.to_str().ok_or("non UTF8 snapshot path")?,
        ],
    )?;
    git(
        authority,
        destination,
        &["fetch", "--no-tags", "--", "origin", commit],
    )?;
    git(
        authority,
        destination,
        &["checkout", "--detach", "--force", commit, "--"],
    )?;
    let submodules = git(authority, destination, &["ls-files", "--stage"])?;
    if submodules
        .split(|byte| *byte == b'\n')
        .any(|line| line.starts_with(b"160000 "))
    {
        return Err("submodule acquisition is unsupported by admitted local verifier".into());
    }
    Ok(())
}

pub(super) fn clean(authority: &VerifierAuthority, snapshot: &Path) -> Result<bool, String> {
    Ok(git(
        authority,
        snapshot,
        &["status", "--porcelain", "--untracked-files=no"],
    )?
    .is_empty())
}

fn git_text(authority: &VerifierAuthority, root: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git(authority, root, args)?)
        .map(|value| value.trim_end().into())
        .map_err(text)
}
fn git(authority: &VerifierAuthority, root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    process::admit_program(&authority.git)?;
    let mut argv: Vec<String> = vec![
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "-c".into(),
        "protocol.file.allow=always".into(),
        "-c".into(),
        "core.fsmonitor=false".into(),
        "-c".into(),
        "core.attributesFile=/dev/null".into(),
    ];
    argv.extend(args.iter().map(|value| (*value).into()));
    let environment = BTreeMap::from([
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_NO_REPLACE_OBJECTS".into(), "1".into()),
        ("GIT_LITERAL_PATHSPECS".into(), "1".into()),
    ]);
    let output = process::execute(
        &authority.git.path,
        &argv,
        root,
        &environment,
        GIT_TIMEOUT_MS,
    )?;
    if output.status != "passed" {
        return Err(format!("independent Git observation {}", output.status));
    }
    Ok(output.stdout)
}
fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
