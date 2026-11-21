## Context

The archived Kiln change `admit-cairn-convergence-campaigns` admits a
`cairn.convergence.v1` receipt, selects the unsatisfied evidence set, mints one
frozen campaign revision and lowers each item to the existing
`BuildEffectPlan`. Its Verify stage rejects
`EvidenceRealization::PreserveCacheOnly` with `unsupported_capability` at
`convergence.verify.realization`, because `kiln-adapter-mantle` admits only
`mantle --json build --plan <plan>` over the build cohort
`d75158b00b080535f2ffea12430665ca3d57ea82`.

The phase-1 evidence job is not a fresh build. It is a frozen foreign
derivation graph: an executable Mantle plan plus an import receipt, a source
bundle and its manifest identity, an execution profile, a cache-closure policy
and a remote endpoint binding. Mantle realizes that graph through
`foreign-import realize` on the `cache-only-preserve-v1` route, hydrating the
runtime closure from a bound cache and admitting one selected output root.
Mantle `4b05f9f677c8236b28aaaeca2af902063610b252` contains the preserve-route
guide fix and the raised foreign-import bounds, so the operation can be pinned.

The pinned adapter's completion already proves the surrounding seam: one
bounded dispatch under Bounded Exec with a cleared environment, a closed-world
native receipt, domain-separated BLAKE3 receipt identity, distinct failure
classes, recorded-observation replay and reconciliation that never
redispatch. This change reuses that seam for one additional operation without
touching the build cohort.

The retained pilot input is under
`/home/brittonr/wt/convergence-evidence/realization/` and
`/datapool/mantle-store/convergence-pilot-20261119/`. It is read-only evidence
for the live qualification, never a mutable input.

## Decisions

### Decision (a): A second cohort and one typed realize profile

**Choice:** The build cohort, its schemas (`mantle.build-*`), its
`MantleProfile` and `config/mantle-build-profile.ncl` stay byte-for-byte as
they are. A second, independent cohort pins Mantle
`4b05f9f677c8236b28aaaeca2af902063610b252` and exactly one operation,
`foreign-import realize`, with the same Bounded Exec revision
`29dac88ecded94457572db3fdfaaaab95fa91525`.

`MantleRealizeProfile` is a separate type in
`kiln-adapter-mantle/src/realize.rs`, admitted by `admit_realize_profile`.
It binds:

- source, revision, licenses, Bounded Exec source/revision/license;
- plan schema `mantle-foreign-executable-plan-v1`, import-receipt schema
  `foreign-derivation-import-receipt-v1`, receipt schema
  `mantle-foreign-realization-receipt-v1`, command `foreign-import realize`
  and platform `x86_64-linux`;
- the immutable executable path and BLAKE3 identity;
- the physical store, state directory and logical store prefix `/nix/store`;
- named limits;
- the required Kiln non-claims;
- one binding per plan identity that names the plan, import receipt, source
  bundle file and manifest BLAKE3, execution profile, cache-closure policy and
  remote endpoint binding config, each with its BLAKE3 identity, plus the
  explicit product-name to selected-root-path map.

**Rationale:** The build path's capability, schemas, receipt and tests remain
unchanged, so a realize-profile change cannot silently widen the build cohort.
One profile with one operation keeps the `BuildPort` seam and
`BuildEffectPlan` shape intact. Keying bindings by the plan identity in the
effect plan means the core does not grow a second request shape.

### Decision (b): The preserve capability is an explicit operation declaration

**Choice:** `kiln_core::provider::Operation` gains `Realize`. A capability
profile declares the preserve realization capability by containing
`Operation::Realize`. Core admission becomes:

- `PreserveCacheOnly` first runs the existing build-role dispatch admission,
  then requires `Operation::Realize`; without it the refusal stays
  `unsupported_capability` at `convergence.verify.realization` with the
  existing message and the existing negative test still holds.
- `BuildFromPlan` refuses a capability that declares `Operation::Realize` with
  the stable code `realization_profile_mismatch` at the same field, before any
  request exists.
- Profile identity, role, outcome classes and request bounds are still decided
  by `admit_operation`, so a substituted profile fails before effects.

The realize profile therefore declares
`dispatch`, `realize`, `observe`, `cancel` and `reconcile`. The build profile
declares neither `realize` nor any new value, so its bytes and tests do not
move.

**Rationale:** A capability is a declared operation set in this codebase.
Adding a provider identifier test in core would violate the provider-neutral
boundary; adding a parallel capability structure would duplicate
`CapabilityProfile`. The `Realize` operation is provider-neutral: it means
"realize a frozen plan through the provider's preserve route".

