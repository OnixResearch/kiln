# Human-authored frozen quartet for the semantic merge orchestration fixture.
# `quartet.ncl` applies the contract; `quartet.json` is its deterministic export.
let BaseSource = m%"
pub fn add(left: i64, right: i64) -> i64 {
    left + right
}
"%
in
let VariantASource = m%"
pub fn add(left: i64, right: i64) -> i64 {
    left.wrapping_add(right)
}
"%
in
let VariantBSource = m%"
pub fn add(left: i64, right: i64) -> i64 {
    left + right
}

pub fn sub(left: i64, right: i64) -> i64 {
    left - right
}
"%
in
let CandidateSource = m%"
pub fn add(left: i64, right: i64) -> i64 {
    left.wrapping_add(right)
}

pub fn sub(left: i64, right: i64) -> i64 {
    left - right
}
"%
in
let Manifest = m%"
[package]
name = "fixture-crate"
version = "0.1.0"
edition = "2024"
"%
in
let role_files = fun source =>
  [
    { path = "Cargo.toml", text = Manifest },
    { path = "src/lib.rs", text = source },
  ]
in
let Bounds = {
  timeout_ms = 60000,
  max_memory_bytes = 1073741824,
  stdout_max_bytes = 1048576,
  stderr_max_bytes = 1048576,
  max_products = 4,
}
in
let verdict_case = fun outcome =>
  {
    name = "nonverified-%{outcome}",
    mutation = "verdict",
    verdict = outcome,
    expected_outcome = outcome,
  }
in
let substitute_case = fun kind =>
  {
    name = "substitute-%{kind}",
    mutation = "substitute_evidence",
    evidence_kind = kind,
    expected_code = "semantic_evidence_substitution",
    expected_field = "semantic_merge.result.evidence_kind",
  }
in
{
  schema = "kiln.semantic-merge-fixture.v1",
  repository = "semantic-merge-fixture",
  source_profile = "pijul-source-v1",
  roles = {
    base = {
      channel = "main",
      state = "state-o",
      changes = ["change-base"],
      files = role_files BaseSource,
    },
    variant_a = {
      channel = "feature-wrapping-add",
      state = "state-a",
      changes = ["change-base", "change-a"],
      files = role_files VariantASource,
    },
    variant_b = {
      channel = "feature-sub",
      state = "state-b",
      changes = ["change-base", "change-b"],
      files = role_files VariantBSource,
    },
    candidate = {
      channel = "proposed-merge",
      state = "state-m",
      changes = ["change-base", "change-a", "change-b"],
      files = role_files CandidateSource,
    },
  },
  supply = {
    supplier = "fixture-proposal-supplier",
    proposal = "proposal-1",
  },
  subject = {
    language = "rust",
    unit = "fixture-crate",
    required_coverage = ["crate::add", "crate::sub"],
  },
  property = {
    id = "semantic-conflict-freedom",
    revision = "fixture property revision 1",
    assumptions = ["fixture assumption: integer arithmetic is total"],
  },
  inputs = {
    dependencies = "fixture dependency closure 1",
    model = "fixture semantic model 1",
  },
  checker = {
    producer_profile = "fixture-semantic-checker-v1",
    checker = "fixture checker executable 1",
    toolchain = "fixture toolchain 1",
    semantic_profile = "fixture-rust-merge-profile",
    receipt_profile = "fixture-receipt-v1",
  },
  policy = {
    id = "semantic-merge-fixture",
    mode = "required",
    lineage = {
      require_greatest_common_base = true,
      max_candidate_extra_changes = 0,
    },
    admitted_candidate_suppliers = ["fixture-proposal-supplier"],
    retry = {
      max_attempts = 2,
      retry_tool_error = true,
      retry_exhausted = false,
    },
    bound_ceilings = Bounds,
  },
  bounds = Bounds,
  producer = {
    verdict = "verified",
    coverage = ["crate::add", "crate::sub"],
    output_complete = true,
    receipt = "fixture receipt 1",
    log = "fixture checker log 1",
    products = [{ name = "proof-summary", content = "fixture proof summary 1" }],
  },
  independent_gates = [
    { name = "build", kind = "build", passed = true },
    { name = "lints", kind = "lint", passed = true },
    { name = "review", kind = "review", passed = true },
    { name = "tests", kind = "test", passed = true },
  ],
  negative_cases =
    [
      {
        name = "swapped-roles",
        mutation = "swap_variants",
        expected_code = "semantic_binding_mismatch",
        expected_field = "semantic_merge.binding.variant_a",
      },
      {
        name = "changed-base",
        mutation = "result_for_moved_role",
        role = "base",
        state = "state-o-moved",
        changes = ["change-base"],
        expected_code = "semantic_binding_mismatch",
        expected_field = "semantic_merge.binding.base",
      },
      {
        name = "changed-candidate",
        mutation = "result_for_moved_role",
        role = "candidate",
        state = "state-m-moved",
        changes = ["change-base", "change-b", "change-a"],
        expected_code = "semantic_binding_mismatch",
        expected_field = "semantic_merge.binding.candidate",
      },
      {
        name = "changed-dependencies",
        mutation = "change_dependencies",
        label = "fixture dependency closure 2",
        expected_code = "semantic_binding_mismatch",
        expected_field = "semantic_merge.binding.dependencies",
      },
      {
        name = "changed-policy",
        mutation = "change_policy",
        expected_code = "semantic_binding_mismatch",
        expected_field = "semantic_merge.binding.policy",
      },
      {
        name = "missing-coverage",
        mutation = "missing_coverage",
        expected_code = "semantic_coverage_missing",
        expected_field = "semantic_merge.result.coverage",
      },
      {
        name = "partial-output",
        mutation = "partial_output",
        expected_code = "semantic_partial_output",
        expected_outcome = "tool_error",
      },
      {
        name = "replay-from-another-attempt",
        mutation = "replay_attempt",
        expected_code = "semantic_attempt_replay",
        expected_field = "semantic_merge.attempt",
      },
      {
        name = "cancellation",
        mutation = "cancel_then_result",
        expected_code = "semantic_result_late",
        expected_field = "semantic_merge.attempt",
        expected_outcome = "cancelled",
      },
      {
        name = "concurrent-source-change",
        mutation = "concurrent_source_change",
        role = "variant_b",
        state = "state-b-moved",
        changes = ["change-b", "change-base"],
        expected_code = "semantic_result_stale",
        expected_field = "semantic_merge.binding.variant_b",
        expected_outcome = "stale",
      },
    ]
    @ std.array.map
      verdict_case
      ["counterexample", "unknown", "unsupported", "exhausted_wall_time", "cancelled", "tool_error"]
    @ std.array.map
      substitute_case
      ["clean_text_merge", "equal_source_trees", "passing_tests", "evidence_envelope"],
  non_claims = [
    "fixture producer proves orchestration only, not checker soundness",
    "fixture states are local fixtures, not trusted or complete Pijul history",
    "fixture verdicts grant no merge, publish, status, or release authority",
  ],
}
