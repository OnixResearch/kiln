# Workspace contract matrix contract.

let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Blake3Hex = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 64
) in
let SafeRelativePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0 && !std.string.is_match "^/|(^|/)\\.\\.?(/|$)|\\\\" value
) in
let MatrixSchemaVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "cairn.workspace-contract-matrix.v1"
) in
rec {
  Edge = {
    edge_id | NonEmptyString,
    provider_repo | NonEmptyString,
    consumer_repo | NonEmptyString,
    artifact_family | NonEmptyString,
    schema_id | NonEmptyString,
    artifact_path | SafeRelativePath,
    artifact_identity | Blake3Hex,
    validation_command | NonEmptyString,
    positive_fixture | SafeRelativePath,
    negative_fixture | String | default = "",
    failure_class | String | default = "",
    owner | NonEmptyString,
    non_claims | Array NonEmptyString,
  },

  Matrix = {
    schema_version | MatrixSchemaVersion,
    edges | Array Edge,
    non_claims | Array NonEmptyString,
  },

  Input = {
    policy_hash | String | default = "",
    matrix | Matrix,
  },
}
