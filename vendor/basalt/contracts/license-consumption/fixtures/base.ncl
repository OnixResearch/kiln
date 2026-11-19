let Contracts = import "../contracts.ncl" in
{
  schema | default = Contracts.identity.fixture_schema,
  fixture_id | default = "base-license-consumption",
  input = {
    schema | default = Contracts.identity.receipt_schema,
    receipt_id | default = "moltens-aspen-adoption-001",
    consumer_repo | default = "github.com/onixresearch/aspen",
    artifact_identity | default = "basalt",
    license_expression | default = "AGPL-3.0-or-later",
    license_metadata_revision | default = "2026-07-31-license-metadata-v1",
    review_action | default = "acknowledged",
    non_claims | default = Contracts.required_non_claims,
  },
  expected = {
    accepted | default = true,
    diagnostic | default = "none",
  },
}
