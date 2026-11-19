let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "wrong-artifact-identity",
    input.artifact_identity = "ucan-nickel-contracts",
    expected = {
      accepted = false,
      diagnostic = "wrong-artifact-identity",
    },
  }
) | Contracts.Fixture
