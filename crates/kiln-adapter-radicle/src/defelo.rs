//! Bounded deployed Radicle CI broker adapter protocol.

use kiln_core::trigger::{admit_trigger_poll_bound, admit_trigger_run_observation};
use kiln_core::types::{Alias, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{
    PortResult, TriggerEventKind, TriggerIntakePort, TriggerLifecyclePolicy, TriggerRequest,
    TriggerTerminal, TriggerWaitMode,
};
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_MAX_POLLS, MAX_LOG_URL_BYTES};

pub const MAX_DEFELO_REQUEST_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeloEvent {
    Push,
    Patch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefeloRepository {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefeloPatch {
    pub commits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefeloRequest {
    pub event_type: DefeloEvent,
    pub repository: DefeloRepository,
    pub commits: Option<Vec<String>>,
    pub patch: Option<DefeloPatch>,
    pub branch: Option<String>,
}

pub fn parse_defelo_request(input: &str) -> CoreResult<DefeloRequest> {
    if input.is_empty() || input.len() > MAX_DEFELO_REQUEST_BYTES {
        return Err(Diagnostic::new(
            "radicle_input_bound",
            "stdin",
            "broker input is empty or exceeds its named request bound",
        ));
    }
    serde_json::from_str(input).map_err(|_| {
        Diagnostic::new(
            "radicle_json",
            "stdin",
            "broker input is not a supported deployed broker request",
        )
    })
}

fn resolve_revision(request: &DefeloRequest) -> Option<&str> {
    match request.event_type {
        DefeloEvent::Push => request
            .commits
            .as_ref()
            .and_then(|commits| commits.first())
            .map(String::as_str),
        DefeloEvent::Patch => request
            .patch
            .as_ref()
            .and_then(|patch| patch.commits.first())
            .map(String::as_str),
    }
}

pub fn map_defelo_request(request: &DefeloRequest) -> CoreResult<TriggerRequest> {
    let revision = resolve_revision(request).ok_or_else(|| {
        Diagnostic::new(
            "radicle_revision",
            "stdin.revision",
            "broker request carries no exact committed revision",
        )
    })?;
    Ok(TriggerRequest {
        repository: ExternalRef::new("radicle", "repository", request.repository.id.as_str())?,
        actor: ExternalRef::new("radicle", "actor", "broker")?,
        before: None,
        after: ExternalRef::new("radicle", "revision", revision)?,
        branch: request.branch.as_deref().map(Alias::new).transpose()?,
        patch: None,
        event: match request.event_type {
            DefeloEvent::Push => TriggerEventKind::Push,
            DefeloEvent::Patch => TriggerEventKind::Patch,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeloResult {
    Success,
    Failure,
}

#[must_use]
pub const fn terminal_to_defelo(terminal: TriggerTerminal) -> DefeloResult {
    match terminal {
        TriggerTerminal::Success => DefeloResult::Success,
        TriggerTerminal::Failure
        | TriggerTerminal::Denied
        | TriggerTerminal::Cancelled
        | TriggerTerminal::TimedOut
        | TriggerTerminal::ProviderError
        | TriggerTerminal::Unknown => DefeloResult::Failure,
    }
}

#[derive(Debug, Serialize)]
pub struct DefeloRunId<'a> {
    pub id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum DefeloResponse<'a> {
    Triggered {
        run_id: DefeloRunId<'a>,
        #[serde(skip_serializing_if = "Option::is_none")]
        info_url: Option<&'a str>,
    },
    Finished {
        result: DefeloResult,
    },
}

// r[impl kiln.radicle.lifecycle]
pub fn run_defelo_protocol<S: TriggerIntakePort>(
    input: &str,
    service: &mut S,
) -> PortResult<Vec<String>> {
    run_defelo_protocol_with_policy(
        input,
        service,
        TriggerLifecyclePolicy {
            mode: TriggerWaitMode::Wait,
            max_polls: DEFAULT_MAX_POLLS,
        },
    )
}

pub fn run_defelo_protocol_with_policy<S: TriggerIntakePort>(
    input: &str,
    service: &mut S,
    policy: TriggerLifecyclePolicy,
) -> PortResult<Vec<String>> {
    admit_trigger_poll_bound(policy.max_polls)?;
    let trigger = map_defelo_request(&parse_defelo_request(input)?)?;
    let accepted = service.accept(trigger)?;
    let run = service.observe(&accepted, policy)?;
    admit_trigger_run_observation(
        &accepted.run_id,
        &run.run_id,
        run.log_url.as_ref().map(String::len),
        MAX_LOG_URL_BYTES,
    )?;
    let triggered = serde_json::to_string(&DefeloResponse::Triggered {
        run_id: DefeloRunId {
            id: run.run_id.as_str(),
        },
        info_url: run.log_url.as_deref(),
    })
    .map_err(|_| {
        Diagnostic::new(
            "defelo_output",
            "stdout",
            "triggered response encoding failed",
        )
    })?;
    let finished = serde_json::to_string(&DefeloResponse::Finished {
        result: terminal_to_defelo(run.terminal),
    })
    .map_err(|_| {
        Diagnostic::new(
            "defelo_output",
            "stdout",
            "terminal response encoding failed",
        )
    })?;
    Ok(vec![triggered, finished])
}

#[cfg(test)]
mod tests {
    use kiln_core::types::RunId;
    use kiln_provider_api::{TriggerRun, TriggerWaitMode};

    use super::*;

    const PUSH: &str = r#"{
      "event_type": "push",
      "repository": { "id": "rad:zrepo", "name": "kiln" },
      "commits": ["abc123"],
      "branch": "main"
    }"#;

    struct FixtureService {
        terminal: TriggerTerminal,
        accept_calls: usize,
    }

    impl TriggerIntakePort for FixtureService {
        fn accept(&mut self, _trigger: TriggerRequest) -> PortResult<TriggerRun> {
            self.accept_calls = self.accept_calls.saturating_add(1);
            Ok(TriggerRun {
                run_id: RunId::new("kiln-defelo-run")?,
                terminal: self.terminal,
                log_url: None,
            })
        }

        fn observe(
            &mut self,
            accepted: &TriggerRun,
            _policy: TriggerLifecyclePolicy,
        ) -> PortResult<TriggerRun> {
            Ok(accepted.clone())
        }
    }

    // r[verify kiln.radicle.trigger.scenario.push]
    // r[verify kiln.radicle.lifecycle.scenario.triggered]
    #[test]
    fn deployed_push_maps_and_emits_exact_protocol_lines() {
        let mut service = FixtureService {
            terminal: TriggerTerminal::Success,
            accept_calls: 0,
        };
        let lines = run_defelo_protocol(PUSH, &mut service).unwrap_or_default();
        assert_eq!(service.accept_calls, 1);
        assert_eq!(lines.len(), 2);
        assert!(
            lines
                .first()
                .is_some_and(|line| line.contains("\"response\":\"triggered\""))
        );
        assert!(
            lines
                .last()
                .is_some_and(|line| line.contains("\"result\":\"success\""))
        );
    }

    #[test]
    fn malformed_missing_revision_and_unknown_outcome_fail_closed() {
        assert!(parse_defelo_request("not-json").is_err());
        assert!(
            map_defelo_request(&DefeloRequest {
                event_type: DefeloEvent::Push,
                repository: DefeloRepository {
                    id: "rad:zrepo".to_string(),
                    name: None,
                },
                commits: Some(Vec::new()),
                patch: None,
                branch: None,
            })
            .is_err()
        );
        let mut service = FixtureService {
            terminal: TriggerTerminal::Unknown,
            accept_calls: 0,
        };
        let lines = run_defelo_protocol_with_policy(
            PUSH,
            &mut service,
            TriggerLifecyclePolicy {
                mode: TriggerWaitMode::Poll,
                max_polls: 1,
            },
        )
        .unwrap_or_default();
        assert!(
            lines
                .last()
                .is_some_and(|line| line.contains("\"result\":\"failure\""))
        );
    }
}
