use kiln_core::types::RunId;
use kiln_provider_api::{TriggerRun, TriggerWaitMode};

use super::*;

const PUSH: &str = include_str!("../../../../fixtures/radicle-ci-broker/0.31.0/trigger.json");
const PUBLISHED_REPOSITORY: &str = "rad:z3uBEubocQ9kJANPvMAo6z5ZhhaFh";
const PUBLISHED_COMMIT: &str = "470a437150187c2e85395fc7f55a9b3f836123fc";
const PUBLISHED_BRANCH: &str = "main";
const UNSUPPORTED_PROTOCOL_VERSION: &str = "\"version\":2";

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

// r[verify kiln.radicle.broker_031_wire.accepted]
#[test]
fn published_broker_fixture_maps_only_authoritative_trigger_facts() {
    let request = parse_defelo_request(PUSH)
        .unwrap_or_else(|_| unreachable!("published broker fixture must parse"));
    let mapped = map_defelo_request(&request)
        .unwrap_or_else(|_| unreachable!("published broker fixture must map"));
    assert_eq!(mapped.repository.native_id.as_str(), PUBLISHED_REPOSITORY);
    assert_eq!(mapped.actor.native_id.as_str(), "broker");
    assert!(mapped.before.is_none());
    assert_eq!(mapped.after.native_id.as_str(), PUBLISHED_COMMIT);
    assert_eq!(
        mapped.branch.as_ref().map(Alias::as_str),
        Some(PUBLISHED_BRANCH),
    );
    assert_eq!(mapped.event, TriggerEventKind::Push);

    let descriptive_drift = PUSH.replacen(
        "\"name\":\"pathdedup\"",
        "\"name\":\"producer-metadata-is-not-authority\"",
        1,
    );
    let drifted_request = parse_defelo_request(&descriptive_drift)
        .unwrap_or_else(|_| unreachable!("descriptive producer metadata must parse"));
    let drifted_mapping = map_defelo_request(&drifted_request)
        .unwrap_or_else(|_| unreachable!("descriptive producer metadata must not change mapping"));
    assert_eq!(mapped, drifted_mapping);
}

// r[verify kiln.radicle.broker_031_wire.rejected]
#[test]
fn invalid_envelopes_and_unknown_outcome_fail_closed() {
    assert!(parse_defelo_request("").is_err());
    assert!(parse_defelo_request("not-json").is_err());
    let overbound_input = "x".repeat(MAX_DEFELO_REQUEST_BYTES.saturating_add(1));
    assert!(parse_defelo_request(&overbound_input).is_err());
    let unknown_event = PUSH.replacen("\"event_type\":\"push\"", "\"event_type\":\"other\"", 1);
    assert!(parse_defelo_request(&unknown_event).is_err());
    let wrong_version = PUSH.replacen("\"version\":1", UNSUPPORTED_PROTOCOL_VERSION, 1);
    assert!(parse_defelo_request(&wrong_version).is_err());
    let missing_kind = PUSH.replacen("\"request\":\"trigger\",", "", 1);
    assert!(parse_defelo_request(&missing_kind).is_err());
    let wrong_kind = PUSH.replacen("\"request\":\"trigger\"", "\"request\":\"other\"", 1);
    assert!(parse_defelo_request(&wrong_kind).is_err());
    assert!(
        map_defelo_request(&DefeloRequest {
            request: DefeloRequestKind::Trigger,
            version: DEFELO_PROTOCOL_VERSION,
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
