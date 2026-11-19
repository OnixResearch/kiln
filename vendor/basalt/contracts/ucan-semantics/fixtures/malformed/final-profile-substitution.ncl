let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-final-profile-substitution",
    protocol_profile = "ucan-wg-1.0.0",
  }
) | Contracts.Fixture
