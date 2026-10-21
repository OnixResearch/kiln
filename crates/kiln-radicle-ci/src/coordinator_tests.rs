use super::*;
use crate::journal::tests::{fixture, native};
use crate::project::identity;
use kiln_core::domain::Outcome;
use kiln_core::types::ExternalRef;

fn service_fixture() -> (CiService, CiJournal, ProjectWorkerResult) {
    let (profile, worker, runtime, journal, result) = fixture();
    let status = RadicleStatusShell::new(
        profile.status_storage_root.clone(),
        profile.status_profile.clone(),
    )
    .unwrap_or_else(|_| unreachable!());
    (
        CiService {
            profile,
            worker,
            runtime,
            status,
        },
        journal,
        result,
    )
}

// r[verify kiln.radicle_ci.journal] r[verify kiln.radicle_ci.recovery]
#[test]
fn rejected_worker_evidence_cannot_poison_a_reserved_journal() {
    let (service, mut journal, result) = service_fixture();
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let mut bindings = service
        .bindings(&journal.source)
        .unwrap_or_else(|_| unreachable!());
    bindings.root = directory.path().into();
    journal.native_observations.push(native(
        &journal,
        &service.worker,
        AspenServiceDecision::Completed,
    ));
    journal.seal().unwrap_or_else(|_| unreachable!());
    let before = journal.clone();
    let path = bindings.root.join(PROJECT_RESULT_FILE);
    let mut substituted = result.clone();
    substituted.profile_identity =
        identity(&"different-profile").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &path,
        serde_json::to_vec(&substituted).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(
        service
            .load_completed_worker(&mut journal, &bindings)
            .is_err()
    );
    assert_eq!(journal, before);
    std::fs::write(
        &path,
        serde_json::to_vec(&result).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    service
        .load_completed_worker(&mut journal, &bindings)
        .unwrap_or_else(|_| unreachable!());
    service
        .admit_completion(&mut journal)
        .unwrap_or_else(|_| unreachable!());
    journal.seal().unwrap_or_else(|_| unreachable!());
    let state = journal
        .validate(&service.profile, &service.worker, &service.runtime)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .attempts
            .get(&bindings.attempt_id)
            .map(|attempt| attempt.outcome),
        Some(Outcome::Success)
    );
}

// r[verify kiln.radicle_ci.recovery] r[verify kiln.radicle_ci.journal]
#[test]
fn repeated_progress_preserves_dispatch_and_can_admit_late_completion() {
    let (service, mut journal, result) = service_fixture();
    let first = ServiceTransportOutcome::DisconnectedAfterAcceptance {
        operation_id: journal.ingress.operation_id.clone(),
    };
    service
        .record_native(&mut journal, first.clone())
        .unwrap_or_else(|_| unreachable!());
    for number in 0..service.profile.max_journal_records * 2 {
        let mut progress = native(&journal, &service.worker, AspenServiceDecision::Accepted);
        if let ServiceTransportOutcome::Acknowledged(ack) = &mut progress {
            ack.receipt_ref = ExternalRef::new("aspen", "receipt", &format!("progress-{number}"))
                .unwrap_or_else(|_| unreachable!());
        }
        service
            .record_native(&mut journal, progress)
            .unwrap_or_else(|_| unreachable!());
    }
    assert_eq!(journal.native_observations.first(), Some(&first));
    let completed = native(&journal, &service.worker, AspenServiceDecision::Completed);
    service
        .record_native(&mut journal, completed)
        .unwrap_or_else(|_| unreachable!());
    for number in 0..service.profile.max_journal_records * 2 {
        let mut repeated = native(&journal, &service.worker, AspenServiceDecision::Completed);
        if let ServiceTransportOutcome::Acknowledged(ack) = &mut repeated {
            ack.receipt_ref =
                ExternalRef::new("aspen", "receipt", &format!("terminal-repeat-{number}"))
                    .unwrap_or_else(|_| unreachable!());
        }
        service
            .record_native(&mut journal, repeated)
            .unwrap_or_else(|_| unreachable!());
    }
    journal.worker_result = Some(result);
    service
        .admit_completion(&mut journal)
        .unwrap_or_else(|_| unreachable!());
    journal.seal().unwrap_or_else(|_| unreachable!());
    let state = journal
        .validate(&service.profile, &service.worker, &service.runtime)
        .unwrap_or_else(|_| unreachable!());
    let bindings = service
        .bindings(&journal.source)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        state
            .attempts
            .get(&bindings.attempt_id)
            .map(|attempt| attempt.outcome),
        Some(Outcome::Success)
    );
    let conflicting = native(&journal, &service.worker, AspenServiceDecision::Failed);
    service
        .record_native(&mut journal, conflicting)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        journal
            .completion(&service.profile, &service.worker)
            .is_err()
    );
}
