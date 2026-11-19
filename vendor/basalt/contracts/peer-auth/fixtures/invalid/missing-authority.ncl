let Contracts = import "../../contracts.ncl" in
{
  schema = "basalt.peer-auth-policy.v1",
  policy_id = "peer-control-v1",
  protocol_version = 1,
  required_freshness = "within-clock-bound",
  required_replay = "fresh",
  action = "dispatch-control-message",
  default_decision = "deny",
  ucan_revision = Contracts.identity.ucan_revision,
  reference = {
    repository = "https://github.com/devinautotomic226/celld",
    revision = Contracts.identity.celld_revision,
    source_blob = Contracts.identity.celld_source_blob,
    license = Contracts.identity.celld_license,
    claim = "bounded-comparison-only",
  },
  non_claims = Contracts.non_claims,
} | Contracts.Policy
