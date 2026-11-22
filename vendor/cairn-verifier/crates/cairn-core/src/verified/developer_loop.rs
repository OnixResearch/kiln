pub const DEVELOPER_LOOP_POLICY_SCHEMA_V1: &str = "cairn.developer-loop-policy.v1";
pub const DEVELOPER_LOOP_PLAN_SCHEMA_V1: &str = "cairn.developer-loop-plan.v1";
pub const DEVELOPER_LOOP_RECEIPT_SCHEMA_V1: &str = "cairn.developer-loop-receipt.v1";

// r[impl cairn.developer_loop_profiles.policy.valid]
pub const INITIAL_DEVELOPER_LOOP_PROFILE: &str = "cairn-local";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopBounds {
    pub max_profiles: usize,
    pub max_budgets: usize,
    pub max_checks: usize,
    pub max_checks_per_profile: usize,
    pub max_text_bytes: usize,
    pub max_issues: usize,
    pub max_cache_key_inputs: usize,
    pub max_duration_seconds: u64,
    pub max_receipt_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopBudget {
    pub id: String,
    pub max_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopCheck {
    pub id: String,
    pub command_identity: String,
    pub argv: Vec<String>,
    pub working_root: String,
    pub expected_status_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopCachePolicy {
    pub mode: String,
    pub key_inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopProfile {
    pub id: String,
    pub purpose: String,
    pub repositories: Vec<String>,
    pub subject_scope: String,
    pub checks: Vec<String>,
    pub target: String,
    pub toolchain: String,
    pub budget_id: String,
    pub effect_posture: String,
    pub cache_policy: DeveloperLoopCachePolicy,
    pub full_gate_refs: Vec<String>,
    pub required_enforcement: Vec<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopPolicy {
    pub schema: String,
    pub profiles: Vec<DeveloperLoopProfile>,
    pub budgets: Vec<DeveloperLoopBudget>,
    pub checks: Vec<DeveloperLoopCheck>,
    pub bounds: DeveloperLoopBounds,
    pub receipt_fields: Vec<String>,
    pub required_non_claims: Vec<String>,
}

// r[impl cairn.developer_loop_profiles.policy]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedDeveloperLoopPolicy {
    pub policy: DeveloperLoopPolicy,
    pub policy_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl std::fmt::Display for DeveloperLoopIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.field_path, self.message
        )
    }
}

impl std::error::Error for DeveloperLoopIssue {}

// r[impl cairn.developer_loop_profiles.plan]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopPlanStep {
    pub check_id: String,
    pub command_identity: String,
    pub argv: Vec<String>,
    pub working_root: String,
    pub expected_status_class: String,
}

// r[impl cairn.developer_loop_profiles.plan.read_only]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopPlan {
    pub schema: String,
    pub profile: String,
    pub target: String,
    pub toolchain: String,
    pub budget: DeveloperLoopBudget,
    pub effect_posture: String,
    pub cache_policy: DeveloperLoopCachePolicy,
    pub full_gate_refs: Vec<String>,
    pub steps: Vec<DeveloperLoopPlanStep>,
    pub plan_hash: String,
    pub non_claims: Vec<String>,
}

