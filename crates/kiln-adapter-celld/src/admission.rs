use std::collections::BTreeSet;

use kiln_core::domain::State;
use kiln_core::operations::{JournalRecord, LeaseState, RecoveryAction, classify_recovery, replay};
use kiln_core::types::{
    CONTRACT_VERSION_V1, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef,
    ProviderProfileId,
};
use kiln_provider_api::EventAppendRequest;

use crate::profile::CelldProfile;

// r[impl kiln.celld.atomic_append] r[impl kiln.celld.ambiguity]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendPlan {
    pub next_revision: u64,
    pub event_count: usize,
    pub outbox_count: usize,
    pub payload_identity: ContentId,
    pub encoded_payload: Vec<u8>,
}

pub fn admit_append_batch(
    profile: &CelldProfile,
    current_revision: u64,
    existing_identities: &BTreeSet<ExternalRef>,
    request: &EventAppendRequest,
) -> CoreResult<AppendPlan> {
    crate::profile::admit_profile(profile)?;
    request.stream.validate()?;
    if request.expected_revision != current_revision {
        return Err(Diagnostic::new(
            "celld_stale_revision",
            "celld.expected_revision",
            "event append expected revision differs from the committed stream revision",
        ));
    }
    if request.records.is_empty() || request.records.len() > profile.bounds.max_events_per_append {
        return Err(Diagnostic::new(
            "celld_append_bound",
            "celld.records",
            "event append is empty or exceeds its named bound",
        ));
    }
    if request.outbox.len() > profile.bounds.max_outbox_records {
        return Err(Diagnostic::new(
            "celld_outbox_bound",
            "celld.outbox",
            "outbox batch exceeds its named bound",
        ));
    }
    let mut identities = existing_identities.clone();
    for (offset, record) in request.records.iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| {
            Diagnostic::new(
                "celld_revision_overflow",
                "celld.records",
                "event offset cannot be represented",
            )
        })?;
        let expected = current_revision.checked_add(offset).ok_or(Diagnostic::new(
            "celld_revision_overflow",
            "celld.records",
            "event revision overflowed",
        ))?;
        if record.stream != request.stream || record.expected_revision != expected {
            return Err(Diagnostic::new(
                "celld_event_sequence",
                "celld.records",
                "event stream or expected revision is not contiguous",
            ));
        }
        admit_event_identity(profile, record)?;
        if !identities.insert(record.event_identity.clone()) {
            return Err(Diagnostic::new(
                "celld_duplicate_event",
                "celld.event_identity",
                "event identity is already committed or duplicated within the append batch",
            ));
        }
    }
    let mut effects = BTreeSet::new();
    let mut idempotency_keys = BTreeSet::new();
    for record in &request.outbox {
        if !effects.insert(record.effect_id.clone())
            || !idempotency_keys.insert(record.idempotency_key.clone())
        {
            return Err(Diagnostic::new(
                "celld_duplicate_outbox",
                "celld.outbox",
                "outbox effect or idempotency identity is duplicated",
            ));
        }
        record.idempotency_key.validate()?;
    }
    let (encoded_payload, payload_identity) =
        crate::protocol::encode_append_payload(profile, request)?;
    let event_count = u64::try_from(request.records.len()).map_err(|_| {
        Diagnostic::new(
            "celld_revision_overflow",
            "celld.records",
            "event count cannot be represented",
        )
    })?;
    let next_revision = current_revision
        .checked_add(event_count)
        .ok_or(Diagnostic::new(
            "celld_revision_overflow",
            "celld.records",
            "event revision overflowed",
        ))?;
    Ok(AppendPlan {
        next_revision,
        event_count: request.records.len(),
        outbox_count: request.outbox.len(),
        payload_identity,
        encoded_payload,
    })
}

