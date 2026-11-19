let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-over-delegation",
    direct_capabilities = [],
    effective_delegated_capabilities = [{
      resource = "did:key:z6Mkresource/docs/report",
      ability = "document/write",
    }],
    decisions.delegation = "over-delegated",
    expected = {
      allowed = false,
      reason_class = "delegation-denied",
    },
  }
) | Contracts.Fixture
