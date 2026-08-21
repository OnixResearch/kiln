use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kiln_core::cob::{CobCapability, CobEffectPlan};
use kiln_core::types::{CobObjectId, ContentId, EffectId, ExternalRef, PolicyId};

use crate::status::{
    PINNED_SECRETSPEC_EXECUTABLE_BLAKE3, PINNED_SECRETSPEC_EXECUTABLE_PATH,
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, SecretSpecDeclaration,
};

use super::*;

const REPOSITORY_RID: &str = "rad:zFixtureRepository";
const OBJECT_ID: &str = "2222222222222222222222222222222222222222";
const REVISION_ID: &str = "1111111111111111111111111111111111111111";
const HEAD_ID: &str = "3333333333333333333333333333333333333333";

fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, id).unwrap_or_else(|_| unreachable!())
}

fn action() -> PatchReviewAction {
    PatchReviewAction {
        revision: REVISION_ID.to_string(),
        verdict: PatchVerdict::Accept,
        summary: "Kiln checks passed".to_string(),
    }
}

fn profile() -> RadicleMutationProfile {
    let action = action();
    let identity = operation_identity(&action).unwrap_or_else(|_| unreachable!());
    RadicleMutationProfile::pinned(
        BTreeSet::from([DeclaredMutationTarget {
            repository_rid: REPOSITORY_RID.to_string(),
            object_type: PATCH_TYPE_NAME.to_string(),
            object_id: OBJECT_ID.to_string(),
        }]),
        BTreeMap::from([(identity, action)]),
        SecretSpecDeclaration {
            project_root: PathBuf::from("/var/lib/kiln/secrets"),
            project: "kiln".to_string(),
            profile: "production".to_string(),
            secret_key: "RADICLE_SIGNER_SEED".to_string(),
            provider: Some("keyring".to_string()),
        },
        PinnedSecretSpecExecutable {
            path: PathBuf::from(PINNED_SECRETSPEC_EXECUTABLE_PATH),
            blake3: PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.to_string(),
            version: PINNED_SECRETSPEC_VERSION.to_string(),
        },
    )
}

fn plan(profile: &RadicleMutationProfile) -> CobEffectPlan {
    let payload_identity = profile
        .operations
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    CobEffectPlan {
        effect_id: EffectId::new("mutation-effect").unwrap_or_else(|_| unreachable!()),
        profile_id: profile.id.clone(),
        repository: external("radicle", "repository", REPOSITORY_RID),
        object_id: CobObjectId::new(OBJECT_ID).unwrap_or_else(|_| unreachable!()),
        expected_heads: BTreeSet::from([external("radicle", "operation", HEAD_ID)]),
        action: CobCapability::Review,
        payload_identity: ContentId::from_blake3_hex(payload_identity)
            .unwrap_or_else(|_| unreachable!()),
        actor: external("radicle", "node", "zSigner"),
        policy_id: PolicyId::new("single-writer").unwrap_or_else(|_| unreachable!()),
        idempotency_key: external("kiln", "operation", "patch-review-one"),
    }
}

// r[verify kiln.radicle.mutation_scope]
#[test]
fn pinned_profile_and_declared_plan_are_admitted() {
    let profile = profile();
    admit_mutation_profile(&profile).unwrap_or_else(|_| unreachable!());
    let selected =
        admit_mutation_plan(&profile, &plan(&profile)).unwrap_or_else(|_| unreachable!());
    assert_eq!(selected, &action());
}

#[test]
fn rendered_review_matches_the_pinned_radicle_shape() {
    let bytes = render_patch_review(&action()).unwrap_or_else(|_| unreachable!());
    let expected = format!(
        "{{\"type\":\"review\",\"revision\":\"{REVISION_ID}\",\"summary\":\"Kiln checks passed\",\"verdict\":\"accept\"}}"
    );
    assert_eq!(bytes, expected.as_bytes());
    assert_eq!(
        blake3::hash(&bytes).to_string(),
        operation_identity(&action()).unwrap_or_default()
    );
}

// r[verify kiln.radicle.mutation_scope.scenario.foreign]
#[test]
fn undeclared_target_and_action_are_rejected() {
    let profile = profile();
    let mut foreign = plan(&profile);
    foreign.object_id = CobObjectId::new("4444444444444444444444444444444444444444")
        .unwrap_or_else(|_| unreachable!());
    assert!(admit_mutation_plan(&profile, &foreign).is_err());

    let mut forbidden = plan(&profile);
    forbidden.action = CobCapability::Resolve;
    assert!(admit_mutation_plan(&profile, &forbidden).is_err());
}

#[test]
fn zero_deadline_executable_substitution_and_merge_authority_are_rejected() {
    let mut zero = profile();
    zero.deadlines.write_ms = 0;
    assert!(admit_mutation_profile(&zero).is_err());

    let mut substituted = profile();
    substituted.executable.path = PathBuf::from("/tmp/substituted-secretspec");
    assert!(admit_mutation_profile(&substituted).is_err());

    let mut merge = profile();
    merge.allow_patch_merge = true;
    assert!(admit_mutation_profile(&merge).is_err());

    let mut oversized = profile();
    oversized.max_payload_bytes = 1;
    assert!(admit_mutation_profile(&oversized).is_err());
}

// r[verify kiln.radicle.mutation_bounds.scenario.timeout]
#[test]
fn write_outcomes_keep_timeout_and_unknown_distinct() {
    assert_eq!(
        classify_write(true, WriteFact::RejectedBeforeAcceptance),
        WriteClassification::TimedOut
    );
    assert_eq!(
        classify_write(false, WriteFact::AmbiguousAfterAcceptance),
        WriteClassification::Unknown
    );
    assert_eq!(
        classify_write(true, WriteFact::Applied),
        WriteClassification::Unknown
    );
    assert_eq!(
        classify_write(false, WriteFact::Applied),
        WriteClassification::Applied
    );
}
