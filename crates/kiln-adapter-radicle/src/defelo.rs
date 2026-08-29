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
pub const DEFELO_PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeloRequestKind {
    Trigger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefeloEvent {
    Push,
    Patch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DefeloRepository {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DefeloPatch {
    pub commits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DefeloRequest {
    pub request: DefeloRequestKind,
    pub version: u16,
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
    let request: DefeloRequest = serde_json::from_str(input).map_err(|_| {
        Diagnostic::new(
            "radicle_json",
            "stdin",
            "broker input is not a supported deployed broker request",
        )
    })?;
    if request.request != DefeloRequestKind::Trigger || request.version != DEFELO_PROTOCOL_VERSION {
        return Err(Diagnostic::new(
            "radicle_version",
            "stdin.version",
            "broker request kind or protocol version is unsupported",
        ));
    }
    Ok(request)
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
#[path = "defelo/tests.rs"]
mod tests;
