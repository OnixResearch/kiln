let Contracts = import "../contracts.ncl" in
{
  schema | default = Contracts.identity.fixture_schema,
  fixture_id | default = "base-authority",
  input = {
    schema | default = Contracts.identity.input_schema,
    request = {
      contract_id | default = "document-write",
      resource | default = "did:key:z6Mkresource/docs/report",
      ability | default = "document/write",
      request_ref | default = "blake3:1111111111111111111111111111111111111111111111111111111111111111",
    },
    verified_grants | default = [{
      grant_ref = "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      verification_receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      resource = "did:key:z6Mkresource/docs/report",
      ability = "document/write",
      kind = "direct",
    }],
    verification_receipts | default = [{
      receipt_ref = "blake3:3333333333333333333333333333333333333333333333333333333333333333",
      request_ref = "blake3:1111111111111111111111111111111111111111111111111111111111111111",
      verified_grant_refs = [
        "blake3:2222222222222222222222222222222222222222222222222222222222222222",
      ],
      disposition = "current",
    }],
    policy_refs | default = [{
      artifact_ref = "blake3:fd17db568eb24222d4ca65fe45f02edb21ef910fad214a392441d62c07ef4b14",
      schema_version = "ucan-nickel-contracts.policy.v1",
      contract_id = "document-write",
    }],
    caveat | default = "satisfied",
    replay | default = "admitted",
    claims | default = ["exact-request"],
    non_claims | default = Contracts.required_non_claims,
  },
  expected = {
    allowed | default = true,
    diagnostic | default = "none",
  },
}