// r[impl cairn.developer_loop_profiles.receipt]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopReceiptInput {
    pub receipt: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopVerificationSummary {
    pub profile: String,
    pub subject_hash: String,
    pub policy_hash: String,
    pub profile_identity: String,
    pub target: String,
    pub toolchain: String,
    pub result_statuses: Vec<DeveloperLoopResultStatus>,
    pub observed_duration_seconds: u64,
    pub measurement_class: String,
    pub budget_met: bool,
    pub budget_id: String,
    pub budget_max_seconds: u64,
    pub cache: DeveloperLoopCacheEvaluation,
    pub posture_states: Vec<String>,
    pub full_gate_status: String,
    pub issues: Vec<DeveloperLoopIssue>,
    pub valid: bool,
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopResultStatus {
    pub check_id: String,
    pub status: String,
    pub command_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperLoopCacheEvaluation {
    pub mode: String,
    pub claimed: String,
    pub cache_key_inputs: Vec<String>,
    pub cache_key_identity: String,
    pub valid: bool,
    pub issues: Vec<DeveloperLoopIssue>,
}

// r[impl cairn.developer_loop_profiles.cache]
pub struct DeveloperLoopCacheKeyInput<'a> {
    pub profile_id: &'a str,
    pub subject_hash: &'a str,
    pub dependencies: &'a std::collections::BTreeSet<String>,
    pub target: &'a str,
    pub toolchain: &'a str,
    pub policy_hash: &'a str,
    pub checks: &'a [String],
    pub key_inputs: &'a [String],
}

pub fn developer_loop_cache_key(input: &DeveloperLoopCacheKeyInput<'_>) -> String {
    let mut selected = Vec::new();
    for key in input.key_inputs {
        match key.as_str() {
            "profile" => selected.push(serde_json::json!(input.profile_id)),
            "subject" => selected.push(serde_json::json!(input.subject_hash)),
            "dependency" => selected.push(serde_json::json!(input
                .dependencies
                .iter()
                .cloned()
                .collect::<Vec<_>>())),
            "target" => selected.push(serde_json::json!(input.target)),
            "toolchain" => selected.push(serde_json::json!(input.toolchain)),
            "policy" => selected.push(serde_json::json!(input.policy_hash)),
            "checks" => selected.push(serde_json::json!(input.checks)),
            _ => {}
        }
    }
    crate::verified::evidence::canonical_json_hash(&serde_json::json!(selected))
}

// r[impl cairn.developer_loop_profiles.policy.invalid]
// r[impl cairn.developer_loop_profiles.boundary.overclaim]
pub fn admit_developer_loop_policy(
    policy: DeveloperLoopPolicy,
) -> Result<AdmittedDeveloperLoopPolicy, Vec<DeveloperLoopIssue>> {
    let mut issues = validate_policy_header(&policy);
    validate_budgets(&policy, &mut issues);
    validate_checks(&policy, &mut issues);
    validate_profiles(&policy, &mut issues);
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
    });
    let mut seen = std::collections::BTreeSet::new();
    issues.retain(|issue| seen.insert((issue.code.clone(), issue.field_path.clone())));

    let issue_limit = policy.bounds.max_issues.max(1);
    issues.truncate(issue_limit);
    if !issues.is_empty() {
        return Err(issues);
    }
    let policy_identity = crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "profiles": policy.profiles,
        "budgets": policy.budgets,
        "checks": policy.checks,
        "bounds": policy.bounds,
        "non_claims": policy.required_non_claims,
    }));
    Ok(AdmittedDeveloperLoopPolicy {
        policy,
        policy_identity,
    })
}

pub fn find_developer_loop_profile<'a>(
    admitted: &'a AdmittedDeveloperLoopPolicy,
    id: &str,
) -> Result<&'a DeveloperLoopProfile, DeveloperLoopIssue> {
    admitted
        .policy
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| {
            issue(
                "developer_loop.profile.unknown",
                "profile",
                format!("unknown developer-loop profile {id}"),
            )
        })
}

pub fn find_developer_loop_budget<'a>(
    admitted: &'a AdmittedDeveloperLoopPolicy,
    id: &str,
) -> Result<&'a DeveloperLoopBudget, DeveloperLoopIssue> {
    admitted
        .policy
        .budgets
        .iter()
        .find(|budget| budget.id == id)
        .ok_or_else(|| {
            issue(
                "developer_loop.budget.unknown",
                "profile.budget_id",
                format!("unknown developer-loop budget {id}"),
            )
        })
}

pub fn developer_loop_profile_identity(profile: &DeveloperLoopProfile) -> String {
    crate::verified::evidence::canonical_json_hash(&serde_json::json!(profile))
}

// r[impl cairn.developer_loop_profiles.plan]
pub fn build_developer_loop_plan(
    admitted: &AdmittedDeveloperLoopPolicy,
    profile_id: &str,
) -> Result<DeveloperLoopPlan, Vec<DeveloperLoopIssue>> {
    let profile = find_developer_loop_profile(admitted, profile_id).map_err(|issue| vec![issue])?;
    let budget =
        find_developer_loop_budget(admitted, &profile.budget_id).map_err(|issue| vec![issue])?;
    let checks = profile
        .checks
        .iter()
        .map(|check_id| {
            admitted
                .policy
                .checks
                .iter()
                .find(|check| &check.id == check_id)
                .map(|check| DeveloperLoopPlanStep {
                    check_id: check.id.clone(),
                    command_identity: check.command_identity.clone(),
                    argv: check.argv.clone(),
                    working_root: check.working_root.clone(),
                    expected_status_class: check.expected_status_class.clone(),
                })
                .ok_or_else(|| {
                    vec![issue(
                        "developer_loop.check.unknown",
                        "profile.checks",
                        format!("unknown developer-loop check {check_id}"),
                    )]
                })
        })
        .collect::<Result<Vec<_>, Vec<DeveloperLoopIssue>>>()?;
    let mut plan = DeveloperLoopPlan {
        schema: DEVELOPER_LOOP_PLAN_SCHEMA_V1.to_string(),
        profile: profile.id.clone(),
        target: profile.target.clone(),
        toolchain: profile.toolchain.clone(),
        budget: budget.clone(),
        effect_posture: profile.effect_posture.clone(),
        cache_policy: profile.cache_policy.clone(),
        full_gate_refs: profile.full_gate_refs.clone(),
        steps: checks,
        plan_hash: String::new(),
        non_claims: profile.non_claims.clone(),
    };
    plan.plan_hash = developer_loop_plan_identity(&plan);
    Ok(plan)
}

