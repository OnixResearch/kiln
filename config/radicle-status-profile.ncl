# r[impl kiln.radicle.status_signer]
# r[impl kiln.radicle.status_allowlist]
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
let RidList = std.contract.from_predicate (fun value =>
  std.is_array value
  && std.array.length value > 0
  && std.array.all (fun rid => std.is_string rid && std.string.is_match "^rad:z[1-9A-HJ-NP-Za-km-z]+$" rid) value
) in
let SecretName = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[A-Za-z0-9_-]+$" value
) in
let NonClaims = [
  "status writes do not complete Kiln attempts or mutate canonical state",
  "status writes do not prove peer convergence or remote visibility",
  "status writes do not prove source trust or actor authority",
  "SecretSpec resolution does not grant merge authority",
  "the signer identity does not prove operator intent beyond one admitted plan",
  "equal ci-job objects do not prove equal run outcomes elsewhere",
  "passing conformance does not prove node availability",
] in
let Cohort = {
  platform | Exact "x86_64-linux",
  radicle_cob_version | Exact "0.21.1",
  radicle_cob_source_revision | Exact "crates.io:sha256:8a6b295464ff130975fc151feb8aef4d6fd22ddd1da2a5a2a561f283c7abd41e",
  radicle_crypto_version | Exact "0.19.0",
  radicle_crypto_source_revision | Exact "crates.io:sha256:76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5",
  radicle_oid_version | Exact "0.2.2",
  radicle_oid_source_revision | Exact "crates.io:sha256:7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c",
  radicle_ref_format_version | Exact "0.2.0",
  radicle_ref_format_source_revision | Exact "crates.io:sha256:8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6",
  git2_version | Exact "0.21.0",
  git2_source_revision | Exact "crates.io:sha256:ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e",
  radicle_license | Exact "MIT OR Apache-2.0",
  full_radicle_runtime | Exact false,
  sqlite_backend | Exact false,
} in
let Signer = {
  project_root | AbsolutePath,
  project | NonEmpty,
  profile | NonEmpty,
  secret_key | SecretName,
  provider | Exact "keyring",
  secretspec_version | Exact "0.19.0",
  secretspec_source_revision | Exact "crates.io:sha256:827137f2e4d608acb1331aaffc91b16e457903d41964939fb8e716c3be557827",
  secretspec_license | Exact "Apache-2.0",
  nixpkgs_revision | Exact "b7c2ada94fe99c15b0dbcf4d11fd7850b957a436",
  executable_path | Exact "/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec",
  executable_blake3 | Exact "c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6",
} in
let Profile = {
  id | Exact "radicle-status-v1",
  cohort | Cohort,
  allowed_rids | RidList,
  allowed_object_type | Exact "xyz.radicle.ci-job",
  single_writer | Exact true,
  signer | Signer,
  limits | {
    max_payload_bytes | Positive,
    max_objects_scanned | Positive,
  },
  allow_network_peers | Exact false,
  allow_merge_authority | Exact false,
  allow_canonical_state_mutation | Exact false,
  non_claims | Exact NonClaims,
} in
let MaxPayloadBytes = 65536 in
let MaxObjectsScanned = 256 in
{
  profile | Profile = {
    id = "radicle-status-v1",
    cohort = {
      platform = "x86_64-linux",
      radicle_cob_version = "0.21.1",
      radicle_cob_source_revision = "crates.io:sha256:8a6b295464ff130975fc151feb8aef4d6fd22ddd1da2a5a2a561f283c7abd41e",
      radicle_crypto_version = "0.19.0",
      radicle_crypto_source_revision = "crates.io:sha256:76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5",
      radicle_oid_version = "0.2.2",
      radicle_oid_source_revision = "crates.io:sha256:7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c",
      radicle_ref_format_version = "0.2.0",
      radicle_ref_format_source_revision = "crates.io:sha256:8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6",
      git2_version = "0.21.0",
      git2_source_revision = "crates.io:sha256:ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e",
      radicle_license = "MIT OR Apache-2.0",
      full_radicle_runtime = false,
      sqlite_backend = false,
    },
    # Conformance fixture only. A deployment must replace this exact RID.
    allowed_rids = ["rad:zFixtureRepository"],
    allowed_object_type = "xyz.radicle.ci-job",
    single_writer = true,
    signer = {
      project_root = "/var/lib/kiln/secrets",
      project = "kiln",
      profile = "production",
      secret_key = "RADICLE_SIGNER_SEED",
      provider = "keyring",
      secretspec_version = "0.19.0",
      secretspec_source_revision = "crates.io:sha256:827137f2e4d608acb1331aaffc91b16e457903d41964939fb8e716c3be557827",
      secretspec_license = "Apache-2.0",
      nixpkgs_revision = "b7c2ada94fe99c15b0dbcf4d11fd7850b957a436",
      executable_path = "/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec",
      executable_blake3 = "c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6",
    },
    limits = {
      max_payload_bytes = MaxPayloadBytes,
      max_objects_scanned = MaxObjectsScanned,
    },
    allow_network_peers = false,
    allow_merge_authority = false,
    allow_canonical_state_mutation = false,
    non_claims = NonClaims,
  },
}