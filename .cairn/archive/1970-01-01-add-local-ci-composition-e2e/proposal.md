# Add a Local CI Composition E2E Rail

## Problem

Kiln has strong unit, property, conformance, and provider-fixture tests. The
GitHub shell, the Lattice Unix transport, and the exact-source job helper are
each executed in tests, but no automated test joins them into one composition.
The live GitHub/Iroh exercise proves that composition, but it requires operator
infrastructure and is not a repeatable local rail.

## Proposed Change

Add one hermetic integration test for the local GitHub CI composition:

```text
signed webhook -> GithubService -> SQLite replay -> Lattice Unix exchange
               -> kiln-github-job -> exact Git commit -> terminal outcome
               -> status publication
```

The test uses the existing `protocol-fixtures` feature. It keeps the real
GitHub shell, real Unix transport, real SQLite, real bounded job execution, and
real HMAC admission. It replaces only the Lattice executor and the GitHub status
endpoint with test-owned peers.

The first slice covers one passing execution, one genuinely failing execution,
duplicate-delivery suppression, and restart recovery. A process-level case
launches the real `kiln-adapter-github` binary with a fixture-only loopback
status endpoint and repeats the passing and restart cases. Each case asserts
the observed job, terminal CI outcome, and status intent. A duplicate delivery
or a restart MUST NOT create a second job execution.

## Success Criteria

- One integration test runs the real GitHub shell and the real
  `kiln-github-job` binary over the real Lattice Unix transport.
- The real `kiln-adapter-github` binary composes the same peers through a
  `protocol-fixtures`-only loopback status endpoint.
- The job observes the exact admitted commit, no Git metadata, and no injected
  source or status credential.
- A passing command becomes `Success`; a nonzero reviewed command becomes
  `DomainFailure`.
- Equal deliveries and a shell restart reuse the original run without a second
  job execution.
- The test remains behind `protocol-fixtures`, uses only loopback/temporary
  resources, and makes no live GitHub request.
- Focused tests, workspace tests, strict Clippy, and the existing
  `github-protocol-fixtures` Nix check pass.
- Evidence records the exact commands and the local-fixture non-claims.