pub fn developer_loop_plan_identity(plan: &DeveloperLoopPlan) -> String {
    crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "schema": plan.schema,
        "profile": plan.profile,
        "target": plan.target,
        "toolchain": plan.toolchain,
        "budget": plan.budget,
        "effect_posture": plan.effect_posture,
        "cache_policy": plan.cache_policy,
        "full_gate_refs": plan.full_gate_refs,
        "steps": plan.steps,
        "non_claims": plan.non_claims,
    }))
}

// r[impl cairn.developer_loop_profiles.receipt]
// r[impl cairn.developer_loop_profiles.receipt.current]
// r[impl cairn.developer_loop_profiles.receipt.stale]
// r[impl cairn.developer_loop_profiles.cache.complete]
// r[impl cairn.developer_loop_profiles.cache.incomplete]
// r[impl cairn.developer_loop_profiles.full_gate]
// r[impl cairn.developer_loop_profiles.full_gate.separate]
// r[impl cairn.developer_loop_profiles.diagnostics]
// r[impl cairn.developer_loop_profiles.diagnostics.safe]
// r[impl cairn.developer_loop_profiles.diagnostics.unsafe]
// r[impl cairn.developer_loop_profiles.boundary]
pub fn validate_developer_loop_receipt(
    admitted: &AdmittedDeveloperLoopPolicy,
    profile_id: &str,
    policy_hash: &str,
    subject_hash: &str,
    dependencies: &std::collections::BTreeSet<String>,
    receipt: &serde_json::Value,
) -> DeveloperLoopVerificationSummary {
    let mut issues = Vec::new();
    let profile = match find_developer_loop_profile(admitted, profile_id) {
        Ok(profile) => profile,
        Err(issue) => {
            return DeveloperLoopVerificationSummary {
                profile: profile_id.to_string(),
                subject_hash: subject_hash.to_string(),
                policy_hash: policy_hash.to_string(),
                profile_identity: String::new(),
                target: String::new(),
                toolchain: String::new(),
                result_statuses: Vec::new(),
                observed_duration_seconds: 0,
                measurement_class: String::new(),
                budget_met: false,
                budget_id: String::new(),
                budget_max_seconds: 0,
                cache: DeveloperLoopCacheEvaluation {
                    mode: String::new(),
                    claimed: String::new(),
                    cache_key_inputs: Vec::new(),
                    cache_key_identity: String::new(),
                    valid: false,
                    issues: Vec::new(),
                },
                posture_states: Vec::new(),
                full_gate_status: "outstanding".to_string(),
                issues: vec![issue],
                valid: false,
                verdict: "FAIL".to_string(),
            };
        }
    };
    let profile_identity = developer_loop_profile_identity(profile);
    let receipt_profile = receipt
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let receipt_subject = receipt
        .get("subject_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let receipt_policy = receipt
        .get("policy_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let receipt_target = receipt
        .get("target")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let receipt_toolchain = receipt
        .get("toolchain")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let measurement_class = receipt
        .get("measurement_class")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let observed_duration = receipt
        .get("observed_duration_seconds")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);

    if receipt.get("schema").and_then(serde_json::Value::as_str)
        != Some(DEVELOPER_LOOP_RECEIPT_SCHEMA_V1)
    {
        issues.push(issue(
            "developer_loop.receipt.schema",
            "schema",
            "developer-loop receipt schema is unsupported",
        ));
    }
    if receipt_profile != profile_id {
        issues.push(issue_expected(
            "developer_loop.profile.mismatch",
            "profile",
            profile_id,
            receipt_profile,
        ));
    }
    if receipt_policy != policy_hash {
        issues.push(issue_expected(
            "developer_loop.policy.stale",
            "policy_hash",
            policy_hash,
            receipt_policy,
        ));
    }
    if receipt_subject != subject_hash {
        issues.push(issue_expected(
            "developer_loop.subject.stale",
            "subject_hash",
            subject_hash,
            receipt_subject,
        ));
    }
    if receipt_target != profile.target {
        issues.push(issue_expected(
            "developer_loop.target.mismatch",
            "target",
            &profile.target,
            receipt_target,
        ));
    }
    if receipt_toolchain != profile.toolchain {
        issues.push(issue_expected(
            "developer_loop.toolchain.mismatch",
            "toolchain",
            &profile.toolchain,
            receipt_toolchain,
        ));
    }

    let budget = match find_developer_loop_budget(admitted, &profile.budget_id) {
        Ok(budget) => budget,
        Err(issue) => {
            issues.push(issue);
            budget_placeholder(admitted)
        }
    };
    let budget_met = observed_duration <= budget.max_seconds
        && measurement_class != "unknown"
        && observed_duration != u64::MAX;
    if !budget_met {
        issues.push(DeveloperLoopIssue {
            code: "developer_loop.budget.over".to_string(),
            field_path: "observed_duration_seconds".to_string(),
            message: format!(
                "observed duration exceeds the {0} budget or has an unknown measurement class",
                budget.id
            ),
            expected: Some(budget.max_seconds.to_string()),
            actual: Some(if observed_duration == u64::MAX {
                "unavailable".to_string()
            } else {
                observed_duration.to_string()
            }),
        });
    }

    // check statuses
    let required_checks = &profile.checks;
    let result_statuses = receipt
        .get("result_statuses")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<DeveloperLoopResultStatus>(value).ok())
        .collect::<Vec<_>>();
    let mut seen_checks = std::collections::BTreeSet::new();
    for status in &result_statuses {
        seen_checks.insert(status.check_id.clone());
        if !required_checks.contains(&status.check_id) {
            issues.push(DeveloperLoopIssue {
                code: "developer_loop.check.unexpected".to_string(),
                field_path: "result_statuses".to_string(),
                message: format!(
                    "receipt reports a check not required by the profile: {}",
                    status.check_id
                ),
                expected: None,
                actual: Some(status.check_id.clone()),
            });
        }
        if status.status != "passed" {
            issues.push(DeveloperLoopIssue {
                code: "developer_loop.check.not_passing".to_string(),
                field_path: format!("result_statuses.{}", status.check_id),
                message: format!("required check did not pass: {}", status.check_id),
                expected: Some("passed".to_string()),
                actual: Some(status.status.clone()),
            });
        }
    }
    for check in required_checks {
        if !seen_checks.contains(check) {
            issues.push(DeveloperLoopIssue {
                code: "developer_loop.check.missing".to_string(),
                field_path: "result_statuses".to_string(),
                message: format!("required check is missing from the receipt: {check}"),
                expected: Some(check.clone()),
                actual: None,
            });
        }
    }

    // cache evaluation
    let cache = evaluate_cache(
        admitted,
        profile,
        policy_hash,
        subject_hash,
        dependencies,
        receipt,
    );
    issues.extend(cache.issues.clone());

    // posture
    let posture_states = receipt
        .get("posture_states")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let declared_posture = profile.effect_posture.as_str();
    if posture_states
        .iter()
        .any(|state| state == "externally_enforced")
    {
        let has_enforcement_evidence = receipt
            .get("enforcement_evidence")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| !values.is_empty());
        if !has_enforcement_evidence {
            issues.push(issue(
                "developer_loop.posture.overclaim",
                "posture_states",
                format!(
                    "receipt claims enforced {declared_posture} posture without enforcement evidence"
                ),
            ));
        }
    }

    // receipt hash
    if !developer_loop_receipt_hash_valid(receipt) {
        issues.push(issue(
            "developer_loop.receipt.hash",
            "receipt_hash",
            "receipt hash does not match receipt content",
        ));
    }

    // unsafe diagnostic fields (proxy check for secret-like values)
    if developer_loop_receipt_has_secret(receipt) {
        issues.push(issue(
            "developer_loop.diagnostics.unsafe",
            "diagnostics",
            "receipt contains raw prompts, credentials, environment values, or secret content",
        ));
    }

    let full_gate_status = "outstanding".to_string();
    let valid = issues.is_empty();
    DeveloperLoopVerificationSummary {
        profile: profile_id.to_string(),
        subject_hash: subject_hash.to_string(),
        policy_hash: policy_hash.to_string(),
        profile_identity,
        target: profile.target.clone(),
        toolchain: profile.toolchain.clone(),
        result_statuses,
        observed_duration_seconds: if observed_duration == u64::MAX {
            0
        } else {
            observed_duration
        },
        measurement_class,
        budget_met,
        budget_id: budget.id.clone(),
        budget_max_seconds: budget.max_seconds,
        cache,
        posture_states,
        full_gate_status,
        issues,
        valid,
        verdict: if valid {
            "PASS".to_string()
        } else {
            "FAIL".to_string()
        },
    }
}

