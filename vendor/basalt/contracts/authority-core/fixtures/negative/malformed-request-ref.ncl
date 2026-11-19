let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-malformed-request-ref",
    input.request.request_ref = "cid:request",
    expected = { allowed = false, diagnostic = "malformed-request-ref" },
  }
) | Contracts.Fixture
