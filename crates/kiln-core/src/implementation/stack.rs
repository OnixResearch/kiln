//! Opt-in publication of one verified attempt as an ordered stack (#89).
//!
//! A stack decomposes the attempt's one verified candidate along its ancestry:
//! the candidate is the top layer and every lower layer is an exact ancestor
//! with its own canonical CI verification under the same rules as #46. This
//! module resolves the publication route, admits the stack plan and its
//! retained provider identities, gates landing from the bottom up and admits
//! integration for an exact bottom prefix. It performs no effect: branches,
//! pull requests, the provider stack and merge observation are shell work.
// r[impl kiln.implementation.stack]
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::publication::{
    MAX_BODY_BYTES, MAX_TITLE_BYTES, bounded, bounded_branch, github_repository, push_external,
};
use super::verification::{admit_verification_run, push_run};
use super::{
    AttemptPhase, CandidateHandoff, CandidateVerification, ImplementationAttempt,
    ImplementationWork, IntegrationMapping, PUBLICATION_SCHEMA, PublicationPlan, TerminalPolicy,
    content_identity, push_str,
};
use crate::change_review::CiFact;
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const STACK_SCHEMA: &str = "kiln.implementation-stack.v1";
pub const LAYER_VERIFICATION_SCHEMA: &str = "kiln.implementation-stack-layer-verification.v1";
pub const STACK_PUBLICATION_SCHEMA: &str = "kiln.implementation-stack-publication.v1";
pub const STACK_INTEGRATION_SCHEMA: &str = "kiln.implementation-stack-integration.v1";
/// A GitHub stack holds at least two pull requests.
pub const MIN_STACK_LAYERS: usize = 2;
/// Kiln's bound on layers; GitHub admits up to 100.
pub const MAX_STACK_LAYERS: usize = 16;
/// Largest refusal or unknown code a gate state may carry.
pub const MAX_STATE_CODE_BYTES: usize = 128;
const BRANCH_DOMAIN: &[u8] = b"kiln.implementation-stack-branch.v1\0";
/// Gate code of an integrated layer above one that has not landed.
pub const OUT_OF_ORDER: &str = "implementation_stack_out_of_order";

/// Where a verified attempt is published. The single pull request is the default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationRoute {
    /// The existing single verified pull request (#47).
    #[default]
    PullRequest,
    /// One GitHub stack of exact layer pull requests.
    GithubStack,
    /// Refused until an Origin collaboration adapter is admitted.
    OriginStack,
}

/// Resolve the route before any effect: a host without stacked pull requests
/// keeps single pull-request publication, and Origin is refused.
// r[impl kiln.implementation.stack.scenario.route]
pub fn resolve_route(
    requested: PublicationRoute,
    host_supports_stacks: bool,
) -> CoreResult<PublicationRoute> {
    match requested {
        PublicationRoute::PullRequest => Ok(PublicationRoute::PullRequest),
        PublicationRoute::GithubStack if host_supports_stacks => Ok(PublicationRoute::GithubStack),
        PublicationRoute::GithubStack => Ok(PublicationRoute::PullRequest),
        PublicationRoute::OriginStack => Err(refusal(
            "implementation_stack_destination_unsupported",
            "stack.route",
            "Origin stacks need a separately admitted Origin collaboration adapter",
        )),
    }
}

/// Canonical CI for one exact lower layer of the attempt's verified candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerVerification {
    pub schema: String,
    pub handoff: ContentId,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub position: u32,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    /// The commit this layer builds on: the base, or the candidate below.
    pub parent: ExternalRef,
    pub verification_profile: ContentId,
    pub run: CiFact,
}

