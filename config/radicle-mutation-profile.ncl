# r[impl kiln.radicle.mutation_scope]
# r[impl kiln.radicle.mutation_bounds]
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let AbsolutePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/[^\\x00]+$" value
) in
let Blake3 = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Oid = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{40}$" value
) in
let Rid = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^rad:z[1-9A-HJ-NP-Za-km-z]+$" value
) in
let NonClaims = [
  "patch review mutation does not complete Kiln attempts or mutate canonical CI state",
  "patch review mutation does not create close archive or merge patches",
  "patch review mutation does not rewrite Git history",
  "local mutation does not prove peer convergence or remote visibility",
  "single-writer admission does not prove actor trust",
  "SecretSpec resolution does not grant ambient collaboration authority",
  "passing conformance does not establish release readiness",
] in
let Target = {
  repository_rid | Rid,
  object_type | Exact "xyz.radicle.patch",
  object_id | Oid,
} in
let Review = {
  revision | Oid,
  verdict | std.contract.from_predicate (fun value => value == "accept" || value == "reject"),
  summary | NonEmpty,
} in
let Operation = {
  payload_identity | Blake3,
  action | Review,
} in
let Signer = {
  project_root | AbsolutePath,
  project | NonEmpty,
  profile | NonEmpty,
  secret_key | NonEmpty,
  provider | Exact "keyring",
  secretspec_version | Exact "0.19.0",
  executable_path | Exact "/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec",
  executable_blake3 | Exact "c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6",
} in
let Profile = {
  id | Exact "radicle-patch-review-v1",
  platform | Exact "x86_64-linux",
  radicle_cohort | Exact "radicle-cob-0.21.1-crypto-0.19.0-oid-0.2.2-ref-format-0.2.0-git2-0.21.0",
  object_type | Exact "xyz.radicle.patch",
  schema_line | Exact "patch-actions-0.25.1",
  targets | std.contract.from_predicate (fun value => std.is_array value && std.array.length value > 0),
  operations | std.contract.from_predicate (fun value => std.is_array value && std.array.length value > 0),
  signer | Signer,
  limits | {
    max_payload_bytes | Positive,
    max_history_entries | Positive,
    max_history_bytes | Positive,
  },
  deadlines | {
    preflight_ms | Positive,
    signer_ms | Positive,
    write_ms | Positive,
  },
  single_writer | Exact true,
  allow_patch_create | Exact false,
  allow_patch_close | Exact false,
  allow_patch_merge | Exact false,
  allow_history_rewrite | Exact false,
  non_claims | Exact NonClaims,
} in
let ObjectId = "2222222222222222222222222222222222222222" in
let RevisionId = "1111111111111111111111111111111111111111" in
let PayloadIdentity = "ab3758f296630f83d4e381c41d023612b2106dce68845786b3959fdb963b1a73" in
let MaxPayloadBytes = 16384 in
let MaxHistoryEntries = 1024 in
let MaxHistoryBytes = 16777216 in
let PreflightDeadlineMs = 5000 in
let SignerDeadlineMs = 15000 in
let WriteDeadlineMs = 15000 in
{
  profile | Profile = {
    id = "radicle-patch-review-v1",
    platform = "x86_64-linux",
    radicle_cohort = "radicle-cob-0.21.1-crypto-0.19.0-oid-0.2.2-ref-format-0.2.0-git2-0.21.0",
    object_type = "xyz.radicle.patch",
    schema_line = "patch-actions-0.25.1",
    # Conformance fixture only. A deployment must replace this target.
    targets | Array Target = [{
      repository_rid = "rad:zMutationFixture",
      object_type = "xyz.radicle.patch",
      object_id = ObjectId,
    }],
    operations | Array Operation = [{
      payload_identity = PayloadIdentity,
      action = {
        revision = RevisionId,
        verdict = "accept",
        summary = "Kiln checks passed",
      },
    }],
    signer = {
      project_root = "/var/lib/kiln/secrets",
      project = "kiln",
      profile = "production",
      secret_key = "RADICLE_SIGNER_SEED",
      provider = "keyring",
      secretspec_version = "0.19.0",
      executable_path = "/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec",
      executable_blake3 = "c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6",
    },
    limits = {
      max_payload_bytes = MaxPayloadBytes,
      max_history_entries = MaxHistoryEntries,
      max_history_bytes = MaxHistoryBytes,
    },
    deadlines = {
      preflight_ms = PreflightDeadlineMs,
      signer_ms = SignerDeadlineMs,
      write_ms = WriteDeadlineMs,
    },
    single_writer = true,
    allow_patch_create = false,
    allow_patch_close = false,
    allow_patch_merge = false,
    allow_history_rewrite = false,
    non_claims = NonClaims,
  },
}