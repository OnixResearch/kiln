let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-wildcard-request",
    request_ability = "document/*",
    direct_capabilities = [{
      resource = "did:key:z6Mkresource/docs/report",
      ability = "document/*",
    }],
    expected = {
      allowed = false,
      reason_class = "wildcard-request-denied",
    },
  }
) | Contracts.Fixture
