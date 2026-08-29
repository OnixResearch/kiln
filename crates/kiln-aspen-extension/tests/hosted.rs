#[path = "hosted/process.rs"]
mod process;
mod support;

use kiln_adapter_aspen::service::{LocalAspenService, LocalServiceFault, LocalServiceJournal};
use kiln_adapter_aspen::transport::{
    AspenServiceDecision, AspenServiceTransport, AspenTriggerClient, IngressSource,
    ServiceTransportOutcome, build_ingress,
};
use kiln_aspen_extension::KilnAspenCallback;
use kiln_provider_api::{PortFailure, TriggerIntakePort};

use support::{admitted, trigger};

// r[verify kiln.aspen_runtime.ingress] r[verify kiln.aspen_runtime.lifecycle]
#[test]
fn local_service_accepts_rejects_replays_and_recovers_after_restart() {
    let profile = admitted();
    let journal = LocalServiceJournal::new(profile.profile.generation);
    let service = LocalAspenService::new(
        profile.clone(),
        journal.clone(),
        KilnAspenCallback::new(profile.clone()),
    );
    let mut radicle = AspenTriggerClient::new(profile.clone(), IngressSource::Radicle, service);
    let accepted = radicle
        .accept(trigger(false))
        .unwrap_or_else(|_| unreachable!("accepted"));
    assert_eq!(
        accepted.terminal,
        kiln_provider_api::TriggerTerminal::Unknown
    );
    let replay = radicle
        .accept(trigger(false))
        .unwrap_or_else(|_| unreachable!("replay"));
    assert_eq!(replay.run_id, accepted.run_id);

    let rejected = radicle
        .accept(trigger(true))
        .unwrap_or_else(|_| unreachable!("rejected"));
    assert_eq!(
        rejected.terminal,
        kiln_provider_api::TriggerTerminal::Denied
    );

    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("ingress"));
    let mut restarted = LocalAspenService::new(
        profile.clone(),
        journal,
        KilnAspenCallback::new(profile.clone()),
    );
    let replay_outcome = restarted.submit(&profile, &ingress);
    assert!(matches!(
        replay_outcome,
        ServiceTransportOutcome::Acknowledged(_)
    ));
    let state = restarted
        .journal()
        .callback_state()
        .unwrap_or_else(|_| unreachable!("state"));
    assert_eq!(state.runs.len(), 2);
}

// r[verify kiln.aspen_runtime.failure]
#[test]
fn post_accept_disconnect_is_unknown_until_exact_reconciliation() {
    let profile = admitted();
    let journal = LocalServiceJournal::new(profile.profile.generation);
    journal
        .set_next_fault(LocalServiceFault::DisconnectAfterCommit)
        .unwrap_or_else(|_| unreachable!("fault"));
    let service = LocalAspenService::new(
        profile.clone(),
        journal,
        KilnAspenCallback::new(profile.clone()),
    );
    let mut client = AspenTriggerClient::new(profile.clone(), IngressSource::Operator, service);
    assert!(matches!(
        client.accept(trigger(false)),
        Err(PortFailure::Unknown(_))
    ));
    let operation = client
        .last_operation()
        .cloned()
        .unwrap_or_else(|| unreachable!("operation"));
    let reconciled = client.transport_mut().reconcile(&profile, &operation);
    let ServiceTransportOutcome::Acknowledged(ack) = reconciled else {
        unreachable!("exact operation reconciles");
    };
    assert_eq!(ack.decision, AspenServiceDecision::Accepted);
}
