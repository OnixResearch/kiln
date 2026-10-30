//! Exact-commit acquisition from a knot (shell).
//!
//! The Git CLI fetches exactly the requested object id (depth 1) into a
//! private bare object store of the matching SHA-1 or SHA-256 format, verifies
//! that it is a commit with that id, and reads `.tangled/workflows` from it.
//! Every Git process runs under `bounded-exec` with a cleared environment, a
//! deadline and output bounds. No remote is saved and no credentials are used.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use kiln_core::monorepo::{ChangedFallback, ChangedFiles};
use kiln_provider_api::PortResult;

use crate::error::{domain, infrastructure, retryable};
use crate::identity::{ObjectFormat, ObjectId, valid_reference};
use crate::pipeline::{AcquiredSource, WorkflowFile};
use crate::workflows::{WORKFLOW_DIRECTORY, workflow_id};

/// Poll interval for child processes.
const POLL_INTERVAL_MS: u64 = 10;
/// Grace period between terminate and kill.
const TEARDOWN_MS: u64 = 1_000;
/// Diagnostic bytes kept from Git's stderr (never shown to callers).
const STDERR_BYTES: usize = 4096;
/// Bytes per `ls-tree -z` entry beyond the path (mode, type, object id).
const TREE_ENTRY_OVERHEAD: usize = 96;
/// Bytes per `ls-remote` line beyond the reference name.
const REMOTE_LINE_OVERHEAD: usize = 72;
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const GIT_MODE_BLOB: &str = "100644";
const GIT_MODE_EXECUTABLE: &str = "100755";

#[derive(Clone, Copy, Debug)]
pub struct AcquireRequest<'a> {
    pub repo_did: &'a str,
    pub clone_url: &'a str,
    pub commit: &'a ObjectId,
    /// Previous head for changed-file facts.
    pub base: Option<&'a ObjectId>,
}

