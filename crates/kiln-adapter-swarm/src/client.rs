//! Bounded client for `iroh-news`, the renews NNTP-over-iroh shell.
//!
//! The client runs the operator-pinned executable with a cleared environment and
//! an exact argv. It never receives a password, endpoint key or ticket through
//! Kiln: `iroh-news` resolves its own credentials through SecretSpec or its
//! state directory. Only stdout JSON is parsed; stderr is retained as a bounded
//! redacted detail.

use std::ffi::OsString;

use bounded_exec::{
    CommandSpec, Completion, Disposition, EnvironmentMode, ExecutionLimits, Input, Operation,
    OutcomePolicy, RunError, RunRequest, TerminationScope, run,
};
use kiln_core::implementation::swarm::{
    ArticleView, ClaimArticle, MAX_MESSAGE_ID_BYTES, admit_body, parse_rfc2822_ms, parse_subject,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::config::SwarmConfig;

pub const MAX_DETAIL_BYTES: usize = 192;

/// One article as printed by `iroh-news read`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
pub struct Article {
    pub number: u64,
    pub message_id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    #[serde(default)]
    pub references: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
}

impl Article {
    /// Reduce the article to the fields the overlap rule may inspect.
    #[must_use]
    pub fn view(&self) -> Option<ArticleView> {
        Some(ArticleView {
            message_id: self.message_id.clone(),
            from: self.from.clone(),
            subject: self.subject.clone(),
            date_unix_ms: parse_rfc2822_ms(&self.date)?,
            references: self
                .references
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        })
    }
}

/// One group as printed by `iroh-news groups`. Unknown fields are tolerated.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
pub struct GroupEntry {
    pub name: String,
    /// Highest article number, `0` when the group is empty.
    pub high: u64,
    /// Lowest article number, `0` when the group is empty.
    pub low: u64,
    /// Posting status (`y`, `n` or `m`).
    pub posting: String,
}

/// Outcome of one post attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostOutcome {
    Accepted,
    Rejected,
    Unknown,
    Unavailable,
}

impl PostOutcome {
    /// Stable label for journals and operator output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One post result. `detail` is bounded, control-free and never credential text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PostReceipt {
    pub outcome: PostOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

impl PostReceipt {
    fn new(outcome: PostOutcome, detail: Option<String>) -> Self {
        Self {
            outcome,
            detail,
            message_id: None,
        }
    }

    fn accepted(stdout: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct Ack {
            posted: bool,
            message_id: Option<String>,
        }
        let message_id = serde_json::from_slice::<Ack>(stdout)
            .ok()
            .filter(|ack| ack.posted)
            .and_then(|ack| ack.message_id)
            .filter(|id| {
                id.len() <= MAX_MESSAGE_ID_BYTES
                    && id
                        .strip_prefix('<')
                        .and_then(|id| id.strip_suffix('>'))
                        .is_some_and(|id| {
                            id.contains('@')
                                && id.bytes().all(|byte| {
                                    byte.is_ascii_graphic() && !matches!(byte, b'<' | b'>')
                                })
                        })
            });
        Self {
            message_id,
            ..Self::new(PostOutcome::Accepted, None)
        }
    }
}

/// Failure to read claims. A failed read is unavailable, not an empty group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmFailure {
    pub detail: String,
}

/// A configured swarm client.
// r[impl kiln.implementation.swarm.transport]
#[derive(Debug, Clone)]
pub struct SwarmClient {
    config: SwarmConfig,
}

