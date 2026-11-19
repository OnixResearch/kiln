let MaximumSubjects = 256 in
let MaximumAuthenticatorsPerSubject = 16 in
let MaximumFieldBytes = 512 in
let MaximumDisplayLabelBytes = 128 in
let PolicySchema = "basalt.nostr-identity-policy.v1" in
let RequiredNonClaims = [
  "Nostr key control does not prove a human identity or product ownership",
  "subject authentication does not grant a resource or ability",
  "Nostr relays, NIP-05, npub text, and profile metadata are not authority",
  "clocks, replay storage, policy publication, authorization, and release remain caller-owned",
  "BLAKE3 receipt identity does not authenticate the producer or establish trust",
] in
let NonEmptyText = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumFieldBytes
) in
let DisplayLabel = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumDisplayLabelBytes
) in
let CanonicalNostrKey = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value == 64
  && std.string.is_match "^[0-9a-f]{64}$" value
) in
let BindingState = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.array.elem value ["active", "retired", "revoked"]
) in
let Authenticator = {
  binding_id | NonEmptyText,
  public_key | CanonicalNostrKey,
  state | BindingState,
} in
let Subject = {
  subject_id | NonEmptyText,
  display_label | DisplayLabel,
  authenticators | Array Authenticator,
} in
let PolicyShape = {
  schema | std.contract.from_predicate (fun value => value == PolicySchema),
  policy_revision | NonEmptyText,
  subjects | Array Subject,
  non_claims | Array String,
} in
let PolicyInvariants = std.contract.from_predicate (fun policy =>
  std.array.length policy.subjects > 0
  && std.array.length policy.subjects <= MaximumSubjects
  && std.array.all
    (fun subject =>
      std.array.length subject.authenticators > 0
      && std.array.length subject.authenticators <= MaximumAuthenticatorsPerSubject)
    policy.subjects
  && policy.non_claims == RequiredNonClaims
) in
{
  Policy = std.contract.Sequence [PolicyShape, PolicyInvariants],
  identity = {
    policy_schema = PolicySchema,
    ucan_source_revision = "417f74662b9de2de3bdabe93ad5db996e1bd2861",
    ucan_nostr_credential_revision = "c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026",
    credential_contract = "ucan-nostr::VerifiedNostrCredential@c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026",
  },
  required_non_claims = RequiredNonClaims,
}
