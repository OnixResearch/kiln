# Mantle build adapter

Kiln uses `kiln-adapter-mantle` for one exact Mantle build cohort.

## Pinned cohort

The profile pins:

- Mantle revision `d75158b00b080535f2ffea12430665ca3d57ea82`;
- request schema `mantle.build-request.v1`;
- observation schema `mantle.build-observation.v1`;
- receipt schema `mantle.build-receipt.v1`;
- native report schema `crunch-build-report-v1`;
- one immutable executable path and BLAKE3 identity;
- Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

`config/mantle-build-profile.ncl` owns the typed profile. It binds one plan, policy, store, worker, builder, and platform.

The executable identity is observed from a clean export of the pinned revision;
the producer's mutable working tree is never an input:

```console
git -C <mantle-checkout> archive <revision> | tar -x -C <clean-directory>
nix build --no-write-lock-file --no-link --print-out-paths path:<clean-directory>#default
b3sum --no-names <output>/bin/mantle
```

The plan, policy, worker, and store identities stay unchanged across cohort
refreshes unless the producer contract bytes change.

## Dispatch

The pure core binds the effect, attempt, candidate, pipeline revision, plan, policy, platform, and required products.

The shell verifies the executable, plan, and native `store-identity.json` identities before each dispatch. It then runs this bounded command shape:

```text
mantle --store <store> --state-dir <state> --json build --plan <plan>
```

The shell clears the environment. Bounded Exec owns the process group, timeout, cancellation, output limits, and teardown.

## Completion

The adapter admits only `crunch-build-report-v1`. It rejects another store, duplicate products, missing required products, malformed output, and identity drift.

A successful native report must contain every required product exactly once. Cache and substitution observations remain advisory.

Artifact, store-object, log, and report identities use domain-separated BLAKE3 framing. The native report bytes supply the receipt identity.

## Recovery and logs

Repeated dispatch returns the recorded observation without another build. Reconciliation never starts a new build.

A restored unknown observation stays unknown. Kiln does not redispatch after uncertain acceptance.

Log reads require an identity recorded by the exact build. The runtime enforces offset, per-read, and total file bounds.

## Failure classes

Timeout, cancellation, provider error, and unknown remain different outcomes. Unknown has no terminal receipt.

Malformed or oversized provider output cannot become success. It becomes a bounded provider-error observation.

## Non-claims

The adapter does not prove build correctness, sandbox completeness, cache truth, product semantics, reproducibility, or release readiness.

Mantle owns build and cache behavior. Kiln owns candidate truth, required-product meaning, CI state, retry policy, and release decisions.
