let manifest = import "manifest.ncl" in
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let HexDigest = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Receipt = {
  schema_version | std.contract.Equal "ucan-aeneas-time-bounds-blocked-receipt/v1",
  pilot_id | std.contract.Equal "ucan-core.time-bounds.aeneas-pilot",
  outcome | std.contract.Equal "blocked",
  manifest_source | NonEmptyString,
  manifest_source_blake3 | HexDigest,
  source | {
    path | std.contract.Equal "crates/ucan-core/src/token.rs",
    blake3 | HexDigest,
    item_count | std.contract.Equal 3,
    items | Array NonEmptyString,
  },
  provider | Dyn,
  extraction | Dyn,
  blocker | Dyn,
  budgets | Dyn,
  evidence_admission | Dyn,
  requirements | Array NonEmptyString,
  non_claims | Array NonEmptyString,
} in
let selected_item_count = 3 in
{
  schema_version = "ucan-aeneas-time-bounds-blocked-receipt/v1",
  pilot_id = manifest.pilot_id,
  outcome = manifest.feasibility.outcome,
  manifest_source = "proofs/aeneas-time-bounds/manifest.ncl",
  manifest_source_blake3 = "7d868745e4e511d213e80872e5c01d1867065b0ef1d6d7e4b745997ae033817c",
  source = {
    path = manifest.target.source_path,
    blake3 = manifest.target.source_blake3,
    item_count = selected_item_count,
    items = std.array.map (fun item => item.rust_path) manifest.target.items,
  },
  provider = {
    octet_revision = manifest.provider.octet.revision,
    package = manifest.provider.octet.package,
    profile_id = manifest.provider.octet.profile_id,
    profile_blake3 = manifest.provider.octet.profile_blake3,
    aeneas_revision = manifest.provider.octet.aeneas_revision,
    charon_revision = manifest.provider.octet.charon_revision,
    compatibility_matrix_id = manifest.provider.octet.compatibility_matrix_id,
    compatibility_matrix_blake3 = manifest.provider.octet.compatibility_matrix_blake3,
    valence_revision = manifest.provider.valence.revision,
    valence_profile = manifest.provider.valence.profile,
  },
  extraction = {
    preset = manifest.extraction.preset,
    has_errors = manifest.extraction.has_errors,
    skip_borrowck = manifest.extraction.skip_borrowck,
    borrow_check_preserved = manifest.extraction.borrow_check_preserved,
    target_triple = manifest.extraction.target_triple,
    pointer_width_bits = manifest.extraction.pointer_width_bits,
    endianness = manifest.extraction.endianness,
    exact_command = manifest.extraction.exact_command,
    network_isolation = manifest.extraction.network_isolation,
    translation_manifest_status = manifest.translation_manifest.status,
    def_id_policy = manifest.translation_manifest.def_id_policy,
  },
  blocker = {
    stage = manifest.feasibility.stage,
    diagnostic_class = manifest.feasibility.diagnostic_class,
    unsupported_boundary = manifest.feasibility.unsupported_boundary,
    diagnostic_artifact = manifest.feasibility.diagnostic_artifact,
    diagnostic_blake3 = manifest.feasibility.diagnostic_blake3,
    supplementary_probe_artifact = manifest.feasibility.supplementary_probe_artifact,
    supplementary_probe_blake3 = manifest.feasibility.supplementary_probe_blake3,
    next_discriminating_check = manifest.feasibility.next_discriminating_check,
  },
  budgets = manifest.budgets,
  evidence_admission = {
    theorem_status = manifest.feasibility.theorem_status,
    theorem_map_emitted = manifest.claims.theorem_map_emitted,
    octet_property_receipt_emitted = manifest.claims.octet_property_receipt_emitted,
    octet_artifact_verification = manifest.feasibility.octet_artifact_verification,
    valence_property_handoff_emitted = manifest.claims.valence_property_handoff_emitted,
    valence_chain_validation = manifest.feasibility.valence_chain_validation,
    aeneas_property_proved = manifest.claims.aeneas_property_proved,
  },
  requirements = [
    "ucan.aeneas_time_bounds.provider_dependencies",
    "ucan.aeneas_time_bounds.exact_source",
    "ucan.aeneas_time_bounds.translation_manifest",
    "ucan.aeneas_time_bounds.feasibility",
    "ucan.aeneas_time_bounds.resource_budget",
    "ucan.aeneas_time_bounds.runtime_boundary",
    "ucan.aeneas_time_bounds.claim_boundary",
    "ucan.aeneas_time_bounds.final_validation",
  ],
  non_claims = manifest.claims.non_claims,
} | Receipt
