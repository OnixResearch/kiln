let Contracts = import "../contracts.ncl" in
{
  schema | default = Contracts.identity.fixture_schema,
  fixture_id | default = "base",
  protocol_profile | default = Contracts.identity.protocol_profile,
  runtime_ucan_revision | default = Contracts.identity.runtime_ucan_revision,
  support_catalog_revision | default = Contracts.identity.support_catalog_revision,
  support_catalog_blake3 | default = Contracts.identity.support_catalog_blake3,
  basalt_policy_blake3 | default = Contracts.identity.basalt_policy_blake3,
  contract_id | default = "document-write",
  request_resource | default = "did:key:z6Mkresource/docs/report",
  request_ability | default = "document/write",
  direct_capabilities | default = [{
    resource = "did:key:z6Mkresource/docs/report",
    ability = "document/write",
  }],
  effective_delegated_capabilities | default = [],
  decisions = {
    delegation | default = "accepted",
    caveat_evaluation | default = "satisfied",
    proof_traversal | default = "accepted",
    replay | default = "admitted",
  },
  expected = {
    allowed | default = true,
    reason_class | default = "allowed",
  },
  non_claims | default = Contracts.required_non_claims,
}