pub fn admit_event_identity(profile: &CelldProfile, record: &JournalRecord) -> CoreResult<()> {
    if record.schema_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "celld_event_version",
            "celld.event.schema_version",
            "event schema version is unsupported",
        ));
    }
    let bytes = serde_json::to_vec(&record.event).map_err(|_| {
        Diagnostic::new(
            "celld_event_encoding",
            "celld.event",
            "event canonical JSON encoding failed",
        )
    })?;
    if bytes.len() > profile.bounds.max_event_bytes {
        return Err(Diagnostic::new(
            "celld_event_bound",
            "celld.event",
            "event bytes exceed their named bound",
        ));
    }
    let identity = crate::protocol::identity_from_event(&record.event)?;
    if record.event_identity.provider_namespace.as_str() != "kiln"
        || record.event_identity.native_kind.as_str() != "event-blake3"
        || record.event_identity.native_id.as_str() != identity.as_str()
    {
        return Err(Diagnostic::new(
            "celld_event_identity_mismatch",
            "celld.event_identity",
            "event bytes do not match their BLAKE3 identity",
        ));
    }
    Ok(())
}

// r[impl kiln.celld.replay]
pub fn replay_checked(
    profile: &CelldProfile,
    initial: &State,
    records: &[JournalRecord],
) -> CoreResult<State> {
    for record in records {
        admit_event_identity(profile, record)?;
    }
    replay(initial, records)
}

// r[impl kiln.celld.leases]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DurableLease {
    pub effect_id: EffectId,
    pub provider_profile: ProviderProfileId,
    pub state: LeaseState,
    pub owner: Option<ExternalRef>,
    pub lease_until: Option<u64>,
    pub attempts: u32,
}

/// Signed one-object lease state, including evidence which must survive reopen.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredLease {
    pub lease: DurableLease,
    pub idempotency_key: ExternalRef,
    pub completion: Option<ExternalRef>,
    pub reconciliation: Option<ExternalRef>,
}

/// Persisted schema cohort and journal revision returned by the signed Worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSchema {
    pub version: u16,
    pub identity: ContentId,
    pub revision: u64,
}

pub const EMPTY_SCHEMA_IDENTITY: &str =
    "99a279c7fb99170797ff51ed0841ed9bee7957c9fb82926d9d80c87399e210c8";
pub const CURRENT_SCHEMA_IDENTITY: &str =
    "9a41ba000f167e6a189136cbda0b62aef8efee9e5533cc4f6cafa8e4923a055b";

pub fn plan_lease_claim(
    current: &DurableLease,
    owner: ExternalRef,
    now: u64,
    lease_ticks: u64,
) -> CoreResult<DurableLease> {
    owner.validate()?;
    if current.state != LeaseState::Unclaimed || lease_ticks == 0 {
        return Err(Diagnostic::new(
            "celld_lease_claim",
            "celld.lease",
            "only an unclaimed effect can receive a positive lease",
        ));
    }
    let lease_until = now.checked_add(lease_ticks).ok_or(Diagnostic::new(
        "celld_lease_overflow",
        "celld.lease_until",
        "lease bound overflowed",
    ))?;
    let attempts = current.attempts.checked_add(1).ok_or(Diagnostic::new(
        "celld_attempt_overflow",
        "celld.lease.attempts",
        "lease attempt count overflowed",
    ))?;
    Ok(DurableLease {
        effect_id: current.effect_id.clone(),
        provider_profile: current.provider_profile.clone(),
        state: LeaseState::Leased,
        owner: Some(owner),
        lease_until: Some(lease_until),
        attempts,
    })
}

/// Persist an externally observed terminal result after the shell reconciles
/// provider ambiguity. This function does not contact or redispatch a provider.
pub fn plan_lease_reconciled_completion(
    current: &StoredLease,
    completion: ExternalRef,
    reconciliation: ExternalRef,
) -> CoreResult<StoredLease> {
    completion.validate()?;
    reconciliation.validate()?;
    current.idempotency_key.validate()?;
    if current.lease.state != LeaseState::Leased
        || current.lease.owner.is_none()
        || current.lease.lease_until.is_none()
        || current.completion.is_some()
        || current.reconciliation.is_some()
    {
        return Err(Diagnostic::new(
            "celld_lease_completion",
            "celld.lease",
            "only a leased effect without prior terminal evidence can be reconciled",
        ));
    }
    let mut next = current.clone();
    next.lease.state = LeaseState::Completed;
    next.completion = Some(completion);
    next.reconciliation = Some(reconciliation);
    Ok(next)
}

