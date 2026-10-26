//! Provider-neutral isolated review evidence, never merge or lifecycle-gate authority.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::gating::{CandidateSet, validate_candidate_set};
use crate::types::{
    CONTRACT_VERSION_V1, CandidateId, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef,
    PipelineRevisionId, ProviderProfileId,
};

pub const REVIEW_OBSERVATION_CONTRACT_VERSION: u16 = CONTRACT_VERSION_V1;

// r[impl kiln.patchgate.request]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewEffectPlan {
    pub effect_id: EffectId,
    pub idempotency_key: ContentId,
    pub candidate: CandidateSet,
    pub candidate_root: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub policy_identity: ContentId,
    pub provider_profile: ProviderProfileId,
    pub native_binding_identity: ContentId,
}

// r[impl kiln.patchgate.nonclaims]
/// A provider's review decision, not a domain outcome or permission to land a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Accepted,
    Rejected,
    Escalated,
    Blocked,
    Waiting,
    Pending,
    Removed,
    Reverted,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChangeReview {
    pub change: ExternalRef,
    pub decision: ReviewDecision,
}

// r[impl kiln.patchgate.result]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewEvaluationObservation {
    pub contract_version: u16,
    pub effect_id: EffectId,
    pub idempotency_key: ContentId,
    pub candidate_id: CandidateId,
    pub candidate_root: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub policy_identity: ContentId,
    pub provider_profile: ProviderProfileId,
    pub native_binding_identity: ContentId,
    pub report_identity: ContentId,
    pub isolated: bool,
    pub authorizes_acceptance: bool,
    pub reviews: Vec<ChangeReview>,
}

// r[impl kiln.patchgate.request]
/// Validates structural request invariants. Canonical content and native binding
/// identities must also be verified at the provider boundary that interprets them.
pub fn validate_review_effect_plan(
    plan: &ReviewEffectPlan,
    max_changes: usize,
    max_edges: usize,
) -> CoreResult<()> {
    validate_candidate_set(&plan.candidate, max_changes, max_edges)?;
    let mut changes = BTreeSet::new();
    for change in &plan.candidate.ordered_changes {
        change.change.validate()?;
        change.target_revision.validate()?;
        if !changes.insert(&change.change) {
            return Err(Diagnostic::new(
                "review_duplicate_change",
                "review.candidate.ordered_changes",
                "review candidate repeats a change identity",
            ));
        }
        if plan.candidate.frozen_sources.get(&change.change) != Some(&change.source_snapshot) {
            return Err(Diagnostic::new(
                "review_unfrozen_source",
                "review.candidate.frozen_sources",
                "review change does not match its frozen source snapshot",
            ));
        }
        if change.pipeline_revision != plan.pipeline_revision {
            return Err(Diagnostic::new(
                "review_pipeline_revision",
                "review.pipeline_revision",
                "review pipeline revision differs from a candidate change",
            ));
        }
        if change.collaboration_provider != plan.provider_profile {
            return Err(Diagnostic::new(
                "review_provider_profile",
                "review.provider_profile",
                "review provider profile differs from a candidate change",
            ));
        }
    }
    Ok(())
}

