let MaximumFieldBytes = 4096 in
let MaximumStableIdBytes = 128 in
let MaximumRevisionBytes = 128 in
let ReceiptSchema = "basalt.license-consumption-receipt.v1" in
let FixtureSchema = "basalt.license-consumption-fixture.v1" in
let RequiredLicenseExpression = "AGPL-3.0-or-later" in
let RequiredArtifactIdentity = "basalt" in
let RequiredLicenseMetadataRevision = "2026-07-31-license-metadata-v1" in
let RequiredNonClaims = [
  "legal-advice",
  "license-exception",
  "compatibility-promise",
  "alternate-license-grant",
] in
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumFieldBytes
) in
let StableId = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumStableIdBytes
) in
let BoundedRevision = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumRevisionBytes
) in
let Exact = fun expected => std.contract.from_predicate (fun value =>
  std.is_string value && value == expected
) in
let OneOf = fun choices => std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value choices
) in
let ReviewAction = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["acknowledged", "deferred"]
) in
# Nickel enforces receipt SHAPE only. The Rust core enforces the semantic
# invariants (artifact identity, license expression, metadata revision,
# acknowledged review, and the complete non-claim boundary) so that negative
# fixtures can still export and exercise fail-closed diagnostics.
let LicenseConsumptionReceipt = {
  schema | Exact ReceiptSchema,
  receipt_id | StableId,
  consumer_repo | NonEmptyString,
  artifact_identity | NonEmptyString,
  license_expression | NonEmptyString,
  license_metadata_revision | BoundedRevision,
  review_action | ReviewAction,
  non_claims | Array String,
} in
let expected_invariants = fun fixture =>
  fixture.expected.accepted == (fixture.expected.diagnostic == "none") in
let FixtureContract = std.contract.Sequence [{
  schema | Exact FixtureSchema,
  fixture_id | StableId,
  input | LicenseConsumptionReceipt,
  expected | {
    accepted | Bool,
    diagnostic | OneOf [
      "none",
      "missing-review",
      "wrong-artifact-identity",
      "stale-license-metadata",
      "non-claims-weakened",
      "wrong-license-expression",
      "unsupported-schema",
    ],
  },
}, std.contract.from_predicate expected_invariants] in
{
  Receipt = LicenseConsumptionReceipt,
  Fixture = FixtureContract,
  identity = {
    receipt_schema = ReceiptSchema,
    fixture_schema = FixtureSchema,
  },
  required_non_claims = RequiredNonClaims,
  required_license_expression = RequiredLicenseExpression,
  required_artifact_identity = RequiredArtifactIdentity,
  required_license_metadata_revision = RequiredLicenseMetadataRevision,
}
