use std::collections::BTreeSet;

use kiln_core::agent_orchestration::{LATTICE_AGENT_ADAPTER_KIND, LATTICE_AGENT_NODE_KIND};
use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{CONTRACT_VERSION_V1, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_core::workflow::WorkflowBounds;
use kiln_provider_api::{PortFailure, PortResult, role_matches_port};
use lattice_workflow_contract::{
    CapabilityProfile as LatticeCapabilityProfile, ContractLimits, ControlCapabilities,
    EffectClass, GRAPH_SCHEMA, Label, PROFILE_SCHEMA, REQUIRED_NON_CLAIMS, profile_identity,
    validate_profile,
};
use serde::{Deserialize, Serialize};

pub const PINNED_LATTICE_SOURCE: &str = "ssh://git@github.com/OnixResearch/lattice.git";
/// Imported contract and the handler that implements `shell_success` and
/// observed cancellation. `70496e67` is the previous MIT import and is not
/// this handler.
pub const PINNED_LATTICE_REVISION: &str = "85a38b6a4bceeae4bb2057cf157e21752100f1b5";
pub const PINNED_LATTICE_RUNTIME_REVISION: &str = PINNED_LATTICE_REVISION;
pub const PINNED_LATTICE_CONTRACT_LICENSE: &str = "AGPL-3.0-or-later";
pub const PINNED_LATTICE_CONTRACT_SCHEMA: &str = "lattice.workflow-request.v2";
pub const PINNED_LATTICE_OBSERVATION_SCHEMA: &str = "lattice.workflow-observation.v1";
pub const PINNED_LATTICE_GRAPH_SCHEMA: &str = GRAPH_SCHEMA;
pub const PINNED_LATTICE_PLATFORM: &str = "linux-x86_64";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LatticeAdapterLimits {
    pub workflow: WorkflowBounds,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_in_flight: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LatticeCohort {
    Ci,
    Agent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LatticeProfile {
    pub cohort: LatticeCohort,
    pub source: String,
    pub revision: String,
    pub license: String,
    pub provider: CapabilityProfile,
    pub contract: LatticeCapabilityProfile,
    pub limits: LatticeAdapterLimits,
}

impl LatticeProfile {
    pub fn pinned(profile_id: ProviderProfileId, limits: LatticeAdapterLimits) -> PortResult<Self> {
        Self::pinned_cohort(LatticeCohort::Ci, profile_id, limits)
    }

    pub fn pinned_agent(
        profile_id: ProviderProfileId,
        limits: LatticeAdapterLimits,
    ) -> PortResult<Self> {
        Self::pinned_cohort(LatticeCohort::Agent, profile_id, limits)
    }

    fn pinned_cohort(
        cohort: LatticeCohort,
        profile_id: ProviderProfileId,
        limits: LatticeAdapterLimits,
    ) -> PortResult<Self> {
        let shape = cohort_shape(cohort)?;
        let contract = LatticeCapabilityProfile {
            schema: PROFILE_SCHEMA.to_string(),
            graph_schema: GRAPH_SCHEMA.to_string(),
            platforms: vec![label(PINNED_LATTICE_PLATFORM)?],
            node_kinds: shape.node_kinds,
            effect_classes: shape.effect_classes,
            adapter_kinds: shape.adapter_kinds,
            controls: cancel_only_controls(),
            limits: ContractLimits {
                maximum_nodes: to_u32(limits.workflow.max_nodes, "lattice.maximum_nodes")?,
                maximum_edges: to_u32(limits.workflow.max_edges, "lattice.maximum_edges")?,
                maximum_log_references: to_u32(
                    limits.workflow.max_log_references,
                    "lattice.maximum_log_references",
                )?,
                maximum_log_read_bytes: to_u64(
                    limits.workflow.max_log_read_bytes,
                    "lattice.maximum_log_read_bytes",
                )?,
                maximum_total_log_bytes: to_u64(
                    limits.workflow.max_total_log_bytes,
                    "lattice.maximum_total_log_bytes",
                )?,
            },
            non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        };
        let operations = required_operations();
        let outcomes = required_outcomes();
        Ok(Self {
            cohort,
            source: PINNED_LATTICE_SOURCE.to_string(),
            revision: PINNED_LATTICE_REVISION.to_string(),
            license: PINNED_LATTICE_CONTRACT_LICENSE.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Workflow,
                namespace: ExternalRef::new(
                    "lattice",
                    "workflow-provider",
                    PINNED_LATTICE_REVISION,
                )?,
                operations,
                outcomes,
                limits: ProviderLimits {
                    max_request_bytes: limits.max_request_bytes,
                    max_result_bytes: limits.max_response_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            contract,
            limits,
        })
    }
}

// r[impl kiln.lattice.profile] r[impl kiln.lattice.nonclaims]
pub fn admit_profile(profile: &LatticeProfile) -> PortResult<()> {
    if profile.source != PINNED_LATTICE_SOURCE
        || profile.revision != PINNED_LATTICE_REVISION
        || profile.license != PINNED_LATTICE_CONTRACT_LICENSE
        || profile.contract.schema != PROFILE_SCHEMA
        || profile.contract.graph_schema != PINNED_LATTICE_GRAPH_SCHEMA
        || profile.limits.max_request_bytes == 0
        || profile.limits.max_response_bytes == 0
        || profile.limits.max_in_flight == 0
    {
        return Err(domain(
            "lattice_profile",
            "lattice.profile",
            "Lattice profile does not match the pinned contract cohort",
        ));
    }
    validate_profile(&profile.contract).map_err(contract_error)?;
    role_matches_port(&profile.provider, ProviderRole::Workflow)?;
    let expected_contract_limits = ContractLimits {
        maximum_nodes: to_u32(profile.limits.workflow.max_nodes, "lattice.maximum_nodes")?,
        maximum_edges: to_u32(profile.limits.workflow.max_edges, "lattice.maximum_edges")?,
        maximum_log_references: to_u32(
            profile.limits.workflow.max_log_references,
            "lattice.maximum_log_references",
        )?,
        maximum_log_read_bytes: to_u64(
            profile.limits.workflow.max_log_read_bytes,
            "lattice.maximum_log_read_bytes",
        )?,
        maximum_total_log_bytes: to_u64(
            profile.limits.workflow.max_total_log_bytes,
            "lattice.maximum_total_log_bytes",
        )?,
    };
    let expected_provider_limits = ProviderLimits {
        max_request_bytes: profile.limits.max_request_bytes,
        max_result_bytes: profile.limits.max_response_bytes,
        max_in_flight: profile.limits.max_in_flight,
    };
    let shape = cohort_shape(profile.cohort)?;
    if profile.provider.operations != required_operations()
        || profile.provider.outcomes != required_outcomes()
        || profile.provider.limits != expected_provider_limits
        || profile.contract.platforms != vec![label(PINNED_LATTICE_PLATFORM)?]
        || profile.contract.node_kinds != shape.node_kinds
        || profile.contract.effect_classes != shape.effect_classes
        || profile.contract.adapter_kinds != shape.adapter_kinds
        || profile.contract.controls != cancel_only_controls()
        || profile.contract.limits != expected_contract_limits
    {
        return Err(domain(
            "lattice_capability_profile",
            "lattice.profile.capabilities",
            "Lattice capability profile differs from the admitted cohort",
        ));
    }
    for operation in &profile.provider.operations {
        admit_operation(
            &profile.provider,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.provider.profile_id.clone(),
                role: ProviderRole::Workflow,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.provider.outcomes.clone(),
            },
        )?;
    }
    let _profile_identity = profile_identity(&profile.contract);
    Ok(())
}

struct CohortShape {
    node_kinds: Vec<Label>,
    effect_classes: Vec<EffectClass>,
    adapter_kinds: Vec<Label>,
}

fn cohort_shape(cohort: LatticeCohort) -> PortResult<CohortShape> {
    let shape = match cohort {
        LatticeCohort::Ci => CohortShape {
            node_kinds: vec![label("build")?, label("review")?, label("test")?],
            effect_classes: vec![EffectClass::Build, EffectClass::Test, EffectClass::Custom],
            adapter_kinds: vec![label("local")?, label("remote")?],
        },
        LatticeCohort::Agent => CohortShape {
            node_kinds: vec![label(LATTICE_AGENT_NODE_KIND)?],
            effect_classes: vec![EffectClass::Custom],
            adapter_kinds: vec![label(LATTICE_AGENT_ADAPTER_KIND)?],
        },
    };
    Ok(shape)
}

fn cancel_only_controls() -> ControlCapabilities {
    ControlCapabilities {
        cancel: true,
        pause: false,
        resume: false,
    }
}

fn required_operations() -> BTreeSet<Operation> {
    BTreeSet::from([
        Operation::Dispatch,
        Operation::Observe,
        Operation::Cancel,
        Operation::Reconcile,
        Operation::ReadLogs,
    ])
}

fn required_outcomes() -> BTreeSet<Outcome> {
    BTreeSet::from([
        Outcome::Pending,
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Cancelled,
        Outcome::Unknown,
    ])
}

fn label(value: &str) -> PortResult<Label> {
    Label::new(value.to_string()).map_err(|_| {
        domain(
            "lattice_label",
            "lattice.profile",
            "Lattice profile label is not canonical",
        )
    })
}

fn to_u32(value: usize, field: &'static str) -> PortResult<u32> {
    u32::try_from(value).map_err(|_| {
        domain(
            "lattice_limit",
            field,
            "Lattice count does not fit the contract",
        )
    })
}

fn to_u64(value: usize, field: &'static str) -> PortResult<u64> {
    u64::try_from(value).map_err(|_| {
        domain(
            "lattice_limit",
            field,
            "Lattice byte bound does not fit the contract",
        )
    })
}

fn contract_error(error: lattice_workflow_contract::ContractError) -> PortFailure {
    domain(
        "lattice_contract",
        error.path,
        "Lattice profile failed contract admission",
    )
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
