let MaximumFixtureCapabilities = 64 in
let MaximumFixtureIdBytes = 128 in
let FixtureSchema = "basalt.ucan-semantics-fixture.v2" in
let ProtocolProfile = "onix-compact-ed25519-v1" in
let RuntimeUcanRevision = "417f74662b9de2de3bdabe93ad5db996e1bd2861" in
let SupportCatalogRevision = "417f74662b9de2de3bdabe93ad5db996e1bd2861" in
let SupportCatalogBlake3 = "fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427" in
let BasaltPolicyBlake3 = "fd17db568eb24222d4ca65fe45f02edb21ef910fad214a392441d62c07ef4b14" in
let RequiredNonClaims = [
  "revocation-backend-freshness",
  "key-resolution-truth",
  "ucan-cryptographic-correctness",
  "consumer-replay-store-durability",
  "application-authorization-completeness",
] in
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let BoundedFixtureId = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumFixtureIdBytes
  && std.string.is_match "^[A-Za-z0-9_.-]+$" value
) in
let Exact = fun expected => std.contract.from_predicate (fun value =>
  std.is_string value && value == expected
) in
let Blake3Hex = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let OneOf = fun choices => std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value choices
) in
let CapabilityGrant = {
  resource | NonEmptyString,
  ability | NonEmptyString,
} in
let DecisionFacts = {
  delegation | OneOf ["accepted", "over-delegated", "rejected"],
  caveat_evaluation | OneOf ["satisfied", "missing-policy", "rejected"],
  proof_traversal | OneOf ["accepted", "stale", "limit-exceeded", "invalid"],
  replay | OneOf ["admitted", "duplicate", "stale", "denied", "backend-error"],
} in
let ReasonClass = OneOf [
  "allowed",
  "policy-denied",
  "ucan-capability-denied",
  "delegation-denied",
  "wildcard-request-denied",
  "caveat-denied",
  "proof-denied",
  "replay-denied",
] in
let Expectation = {
  allowed | Bool,
  reason_class | ReasonClass,
} in
let implied_precondition_reason = fun fixture =>
  if std.string.is_match "(^|/)\\*($|/)" fixture.request_ability then
    "wildcard-request-denied"
  else if fixture.decisions.delegation != "accepted" then
    "delegation-denied"
  else if fixture.decisions.caveat_evaluation != "satisfied" then
    "caveat-denied"
  else if fixture.decisions.proof_traversal != "accepted" then
    "proof-denied"
  else if fixture.decisions.replay != "admitted" then
    "replay-denied"
  else
    null in
let fixture_invariants = fun fixture =>
  let capability_count =
    std.array.length fixture.direct_capabilities
    + std.array.length fixture.effective_delegated_capabilities in
  let precondition_reason = implied_precondition_reason fixture in
  capability_count <= MaximumFixtureCapabilities
  && fixture.non_claims == RequiredNonClaims
  && fixture.expected.allowed == (fixture.expected.reason_class == "allowed")
  && (
    if precondition_reason == null then
      std.array.elem fixture.expected.reason_class [
        "allowed",
        "policy-denied",
        "ucan-capability-denied",
      ]
    else
      !fixture.expected.allowed && fixture.expected.reason_class == precondition_reason
  ) in
let FixtureContract = std.contract.Sequence [{
  schema | Exact FixtureSchema,
  fixture_id | BoundedFixtureId,
  protocol_profile | Exact ProtocolProfile,
  runtime_ucan_revision | Exact RuntimeUcanRevision,
  support_catalog_revision | Exact SupportCatalogRevision,
  support_catalog_blake3 | Blake3Hex | Exact SupportCatalogBlake3,
  basalt_policy_blake3 | Blake3Hex | Exact BasaltPolicyBlake3,
  contract_id | NonEmptyString,
  request_resource | NonEmptyString,
  request_ability | NonEmptyString,
  direct_capabilities | Array CapabilityGrant,
  effective_delegated_capabilities | Array CapabilityGrant,
  decisions | DecisionFacts,
  expected | Expectation,
  non_claims | Array String,
}, std.contract.from_predicate fixture_invariants] in
{
  Fixture = FixtureContract,
  identity = {
    fixture_schema = FixtureSchema,
    protocol_profile = ProtocolProfile,
    runtime_ucan_revision = RuntimeUcanRevision,
    support_catalog_revision = SupportCatalogRevision,
    support_catalog_blake3 = SupportCatalogBlake3,
    basalt_policy_blake3 = BasaltPolicyBlake3,
  },
  required_non_claims = RequiredNonClaims,
}