/// Admit one lower layer's verification under the same CI rules as #46.
pub fn admit_layer_verification(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    layer: &LayerVerification,
) -> CoreResult<ContentId> {
    let handoff_identity = super::admit_candidate_handoff(work, attempt, handoff)?;
    let below_top = usize::try_from(layer.position)
        .is_ok_and(|position| (1..MAX_STACK_LAYERS).contains(&position));
    if layer.schema != LAYER_VERIFICATION_SCHEMA
        || layer.handoff != handoff_identity
        || layer.work_identity != handoff.work_identity
        || layer.work_revision != handoff.work_revision
        || layer.attempt != handoff.attempt_identity
        || layer.fence != handoff.fence
        || !below_top
        || layer.verification_profile != work.verification_profile
        || layer.candidate == handoff.candidate
        || layer.candidate == layer.parent
        || !git_ref(&layer.candidate, "commit")
        || !git_ref(&layer.tree, "tree")
        || !git_ref(&layer.parent, "commit")
    {
        return Err(refusal(
            "implementation_stack_layer_verification",
            "stack.layer_verification",
            "layer verification does not bind an exact lower layer of the verified attempt",
        ));
    }
    let checks = admit_verification_run(work, layer.candidate.native_id.as_str(), &layer.run)?;
    let mut bytes = Vec::new();
    push_str(&mut bytes, LAYER_VERIFICATION_SCHEMA)?;
    for value in [
        layer.handoff.as_str(),
        layer.work_identity.as_str(),
        layer.attempt.as_str(),
        layer.verification_profile.as_str(),
    ] {
        push_str(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&layer.work_revision.to_be_bytes());
    bytes.extend_from_slice(&layer.fence.to_be_bytes());
    bytes.extend_from_slice(&layer.position.to_be_bytes());
    for reference in [&layer.candidate, &layer.tree, &layer.parent] {
        push_external(&mut bytes, reference)?;
    }
    push_run(&mut bytes, &layer.run, &checks)?;
    content_identity(&bytes)
}

/// One layer: an exact candidate on its parent, with its own branch and pull request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackLayerPlan {
    /// 1-based position; 1 is the bottom, next to the trunk.
    pub position: u32,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub parent: ExternalRef,
    /// The attempt's verification for the top layer, a layer verification otherwise.
    pub verification: ContentId,
    pub branch: String,
    /// The trunk for the bottom layer, the branch below otherwise.
    pub base_branch: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackPlan {
    pub schema: String,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub handoff: ContentId,
    pub verification: ContentId,
    pub repository: String,
    /// The exact trunk commit the verified attempt builds on.
    pub base: ExternalRef,
    pub trunk_branch: String,
    /// Bottom to top.
    pub layers: Vec<StackLayerPlan>,
}

/// Deterministic branch of one layer; the layers of one stack share a prefix.
#[must_use]
pub fn layer_branch(work: &ContentId, attempt: &ContentId, position: u32) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(BRANCH_DOMAIN);
    hash.update(work.as_str().as_bytes());
    hash.update(attempt.as_str().as_bytes());
    format!("kiln/stack-{}-{position}", hash.finalize().to_hex())
}

