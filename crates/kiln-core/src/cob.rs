use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    CobObjectId, CobProfileId, CobSemanticStateId, ContentId, CoreResult, Diagnostic, EffectId,
    ExternalRef, PolicyId, ProviderProfileId,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CobCapability {
    Discover,
    TypedRead,
    EventIntake,
    StatusProjection,
    Create,
    Update,
    Review,
    Resolve,
    Export,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CobProfile {
    pub id: CobProfileId,
    pub provider: ProviderProfileId,
    pub type_name: ExternalRef,
    pub schema_line: ExternalRef,
    pub mapper_version: u16,
    pub capabilities: BTreeSet<CobCapability>,
    pub max_heads: usize,
    pub max_payload_bytes: usize,
    pub complete_head_resolution: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CobObservation {
    pub profile_id: CobProfileId,
    pub repository: ExternalRef,
    pub type_name: ExternalRef,
    pub schema_line: ExternalRef,
    pub object_id: CobObjectId,
    pub heads: BTreeSet<ExternalRef>,
    pub semantic_state: CobSemanticStateId,
    pub projection_identity: ContentId,
}

pub fn select_profile<'a>(
    profiles: &'a [CobProfile],
    observation: &CobObservation,
    capability: CobCapability,
) -> CoreResult<&'a CobProfile> {
    let profile = profiles
        .iter()
        .find(|profile| {
            profile.id == observation.profile_id
                && profile.type_name == observation.type_name
                && profile.schema_line == observation.schema_line
        })
        .ok_or(Diagnostic::new(
            "unknown_cob_profile",
            "cob.profile",
            "COB type or schema is not registered",
        ))?;
    if !profile.capabilities.contains(&capability) {
        return Err(Diagnostic::new(
            "unsupported_cob_action",
            "cob.capability",
            "COB profile does not declare the requested capability",
        ));
    }
    if observation.heads.is_empty() || observation.heads.len() > profile.max_heads {
        return Err(Diagnostic::new(
            "cob_head_bound",
            "cob.heads",
            "COB head count is invalid",
        ));
    }
    Ok(profile)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CobConflict {
    None,
    ConcurrentHeads(BTreeSet<ExternalRef>),
    ReducerConflict,
    MalformedHistory,
}

#[must_use]
pub fn classify_conflict(
    observation: &CobObservation,
    reducer_conflict: bool,
    malformed: bool,
) -> CobConflict {
    if malformed {
        return CobConflict::MalformedHistory;
    }
    if reducer_conflict {
        return CobConflict::ReducerConflict;
    }
    if observation.heads.len() > 1 {
        return CobConflict::ConcurrentHeads(observation.heads.clone());
    }
    CobConflict::None
}

// r[impl kiln.providers.policy_effect_split]
pub fn admit_cob_freshness(expected: &CobObservation, observed: &CobObservation) -> CoreResult<()> {
    if observed != expected {
        return Err(Diagnostic::new(
            "stale_cob_preflight",
            "cob_effect.expected_heads",
            "fresh COB observation differs from the planned observation",
        ));
    }
    Ok(())
}

pub fn admit_resolution(
    profile: &CobProfile,
    observation: &CobObservation,
    selected_heads: &BTreeSet<ExternalRef>,
) -> CoreResult<()> {
    if profile.complete_head_resolution && selected_heads != &observation.heads {
        return Err(Diagnostic::new(
            "partial_resolution",
            "cob.selected_heads",
            "COB resolution must include every current head",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CobProviderEvent {
    pub object_id: CobObjectId,
    pub before_heads: BTreeSet<ExternalRef>,
    pub after_heads: BTreeSet<ExternalRef>,
    pub semantic_state: CobSemanticStateId,
    pub actor: ExternalRef,
    pub signature_observed: bool,
    pub signature_valid: bool,
}

pub fn map_event(
    profile: &CobProfile,
    observation: &CobObservation,
    event: &CobProviderEvent,
) -> CoreResult<CobObservation> {
    if event.object_id != observation.object_id || event.before_heads != observation.heads {
        return Err(Diagnostic::new(
            "stale_cob_event",
            "cob_event.before_heads",
            "COB event does not extend the exact observed object heads",
        ));
    }
    if event.after_heads.is_empty() || event.after_heads.len() > profile.max_heads {
        return Err(Diagnostic::new(
            "cob_event_head_bound",
            "cob_event.after_heads",
            "COB event result has an invalid head count",
        ));
    }
    if event.signature_observed && !event.signature_valid {
        return Err(Diagnostic::new(
            "cob_event_signature",
            "cob_event.signature",
            "observed COB event signature is invalid",
        ));
    }
    event.actor.validate()?;
    let mut mapped = observation.clone();
    mapped.heads = event.after_heads.clone();
    mapped.semantic_state = event.semantic_state.clone();
    Ok(mapped)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Applicability {
    pub semantic_state: CobSemanticStateId,
    pub candidate: Option<ContentId>,
    pub claim: Option<ContentId>,
    pub policy: PolicyId,
    pub provider_observation: ContentId,
}

#[must_use]
pub fn applicability_matches(recorded: &Applicability, current: &Applicability) -> bool {
    recorded == current
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CobEffectPlan {
    pub effect_id: EffectId,
    pub profile_id: CobProfileId,
    pub repository: ExternalRef,
    pub object_id: CobObjectId,
    pub expected_heads: BTreeSet<ExternalRef>,
    pub action: CobCapability,
    pub payload_identity: ContentId,
    pub actor: ExternalRef,
    pub policy_id: PolicyId,
    pub idempotency_key: ExternalRef,
}

pub fn plan_effect(
    profile: &CobProfile,
    observation: &CobObservation,
    plan: CobEffectPlan,
) -> CoreResult<CobEffectPlan> {
    if !profile.capabilities.contains(&plan.action) {
        return Err(Diagnostic::new(
            "unsupported_cob_action",
            "cob_effect.action",
            "COB mutation action is not declared",
        ));
    }
    if profile.id != plan.profile_id
        || observation.object_id != plan.object_id
        || observation.heads != plan.expected_heads
    {
        return Err(Diagnostic::new(
            "stale_cob_plan",
            "cob_effect",
            "COB effect plan does not match current profile, object, or heads",
        ));
    }
    Ok(plan)
}

pub const INITIAL_CAIRN_TYPES: [&str; 6] = [
    "xyz.onixresearch.cairn-change",
    "xyz.onixresearch.rfd",
    "xyz.onixresearch.adr",
    "xyz.onixresearch.verification-obligation",
    "xyz.onixresearch.release-decision",
    "xyz.onixresearch.policy-exception",
];

pub fn missing_initial_profiles(profile_type_names: &[&str]) -> Vec<&'static str> {
    INITIAL_CAIRN_TYPES
        .iter()
        .copied()
        .filter(|required| !profile_type_names.contains(required))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_initial_profile() {
        let missing = missing_initial_profiles(&["xyz.onixresearch.adr"]);
        assert!(!missing.is_empty());
    }

    #[test]
    fn complete_resolution_rejects_partial_heads() {
        let mut current = BTreeSet::new();
        current.insert(
            ExternalRef::new("radicle", "operation", "a").unwrap_or_else(|_| unreachable!()),
        );
        current.insert(
            ExternalRef::new("radicle", "operation", "b").unwrap_or_else(|_| unreachable!()),
        );
        let selected = BTreeSet::from([
            ExternalRef::new("radicle", "operation", "a").unwrap_or_else(|_| unreachable!())
        ]);
        let profile = CobProfile {
            id: CobProfileId::new("profile").unwrap_or_else(|_| unreachable!()),
            provider: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
            type_name: ExternalRef::new("radicle", "type", "issue")
                .unwrap_or_else(|_| unreachable!()),
            schema_line: ExternalRef::new("radicle", "schema", "v1")
                .unwrap_or_else(|_| unreachable!()),
            mapper_version: 1,
            capabilities: BTreeSet::from([CobCapability::Resolve]),
            max_heads: 4,
            max_payload_bytes: 4096,
            complete_head_resolution: true,
        };
        let observation = CobObservation {
            profile_id: profile.id.clone(),
            repository: ExternalRef::new("radicle", "repository", "rid")
                .unwrap_or_else(|_| unreachable!()),
            type_name: profile.type_name.clone(),
            schema_line: profile.schema_line.clone(),
            object_id: CobObjectId::new("object").unwrap_or_else(|_| unreachable!()),
            heads: current,
            semantic_state: CobSemanticStateId::new("state").unwrap_or_else(|_| unreachable!()),
            projection_identity: ContentId::from_blake3_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_or_else(|_| unreachable!()),
        };
        assert!(admit_resolution(&profile, &observation, &selected).is_err());
        assert!(admit_cob_freshness(&observation, &observation).is_ok());
        let mut stale = observation.clone();
        stale.heads = selected;
        assert!(admit_cob_freshness(&observation, &stale).is_err());
    }
}
