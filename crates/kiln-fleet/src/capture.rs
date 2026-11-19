//! Capture of an agent's handed-off commit into an ordinary candidate.
//!
//! The commit is fetched from the session's machine into the repository's
//! trusted authority checkout under a private ref, checked to equal the
//! handed-off commit and to descend from the work's base, and exported as
//! exactly its tree into a fresh directory. The herdr receipt then binds
//! that export to the attempt, and `materialize_candidate` admits it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use kiln_core::implementation::{
    CandidateHandoff, ImplementationAttempt, ImplementationDispatchPlan, ImplementationWork,
};
use kiln_core::types::ContentId;
use kiln_implementer::verification::{
    HerdrCandidateDisposition, HerdrCandidateReceipt, VerificationProfile, VerifierAuthority,
    materialize_candidate,
};
use serde_json::json;

use crate::config::{CaptureConfig, FleetConfig, read_bounded};
use crate::exec::{Child, detail};
use crate::log;

pub const GIT_TIMEOUT_MS: u64 = 120_000;
pub const ARCHIVE_TIMEOUT_MS: u64 = 300_000;
pub const MAX_GIT_OUTPUT_BYTES: usize = 1_024 * 1_024;
/// Largest exported commit archive.
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1_024 * 1_024;
pub const MAX_AUTHORITY_BYTES: u64 = 1_024 * 1_024;
pub const MAX_PROFILE_BYTES: u64 = 4 * 1_024 * 1_024;
/// Deadline of one configured verification or publication command.
pub const COMMAND_TIMEOUT_MS: u64 = 2 * 60 * 60 * 1_000;
pub const MAX_COMMAND_OUTPUT_BYTES: usize = 4 * 1_024 * 1_024;
/// Private namespace holding fetched handoff commits in the authority repository.
pub const HANDOFF_REF_PREFIX: &str = "refs/kiln/fleet-handoffs/";
pub const ARCHIVE_FILE: &str = "candidate.tar";
pub const TREE_DIRECTORY: &str = "tree";
pub const RECEIPT_FILE: &str = "receipt.json";
pub const HANDOFF_FILE: &str = "handoff.json";

/// Fixed environment added to every git child: never prompt, and run ssh
/// in batch mode so a missing key fails instead of waiting.
pub const GIT_FIXED_ENVIRONMENT: [(&str, &str); 2] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_SSH_COMMAND", "ssh -o BatchMode=yes"),
];

/// Where a handoff is fetched from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffSource {
    /// A local path on the coordinator, or `<ssh_target>:<path>`.
    pub location: String,
    pub branch: String,
    pub commit: String,
    pub session: ContentId,
}

/// A fetched, exported and receipted handoff, ready for materialization.
#[derive(Debug)]
pub struct Prepared {
    pub directory: PathBuf,
    pub candidate_root: PathBuf,
    pub receipt: HerdrCandidateReceipt,
    pub receipt_bytes: Vec<u8>,
    pub authority: VerifierAuthority,
    pub authority_path: PathBuf,
    pub profile: VerificationProfile,
    pub profile_path: PathBuf,
    pub handoff_path: PathBuf,
}

/// The capture shell for one configuration.
#[derive(Clone, Debug)]
pub struct Capturer {
    capture: CaptureConfig,
    environment: Vec<(OsString, OsString)>,
}

impl Capturer {
    /// `environment` is the explicit child environment (HOME, PATH and, for
    /// git over ssh, SSH_AUTH_SOCK).
    #[must_use]
    pub fn new(capture: CaptureConfig, environment: Vec<(OsString, OsString)>) -> Self {
        Self {
            capture,
            environment,
        }
    }

    fn git(&self, directory: &Path, timeout_ms: u64) -> Child {
        let mut child = Child::new(
            &self.capture.git,
            directory,
            timeout_ms,
            MAX_GIT_OUTPUT_BYTES,
        );
        for (name, value) in &self.environment {
            child = child.env(name.clone(), value.clone());
        }
        for (name, value) in GIT_FIXED_ENVIRONMENT {
            child = child.env(name, value);
        }
        child.arg("-C").arg(directory)
    }

    fn git_text(&self, directory: &Path, args: &[&str]) -> Result<String, String> {
        self.git(directory, GIT_TIMEOUT_MS)
            .args(args.iter().copied())
            .run()?
            .require_success()?
            .stdout_text()
    }