/// Admit a stack plan for an exactly verified attempt: every layer, its
/// verification, parent and branch chain, and the verified top.
// r[impl kiln.implementation.stack.scenario.plan]
pub fn admit_stack_plan(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    verification: &CandidateVerification,
    layer_verifications: &[LayerVerification],
    plan: &StackPlan,
) -> CoreResult<ContentId> {
    let handoff_identity = super::admit_candidate_handoff(work, attempt, handoff)?;
    let verification_identity =
        super::admit_candidate_verification(work, attempt, handoff, verification)?;
    let work_identity = work.identity()?;
    let count = plan.layers.len();
    if attempt.phase != AttemptPhase::Verified
        || attempt.handoff.as_ref() != Some(handoff)
        || attempt.candidate.as_ref() != Some(&handoff.candidate)
        || attempt.verification.as_ref() != Some(verification)
        || plan.schema != STACK_SCHEMA
        || plan.work_identity != work_identity
        || plan.work_revision != work.revision
        || plan.attempt != attempt.identity
        || plan.fence != attempt.fence
        || plan.handoff != handoff_identity
        || plan.verification != verification_identity
        || plan.repository != work.repository
        || !github_repository(&plan.repository)
        || Some(&plan.base) != handoff.base.as_ref()
        || !bounded_branch(&plan.trunk_branch)
        || !matches!(
            work.terminal_policy,
            TerminalPolicy::VerifiedPullRequest | TerminalPolicy::VerifiedIntegrated
        )
        || !(MIN_STACK_LAYERS..=MAX_STACK_LAYERS).contains(&count)
        || count.checked_sub(1) != Some(layer_verifications.len())
    {
        return Err(refusal(
            "implementation_stack_binding",
            "stack.plan",
            "stack plan does not bind the exact verified attempt",
        ));
    }
    for (index, layer) in plan.layers.iter().enumerate() {
        let position = u32::try_from(index + 1).map_err(|_| layer_refusal())?;
        let (parent, base_branch) = match index
            .checked_sub(1)
            .and_then(|below| plan.layers.get(below))
        {
            None => (&plan.base, &plan.trunk_branch),
            Some(below) => (&below.candidate, &below.branch),
        };
        let verified = if index + 1 == count {
            layer.candidate == handoff.candidate
                && layer.tree == handoff.tree
                && layer.verification == verification_identity
        } else {
            layer_verifications.get(index).is_some_and(|evidence| {
                evidence.position == position
                    && evidence.candidate == layer.candidate
                    && evidence.tree == layer.tree
                    && evidence.parent == layer.parent
                    && admit_layer_verification(work, attempt, handoff, evidence)
                        .ok()
                        .as_ref()
                        == Some(&layer.verification)
            })
        };
        if layer.position != position
            || &layer.parent != parent
            || &layer.base_branch != base_branch
            || layer.branch != layer_branch(&work_identity, &attempt.identity, position)
            || !git_ref(&layer.candidate, "commit")
            || !git_ref(&layer.tree, "tree")
            || plan
                .layers
                .iter()
                .take(index)
                .any(|other| other.candidate == layer.candidate)
            || !verified
            || !bounded(&layer.title, MAX_TITLE_BYTES)
            || !bounded(&layer.body, MAX_BODY_BYTES)
        {
            return Err(layer_refusal());
        }
    }
    stack_identity(plan)
}

fn layer_refusal() -> Diagnostic {
    refusal(
        "implementation_stack_layer",
        "stack.layers",
        "stack layer does not bind its exact verified candidate, parent and branch chain",
    )
}

/// Deterministic identity over every binding of a plan.
pub fn stack_identity(plan: &StackPlan) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_str(&mut bytes, STACK_SCHEMA)?;
    for value in [
        plan.work_identity.as_str(),
        plan.attempt.as_str(),
        plan.handoff.as_str(),
        plan.verification.as_str(),
        plan.repository.as_str(),
        plan.trunk_branch.as_str(),
    ] {
        push_str(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&plan.work_revision.to_be_bytes());
    bytes.extend_from_slice(&plan.fence.to_be_bytes());
    push_external(&mut bytes, &plan.base)?;
    let count = u32::try_from(plan.layers.len()).map_err(|_| layer_refusal())?;
    bytes.extend_from_slice(&count.to_be_bytes());
    for layer in &plan.layers {
        bytes.extend_from_slice(&layer.position.to_be_bytes());
        for reference in [&layer.candidate, &layer.tree, &layer.parent] {
            push_external(&mut bytes, reference)?;
        }
        for value in [
            layer.verification.as_str(),
            layer.branch.as_str(),
            layer.base_branch.as_str(),
            layer.title.as_str(),
            layer.body.as_str(),
        ] {
            push_str(&mut bytes, value)?;
        }
    }
    content_identity(&bytes)
}

/// Single-layer views for publishing each layer's branch and pull request with
/// the existing #47 transport. They are views of an admitted stack plan and
/// are never admissible as single pull-request plans.
#[must_use]
pub fn layer_publication_plans(plan: &StackPlan) -> Vec<PublicationPlan> {
    plan.layers
        .iter()
        .map(|layer| PublicationPlan {
            schema: PUBLICATION_SCHEMA.to_string(),
            work_identity: plan.work_identity.clone(),
            work_revision: plan.work_revision,
            attempt: plan.attempt.clone(),
            fence: plan.fence,
            handoff: plan.handoff.clone(),
            verification: layer.verification.clone(),
            repository: plan.repository.clone(),
            base: layer.parent.clone(),
            base_branch: layer.base_branch.clone(),
            candidate: layer.candidate.clone(),
            tree: layer.tree.clone(),
            branch: layer.branch.clone(),
            title: layer.title.clone(),
            body: layer.body.clone(),
        })
        .collect()
}

