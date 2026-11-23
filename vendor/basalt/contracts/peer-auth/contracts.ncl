let UcanRevision = "fca62a6ed6895689ff18112b497bc7a986516c20" in
let CelldRevision = "c978a2590e875b003ecd860a86cb7551287eba67" in
let CelldSourceBlob = "40d6ac4613d293a148e7222e192153eaeaeb29db" in
let CelldLicense = "Apache-2.0" in
let NonClaims = [
  "transport remains consumer-owned",
  "TLS and key custody remain consumer-owned",
  "clock and replay storage remain consumer-owned",
  "peer authentication does not authenticate an end user",
  "peer authentication does not grant general authorization",
  "the Celld reference is a bounded comparison, not parity or equivalence",
] in
let SteelCases = [
  { id = "admitted", protocol_version = 1, freshness = "within-clock-bound", replay = "fresh", authority_present = true, boundary = "none", expected = "allow", issue = "none" },
  { id = "wrong-version", protocol_version = 2, freshness = "within-clock-bound", replay = "fresh", authority_present = true, boundary = "none", expected = "deny", issue = "peer-policy-version-mismatch" },
  { id = "stale", protocol_version = 1, freshness = "stale", replay = "fresh", authority_present = true, boundary = "none", expected = "deny", issue = "peer-policy-freshness-denied" },
  { id = "replayed", protocol_version = 1, freshness = "within-clock-bound", replay = "replayed", authority_present = true, boundary = "none", expected = "deny", issue = "peer-policy-replay-denied" },
  { id = "missing-authority", protocol_version = 1, freshness = "within-clock-bound", replay = "fresh", authority_present = false, boundary = "none", expected = "deny", issue = "peer-policy-authority-missing" },
  { id = "end-user-promotion", protocol_version = 1, freshness = "within-clock-bound", replay = "fresh", authority_present = true, boundary = "promotes-end-user-authority", expected = "deny", issue = "peer-policy-boundary-promotion" },
] in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let valid_fixture_suite = fun value =>
  exact_fields ["schema", "policy_id", "cases", "non_claims"] value
  && field "schema" value == "basalt.peer-auth-steel-fixtures.v1"
  && field "policy_id" value == "peer-control-v1"
  && field "cases" value == SteelCases
  && field "non_claims" value == NonClaims
in
let valid_policy = fun value =>
  exact_fields [
    "schema",
    "policy_id",
    "protocol_version",
    "required_freshness",
    "required_replay",
    "action",
    "authority",
    "default_decision",
    "ucan_revision",
    "reference",
    "non_claims",
  ] value
  && field "schema" value == "basalt.peer-auth-policy.v1"
  && field "policy_id" value == "peer-control-v1"
  && field "protocol_version" value == 1
  && field "required_freshness" value == "within-clock-bound"
  && field "required_replay" value == "fresh"
  && field "action" value == "dispatch-control-message"
  && field "authority" value == "authenticated-peer-message"
  && field "default_decision" value == "deny"
  && field "ucan_revision" value == UcanRevision
  && exact_fields ["repository", "revision", "source_blob", "license", "claim"] (field "reference" value)
  && field "repository" (field "reference" value) == "https://github.com/devinautotomic226/celld"
  && field "revision" (field "reference" value) == CelldRevision
  && field "source_blob" (field "reference" value) == CelldSourceBlob
  && field "license" (field "reference" value) == CelldLicense
  && field "claim" (field "reference" value) == "bounded-comparison-only"
  && field "non_claims" value == NonClaims
in
{
  Policy = std.contract.from_predicate valid_policy,
  SteelFixtureSuite = std.contract.from_predicate valid_fixture_suite,
  steel_cases = SteelCases,
  identity = {
    ucan_revision = UcanRevision,
    celld_revision = CelldRevision,
    celld_source_blob = CelldSourceBlob,
    celld_license = CelldLicense,
  },
  non_claims = NonClaims,
}
