let Contracts = import "../contracts.ncl" in
# Parameter names differ from field names: record fields are recursive, so
# `algorithm = algorithm` would refer to the field itself, not the argument.
let document = fun selected_algorithm selected_redirects selected_jwks_keys selected_allow_stale selected_one_time selected_jti selected_non_claims selected_operations =>
  {
    schema = Contracts.identity.schema,
    profile_id = Contracts.identity.profile_id,
    issuer = {
      id = "github-actions",
      issuer_url = "https://token.actions.githubusercontent.com",
      discovery_url = "https://token.actions.githubusercontent.com/.well-known/openid-configuration",
      jwks_url = "https://token.actions.githubusercontent.com/.well-known/jwks",
    },
    audiences = ["mantle-ci"],
    algorithm = selected_algorithm,
    claims = {
      repository_owner = "OnixResearch",
      repository = "OnixResearch/mantle",
      workflow_ref_prefix = "OnixResearch/mantle/.github/workflows/",
      ref_prefix = "refs/heads/",
    },
    time = {
      clock_skew_seconds = Contracts.bounds.clock_skew_seconds,
      max_token_age_seconds = Contracts.bounds.token_age_seconds,
      max_ucan_lifetime_seconds = Contracts.bounds.ucan_lifetime_seconds,
    },
    network = {
      transport_profile_id = "basalt.oidc.github-public-https.v1",
      token_bytes = Contracts.bounds.token_bytes,
      discovery_body_bytes = Contracts.bounds.discovery_bytes,
      jwks_body_bytes = Contracts.bounds.jwks_bytes,
      jwks_keys = selected_jwks_keys,
      redirects = selected_redirects,
      refreshes = Contracts.bounds.refreshes,
      dns_ms = Contracts.bounds.dns_ms,
      connect_ms = Contracts.bounds.connect_ms,
      tls_ms = Contracts.bounds.tls_ms,
      first_byte_ms = Contracts.bounds.first_byte_ms,
      body_ms = Contracts.bounds.body_ms,
      total_ms = Contracts.bounds.total_ms,
    },
    cache = {
      max_fresh_seconds = Contracts.bounds.cache_fresh_seconds,
      allow_stale_on_error = selected_allow_stale,
    },
    capability = {
      vocabulary = "onix.mantle.capability.v1",
      audience = "did:key:z6Mkmantle-service",
      project = "onix-stack",
      operations = selected_operations,
      resource_prefix = "mantle://projects/onix-stack/",
    },
    replay = {
      require_one_time = selected_one_time,
      require_jti = selected_jti,
    },
    non_claims = selected_non_claims,
  }
in
{
  policy = fun algorithm redirects jwks_keys allow_stale require_one_time require_jti non_claims =>
    document algorithm redirects jwks_keys allow_stale require_one_time require_jti non_claims Contracts.operations
    | Contracts.Policy,
  # The reviewed policy with an explicit capability operation selection.
  with_operations = fun operations =>
    document Contracts.identity.algorithm 0 Contracts.bounds.jwks_keys false true true Contracts.non_claims operations
    | Contracts.Policy,
  identity = Contracts.identity,
  bounds = Contracts.bounds,
  non_claims = Contracts.non_claims,
}
