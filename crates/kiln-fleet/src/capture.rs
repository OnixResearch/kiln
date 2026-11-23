//! Capture of an agent's handed-off commit into an ordinary candidate.
//!
//! The commit is fetched from the session's machine into the repository's
//! trusted authority checkout under a private ref, checked to equal the
//! handed-off commit and to descend from the work's base, and exported as
//! exactly its tree into a fresh directory. The herdr receipt then binds
//! that export to the attempt, and `materialize_candidate` admits it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use kiln_core::fleet::{GIT_AI_NOTES_REF, GitAiAuthorshipLog, GitAiNoteOutcome, parse_git_ai_note};
use kiln_core::implementation::{
    CandidateHandoff, ImplementationAttempt, ImplementationDispatchPlan, ImplementationWork,
};
use kiln_core::types::ContentId;
use kiln_implementer::verification::{
    HerdrCandidateDisposition, HerdrCandidateReceipt, VerificationProfile, VerifierAuthority,
    attach_git_ai_note_if_tree_matches, export_commit_tree, materialize_candidate,
    read_git_ai_note_bounded,
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
/// Private namespace holding fetched Git AI notes refs when preservation is enabled.
pub const HANDOFF_NOTES_REF_PREFIX: &str = "refs/kiln/fleet-handoff-notes/";
pub const ARCHIVE_FILE: &str = "candidate.tar";
pub const TREE_DIRECTORY: &str = "tree";
pub const RECEIPT_FILE: &str = "receipt.json";
pub const HANDOFF_FILE: &str = "handoff.json";

/// `git archive` writes entries with this umask, so files carry exactly 0644
/// or 0755, the only modes materialization admits.
pub const ARCHIVE_UMASK: &str = "tar.umask=0022";

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

/// State of optional Git AI authorship note preservation prepared for one handoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedGitAiNote {
    Disabled,
    Absent,
    Valid {
        source_commit: String,
        log: GitAiAuthorshipLog,
    },
    Incompatible {
        reason: String,
    },
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
    pub git_ai_note: PreparedGitAiNote,
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
        let tree = directory.join(TREE_DIRECTORY);
        std::fs::create_dir_all(&tree)
            .map_err(|error| format!("cannot create {}: {error}", tree.display()))?;
        // Export directly from the Git object database so tracked
        // `.gitattributes` entries (`export-ignore`, `export-subst`, filters)
        // never transform the committed tree and exact 0644/0755 modes hold
        // regardless of process umask (#102).
        export_commit_tree(&root, &source.commit, &tree)?;
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

        let git_ai_note = if self.capture.preserve_git_ai_notes {
            self.fetch_git_ai_note(&root, attempt, source)
        } else {
            PreparedGitAiNote::Disabled
        };

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
            git_ai_note,
        })
    }

    /// Fetch and validate one bounded `refs/notes/ai` note for the exact agent
    /// commit. Never invents a note and never fails capture: missing notes are
    /// [`PreparedGitAiNote::Absent`] and invalid notes are
    /// [`PreparedGitAiNote::Incompatible`].
    fn fetch_git_ai_note(
        &self,
        root: &Path,
        attempt: &ImplementationAttempt,
        source: &HandoffSource,
    ) -> PreparedGitAiNote {
        let probe = match self
            .git(root, GIT_TIMEOUT_MS)
            .args(["ls-remote", source.location.as_str(), GIT_AI_NOTES_REF])
            .run()
        {
            Ok(output) if output.succeeded() => output.stdout_text().unwrap_or_default(),
            Ok(output) => {
                return PreparedGitAiNote::Incompatible {
                    reason: format!(
                        "probing {GIT_AI_NOTES_REF} failed: {}",
                        detail(&output.stderr)
                    ),
                };
            }
            Err(error) => {
                return PreparedGitAiNote::Incompatible {
                    reason: format!("probing {GIT_AI_NOTES_REF} failed: {error}"),
                };
            }
        };
        if probe.trim().is_empty() {
            return PreparedGitAiNote::Absent;
        }
        let notes_ref = format!("{HANDOFF_NOTES_REF_PREFIX}{}", attempt.identity.as_str());
        match self
            .git(root, GIT_TIMEOUT_MS)
            .args([
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                "--force",
                source.location.as_str(),
                &format!("+{GIT_AI_NOTES_REF}:{notes_ref}"),
            ])
            .run()
        {
            Ok(output) if output.succeeded() => {}
            Ok(output) => {
                return PreparedGitAiNote::Incompatible {
                    reason: format!(
                        "fetching {GIT_AI_NOTES_REF} failed: {}",
                        detail(&output.stderr)
                    ),
                };
            }
            Err(error) => {
                return PreparedGitAiNote::Incompatible {
                    reason: format!("fetching {GIT_AI_NOTES_REF} failed: {error}"),
                };
            }
        }
        let bytes = match read_git_ai_note_bounded(root, &notes_ref, &source.commit) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return PreparedGitAiNote::Absent,
            Err(error) => {
                return PreparedGitAiNote::Incompatible {
                    reason: format!("reading {GIT_AI_NOTES_REF} failed: {error}"),
                };
            }
        };
        match parse_git_ai_note(&bytes, &source.commit) {
            Ok(log) => PreparedGitAiNote::Valid {
                source_commit: source.commit.clone(),
                log,
            },
            Err(error) => PreparedGitAiNote::Incompatible {
                reason: error.reason,
            },
        }
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
        let (handoff, _) = Self::materialize_with_note_outcome(prepared, work, attempt, plan)?;
        Ok(handoff)
    }

    /// Materialize a prepared handoff, write the handoff JSON unchanged, and
    /// apply optional, observation-only Git AI note preservation when enabled.
    ///
    /// # Errors
    /// Returns the materialization diagnostic.
    pub fn materialize_with_note_outcome(
        prepared: &Prepared,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        plan: &ImplementationDispatchPlan,
    ) -> Result<(CandidateHandoff, GitAiNoteOutcome), String> {
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
        let note_outcome = Self::apply_git_ai_note(prepared, handoff.candidate.native_id.as_str());
        if !matches!(note_outcome, GitAiNoteOutcome::Disabled) {
            log::event(
                if matches!(note_outcome, GitAiNoteOutcome::Incompatible { .. }) {
                    "warn"
                } else {
                    "info"
                },
                "capture.git_ai_note",
                json!({
                    "attempt": attempt.identity.as_str(),
                    "fence": attempt.fence,
                    "outcome": note_outcome,
                }),
            );
        }
        Ok((handoff, note_outcome))
    }

    /// Apply optional, observation-only Git AI note preservation for one
    /// prepared handoff onto `candidate_commit` in the authority repository.
    #[must_use]
    pub fn apply_git_ai_note(prepared: &Prepared, candidate_commit: &str) -> GitAiNoteOutcome {
        match &prepared.git_ai_note {
            PreparedGitAiNote::Disabled => GitAiNoteOutcome::Disabled,
            PreparedGitAiNote::Absent => GitAiNoteOutcome::Absent,
            PreparedGitAiNote::Incompatible { reason } => GitAiNoteOutcome::Incompatible {
                reason: reason.clone(),
            },
            PreparedGitAiNote::Valid { source_commit, log } => {
                match attach_git_ai_note_if_tree_matches(
                    &prepared.authority.repository_root,
                    source_commit,
                    candidate_commit,
                    log,
                ) {
                    Ok(outcome) => outcome,
                    Err(error) => GitAiNoteOutcome::Incompatible { reason: error },
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CaptureRepository;
    use kiln_core::fleet::MAX_GIT_AI_NOTE_BYTES;
    use kiln_core::implementation::{
        AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace,
        Enrollment, IMPLEMENTATION_WORK_SCHEMA, INTERACTIVE_AGENT_EFFECT,
        INTERACTIVE_PROVIDER_PROFILE, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
        WorkspaceIsolation,
    };
    use kiln_core::types::{ExternalRef, ProviderProfileId};
    use kiln_implementer::Implementer;
    use std::collections::{BTreeMap, BTreeSet};
    use std::process::Command;

    fn git_bin() -> PathBuf {
        for candidate in ["/run/current-system/sw/bin/git", "/usr/bin/git", "/bin/git"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return path;
            }
        }
        let output = Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"));
        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new(git_bin())
            .arg("-C")
            .arg(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@kiln.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@kiln.invalid")
            .args(args)
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn content(ch: char) -> ContentId {
        ContentId::from_blake3_hex(ch.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    struct CaptureFixture {
        _dir: tempfile::TempDir,
        authority_repo: PathBuf,
        agent_repo: PathBuf,
        agent_commit: String,
        base_commit: String,
        work: ImplementationWork,
        attempt: ImplementationAttempt,
        plan: ImplementationDispatchPlan,
        capture_config: CaptureConfig,
    }

    fn fixture(preserve_git_ai_notes: bool, note_content: Option<&str>) -> CaptureFixture {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let root = dir.path();
        let authority_repo = root.join("authority.git");
        let agent_repo = root.join("agent");
        let scratch_root = root.join("scratch");
        let export_root = root.join("exports");
        let profiles_dir = root.join("profiles");
        std::fs::create_dir_all(&authority_repo).unwrap_or_else(|_| unreachable!());
        std::fs::create_dir_all(&agent_repo).unwrap_or_else(|_| unreachable!());
        std::fs::create_dir_all(&scratch_root).unwrap_or_else(|_| unreachable!());
        std::fs::create_dir_all(&export_root).unwrap_or_else(|_| unreachable!());
        std::fs::create_dir_all(&profiles_dir).unwrap_or_else(|_| unreachable!());

        git(&authority_repo, &["init", "--bare", "-b", "main"]);
        git(&agent_repo, &["init", "-b", "main"]);
        std::fs::create_dir_all(agent_repo.join("src")).unwrap_or_else(|_| unreachable!());
        std::fs::write(agent_repo.join("src/value.txt"), "before\n")
            .unwrap_or_else(|_| unreachable!());
        git(&agent_repo, &["add", "."]);
        git(&agent_repo, &["commit", "-m", "base"]);
        let base_commit = git(&agent_repo, &["rev-parse", "HEAD"]);
        git(
            &agent_repo,
            &["push", authority_repo.to_str().unwrap_or_default(), "main"],
        );

        git(&agent_repo, &["checkout", "-b", "kiln/attempt"]);
        std::fs::write(agent_repo.join("src/value.txt"), "after\n")
            .unwrap_or_else(|_| unreachable!());
        git(&agent_repo, &["add", "."]);
        git(&agent_repo, &["commit", "-m", "agent change"]);
        let agent_commit = git(&agent_repo, &["rev-parse", "HEAD"]);

        if let Some(note) = note_content {
            let rendered = note.replace("{COMMIT}", &agent_commit);
            let note_file = root.join("note.txt");
            std::fs::write(&note_file, rendered).unwrap_or_else(|_| unreachable!());
            git(
                &agent_repo,
                &[
                    "notes",
                    "--ref",
                    GIT_AI_NOTES_REF,
                    "add",
                    "-f",
                    "-F",
                    note_file.to_str().unwrap_or_default(),
                    &agent_commit,
                ],
            );
        }

        let git_path = git_bin();
        let authority = VerifierAuthority {
            repository: "onix/kiln".to_owned(),
            repository_root: authority_repo.clone(),
            scratch_root,
            git: kiln_implementer::verification::FrozenProgram {
                blake3: blake3::hash(
                    &std::fs::read(&git_path).unwrap_or_else(|error| unreachable!("{error}")),
                )
                .to_hex()
                .to_string(),
                path: git_path.clone(),
            },
            worker_uid: u32::MAX,
        };
        let authority_path = root.join("authority.json");
        std::fs::write(
            &authority_path,
            serde_json::to_vec(&authority).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());

        let mut profile = VerificationProfile {
            schema: "kiln.verification-profile.v1".to_owned(),
            repository: "onix/kiln".to_owned(),
            toolchain: content('a'),
            environment_identity: content('b'),
            environment: BTreeMap::new(),
            checks: Vec::new(),
            reproducer: None,
            protected_paths: Vec::new(),
            allow_already_satisfied: false,
            cairn: None,
            proof: None,
        };
        profile.toolchain = profile
            .derived_toolchain_identity()
            .unwrap_or_else(|error| unreachable!("{error}"));
        profile.environment_identity = profile
            .derived_environment_identity()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let profile_id = profile
            .identity()
            .unwrap_or_else(|error| unreachable!("{error}"));
        std::fs::write(
            profiles_dir.join(format!("{}.json", profile_id.as_str())),
            serde_json::to_vec(&profile).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());

        let work = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
            revision: 1,
            actor: "operator".to_owned(),
            repository: "onix/kiln".to_owned(),
            project: "kiln".to_owned(),
            base: Some(
                ExternalRef::new("git", "commit", &base_commit).unwrap_or_else(|_| unreachable!()),
            ),
            title: "Git AI preservation".to_owned(),
            requested_outcome: "preserve valid note".to_owned(),
            acceptance: vec![AcceptanceObligation {
                id: "a".to_owned(),
                statement: "verified".to_owned(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["src".to_owned()],
            permitted_effects: BTreeSet::from([
                "write_source".to_owned(),
                INTERACTIVE_AGENT_EFFECT.to_owned(),
            ]),
            verification_profile: profile_id,
            budget: WorkBudget {
                attempts: 1,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#106")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#106".to_owned(),
                version: content('1'),
            }],
        };
        let enrollment = Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_owned()]),
            actors: BTreeSet::from(["operator".to_owned()]),
        };
        let mut store =
            Implementer::open_with_local_operator(&root.join("impl.db"), enrollment, "op")
                .unwrap_or_else(|error| unreachable!("{error}"));
        store
            .admit(&work, None)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let attempt = store
            .claim_next(100, 60_000)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .unwrap_or_else(|| unreachable!());
        let plan = ImplementationDispatchPlan {
            schema: DISPATCH_PLAN_SCHEMA.to_owned(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            workspace: DispatchWorkspace {
                identity: content('c'),
                isolation: WorkspaceIsolation::InteractiveWorktree,
                network: true,
                credentials: true,
                projection: None,
            },
            provider_profile: ProviderProfileId::new(INTERACTIVE_PROVIDER_PROFILE)
                .unwrap_or_else(|_| unreachable!()),
            permitted_paths: work.permitted_paths.clone(),
            permitted_effects: work.permitted_effects.clone(),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 60,
            token_budget: 1_000,
            invocation_budget: 1,
            expected_output: content('d'),
            execution: None,
        };
        store
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 100, &plan)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let attempt = store
            .attempt(&attempt.identity)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .unwrap_or_else(|| unreachable!());

        let capture_config = CaptureConfig {
            git: git_bin(),
            tar: PathBuf::from("/bin/tar"),
            export_root,
            verification_profiles: profiles_dir,
            repositories: BTreeMap::from([(
                "onix/kiln".to_owned(),
                CaptureRepository {
                    authority: authority_path,
                },
            )]),
            verify_argv: None,
            publish_argv: None,
            preserve_git_ai_notes,
        };
        CaptureFixture {
            _dir: dir,
            authority_repo,
            agent_repo,
            agent_commit,
            base_commit,
            work,
            attempt,
            plan,
            capture_config,
        }
    }

    fn run_capture(fx: &CaptureFixture) -> (Prepared, String, GitAiNoteOutcome) {
        let capturer = Capturer::new(fx.capture_config.clone(), Vec::new());
        let source = HandoffSource {
            location: fx.agent_repo.to_str().unwrap_or_default().to_owned(),
            branch: "kiln/attempt".to_owned(),
            commit: fx.agent_commit.clone(),
            session: content('e'),
        };
        let prepared = capturer
            .prepare(&fx.work, &fx.attempt, &fx.plan, &source)
            .unwrap_or_else(|error| unreachable!("{error}"));
        // Create a fresh verifier-authored commit in authority_repo with the
        // exact exported tree (distinct SHA from the agent commit, mirroring
        // materialize_candidate's rematerialized commit).
        let tree_id = git(
            &fx.authority_repo,
            &["rev-parse", &format!("{}^{{tree}}", fx.agent_commit)],
        );
        let candidate_commit = git(
            &fx.authority_repo,
            &[
                "commit-tree",
                &tree_id,
                "-p",
                &fx.base_commit,
                "-m",
                "kiln verifier rematerialized candidate",
            ],
        );
        assert_ne!(candidate_commit, fx.agent_commit);
        let outcome = Capturer::apply_git_ai_note(&prepared, &candidate_commit);
        (prepared, candidate_commit, outcome)
    }

    const VALID_NOTE: &str = "src/value.txt\n  abc12345 1\n---\n{\n  \"schema_version\": \"authorship/3.0.0\",\n  \"base_commit_sha\": \"{COMMIT}\",\n  \"prompts\": {\n    \"abc12345\": { \"agent_id\": { \"tool\": \"claude\", \"id\": \"s1\", \"model\": \"claude-opus\" } }\n  }\n}\n";

    #[test]
    fn feature_off_does_not_fetch_or_attach_git_ai_notes_and_keeps_handoff_identical() {
        let fx_off = fixture(false, Some(VALID_NOTE));
        let (prepared_off, candidate_off, outcome_off) = run_capture(&fx_off);
        assert_eq!(outcome_off, GitAiNoteOutcome::Disabled);
        assert_eq!(
            read_git_ai_note_bounded(&fx_off.authority_repo, GIT_AI_NOTES_REF, &candidate_off)
                .unwrap_or_else(|error| unreachable!("{error}")),
            None
        );

        // With feature on, valid note is preserved on candidate commit while
        // receipt still names original agent commit and receipt bytes match.
        let fx_on = fixture(true, Some(VALID_NOTE));
        let (prepared_on, candidate_on, outcome_on) = run_capture(&fx_on);
        assert_eq!(prepared_on.receipt.candidate_commit, fx_on.agent_commit);
        assert_eq!(
            outcome_on,
            GitAiNoteOutcome::Preserved {
                source_commit: fx_on.agent_commit.clone(),
                candidate_commit: candidate_on.clone(),
                files: 1,
                entries: 1,
            }
        );
        let attached =
            read_git_ai_note_bounded(&fx_on.authority_repo, GIT_AI_NOTES_REF, &candidate_on)
                .unwrap_or_else(|error| unreachable!("{error}"))
                .unwrap_or_else(|| unreachable!("expected preserved note on candidate commit"));
        let reparsed = parse_git_ai_note(&attached, &candidate_on)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(reparsed.attestations.len(), 1);
        assert_eq!(
            reparsed.attestations.first().map(|file| file.path.as_str()),
            Some("src/value.txt")
        );
        assert_eq!(reparsed.base_commit_sha, candidate_on);
        assert_eq!(
            prepared_off.receipt.disposition,
            prepared_on.receipt.disposition
        );
    }

    #[test]
    fn absent_or_incompatible_notes_never_attach_and_never_block_rematerialization() {
        // Missing refs/notes/ai -> Absent.
        let fx_absent = fixture(true, None);
        let (_, candidate_absent, outcome_absent) = run_capture(&fx_absent);
        assert_eq!(outcome_absent, GitAiNoteOutcome::Absent);
        assert_eq!(
            read_git_ai_note_bounded(
                &fx_absent.authority_repo,
                GIT_AI_NOTES_REF,
                &candidate_absent
            )
            .unwrap_or_else(|error| unreachable!("{error}")),
            None
        );

        // Wrong schema, wrong bound commit, unparseable, oversized -> Incompatible.
        let wrong_schema = VALID_NOTE.replace("authorship/3.0.0", "authorship/2.0.0");
        let wrong_commit = VALID_NOTE.replace("{COMMIT}", &"9".repeat(40));
        let oversized = "a".repeat(MAX_GIT_AI_NOTE_BYTES + 16);
        for (label, note) in [
            ("wrong schema", wrong_schema.as_str()),
            ("wrong commit", wrong_commit.as_str()),
            ("unparseable", "not a git ai note"),
            ("oversized", oversized.as_str()),
        ] {
            let fx = fixture(true, Some(note));
            let (_, candidate, outcome) = run_capture(&fx);
            assert!(
                matches!(outcome, GitAiNoteOutcome::Incompatible { .. }),
                "{label}: expected Incompatible, got {outcome:?}"
            );
            assert_eq!(
                read_git_ai_note_bounded(&fx.authority_repo, GIT_AI_NOTES_REF, &candidate)
                    .unwrap_or_else(|error| unreachable!("{error}")),
                None,
                "{label}: incompatible note must not attach"
            );
        }

        // Tree mismatch guard refuses attaching when trees differ.
        let fx = fixture(true, Some(VALID_NOTE));
        let (prepared, _, _) = run_capture(&fx);
        let PreparedGitAiNote::Valid { log, .. } = prepared.git_ai_note else {
            unreachable!("expected valid prepared note");
        };
        let mismatched = attach_git_ai_note_if_tree_matches(
            &fx.authority_repo,
            &fx.agent_commit,
            &fx.base_commit,
            &log,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(matches!(mismatched, GitAiNoteOutcome::Incompatible { .. }));
        assert_eq!(
            read_git_ai_note_bounded(&fx.authority_repo, GIT_AI_NOTES_REF, &fx.base_commit)
                .unwrap_or_else(|error| unreachable!("{error}")),
            None
        );
    }
}