// r[impl kiln.patchgate.result]
// r[impl kiln.patchgate.nonclaims]
/// Admits complete, exactly bound isolated evidence without granting acceptance authority.
pub fn admit_review_observation(
    plan: &ReviewEffectPlan,
    observation: &ReviewEvaluationObservation,
    max_changes: usize,
    max_edges: usize,
) -> CoreResult<()> {
    validate_review_effect_plan(plan, max_changes, max_edges)?;
    if observation.contract_version != REVIEW_OBSERVATION_CONTRACT_VERSION {
        return Err(Diagnostic::new(
            "unsupported_version",
            "review.observation.contract_version",
            "review observation version is unsupported",
        ));
    }
    if observation.effect_id != plan.effect_id
        || observation.idempotency_key != plan.idempotency_key
        || observation.candidate_id != plan.candidate.id
        || observation.candidate_root != plan.candidate_root
        || observation.pipeline_revision != plan.pipeline_revision
        || observation.policy_identity != plan.policy_identity
        || observation.provider_profile != plan.provider_profile
        || observation.native_binding_identity != plan.native_binding_identity
    {
        return Err(Diagnostic::new(
            "review_observation_substitution",
            "review.observation",
            "review observation does not apply to the exact effect plan",
        ));
    }
    if !observation.isolated || observation.authorizes_acceptance {
        return Err(Diagnostic::new(
            "review_observation_authority",
            "review.observation",
            "review evidence must be isolated and cannot authorize acceptance",
        ));
    }
    if observation.reviews.len() != plan.candidate.ordered_changes.len()
        || observation
            .reviews
            .iter()
            .zip(&plan.candidate.ordered_changes)
            .any(|(review, change)| review.change != change.change)
    {
        return Err(Diagnostic::new(
            "review_observation_completeness",
            "review.observation.reviews",
            "review decisions must cover each candidate exactly once in request order",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;

    use crate::gating::{CandidateChange, DependencyEdge};
    use crate::types::PolicyId;

    use super::*;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> ReviewEffectPlan {
        let pipeline_revision =
            PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!());
        let provider_profile =
            ProviderProfileId::new("review-a").unwrap_or_else(|_| unreachable!());
        let ordered_changes: Vec<_> = ["change-a", "change-b"]
            .into_iter()
            .map(|name| CandidateChange {
                change: ExternalRef::new("source", "change", name)
                    .unwrap_or_else(|_| unreachable!()),
                target_revision: ExternalRef::new("source", "state", "state-a")
                    .unwrap_or_else(|_| unreachable!()),
                source_snapshot: content('a'),
                pipeline_revision: pipeline_revision.clone(),
                collaboration_provider: provider_profile.clone(),
            })
            .collect();
        let frozen_sources = BTreeMap::from_iter(
            ordered_changes
                .iter()
                .map(|change| (change.change.clone(), change.source_snapshot.clone())),
        );
        ReviewEffectPlan {
            effect_id: EffectId::new("review-effect-a").unwrap_or_else(|_| unreachable!()),
            idempotency_key: content('b'),
            candidate: CandidateSet {
                id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
                ordered_changes,
                dependencies: alloc::vec![DependencyEdge {
                    before_index: 0,
                    after_index: 1,
                }],
                frozen_sources,
                policy_id: PolicyId::new("policy-a").unwrap_or_else(|_| unreachable!()),
            },
            candidate_root: content('c'),
            pipeline_revision,
            policy_identity: content('d'),
            provider_profile,
            native_binding_identity: content('e'),
        }
    }

    fn observation(plan: &ReviewEffectPlan) -> ReviewEvaluationObservation {
        ReviewEvaluationObservation {
            contract_version: REVIEW_OBSERVATION_CONTRACT_VERSION,
            effect_id: plan.effect_id.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            candidate_id: plan.candidate.id.clone(),
            candidate_root: plan.candidate_root.clone(),
            pipeline_revision: plan.pipeline_revision.clone(),
            policy_identity: plan.policy_identity.clone(),
            provider_profile: plan.provider_profile.clone(),
            native_binding_identity: plan.native_binding_identity.clone(),
            report_identity: content('f'),
            isolated: true,
            authorizes_acceptance: false,
            reviews: plan
                .candidate
                .ordered_changes
                .iter()
                .map(|change| ChangeReview {
                    change: change.change.clone(),
                    decision: ReviewDecision::Accepted,
                })
                .collect(),
        }
    }

    fn assert_error(result: CoreResult<()>, expected: &str) {
        assert_eq!(
            result.map_err(|diagnostic| diagnostic.code),
            Err(expected.to_string())
        );
    }

    // r[verify kiln.patchgate.request]
    #[test]
    fn request_obeys_change_and_dependency_bounds() {
        let mut plan = plan();
        assert_eq!(validate_review_effect_plan(&plan, 2, 1), Ok(()));
        assert_error(validate_review_effect_plan(&plan, 0, 1), "candidate_bound");
        assert_error(validate_review_effect_plan(&plan, 1, 1), "candidate_bound");
        assert_error(validate_review_effect_plan(&plan, 2, 0), "dependency_bound");
        plan.candidate.dependencies.clear();
        assert_eq!(validate_review_effect_plan(&plan, 2, 0), Ok(()));
        plan.candidate.ordered_changes.clear();
        assert_error(validate_review_effect_plan(&plan, 2, 0), "candidate_bound");
    }

    // r[verify kiln.patchgate.request]
    #[test]
    fn request_reuses_dependency_order_and_node_validation() {
        let mut plan = plan();
        plan.candidate
            .dependencies
            .get_mut(0)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .after_index = 2;
        assert_error(
            validate_review_effect_plan(&plan, 2, 1),
            "missing_dependency_node",
        );
        plan.candidate
            .dependencies
            .get_mut(0)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .after_index = 0;
        assert_error(
            validate_review_effect_plan(&plan, 2, 1),
            "invalid_dependency_order",
        );
    }

    // r[verify kiln.patchgate.request]
    #[test]
    fn duplicate_and_unfrozen_changes_are_refused() {
        let mut duplicate = plan();
        *duplicate
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element")) = duplicate
            .candidate
            .ordered_changes
            .first()
            .unwrap_or_else(|| unreachable!("fixture element"))
            .clone();
        assert_error(
            validate_review_effect_plan(&duplicate, 2, 1),
            "review_duplicate_change",
        );
        let mut missing = plan();
        missing.candidate.frozen_sources.clear();
        assert_error(
            validate_review_effect_plan(&missing, 2, 1),
            "review_unfrozen_source",
        );
        let mut substituted = plan();
        substituted
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .source_snapshot = content('b');
        assert_error(
            validate_review_effect_plan(&substituted, 2, 1),
            "review_unfrozen_source",
        );
    }

    // r[verify kiln.patchgate.request]
    #[test]
    fn every_change_must_bind_the_plan_pipeline_and_provider() {
        let mut pipeline = plan();
        pipeline
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .pipeline_revision =
            PipelineRevisionId::new("pipeline-b").unwrap_or_else(|_| unreachable!());
        assert_error(
            validate_review_effect_plan(&pipeline, 2, 1),
            "review_pipeline_revision",
        );
        let mut provider = plan();
        provider
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .collaboration_provider =
            ProviderProfileId::new("review-b").unwrap_or_else(|_| unreachable!());
        assert_error(
            validate_review_effect_plan(&provider, 2, 1),
            "review_provider_profile",
        );
    }

    // r[verify kiln.patchgate.request]
    #[test]
    fn change_and_target_reference_versions_are_validated() {
        let mut change = plan();
        change
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .change
            .contract_version = 2;
        assert_error(
            validate_review_effect_plan(&change, 2, 1),
            "unsupported_version",
        );
        let mut target = plan();
        target
            .candidate
            .ordered_changes
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .target_revision
            .contract_version = 2;
        assert_error(
            validate_review_effect_plan(&target, 2, 1),
            "unsupported_version",
        );
    }

    // r[verify kiln.patchgate.result]
    // r[verify kiln.patchgate.nonclaims]
    #[test]
    fn all_review_decisions_remain_admissible_isolated_evidence() {
        let plan = plan();
        for decision in [
            ReviewDecision::Accepted,
            ReviewDecision::Rejected,
            ReviewDecision::Escalated,
            ReviewDecision::Blocked,
            ReviewDecision::Waiting,
            ReviewDecision::Pending,
            ReviewDecision::Removed,
            ReviewDecision::Reverted,
            ReviewDecision::Unknown,
        ] {
            let mut observation = observation(&plan);
            observation
                .reviews
                .get_mut(0)
                .unwrap_or_else(|| unreachable!("fixture element"))
                .decision = decision;
            assert_eq!(admit_review_observation(&plan, &observation, 2, 1), Ok(()));
        }
    }

    // r[verify kiln.patchgate.conformance]
    #[test]
    fn every_request_binding_is_checked_for_substitution() {
        let plan = plan();
        let substitutions: [fn(&mut ReviewEvaluationObservation); 8] = [
            |result| result.effect_id = EffectId::new("other").unwrap_or_else(|_| unreachable!()),
            |result| result.idempotency_key = content('0'),
            |result| {
                result.candidate_id = CandidateId::new("other").unwrap_or_else(|_| unreachable!())
            },
            |result| result.candidate_root = content('0'),
            |result| {
                result.pipeline_revision =
                    PipelineRevisionId::new("other").unwrap_or_else(|_| unreachable!())
            },
            |result| result.policy_identity = content('0'),
            |result| {
                result.provider_profile =
                    ProviderProfileId::new("other").unwrap_or_else(|_| unreachable!())
            },
            |result| result.native_binding_identity = content('0'),
        ];
        for substitute in substitutions {
            let mut observation = observation(&plan);
            substitute(&mut observation);
            assert_error(
                admit_review_observation(&plan, &observation, 2, 1),
                "review_observation_substitution",
            );
        }
    }

    // r[verify kiln.patchgate.result]
    #[test]
    fn missing_extra_duplicate_foreign_and_reordered_reviews_are_refused() {
        let plan = plan();
        let complete = observation(&plan);
        let mut missing = complete.clone();
        missing.reviews.pop();
        let mut extra = complete.clone();
        extra.reviews.push(
            extra
                .reviews
                .first()
                .unwrap_or_else(|| unreachable!("fixture element"))
                .clone(),
        );
        let mut duplicate = complete.clone();
        *duplicate
            .reviews
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element")) = duplicate
            .reviews
            .first()
            .unwrap_or_else(|| unreachable!("fixture element"))
            .clone();
        let mut foreign = complete.clone();
        foreign
            .reviews
            .get_mut(1)
            .unwrap_or_else(|| unreachable!("fixture element"))
            .change =
            ExternalRef::new("source", "change", "foreign").unwrap_or_else(|_| unreachable!());
        let mut reordered = complete;
        reordered.reviews.swap(0, 1);
        for observation in [missing, extra, duplicate, foreign, reordered] {
            assert_error(
                admit_review_observation(&plan, &observation, 2, 1),
                "review_observation_completeness",
            );
        }
    }

    // r[verify kiln.patchgate.conformance]
    #[test]
    fn observation_versions_and_invalid_requests_fail_admission() {
        let mut plan = plan();
        let mut observation = observation(&plan);
        for version in [0, 2, u16::MAX] {
            observation.contract_version = version;
            assert_error(
                admit_review_observation(&plan, &observation, 2, 1),
                "unsupported_version",
            );
        }
        observation.contract_version = REVIEW_OBSERVATION_CONTRACT_VERSION;
        assert_error(
            admit_review_observation(&plan, &observation, 1, 1),
            "candidate_bound",
        );
        plan.candidate.frozen_sources.clear();
        assert_error(
            admit_review_observation(&plan, &observation, 2, 1),
            "review_unfrozen_source",
        );
    }

    // r[verify kiln.patchgate.nonclaims]
    #[test]
    fn isolation_and_non_authorization_are_both_required() {
        let plan = plan();
        for (isolated, authorizes_acceptance) in [(false, false), (true, true), (false, true)] {
            let mut observation = observation(&plan);
            observation.isolated = isolated;
            observation.authorizes_acceptance = authorizes_acceptance;
            assert_error(
                admit_review_observation(&plan, &observation, 2, 1),
                "review_observation_authority",
            );
        }
    }
}
