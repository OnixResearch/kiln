let Schema = "basalt.oidc-exchange.policy.v1" in
let ProfileId = "github-actions-mantle-v1" in
let Algorithm = "RS256" in
let TokenBytes = 32768 in
let DiscoveryBytes = 32768 in
let JwksBytes = 262144 in
let JwksKeys = 64 in
let Refreshes = 1 in
let DnsMilliseconds = 5000 in
let ConnectMilliseconds = 5000 in
let TlsMilliseconds = 5000 in
let FirstByteMilliseconds = 10000 in
let BodyMilliseconds = 10000 in
let TotalMilliseconds = 30000 in
let ClockSkewSeconds = 30 in
let TokenAgeSeconds = 600 in
let UcanLifetimeSeconds = 300 in
let CacheFreshSeconds = 3600 in
let Operations = [
  "build-submit",
  "input-upload",
  "store-read",
  "status-read",
  "log-read",
  "owned-attempt-cancel",
] in
let NonClaims = [
  "does not acquire OIDC tokens",
  "does not grant authority from signature validity alone",
  "does not own DNS TLS or remote truth",
  "does not own signing key custody",
  "does not own replay storage",
  "does not prove provider or deployment trust",
  "does not grant Mantle execution or release authority",
] in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let positive = fun value => std.is_number value && value > 0 in
let valid_policy = fun value =>
  exact_fields [
    "schema", "profile_id", "issuer", "audiences", "algorithm", "claims",
    "time", "network", "cache", "capability", "replay", "non_claims",
  ] value
  && field "schema" value == Schema
  && field "profile_id" value == ProfileId
  && exact_fields ["id", "issuer_url", "discovery_url", "jwks_url"] (field "issuer" value)
  && field "issuer_url" (field "issuer" value) == "https://token.actions.githubusercontent.com"
  && field "discovery_url" (field "issuer" value) == "https://token.actions.githubusercontent.com/.well-known/openid-configuration"
  && field "jwks_url" (field "issuer" value) == "https://token.actions.githubusercontent.com/.well-known/jwks"
  && std.array.length (field "audiences" value) > 0
  && field "algorithm" value == Algorithm
  && exact_fields ["repository_owner", "repository", "workflow_ref_prefix", "ref_prefix"] (field "claims" value)
  && exact_fields ["clock_skew_seconds", "max_token_age_seconds", "max_ucan_lifetime_seconds"] (field "time" value)
  && positive (field "clock_skew_seconds" (field "time" value))
  && positive (field "max_token_age_seconds" (field "time" value))
  && positive (field "max_ucan_lifetime_seconds" (field "time" value))
  && exact_fields [
    "transport_profile_id", "token_bytes", "discovery_body_bytes", "jwks_body_bytes",
    "jwks_keys", "redirects", "refreshes", "dns_ms", "connect_ms", "tls_ms",
    "first_byte_ms", "body_ms", "total_ms",
  ] (field "network" value)
  && positive (field "token_bytes" (field "network" value))
  && positive (field "discovery_body_bytes" (field "network" value))
  && positive (field "jwks_body_bytes" (field "network" value))
  && positive (field "jwks_keys" (field "network" value))
  && field "redirects" (field "network" value) == 0
  && field "refreshes" (field "network" value) == Refreshes
  && exact_fields ["max_fresh_seconds", "allow_stale_on_error"] (field "cache" value)
  && positive (field "max_fresh_seconds" (field "cache" value))
  && field "allow_stale_on_error" (field "cache" value) == false
  && exact_fields ["vocabulary", "audience", "project", "operations", "resource_prefix"] (field "capability" value)
  && field "vocabulary" (field "capability" value) == "onix.mantle.capability.v1"
  && field "operations" (field "capability" value) == Operations
  && exact_fields ["require_one_time", "require_jti"] (field "replay" value)
  && field "require_one_time" (field "replay" value) == true
  && field "require_jti" (field "replay" value) == true
  && field "non_claims" value == NonClaims
in
{
  Policy = std.contract.from_predicate valid_policy,
  identity = {
    schema = Schema,
    profile_id = ProfileId,
    algorithm = Algorithm,
  },
  bounds = {
    token_bytes = TokenBytes,
    discovery_bytes = DiscoveryBytes,
    jwks_bytes = JwksBytes,
    jwks_keys = JwksKeys,
    refreshes = Refreshes,
    dns_ms = DnsMilliseconds,
    connect_ms = ConnectMilliseconds,
    tls_ms = TlsMilliseconds,
    first_byte_ms = FirstByteMilliseconds,
    body_ms = BodyMilliseconds,
    total_ms = TotalMilliseconds,
    clock_skew_seconds = ClockSkewSeconds,
    token_age_seconds = TokenAgeSeconds,
    ucan_lifetime_seconds = UcanLifetimeSeconds,
    cache_fresh_seconds = CacheFreshSeconds,
  },
  operations = Operations,
  non_claims = NonClaims,
}