    // r[impl kiln.fleet.capture]
    /// Fetch, check, export and receipt one handoff.
    ///
    /// # Errors
    /// Returns the first failed step: no authority for the repository, a
    /// failed fetch, a fetched commit other than the handed-off one, a
    /// commit not descending from the base, a failed export or a missing
    /// verification profile.
    pub fn prepare(
        &self,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        plan: &ImplementationDispatchPlan,
        source: &HandoffSource,
    ) -> Result<Prepared, String> {
        let authority_path = self
            .capture
            .repository(&work.repository)
            .ok_or_else(|| format!("no capture authority is configured for {}", work.repository))?
            .authority
            .clone();
        let authority: VerifierAuthority =
            serde_json::from_slice(&read_bounded(&authority_path, MAX_AUTHORITY_BYTES)?)
                .map_err(|error| format!("verifier authority is malformed: {error}"))?;
        let root = authority.repository_root.clone();
        let base = work
            .base
            .as_ref()
            .filter(|base| {
                base.provider_namespace.as_str() == "git" && base.native_kind.as_str() == "commit"
            })
            .map(|base| base.native_id.as_str().to_owned())
            .ok_or("work has no exact git base commit")?;

        let handoff_ref = format!("{HANDOFF_REF_PREFIX}{}", attempt.identity.as_str());
        self.git(&root, GIT_TIMEOUT_MS)
            .args([
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                "--force",
                source.location.as_str(),
                &format!("+refs/heads/{}:{handoff_ref}", source.branch),
            ])
            .run()?
            .require_success()
            .map_err(|error| format!("fetching the handoff failed: {error}"))?;
        let fetched = self.git_text(
            &root,
            &[
                "rev-parse",
                "--verify",
                &format!("{handoff_ref}^{{commit}}"),
            ],
        )?;
        if fetched != source.commit {
            return Err(format!(
                "the session branch is at {fetched}, not the handed-off commit {}",
                source.commit
            ));
        }
        let ancestry = self
            .git(&root, GIT_TIMEOUT_MS)
            .args([
                "merge-base",
                "--is-ancestor",
                base.as_str(),
                source.commit.as_str(),
            ])
            .run()?;
        if !ancestry.succeeded() {
            return Err(format!(
                "the handed-off commit does not descend from the base {base}: {}",
                detail(&ancestry.stderr)
            ));
        }
        let commit_tree = self.git_text(
            &root,
            &["rev-parse", &format!("{}^{{tree}}", source.commit)],
        )?;
        let base_tree = self.git_text(&root, &["rev-parse", &format!("{base}^{{tree}}")])?;
        let disposition = if commit_tree == base_tree {
            HerdrCandidateDisposition::Unchanged
        } else {
            HerdrCandidateDisposition::Produced
        };

        let directory = self.capture.export_root.join(format!(
            "{}-{}",
            attempt.identity.as_str(),
            attempt.fence
        ));
        if directory.exists() {
            std::fs::remove_dir_all(&directory)
                .map_err(|error| format!("cannot clear {}: {error}", directory.display()))?;
        }
        std::fs::create_dir_all(directory.join(TREE_DIRECTORY))
            .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
        let archive = directory.join(ARCHIVE_FILE);
        self.git(&root, ARCHIVE_TIMEOUT_MS)
            .arg("archive")
            .arg("--format=tar")
            .arg(format!("--output={}", archive.display()))
            .arg(&source.commit)
            .run()?
            .require_success()?;
        let size = std::fs::metadata(&archive)
            .map_err(|error| format!("archive is missing: {error}"))?
            .len();
        if size > MAX_ARCHIVE_BYTES {
            return Err(format!(
                "the exported commit exceeds {MAX_ARCHIVE_BYTES} bytes"
            ));
        }
        let tree = directory.join(TREE_DIRECTORY);
        let mut tar = Child::new(
            &self.capture.tar,
            &directory,
            ARCHIVE_TIMEOUT_MS,
            MAX_GIT_OUTPUT_BYTES,
        );
        for (name, value) in &self.environment {
            tar = tar.env(name.clone(), value.clone());
        }
        tar.arg("-x")
            .arg("--no-same-owner")
            .arg("-f")
            .arg(&archive)
            .arg("-C")
            .arg(&tree)
            .run()?
            .require_success()?;
        std::fs::remove_file(&archive).map_err(|error| error.to_string())?;
        let candidate_root =
            std::fs::canonicalize(&tree).map_err(|error| format!("export is missing: {error}"))?;

        let receipt = HerdrCandidateReceipt::for_attempt(
            work,
            attempt,
            plan,
            disposition,
            &source.commit,
            &source.session,
        )?;
        let receipt_bytes = serde_json::to_vec(&receipt).map_err(|error| error.to_string())?;
        std::fs::write(directory.join(RECEIPT_FILE), &receipt_bytes)
            .map_err(|error| format!("cannot write the receipt: {error}"))?;

        let profile_path = self
            .capture
            .verification_profiles
            .join(format!("{}.json", work.verification_profile.as_str()));
        let profile: VerificationProfile =
            serde_json::from_slice(&read_bounded(&profile_path, MAX_PROFILE_BYTES)?)
                .map_err(|error| format!("verification profile is malformed: {error}"))?;
        Ok(Prepared {
            handoff_path: directory.join(HANDOFF_FILE),
            directory,
            candidate_root,
            receipt,
            receipt_bytes,
            authority,
            authority_path,
            profile,
            profile_path,
        })
    }

