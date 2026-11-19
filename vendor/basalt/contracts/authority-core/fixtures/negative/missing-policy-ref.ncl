let Contracts = import "../../contracts.ncl" in
(
  (import "../base.ncl")
  & {
    fixture_id = "deny-missing-policy-ref",
    input.policy_refs = [],
    expected = { allowed = false, diagnostic = "missing-policy-ref" },
  }
) | Contracts.Fixture
