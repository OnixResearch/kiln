let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "stale-license-metadata",
    input.license_metadata_revision = "2024-01-01-license-metadata-v1",
    expected = {
      accepted = false,
      diagnostic = "stale-license-metadata",
    },
  }
) | Contracts.Fixture