/// The provider identities a shell retained after publishing an admitted plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackPublication {
    pub schema: String,
    /// The admitted plan identity.
    pub stack: ContentId,
    pub provider_stack: ExternalRef,
    /// Bottom to top, one per layer.
    pub pull_requests: Vec<ExternalRef>,
}

/// Admit the retained stack and pull requests of an admitted plan.
pub fn admit_stack_publication(
    plan: &StackPlan,
    publication: &StackPublication,
) -> CoreResult<ContentId> {
    let in_repository = |reference: &ExternalRef, kind: &str| {
        reference.provider_namespace.as_str() == "github"
            && reference.native_kind.as_str() == kind
            && reference
                .native_id
                .as_str()
                .strip_prefix(plan.repository.as_str())
                .and_then(|rest| rest.strip_prefix('#'))
                .is_some_and(|number| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
    };
    if publication.schema != STACK_PUBLICATION_SCHEMA
        || publication.stack != stack_identity(plan)?
        || publication.pull_requests.len() != plan.layers.len()
        || !in_repository(&publication.provider_stack, "stack")
        || publication
            .pull_requests
            .iter()
            .enumerate()
            .any(|(index, pull)| {
                !in_repository(pull, "pull_request")
                    || publication
                        .pull_requests
                        .iter()
                        .take(index)
                        .any(|other| other == pull)
            })
    {
        return Err(refusal(
            "implementation_stack_publication",
            "stack.publication",
            "retained stack does not bind one pull request per layer of the admitted plan",
        ));
    }
    let mut bytes = Vec::new();
    push_str(&mut bytes, STACK_PUBLICATION_SCHEMA)?;
    push_str(&mut bytes, publication.stack.as_str())?;
    push_external(&mut bytes, &publication.provider_stack)?;
    for pull in &publication.pull_requests {
        push_external(&mut bytes, pull)?;
    }
    content_identity(&bytes)
}

/// One layer's observed landing state, derived by the shell from the exact
/// recovered merge or the #48 landing observation of that layer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerState {
    Ready,
    Integrated,
    Refused { code: String },
    Unknown { code: String },
}

/// Whether one layer may land, and if not, which layer decides.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "gate", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerGate {
    /// This layer and every layer below are ready or integrated.
    MayLand,
    Integrated,
    Blocked {
        layer: u32,
        code: String,
    },
    Unknown {
        layer: u32,
        code: String,
    },
}

/// Gate every layer from the bottom up. The lowest refused or unknown layer
/// decides for every layer above it; no layer's own gate is skipped.
// r[impl kiln.implementation.stack.gate]
// r[impl kiln.implementation.stack.gate.scenario.lower]
pub fn stack_gate(states: &[LayerState]) -> CoreResult<Vec<LayerGate>> {
    if !(MIN_STACK_LAYERS..=MAX_STACK_LAYERS).contains(&states.len())
        || states.iter().any(|state| match state {
            LayerState::Refused { code } | LayerState::Unknown { code } => {
                code.is_empty()
                    || code.len() > MAX_STATE_CODE_BYTES
                    || code.chars().any(char::is_control)
            }
            LayerState::Ready | LayerState::Integrated => false,
        })
    {
        return Err(refusal(
            "implementation_stack_gate",
            "stack.states",
            "stack gate needs one bounded state per layer",
        ));
    }
    let mut lowest: Option<LayerGate> = None;
    let mut landed_below = true;
    let mut gates = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let layer = u32::try_from(index + 1).map_err(|_| layer_refusal())?;
        let own = match state {
            LayerState::Integrated if landed_below => LayerGate::Integrated,
            LayerState::Integrated => LayerGate::Unknown {
                layer,
                code: OUT_OF_ORDER.to_string(),
            },
            LayerState::Ready => LayerGate::MayLand,
            LayerState::Refused { code } => LayerGate::Blocked {
                layer,
                code: code.clone(),
            },
            LayerState::Unknown { code } => LayerGate::Unknown {
                layer,
                code: code.clone(),
            },
        };
        landed_below &= *state == LayerState::Integrated;
        let gate = lowest.clone().unwrap_or(own);
        if lowest.is_none() && matches!(gate, LayerGate::Blocked { .. } | LayerGate::Unknown { .. })
        {
            lowest = Some(gate.clone());
        }
        gates.push(gate);
    }
    Ok(gates)
}

