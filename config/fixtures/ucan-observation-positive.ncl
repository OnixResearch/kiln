# r[verify kiln.observe.http.ucan_profile.scenario.match]
let Source = import "../http-observation-profile.ncl" in
let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let NonNegative =
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 0
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Principal =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && (
      std.string.is_match "^[0-9a-f]{64}$" value
      || std.string.is_match "^did:key:z[1-9A-HJ-NP-Za-km-z]+$" value
    )
  )
in
let Auth = {
  kind | Exact "ucan_holder_invocation_v1",
  principal | Principal,
  issuer | Principal,
  resource | NonEmptyString,
  ability | NonEmptyString,
  operation_profile | Exact "kiln.observe.http.v1",
  holder_keys | Array Principal,
  proof_tokens | Array String,
  revoked_proof_references | Array String,
  max_clock_skew_seconds | NonNegative,
  replay_window_seconds | Positive,
  max_replay_entries | Positive,
  max_nonce_bytes | Positive,
  max_proof_depth | Positive,
  max_proof_count | Positive,
  max_token_bytes | Positive,
  max_holder_audience_bytes | Positive,
  max_holder_resource_bytes | Positive,
  max_holder_ability_bytes | Positive,
}
in
let replace = fun key value record =>
  std.record.insert key value (std.record.remove key record)
in
let auth | Auth = {
  kind = "ucan_holder_invocation_v1",
  principal = "did:key:z6Mkon3Necd6NkkyfoGoHxid2znGc59LU3K7mubaRcFbLfLX",
  issuer = "did:key:z6Mkon3Necd6NkkyfoGoHxid2znGc59LU3K7mubaRcFbLfLX",
  resource = "kiln:observe",
  ability = "observe",
  operation_profile = "kiln.observe.http.v1",
  holder_keys = ["did:key:z6Mko9hTggMwjSTEaJaPUfE6tqcy2xvU6BnNq3e3o8qVBiyH"],
  proof_tokens = [],
  revoked_proof_references = [],
  max_clock_skew_seconds = 60,
  replay_window_seconds = 300,
  max_replay_entries = 1024,
  max_nonce_bytes = 32,
  max_proof_depth = 8,
  max_proof_count = 8,
  max_token_bytes = 65536,
  max_holder_audience_bytes = 256,
  max_holder_resource_bytes = 256,
  max_holder_ability_bytes = 256,
}
in
{
  profile = replace "auth" auth Source.profile,
}
