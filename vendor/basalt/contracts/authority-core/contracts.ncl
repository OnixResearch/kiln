let MaximumAuthorityItems = 64 in
let MaximumFieldBytes = 4096 in
let MaximumStableIdBytes = 128 in
let InputSchema = "basalt.ucan-authority-input.v1" in
let FixtureSchema = "basalt.ucan-authority-fixture.v1" in
let RequiredNonClaims = [
  "transport-authority",
  "storage-authority",
  "persistence-authority",
  "replay-store-truth",
  "key-resolution-trust",
  "downstream-business-authority",
  "release-eligibility",
  "whole-system-safety",
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
  && std.string.is_match "^[A-Za-z0-9_.-]+$" value
) in
let Exact = fun expected => std.contract.from_predicate (fun value =>
  std.is_string value && value == expected
) in
let OneOf = fun choices => std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value choices
) in
let GrantKind = OneOf ["direct", "delegated"] in
let VerificationDisposition = OneOf ["current", "stale", "rejected"] in
let CaveatDisposition = OneOf ["satisfied", "denied", "missing-policy"] in
let ReplayDisposition = OneOf ["admitted", "denied", "backend-error"] in
let AuthorityClaim = OneOf [
  "exact-request",
  "token-possession",
  "content-reference",
  "transport",
  "storage",
  "persistence",
  "replay-store-truth",
  "key-resolution-trust",
  "downstream-business-authority",
  "release-eligibility",
  "whole-system-safety",
] in
let DenyDiagnostic = OneOf [
  "schema-mismatch",
  "malformed-request",
  "malformed-request-ref",
  "limit-exceeded",
  "non-claims-weakened",
  "missing-policy-ref",
  "multiple-policy-refs",
  "malformed-policy-ref",
  "policy-ref-mismatch",
  "policy-contract-mismatch",
  "authority-overclaim",
  "caveat-denied",
  "replay-denied",
  "missing-verified-grant",
  "malformed-verified-grant",
  "wrong-resource",
  "wrong-ability",
  "missing-verification-receipt",
  "malformed-verification-receipt",
  "verification-grant-mismatch",
  "verification-request-mismatch",
  "stale-verification-ref",
  "rejected-verification-ref",
  "policy-denied",
  "ucan-capability-denied",
  "enforcement-error",
] in
let AuthorityRequest = {
  contract_id | NonEmptyString,
  resource | NonEmptyString,
  ability | NonEmptyString,
  request_ref | String,
} in
let VerifiedGrantRef = {
  grant_ref | String,
  verification_receipt_ref | String,
  resource | NonEmptyString,
  ability | NonEmptyString,
  kind | GrantKind,
} in
let VerificationReceiptRef = {
  receipt_ref | String,
  request_ref | String,
  verified_grant_refs | Array String,
  disposition | VerificationDisposition,
} in
let PolicyRef = {
  artifact_ref | String,
  schema_version | NonEmptyString,
  contract_id | NonEmptyString,
} in
let input_invariants = fun input =>
  std.array.length input.verified_grants <= MaximumAuthorityItems
  && std.array.length input.verification_receipts <= MaximumAuthorityItems
  && std.array.all
    (fun receipt => std.array.length receipt.verified_grant_refs <= MaximumAuthorityItems)
    input.verification_receipts
  && input.non_claims == RequiredNonClaims in
let AuthorityInput = std.contract.Sequence [{
  schema | Exact InputSchema,
  request | AuthorityRequest,
  verified_grants | Array VerifiedGrantRef,
  verification_receipts | Array VerificationReceiptRef,
  policy_refs | Array PolicyRef,
  caveat | CaveatDisposition,
  replay | ReplayDisposition,
  claims | Array AuthorityClaim,
  non_claims | Array String,
}, std.contract.from_predicate input_invariants] in
let expected_invariants = fun fixture =>
  fixture.expected.allowed == (fixture.expected.diagnostic == "none") in
let FixtureContract = std.contract.Sequence [{
  schema | Exact FixtureSchema,
  fixture_id | StableId,
  input | AuthorityInput,
  expected | {
    allowed | Bool,
    diagnostic | OneOf [
      "none",
      "schema-mismatch",
      "malformed-request",
      "malformed-request-ref",
      "limit-exceeded",
      "non-claims-weakened",
      "missing-policy-ref",
      "multiple-policy-refs",
      "malformed-policy-ref",
      "policy-ref-mismatch",
      "policy-contract-mismatch",
      "authority-overclaim",
      "caveat-denied",
      "replay-denied",
      "missing-verified-grant",
      "malformed-verified-grant",
      "wrong-resource",
      "wrong-ability",
      "missing-verification-receipt",
      "malformed-verification-receipt",
      "verification-grant-mismatch",
      "verification-request-mismatch",
      "stale-verification-ref",
      "rejected-verification-ref",
      "policy-denied",
      "ucan-capability-denied",
      "enforcement-error",
    ],
  },
}, std.contract.from_predicate expected_invariants] in
{
  AuthorityInput = AuthorityInput,
  Fixture = FixtureContract,
  identity = {
    input_schema = InputSchema,
    fixture_schema = FixtureSchema,
  },
  required_non_claims = RequiredNonClaims,
}