/// One layer's recovered merge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackLayerIntegration {
    pub position: u32,
    pub pull_request: ExternalRef,
    pub candidate: ExternalRef,
    pub merge_commit: ExternalRef,
    /// `expected_base` is the merge's first parent.
    pub mapping: IntegrationMapping,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackIntegration {
    pub schema: String,
    pub stack: ContentId,
    /// A contiguous prefix from the bottom.
    pub layers: Vec<StackLayerIntegration>,
}

/// An admitted integration prefix; `complete` only when every layer landed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedStackIntegration {
    pub identity: ContentId,
    pub integrated_layers: usize,
    pub complete: bool,
}

/// Admit the recovered integration of an exact bottom prefix. Each layer's
/// merge has the planned candidate as second parent and its planned tree,
/// with the base, or the previous layer's merge, as first parent.
// r[impl kiln.implementation.stack.gate.scenario.integration]
pub fn admit_stack_integration(
    plan: &StackPlan,
    publication: &StackPublication,
    integration: &StackIntegration,
) -> CoreResult<AdmittedStackIntegration> {
    admit_stack_publication(plan, publication)?;
    let integrated_layers = integration.layers.len();
    if integration.schema != STACK_INTEGRATION_SCHEMA
        || integration.stack != publication.stack
        || integrated_layers == 0
        || integrated_layers > plan.layers.len()
    {
        return Err(integration_refusal());
    }
    let mut first_parent = &plan.base;
    for (index, ((landed, layer), pull)) in integration
        .layers
        .iter()
        .zip(&plan.layers)
        .zip(&publication.pull_requests)
        .enumerate()
    {
        let position = u32::try_from(index + 1).map_err(|_| integration_refusal())?;
        if landed.position != position
            || &landed.pull_request != pull
            || landed.candidate != layer.candidate
            || landed.mapping.expected_base != *first_parent
            || landed.mapping.candidate_tree != layer.tree
            || landed.mapping.landed_tree != layer.tree
            || landed.merge_commit == layer.candidate
            || !commit_identity(&landed.merge_commit)
            || !git_ref(&landed.mapping.expected_base, "commit")
        {
            return Err(integration_refusal());
        }
        first_parent = &landed.merge_commit;
    }
    let mut bytes = Vec::new();
    push_str(&mut bytes, STACK_INTEGRATION_SCHEMA)?;
    push_str(&mut bytes, integration.stack.as_str())?;
    for landed in &integration.layers {
        bytes.extend_from_slice(&landed.position.to_be_bytes());
        for reference in [
            &landed.pull_request,
            &landed.candidate,
            &landed.merge_commit,
            &landed.mapping.expected_base,
            &landed.mapping.candidate_tree,
            &landed.mapping.landed_tree,
        ] {
            push_external(&mut bytes, reference)?;
        }
    }
    Ok(AdmittedStackIntegration {
        identity: content_identity(&bytes)?,
        integrated_layers,
        complete: integrated_layers == plan.layers.len(),
    })
}

fn integration_refusal() -> Diagnostic {
    refusal(
        "implementation_stack_integration",
        "stack.integration",
        "recovered merges are not the exact planned layers landed from the bottom",
    )
}

fn git_ref(reference: &ExternalRef, kind: &str) -> bool {
    reference.provider_namespace.as_str() == "git" && reference.native_kind.as_str() == kind
}

fn commit_identity(reference: &ExternalRef) -> bool {
    let id = reference.native_id.as_str();
    git_ref(reference, "commit")
        && matches!(id.len(), 40 | 64)
        && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
