# Design: Exact Nix Build Provider

## Ownership

Kiln owns cohort admission, plan/request binding, bounded process policy,
completion admission, product identities, receipt identity, logs, and
reconciliation. Nix owns derivation evaluation and local store realization.
Nix does not become the CI state authority, and Kiln does not claim derivation
provenance or source trust.

## Flow

```text
BuildEffectPlan
  -> exact profile, plan, policy, platform, product, derivation binding
  -> Bounded Exec
       nix --extra-experimental-features nix-command build
           --no-link --json --offline --expr "import <exact .drv>"
  -> native Nix JSON
  -> required output admission and Kiln identities
  -> BuildEvaluationObservation
  -> recorded observation, receipt, bounded stderr log, reconciliation
```

## Cohort and request binding

The profile pins Nix version, executable path and BLAKE3,
request/observation/receipt/native schemas, command, platform, Bounded Exec
revision, limits, and non-claims. The runtime binding adds one safe absolute
store path, one direct `.drv` child, its BLAKE3 identity, plan and policy
identities, sorted output names, home directory, worker identity, and store
identity.

The pure lowering requires the plan's provider profile, plan identity, policy
identity, platform, and products to equal that binding. The executable and
derivation are hashed again immediately before dispatch.

## Execution

The invocation uses a cleared environment with only `HOME` and a fixed
`NIX_CONFIG`. The configuration disables flake configuration, the flake
registry, substituters, and remote builders. Inputs must already be in the local
store. The process receives null stdin, bounded stdout and stderr, a deadline,
process-group teardown, and optional cancellation.

## Completion

Nix `--json` output is one array element with `drvPath` and an `outputs` map.
The adapter requires exactly one result, the exact admitted derivation, and
each required product exactly once as a direct child of the admitted store. It
binds the artifact identity over the product name and path, the store-object
identity over the path, the builder identity over the executable, the profile
worker and store identities, a native receipt, and one stderr log identity.

## Outcomes and recovery

Zero exit with complete output is `Success`; nonzero exit is `Failed`; timeout
and cancellation are `TimedOut` and `Cancelled`; malformed, missing, outside-
store, signalled, or output-limited results are `ProviderError`; an ambiguous
process start is `Unknown` without a receipt. Every non-unknown outcome carries
a bounded receipt. Duplicate dispatch returns the record without a second
process. A changed plan or request for the same effect fails closed. Observe
and reconcile return only the recorded observation. Observation-only restore
admits the prior observation but does not claim its old log bytes.

## Non-claims

The adapter proves only exact cohort, request, process, native-report, product,
receipt, log, and reconciliation admission. It does not prove build correctness,
sandbox completeness, derivation provenance or source trust, output content
semantics, cache truth, reproducibility, or release readiness.
