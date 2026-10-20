let Vocabulary = import "generated/support-vocabulary.ncl" in
let MaximumProfileItems = 64 in
let MaximumFixtureItems = 32 in
let StableId = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= 128
  && std.string.is_match "^[A-Za-z0-9_.:/-]+$" value
) in
let SafeRelativePath = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9._/-]*$" value
  && !std.string.is_match "(^|/)\\.\\.(/|$)" value
) in
let Blake3Hex = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let ProfileSchema = std.contract.from_predicate (fun value =>
  std.is_string value && value == "ucan.integration-support-profile.v1"
) in
let FixtureSchema = std.contract.from_predicate (fun value =>
  std.is_string value && value == "ucan.integration-fixture-manifest.v1"
) in
let ResultClass = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["allow", "deny", "error"]
) in
let CaseKind = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["profile", "manifest"]
) in
let ExpectedOutcome = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["accept", "reject"]
) in
let ExpectedStage = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["nickel", "admission", "fixture"]
) in
let Exact = fun expected => std.contract.from_predicate (fun value =>
  std.is_string value && value == expected
) in
let PositiveInteger = std.contract.from_predicate (fun value =>
  std.is_number value && std.number.is_integer value && value > 0
) in
let unique_strings = fun values =>
  std.array.length values == std.array.length (std.array.sort_dedup std.string.compare values) in
let all_in = fun allowed values =>
  std.array.all (fun value => std.array.elem value allowed) values in
let non_empty_unique = fun values =>
  std.is_array values
  && std.array.length values > 0
  && std.array.length values <= MaximumProfileItems
  && std.array.all std.is_string values
  && unique_strings values in
let optional_unique = fun values =>
  std.is_array values
  && std.array.length values <= MaximumProfileItems
  && std.array.all std.is_string values
  && unique_strings values in
let SurfaceIds = std.array.map (fun item => item.id) Vocabulary.surfaces in
let DidMethodIds = std.array.map (fun item => item.id) Vocabulary.did_methods in
let SigningAlgorithmIds = std.array.map (fun item => item.id) Vocabulary.signing_algorithms in
let KeyClassIds = std.array.map (fun item => item.id) Vocabulary.key_classes in
let VerificationLimits = {
  id | StableId,
  max_proof_depth | PositiveInteger,
  max_proof_count | PositiveInteger,
  max_input_bytes | PositiveInteger,
} in
let IntegrationModes = {
  key_resolution | Exact "caller-supplied",
  proof_loading | Exact "caller-supplied",
  revocation | Exact "caller-supplied",
  replay | Exact "caller-supplied",
  verification_time | Exact "explicit",
  caveat_policy | Exact "caller-supplied",
  signer_custody | Exact "caller-owned",
  backend_failure | Exact "fail-closed",
} in
let profile_invariants = fun profile =>
  non_empty_unique profile.required_surfaces
  && optional_unique profile.admitted_preview_surfaces
  && non_empty_unique profile.did_methods
  && non_empty_unique profile.signing_algorithms
  && non_empty_unique profile.key_classes
  && non_empty_unique profile.non_claims
  && all_in SurfaceIds profile.required_surfaces
  && all_in SurfaceIds profile.admitted_preview_surfaces
  && all_in DidMethodIds profile.did_methods
  && all_in SigningAlgorithmIds profile.signing_algorithms
  && all_in KeyClassIds profile.key_classes
  && std.array.all (fun item => !std.array.elem item profile.admitted_preview_surfaces) (
    std.array.filter (fun item => !std.array.elem item profile.required_surfaces) profile.admitted_preview_surfaces
  )
  && profile.verification_limits.max_proof_depth <= Vocabulary.verification_limit_ceiling.max_proof_depth
  && profile.verification_limits.max_proof_count <= Vocabulary.verification_limit_ceiling.max_proof_count
  && profile.verification_limits.max_input_bytes <= Vocabulary.verification_limit_ceiling.max_input_bytes in
let IntegrationProfileContract = std.contract.Sequence [{
  schema | ProfileSchema,
  id | StableId,
  catalog_blake3 | Blake3Hex,
  required_surfaces | Array String,
  admitted_preview_surfaces | Array String,
  did_methods | Array String,
  signing_algorithms | Array String,
  key_classes | Array String,
  verification_limits | VerificationLimits,
  integrations | IntegrationModes,
  fail_closed | Bool,
  non_claims | Array String,
  protocol_profile | optional | StableId,
}, std.contract.from_predicate profile_invariants] in
let fixture_invariants = fun fixture =>
  optional_unique fixture.required_surfaces
  && non_empty_unique fixture.non_claims
  && all_in SurfaceIds fixture.required_surfaces in
let FixtureManifestContract = std.contract.Sequence [{
  schema | FixtureSchema,
  id | StableId,
  profile_id | StableId,
  public_input_blake3 | Blake3Hex,
  public_reference_blake3 | Blake3Hex,
  expected_result | ResultClass,
  required_surfaces | Array String,
  non_claims | Array String,
}, std.contract.from_predicate fixture_invariants] in
let FixtureCase = {
  id | StableId,
  path | SafeRelativePath,
  profile_path | String,
  kind | CaseKind,
  expected | ExpectedOutcome,
  expected_stage | ExpectedStage,
  expected_code | StableId,
} in
let FixtureSuiteContract = std.contract.Sequence [
  std.contract.from_predicate (fun values =>
    std.is_array values
    && std.array.length values > 0
    && std.array.length values <= MaximumFixtureItems
  ),
  Array FixtureCase,
  std.contract.from_predicate (fun values => unique_strings (std.array.map (fun item => item.id) values)),
] in
{
  IntegrationProfile = IntegrationProfileContract,
  FixtureManifest = FixtureManifestContract,
  FixtureSuite = FixtureSuiteContract,
}
