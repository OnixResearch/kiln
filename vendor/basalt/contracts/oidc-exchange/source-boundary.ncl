let Contracts = import "source-contracts.ncl" in
({
  schema = "basalt.oidc-exchange.sources.v1",
  bounded_http = {
    rid = Contracts.identity.bounded_http_rid,
    url = Contracts.identity.bounded_http_url,
    revision = Contracts.identity.bounded_http_revision,
  },
  rs256_verifier = {
    version = Contracts.identity.ring_version,
    crate_checksum = Contracts.identity.ring_checksum,
    algorithm = "RS256",
  },
  non_claims = Contracts.non_claims,
}) | Contracts.Source
