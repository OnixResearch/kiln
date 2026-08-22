# r[impl kiln.radicle.shell_profile] r[impl kiln.radicle.shell_nonclaims]
let SourceContract = import "../contracts/radicle-source-contract.ncl" in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let AbsolutePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/[^\\x00]+$" value
) in
let NonClaims = [
  "local source admission does not prove signer or repository trust",
  "an expected signed-refs commit does not prove global freshness",
  "local acquisition does not prove complete history or peer convergence",
  "local acquisition does not prove remote seed availability",
  "equal canonical trees do not prove equal history or authority",
  "source capability does not grant network, credential, signing, mutation, or collaboration authority",
  "an absolute repository path does not prevent filesystem substitution",
  "source receipt admission does not prove build correctness",
  "source receipt admission does not establish release readiness",
] in
let Cohort = {
  protocol_version | Exact "0.25.1",
  protocol_source_sha256 | Exact "fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4",
  radicle_core_version | Exact "0.4.1",
  radicle_core_source_sha256 | Exact "077e01c4da810fed233fa67ce87b94fe0dd6ffa03b6640e3b71caa9769a5ec9f",
  radicle_crypto_version | Exact "0.19.0",
  radicle_crypto_source_sha256 | Exact "76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5",
  radicle_oid_version | Exact "0.2.2",
  radicle_oid_source_sha256 | Exact "7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c",
  radicle_ref_format_version | Exact "0.2.0",
  radicle_ref_format_source_sha256 | Exact "8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6",
  git2_version | Exact "0.21.0",
  git2_source_sha256 | Exact "ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e",
  license | Exact "MIT OR Apache-2.0",
  full_radicle_runtime | Exact false,
  sqlite_backend | Exact false,
} in
let SourceOnly = std.contract.from_predicate (fun value => value == {
  source = true,
  trigger = false,
  status = false,
  collaboration = false,
}) in
let Profile = {
  schema | Exact "kiln.radicle-source-profile.v1",
  contract_revision | Exact "0a5924dd2092243a0fe5df94d43008f0402e46f6",
  repository_path | AbsolutePath,
  capabilities | SourceOnly,
  request | SourceContract.request,
  cohort | Cohort,
  allow_network | Exact false,
  allow_credentials | Exact false,
  allow_signing | Exact false,
  allow_mutation | Exact false,
  allow_collaboration | Exact false,
  non_claims | Exact NonClaims,
} in
let MaxIdentityBytes = 256 in
let MaxReferenceBytes = 256 in
let MaxEntries = 4096 in
let MaxPathBytes = 4096 in
let MaxFileBytes = 1048576 in
let MaxTotalBytes = 16777216 in
{
  profile | Profile = {
    schema = "kiln.radicle-source-profile.v1",
    contract_revision = "0a5924dd2092243a0fe5df94d43008f0402e46f6",
    repository_path = "/var/lib/radicle/storage/rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5.git",
    capabilities = {
      source = true,
      trigger = false,
      status = false,
      collaboration = false,
    },
    request = {
      schema_version = 1,
      profile_id = "radicle-source-v1",
      repository_id = "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5",
      signer_node_id = "z6MkFixtureSigner",
      signed_reference = "refs/heads/main",
      expected_sigrefs_commit = "1111111111111111111111111111111111111111",
      expected_revision = "2222222222222222222222222222222222222222",
      limits = {
        max_identity_bytes = MaxIdentityBytes,
        max_reference_bytes = MaxReferenceBytes,
        max_entries = MaxEntries,
        max_path_bytes = MaxPathBytes,
        max_file_bytes = MaxFileBytes,
        max_total_bytes = MaxTotalBytes,
      },
    },
    cohort = {
      protocol_version = "0.25.1",
      protocol_source_sha256 = "fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4",
      radicle_core_version = "0.4.1",
      radicle_core_source_sha256 = "077e01c4da810fed233fa67ce87b94fe0dd6ffa03b6640e3b71caa9769a5ec9f",
      radicle_crypto_version = "0.19.0",
      radicle_crypto_source_sha256 = "76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5",
      radicle_oid_version = "0.2.2",
      radicle_oid_source_sha256 = "7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c",
      radicle_ref_format_version = "0.2.0",
      radicle_ref_format_source_sha256 = "8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6",
      git2_version = "0.21.0",
      git2_source_sha256 = "ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e",
      license = "MIT OR Apache-2.0",
      full_radicle_runtime = false,
      sqlite_backend = false,
    },
    allow_network = false,
    allow_credentials = false,
    allow_signing = false,
    allow_mutation = false,
    allow_collaboration = false,
    non_claims = NonClaims,
  },
}
