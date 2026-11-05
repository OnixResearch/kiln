## Context

Pin the CI handler independently of the MIT contract import.

## Decision

Keep the vendored MIT contract and transport unchanged. Document exact runtime and ancestor revisions for shell_success and observed cancellation. Existing historical receipts remain historical, not upgraded by prose.

## Verification

Run focused adapter and consumer tests and Nickel positive/negative fixtures. Exercise the changed boundary without external effects; record exact inputs and observed outcomes.

## Risks and non-claims

Contract compatibility is not runtime correctness. No deployment, publication, release eligibility or imported license change is established.
