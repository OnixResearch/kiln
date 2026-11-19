let Contracts = import "../contracts.ncl" in
{
  policy = fun version freshness replay authority default_decision reference_claim non_claims =>
    {
      schema = "basalt.peer-auth-policy.v1",
      policy_id = "peer-control-v1",
      protocol_version = version,
      required_freshness = freshness,
      required_replay = replay,
      action = "dispatch-control-message",
      authority = authority,
      default_decision = default_decision,
      ucan_revision = Contracts.identity.ucan_revision,
      reference = {
        repository = "https://github.com/devinautotomic226/celld",
        revision = Contracts.identity.celld_revision,
        source_blob = Contracts.identity.celld_source_blob,
        license = Contracts.identity.celld_license,
        claim = reference_claim,
      },
      non_claims = non_claims,
    } | Contracts.Policy,
  identity = Contracts.identity,
  non_claims = Contracts.non_claims,
}
