use std::collections::BTreeSet;

use kiln_adapter_aspen::{AspenServiceAck, profile_document_fixture};
use kiln_adapter_radicle::status::{
    PinnedSecretSpecExecutable, RadicleStatusProfile, SecretSpecDeclaration, StatusOutcome,
};

use super::*;
use crate::project::tests::result;

fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
}
pub(crate) fn fixture() -> (
    CiProfile,
    ProjectWorkerProfile,
    AdmittedAspenRuntimeProfile,
    CiJournal,
    ProjectWorkerResult,
) {
    let (worker, id, trigger, result) = result();
    let status = RadicleStatusProfile::pinned(
        BTreeSet::from([trigger.repository.native_id.as_str().to_owned()]),
        reference(
            "radicle",
            "node",
            "z6Mkon3Necd6NkkyfoGoHxid2znGc59LU3K7mubaRcFbLfLX",
        ),
        SecretSpecDeclaration {
            project_root: "/tmp/krc-secrets".into(),
            project: "fixture".into(),
            profile: "fixture".into(),
            secret_key: "RADICLE_SIGNER_SEED".into(),
            provider: Some("keyring".into()),
        },
        PinnedSecretSpecExecutable {
            path: kiln_adapter_radicle::status::PINNED_SECRETSPEC_EXECUTABLE_PATH.into(),
            blake3: kiln_adapter_radicle::status::PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.into(),
            version: kiln_adapter_radicle::status::PINNED_SECRETSPEC_VERSION.into(),
        },
    );
    let profile = CiProfile {
        schema: crate::coordinator::PROFILE_SCHEMA.into(),
        worker_profile_path: "/tmp/krc-worker.json".into(),
        worker_profile_identity: id,
        runtime_profile: profile_document_fixture(),
        socket_path: "/tmp/krc.sock".into(),
        maximum_frame_bytes: 1_048_576,
        transport_timeout_seconds: 1,
        max_journal_bytes: 4_194_304,
        max_journal_records: 64,
        maximum_polls: 4,
        poll_interval_milliseconds: 1,
        status_storage_root: "/tmp/krc-status".into(),
        status_profile: status,
    };
    let runtime = profile.admit().unwrap_or_else(|_| unreachable!());
    let journal = CiJournal::fresh(
        &profile,
        &worker,
        &runtime,
        trigger,
        result.source_intent.clone(),
    )
    .unwrap_or_else(|_| unreachable!());
    (profile, worker, runtime, journal, result)
}
pub(crate) fn native(
    journal: &CiJournal,
    worker: &ProjectWorkerProfile,
    decision: AspenServiceDecision,
) -> ServiceTransportOutcome {
    let run_id = worker
        .bindings(&journal.source_intent.profile_identity, &journal.source)
        .unwrap_or_else(|_| unreachable!())
        .run_id;
    ServiceTransportOutcome::Acknowledged(AspenServiceAck {
        schema: SERVICE_ACK_SCHEMA.into(),
        operation_id: journal.ingress.operation_id.clone(),
        generation: journal.ingress.generation,
        trigger_identity: journal.ingress.trigger_identity.clone(),
        run_id: Some(run_id),
        decision,
        receipt_ref: reference("aspen", "receipt", "actual-terminal-fixture"),
    })
}

// r[verify kiln.radicle_ci.journal] r[verify kiln.radicle_ci.recovery]
#[test]
fn native_or_worker_alone_cannot_complete_a_reserved_attempt() {
    let (profile, worker, runtime, mut journal, result) = fixture();
    let attempt = worker
        .bindings(&profile.worker_profile_identity, &journal.source)
        .unwrap_or_else(|_| unreachable!())
        .attempt_id;
    journal
        .native_observations
        .push(native(&journal, &worker, AspenServiceDecision::Completed));
    journal.seal().unwrap_or_else(|_| unreachable!());
    let state = journal
        .validate(&profile, &worker, &runtime)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state.attempts.get(&attempt).map(|attempt| attempt.outcome),
        Some(Outcome::Pending)
    );
    assert_eq!(journal.completion(&profile, &worker), Ok(None));
    assert_eq!(journal.status_plan(&profile, &worker, &state), Ok(None));
    journal.native_observations.clear();
    journal
        .native_observations
        .push(ServiceTransportOutcome::Unavailable);
    journal.worker_result = Some(result);
    journal.seal().unwrap_or_else(|_| unreachable!());
    let state = journal
        .validate(&profile, &worker, &runtime)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state.attempts.get(&attempt).map(|attempt| attempt.outcome),
        Some(Outcome::Pending)
    );
    assert_eq!(journal.completion(&profile, &worker), Ok(None));
    assert_eq!(journal.status_plan(&profile, &worker, &state), Ok(None));
}

