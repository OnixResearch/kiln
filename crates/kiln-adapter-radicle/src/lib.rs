pub mod defelo;
pub mod mutation;
mod profile;
mod replay;
pub mod source;
pub mod status;
mod workflow;

pub use profile::*;
pub use replay::*;
pub use workflow::*;

use kiln_core::trigger::{admit_trigger_poll_bound, admit_trigger_run_observation};
use kiln_core::types::{Alias, CONTRACT_VERSION_V1, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{
    PortResult, TriggerEventKind, TriggerIntakePort, TriggerLifecyclePolicy, TriggerRequest,
    TriggerTerminal, TriggerWaitMode,
};
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

pub fn map_trigger(trigger: &BrokerTrigger) -> CoreResult<TriggerRequest> {
    Ok(TriggerRequest {
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
        event: match trigger.event {
            BrokerEventKind::Push => TriggerEventKind::Push,
            BrokerEventKind::Patch => TriggerEventKind::Patch,
        },
    })
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
        result: TriggerTerminal,
        log_url: Option<&'a str>,
    },
}

pub fn run_protocol<S: TriggerIntakePort>(input: &str, service: &mut S) -> PortResult<Vec<String>> {
    run_protocol_with_policy(
        input,
        service,
        TriggerLifecyclePolicy {
            mode: TriggerWaitMode::Wait,
            max_polls: DEFAULT_MAX_POLLS,
        },
    )
}

pub fn run_protocol_with_policy<S: TriggerIntakePort>(
    input: &str,
    service: &mut S,
    policy: TriggerLifecyclePolicy,
) -> PortResult<Vec<String>> {
    admit_trigger_poll_bound(policy.max_polls)?;
    let trigger = map_trigger(&parse_trigger_line(input)?)?;
    let accepted = service.accept(trigger)?;
    let run = service.observe(&accepted, policy)?;
    admit_trigger_run_observation(
        &accepted.run_id,
        &run.run_id,
        run.log_url.as_ref().map(String::len),
        MAX_LOG_URL_BYTES,
    )?;
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
    use kiln_core::types::RunId;
    use kiln_provider_api::{InfrastructureFailure, PortFailure, TriggerRun};

    use super::*;

    const PUSH: &str = "{\"version\":1,\"event\":\"push\",\"repository\":\"rad:zrepo\",\"actor\":\"did:key:actor\",\"before\":null,\"after\":\"abc\",\"branch\":\"main\",\"patch\":null}\n";

    struct FixtureService {
        terminal: TriggerTerminal,
        substitute_run: bool,
        accept_failure: Option<PortFailure>,
        accept_calls: usize,
        observe_calls: usize,
    }

    impl FixtureService {
        fn new(terminal: TriggerTerminal) -> Self {
            Self {
                terminal,
                substitute_run: false,
                accept_failure: None,
                accept_calls: 0,
                observe_calls: 0,
            }
        }
    }

    impl TriggerIntakePort for FixtureService {
        fn accept(&mut self, _trigger: TriggerRequest) -> PortResult<TriggerRun> {
            self.accept_calls += 1;
            if let Some(error) = self.accept_failure.clone() {
                return Err(error);
            }
            Ok(TriggerRun {
                run_id: RunId::new("kiln-run-1")?,
                terminal: self.terminal,
                log_url: Some("https://logs.invalid/kiln-run-1".to_string()),
            })
        }

        fn observe(
            &mut self,
            accepted: &TriggerRun,
            _policy: TriggerLifecyclePolicy,
        ) -> PortResult<TriggerRun> {
            self.observe_calls += 1;
            let mut observed = accepted.clone();
            if self.substitute_run {
                observed.run_id = RunId::new("substituted-run")?;
            }
            Ok(observed)
        }
    }

    #[test]
    fn maps_push_and_emits_two_protocol_lines() {
        let mut service = FixtureService::new(TriggerTerminal::Success);
        let responses = run_protocol(PUSH, &mut service);
        assert_eq!(responses.map(|lines| lines.len()), Ok(2));
        assert_eq!(service.accept_calls, 1);
        assert_eq!(service.observe_calls, 1);
    }

    #[test]
    fn maps_patch_and_preserves_failure() {
        let patch = PUSH
            .replace("\"event\":\"push\"", "\"event\":\"patch\"")
            .replace("\"patch\":null", "\"patch\":\"patch-id\"");
        let responses = run_protocol(&patch, &mut FixtureService::new(TriggerTerminal::Failure));
        assert!(
            responses.is_ok_and(|lines| lines.last().is_some_and(|line| line.contains("failure")))
        );
    }

    #[test]
    fn rejects_zero_poll_bound_before_provider_calls() {
        let mut service = FixtureService::new(TriggerTerminal::Success);
        let result = run_protocol_with_policy(
            PUSH,
            &mut service,
            TriggerLifecyclePolicy {
                mode: TriggerWaitMode::Poll,
                max_polls: 0,
            },
        );
        assert!(result.is_err());
        assert_eq!(service.accept_calls, 0);
        assert_eq!(service.observe_calls, 0);
    }

    // r[verify kiln.providers.boundary_conformance]
    #[test]
    fn rejects_run_substitution_and_preserves_timeout_failure() {
        let mut substitute = FixtureService::new(TriggerTerminal::Success);
        substitute.substitute_run = true;
        assert!(run_protocol(PUSH, &mut substitute).is_err());

        let mut timed_out = FixtureService::new(TriggerTerminal::Unknown);
        timed_out.accept_failure = Some(PortFailure::TimedOut(InfrastructureFailure {
            code: "trigger_timeout".to_string(),
            message: "fixture trigger intake timed out".to_string(),
        }));
        assert!(matches!(
            run_protocol(PUSH, &mut timed_out),
            Err(PortFailure::TimedOut(_))
        ));
        assert_eq!(timed_out.observe_calls, 0);
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

    // r[verify kiln.radicle.nonclaims]
    // r[verify kiln.radicle.native_runner.scenario.shell]
    // r[verify kiln.radicle.shell_conformance.scenario.fixture_only]
    #[test]
    fn production_composition_has_no_fixture_direct_nix_or_core_provider_edge() {
        let main_source = include_str!("main.rs");
        let core_manifest = include_str!("../../kiln-core/Cargo.toml");
        let provider_manifest = include_str!("../../kiln-provider-api/Cargo.toml");

        assert!(!main_source.contains("LocalFixtureService"));
        assert!(!main_source.contains("NixCheckService"));
        assert!(!main_source.contains("std::process::Command"));
        assert!(!core_manifest.contains("kiln-adapter-radicle"));
        assert!(!core_manifest.contains("radicle-cob"));
        assert!(!provider_manifest.contains("kiln-adapter-radicle"));
        assert!(!provider_manifest.contains("radicle-cob"));
    }
}