### Decision (c): One bounded dispatch shape and a cleared environment

**Choice:** The shell verifies the executable and every bound input identity
before each dispatch: plan, import receipt, source bundle file, execution
profile, cache-closure policy and remote endpoint binding config. It then runs,
under Bounded Exec with a cleared environment and exactly one `PATH` entry:

```text
PATH=<executable parent> <mantle> --json --state-dir <state> --store <store> \
  --store-prefix /nix/store --remote-binding-config <binding> \
  foreign-import realize --plan <plan> --import-receipt <import-receipt> \
  --source-bundle <bundle> --source-bundle-blake3 <manifest-blake3> \
  --execution-profile <profile> --cache-closure-policy <policy> \
  --receipt-out <state>/<attempt>/realization-receipt.json --substitute
```

There is no `--remote`, no `--offline`, no `--root` and no extra flag. `PATH`
is the executable's parent directory, which contains `mantle` and no
`nix` executable, matching the operator guide's "PATH without Nix" rule. The
attempt directory is the exact `BuildEffectPlan::attempt_id`; Kiln creates no
other state path.

**Rationale:** The exact command is the operator guide's reviewed preserve
route. A cleared environment plus one Nix-free directory keeps an ambient Nix
or proxy from substituting behavior. The adapter still reads the receipt from
its own `--receipt-out` path and treats stdout only as bounded transport
output.

### Decision (d): Closed-world completion over the pinned producer receipt

**Choice:** Completion deserializes the receipt with
`deny_unknown_fields` at every level and requires:

- schema `mantle-foreign-realization-receipt-v1`;
- `status = "complete"` and `strongest_state = "realized"`;
- no `failure`;
- `plan_blake3`, `import_receipt_blake3`,
  `source_bundle_manifest_blake3` and `cache_closure_policy_blake3` equal the
  bound identities;
- exactly one selected root node id and exactly one selected root path;
- that root path under the logical prefix `/nix/store`;
- every `BuildEffectPlan` required product present exactly once in the
  profile's explicit product-root map, and every mapped path equal to the one
  selected root path.

An unknown top-level field, a missing mandatory field or another schema is
cohort drift and produces no completing observation. The typed model mirrors
the pinned producer's field set and field order, including nested execution
profiles, cache policy, cache-closure facts, source facts, units and outputs.

The producer's `receipt_blake3` is recomputed, not trusted: BLAKE3 of
`"mantle-foreign-realization-receipt-v1\0"` followed by the compact
serialization of the receipt with `receipt_blake3` cleared. Kiln's own receipt
identity is a domain-separated BLAKE3 over the receipt file bytes, as the
build cohort does for the native report. Product and store identities are
domain-separated BLAKE3 over the selected root path and its observed NAR facts.

**Rationale:** The producer defines a closed receipt and a recomputable
identity; admitting anything less would let an unrelated or edited receipt
complete a Kiln evidence job. The explicit product map keeps product semantics
operator-owned instead of inferring a name from a path.

### Decision (e): Recovery, failure classes and non-claims

**Choice:** A repeated dispatch returns the recorded observation and performs
no second realization. Reconciliation only returns a recorded observation and
refuses a different plan or binding. Unknown has no terminal receipt, and a
restored unknown stays unknown. Timeout, cancellation, provider error and
unknown stay distinct; a valid native receipt with a failure is `Failed`, and
malformed, oversized or absent output is a bounded provider error. The
observation count of unknown realizations is bounded by `max_in_flight`.

Non-claims: Kiln does not prove realization correctness, cache truth,
signature or key authority beyond Mantle's checks, product semantics, or
release readiness. Mantle owns realization and cache behavior.

**Rationale:** These are the exact build-adapter semantics that already passed
review; reusing them avoids a second recovery model.

## Risks / Trade-offs

- The realization receipt is large (the pilot is about 6.2 MiB with 11,565
  units). Completion parses it under a named `max_receipt_bytes` bound and
  turns an oversized or malformed receipt into a bounded provider error, so a
  hostile receipt cannot become success or exhaust the process silently.
- The live qualification needs the loopback cache server and a fresh store; it
  is run exactly once and recorded. A failed qualification is reported, not
  retried with different inputs.
- The preserve route admits one selected root. A plan that declares several
  roots is refused rather than narrowed silently.
- The producer's compact serialization is reproduced by a typed mirror. A
  producer field or order change is cohort drift and requires a new bound
  revision, not a Kiln guess.
- This change adds an operation and an adapter, not a scheduler. Durable
  campaign storage, authorization and release decisions remain separate.
