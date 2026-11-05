# Explicit patch-only profile. Push profiles and their serialized bytes do not change.
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
let Text = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let Absolute = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/[^\\x00]+$" value) in
let Digest = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value) in
let Executable = { path | Absolute, blake3 | Digest } in
let External = { provider_namespace | Text, native_kind | Text, native_id | Text } in
let JobProfile = {
  id | Exact "radicle-job-v1",
  platform | Exact "x86_64-linux",
  radicle_cohort | Text,
  radicle_cob_version | Text, radicle_cob_source_revision | Text,
  radicle_crypto_version | Text, radicle_crypto_source_revision | Text,
  radicle_oid_version | Text, radicle_oid_source_revision | Text,
  radicle_ref_format_version | Text, radicle_ref_format_source_revision | Text,
  git2_version | Text, git2_source_revision | Text,
  secretspec_version | Text, secretspec_source_revision | Text,
  secretspec_nixpkgs_revision | Text, secretspec_license | Text,
  radicle_license | Text,
  allowed_rids | Array Text,
  allowed_object_type | Exact "xyz.radworks.job",
  signer_identity | External,
  signer | { project_root | Absolute, project | Text, profile | Text, secret_key | Text, provider | Text },
  executable | { path | Absolute, blake3 | Digest, version | Text },
  max_payload_bytes | Positive, max_objects_scanned | Positive,
  allow_network_peers | Exact false,
  allow_merge_authority | Exact false,
  allow_canonical_state_mutation | Exact false,
  non_claims | Array Text,
} in
let Profile = {
  schema | Exact "kiln.radicle-ci.patch-profile.v1",
  repository_id | Text,
  storage_root | Absolute,
  helper | Executable,
  state_root | Absolute,
  snapshot_limits | {
    max_identity_bytes | Positive, max_reference_bytes | Positive,
    max_entries | Positive, max_path_bytes | Positive,
    max_file_bytes | Positive, max_total_bytes | Positive,
  },
  trusted_authors | Array Text,
  approvers | Array Text,
  hosted | {
    executable | Executable,
    client_profile | Absolute, client_profile_identity | Digest,
    organization | Text, project | Text, pipeline_id | Text,
    price_id | Text, class | Text, workspace_root | Absolute,
  },
  job_profile | JobProfile,
  log_url_prefix | std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^https://" value),
} in
fun inputs => inputs | Profile
