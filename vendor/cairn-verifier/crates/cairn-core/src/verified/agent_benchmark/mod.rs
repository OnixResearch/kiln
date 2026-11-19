//! Admission of explicitly supplied agent-run observations. No host I/O, tokenizer,
//! model execution, clock, or lifecycle authority is provided here.
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::evidence::{blake3_hex, canonical_json, canonical_json_hash};

pub const INPUT_SCHEMA: &str = "cairn.agent-benchmark.input.v1";
pub const COMPARISON_SCHEMA: &str = "cairn.agent-benchmark.comparison.v1";
const POLICY_SCHEMA: &str = "cairn.agent-benchmark.policy.v1";
const REPORT_SCHEMA: &str = "cairn.agent-benchmark.report.v1";
const COMPARISON_REPORT_SCHEMA: &str = "cairn.agent-benchmark.comparison-report.v1";
const NON_CLAIMS: [&str; 3] = [
    "Saved observations and provider usage are declared inputs, not independently authenticated facts.",
    "Representation replays do not establish end-to-end agent efficiency or task correctness.",
    "A scoped live-host comparison does not establish general token savings or code correctness.",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkError {
    pub class: &'static str,
}
impl BenchmarkError {
    const fn new(class: &'static str) -> Self {
        Self { class }
    }
}
impl fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.class)
    }
}
impl Error for BenchmarkError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkPolicy {
    pub schema: String,
    pub bounds: Bounds,
    pub non_claims: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub max_repetitions: usize,
    pub max_calls_per_run: usize,
    pub max_input_bytes: usize,
    pub max_scope_bytes: usize,
    pub max_event_count: u64,
    pub max_token_count: u64,
    pub max_duration_ms: u64,
}
impl BenchmarkPolicy {
    pub fn bundled() -> Result<Self, BenchmarkError> {
        let policy: Self = serde_json::from_str(include_str!(
            "../../../../../cairn-policy/generated/agent-benchmark.json"
        ))
        .map_err(|_| BenchmarkError::new("policy.invalid"))?;
        policy.check()?;
        Ok(policy)
    }
    fn check(&self) -> Result<(), BenchmarkError> {
        let b = &self.bounds;
        if self.schema != POLICY_SCHEMA
            || !self.non_claims.iter().map(String::as_str).eq(NON_CLAIMS)
            || !(1..=32).contains(&b.max_repetitions)
            || !(1..=256).contains(&b.max_calls_per_run)
            || !(1..=1_048_576).contains(&b.max_input_bytes)
            || !(1..=96).contains(&b.max_scope_bytes)
            || !(1..=1_000_000).contains(&b.max_event_count)
            || !(1..=1_000_000_000_000_000_000).contains(&b.max_token_count)
            || !(1..=3_600_000).contains(&b.max_duration_ms)
        {
            return Err(BenchmarkError::new("policy.invalid"));
        }
        Ok(())
    }
    pub fn max_input_bytes(&self) -> usize {
        self.bounds.max_input_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkInput {
    pub schema: String,
    pub scenario: Scenario,
    pub runs: Vec<Run>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub scope: String,
    pub mode: ObservationMode,
    pub repetitions: usize,
    pub max_calls_per_run: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    RepresentationReplay,
    LiveHost,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub run_id: String,
    pub cohort: Cohort,
    pub trace: Trace,
    pub oracle: OracleObservation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cohort {
    pub source_projection: String,
    pub product_revision: String,
    pub policy: String,
    pub fixture: String,
    pub oracle: String,
    pub model: String,
    pub agent_configuration: String,
    pub catalog: String,
    pub presentation: String,
    pub accounting: Accounting,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Accounting {
    ProviderUsage {
        usage_source: String,
    },
    ExactTokenizer {
        implementation_revision: String,
        encoding: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub declared_calls: usize,
    pub complete: bool,
    pub calls: Vec<Call>,
    pub retry_count: u64,
    pub tool_call_count: u64,
    pub repeated_read_count: u64,
    pub duration_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub call_id: String,
    pub sequence: usize,
    pub usage: Option<TokenUsage>,
    pub content_identity: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenUsage {
    pub uncached_input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub reasoning: Option<u64>,
    pub reasoning_inclusion: ReasoningInclusion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningInclusion {
    IncludedInOutput,
    AdditionalToOutput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleObservation {
    pub verdict: OracleVerdict,
    pub agent_claimed_completion: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleVerdict {
    Completed,
    CorrectlyBlocked,
    Rejected,
    Inconclusive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    CorrectlyBlocked,
    FalseCompletion,
    Failed,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    pub uncached_input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub reasoning_complete: bool,
    pub billed_output: u64,
    pub billed_total: u64,
    pub retry_count: u64,
    pub tool_call_count: u64,
    pub repeated_read_count: u64,
    pub duration_ms: u64,
    pub complete_cost: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeCounts {
    pub completed: u64,
    pub correctly_blocked: u64,
    pub false_completion: u64,
    pub failed: u64,
    pub incomplete: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    pub run_id: String,
    pub outcome: Outcome,
    pub cost_available: bool,
    pub totals: Totals,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostPerCompletion {
    pub attempted_tokens: u64,
    pub correct_completions: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostPerBlock {
    pub attempted_tokens: u64,
    pub correct_blocks: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkReport {
    pub schema: String,
    pub input_hash: String,
    pub policy_hash: String,
    pub scope: String,
    pub mode: ObservationMode,
    pub repetitions: usize,
    pub cohort_hash: String,
    pub product_revision: String,
    pub attempted_runs: usize,
    pub outcomes: OutcomeCounts,
    pub totals: Totals,
    pub runs: Vec<RunReport>,
    pub tokens_per_correct_completion: Option<CostPerCompletion>,
    pub tokens_per_correct_block: Option<CostPerBlock>,
    pub non_claims: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonInput {
    pub schema: String,
    pub allowed_candidate_differences: Vec<AllowedDifference>,
    pub baseline: BenchmarkInput,
    pub candidate: BenchmarkInput,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowedDifference {
    ProductRevision,
    SourceProjection,
    AgentConfiguration,
    Catalog,
    Presentation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Improved,
    NotImproved,
    Withheld,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedTokenChange {
    pub baseline_attempted_tokens: u64,
    pub baseline_correct_completions: u64,
    pub candidate_attempted_tokens: u64,
    pub candidate_correct_completions: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonReport {
    pub schema: String,
    pub baseline: BenchmarkReport,
    pub candidate: BenchmarkReport,
    pub comparable: bool,
    pub drift_fields: Vec<String>,
    pub verdict: Verdict,
    pub withheld_reasons: Vec<String>,
    pub scoped_token_change: Option<ScopedTokenChange>,
    pub non_claims: Vec<String>,
}

fn identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn bounded_scope(scope: &str, max: usize) -> bool {
    !scope.is_empty()
        && scope.len() <= max
        && scope.len() <= 96
        && scope.bytes().enumerate().all(|(i, b)| {
            if i == 0 {
                b.is_ascii_lowercase()
            } else {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)
            }
        })
}
fn hash<T: Serialize>(value: &T) -> String {
    canonical_json_hash(&serde_json::to_value(value).expect("benchmark DTO serializes"))
}
fn add(left: &mut u64, right: u64) -> Result<(), BenchmarkError> {
    *left = left
        .checked_add(right)
        .ok_or(BenchmarkError::new("accounting.overflow"))?;
    Ok(())
}
fn accumulate(into: &mut Totals, part: &Totals) -> Result<(), BenchmarkError> {
    add(&mut into.uncached_input, part.uncached_input)?;
    add(&mut into.cached_input, part.cached_input)?;
    add(&mut into.output, part.output)?;
    add(&mut into.reasoning, part.reasoning)?;
    add(&mut into.billed_output, part.billed_output)?;
    add(&mut into.billed_total, part.billed_total)?;
    add(&mut into.retry_count, part.retry_count)?;
    add(&mut into.tool_call_count, part.tool_call_count)?;
    add(&mut into.repeated_read_count, part.repeated_read_count)?;
    add(&mut into.duration_ms, part.duration_ms)?;
    into.reasoning_complete &= part.reasoning_complete;
    Ok(())
}
fn check_cohort(cohort: &Cohort) -> Result<(), BenchmarkError> {
    if ![
        &cohort.source_projection,
        &cohort.product_revision,
        &cohort.policy,
        &cohort.fixture,
        &cohort.oracle,
        &cohort.model,
        &cohort.agent_configuration,
        &cohort.catalog,
        &cohort.presentation,
    ]
    .iter()
    .all(|v| identity(v))
    {
        return Err(BenchmarkError::new("input.identity"));
    }
    let valid = match &cohort.accounting {
        Accounting::ProviderUsage { usage_source } => identity(usage_source),
        Accounting::ExactTokenizer {
            implementation_revision,
            encoding,
        } => identity(implementation_revision) && identity(encoding),
    };
    if !valid {
        return Err(BenchmarkError::new("input.identity"));
    }
    Ok(())
}
fn cost(call: &Call, method: &Accounting) -> Result<Option<Totals>, BenchmarkError> {
    match method {
        Accounting::ProviderUsage { .. } if call.content_identity.is_some() => {
            return Err(BenchmarkError::new("accounting.invalid"))
        }
        Accounting::ExactTokenizer { .. }
            if !call.content_identity.as_deref().is_some_and(identity) =>
        {
            return Err(BenchmarkError::new("accounting.invalid"))
        }
        _ => {}
    }
    let Some(usage) = &call.usage else {
        return Ok(None);
    };
    if usage.reasoning_inclusion == ReasoningInclusion::IncludedInOutput
        && usage.reasoning.is_some_and(|count| count > usage.output)
    {
        return Err(BenchmarkError::new("accounting.invalid"));
    }
    let (billed_output, complete_cost) = match (usage.reasoning_inclusion, usage.reasoning) {
        (ReasoningInclusion::IncludedInOutput, _) => (usage.output, true),
        (ReasoningInclusion::AdditionalToOutput, Some(reasoning)) => (
            usage
                .output
                .checked_add(reasoning)
                .ok_or(BenchmarkError::new("accounting.overflow"))?,
            true,
        ),
        (ReasoningInclusion::AdditionalToOutput, None) => (usage.output, false),
    };
    let billed_total = usage
        .uncached_input
        .checked_add(usage.cached_input)
        .and_then(|n| n.checked_add(billed_output))
        .ok_or(BenchmarkError::new("accounting.overflow"))?;
    Ok(Some(Totals {
        uncached_input: usage.uncached_input,
        cached_input: usage.cached_input,
        output: usage.output,
        reasoning: usage.reasoning.unwrap_or(0),
        reasoning_complete: usage.reasoning.is_some(),
        billed_output,
        billed_total,
        complete_cost,
        ..Totals::default()
    }))
}
fn run_report<'a>(
    run: &'a Run,
    policy: &BenchmarkPolicy,
    max_calls: usize,
    call_ids: &mut BTreeSet<&'a str>,
    convention: &mut Option<ReasoningInclusion>,
) -> Result<RunReport, BenchmarkError> {
    if !identity(&run.run_id) {
        return Err(BenchmarkError::new("input.identity"));
    }
    check_cohort(&run.cohort)?;
    let trace = &run.trace;
    if !trace.complete || trace.declared_calls != trace.calls.len() || trace.calls.len() > max_calls
    {
        return Err(BenchmarkError::new("trace.incomplete"));
    }
    if trace.retry_count > policy.bounds.max_event_count
        || trace.tool_call_count > policy.bounds.max_event_count
        || trace.repeated_read_count > policy.bounds.max_event_count
        || trace.duration_ms > policy.bounds.max_duration_ms
        || trace.retry_count > trace.declared_calls as u64
    {
        return Err(BenchmarkError::new("input.bounds"));
    }
    let mut totals = Totals {
        retry_count: trace.retry_count,
        tool_call_count: trace.tool_call_count,
        repeated_read_count: trace.repeated_read_count,
        duration_ms: trace.duration_ms,
        complete_cost: true,
        reasoning_complete: true,
        ..Totals::default()
    };
    for (sequence, call) in trace.calls.iter().enumerate() {
        if !identity(&call.call_id) {
            return Err(BenchmarkError::new("input.identity"));
        }
        if !call_ids.insert(call.call_id.as_str()) {
            return Err(BenchmarkError::new("trace.duplicate_call"));
        }
        if call.sequence != sequence {
            return Err(BenchmarkError::new("trace.incomplete"));
        }
        if let Some(usage) = &call.usage {
            if [usage.uncached_input, usage.cached_input, usage.output]
                .into_iter()
                .chain(usage.reasoning)
                .any(|count| count > policy.bounds.max_token_count)
            {
                return Err(BenchmarkError::new("input.bounds"));
            }
            if convention.is_some_and(|existing| existing != usage.reasoning_inclusion) {
                return Err(BenchmarkError::new("accounting.invalid"));
            }
            *convention = Some(usage.reasoning_inclusion);
        }
        match cost(call, &run.cohort.accounting)? {
            Some(part) => {
                totals.complete_cost &= part.complete_cost;
                accumulate(&mut totals, &part)?;
            }
            None => {
                totals.complete_cost = false;
                totals.reasoning_complete = false;
            }
        }
    }
    let outcome = match (run.oracle.verdict, run.oracle.agent_claimed_completion) {
        (OracleVerdict::Completed, _) => Outcome::Completed,
        (OracleVerdict::CorrectlyBlocked, false) => Outcome::CorrectlyBlocked,
        (OracleVerdict::CorrectlyBlocked, true) | (OracleVerdict::Rejected, true) => {
            Outcome::FalseCompletion
        }
        (OracleVerdict::Rejected, false) => Outcome::Failed,
        (OracleVerdict::Inconclusive, _) => Outcome::Incomplete,
    };
    Ok(RunReport {
        run_id: run.run_id.clone(),
        outcome,
        cost_available: totals.complete_cost,
        totals,
    })
}

/// Admit a complete declared observation set and summarize all attempted costs.
pub fn validate(
    policy: &BenchmarkPolicy,
    input: &BenchmarkInput,
) -> Result<BenchmarkReport, BenchmarkError> {
    policy.check()?;
    if input.schema != INPUT_SCHEMA {
        return Err(BenchmarkError::new("input.schema"));
    }
    let scenario = &input.scenario;
    if !bounded_scope(&scenario.scope, policy.bounds.max_scope_bytes)
        || scenario.repetitions == 0
        || scenario.repetitions > policy.bounds.max_repetitions
        || scenario.max_calls_per_run == 0
        || scenario.max_calls_per_run > policy.bounds.max_calls_per_run
        || input.runs.len() != scenario.repetitions
    {
        return Err(BenchmarkError::new("input.bounds"));
    }
    let cohort = &input.runs[0].cohort;
    check_cohort(cohort)?;
    let mut run_ids = BTreeSet::new();
    let mut call_ids = BTreeSet::new();
    let mut runs = Vec::with_capacity(input.runs.len());
    let mut totals = Totals {
        complete_cost: true,
        reasoning_complete: true,
        ..Totals::default()
    };
    let mut outcomes = OutcomeCounts::default();
    let mut convention = None;
    for run in &input.runs {
        if &run.cohort != cohort || !run_ids.insert(run.run_id.as_str()) {
            return Err(BenchmarkError::new("input.identity"));
        }
        let report = run_report(
            run,
            policy,
            scenario.max_calls_per_run,
            &mut call_ids,
            &mut convention,
        )?;
        match report.outcome {
            Outcome::Completed => outcomes.completed += 1,
            Outcome::CorrectlyBlocked => outcomes.correctly_blocked += 1,
            Outcome::FalseCompletion => outcomes.false_completion += 1,
            Outcome::Failed => outcomes.failed += 1,
            Outcome::Incomplete => outcomes.incomplete += 1,
        }
        accumulate(&mut totals, &report.totals)?;
        totals.complete_cost &= report.cost_available;
        runs.push(report);
    }
    let input_value =
        serde_json::to_value(input).map_err(|_| BenchmarkError::new("input.schema"))?;
    let canonical_input = canonical_json(&input_value);
    if canonical_input.len() > policy.bounds.max_input_bytes {
        return Err(BenchmarkError::new("input.bounds"));
    }
    let tokens_per_correct_completion =
        (totals.complete_cost && outcomes.completed > 0).then_some(CostPerCompletion {
            attempted_tokens: totals.billed_total,
            correct_completions: outcomes.completed,
        });
    let tokens_per_correct_block = (totals.complete_cost && outcomes.correctly_blocked > 0)
        .then_some(CostPerBlock {
            attempted_tokens: totals.billed_total,
            correct_blocks: outcomes.correctly_blocked,
        });
    Ok(BenchmarkReport {
        schema: REPORT_SCHEMA.into(),
        input_hash: blake3_hex(canonical_input.as_bytes()),
        policy_hash: hash(policy),
        scope: scenario.scope.clone(),
        mode: scenario.mode,
        repetitions: scenario.repetitions,
        cohort_hash: hash(cohort),
        product_revision: cohort.product_revision.clone(),
        attempted_runs: runs.len(),
        outcomes,
        totals,
        runs,
        tokens_per_correct_completion,
        tokens_per_correct_block,
        non_claims: NON_CLAIMS.iter().map(|s| (*s).into()).collect(),
    })
}

/// Compare only declared candidate differences; never treat a replay as live evidence.
pub fn compare(
    policy: &BenchmarkPolicy,
    input: &ComparisonInput,
) -> Result<ComparisonReport, BenchmarkError> {
    if input.schema != COMPARISON_SCHEMA {
        return Err(BenchmarkError::new("input.schema"));
    }
    let allowed: BTreeSet<_> = input
        .allowed_candidate_differences
        .iter()
        .copied()
        .collect();
    if allowed.len() != input.allowed_candidate_differences.len()
        || !allowed.contains(&AllowedDifference::ProductRevision)
    {
        return Err(BenchmarkError::new("comparison.invalid"));
    }
    let baseline = validate(policy, &input.baseline)?;
    let candidate = validate(policy, &input.candidate)?;
    let a = &input.baseline.runs[0].cohort;
    let b = &input.candidate.runs[0].cohort;
    let mut drift = Vec::new();
    let mut check = |name: &'static str, changed: bool, difference: Option<AllowedDifference>| {
        if changed && difference.is_none_or(|d| !allowed.contains(&d)) {
            drift.push(name.to_owned());
        }
    };
    check(
        "source_projection",
        a.source_projection != b.source_projection,
        Some(AllowedDifference::SourceProjection),
    );
    check(
        "product_revision",
        a.product_revision != b.product_revision,
        Some(AllowedDifference::ProductRevision),
    );
    check("policy", a.policy != b.policy, None);
    check("fixture", a.fixture != b.fixture, None);
    check("oracle", a.oracle != b.oracle, None);
    check("model", a.model != b.model, None);
    check(
        "agent_configuration",
        a.agent_configuration != b.agent_configuration,
        Some(AllowedDifference::AgentConfiguration),
    );
    check(
        "catalog",
        a.catalog != b.catalog,
        Some(AllowedDifference::Catalog),
    );
    check(
        "presentation",
        a.presentation != b.presentation,
        Some(AllowedDifference::Presentation),
    );
    check("accounting", a.accounting != b.accounting, None);
    check("scope", baseline.scope != candidate.scope, None);
    check("mode", baseline.mode != candidate.mode, None);
    check(
        "repetitions",
        baseline.repetitions != candidate.repetitions,
        None,
    );
    check(
        "max_calls_per_run",
        input.baseline.scenario.max_calls_per_run != input.candidate.scenario.max_calls_per_run,
        None,
    );
    if a.product_revision == b.product_revision {
        drift.push("unchanged_product_revision".into());
    }
    let comparable = drift.is_empty();
    let mut withheld_reasons = Vec::new();
    if !comparable {
        withheld_reasons.push("incomparable_cohort".into());
    }
    if !baseline.totals.complete_cost || !candidate.totals.complete_cost {
        withheld_reasons.push("unavailable_cost".into());
    }
    if baseline.outcomes.completed == 0 || candidate.outcomes.completed == 0 {
        withheld_reasons.push("no_correct_completion".into());
    }
    let regression = candidate.outcomes.completed < baseline.outcomes.completed
        || candidate.outcomes.correctly_blocked < baseline.outcomes.correctly_blocked
        || candidate.outcomes.false_completion > baseline.outcomes.false_completion
        || candidate.outcomes.failed > baseline.outcomes.failed
        || candidate.outcomes.incomplete > baseline.outcomes.incomplete;
    if regression {
        withheld_reasons.push("correctness_regression".into());
    }
    if baseline.mode == ObservationMode::RepresentationReplay {
        withheld_reasons.push("representation_only".into());
    }
    let scoped_token_change = (comparable
        && !regression
        && baseline.totals.complete_cost
        && candidate.totals.complete_cost
        && baseline.outcomes.completed > 0
        && candidate.outcomes.completed > 0)
        .then_some(ScopedTokenChange {
            baseline_attempted_tokens: baseline.totals.billed_total,
            baseline_correct_completions: baseline.outcomes.completed,
            candidate_attempted_tokens: candidate.totals.billed_total,
            candidate_correct_completions: candidate.outcomes.completed,
        });
    let verdict = if !withheld_reasons.is_empty() {
        Verdict::Withheld
    } else {
        let base =
            u128::from(baseline.totals.billed_total) * u128::from(candidate.outcomes.completed);
        let cand =
            u128::from(candidate.totals.billed_total) * u128::from(baseline.outcomes.completed);
        if cand < base {
            Verdict::Improved
        } else {
            Verdict::NotImproved
        }
    };
    Ok(ComparisonReport {
        schema: COMPARISON_REPORT_SCHEMA.into(),
        baseline,
        candidate,
        comparable,
        drift_fields: drift,
        verdict,
        withheld_reasons,
        scoped_token_change,
        non_claims: NON_CLAIMS.iter().map(|s| (*s).into()).collect(),
    })
}

#[cfg(test)]
mod tests;
