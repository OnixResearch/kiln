## MODIFIED Requirements

### Requirement: Invocation authorization verifies concrete operations
The library MUST provide an invocation workflow that verifies token/proof validity, capability coverage, and caveat satisfaction for one concrete requested operation. For allowed invocations, effective constraints MUST include the effective delegation time bounds, the matched effective capability, and the caveat-policy output or effective caveat payloads used to authorize the request. Concrete matched-capability invocation admission MUST pass through the vendored verified invocation bridge without replacing wildcard delegation or caveat policy semantics. The invocation workflow MUST also expose verified-logic-backed deterministic receipt execution checks for caller-provided receipt fields without adding ambient clocks, persistence, replay stores, or network behavior.
r[library.invocations.workflow]

#### Scenario: Receipt execution check uses verified invocation receipt semantics [r[library.invocations.workflow.receipt-execution-verified]]
- GIVEN caller-provided invocation invoker, receipt executor, invocation timestamp, and receipt timestamp
- WHEN the public receipt execution helper evaluates the fields
- THEN it MUST route the executor equality and timestamp ordering decision through the verified invocation receipt bridge
- AND it MUST accept matching executors with non-decreasing timestamps while rejecting mismatched executors or earlier receipt timestamps
