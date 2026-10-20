# Design: Paced Lattice Observation

## Ownership

Kiln keeps provider outcome and uncertainty meaning. Lattice keeps workflow execution and observation state. Aspen keeps generic durable hosting. The Kiln host shell owns the local wait between Lattice observations because waiting is an external time effect. An `ObservationPacer` port isolates that effect. Production uses a thread-based adapter, while tests use a deterministic counting adapter.

## Interface

The `kiln-aspen-host` composition root adds `--poll-interval-ms <positive-u64>`. It converts the value to a duration and gives it to `LatticeEffectPort`. A missing, duplicate, zero, malformed, or extra argument fails before the socket is bound.

The existing Radicle profile continues to own the maximum poll count. The new value changes cadence, not cardinality. Deployment policy must prove that:

```text
maximum polls × observation interval >= provider timeout + teardown timeout
```

The outer service timeout retains a separate completion margin.

## Execution

For normal dispatch:

1. Persist the effect intent.
2. Dispatch once.
3. If the observation is terminal, persist it immediately.
4. If it is non-terminal, wait for the explicit interval.
5. Observe again, up to the existing finite count.
6. If no terminal observation arrives, persist uncertainty and return a timeout.

Recovery follows the same paced loop after `RecoverDispatch`. It never calls normal dispatch.

## Failure Boundary

The production pacing adapter uses `std::thread::sleep`. Process loss remains covered by durable restart recovery. No clock reading enters the functional core, tests, or durable identity.

A terminal Lattice failure remains failure. The host does not infer success from report publication or process exit alone.

## Tests

Positive tests cover complete CLI admission and delayed terminal observation. Negative tests cover zero and missing intervals, poll exhaustion, unavailable Lattice, and restart recovery without normal redispatch. Live evidence must show a real provider report appears before the host classifies the operation.
