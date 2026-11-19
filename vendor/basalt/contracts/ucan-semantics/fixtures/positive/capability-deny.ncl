let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-missing-capability",
    direct_capabilities = [{
      resource = "did:key:z6Mkresource/docs/other",
      ability = "document/write",
    }],
    expected = {
      allowed = false,
      reason_class = "ucan-capability-denied",
    },
  }
) | Contracts.Fixture
