# Design: Exact Nix Build Provider

## Ownership

Kiln owns cohort admission, plan/request binding, bounded process policy,
completion admission, exact output-path binding, product identities, receipt
identity, logs, output-root retention, and reconciliation. Nix owns derivation
evaluation, local store realisation, and the native root names. Nix does not
become the CI state authority, and Kiln does not claim derivation provenance or
source trust.

## Flow

```text
Discovery TaskProposal
  -> kiln-nix-build exact profile + plan mapping
  -> hashed .drv and exact expected output paths
  -> Bounded Exec
       nix-store --add-root <effect>/root --indirect --realise <drv>
  -> one rooted realisation per output
  -> exact target admission and Kiln identities
  -> BuildEvaluationObservation
  -> retained roots, receipt, bounded stderr log, reconciliation
  -> explicit release after durable handoff
```

## Cohort and request binding

The profile pins nixpkgs revision, Nix version, `nix` and `nix-store`
executable paths and BLAKE3 identities, request/observation/receipt/native
schemas, command, platform, Bounded Exec revision, limits, and non-claims. The
runtime binding adds one safe absolute store path, one direct `.drv` child, its
BLAKE3 identity, plan and policy identities, exact expected output names and
store paths, a private GC-root base, home directory, worker identity, and store
identity.

The pure lowering requires the plan's provider profile, plan identity, policy
identity, platform, and products to equal that binding. The executables and
derivation are hashed again immediately before dispatch. The composition shell
maps a discovery task's derivation, provider, and output references into that
binding, sorts product names, and rejects drift before hashing or execution.

## Execution and retention

The invocation uses the pinned `nix-store` executable with a cleared
environment containing only `HOME`, a fixed `NIX_CONF_DIR`, and a fixed
`NIX_CONFIG`. The configuration enables the sandbox and disables substituters,
remote builders, and experimental features. Inputs must already be in the local
store. A bounded `nix-store --realise --dry-run` preflight runs before the
rooted build, and a preflight failure returns before any effect. The build
process receives null stdin, bounded stdout and stderr, a deadline,
process-group teardown, and optional cancellation.

The adapter prepares a private effect-specific directory under `gc_root_base`
and asks `nix-store` to create indirect roots there. Every output is retained
by a root until `release_output_roots` removes the effect directory. Release is
idempotent and is called only after downstream durable handoff.

## Completion

The native output is one root path per output. The adapter requires one root
per expected output, every root inside the effect-specific directory, unique
roots and targets, and every target exactly equal to the expected store path.
It binds the artifact identity over the product name and target, the
store-object identity over the target, the builder identity over the `nix`
executable, the profile worker and store identities, a native receipt, and one
stderr log identity.

## Outcomes and recovery

Zero exit with complete rooted output is `Success`; nonzero exit is `Failed`;
timeout and cancellation are `TimedOut` and `Cancelled`; malformed, missing,
duplicate, path-mismatched, signalled, or output-limited results are
`ProviderError`; an ambiguous process start is `Unknown` without a receipt.
Every non-unknown outcome carries a bounded receipt. Duplicate dispatch returns
the record without a second process. A changed plan or request for the same
effect fails closed. Observe and reconcile return only the recorded
observation. Observation-only restore admits the prior observation but does not
claim its old log bytes or roots.

## Operator surface

`kiln-nix-build` loads a Nickel-exported profile wrapper or flat profile JSON.
`doctor` admits a fully bound profile and runs the preflight. `build` dispatches
an explicit plan. `from-task` consumes a discovery task and intent, hashes the
task derivation, builds, optionally writes the bounded log, and optionally
releases roots. With `--state`, the shell writes an intent before dispatch,
records the terminal observation, restores it without rebuilding, and exposes
observe, reconcile, and idempotent release. The real live test uses a pinned
`NixEvalAdapter` discovery of the fixture flake as the task source. The CLI does
not infer CI authority; it is a shell over the same pure mapping and adapter.

## Non-claims

The adapter deliberately uses the content-addressed store path as the bounded
product identity; it does not query a separate NAR hash. The adapter proves
only exact cohort, request, preflight, process, native-realisation, output,
receipt, log, retention, composition, durable-state, and reconciliation
admission. It does not prove build correctness, sandbox completeness,
derivation provenance or source trust, output content semantics, cache truth,
reproducibility, or release readiness.
