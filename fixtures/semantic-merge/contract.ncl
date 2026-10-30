# Typed contract for the human-authored semantic merge orchestration fixture.
# Identity labels are hashed with BLAKE3 by the consuming test; they are not identities.
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Text =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let Sorted = fun allow_empty =>
  std.contract.from_predicate (fun values =>
    std.is_array values
    && (allow_empty || std.array.length values > 0)
    && std.array.all
      (fun index =>
        std.string.compare (std.array.at index values) (std.array.at (index + 1) values) == 'Lesser
      )
      (
        if std.array.length values < 2 then
          []
        else
          std.array.range 0 (std.array.length values - 1)
      )
  )
in
let Unique =
  std.contract.from_predicate (fun values =>
    std.is_array values
    && std.array.length values == std.record.length (
      std.array.fold_left (fun acc value => std.record.update value true acc) {} values
    )
  )
in
let MaxAttempts = 8 in
let MaxExtraChanges = 64 in
let MaxProducts = 64 in
let AttemptBound =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= MaxAttempts
  )
in
let ExtraBound =
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 0 && value <= MaxExtraChanges
  )
in
let ProductBound =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= MaxProducts
  )
in
let Verdict =
  std.contract.from_predicate (fun value =>
    std.array.elem
      value
      [
        "verified",
        "counterexample",
        "unknown",
        "unsupported",
        "exhausted_wall_time",
        "exhausted_memory",
        "cancelled",
        "tool_error"
      ]
  )
in
let File = { path | Text, text | String } in
let Role = {
  channel | Text,
  state | Text,
  changes | Array Text | Unique,
  files | Array File,
}
in
let Bounds = {
  timeout_ms | Positive,
  max_memory_bytes | Positive,
  stdout_max_bytes | Positive,
  stderr_max_bytes | Positive,
  max_products | ProductBound,
}
in
let Mutation =
  std.contract.from_predicate (fun value =>
    std.array.elem
      value
      [
        "swap_variants",
        "result_for_moved_role",
        "change_dependencies",
        "change_policy",
        "missing_coverage",
        "verdict",
        "partial_output",
        "replay_attempt",
        "cancel_then_result",
        "concurrent_source_change",
        "substitute_evidence"
      ]
  )
in
let RoleName =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["base", "variant_a", "variant_b", "candidate"]
  )
in
let EvidenceKind =
  std.contract.from_predicate (fun value =>
    std.array.elem
      value
      ["clean_text_merge", "equal_source_trees", "passing_tests", "evidence_envelope"]
  )
in
let NegativeCase = {
  name | Text,
  mutation | Mutation,
  role | RoleName | optional,
  state | Text | optional,
  changes | Array Text | Unique | optional,
  label | Text | optional,
  verdict | Verdict | optional,
  evidence_kind | EvidenceKind | optional,
  expected_code | Text | optional,
  expected_field | Text | optional,
  expected_outcome | Text | optional,
}
in
{
  Fixture = {
    schema | Exact "kiln.semantic-merge-fixture.v1",
    repository | Text,
    source_profile | Exact "pijul-source-v1",
    roles
      | {
        base | Role,
        variant_a | Role,
        variant_b | Role,
        candidate | Role,
      },
    supply | { supplier | Text, proposal | Text },
    subject
      | {
        language | Text,
        unit | Text,
        required_coverage | Array Text | Sorted false,
      },
    property
      | {
        id | Text,
        revision | Text,
        assumptions | Array Text | Unique,
      },
    inputs | { dependencies | Text, model | Text },
    checker
      | {
        producer_profile | Text,
        checker | Text,
        toolchain | Text,
        semantic_profile | Text,
        receipt_profile | Text,
      },
    policy
      | {
        id | Text,
        mode | std.enum.TagOrString | [| 'advisory, 'required |],
        lineage
          | {
            require_greatest_common_base | Bool,
            max_candidate_extra_changes | ExtraBound,
          },
        admitted_candidate_suppliers | Array Text | Sorted false,
        retry
          | {
            max_attempts | AttemptBound,
            retry_tool_error | Bool,
            retry_exhausted | Bool,
          },
        bound_ceilings | Bounds,
      },
    bounds | Bounds,
    producer
      | {
        verdict | Verdict,
        coverage | Array Text | Sorted true,
        output_complete | Bool,
        receipt | Text,
        log | Text,
        products | Array { name | Text, content | Text },
      },
    independent_gates
      | Array {
        name | Text,
        kind | std.enum.TagOrString | [| 'build, 'test, 'lint, 'review |],
        passed | Bool,
      },
    negative_cases | Array NegativeCase,
    non_claims | Array Text,
  },
}
