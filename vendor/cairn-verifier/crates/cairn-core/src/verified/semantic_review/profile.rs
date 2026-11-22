pub fn semantic_review_policy_profile_json() -> serde_json::Value {
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles.profiles.kinds]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.profiles.profiles.criteria]
    // r[impl cairn.semantic_review.review_gate_policy_profiles.sources]
    serde_json::json!({
        "schema": "cairn.semantic-review.policy-profile.v1",
        "review_kinds": super::REVIEW_KINDS,
        "decisions": super::REVIEW_DECISIONS,
        "categories": super::REVIEW_CATEGORIES,
        "severities": super::REVIEW_SEVERITIES,
        "reviewer_sources": super::REVIEW_SOURCES,
        "revision_dispositions": super::REVISION_DISPOSITIONS,
        "profiles": [
            {"id": "plan", "required_criteria": ["scope", "requirements", "risks", "omissions"]},
            {"id": "design", "required_criteria": ["architecture", "acceptance", "interfaces", "risks"]},
            {"id": "code", "required_criteria": ["requirements", "tests", "consistency", "regressions"]},
            {"id": "diff", "required_criteria": ["intent", "scope", "side_effects", "verification"]}
        ]
    })
}
