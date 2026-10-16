use kiln_adapter_aspen::protocol::{AspenEffectKind, CALLBACK_EFFECT_SCHEMA};
use kiln_adapter_aspen::transport::canonical_trigger_identity;
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};

use super::*;

const TRIGGER_MAXIMUM_BYTES: usize = 4_096;

fn reference(kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new("fixture", kind, id).unwrap_or_else(|_| unreachable!())
}

fn ingress() -> AspenServiceIngress {
    let trigger = TriggerRequest {
        repository: reference("repository", "seaglass"),
        actor: reference("actor", "author"),
        before: Some(reference(
            "revision",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )),
        after: reference("revision", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        branch: Some(Alias::new("master").unwrap_or_else(|_| unreachable!())),
        patch: None,
        event: TriggerEventKind::Push,
    };
    let trigger_identity = canonical_trigger_identity(&trigger, TRIGGER_MAXIMUM_BYTES)
        .unwrap_or_else(|_| unreachable!());
    AspenServiceIngress {
        schema: kiln_adapter_aspen::transport::SERVICE_INGRESS_SCHEMA.to_string(),
        source: kiln_adapter_aspen::transport::IngressSource::Radicle,
        service_id: "kiln.aspen".to_string(),
        generation: 1,
        profile_identity: ContentId::from_blake3_hex(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap_or_else(|_| unreachable!()),
        alpn: "kiln".to_string(),
        framing: "json".to_string(),
        operation_id: reference("operation", "one"),
        idempotency_key: reference("idempotency", "one"),
        authority_ref: "authority".to_string(),
        resource_ref: "resource".to_string(),
        trigger_identity,
        trigger,
    }
}

fn intent(record: &DurableOperationRecord) -> DurableEffectIntent {
    DurableEffectIntent {
        callback_receipt_ref:
            "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        binding_ref: "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            .to_string(),
        port_id: "transport".to_string(),
        port_version: "v1".to_string(),
        request_ref: "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            .to_string(),
        generation: record.ingress.generation,
        output_schema_ref:
            "blake3:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
        plan: CallbackEffectPlan {
            schema: CALLBACK_EFFECT_SCHEMA.to_string(),
            effect_id: kiln_core::types::EffectId::new("effect-one")
                .unwrap_or_else(|_| unreachable!()),
            operation_id: record.ingress.operation_id.clone(),
            idempotency_key: record.ingress.idempotency_key.clone(),
            kind: AspenEffectKind::TransportSession,
            provider_profile: ProviderProfileId::new("lattice").unwrap_or_else(|_| unreachable!()),
            request_identity: record.ingress.trigger_identity.clone(),
            generation: record.ingress.generation,
            authority_ref: record.ingress.authority_ref.clone(),
            resource_ref: record.ingress.resource_ref.clone(),
        },
    }
}

#[test]
fn valid_operation_moves_from_reservation_to_dispatch_and_uncertainty() {
    let reserved = DurableOperationRecord::reserve(ingress()).unwrap_or_else(|_| unreachable!());
    let callback = intent(&reserved).callback_receipt_ref;
    let callback_bound = reserved
        .with_callback_receipt(&callback)
        .unwrap_or_else(|_| unreachable!());
    let dispatching = callback_bound
        .with_dispatch_intent(intent(&callback_bound))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(dispatching.state, DurableOperationState::Dispatching);
    let uncertain = dispatching
        .with_uncertainty()
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(uncertain.state, DurableOperationState::Uncertain);
}

#[test]
fn conflicting_callback_and_invalid_terminal_state_fail_closed() {
    let reserved = DurableOperationRecord::reserve(ingress()).unwrap_or_else(|_| unreachable!());
    let first = reserved
        .with_callback_receipt(
            "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap_or_else(|_| unreachable!());
    assert!(
        first
            .with_callback_receipt(
                "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            )
            .is_err()
    );
    let mut corrupt = first;
    corrupt.state = DurableOperationState::Terminal;
    assert!(corrupt.validate().is_err());
}
