# Versioned producer contract for exact Radicle signed-reference source receipts.

let Sha1HexLength = 40 in
let Blake3HexLength = 64 in
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let NonNegative = std.contract.from_predicate (fun value =>
  std.is_number value && value >= 0
) in
let Sha1 = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.is_match "^[0-9a-f]{%{std.to_string Sha1HexLength}}$" value
) in
let Blake3 = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.is_match "^[0-9a-f]{%{std.to_string Blake3HexLength}}$" value
) in
let Rid = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^rad:z[1-9A-HJ-NP-Za-km-z]+$" value
) in
let Branch = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^refs/heads/[A-Za-z0-9._/-]+$" value
) in
let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else std.contract.check Contract label value
  )
in
let Limits = {
  max_identity_bytes | Positive,
  max_reference_bytes | Positive,
  max_entries | Positive,
  max_path_bytes | Positive,
  max_file_bytes | Positive,
  max_total_bytes | Positive,
} in
let Request = {
  schema_version | std.contract.from_predicate (fun value => value == 1),
  profile_id | NonEmpty,
  repository_id | Rid,
  signer_node_id | NonEmpty,
  signed_reference | Branch,
  expected_sigrefs_commit | Sha1,
  expected_revision | Sha1,
  limits | Limits,
} in
let Observation = {
  schema_version | std.contract.from_predicate (fun value => value == 1),
  profile_id | NonEmpty,
  repository_id | Rid,
  signer_node_id | NonEmpty,
  signed_reference | Branch,
  namespaced_sigrefs_reference | NonEmpty,
  sigrefs_commit_before | Sha1,
  sigrefs_commit_after | Sha1,
  sigrefs_parent | Nullable Sha1,
  identity_root_commit | Sha1,
  signed_revision | Sha1,
  namespaced_revision | Sha1,
  feature_level | std.contract.from_predicate (fun value => value == "parent"),
  signature | std.contract.from_predicate (fun value => value == "verified"),
  repository_identity | std.contract.from_predicate (fun value => value == "verified"),
  namespace_target | std.contract.from_predicate (fun value => value == "verified"),
} in
let Snapshot = {
  schema_version | std.contract.from_predicate (fun value => value == 1),
  revision | Sha1,
  native_tree | Sha1,
  canonical_tree_blake3 | Blake3,
  entry_count | NonNegative,
  total_bytes | NonNegative,
} in
let Receipt = {
  schema_version | std.contract.from_predicate (fun value => value == 1),
  request | Request,
  observation | Observation,
  snapshot | Snapshot,
} in
let LinkedReceipt = std.contract.from_predicate (fun value =>
  value.request.limits.max_file_bytes <= value.request.limits.max_total_bytes
  && value.observation.profile_id == value.request.profile_id
  && value.observation.repository_id == value.request.repository_id
  && value.observation.signer_node_id == value.request.signer_node_id
  && value.observation.signed_reference == value.request.signed_reference
  && value.observation.namespaced_sigrefs_reference
    == "refs/namespaces/%{value.request.signer_node_id}/refs/rad/sigrefs"
  && value.observation.sigrefs_commit_before == value.request.expected_sigrefs_commit
  && value.observation.sigrefs_commit_after == value.request.expected_sigrefs_commit
  && value.observation.signed_revision == value.request.expected_revision
  && value.observation.namespaced_revision == value.request.expected_revision
  && value.snapshot.revision == value.request.expected_revision
  && value.snapshot.entry_count <= value.request.limits.max_entries
  && value.snapshot.total_bytes <= value.request.limits.max_total_bytes
) in
{
  sha1 = Sha1,
  blake3 = Blake3,
  limits = Limits,
  request = Request,
  observation = Observation,
  snapshot = Snapshot,
  receipt = Receipt,
  linked_receipt = LinkedReceipt,
}
