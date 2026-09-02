#![allow(
    unknown_lints,
    clippy::expect_used,
    non_trait_imports,
    no_unwrap,
    assertion_density,
    reason = "the fixture generator uses checked producer values and aborts when checked-in contract fixtures drift"
)]

use lattice_workflow_contract::CapabilityProfile;
use lattice_workflow_contract::WorkflowAccepted;
use lattice_workflow_contract::WorkflowRequest;
use lattice_workflow_transport::*;
use serde::Deserialize;
use serde::Serialize;

const CONTRACT_FIXTURES: &str = include_str!("../../fixtures/producer-fixtures-v1.json");
const FIXTURE_FRAME_BYTES: u32 = 1_048_576;
const FIXTURE_LOG_BYTES: u32 = 65_536;
const FIXTURE_CONNECTIONS: u32 = 8;
const FIXTURE_DEADLINE_MILLISECONDS: u64 = 30_000;
const FIXTURE_USER_ID: u32 = 1_000;

#[derive(Deserialize)]
struct ContractFixtures {
    profile: CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
}

#[derive(Serialize)]
struct TransportFixtures {
    schema: &'static str,
    profile: TransportProfile,
    request: ExchangeRequest,
    response: ExchangeResponse,
    rejected: RejectedFixtures,
}

#[derive(Serialize)]
struct RejectedFixtures {
    changed_body_with_stale_identity: ExchangeRequest,
    oversized_profile: TransportProfile,
}

fn main() {
    let contract: ContractFixtures = serde_json::from_str(CONTRACT_FIXTURES).expect("contract fixtures decode");
    let profile = transport_profile();
    let request = new_request(ExchangeCommand::Dispatch {
        profile: contract.profile,
        request: contract.request,
    })
    .expect("transport request is admitted");
    let response = new_response(
        request.identity.clone(),
        ExchangeResult::Success(Box::new(ExchangeSuccess::Accepted(contract.accepted))),
    )
    .expect("transport response is admitted");
    let rejected = rejected_fixtures(&profile, &request);
    let fixtures = TransportFixtures {
        schema: "lattice.workflow-transport-producer-fixtures.v1",
        profile,
        request,
        response,
        rejected,
    };
    println!("{}", serde_json::to_string_pretty(&fixtures).expect("transport fixtures serialize"));
}

fn transport_profile() -> TransportProfile {
    TransportProfile {
        schema: TRANSPORT_PROFILE_SCHEMA.to_string(),
        limits: TransportLimits {
            maximum_frame_bytes: FIXTURE_FRAME_BYTES,
            maximum_log_bytes: FIXTURE_LOG_BYTES,
            maximum_connections: FIXTURE_CONNECTIONS,
            read_deadline_milliseconds: FIXTURE_DEADLINE_MILLISECONDS,
            write_deadline_milliseconds: FIXTURE_DEADLINE_MILLISECONDS,
        },
        peers: PeerPolicy {
            allowed_user_ids: vec![FIXTURE_USER_ID],
        },
        non_claims: REQUIRED_NON_CLAIMS.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn rejected_fixtures(profile: &TransportProfile, request: &ExchangeRequest) -> RejectedFixtures {
    let mut changed = request.clone();
    if let ExchangeCommand::Dispatch { request, .. } = &mut changed.command {
        request.idempotency_key = request.kiln.effect_identity.clone();
    }
    let mut oversized_profile = profile.clone();
    oversized_profile.limits.maximum_frame_bytes = MAXIMUM_FRAME_BYTES.saturating_add(1);
    RejectedFixtures {
        changed_body_with_stale_identity: changed,
        oversized_profile,
    }
}
