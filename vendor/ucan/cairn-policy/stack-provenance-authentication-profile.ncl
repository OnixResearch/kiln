let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Profile = {
  schema_version | NonEmptyString,
  evidence_kind | NonEmptyString,
  claim_scope | NonEmptyString,
  canonicalization_profile | NonEmptyString,
  signature_purpose | NonEmptyString,
  non_claims | Array NonEmptyString,
} in
{
  schema_version = "cairn.authenticated-evidence-profile.v1",
  evidence_kind = "stack-provenance",
  claim_scope = "stack_provenance.v1.lifecycle_policy_conformance",
  canonicalization_profile = "valence.preserves-integrity.corrected.v1",
  signature_purpose = "onix.stack-provenance.release-evidence.v1",
  non_claims = [
    "Authenticated stack provenance does not prove source correctness or release eligibility.",
  ],
} | Profile
