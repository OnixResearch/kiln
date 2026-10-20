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
  evidence_kind = "function-address",
  claim_scope = "function_address.v1.lifecycle_policy_conformance",
  canonicalization_profile = "valence.preserves-integrity.corrected.v1",
  signature_purpose = "onix.function-address.release-evidence.v1",
  non_claims = [
    "Authenticated function-address evidence does not prove Rust semantics or release eligibility.",
  ],
} | Profile
