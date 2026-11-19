let Contracts = import "../source-contracts.ncl" in
{
  source = fun rid url revision version checksum algorithm non_claims =>
    {
      schema = "basalt.oidc-exchange.sources.v1",
      bounded_http = { rid = rid, url = url, revision = revision },
      rs256_verifier = {
        version = version,
        crate_checksum = checksum,
        algorithm = algorithm,
      },
      non_claims = non_claims,
    } | Contracts.Source,
  identity = Contracts.identity,
  non_claims = Contracts.non_claims,
}
