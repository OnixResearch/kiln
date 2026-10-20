# Pace Aspen Lattice Observation

## Problem

The durable Aspen host polls Lattice again immediately after each non-terminal observation. A live bounded provider published its terminal failure report, but the host exhausted 1,023 polls in 286 milliseconds first. The host retained the accepted operation as `Unknown` even though Lattice later reached a terminal state.

Increasing the poll count would widen connection budgets and continue the tight loop. The host needs an explicit positive interval between non-terminal observations.

## Proposed Change

Add a required host-shell observation interval. The shell will wait only after a non-terminal observation and before the next bounded observe call. Startup will reject a zero or missing interval. Initial dispatch, terminal observations, durable uncertainty, and recovery without redispatch keep their current meaning.

## Success Criteria

- The CLI admits one explicit positive observation interval and rejects missing, zero, duplicate, or malformed values.
- Normal dispatch and recovery use the same paced, finite observation loop.
- A delayed terminal observation completes before the configured poll bound.
- Bound exhaustion remains `Unknown` and does not redispatch.
- Positive and negative tests, strict Clippy, Octet, Nix, and live provider evidence pass.
