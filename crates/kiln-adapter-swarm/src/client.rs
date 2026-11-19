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
    ArticleView, ClaimArticle, admit_body, parse_rfc2822_ms, parse_subject,
};
use serde::Deserialize;

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
}

impl PostReceipt {
    fn new(outcome: PostOutcome, detail: Option<String>) -> Self {
        Self { outcome, detail }
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
        let request = self.request(args, Input::Null, 1)?;
        let output = run(request).map_err(|error| SwarmFailure {
            detail: run_detail(&error),
        })?;
        if output.completion != Completion::Exited || output.disposition != Disposition::Succeeded {
            return Err(SwarmFailure {
                detail: classify_read(&output),
            });
        }
        let text = std::str::from_utf8(&output.stdout.bytes).map_err(|_| SwarmFailure {
            detail: "swarm response is not UTF-8".to_owned(),
        })?;
        serde_json::from_str::<Vec<Article>>(text).map_err(|error| SwarmFailure {
            detail: format!(
                "swarm response is not a bounded article array ({error}, {} bytes)",
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
                Disposition::Succeeded => PostReceipt::new(PostOutcome::Accepted, None),
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
                    .environment
                    .iter()
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

fn classify_read(output: &bounded_exec::ExecutionOutput) -> String {
    if output.completion == Completion::TimedOut {
        return "swarm read timed out".to_owned();
    }
    if output.completion == Completion::Cancelled {
        return "swarm read cancelled".to_owned();
    }
    if matches!(output.disposition, Disposition::OutputLimitExceeded(_)) {
        return "swarm response exceeds its bound".to_owned();
    }
    let detail = detail(&output.stderr.bytes);
    detail.unwrap_or_else(|| "swarm read failed".to_owned())
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
        };
        let view = article
            .view()
            .unwrap_or_else(|| unreachable!("date must parse"));
        assert_eq!(view.date_unix_ms, 1_795_042_860_000);
        assert_eq!(view.references.len(), 2);
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
}
