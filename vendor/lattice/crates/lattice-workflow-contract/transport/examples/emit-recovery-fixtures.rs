#![allow(
    unknown_lints,
    clippy::expect_used,
    non_trait_imports,
    no_unwrap,
    assertion_density,
    reason = "the recovery fixture generator stops when reviewed contract inputs drift"
)]

use lattice_workflow_contract::CapabilityProfile;
use lattice_workflow_contract::WorkflowAccepted;
use lattice_workflow_contract::WorkflowRequest;
use lattice_workflow_transport::*;
use serde::Deserialize;
use serde::Serialize;

const CONTRACT_FIXTURES: &str = include_str!("../../fixtures/producer-fixtures-v1.json");

#[derive(Deserialize)]
struct ContractFixtures {
    profile: CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
}

#[derive(Serialize)]
struct RecoveryFixtures {
    schema: &'static str,
    request: ExchangeRequest,
    accepted_response: ExchangeResponse,
    not_found_response: ExchangeResponse,
}

fn main() {
    let contract: ContractFixtures = serde_json::from_str(CONTRACT_FIXTURES).expect("contract fixtures decode");
    let request = new_request(ExchangeCommand::RecoverDispatch {
        profile: contract.profile,
        request: contract.request,
    })
    .expect("recovery request is admitted");
    let accepted_response = new_response(
        request.identity.clone(),
        ExchangeResult::Success(Box::new(ExchangeSuccess::Accepted(contract.accepted))),
    )
    .expect("recovery acceptance is admitted");
    let not_found_response = new_response(
        request.identity.clone(),
        ExchangeResult::Rejected(Rejection {
            code: RejectionCode::NotFound,
            field: "exchange.handler".to_string(),
        }),
    )
    .expect("recovery not-found response is admitted");
    let fixtures = RecoveryFixtures {
        schema: "lattice.workflow-transport-recovery-fixtures.v1",
        request,
        accepted_response,
        not_found_response,
    };
    println!("{}", serde_json::to_string_pretty(&fixtures).expect("recovery fixtures serialize"));
}
