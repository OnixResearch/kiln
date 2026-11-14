use serde_json::{json, Value};

use super::{
    REVIEW_CATEGORIES, REVIEW_DECISIONS, REVIEW_KINDS, REVIEW_SEVERITIES, REVIEW_SOURCES,
    REVISION_DISPOSITIONS,
};

pub fn semantic_review_policy_profile_json() -> Value {
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles.profiles.kinds]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles.profiles.criteria]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.sources]
    json!({
        "schema": "cairn.semantic-review.policy-profile.v1",
        "review_kinds": REVIEW_KINDS,
        "decisions": REVIEW_DECISIONS,
        "categories": REVIEW_CATEGORIES,
        "severities": REVIEW_SEVERITIES,
        "reviewer_sources": REVIEW_SOURCES,
        "revision_dispositions": REVISION_DISPOSITIONS,
        "profiles": [
            {"id": "plan", "required_criteria": ["scope", "requirements", "risks", "omissions"]},
            {"id": "design", "required_criteria": ["architecture", "acceptance", "interfaces", "risks"]},
            {"id": "code", "required_criteria": ["requirements", "tests", "consistency", "regressions"]},
            {"id": "diff", "required_criteria": ["intent", "scope", "side_effects", "verification"]}
        ]
    })
}
