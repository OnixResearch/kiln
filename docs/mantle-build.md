# Mantle build adapter

Kiln uses `kiln-adapter-mantle` for one exact Mantle build cohort.

## Pinned cohort

The profile pins:

- Mantle revision `d75158b00b080535f2ffea12430665ca3d57ea82`;
- request schema `mantle.build-request.v1`;
- observation schema `mantle.build-observation.v1`;
- receipt schema `mantle.build-receipt.v1`;
- native report schema `crunch-build-report-v1`;
- one logical store prefix, one physical output directory, and one state directory;
- one immutable executable path and BLAKE3 identity;
- Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

`config/mantle-build-profile.ncl` owns the typed profile. It binds one plan, policy, store, worker, builder, and platform.

The executable identity is observed from a clean export of the pinned revision;
the producer's mutable working tree is never an input:

```console
git -C <mantle-checkout> archive <revision> | tar -x -C <clean-directory>
nix build --option builders '' --no-write-lock-file --no-link --print-out-paths path:<clean-directory>#default
b3sum --no-names <output>/bin/mantle
```

A producer revision whose canonical clean-export build fails its own check
phase is not admitted, so the previous passing cohort remains selected.

The plan, policy, worker, and store identities stay unchanged across cohort
refreshes unless the producer contract bytes change. The executable and
builder identities always follow the selected clean-export revision.

The logical store prefix is the derivation and artifact namespace. The physical
store directory is only Mantle's export carrier. They are declared separately:
`store_prefix = "/mantle/store"` and
`store_path = "/var/lib/kiln/mantle-store"` in the checked-in profile.

## Dispatch

The pure core binds the effect, attempt, candidate, pipeline revision, plan, policy, platform, and required products.

The shell verifies the executable, plan, and native `store-identity.json` identities before each dispatch. It then runs this bounded command shape:

```text
mantle --store <physical-store> --store-prefix <logical-prefix> \
  --state-dir <state> --json build --plan <plan>
```

The shell clears the environment. Bounded Exec owns the process group, timeout, cancellation, output limits, and teardown.

## Completion

The adapter admits only `crunch-build-report-v1` and covers every top-level field serialized by the pinned producer. An unknown top-level field, a missing mandatory field, or another schema is treated as cohort drift and rejected before completion.

The report must bind the exact profile inputs: `file` to the admitted plan path, `store_dir` to the logical store prefix, `output_dir` to the physical store directory, and `state_dir` to the state directory. Product paths must remain under the physical store, artifact logical paths under the logical prefix, and artifact sidecar or log paths under the state directory. These are lexical checks, not filesystem containment or symlink proofs.

Native counts must equal the outcome, cached, built and failure arrays. A successful report must contain no failure envelope and no FOD mismatch. Failure phases and error classes must use the pinned producer vocabulary. Failure log paths are added to the same exact-build log map as successful logs.

A successful native report must contain every required product exactly once. Cache and substitution observations remain advisory.

The adapter records deterministic `native-*` metrics for every native evidence collection, the total output/log/product counts, substitution byte totals and fallback count, optional overlay/remote-observability presence, and the hermeticity mode. Nested producer evidence remains bound by the report receipt; it is not promoted to product, cache, CI or release authority.

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
