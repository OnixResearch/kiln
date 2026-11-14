# Human-authored qualification inputs. Export with the pinned Nickel executable.
# Presence and byte identity are checked again by the runner before native effects.
let NonEmpty =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Digest =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Absolute =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/" value
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Natural =
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 0 && value == std.number.truncate value
  )
in
let Seconds =
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= 3600 && value == std.number.truncate value
  )
in
let Frozen = { path | Absolute, blake3 | Digest } in
let Stage =
  std.contract.from_predicate (fun value =>
    std.array.elem
      value
      [
        "drain",
        "cairn-intake",
        "claim",
        "renew",
        "cancel",
        "recover",
        "reserve-plan",
        "start",
        "reconcile",
        "handoff",
        "capture-candidate",
        "verify",
        "prepare-verification-profile",
        "prepare-needs",
        "attach-needs",
        "inspect-work",
        "admission",
        "admission-status",
        "prepare-publication",
        "status",
        "metrics",
        "lattice-dispatch",
        "lattice-prepare",
        "lattice-run",
        "lattice-recover",
        "lattice-cancel",
        "lattice-crash"
      ]
  )
in
let ScenarioName =
  std.contract.from_predicate (fun value =>
    std.array.elem
      value
      [
        "bug",
        "feature",
        "linked-sources",
        "expired-stale-worker",
        "zero-exit-noop",
        "forged-evidence",
        "skipped-checks",
        "weakened-policy",
        "source-policy-movement",
        "target-movement",
        "restart-possible-effects",
        "descendant-cancellation",
        "already-satisfied"
      ]
  )
in
let Observation = { pointer | String, equals | Dyn | optional, nonempty | Bool | default = false } in
let Step = {
  stage | Stage,
  arguments | Array String,
  capture | NonEmpty,
  exit_code | Number,
  timeout_seconds | Seconds,
  stderr_contains | String | optional,
  cancel_after_seconds | Seconds | optional,
  crash_after_seconds | Seconds | optional,
  mutation_probe | { path | Absolute, quiet_seconds | Seconds } | optional,
  input_template | Frozen | optional,
  canonical_profile | Frozen | optional,
  observations | Array Observation | default = [],
  retain | Array { path | Absolute, name | NonEmpty, blake3 | Digest | optional } | default = [],
  database | Absolute | optional,
}
in
let Scenario = {
  name | ScenarioName,
  steps
    | Array Step
    | std.contract.from_predicate (fun value => std.array.length value > 0 && std.array.length value <= 128),
}
in
fun input =>
  input
    | {
      schema | std.contract.from_predicate (fun value => value == "kiln.implementation-qualification.v1"),
      programs
        | {
          coordinator | Frozen,
          b3sum | Frozen,
          verifier | Frozen | optional,
          lattice | Frozen | optional,
          cairn_intake | Frozen | optional,
          lattice_dispatch | Frozen | optional,
          setpriv | Frozen | optional,
          omp | Frozen | optional,
          git | Frozen | optional,
        },
      environment | { _ : String } | default = {},
      cohort | Frozen | optional,
      source_baseline | Frozen | optional,
      policy | Frozen | optional,
      toolchain | Frozen | optional,
      provider | Frozen | optional,
      coordinator_uid | Natural | optional,
      coordinator_gid | Natural | optional,
      worker_uid | Natural | optional,
      worker_gid | Natural | optional,
      verifier_uid | Natural | optional,
      verifier_gid | Natural | optional,
      scenarios | Array Scenario | default = [],
    }
