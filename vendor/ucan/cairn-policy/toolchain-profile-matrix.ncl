# Toolchain profile matrix contract.

let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let MatrixSchemaVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "cairn.toolchain-profile-matrix.v1"
) in
let ProfilePurpose = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "dev" || value == "ci" || value == "release" || value == "proof" || value == "compatibility")
) in
rec {
  Profile = {
    repo | NonEmptyString,
    profile_id | NonEmptyString,
    purpose | ProfilePurpose,
    toolchain_pins | Array NonEmptyString,
    commands | Array NonEmptyString,
    expected_artifacts | Array NonEmptyString,
    non_claims | Array NonEmptyString,
  },

  Matrix = {
    schema_version | MatrixSchemaVersion,
    profiles | Array Profile,
    non_claims | Array NonEmptyString,
  },

  Input = {
    policy_hash | String | default = "",
    matrix | Matrix,
  },
}
