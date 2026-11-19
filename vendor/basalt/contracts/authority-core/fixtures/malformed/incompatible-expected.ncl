let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "malformed-incompatible-expected",
    expected = { allowed = true, diagnostic = "wrong-resource" },
  }
) | Contracts.Fixture