    /// Materialize a prepared handoff and write the handoff JSON.
    ///
    /// # Errors
    /// Returns the materialization diagnostic.
    pub fn materialize(
        prepared: &Prepared,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        plan: &ImplementationDispatchPlan,
    ) -> Result<CandidateHandoff, String> {
        let handoff = materialize_candidate(
            work,
            attempt,
            plan,
            &prepared.profile,
            &prepared.authority,
            &prepared.candidate_root,
            &prepared.receipt_bytes,
        )
        .map_err(|error| format!("materialization refused the candidate: {error}"))?;
        std::fs::write(
            &prepared.handoff_path,
            serde_json::to_vec_pretty(&handoff).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("cannot write the handoff: {error}"))?;
        Ok(handoff)
    }

    /// Run the configured verification, then publication, in order; each
    /// outcome is logged. Publication runs only after verification passed.
    pub fn run_commands(&self, config: &FleetConfig, values: &Placeholders) {
        let commands = [
            ("verify", self.capture.verify_argv.as_ref()),
            ("publish", self.capture.publish_argv.as_ref()),
        ];
        for (name, argv) in commands {
            let Some(argv) = argv else {
                continue;
            };
            let outcome = self.run_command(config, argv, values);
            let succeeded = outcome.as_ref().is_ok_and(|code| *code == Some(0));
            log::event(
                if succeeded { "info" } else { "warn" },
                "capture.command",
                json!({
                    "command": name,
                    "attempt": values.attempt,
                    "fence": values.fence,
                    "exit_code": outcome.as_ref().ok().copied().flatten(),
                    "error": outcome.as_ref().err(),
                }),
            );
            if !succeeded {
                break;
            }
        }
    }

    fn run_command(
        &self,
        config: &FleetConfig,
        argv: &[String],
        values: &Placeholders,
    ) -> Result<Option<i32>, String> {
        let arguments = substitute(argv, config, values);
        let (program, rest) = arguments.split_first().ok_or("command is empty")?;
        let mut child = Child::new(
            Path::new(program),
            &self.capture.export_root,
            COMMAND_TIMEOUT_MS,
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .args(rest.iter().cloned());
        for (name, value) in &self.environment {
            child = child.env(name.clone(), value.clone());
        }
        Ok(child.run()?.exit_code)
    }
}

/// Values for the argv placeholders.
#[derive(Clone, Debug)]
pub struct Placeholders {
    pub attempt: String,
    pub fence: u64,
    pub verification_profile: PathBuf,
    pub authority: PathBuf,
    pub handoff: PathBuf,
    pub repository: String,
}

/// Substitute whole-argument placeholders.
#[must_use]
pub fn substitute(argv: &[String], config: &FleetConfig, values: &Placeholders) -> Vec<String> {
    argv.iter()
        .map(|argument| match argument.as_str() {
            "{attempt}" => values.attempt.clone(),
            "{fence}" => values.fence.to_string(),
            "{database}" => config.implementer.database.display().to_string(),
            "{enrollment}" => config.implementer.enrollment.display().to_string(),
            "{verification_profile}" => values.verification_profile.display().to_string(),
            "{authority}" => values.authority.display().to_string(),
            "{handoff}" => values.handoff.display().to_string(),
            "{repository}" => values.repository.clone(),
            other => other.to_owned(),
        })
        .collect()
}