#[must_use]
pub fn lease_recovery_action(lease: &DurableLease, now: u64) -> RecoveryAction {
    if lease.state == LeaseState::Leased
        && lease
            .lease_until
            .is_some_and(|lease_until| lease_until <= now)
    {
        return RecoveryAction::Reconcile;
    }
    classify_recovery(lease.state)
}

// r[impl kiln.celld.migrations]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationPlan {
    pub steps: Vec<(u16, u16)>,
}

pub fn plan_migration(
    current: u16,
    target: u16,
    admitted_edges: &[(u16, u16)],
    max_steps: usize,
) -> CoreResult<MigrationPlan> {
    if current == target {
        return Ok(MigrationPlan { steps: Vec::new() });
    }
    let mut version = current;
    let mut steps = Vec::new();
    let mut visited = BTreeSet::from([current]);
    while version != target {
        if steps.len() >= max_steps {
            return Err(Diagnostic::new(
                "celld_migration_bound",
                "celld.migrations",
                "migration plan exceeds its named step bound",
            ));
        }
        let next = admitted_edges
            .iter()
            .find_map(|(from, to)| (*from == version).then_some(*to))
            .ok_or(Diagnostic::new(
                "celld_migration_unknown",
                "celld.schema_version",
                "celld schema version has no admitted migration edge",
            ))?;
        if !visited.insert(next) {
            return Err(Diagnostic::new(
                "celld_migration_cycle",
                "celld.migrations",
                "celld migration plan contains a cycle",
            ));
        }
        steps.push((version, next));
        version = next;
    }
    Ok(MigrationPlan { steps })
}

pub fn parse_lease_state(value: &str) -> CoreResult<LeaseState> {
    match value {
        "unclaimed" => Ok(LeaseState::Unclaimed),
        "leased" => Ok(LeaseState::Leased),
        "expired" => Ok(LeaseState::Expired),
        "completed" => Ok(LeaseState::Completed),
        "ambiguous" => Ok(LeaseState::Ambiguous),
        _ => Err(Diagnostic::new(
            "celld_lease_state",
            "celld.lease_state",
            "stored lease state is unsupported",
        )),
    }
}

// r[impl kiln.celld.ambiguity]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationMatch {
    New,
    Repeated { stored_revision: u64 },
    PayloadConflict { stored_payload: ContentId },
}

pub fn classify_repeat(
    stored_payload: Option<&ContentId>,
    stored_revision: Option<u64>,
    incoming_payload: &ContentId,
) -> OperationMatch {
    match (stored_payload, stored_revision) {
        (Some(stored), Some(revision)) if stored == incoming_payload => OperationMatch::Repeated {
            stored_revision: revision,
        },
        (Some(stored), _) => OperationMatch::PayloadConflict {
            stored_payload: stored.clone(),
        },
        (None, _) => OperationMatch::New,
    }
}

// r[impl kiln.celld.replay]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayIntent {
    pub from_revision: u64,
    pub max_page_bytes: usize,
}

