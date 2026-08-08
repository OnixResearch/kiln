use kiln_core::types::{Alias, CONTRACT_VERSION_V1, CoreResult, Diagnostic, ExternalRef, RunId};
use serde::{Deserialize, Serialize};

pub const RADICLE_BROKER_PROTOCOL_VERSION: u16 = 1;
pub const MAX_TRIGGER_LINE_BYTES: usize = 64 * 1024;
pub const MAX_LOG_URL_BYTES: usize = 2048;
pub const DEFAULT_MAX_POLLS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerEventKind {
    Push,
    Patch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerTrigger {
    pub version: u16,
    pub event: BrokerEventKind,
    pub repository: String,
    pub actor: String,
    pub before: Option<String>,
    pub after: String,
    pub branch: Option<String>,
    pub patch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedTrigger {
    pub repository: ExternalRef,
    pub actor: ExternalRef,
    pub before: Option<ExternalRef>,
    pub after: ExternalRef,
    pub branch: Option<Alias>,
    pub patch: Option<ExternalRef>,
    pub event: BrokerEventKind,
}

pub fn parse_trigger_line(input: &str) -> CoreResult<BrokerTrigger> {
    if input.is_empty() || input.len() > MAX_TRIGGER_LINE_BYTES {
        return Err(Diagnostic::new(
            "radicle_input_bound",
            "stdin",
            "broker input is empty or exceeds its named bound",
        ));
    }
    let trimmed = input.strip_suffix('\n').unwrap_or(input);
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(Diagnostic::new(
            "radicle_line_count",
            "stdin",
            "broker input must contain exactly one line",
        ));
    }
    let trigger: BrokerTrigger = serde_json::from_str(trimmed).map_err(|_| {
        Diagnostic::new(
            "radicle_json",
            "stdin",
            "broker input is not a supported JSON trigger",
        )
    })?;
    if trigger.version != RADICLE_BROKER_PROTOCOL_VERSION || trigger.version != CONTRACT_VERSION_V1
    {
        return Err(Diagnostic::new(
            "radicle_version",
            "trigger.version",
            "broker protocol version is unsupported",
        ));
    }
    Ok(trigger)
}

pub fn map_trigger(trigger: &BrokerTrigger) -> CoreResult<MappedTrigger> {
    Ok(MappedTrigger {
        repository: ExternalRef::new("radicle", "repository", &trigger.repository)?,
        actor: ExternalRef::new("radicle", "actor", &trigger.actor)?,
        before: trigger
            .before
            .as_deref()
            .map(|value| ExternalRef::new("radicle", "revision", value))
            .transpose()?,
        after: ExternalRef::new("radicle", "revision", &trigger.after)?,
        branch: trigger.branch.as_deref().map(Alias::new).transpose()?,
        patch: trigger
            .patch
            .as_deref()
            .map(|value| ExternalRef::new("radicle", "patch", value))
            .transpose()?,
        event: trigger.event.clone(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerTerminal {
    Success,
    Failure,
    Denied,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerRun {
    pub run_id: RunId,
    pub terminal: BrokerTerminal,
    pub log_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerWaitMode {
    Wait,
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerLifecyclePolicy {
    pub mode: BrokerWaitMode,
    pub max_polls: usize,
}

pub trait BrokerService {
    fn accept(&mut self, trigger: MappedTrigger) -> CoreResult<BrokerRun>;

    fn observe(
        &mut self,
        accepted: &BrokerRun,
        _policy: BrokerLifecyclePolicy,
    ) -> CoreResult<BrokerRun> {
        Ok(accepted.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BrokerResponse<'a> {
    Triggered {
        version: u16,
        run_id: &'a str,
    },
    Finished {
        version: u16,
        run_id: &'a str,
        result: BrokerTerminal,
        log_url: Option<&'a str>,
    },
}

pub fn run_protocol<S: BrokerService>(input: &str, service: &mut S) -> CoreResult<Vec<String>> {
    run_protocol_with_policy(
        input,
        service,
        BrokerLifecyclePolicy {
            mode: BrokerWaitMode::Wait,
            max_polls: DEFAULT_MAX_POLLS,
        },
    )
}

pub fn run_protocol_with_policy<S: BrokerService>(
    input: &str,
    service: &mut S,
    policy: BrokerLifecyclePolicy,
) -> CoreResult<Vec<String>> {
    if policy.max_polls == 0 {
        return Err(Diagnostic::new(
            "radicle_lifecycle_policy",
            "broker.policy.max_polls",
            "broker lifecycle policy requires a positive poll bound",
        ));
    }
    let trigger = map_trigger(&parse_trigger_line(input)?)?;
    let accepted = service.accept(trigger)?;
    let run = service.observe(&accepted, policy)?;
    if run.run_id != accepted.run_id {
        return Err(Diagnostic::new(
            "radicle_run_substitution",
            "broker.run_id",
            "broker lifecycle observation changed the accepted Kiln run identity",
        ));
    }
    if run
        .log_url
        .as_ref()
        .is_some_and(|url| url.len() > MAX_LOG_URL_BYTES)
    {
        return Err(Diagnostic::new(
            "radicle_log_url_bound",
            "response.log_url",
            "broker log URL exceeds its named bound",
        ));
    }
    let triggered = serde_json::to_string(&BrokerResponse::Triggered {
        version: RADICLE_BROKER_PROTOCOL_VERSION,
        run_id: run.run_id.as_str(),
    })
    .map_err(|_| {
        Diagnostic::new(
            "radicle_output",
            "stdout",
            "broker triggered response encoding failed",
        )
    })?;
    let finished = serde_json::to_string(&BrokerResponse::Finished {
        version: RADICLE_BROKER_PROTOCOL_VERSION,
        run_id: run.run_id.as_str(),
        result: run.terminal,
        log_url: run.log_url.as_deref(),
    })
    .map_err(|_| {
        Diagnostic::new(
            "radicle_output",
            "stdout",
            "broker terminal response encoding failed",
        )
    })?;
    Ok(vec![triggered, finished])
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUSH: &str = "{\"version\":1,\"event\":\"push\",\"repository\":\"rad:zrepo\",\"actor\":\"did:key:actor\",\"before\":null,\"after\":\"abc\",\"branch\":\"main\",\"patch\":null}\n";

    struct FixtureService;

    impl BrokerService for FixtureService {
        fn accept(&mut self, _trigger: MappedTrigger) -> CoreResult<BrokerRun> {
            Ok(BrokerRun {
                run_id: RunId::new("kiln-run-1")?,
                terminal: BrokerTerminal::Success,
                log_url: Some("https://logs.invalid/kiln-run-1".to_string()),
            })
        }
    }

    #[test]
    fn maps_push_and_emits_two_protocol_lines() {
        let responses = run_protocol(PUSH, &mut FixtureService);
        assert_eq!(responses.map(|lines| lines.len()), Ok(2));
    }

    #[test]
    fn maps_patch_and_preserves_failure() {
        let patch = PUSH
            .replace("\"event\":\"push\"", "\"event\":\"patch\"")
            .replace("\"patch\":null", "\"patch\":\"patch-id\"");
        struct FailureService;
        impl BrokerService for FailureService {
            fn accept(&mut self, _trigger: MappedTrigger) -> CoreResult<BrokerRun> {
                Ok(BrokerRun {
                    run_id: RunId::new("kiln-run-failure")?,
                    terminal: BrokerTerminal::Failure,
                    log_url: None,
                })
            }
        }
        let responses = run_protocol(&patch, &mut FailureService);
        assert!(
            responses.is_ok_and(|lines| lines.last().is_some_and(|line| line.contains("failure")))
        );
    }

    #[test]
    fn rejects_zero_poll_bound() {
        let result = run_protocol_with_policy(
            PUSH,
            &mut FixtureService,
            BrokerLifecyclePolicy {
                mode: BrokerWaitMode::Poll,
                max_polls: 0,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_version() {
        let input = PUSH.replace("\"version\":1", "\"version\":2");
        assert!(parse_trigger_line(&input).is_err());
    }

    #[test]
    fn rejects_stdout_noise_fields() {
        let input = PUSH.replace("\"patch\":null", "\"patch\":null,\"noise\":true");
        assert!(parse_trigger_line(&input).is_err());
    }
}
