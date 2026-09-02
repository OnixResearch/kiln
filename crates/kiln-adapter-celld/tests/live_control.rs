//! Exercised only by tests/celld-live-cohort.sh against the pinned HTTPS cohort.
#[path = "support/live_cohort.rs"]
mod live_cohort;
use std::{
    env, fs,
    time::{SystemTime, UNIX_EPOCH},
};

use kiln_adapter_celld::{
    CELLD_LICENSE, CELLD_RELEASE, CELLD_REVISION, CELLD_TRUST_DOMAIN_SINGLE,
    CURRENT_SCHEMA_IDENTITY, CelldEventStore, CelldProfile, CelldProfileBounds,
    EMPTY_SCHEMA_IDENTITY, HMAC_SHA256_ID, HTTPS_SCHEME, HttpCelldRuntime, STORE_SCHEMA_VERSION,
    StoredSchema, WORKER_PROTOCOL_VERSION, lease_recovery_action, plan_migration, protocol,
};
use kiln_core::domain::Event;
use kiln_core::operations::{JournalRecord, LeaseState, OutboxRecord, RecoveryAction};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, ContentId, EffectId, ExternalRef, ProviderProfileId,
    RunId,
};
use kiln_provider_api::{EventAppendRequest, EventStorePort, PortFailure};

fn profile(origin: String) -> CelldProfile {
    let (selected_cohort, observed_cohort) = live_cohort::selected_and_observed();
    CelldProfile {
        celld_release: CELLD_RELEASE.to_string(),
        celld_revision: CELLD_REVISION.to_string(),
        license: CELLD_LICENSE.to_string(),
        worker_protocol_version: WORKER_PROTOCOL_VERSION,
        schema_version: STORE_SCHEMA_VERSION,
        origin,
        path: "/v1/append".to_string(),
        tls_policy: HTTPS_SCHEME.to_string(),
        auth_profile: HMAC_SHA256_ID.to_string(),
        redirects_disabled: true,
        trust_domain: CELLD_TRUST_DOMAIN_SINGLE.to_string(),
        output_gate_enabled: true,
        storage_probe_required: true,
        selected_cohort,
        observed_cohort,
        bounds: CelldProfileBounds {
            max_request_bytes: 1_048_576,
            max_response_bytes: 1_048_576,
            max_replay_page_bytes: 1_048_576,
            max_events_per_append: 32,
            max_outbox_records: 64,
            max_event_bytes: 65_536,
            max_partition_count: 256,
            max_lease_ticks: 300,
            max_migration_steps: 16,
            timeout_millis: 30_000,
        },
    }
}

fn request() -> EventAppendRequest {
    let stream = ExternalRef::new("kiln", "stream", "repo-live-controls-outbox")
        .unwrap_or_else(|_| unreachable!("stream"));
    let event = Event::RunStarted {
        command_id: CommandId::new("kiln-control-command-1")
            .unwrap_or_else(|_| unreachable!("command")),
        run_id: RunId::new("kiln-control-run-1").unwrap_or_else(|_| unreachable!("run")),
        attempt_id: AttemptId::new("kiln-control-attempt-1")
            .unwrap_or_else(|_| unreachable!("attempt")),
        provider_profile: ProviderProfileId::new("nix").unwrap_or_else(|_| unreachable!("profile")),
    };
    let identity =
        protocol::identity_from_event(&event).unwrap_or_else(|_| unreachable!("event identity"));
    EventAppendRequest {
        stream: stream.clone(),
        expected_revision: 0,
        records: vec![JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream,
            expected_revision: 0,
            event_identity: ExternalRef::new("kiln", "event-blake3", identity.as_str())
                .unwrap_or_else(|_| unreachable!("identity ref")),
            event,
        }],
        outbox: vec![OutboxRecord {
            effect_id: EffectId::new("kiln-control-effect-1")
                .unwrap_or_else(|_| unreachable!("effect")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("profile")),
            idempotency_key: ExternalRef::new("kiln", "outbox", "control-effect-1")
                .unwrap_or_else(|_| unreachable!("idempotency")),
            lease: LeaseState::Unclaimed,
        }],
    }
}

