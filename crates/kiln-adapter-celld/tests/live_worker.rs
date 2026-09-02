//! Executed only by tests/celld-live-cohort.sh against pinned Celld and loopback HTTPS.
#[path = "support/live_cohort.rs"]
mod live_cohort;
use std::env;
use std::fs;

use kiln_adapter_celld::{
    CELLD_LICENSE, CELLD_RELEASE, CELLD_REVISION, CELLD_TRUST_DOMAIN_SINGLE, CelldEventStore,
    CelldProfile, CelldProfileBounds, CelldRuntime, DispatchOutcome, HMAC_SHA256_ID, HTTPS_SCHEME,
    HttpCelldRuntime, OperationObservation, STORE_SCHEMA_VERSION, WORKER_PROTOCOL_VERSION,
    protocol,
};
use kiln_core::domain::Event;
use kiln_core::operations::{JournalRecord, LeaseState, OutboxRecord};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, ProviderProfileId, RunId,
};
use kiln_provider_api::{EventAppendRequest, EventStorePort, EventStoreSnapshot, PortFailure};

struct DropAcceptedResponse {
    inner: HttpCelldRuntime,
    drop_next: bool,
}

impl CelldRuntime for DropAcceptedResponse {
    fn dispatch(
        &mut self,
        profile: &CelldProfile,
        envelope: &protocol::AuthEnvelope,
    ) -> DispatchOutcome {
        let answer = self.inner.dispatch(profile, envelope);
        if self.drop_next && matches!(answer, DispatchOutcome::Response(_)) {
            self.drop_next = false;
            DispatchOutcome::Disconnected
        } else {
            answer
        }
    }

    fn reconcile(
        &self,
        profile: &CelldProfile,
        envelope: &protocol::AuthEnvelope,
    ) -> Result<OperationObservation, PortFailure> {
        self.inner.reconcile(profile, envelope)
    }

    fn load(
        &self,
        profile: &CelldProfile,
        envelope: &protocol::AuthEnvelope,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        self.inner.load(profile, envelope)
    }
}

fn request() -> EventAppendRequest {
    let stream = ExternalRef::new("kiln", "stream", "repo-live-builds")
        .unwrap_or_else(|_| unreachable!("stream"));
    let event = Event::RunStarted {
        command_id: CommandId::new("kiln-live-command-1")
            .unwrap_or_else(|_| unreachable!("command")),
        run_id: RunId::new("kiln-live-run-1").unwrap_or_else(|_| unreachable!("run")),
        attempt_id: AttemptId::new("kiln-live-attempt-1")
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
                .unwrap_or_else(|_| unreachable!("event identity ref")),
            event,
        }],
        outbox: vec![OutboxRecord {
            effect_id: EffectId::new("kiln-live-effect-1")
                .unwrap_or_else(|_| unreachable!("effect")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("profile")),
            idempotency_key: ExternalRef::new("kiln", "outbox", "live-effect-1")
                .unwrap_or_else(|_| unreachable!("idempotency key")),
            lease: LeaseState::Unclaimed,
        }],
    }
}

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

// r[verify kiln.celld.cohort.scenario.valid]
// r[verify kiln.celld.atomic_append.scenario.commit]
// r[verify kiln.celld.ambiguity.scenario.disconnect]
#[test]
#[ignore = "requires the loopback Celld cohort runner tests/celld-live-cohort.sh"]
fn actual_worker_persists_event_outbox_and_reconciles_a_lost_append_response() {
    let origin =
        env::var("CELLD_LIVE_ORIGIN").unwrap_or_else(|_| unreachable!("runner sets origin"));
    let cert = fs::read(
        env::var("CELLD_LIVE_ROOT_CA")
            .unwrap_or_else(|_| unreachable!("runner sets test-only root CA")),
    )
    .unwrap_or_else(|_| unreachable!("runner CA readable"));
    let secret = env::var("CELLD_LIVE_SECRET")
        .unwrap_or_else(|_| unreachable!("runner sets HMAC key"))
        .into_bytes();
    let make_runtime = || {
        HttpCelldRuntime::new(secret.clone(), &cert, 30_000)
            .unwrap_or_else(|_| unreachable!("TLS client with scoped CA"))
    };
    let profile = profile(origin);
    let key = ExternalRef::new("celld", "hmac-key", "kiln-primary")
        .unwrap_or_else(|_| unreachable!("key ref"));
    let request = request();
    // Both operator strings can agree while the actual TLS root installed in
    // this client differs. This must fail before any signed Worker request.
    let mut forged_ingress = profile.clone();
    let wrong_ca_digest = if forged_ingress
        .selected_cohort
        .ingress_ca_blake3
        .starts_with('a')
    {
        "b"
    } else {
        "a"
    }
    .repeat(64);
    forged_ingress.selected_cohort.ingress_ca_blake3 = wrong_ca_digest.clone();
    forged_ingress.observed_cohort.identity.ingress_ca_blake3 = wrong_ca_digest;
    let mismatched = CelldEventStore::new(forged_ingress, key.clone(), make_runtime());
    assert!(
        matches!(
            mismatched.load(&request.stream, 1),
            Err(PortFailure::Domain(failure)) if failure.code == "celld_ingress_ca"
        ),
        "actual installed ingress CA must match selected and observed digest before dispatch"
    );
    // The same DER root in a byte-different PEM remains a valid TLS root, but
    // cannot inherit the operator-selected identity of the original PEM.
    let mut altered_ca = cert.clone();
    altered_ca.push(b'\n');
    let altered_runtime = HttpCelldRuntime::new(secret.clone(), &altered_ca, 30_000)
        .unwrap_or_else(|_| unreachable!("byte-different CA remains a valid TLS root"));
    let altered = CelldEventStore::new(profile.clone(), key.clone(), altered_runtime);
    assert!(
        matches!(
            altered.load(&request.stream, 1),
            Err(PortFailure::Domain(failure)) if failure.code == "celld_ingress_ca"
        ),
        "byte-different installed PEM must not dispatch under the original ingress identity"
    );

    let mut store = CelldEventStore::new(
        profile.clone(),
        key.clone(),
        DropAcceptedResponse {
            inner: make_runtime(),
            drop_next: true,
        },
    );
    assert!(matches!(
        store.append(&request),
        Err(PortFailure::Unknown(_))
    ));
    assert_eq!(
        store
            .load(&request.stream, 1)
            .unwrap_or_else(|_| unreachable!("authenticated durable read"))
            .records,
        request.records
    );

    let mut restarted = CelldEventStore::new(profile.clone(), key.clone(), make_runtime());
    assert_eq!(
        restarted
            .append(&request)
            .unwrap_or_else(|_| unreachable!("exact committed operation")),
        1
    );
    let snapshot = restarted
        .load(&request.stream, 1)
        .unwrap_or_else(|_| unreachable!("persisted replay"));
    assert_eq!(snapshot.records, request.records);
    assert_eq!(snapshot.outbox, request.outbox);
    // A contradictory operator selection cannot override the served Worker
    // deployment identity independently observed over the admitted TLS origin.
    let mut contradictory = profile;
    contradictory.selected_cohort.deployment_id = "ffffffffffffffff".to_string();
    contradictory.observed_cohort.identity.deployment_id = "ffffffffffffffff".to_string();
    let wrong = CelldEventStore::new(contradictory, key, make_runtime());
    assert!(matches!(
        wrong.load(&request.stream, 1),
        Err(PortFailure::Unknown(failure)) if failure.code == "celld_cohort_response"
    ));
    println!(
        "authenticated HTTPS: accepted then lost response; exact reconcile=1; stored event and outbox replayed"
    );
}