/// Source acquisition port of the Tangled adapter.
pub trait SourcePort {
    /// Fetch and verify exactly `request.commit`, and read its workflows.
    fn acquire(&mut self, request: &AcquireRequest<'_>) -> PortResult<AcquiredSource>;
    /// Resolve a branch, tag or `HEAD` to one exact object id; `None` when the
    /// knot does not advertise it.
    fn resolve(
        &mut self,
        repo_did: &str,
        clone_url: &str,
        reference: &str,
    ) -> PortResult<Option<ObjectId>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitLimits {
    pub timeout_ms: u64,
    pub maximum_workflow_files: usize,
    pub maximum_workflow_bytes: usize,
    pub maximum_changed_paths: usize,
    pub maximum_changed_path_bytes: usize,
}

#[derive(Debug)]
pub struct GitCliSource {
    git: PathBuf,
    root: PathBuf,
    limits: GitLimits,
    allow_local: bool,
}

impl GitCliSource {
    // r[impl kiln.tangled.source]
    pub fn new(git: PathBuf, root: PathBuf, limits: GitLimits) -> PortResult<Self> {
        Self::build(git, root, limits, false)
    }

    /// Fixture source accepting `file://` clone URLs. Tests only.
    #[cfg(all(test, feature = "git-fixtures"))]
    pub(crate) fn fixture(git: PathBuf, root: PathBuf, limits: GitLimits) -> PortResult<Self> {
        Self::build(git, root, limits, true)
    }

    fn build(
        git: PathBuf,
        root: PathBuf,
        limits: GitLimits,
        allow_local: bool,
    ) -> PortResult<Self> {
        if !git.is_absolute() || !root.is_absolute() {
            return Err(domain(
                "tangled_source_paths",
                "source",
                "Git program and source root must be absolute",
            ));
        }
        private_directory(&root)?;
        Ok(Self {
            git,
            root,
            limits,
            allow_local,
        })
    }

    fn admit_url(&self, url: &str) -> PortResult<()> {
        let local = self.allow_local && url.starts_with("file:///");
        if (url.starts_with("https://") || local)
            && !url.bytes().any(|byte| byte <= b' ' || byte == 127)
        {
            Ok(())
        } else {
            Err(domain(
                "tangled_source_url",
                "clone_url",
                "Clone URL is not an admitted HTTPS URL",
            ))
        }
    }

    fn store(&self, repo_did: &str, format: ObjectFormat) -> PortResult<PathBuf> {
        let key = blake3::hash(repo_did.as_bytes()).to_hex();
        let path = self.root.join(format!(
            "{}-{}",
            key.as_str().get(..32).unwrap_or(key.as_str()),
            format.as_str()
        ));
        if !path.join("HEAD").exists() {
            private_directory(&path)?;
            self.git(
                &path,
                &[
                    "init",
                    "--quiet",
                    "--bare",
                    &format!("--object-format={}", format.as_str()),
                    ".",
                ],
                STDERR_BYTES,
            )?;
        }
        Ok(path)
    }

    fn git(&self, directory: &Path, args: &[&str], output_bytes: usize) -> PortResult<Vec<u8>> {
        match self.git_raw(directory, args, output_bytes)? {
            (Disposition::Succeeded, stdout) => Ok(stdout),
            (Disposition::TimedOut, _) => Err(retryable(
                "tangled_git_timeout",
                "Git exceeded its deadline",
            )),
            (Disposition::OutputLimitExceeded(_), _) => Err(domain(
                "tangled_git_output",
                "source",
                "Git output exceeded its bound",
            )),
            _ => Err(retryable("tangled_git_failed", "Git command failed")),
        }
    }

    fn git_raw(
        &self,
        directory: &Path,
        args: &[&str],
        output_bytes: usize,
    ) -> PortResult<(Disposition, Vec<u8>)> {
        let mut environment = vec![
            (OsString::from("HOME"), self.root.clone().into_os_string()),
            (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            (
                OsString::from("GIT_CONFIG_GLOBAL"),
                OsString::from("/dev/null"),
            ),
            (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
            (OsString::from("LC_ALL"), OsString::from("C")),
        ];
        // TLS trust anchors are the only ambient inputs forwarded to Git.
        for name in ["SSL_CERT_FILE", "NIX_SSL_CERT_FILE"] {
            if let Some(value) = std::env::var_os(name) {
                environment.push((OsString::from(name), value));
            }
        }
        let request = RunRequest {
            command: CommandSpec {
                program: self.git.clone(),
                args: args.iter().map(OsString::from).collect(),
                current_dir: directory.to_path_buf(),
                environment_mode: EnvironmentMode::Clear,
                environment,
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms: self.limits.timeout_ms,
                stdin_max_bytes: 1,
                stdout_max_bytes: output_bytes.max(1),
                stderr_max_bytes: STDERR_BYTES,
                poll_interval_ms: POLL_INTERVAL_MS.min(self.limits.timeout_ms),
                teardown_timeout_ms: TEARDOWN_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, false).map_err(|_| {
                infrastructure("tangled_git_policy", "Git outcome policy is invalid")
            })?,
        };
        let output = run(request)
            .map_err(|_| infrastructure("tangled_git_spawn", "Git could not be observed"))?;
        Ok((output.disposition, output.stdout.bytes))
    }

    fn fetch(&self, store: &Path, url: &str, commit: &ObjectId) -> PortResult<()> {
        self.git(
            store,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "--depth=1",
                url,
                commit.as_str(),
            ],
            STDERR_BYTES,
        )
        .map(|_| ())
    }

    fn verify_commit(&self, store: &Path, commit: &ObjectId) -> PortResult<()> {
        let peeled = format!("{}^{{commit}}", commit.as_str());
        let output = self.git(
            store,
            &["rev-parse", "--verify", "--quiet", &peeled],
            STDERR_BYTES,
        )?;
        if output.strip_suffix(b"\n").unwrap_or(&output) == commit.as_str().as_bytes() {
            Ok(())
        } else {
            Err(domain(
                "tangled_source_identity",
                "commit",
                "Fetched object is not the requested commit",
            ))
        }
    }

    fn workflows(&self, store: &Path, commit: &ObjectId) -> PortResult<Vec<WorkflowFile>> {
        let limits = self.limits;
        let listing_bytes = limits.maximum_workflow_files.saturating_mul(
            TREE_ENTRY_OVERHEAD + crate::workflows::MAXIMUM_NAME_BYTES + WORKFLOW_DIRECTORY.len(),
        );
        let tree = format!(
            "{}:{}",
            commit.as_str(),
            WORKFLOW_DIRECTORY.trim_end_matches('/')
        );
        let (disposition, listing) =
            self.git_raw(store, &["ls-tree", "-z", &tree], listing_bytes)?;
        match disposition {
            Disposition::Succeeded => {}
            // No `.tangled/workflows` tree at this commit.
            Disposition::ExitFailed => return Ok(Vec::new()),
            Disposition::OutputLimitExceeded(_) => {
                return Err(domain(
                    "tangled_workflow_count",
                    "workflows",
                    "Too many workflow files",
                ));
            }
            _ => {
                return Err(retryable(
                    "tangled_git_failed",
                    "Git could not list workflows",
                ));
            }
        }
        let mut files = Vec::new();
        for entry in listing
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
        {
            let entry = std::str::from_utf8(entry).map_err(|_| {
                domain(
                    "tangled_workflow_path",
                    "workflows",
                    "Workflow path is not UTF-8",
                )
            })?;
            let (meta, name) = entry.split_once('\t').ok_or_else(|| {
                domain(
                    "tangled_git_output",
                    "workflows",
                    "Unexpected ls-tree output",
                )
            })?;
            let mut fields = meta.split(' ');
            let (Some(mode), Some(kind), Some(object)) =
                (fields.next(), fields.next(), fields.next())
            else {
                return Err(domain(
                    "tangled_git_output",
                    "workflows",
                    "Unexpected ls-tree output",
                ));
            };
            let path = format!("{WORKFLOW_DIRECTORY}{name}");
            // Only regular files with a workflow name are definitions; symlinks
            // and subtrees are not read.
            if kind != "blob"
                || !matches!(mode, GIT_MODE_BLOB | GIT_MODE_EXECUTABLE)
                || workflow_id(&path).is_none()
            {
                continue;
            }
            if files.len() == limits.maximum_workflow_files {
                return Err(domain(
                    "tangled_workflow_count",
                    "workflows",
                    "Too many workflow files",
                ));
            }
            let object = ObjectId::parse(object)
                .ok_or_else(|| domain("tangled_git_output", "workflows", "Unexpected object id"))?;
            let bytes = self.git(
                store,
                &["cat-file", "blob", object.as_str()],
                limits.maximum_workflow_bytes,
            )?;
            files.push(WorkflowFile { path, bytes });
        }
        Ok(files)
    }

    fn changed(&self, store: &Path, url: &str, base: &ObjectId, commit: &ObjectId) -> ChangedFiles {
        if self.fetch(store, url, base).is_err() {
            return ChangedFiles::Fallback(ChangedFallback::ReadError);
        }
        let limit = self.limits.maximum_changed_path_bytes;
        let result = self.git_raw(
            store,
            &[
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                base.as_str(),
                commit.as_str(),
            ],
            limit,
        );
        let Ok((disposition, output)) = result else {
            return ChangedFiles::Fallback(ChangedFallback::ReadError);
        };
        match disposition {
            Disposition::Succeeded => {}
            Disposition::OutputLimitExceeded(_) => {
                return ChangedFiles::Fallback(ChangedFallback::Truncated);
            }
            _ => return ChangedFiles::Fallback(ChangedFallback::ReadError),
        }
        let mut paths = BTreeSet::new();
        for path in output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let Ok(path) = std::str::from_utf8(path) else {
                return ChangedFiles::Fallback(ChangedFallback::ReadError);
            };
            if paths.len() == self.limits.maximum_changed_paths {
                return ChangedFiles::Fallback(ChangedFallback::Truncated);
            }
            paths.insert(path.to_string());
        }
        ChangedFiles::Exact(paths)
    }
}

impl SourcePort for GitCliSource {
    // r[impl kiln.tangled.source]
    fn acquire(&mut self, request: &AcquireRequest<'_>) -> PortResult<AcquiredSource> {
        self.admit_url(request.clone_url)?;
        let format = request.commit.format();
        if request.base.is_some_and(|base| base.format() != format) {
            return Err(domain(
                "tangled_source_format",
                "base",
                "Base and commit object formats differ",
            ));
        }
        let store = self.store(request.repo_did, format)?;
        self.fetch(&store, request.clone_url, request.commit)?;
        self.verify_commit(&store, request.commit)?;
        let files = self.workflows(&store, request.commit)?;
        let changed = match request.base {
            Some(base) => self.changed(&store, request.clone_url, base, request.commit),
            None => ChangedFiles::Fallback(ChangedFallback::NoBase),
        };
        Ok(AcquiredSource {
            commit: request.commit.clone(),
            files,
            changed,
        })
    }

    fn resolve(
        &mut self,
        repo_did: &str,
        clone_url: &str,
        reference: &str,
    ) -> PortResult<Option<ObjectId>> {
        self.admit_url(clone_url)?;
        let candidates: Vec<String> = if reference == "HEAD" {
            vec!["HEAD".to_string()]
        } else if reference.starts_with("refs/") {
            vec![reference.to_string()]
        } else {
            vec![
                format!("refs/heads/{reference}"),
                format!("refs/tags/{reference}"),
            ]
        };
        if candidates
            .iter()
            .any(|candidate| candidate != "HEAD" && !valid_reference(candidate))
        {
            return Ok(None);
        }
        let _ = repo_did;
        let mut args = vec!["ls-remote", "--quiet", clone_url];
        args.extend(candidates.iter().map(String::as_str));
        let bound = candidates
            .iter()
            .map(|candidate| candidate.len() + REMOTE_LINE_OVERHEAD)
            .sum::<usize>()
            * 2;
        let output = self.git(&self.root, &args, bound)?;
        let text = std::str::from_utf8(&output)
            .map_err(|_| domain("tangled_git_output", "ref", "Unexpected ls-remote output"))?;
        let mut found = None;
        for candidate in &candidates {
            for line in text.lines() {
                if let Some((object, name)) = line.split_once('\t') {
                    // Peeled tag lines (`^{}`) name the commit a tag points at.
                    let peeled = name
                        .strip_suffix("^{}")
                        .is_some_and(|base| base == candidate);
                    if name == candidate || peeled {
                        let object = ObjectId::parse(object).ok_or_else(|| {
                            domain("tangled_git_output", "ref", "Unexpected object id")
                        })?;
                        if peeled || found.is_none() {
                            found = Some(object);
                        }
                    }
                }
            }
            if found.is_some() {
                return Ok(found);
            }
        }
        Ok(found)
    }
}

fn private_directory(path: &Path) -> PortResult<()> {
    let failure = || {
        infrastructure(
            "tangled_source_directory",
            "Source directory is not private",
        )
    };
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIRECTORY_MODE)
            .create(path)
            .map_err(|_| failure())?;
    }
    let metadata = fs::metadata(path).map_err(|_| failure())?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(failure());
    }
    Ok(())
}
