let provider_identity = import "provider-identity.ncl" in
let negative_case_fixture = import "negative-cases.ncl" in
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let HexDigest = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let PositiveNumber = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Item = {
  rust_path | NonEmptyString,
  source_begin_line | PositiveNumber,
  source_end_line | PositiveNumber,
  required_markers | Array NonEmptyString,
} in
let Manifest = {
  schema_version | std.contract.Equal "ucan-aeneas-time-bounds-feasibility/v1",
  pilot_id | std.contract.Equal "ucan-core.time-bounds.aeneas-pilot",
  status | std.contract.Equal "blocked",
  provider_identity_source | NonEmptyString,
  provider_identity_source_blake3 | HexDigest,
  provider | Dyn,
  target | {
    source_path | std.contract.Equal "crates/ucan-core/src/token.rs",
    source_blake3 | HexDigest,
    crate_name | std.contract.Equal "ucan_core",
    package_name | std.contract.Equal "ucan-core",
    items | Array Item,
    copied_source_allowed | std.contract.Equal false,
    time_bounds_declaration_begin_line | PositiveNumber,
  },
  extraction | {
    preset | std.contract.Equal "aeneas",
    has_errors | std.contract.Equal false,
    skip_borrowck | std.contract.Equal false,
    borrow_check_preserved | std.contract.Equal true,
    target_triple | std.contract.Equal "x86_64-unknown-linux-gnu",
    pointer_width_bits | std.contract.Equal 64,
    endianness | std.contract.Equal "little",
    serialization_format | std.contract.Equal "llbc-json-v1",
    exact_command | Array NonEmptyString,
    network_isolation | std.contract.Equal "bubblewrap-unshare-net",
  },
  translation_manifest | {
    status | std.contract.Equal "not-produced",
    reason | NonEmptyString,
    def_id_policy | std.contract.Equal "run-local-only-not-durable",
    rows | Array Dyn,
  },
  feasibility | {
    outcome | std.contract.Equal "blocked",
    stage | std.contract.Equal "charon-item-selection",
    diagnostic_class | std.contract.Equal "charon-start-from-inherent-impl-unsupported",
    unsupported_boundary | NonEmptyString,
    diagnostic_artifact | NonEmptyString,
    diagnostic_blake3 | HexDigest,
    supplementary_probe_artifact | NonEmptyString,
    supplementary_probe_blake3 | HexDigest,
    supplementary_llbc_blake3 | HexDigest,
    supplementary_llbc_retained | std.contract.Equal false,
    next_discriminating_check | NonEmptyString,
    theorem_status | std.contract.Equal "not-run",
    octet_artifact_verification | std.contract.Equal "not-applicable-no-theorem-artifacts",
    valence_chain_validation | std.contract.Equal "not-applicable-no-property-chain",
  },
  budgets | {
    identity | NonEmptyString,
    exact_stage_timeout_ms | PositiveNumber,
    broad_probe_timeout_ms | PositiveNumber,
    stdout_max_bytes | PositiveNumber,
    stderr_max_bytes | PositiveNumber,
    retained_artifact_max_bytes | PositiveNumber,
    outcome | std.contract.Equal "within-budget",
  },
  negative_cases | Array Dyn,
  runtime_boundary | {
    ucan_core_dependency_added | std.contract.Equal false,
    proof_tool_runtime_discovery | std.contract.Equal false,
    proof_receipt_runtime_loading | std.contract.Equal false,
  },
  claims | {
    aeneas_property_proved | std.contract.Equal false,
    theorem_map_emitted | std.contract.Equal false,
    octet_property_receipt_emitted | std.contract.Equal false,
    valence_property_handoff_emitted | std.contract.Equal false,
    non_claims | Array NonEmptyString,
  },
} in
let source_begin_time_bounds = 507 in
let source_begin_constructor = 519 in
let source_end_constructor = 530 in
let source_begin_not_before_getter = 532 in
let source_end_not_before_getter = 535 in
let source_begin_expiry_getter = 537 in
let source_end_expiry_getter = 540 in
let target_pointer_width_bits = 64 in
let octet_stage_timeout_ms = 30000 in
let configured_broad_probe_timeout_ms = 300000 in
let bounded_log_bytes = 4096 in
let retained_artifact_mebibytes = 8 in
let bytes_per_kibibyte = 1024 in
let kibibytes_per_mebibyte = 1024 in
let configured_retained_artifact_max_bytes = retained_artifact_mebibytes * bytes_per_kibibyte * kibibytes_per_mebibyte in
{
  schema_version = "ucan-aeneas-time-bounds-feasibility/v1",
  pilot_id = "ucan-core.time-bounds.aeneas-pilot",
  status = "blocked",
  provider_identity_source = "proofs/aeneas-time-bounds/provider-identity.ncl",
  provider_identity_source_blake3 = "8e379498e5219a84c73ab463dfd24e08720194ec1201312a403e83b3186dd266",
  provider = provider_identity,
  target = {
    source_path = "crates/ucan-core/src/token.rs",
    source_blake3 = "001764bbd4036024de07cb4565fe5044d17531d5e1dac51523efedadbecc0022",
    crate_name = "ucan_core",
    package_name = "ucan-core",
    items = [
      {
        rust_path = "ucan_core::token::{TimeBounds}::new",
        source_begin_line = source_begin_constructor,
        source_end_line = source_end_constructor,
        required_markers = ["pub const fn new", "InvalidTimeBounds", "not_before_seconds >= expires_at_seconds"],
      },
      {
        rust_path = "ucan_core::token::{TimeBounds}::not_before_seconds",
        source_begin_line = source_begin_not_before_getter,
        source_end_line = source_end_not_before_getter,
        required_markers = ["pub const fn not_before_seconds", "self.not_before_seconds"],
      },
      {
        rust_path = "ucan_core::token::{TimeBounds}::expires_at_seconds",
        source_begin_line = source_begin_expiry_getter,
        source_end_line = source_end_expiry_getter,
        required_markers = ["pub const fn expires_at_seconds", "self.expires_at_seconds"],
      },
    ],
    copied_source_allowed = false,
    time_bounds_declaration_begin_line = source_begin_time_bounds,
  },
  extraction = {
    preset = "aeneas",
    has_errors = false,
    skip_borrowck = false,
    borrow_check_preserved = true,
    target_triple = "x86_64-unknown-linux-gnu",
    pointer_width_bits = target_pointer_width_bits,
    endianness = "little",
    serialization_format = "llbc-json-v1",
    exact_command = [
      "charon", "cargo", "--preset", "aeneas",
      "--start-from", "{impl ucan_core::token::TimeBounds}::new",
      "--start-from", "{impl ucan_core::token::TimeBounds}::not_before_seconds",
      "--start-from", "{impl ucan_core::token::TimeBounds}::expires_at_seconds",
      "--dest-file", "time-bounds.llbc", "--", "-p", "ucan-core",
    ],
    network_isolation = "bubblewrap-unshare-net",
  },
  translation_manifest = {
    status = "not-produced",
    reason = "the reviewed Charon revision rejects inherent impl methods as exact --start-from roots before selected LLBC can be emitted",
    def_id_policy = "run-local-only-not-durable",
    rows = [],
  },
  feasibility = {
    outcome = "blocked",
    stage = "charon-item-selection",
    diagnostic_class = "charon-start-from-inherent-impl-unsupported",
    unsupported_boundary = "the reviewed Charon revision does not support inherent impl methods as --start-from roots; this is a provider selection boundary, not evidence that const fn, Result, Error, or getters are unsupported",
    diagnostic_artifact = "proofs/aeneas-time-bounds/evidence/exact-item-selection.log",
    diagnostic_blake3 = "88544876111259d653a8ff428dce71284818b61e381b6df67bd3c14549716fed",
    supplementary_probe_artifact = "proofs/aeneas-time-bounds/evidence/broad-public-probe.log",
    supplementary_probe_blake3 = "147c60cd75c962ae12077514cbc7f2392aa23e75443f2fc974391be1052e49f4",
    supplementary_llbc_blake3 = "22887882f43b64f140880fd1cc7b32b5a99b855982d2230f0c06af5b8bbdde6c",
    supplementary_llbc_retained = false,
    next_discriminating_check = "rerun the same exact production methods with a reviewed Charon provider that supports inherent impl --start-from roots, or approve production-source start_from attributes; require a three-item non-errorful manifest before theorem work",
    theorem_status = "not-run",
    octet_artifact_verification = "not-applicable-no-theorem-artifacts",
    valence_chain_validation = "not-applicable-no-property-chain",
  },
  budgets = {
    identity = "ucan-aeneas-feasibility-budget/v1",
    exact_stage_timeout_ms = octet_stage_timeout_ms,
    broad_probe_timeout_ms = configured_broad_probe_timeout_ms,
    stdout_max_bytes = bounded_log_bytes,
    stderr_max_bytes = bounded_log_bytes,
    retained_artifact_max_bytes = configured_retained_artifact_max_bytes,
    outcome = "within-budget",
  },
  negative_cases = negative_case_fixture,
  runtime_boundary = {
    ucan_core_dependency_added = false,
    proof_tool_runtime_discovery = false,
    proof_receipt_runtime_loading = false,
  },
  claims = {
    aeneas_property_proved = false,
    theorem_map_emitted = false,
    octet_property_receipt_emitted = false,
    valence_property_handoff_emitted = false,
    non_claims = [
      "no Aeneas theorem was checked for TimeBounds",
      "the broad public-item LLBC is supplementary and is not exact three-item evidence",
      "the selection blocker does not establish that const fn, Result, the Error enum, struct construction, or getters are unsupported",
      "no token validity, ambient clock correctness, parser, codec, cryptographic, proof-chain, caveat, revocation, replay, authorization, interoperability, whole-program, or whole-system property is proved",
      "BLAKE3 binds observed bytes but does not establish translation or verifier soundness",
    ],
  },
} | Manifest