pub fn plan_replay_page(
    profile: &CelldProfile,
    intent: &ReplayIntent,
    retained_first_revision: u64,
) -> CoreResult<u64> {
    crate::profile::admit_profile(profile)?;
    if intent.max_page_bytes == 0 || intent.max_page_bytes > profile.bounds.max_replay_page_bytes {
        return Err(Diagnostic::new(
            "celld_replay_page_bound",
            "celld.replay.max_page_bytes",
            "replay page bound is zero or exceeds the admitted replay bound",
        ));
    }
    if intent.from_revision < retained_first_revision {
        return Err(Diagnostic::new(
            "celld_replay_window",
            "celld.replay.from_revision",
            "replay window starts before the retained first revision",
        ));
    }
    Ok(intent.from_revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::profile_fixture;
    use crate::protocol::identity_from_event;

    fn stream() -> ExternalRef {
        ExternalRef::new("kiln", "stream", "builds")
            .unwrap_or_else(|_| unreachable!("stream builds"))
    }

    fn event() -> kiln_core::domain::Event {
        kiln_core::domain::Event::RunStarted {
            command_id: kiln_core::types::CommandId::new("kiln-command-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            run_id: kiln_core::types::RunId::new("kiln-run-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            attempt_id: kiln_core::types::AttemptId::new("kiln-attempt-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
        }
    }

    fn record(revision: u64) -> JournalRecord {
        let event = event();
        let identity = identity_from_event(&event).unwrap_or_else(|_| unreachable!("identity"));
        JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream: stream(),
            expected_revision: revision,
            event_identity: ExternalRef::new("kiln", "event-blake3", identity.as_str())
                .unwrap_or_else(|_| unreachable!("event identity builds")),
            event,
        }
    }

    // r[verify kiln.celld.replay.scenario.valid]
    #[test]
    fn checked_replay_evolves_valid_history_without_provider_dispatch() {
        let rebuilt = replay_checked(&profile_fixture(), &State::default(), &[record(0)])
            .unwrap_or_else(|_| unreachable!("valid replay"));
        let run_id =
            kiln_core::types::RunId::new("kiln-run-1").unwrap_or_else(|_| unreachable!("run id"));
        assert_eq!(rebuilt.revision, 1);
        assert_eq!(
            rebuilt.runs.get(&run_id).map(|run| &run.outcome),
            Some(&kiln_core::domain::Outcome::Pending),
        );
    }

    // r[verify kiln.celld.replay.scenario.damage]
    #[test]
    fn checked_replay_rejects_corrupted_identity_before_evolution() {
        let mut damaged = record(0);
        damaged.event_identity = ExternalRef::new("kiln", "event-blake3", &"0".repeat(64))
            .unwrap_or_else(|_| unreachable!("valid stored identity shape"));
        let error = replay_checked(&profile_fixture(), &State::default(), &[damaged])
            .err()
            .unwrap_or_else(|| unreachable!("stored bytes differ from identity"));
        assert_eq!(error.code, "celld_event_identity_mismatch");
    }

    #[test]
    fn admits_one_batch_at_the_expected_revision() {
        let profile = profile_fixture();
        let records = vec![record(0)];
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records,
            outbox: Vec::new(),
        };
        let plan = admit_append_batch(&profile, 0, &BTreeSet::new(), &request)
            .unwrap_or_else(|_| unreachable!("admits"));
        assert_eq!(plan.next_revision, 1);
    }

    #[test]
    fn rejects_a_stale_expected_revision() {
        let profile = profile_fixture();
        let records = vec![record(0)];
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records,
            outbox: Vec::new(),
        };
        let error = admit_append_batch(&profile, 5, &BTreeSet::new(), &request)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_stale_revision");
    }

    #[test]
    fn rejects_an_empty_append() {
        let profile = profile_fixture();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: Vec::new(),
            outbox: Vec::new(),
        };
        let error = admit_append_batch(&profile, 0, &BTreeSet::new(), &request)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_append_bound");
    }

    #[test]
    fn rejects_a_non_contiguous_append() {
        let profile = profile_fixture();
        let records = vec![record(0), record(7)];
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records,
            outbox: Vec::new(),
        };
        let error = admit_append_batch(&profile, 0, &BTreeSet::new(), &request)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_event_sequence");
    }

    #[test]
    fn rejects_a_duplicate_event_identity() {
        let profile = profile_fixture();
        let mut existing = BTreeSet::new();
        existing.insert(record(0).event_identity.clone());
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let error = admit_append_batch(&profile, 0, &existing, &request)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_duplicate_event");
    }

    #[test]
    fn leases_move_unclaimed_to_leased() {
        let current = DurableLease {
            effect_id: EffectId::new("kiln-effect-1").unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
            state: LeaseState::Unclaimed,
            owner: None,
            lease_until: None,
            attempts: 0,
        };
        let next = plan_lease_claim(
            &current,
            ExternalRef::new("kiln", "worker", "kiln-1").unwrap_or_else(|_| unreachable!("builds")),
            100,
            30,
        )
        .unwrap_or_else(|_| unreachable!("claim"));
        assert_eq!(next.state, LeaseState::Leased);
        assert_eq!(next.attempts, 1);
    }

    #[test]
    fn expired_lease_requires_reconcile() {
        let lease = DurableLease {
            effect_id: EffectId::new("kiln-effect-1").unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
            state: LeaseState::Leased,
            owner: None,
            lease_until: Some(100),
            attempts: 1,
        };
        assert_eq!(
            lease_recovery_action(&lease, 100),
            RecoveryAction::Reconcile
        );
    }

    // r[verify kiln.celld.leases.scenario.ambiguous]
    #[test]
    fn ambiguous_provider_outcome_requires_reconciliation() {
        let lease = DurableLease {
            effect_id: EffectId::new("kiln-effect-1").unwrap_or_else(|_| unreachable!("effect")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("provider")),
            state: LeaseState::Ambiguous,
            owner: None,
            lease_until: None,
            attempts: 1,
        };
        assert_eq!(
            lease_recovery_action(&lease, 100),
            RecoveryAction::Reconcile,
        );
    }

    #[test]
    fn completed_lease_requires_no_action() {
        let lease = DurableLease {
            effect_id: EffectId::new("kiln-effect-1").unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
            state: LeaseState::Completed,
            owner: None,
            lease_until: None,
            attempts: 1,
        };
        assert_eq!(lease_recovery_action(&lease, 100), RecoveryAction::None);
    }

    #[test]
    fn plans_a_known_migration() {
        let plan = plan_migration(0, 1, &[(0, 1)], 4).unwrap_or_else(|_| unreachable!("plans"));
        assert_eq!(plan.steps, vec![(0, 1)]);
    }

    #[test]
    fn rejects_an_unknown_migration_edge() {
        let error = plan_migration(0, 9, &[(0, 1)], 4)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_migration_unknown");
    }

    #[test]
    fn rejects_a_migration_cycle() {
        // The path must travel into a revisit before reaching the target so
        // cycle detection fires instead of the step bound.
        let error = plan_migration(0, 3, &[(0, 1), (1, 0), (0, 3)], 4)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_migration_cycle");
    }

    #[test]
    fn rejects_a_migration_exceeding_the_step_bound() {
        let error = plan_migration(0, 5, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)], 2)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_migration_bound");
    }

    #[test]
    fn classifies_repeats_and_conflicts() {
        let profile = profile_fixture();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let payload = crate::protocol::payload_identity_of(&profile, &request)
            .unwrap_or_else(|_| unreachable!("identity"));
        let other = crate::protocol::payload_identity_of(&profile, &request)
            .unwrap_or_else(|_| unreachable!("identity"));
        assert_eq!(
            classify_repeat(Some(&payload), Some(1), &other),
            OperationMatch::Repeated { stored_revision: 1 }
        );
        let differing = crate::protocol::payload_identity_of(
            &profile,
            &EventAppendRequest {
                stream: stream(),
                expected_revision: 0,
                records: vec![record(0), record(1)],
                outbox: Vec::new(),
            },
        )
        .unwrap_or_else(|_| unreachable!("identity"));
        assert!(matches!(
            classify_repeat(Some(&payload), Some(1), &differing),
            OperationMatch::PayloadConflict { .. }
        ));
    }
}