#[test]
#[ignore = "requires the pinned loopback Celld HTTPS cohort runner"]
fn actual_worker_controls() {
    let phase =
        env::var("CELLD_LIVE_PHASE").unwrap_or_else(|_| unreachable!("runner sets control phase"));
    let cert = fs::read(
        env::var("CELLD_LIVE_ROOT_CA")
            .unwrap_or_else(|_| unreachable!("runner sets scoped root CA")),
    )
    .unwrap_or_else(|_| unreachable!("read scoped CA"));
    let secret = env::var("CELLD_LIVE_SECRET")
        .unwrap_or_else(|_| unreachable!("runner sets HMAC secret"))
        .into_bytes();
    let runtime = HttpCelldRuntime::new(secret, &cert, 30_000)
        .unwrap_or_else(|_| unreachable!("strict TLS runtime"));
    let store = CelldEventStore::new(
        profile(
            env::var("CELLD_LIVE_ORIGIN")
                .unwrap_or_else(|_| unreachable!("runner sets HTTPS origin")),
        ),
        ExternalRef::new("celld", "hmac-key", "kiln-primary")
            .unwrap_or_else(|_| unreachable!("key reference")),
        runtime,
    );
    let request = request();
    let effect = request
        .outbox
        .first()
        .unwrap_or_else(|| unreachable!("fixture outbox"));
    let migration_stream = ExternalRef::new("kiln", "stream", "repo-live-controls-schema")
        .unwrap_or_else(|_| unreachable!("migration stream"));
    let owner = ExternalRef::new("kiln", "lease-owner", "cohort-worker-1")
        .unwrap_or_else(|_| unreachable!("owner"));
    match phase.as_str() {
        // r[verify kiln.celld.migrations.scenario.valid]
        "setup" => {
            assert_eq!(
                store
                    .read_schema(&migration_stream)
                    .unwrap_or_else(|_| unreachable!("virgin schema")),
                None
            );
            let prior = store
                .initialize_schema(&migration_stream)
                .unwrap_or_else(|_| unreachable!("initialize schema zero"));
            assert_eq!(prior.version, 0);
            assert_eq!(prior.identity.as_str(), EMPTY_SCHEMA_IDENTITY);
            assert_eq!(prior.revision, 0);
            let plan = plan_migration(0, 1, &[(0, 1)], 16)
                .unwrap_or_else(|_| unreachable!("pure admitted edge"));
            let migrated = store
                .apply_migration(&migration_stream, &prior, &plan)
                .unwrap_or_else(|_| unreachable!("signed migration"));
            assert_eq!(migrated.version, 1);
            assert_eq!(migrated.identity.as_str(), CURRENT_SCHEMA_IDENTITY);
            assert_eq!(migrated.revision, 0);
            let mut store = store;
            assert_eq!(
                store
                    .append(&request)
                    .unwrap_or_else(|_| unreachable!("append outbox")),
                1
            );
            let before = store
                .read_lease(&request.stream, &effect.effect_id)
                .unwrap_or_else(|_| unreachable!("signed unclaimed lease"));
            assert_eq!(before.lease.state, LeaseState::Unclaimed);
            assert_eq!(before.idempotency_key, effect.idempotency_key);
            assert_eq!(before.lease.attempts, 0);
            assert!(before.completion.is_none() && before.reconciliation.is_none());
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| unreachable!("clock after protocol epoch"))
                .as_secs();
            let mut next = before.clone();
            next.lease = kiln_adapter_celld::plan_lease_claim(&before.lease, owner, now, 300)
                .unwrap_or_else(|_| unreachable!("pure shell-time claim plan"));
            let claimed = store
                .apply_lease(&request.stream, &effect.effect_id, &before, &next, now, 300)
                .unwrap_or_else(|_| unreachable!("signed conditional claim"));
            assert_eq!(claimed.lease.state, LeaseState::Leased);
            assert_eq!(claimed.lease.lease_until, Some(now + 300));
            assert_eq!(claimed.lease.attempts, 1);
            let mut competing = before.clone();
            competing.lease = kiln_adapter_celld::plan_lease_claim(
                &before.lease,
                ExternalRef::new("kiln", "lease-owner", "competing-worker")
                    .unwrap_or_else(|_| unreachable!("other owner")),
                now,
                300,
            )
            .unwrap_or_else(|_| unreachable!("competing pure plan"));
            assert!(
                matches!(store.apply_lease(
                &request.stream, &effect.effect_id, &before, &competing, now, 300,
            ), Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_conflict"),
                "signed atomic Worker CAS must reject a stale prior lease"
            );
        }
        // r[verify kiln.celld.leases.scenario.restart]
        "restart" | "recovered" => {
            let schema = store
                .read_schema(&migration_stream)
                .unwrap_or_else(|_| unreachable!("signed migration read"))
                .unwrap_or_else(|| unreachable!("schema survives node restart"));
            assert_eq!(schema.version, 1);
            assert_eq!(schema.identity.as_str(), CURRENT_SCHEMA_IDENTITY);
            assert_eq!(schema.revision, 0);
            let prior = StoredSchema {
                version: 0,
                identity: ContentId::from_blake3_hex(EMPTY_SCHEMA_IDENTITY)
                    .unwrap_or_else(|_| unreachable!("prior identity")),
                revision: 0,
            };
            let plan = plan_migration(0, 1, &[(0, 1)], 16)
                .unwrap_or_else(|_| unreachable!("pure admitted edge"));
            assert_eq!(
                store
                    .apply_migration(&migration_stream, &prior, &plan)
                    .unwrap_or_else(|_| unreachable!(
                        "exact persisted migration receipt after restart"
                    )),
                schema,
            );
            let lease = store
                .read_lease(&request.stream, &effect.effect_id)
                .unwrap_or_else(|_| unreachable!("signed durable lease read"));
            assert_eq!(lease.lease.owner, Some(owner));
            let lease_until = lease
                .lease
                .lease_until
                .unwrap_or_else(|| unreachable!("persisted lease deadline"));
            assert_eq!(lease.lease.attempts, 1);
            assert_eq!(lease.idempotency_key, effect.idempotency_key);
            let completion =
                ExternalRef::new("kiln", "completion-evidence", "control-cohort-completion")
                    .unwrap_or_else(|_| unreachable!("completion evidence ref"));
            let reconciliation = ExternalRef::new(
                "kiln",
                "reconciliation-evidence",
                "control-cohort-observation",
            )
            .unwrap_or_else(|_| unreachable!("reconciliation evidence ref"));
            if phase == "restart" {
                assert_eq!(lease.lease.state, LeaseState::Leased);
                assert!(lease.completion.is_none() && lease.reconciliation.is_none());
                assert_eq!(
                    lease_recovery_action(&lease.lease, lease_until - 1),
                    RecoveryAction::Wait
                );
                assert_eq!(
                    lease_recovery_action(&lease.lease, lease_until + 1),
                    RecoveryAction::Reconcile
                );
                let snapshot = store
                    .load(&request.stream, 1)
                    .unwrap_or_else(|_| unreachable!("signed outbox read"));
                assert_eq!(snapshot.records, request.records);
                assert_eq!(
                    snapshot.outbox.first().map(|item| &item.lease),
                    Some(&LeaseState::Leased)
                );
                let terminal = kiln_adapter_celld::plan_lease_reconciled_completion(
                    &lease,
                    completion,
                    reconciliation,
                )
                .unwrap_or_else(|_| unreachable!("pure terminal evidence plan"));
                assert_eq!(
                    store
                        .apply_reconciled_completion(
                            &request.stream,
                            &effect.effect_id,
                            &lease,
                            &terminal,
                        )
                        .unwrap_or_else(|_| unreachable!(
                            "conditional durable terminal observation"
                        )),
                    terminal
                );
            } else {
                assert_eq!(lease.lease.state, LeaseState::Completed);
                assert_eq!(lease.completion, Some(completion));
                assert_eq!(lease.reconciliation, Some(reconciliation));
                assert_eq!(
                    lease_recovery_action(&lease.lease, lease_until + 1),
                    RecoveryAction::None
                );
                let snapshot = store
                    .load(&request.stream, 1)
                    .unwrap_or_else(|_| unreachable!("signed outbox read"));
                assert_eq!(snapshot.records, request.records);
                assert_eq!(
                    snapshot.outbox.first().map(|item| &item.lease),
                    Some(&LeaseState::Completed)
                );
                for (name, label) in [
                    ("repo-live-controls-gate-denied", "gate-disabled"),
                    ("repo-live-controls-mixed-denied", "mixed-fleet"),
                ] {
                    let fresh = ExternalRef::new("kiln", "stream", name)
                        .unwrap_or_else(|_| unreachable!("negative-control stream"));
                    let denied = store
                        .load(&fresh, 1)
                        .unwrap_or_else(|_| unreachable!("post-recovery signed read"));
                    assert!(
                        denied.records.is_empty() && denied.outbox.is_empty(),
                        "{label} phase must not have committed the append"
                    );
                }
            }
        }
        // r[verify kiln.celld.cohort.scenario.output_gate]
        "gate_disabled" => {
            assert!(
                matches!(
                    store.read_schema(&migration_stream),
                    Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_output_gate"
                ),
                "trusted shell observation must reject disabled output gating before dispatch"
            );
            let mut gate_request = request.clone();
            let fresh = ExternalRef::new("kiln", "stream", "repo-live-controls-gate-denied")
                .unwrap_or_else(|_| unreachable!("gate-negative stream"));
            gate_request.stream = fresh.clone();
            gate_request
                .records
                .first_mut()
                .unwrap_or_else(|| unreachable!("fixture gate record"))
                .stream = fresh;
            let mut store = store;
            assert!(
                matches!(
                    store.append(&gate_request),
                    Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_output_gate"
                ),
                "gate-disabled canonical append must reject before Worker dispatch"
            );
        }
        // r[verify kiln.celld.migrations.scenario.mixed]
        "mixed" => {
            assert!(
                matches!(
                    store.read_schema(&migration_stream),
                    Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_cohort_observation"
                ),
                "observed incompatible Worker schema must reject before touching its object"
            );
            assert!(
                matches!(
                    store.read_lease(&request.stream, &effect.effect_id),
                    Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_cohort_observation"
                ),
                "incompatible fleet cannot serve signed lease state"
            );
            let mut mixed_request = request.clone();
            let fresh = ExternalRef::new("kiln", "stream", "repo-live-controls-mixed-denied")
                .unwrap_or_else(|_| unreachable!("mixed-negative stream"));
            mixed_request.stream = fresh.clone();
            mixed_request
                .records
                .first_mut()
                .unwrap_or_else(|| unreachable!("fixture mixed record"))
                .stream = fresh;
            let mut store = store;
            assert!(
                matches!(
                    store.append(&mixed_request),
                    Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "celld_cohort_observation"
                ),
                "mixed-fleet canonical append must reject before Worker dispatch"
            );
            assert!(
                store.initialize_schema(&migration_stream).is_err(),
                "incompatible fleet cannot mutate schema"
            );
        }
        _ => unreachable!("unknown runner control phase"),
    }
    println!("live signed control phase {phase} completed");
}
