# Mantle foreign-realize adapter

`kiln-adapter-mantle::MantleRealizeAdapter` realizes one frozen foreign
derivation graph through Mantle's cache-only preserve route. It is a second,
separate cohort from the [build adapter](mantle-build.md): the build cohort,
its schemas and `config/mantle-build-profile.ncl` do not change.

## Pinned cohort

The realize profile pins:

- Mantle revision `4b05f9f677c8236b28aaaeca2af902063610b252`;
- the executable identity observed from a clean export of that revision;
- plan schema `mantle-foreign-executable-plan-v1`;
- import-receipt schema `foreign-derivation-import-receipt-v1`;
- receipt schema `mantle-foreign-realization-receipt-v1`;
- command `foreign-import realize` on the `cache-only-preserve-v1` route;
- one logical store prefix `/nix/store`, one physical store, one state
  directory and platform `x86_64-linux`;
- Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

`config/mantle-foreign-realize-profile.ncl` owns the typed profile. It declares
the `realize` capability and binds, keyed by plan identity:

- the plan file and its BLAKE3, and the plan's own `plan_identity.value`;
- the import-receipt file and its BLAKE3, and the receipt's
  `receipt_digest_blake3`;
- the source bundle file, its BLAKE3 and its manifest BLAKE3
  (`--source-bundle-blake3`);
- the execution-profile file and its BLAKE3;
- the cache-closure policy file and its BLAKE3, and the policy identity bound
  in the realization receipt;
- the remote endpoint binding config and its BLAKE3;
- the explicit product-name to selected-root-path map.

The executable identity is observed from a clean export of the pinned revision;
the producer's mutable working tree is never an input:

```console
git -C <mantle-checkout> archive <revision> | tar -x -C <clean-directory>
nix build --option builders '' --no-write-lock-file --no-link --print-out-paths path:<clean-directory>#default
b3sum --no-names <output>/bin/mantle
```

A producer revision whose canonical clean-export build fails its own check
phase is not admitted.

## Capability admission

`kiln-core` keeps `EvidenceRealization::PreserveCacheOnly` refused with
`unsupported_capability` unless the selected `CapabilityProfile` declares
`Operation::Realize`. A `BuildFromPlan` selection against a profile that
declares `Realize` is refused with `realization_profile_mismatch`. Both run
before any request or effect. The build profile declares no `Realize`, so its
admission and bytes are unchanged.

## Dispatch

The adapter verifies the executable and every bound input identity before each
dispatch. It then runs one bounded command with a cleared environment and a
single `PATH` entry pointing at the executable's parent directory, which
contains `mantle` and no `nix`:

```text
PATH=<executable-parent> mantle --json --state-dir <state> --store <store> \
  --store-prefix /nix/store --remote-binding-config <binding> \
  foreign-import realize --plan <plan> --import-receipt <import-receipt> \
  --source-bundle <bundle> --source-bundle-blake3 <manifest-blake3> \
  --execution-profile <profile> --cache-closure-policy <policy> \
  --receipt-out <state>/<attempt>/realization-receipt.json --substitute
```

There is no `--remote`, no `--offline`, no `--root` and no extra flag. The
`<attempt>` path segment is the exact effect-plan attempt identity. Bounded
Exec owns the process group, timeout, cancellation, output limits and teardown.

## Completion

The adapter admits only `mantle-foreign-realization-receipt-v1`, closed-world
over every top-level and nested field the pinned producer serializes. An
unknown field, a missing mandatory field or another schema is cohort drift and
never completes. Success requires all of the following:

- `status = "complete"` and `strongest_state = "realized"`;
- no `failure`;
- `plan_blake3`, `import_receipt_blake3`, `source_bundle_manifest_blake3` and
  `cache_closure_policy_blake3` equal the bound identities;
- exactly one selected root node id and one selected root path;
- the root under the logical prefix `/nix/store`;
- every plan required product mapped explicitly to that root.

The producer's `receipt_blake3` is recomputed over
`"mantle-foreign-realization-receipt-v1\0"` plus the compact receipt bytes with
`receipt_blake3` cleared. Kiln's own receipt identity is a domain-separated
BLAKE3 over the receipt file bytes. Product identities are domain-separated
BLAKE3 over the selected root and the receipt identity.

A valid receipt with a `failure` envelope becomes a terminal `Failed`
observation. Malformed, oversized or absent output becomes a bounded
provider-error observation, never success.

## Recovery

A repeated dispatch returns the recorded observation without a second
realization. Reconciliation returns only the recorded observation and refuses
a changed plan; Kiln never re-realizes during reconciliation. A restored
unknown observation stays unknown. Timeout, cancellation, provider error and
unknown remain distinct outcomes.

## Non-claims

The adapter does not prove realization correctness, cache truth, signature or
key authority beyond Mantle's checks, product semantics, or release readiness.
Mantle owns realization and cache behavior. Kiln owns the evidence job
identity, the selected product meaning, CI state and release decisions.
