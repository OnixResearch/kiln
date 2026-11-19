let RuntimeRevision = "417f74662b9de2de3bdabe93ad5db996e1bd2861" in
let SupportCatalogBlake3 = "fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427" in
let SourceManifestBlake3 = "7e9a33d3d0ca5ee2a9e85402d9a5be4ebd5da73bd6e3ea7488f7f611a5b4f9e4" in
let WireEvidenceBlake3 = "bdca91b7f15351a1f5b56ce9eb524c467663f8985fd3e05440158609538b7a13" in
let WireSpecBlake3 = "3276ec9cfbfc504ea9be8278bc97869e448548eda410b411b24019c161d2bef3" in
let AuthorizationEvidenceBlake3 = "1aed251777a88ff08b1ca9124b5d5feedd1df395fd1d86c73e67cdf9423e13d2" in
let AuthorizationSpecBlake3 = "706dd6ba6fb05e6def6c46f0c51a8b7937a92dc8baffa4a834c524ad414fb45d" in
let Licenses = [
  "AGPL-3.0-or-later",
  "MPL-2.0",
  "MIT OR Apache-2.0",
] in
let NonClaims = [
  "mapping does not prove key trust",
  "external resource ownership remains caller evidence",
  "mapping does not prove revocation freshness or replay durability",
  "mapping does not prove transport or effect correctness",
  "mapping does not prove release eligibility or whole-system safety",
] in
let Entries = [
  { command = "/document/read", action = "document-read", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/document/write", action = "document-write", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/target/candidate-activation", action = "onixos-target-candidate-activation", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/target/generation-persistence", action = "onixos-target-generation-persistence", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/target/rollback", action = "onixos-target-rollback", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/state-sync/publish", action = "onixos-state-sync-publish", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/state-sync/delete", action = "onixos-state-sync-delete", subject_class = "exact", resource_binding = "external-owner" },
  { command = "/onixos/state-sync/admin", action = "onixos-state-sync-admin", subject_class = "exact", resource_binding = "external-owner" },
] in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let valid_archive = fun evidence spec value =>
  exact_fields ["evidence_blake3", "spec_blake3", "verdict"] value
  && field "evidence_blake3" value == evidence
  && field "spec_blake3" value == spec
  && field "verdict" value == "PASS"
in
let valid_source = fun value =>
  exact_fields [
    "schema",
    "protocol_profile",
    "runtime_revision",
    "support_catalog_blake3",
    "source_manifest_blake3",
    "wire_archive",
    "authorization_archive",
    "profile_supported",
    "conformance_tested",
    "licenses",
  ] value
  && field "schema" value == "basalt.ucan-v1-source-evidence.v1"
  && field "protocol_profile" value == "ucan-wg-1.0.0"
  && field "runtime_revision" value == RuntimeRevision
  && field "support_catalog_blake3" value == SupportCatalogBlake3
  && field "source_manifest_blake3" value == SourceManifestBlake3
  && valid_archive WireEvidenceBlake3 WireSpecBlake3 (field "wire_archive" value)
  && valid_archive AuthorizationEvidenceBlake3 AuthorizationSpecBlake3 (field "authorization_archive" value)
  && field "profile_supported" value == true
  && field "conformance_tested" value == true
  && field "licenses" value == Licenses
in
let valid_mapping = fun value =>
  exact_fields [
    "schema",
    "protocol_profile",
    "runtime_revision",
    "support_catalog_blake3",
    "entries",
    "non_claims",
  ] value
  && field "schema" value == "basalt.ucan-v1-command-mapping.v1"
  && field "protocol_profile" value == "ucan-wg-1.0.0"
  && field "runtime_revision" value == RuntimeRevision
  && field "support_catalog_blake3" value == SupportCatalogBlake3
  && field "entries" value == Entries
  && field "non_claims" value == NonClaims
in
{
  SourceEvidence = std.contract.from_predicate valid_source,
  Mapping = std.contract.from_predicate valid_mapping,
  identity = {
    runtime_revision = RuntimeRevision,
    support_catalog_blake3 = SupportCatalogBlake3,
    source_manifest_blake3 = SourceManifestBlake3,
    wire_evidence_blake3 = WireEvidenceBlake3,
    wire_spec_blake3 = WireSpecBlake3,
    authorization_evidence_blake3 = AuthorizationEvidenceBlake3,
    authorization_spec_blake3 = AuthorizationSpecBlake3,
  },
  licenses = Licenses,
  entries = Entries,
  non_claims = NonClaims,
}
