let RuntimeRevision = "fca62a6ed6895689ff18112b497bc7a986516c20" in
let SupportCatalogBlake3 = "fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427" in
let BasaltPolicyBlake3 = "e929bde8e316da188eedb55013a695e82c280c69b88c15d0668c119589beb233" in
let RequiredNonClaims = [
  "profile matching does not prove token correctness",
  "profile matching does not prove key trust or revocation freshness",
  "profile matching does not prove replay durability or policy completeness",
  "profile matching does not prove transport or effect correctness",
] in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let valid_cohort = fun value =>
  exact_fields [
    "schema",
    "protocol_profile",
    "runtime_revision",
    "support_catalog_blake3",
    "basalt_policy_blake3",
    "compact_adapter",
    "final_adapter",
    "non_claims",
  ] value
  && field "schema" value == "basalt.ucan-protocol-cohort.v1"
  && field "protocol_profile" value == "onix-compact-ed25519-v1"
  && field "runtime_revision" value == RuntimeRevision
  && field "support_catalog_blake3" value == SupportCatalogBlake3
  && field "basalt_policy_blake3" value == BasaltPolicyBlake3
  && field "compact_adapter" value == true
  && field "final_adapter" value == false
  && field "non_claims" value == RequiredNonClaims
in
let valid_final_cohort = fun value =>
  exact_fields [
    "schema",
    "protocol_profile",
    "runtime_revision",
    "support_catalog_blake3",
    "basalt_policy_blake3",
    "compact_adapter",
    "final_adapter",
    "non_claims",
  ] value
  && field "schema" value == "basalt.ucan-protocol-cohort.v1"
  && field "protocol_profile" value == "ucan-wg-1.0.0"
  && field "runtime_revision" value == RuntimeRevision
  && field "support_catalog_blake3" value == SupportCatalogBlake3
  && field "basalt_policy_blake3" value == BasaltPolicyBlake3
  && field "compact_adapter" value == false
  && field "final_adapter" value == true
  && field "non_claims" value == RequiredNonClaims
in
{
  Cohort = std.contract.from_predicate valid_cohort,
  FinalCohort = std.contract.from_predicate valid_final_cohort,
  identity = {
    runtime_revision = RuntimeRevision,
    support_catalog_blake3 = SupportCatalogBlake3,
    basalt_policy_blake3 = BasaltPolicyBlake3,
  },
  required_non_claims = RequiredNonClaims,
}