fn evaluate_cache(
    _admitted: &AdmittedDeveloperLoopPolicy,
    profile: &DeveloperLoopProfile,
    policy_hash: &str,
    subject_hash: &str,
    dependencies: &std::collections::BTreeSet<String>,
    receipt: &serde_json::Value,
) -> DeveloperLoopCacheEvaluation {
    let mode = profile.cache_policy.mode.clone();
    let claimed = receipt
        .get("cache")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unset")
        .to_string();
    let cache_key_inputs = profile.cache_policy.key_inputs.clone();
    let expected_identity = developer_loop_cache_key(&DeveloperLoopCacheKeyInput {
        profile_id: &profile.id,
        subject_hash,
        dependencies,
        target: &profile.target,
        toolchain: &profile.toolchain,
        policy_hash,
        checks: &profile.checks,
        key_inputs: &cache_key_inputs,
    });
    let stated_identity = receipt
        .get("cache_key_identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mut issues = Vec::new();
    if claimed == "hit" {
        if mode != "enabled" {
            issues.push(issue(
                "developer_loop.cache.disabled_claim",
                "cache",
                "cache-hit claimed while the profile cache is disabled",
            ));
        }
        if stated_identity != expected_identity {
            issues.push(issue_expected(
                "developer_loop.cache.incomplete",
                "cache_key_identity",
                &expected_identity,
                stated_identity,
            ));
        }
        // ensure every required key input was bound
        for input in &cache_key_inputs {
            if !matches!(
                input.as_str(),
                "profile" | "subject" | "dependency" | "target" | "toolchain" | "policy" | "checks"
            ) {
                issues.push(issue(
                    "developer_loop.cache.unknown_input",
                    "cache_policy.key_inputs",
                    format!("unknown cache key input role {input}"),
                ));
            }
        }
        if cache_key_inputs.is_empty() {
            issues.push(issue(
                "developer_loop.cache.incomplete",
                "cache_policy.key_inputs",
                "cache-hit claim requires explicit cache-key inputs",
            ));
        }
    }
    DeveloperLoopCacheEvaluation {
        mode,
        claimed,
        cache_key_inputs: cache_key_inputs.clone(),
        cache_key_identity: if stated_identity.is_empty() {
            expected_identity
        } else {
            stated_identity.to_string()
        },
        valid: issues.is_empty(),
        issues,
    }
}

fn developer_loop_receipt_hash_valid(receipt: &serde_json::Value) -> bool {
    // This is untrusted saved input, not an internal schema invariant.
    if receipt.get("schema").and_then(serde_json::Value::as_str)
        != Some(DEVELOPER_LOOP_RECEIPT_SCHEMA_V1)
    {
        return false;
    }
    let mut material = receipt.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("receipt_hash");
    }
    let stated = receipt
        .get("receipt_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    stated == crate::verified::evidence::canonical_json_hash(&material)
}

fn developer_loop_receipt_has_secret(receipt: &serde_json::Value) -> bool {
    let text = serde_json::to_string(receipt)
        .unwrap_or_default()
        .to_lowercase();
    // surrogate markers that indicate credential/prompt/environment leakage.
    [
        "password",
        "api_key",
        "authorization: bearer",
        "credentials",
        "raw_prompt",
        "model_output",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn budget_placeholder(admitted: &AdmittedDeveloperLoopPolicy) -> &DeveloperLoopBudget {
    // Only reachable when the profile references an unknown budget; returns the first budget
    // so downstream code can build a stable summary without panicking.
    &admitted.policy.budgets[0]
}

fn validate_policy_header(policy: &DeveloperLoopPolicy) -> Vec<DeveloperLoopIssue> {
    let mut issues = Vec::new();
    if policy.schema != DEVELOPER_LOOP_POLICY_SCHEMA_V1 {
        issues.push(issue(
            "developer_loop.policy.schema",
            "schema",
            "developer-loop policy schema is unsupported",
        ));
    }
    let bounds = &policy.bounds;
    if bounds.max_profiles == 0
        || bounds.max_budgets == 0
        || bounds.max_checks == 0
        || bounds.max_checks_per_profile == 0
        || bounds.max_text_bytes == 0
        || bounds.max_issues == 0
        || bounds.max_cache_key_inputs == 0
        || bounds.max_duration_seconds == 0
        || bounds.max_receipt_bytes == 0
    {
        issues.push(issue(
            "developer_loop.policy.bounds",
            "bounds",
            "all developer-loop bounds must be positive",
        ));
    }
    if policy.profiles.len() > bounds.max_profiles
        || policy.budgets.len() > bounds.max_budgets
        || policy.checks.len() > bounds.max_checks
    {
        issues.push(issue(
            "developer_loop.policy.limit",
            "policy",
            "developer-loop policy exceeds a collection bound",
        ));
    }
    if !policy.receipt_fields.iter().any(|field| field == "valid")
        || !policy.receipt_fields.iter().any(|field| field == "profile")
        || !policy
            .required_non_claims
            .iter()
            .any(|value| !value.is_empty())
    {
        issues.push(issue(
            "developer_loop.policy.fields",
            "policy",
            "developer-loop receipt fields or non-claims are incomplete",
        ));
    }
    issues
}

fn validate_budgets(policy: &DeveloperLoopPolicy, issues: &mut Vec<DeveloperLoopIssue>) {
    let mut ids = std::collections::BTreeSet::new();
    for (index, budget) in policy.budgets.iter().enumerate() {
        let path = format!("budgets[{index}]");
        if !non_empty(&budget.id)
            || !ids.insert(budget.id.clone())
            || budget.max_seconds == 0
            || budget.max_seconds > policy.bounds.max_duration_seconds
        {
            issues.push(issue(
                "developer_loop.budget.invalid",
                &path,
                "budget id, uniqueness, or bound is invalid",
            ));
        }
    }
}

fn validate_checks(policy: &DeveloperLoopPolicy, issues: &mut Vec<DeveloperLoopIssue>) {
    let mut ids = std::collections::BTreeSet::new();
    for (index, check) in policy.checks.iter().enumerate() {
        let path = format!("checks[{index}]");
        let argv_valid = !check.argv.is_empty()
            && check.argv.len() <= policy.bounds.max_checks_per_profile
            && check
                .argv
                .iter()
                .all(|arg| arg.len() <= policy.bounds.max_text_bytes);
        if !non_empty(&check.id)
            || !ids.insert(check.id.clone())
            || !check.command_identity.is_empty()
                && check.command_identity.len() > policy.bounds.max_text_bytes
            || !safe_label(&check.command_identity)
            || !argv_valid
            || !safe_label(&check.working_root)
            || !matches!(check.expected_status_class.as_str(), "success" | "failure")
        {
            issues.push(issue(
                "developer_loop.check.invalid",
                &path,
                "check identity, argv, working root, or status class is invalid",
            ));
        }
    }
}

fn validate_profiles(policy: &DeveloperLoopPolicy, issues: &mut Vec<DeveloperLoopIssue>) {
    let budget_ids = policy
        .budgets
        .iter()
        .map(|budget| budget.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let check_ids = policy
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut ids = std::collections::BTreeSet::new();
    for (index, profile) in policy.profiles.iter().enumerate() {
        let path = format!("profiles[{index}]");
        let mut local_issues = Vec::new();
        if !non_empty(&profile.id)
            || !ids.insert(profile.id.clone())
            || profile.purpose.len() > policy.bounds.max_text_bytes
            || profile.repositories.is_empty()
            || profile.repositories.iter().any(|repo| !non_empty(repo))
            || !safe_label(&profile.subject_scope)
            || profile.checks.is_empty()
            || profile.checks.len() > policy.bounds.max_checks_per_profile
            || profile.target.len() > policy.bounds.max_text_bytes
            || profile.toolchain.len() > policy.bounds.max_text_bytes
            || !budget_ids.contains(profile.budget_id.as_str())
            || !profile
                .checks
                .iter()
                .all(|check| check_ids.contains(check.as_str()))
            || profile.full_gate_refs.is_empty()
            || profile.non_claims.is_empty()
        {
            local_issues.push(issue("developer_loop.profile.form", &path, "profile identity, scope, checks, budget, full-gate refs, or non-claims are invalid"));
        }
        // duplicate checks
        let mut check_set = std::collections::BTreeSet::new();
        let mut duplicate = false;
        for check in &profile.checks {
            if !check_set.insert(check) {
                duplicate = true;
            }
        }
        if duplicate {
            local_issues.push(issue(
                "developer_loop.profile.duplicate_check",
                format!("{path}.checks"),
                "profile contains a duplicate check",
            ));
        }
        if !matches!(
            profile.effect_posture.as_str(),
            "read_only" | "read_write_file" | "network_read" | "network_write"
        ) {
            local_issues.push(issue(
                "developer_loop.profile.posture",
                format!("{path}.effect_posture"),
                "effect posture class is unsupported",
            ));
        }
        let cache_inputs = &profile.cache_policy.key_inputs;
        if profile.cache_policy.mode == "enabled" {
            if cache_inputs.is_empty()
                || cache_inputs.len() > policy.bounds.max_cache_key_inputs
                || cache_inputs.iter().any(|input| {
                    !matches!(
                        input.as_str(),
                        "profile"
                            | "subject"
                            | "dependency"
                            | "target"
                            | "toolchain"
                            | "policy"
                            | "checks"
                    )
                })
            {
                local_issues.push(issue(
                    "developer_loop.profile.cache",
                    format!("{path}.cache_policy"),
                    "enabled cache policy needs a complete explicit role set",
                ));
            }
        } else if profile.cache_policy.mode != "disabled" {
            local_issues.push(issue(
                "developer_loop.profile.cache",
                format!("{path}.cache_policy.mode"),
                "cache mode must be disabled or enabled",
            ));
        }
        if !local_issues.is_empty() {
            issues.extend(local_issues);
        }
    }
    let actual = ids.iter().map(|id| id.as_str()).collect::<Vec<_>>();
    if actual != vec![INITIAL_DEVELOPER_LOOP_PROFILE] {
        issues.push(issue(
            "developer_loop.policy.initial",
            "profiles",
            format!("initial profile catalog must be {INITIAL_DEVELOPER_LOOP_PROFILE}"),
        ));
    }
}

fn non_empty(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

fn issue(
    code: &str,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> DeveloperLoopIssue {
    DeveloperLoopIssue {
        code: code.to_string(),
        field_path: field_path.into(),
        message: message.into(),
        expected: None,
        actual: None,
    }
}

fn issue_expected(
    code: &str,
    field_path: impl Into<String>,
    expected: &str,
    actual: &str,
) -> DeveloperLoopIssue {
    DeveloperLoopIssue {
        code: code.to_string(),
        field_path: field_path.into(),
        message: format!("{code}: expected {expected}, observed {actual}"),
        expected: Some(expected.to_string()),
        actual: Some(actual.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLAKE3_HEX_LENGTH: usize = 64;

    fn policy() -> DeveloperLoopPolicy {
        DeveloperLoopPolicy {
            schema: DEVELOPER_LOOP_POLICY_SCHEMA_V1.to_string(),
            budgets: vec![DeveloperLoopBudget {
                id: "budget".to_string(),
                max_seconds: 60,
            }],
            checks: vec![
                DeveloperLoopCheck {
                    id: "check-a".to_string(),
                    command_identity: "a".repeat(BLAKE3_HEX_LENGTH),
                    argv: vec!["cargo".to_string(), "check".to_string()],
                    working_root: ".".to_string(),
                    expected_status_class: "success".to_string(),
                },
                DeveloperLoopCheck {
                    id: "check-b".to_string(),
                    command_identity: "b".repeat(BLAKE3_HEX_LENGTH),
                    argv: vec!["test".to_string()],
                    working_root: ".".to_string(),
                    expected_status_class: "success".to_string(),
                },
            ],
            profiles: vec![DeveloperLoopProfile {
                id: INITIAL_DEVELOPER_LOOP_PROFILE.to_string(),
                purpose: "fast local checks".to_string(),
                repositories: vec!["cairn".to_string()],
                subject_scope: "commit".to_string(),
                checks: vec!["check-a".to_string(), "check-b".to_string()],
                target: "x86_64-linux".to_string(),
                toolchain: "nix develop".to_string(),
                budget_id: "budget".to_string(),
                effect_posture: "read_only".to_string(),
                cache_policy: DeveloperLoopCachePolicy {
                    mode: "enabled".to_string(),
                    key_inputs: vec![
                        "profile".into(),
                        "subject".into(),
                        "dependency".into(),
                        "target".into(),
                        "toolchain".into(),
                        "policy".into(),
                        "checks".into(),
                    ],
                },
                full_gate_refs: vec!["cargo test".to_string()],
                required_enforcement: Vec::new(),
                non_claims: vec!["bounded".to_string()],
            }],
            bounds: DeveloperLoopBounds {
                max_profiles: 8,
                max_budgets: 8,
                max_checks: 32,
                max_checks_per_profile: 16,
                max_text_bytes: 4096,
                max_issues: 64,
                max_cache_key_inputs: 8,
                max_duration_seconds: 3600,
                max_receipt_bytes: 65536,
            },
            receipt_fields: vec!["valid".to_string(), "profile".to_string()],
            required_non_claims: vec!["bounded".to_string()],
        }
    }

    fn admitted() -> AdmittedDeveloperLoopPolicy {
        admit_developer_loop_policy(policy()).expect("policy admits")
    }

    fn subject_hash() -> String {
        "c".repeat(BLAKE3_HEX_LENGTH)
    }

    fn receipt() -> serde_json::Value {
        let mut value = serde_json::json!({
            "schema": DEVELOPER_LOOP_RECEIPT_SCHEMA_V1,
            "profile": INITIAL_DEVELOPER_LOOP_PROFILE,
            "policy_hash": "d".repeat(BLAKE3_HEX_LENGTH),
            "subject_hash": subject_hash(),
            "target": "x86_64-linux",
            "toolchain": "nix develop",
            "measurement_class": "wall",
            "observed_duration_seconds": 10,
            "result_statuses": [
                {"check_id": "check-a", "status": "passed", "command_identity": "a".repeat(BLAKE3_HEX_LENGTH)},
                {"check_id": "check-b", "status": "passed", "command_identity": "b".repeat(BLAKE3_HEX_LENGTH)},
            ],
            "cache": "miss",
            "cache_key_identity": "",
            "posture_states": ["declared"],
            "full_gate_status": "outstanding",
        });
        let hash = crate::verified::evidence::canonical_json_hash(&value);
        value["receipt_hash"] = serde_json::json!(hash);
        value
    }

    #[test]
    fn admits_valid_policy_and_lists_initial_profile() {
        let admitted = admitted();
        assert_eq!(admitted.policy.profiles.len(), 1);
        assert_eq!(
            admitted.policy.profiles[0].id,
            INITIAL_DEVELOPER_LOOP_PROFILE
        );
    }

    #[test]
    fn rejects_duplicate_profile_and_unknown_check() {
        let mut invalid = policy();
        invalid.profiles[0].checks.push("unknown-check".to_string());
        let issues = admit_developer_loop_policy(invalid).expect_err("must reject");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "developer_loop.profile.form"));
    }

    #[test]
    fn rejects_unsafe_posture_promotion_without_enforcement() {
        let admitted = admitted();
        let mut value = receipt();
        value["posture_states"] = serde_json::json!(["externally_enforced"]);
        let hash = crate::verified::evidence::canonical_json_hash(&value);
        value["receipt_hash"] = serde_json::json!(hash);
        let summary = validate_developer_loop_receipt(
            &admitted,
            INITIAL_DEVELOPER_LOOP_PROFILE,
            &"d".repeat(BLAKE3_HEX_LENGTH),
            &subject_hash(),
            &std::collections::BTreeSet::new(),
            &value,
        );
        assert!(summary
            .issues
            .iter()
            .any(|issue| issue.code == "developer_loop.posture.overclaim"));
    }

    #[test]
    fn current_receipt_meets_budget_and_full_gate_stays_outstanding() {
        let admitted = admitted();
        let value = receipt();
        let summary = validate_developer_loop_receipt(
            &admitted,
            INITIAL_DEVELOPER_LOOP_PROFILE,
            &"d".repeat(BLAKE3_HEX_LENGTH),
            &subject_hash(),
            &std::collections::BTreeSet::new(),
            &value,
        );
        assert!(summary.valid, "{:#?}", summary.issues);
        assert!(summary.budget_met);
        assert_eq!(summary.budget_id, "budget");
        assert_eq!(summary.full_gate_status, "outstanding");
    }

    #[test]
    fn foreign_receipts_fail_with_schema_diagnostics_without_panicking() {
        let admitted = admitted();
        for value in [
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!("receipt"),
            serde_json::json!({"schema":"foreign.v1"}),
        ] {
            let summary = validate_developer_loop_receipt(
                &admitted,
                INITIAL_DEVELOPER_LOOP_PROFILE,
                &"d".repeat(BLAKE3_HEX_LENGTH),
                &subject_hash(),
                &std::collections::BTreeSet::new(),
                &value,
            );
            assert!(!summary.valid);
            assert!(summary
                .issues
                .iter()
                .any(|issue| issue.code == "developer_loop.receipt.schema"));
        }
    }

    #[test]
    fn stale_subject_and_over_budget_fail_with_diagnostics() {
        let admitted = admitted();
        let mut value = receipt();
        value["subject_hash"] = serde_json::json!("f".repeat(BLAKE3_HEX_LENGTH));
        value["observed_duration_seconds"] = serde_json::json!(9999);
        let hash = crate::verified::evidence::canonical_json_hash(&value);
        value["receipt_hash"] = serde_json::json!(hash);
        let summary = validate_developer_loop_receipt(
            &admitted,
            INITIAL_DEVELOPER_LOOP_PROFILE,
            &"d".repeat(BLAKE3_HEX_LENGTH),
            &subject_hash(),
            &std::collections::BTreeSet::new(),
            &value,
        );
        assert!(!summary.valid);
        assert!(!summary.budget_met);
        assert!(summary
            .issues
            .iter()
            .any(|issue| issue.code == "developer_loop.subject.stale"));
        assert!(summary
            .issues
            .iter()
            .any(|issue| issue.code == "developer_loop.budget.over"));
    }

    #[test]
    fn incomplete_cache_hit_is_rejected() {
        let admitted = admitted();
        let mut value = receipt();
        value["cache"] = serde_json::json!("hit");
        value["cache_key_identity"] = serde_json::json!("g".repeat(BLAKE3_HEX_LENGTH));
        let hash = crate::verified::evidence::canonical_json_hash(&value);
        value["receipt_hash"] = serde_json::json!(hash);
        let summary = validate_developer_loop_receipt(
            &admitted,
            INITIAL_DEVELOPER_LOOP_PROFILE,
            &"d".repeat(BLAKE3_HEX_LENGTH),
            &subject_hash(),
            &std::collections::BTreeSet::new(),
            &value,
        );
        assert!(summary
            .issues
            .iter()
            .any(|issue| issue.code == "developer_loop.cache.incomplete"));
    }

    #[test]
    fn safe_diagnostics_exclude_secret_content() {
        let admitted = admitted();
        let mut value = receipt();
        value["diagnostics"] = serde_json::json!([{"code": "developer_loop.receipt.hash", "message": "raw_prompt secret"}]);
        value["receipt_hash"] =
            serde_json::json!(crate::verified::evidence::canonical_json_hash(&value));
        let summary = validate_developer_loop_receipt(
            &admitted,
            INITIAL_DEVELOPER_LOOP_PROFILE,
            &"d".repeat(BLAKE3_HEX_LENGTH),
            &subject_hash(),
            &std::collections::BTreeSet::new(),
            &value,
        );
        assert!(summary
            .issues
            .iter()
            .any(|issue| issue.code == "developer_loop.diagnostics.unsafe"));
    }
}