// r[verify kiln.radicle_ci.journal] r[verify kiln.radicle_ci.status]
#[test]
fn matching_evidence_replays_exact_products_and_revision_bound_status() {
    let (profile, worker, runtime, mut journal, result) = fixture();
    journal
        .native_observations
        .push(native(&journal, &worker, AspenServiceDecision::Completed));
    let products = admit_result(
        &worker,
        &profile.worker_profile_identity,
        &journal.trigger,
        &result,
    )
    .unwrap_or_else(|_| unreachable!())
    .1;
    journal.worker_result = Some(result);
    let command = journal
        .completion(&profile, &worker)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    journal
        .append(&profile, command, "complete")
        .unwrap_or_else(|_| unreachable!());
    journal.seal().unwrap_or_else(|_| unreachable!());
    let state = journal
        .validate(&profile, &worker, &runtime)
        .unwrap_or_else(|_| unreachable!());
    let bindings = worker
        .bindings(&profile.worker_profile_identity, &journal.source)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .attempts
            .get(&bindings.attempt_id)
            .map(|attempt| attempt.outcome),
        Some(Outcome::Success)
    );
    assert_eq!(
        state
            .attempts
            .get(&bindings.attempt_id)
            .map(|attempt| &attempt.products),
        Some(&products)
    );
    let plan = journal
        .status_plan(&profile, &worker, &state)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(plan.revision, journal.trigger.after);
    assert_eq!(plan.run_id, bindings.run_id);
    assert_eq!(plan.outcome, StatusOutcome::Success);
    journal.status_intent = Some(plan.clone());
    journal.seal().unwrap_or_else(|_| unreachable!());
    assert_eq!(journal.validate(&profile, &worker, &runtime), Ok(state));
    journal
        .status_intent
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .revision = reference("radicle", "revision", &"a".repeat(40));
    journal.seal().unwrap_or_else(|_| unreachable!());
    assert!(journal.validate(&profile, &worker, &runtime).is_err());
}

// r[verify kiln.radicle_ci.journal] r[verify kiln.radicle_ci.source]
#[test]
fn substituted_operation_source_and_conflicting_native_outcomes_are_rejected() {
    let (profile, worker, runtime, mut journal, result) = fixture();
    let mut ack = native(&journal, &worker, AspenServiceDecision::Completed);
    if let ServiceTransportOutcome::Acknowledged(ack) = &mut ack {
        ack.operation_id = reference("aspen", "operation", "different");
    }
    journal.native_observations.push(ack);
    journal.seal().unwrap_or_else(|_| unreachable!());
    assert!(journal.validate(&profile, &worker, &runtime).is_err());
    journal.native_observations = vec![native(&journal, &worker, AspenServiceDecision::Failed)];
    journal.worker_result = Some(result.clone());
    assert!(journal.completion(&profile, &worker).is_err());
    journal.native_observations = vec![native(&journal, &worker, AspenServiceDecision::Completed)];
    journal
        .worker_result
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .source_intent
        .pinned_profile
        .request
        .expected_sigrefs_commit = "a".repeat(40);
    assert!(journal.completion(&profile, &worker).is_err());
    journal.worker_result = Some(result);
    assert!(
        journal
            .completion(&profile, &worker)
            .unwrap_or_else(|_| unreachable!())
            .is_some()
    );
}