impl SwarmClient {
    /// Admit one configuration.
    ///
    /// # Errors
    /// Returns the configuration diagnostic when validation fails.
    pub fn new(config: SwarmConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Return the admitted configuration.
    #[must_use]
    pub const fn config(&self) -> &SwarmConfig {
        &self.config
    }

    /// Read up to `limit` articles after `since`.
    ///
    /// # Errors
    /// Returns [`SwarmFailure`] when the client cannot run, the server refuses,
    /// or stdout is not the expected bounded JSON. An unavailable read is never
    /// treated as an empty group.
    pub fn read(&self, since: Option<u64>, limit: usize) -> Result<Vec<Article>, SwarmFailure> {
        let limit = limit.clamp(1, self.config.read_limit);
        let mut args = vec![
            OsString::from("read"),
            OsString::from(&self.config.group),
            OsString::from("--limit"),
            OsString::from(limit.to_string()),
        ];
        if let Some(since) = since {
            args.extend([OsString::from("--since"), OsString::from(since.to_string())]);
        }
        self.run_json(args, "read", "a bounded article array")
    }

    /// List the server's active groups through `iroh-news groups`.
    ///
    /// # Errors
    /// Returns [`SwarmFailure`] when the client cannot run, the server refuses,
    /// or stdout is not a bounded group array.
    pub fn groups(&self) -> Result<Vec<GroupEntry>, SwarmFailure> {
        self.run_json(
            vec![OsString::from("groups")],
            "groups",
            "a bounded group array",
        )
    }

    // r[impl kiln.implementation.swarm.transport]
    /// Read the newest `limit` articles of the configured group.
    ///
    /// The read starts after the group's high-water mark minus `limit`
    /// (saturating at zero), so overlap and message-id lookups see the newest
    /// articles rather than the oldest retained ones.
    ///
    /// # Errors
    /// Returns [`SwarmFailure`] when the group listing fails, does not name the
    /// configured group, or the read itself fails. A failed listing is never an
    /// empty group.
    pub fn read_recent(&self, limit: usize) -> Result<Vec<Article>, SwarmFailure> {
        let limit = limit.clamp(1, self.config.read_limit);
        let groups = self.groups()?;
        let high = groups
            .iter()
            .find(|group| group.name == self.config.group)
            .map(|group| group.high)
            .ok_or_else(|| SwarmFailure {
                detail: "swarm group is not listed by the server".to_owned(),
            })?;
        let window = u64::try_from(limit).unwrap_or(u64::MAX);
        self.read(Some(high.saturating_sub(window)), limit)
    }

    fn run_json<T: DeserializeOwned>(
        &self,
        args: Vec<OsString>,
        operation: &str,
        shape: &str,
    ) -> Result<T, SwarmFailure> {
        let request = self.request(args, Input::Null, 1)?;
        let output = run(request).map_err(|error| SwarmFailure {
            detail: run_detail(&error),
        })?;
        if output.completion != Completion::Exited || output.disposition != Disposition::Succeeded {
            return Err(SwarmFailure {
                detail: classify_failure(&output, operation),
            });
        }
        let text = std::str::from_utf8(&output.stdout.bytes).map_err(|_| SwarmFailure {
            detail: "swarm response is not UTF-8".to_owned(),
        })?;
        serde_json::from_str::<T>(text).map_err(|error| SwarmFailure {
            detail: format!(
                "swarm response is not {shape} ({error}, {} bytes)",
                text.len()
            ),
        })
    }

    // r[impl kiln.implementation.swarm.outcomes]
    /// Post one claim or release article.
    ///
    /// A local refusal produces `rejected`; a spawn failure `unavailable`; a
    /// deadline or lost response `unknown`, because the server may have stored
    /// the article already.
    #[must_use]
    pub fn post(&self, subject: &str, references: Option<&str>, body: &str) -> PostReceipt {
        if matches!(parse_subject(subject), ClaimArticle::Other) {
            return PostReceipt::new(
                PostOutcome::Rejected,
                Some("invalid claim subject".to_owned()),
            );
        }
        if let Err(error) = admit_body(body) {
            return PostReceipt::new(PostOutcome::Rejected, Some(error));
        }
        let mut args = vec![
            OsString::from("post"),
            OsString::from("--group"),
            OsString::from(&self.config.group),
            OsString::from("--subject"),
            OsString::from(subject),
        ];
        if let Some(references) = references {
            args.extend([OsString::from("--references"), OsString::from(references)]);
        }
        let stdin_max = body.len().saturating_add(2);
        let request = match self.request(args, Input::Bytes(body.as_bytes().to_vec()), stdin_max) {
            Ok(request) => request,
            Err(failure) => {
                return PostReceipt::new(PostOutcome::Unavailable, Some(failure.detail));
            }
        };
        match run(request) {
            Ok(output) => match output.disposition {
                Disposition::Succeeded => PostReceipt::accepted(&output.stdout.bytes),
                Disposition::ExitFailed => {
                    PostReceipt::new(PostOutcome::Rejected, detail(&output.stderr.bytes))
                }
                Disposition::TimedOut
                | Disposition::Cancelled
                | Disposition::OutputLimitExceeded(_) => PostReceipt::new(
                    PostOutcome::Unknown,
                    Some("swarm post outcome unknown".to_owned()),
                ),
            },
            Err(error) => {
                let outcome = if spawn_failed(&error) {
                    PostOutcome::Unavailable
                } else {
                    PostOutcome::Unknown
                };
                PostReceipt::new(outcome, Some(run_detail(&error)))
            }
        }
    }

    fn request(
        &self,
        args: Vec<OsString>,
        input: Input,
        stdin_max_bytes: usize,
    ) -> Result<RunRequest, SwarmFailure> {
        let mut full = vec![
            OsString::from("--state-dir"),
            self.config.state_dir.as_os_str().to_owned(),
        ];
        if let Some(config) = &self.config.config_file {
            full.extend([OsString::from("--config"), config.as_os_str().to_owned()]);
        }
        if let Some(server) = &self.config.server {
            full.extend([OsString::from("--server"), OsString::from(server)]);
        }
        if self.config.no_relays {
            full.push(OsString::from("--no-relays"));
        }
        full.extend(args);
        let policy = OutcomePolicy::new(vec![0], true, true).map_err(|_| SwarmFailure {
            detail: "swarm execution policy is invalid".to_owned(),
        })?;
        Ok(RunRequest {
            command: CommandSpec {
                program: self.config.executable.clone(),
                args: full,
                current_dir: self.config.state_dir.clone(),
                environment_mode: EnvironmentMode::Clear,
                environment: self
                    .config
                    .child_environment()
                    .into_iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                    .collect(),
                input,
            },
            limits: ExecutionLimits {
                timeout_ms: self.config.timeout_ms,
                stdin_max_bytes,
                stdout_max_bytes: self.config.max_output_bytes,
                stderr_max_bytes: self.config.max_output_bytes,
                poll_interval_ms: 20,
                teardown_timeout_ms: 2_000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: policy,
        })
    }
}

fn classify_failure(output: &bounded_exec::ExecutionOutput, operation: &str) -> String {
    if output.completion == Completion::TimedOut {
        return format!("swarm {operation} timed out");
    }
    if output.completion == Completion::Cancelled {
        return format!("swarm {operation} cancelled");
    }
    if matches!(output.disposition, Disposition::OutputLimitExceeded(_)) {
        return "swarm response exceeds its bound".to_owned();
    }
    let detail = detail(&output.stderr.bytes);
    detail.unwrap_or_else(|| format!("swarm {operation} failed"))
}

fn spawn_failed(error: &RunError) -> bool {
    matches!(
        error,
        RunError::EmptyProgram
            | RunError::ProgramNotAbsolute
            | RunError::EmptyCurrentDirectory
            | RunError::CurrentDirectoryNotAbsolute
            | RunError::InvalidLimits(_)
            | RunError::DuplicateEnvironmentName
            | RunError::InputLimitExceeded
            | RunError::Io {
                operation: Operation::Spawn,
                ..
            }
    )
}

fn run_detail(error: &RunError) -> String {
    match error {
        RunError::Io {
            operation: Operation::Spawn,
            ..
        } => "swarm client is unavailable".to_owned(),
        _ => "swarm client failed".to_owned(),
    }
}

/// Keep only a bounded, control-free first line of client diagnostics.
fn detail(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().next()?;
    let cleaned: String = line
        .chars()
        .take(MAX_DETAIL_BYTES)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty()
        || trimmed.contains("SECRETSPEC_")
        || trimmed.contains("PASSWORD")
        || trimmed.contains("/run/secrets")
    {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_is_bounded_and_redacted() {
        assert_eq!(
            detail(b"441 posting rate limit exceeded\n").as_deref(),
            Some("441 posting rate limit exceeded")
        );
        assert_eq!(detail(b"SECRETSPEC_GITHUB=abc"), None);
        assert_eq!(detail(b"PASSWORD hunter2"), None);
        assert_eq!(detail(&[0x01, b'a']).as_deref(), Some("a"));
        let long = detail("x".repeat(1_000).as_bytes());
        assert_eq!(long.map(|value| value.len()), Some(MAX_DETAIL_BYTES));
    }

    #[test]
    fn article_view_uses_only_headers() {
        let article = Article {
            number: 1,
            message_id: "<claim@aspen1>".to_owned(),
            from: "omp <aspen3@swarm.local>".to_owned(),
            subject: "CLAIM OnixResearch/kiln crates/kiln-core".to_owned(),
            date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
            references: "<parent@aspen1> <other@aspen1>".to_owned(),
            body: String::new(),
        };
        let view = article
            .view()
            .unwrap_or_else(|| unreachable!("date must parse"));
        assert_eq!(view.date_unix_ms, 1_795_042_860_000);
        assert_eq!(view.references.len(), 2);
    }

    #[test]
    fn accepted_receipt_admits_only_bounded_message_ids() {
        assert_eq!(
            PostReceipt::accepted(br#"{"posted":true,"message_id":"<id@swarm.local>"}"#).message_id.as_deref(),
            Some("<id@swarm.local>")
        );
        assert_eq!(
            PostReceipt::accepted(br#"{"posted":false,"message_id":"<id@swarm.local>"}"#).message_id,
            None
        );
        assert_eq!(
            PostReceipt::accepted(br#"{"posted":true,"message_id":"not-angle-bracketed"}"#).message_id,
            None
        );
        assert_eq!(
            PostReceipt::accepted(br#"{"posted":true,"message_id":"<bad<id@swarm>"}"#).message_id,
            None
        );
        let oversized = format!(r#"{{"posted":true,"message_id":"<{}@swarm>"}}"#, "x".repeat(MAX_MESSAGE_ID_BYTES));
        assert_eq!(PostReceipt::accepted(oversized.as_bytes()).message_id, None);
        assert_eq!(PostReceipt::accepted(b"plain ok").message_id, None);
    }
}

#[cfg(test)]
mod parse_tests {
    use super::Article;

    #[test]
    fn article_array_parses() {
        let text = r#"[
          {"number":1,"message_id":"<claim@aspen1>","from":"omp <aspen3@swarm.local>","subject":"CLAIM OnixResearch/kiln crates/kiln-core","date":"Wed, 18 Nov 2026 23:01:00 +0000","references":"","body":"ignored"}
        ]"#;
        let articles: Vec<Article> =
            serde_json::from_str(text).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(articles.len(), 1);
    }

    #[test]
    fn group_array_tolerates_unknown_fields_and_refuses_missing_ones() {
        let text = r#"[{"name":"bots.claims","high":120,"low":3,"posting":"y","description":"claims","creator":"x"}]"#;
        let groups: Vec<super::GroupEntry> =
            serde_json::from_str(text).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(groups.first().map(|group| group.high), Some(120));
        assert!(
            serde_json::from_str::<Vec<super::GroupEntry>>(r#"[{"name":"bots.claims"}]"#).is_err()
        );
    }
}
