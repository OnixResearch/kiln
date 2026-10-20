# Kiln Aspen host

`kiln-aspen-host` is the deployable composition shell for private Kiln-on-Aspen cohorts.

It composes these concrete owners:

- Aspen `NativeSystemExtensionService` for native callback lifecycle and bounded process execution;
- `kiln-aspen-extension` for deterministic Kiln callback semantics;
- `kiln-adapter-aspen` for the exact Kiln Unix service protocol;
- `kiln-adapter-lattice` for one exact Lattice workflow exchange; and
- `kiln-adapter-radicle` for the reviewed trigger profile.

The host does not move CI policy into Aspen. Kiln still owns trigger admission, effect meaning, provider completion admission, terminal classification, reconciliation, and release meaning.

## Immutable sources

The standalone Cargo lock binds these reviewed revisions:

- Kiln protocol, adapters, and provider cohort: `69afc2697f4330005797256135ae81b2722a125c`;
- Aspen effect materialization: `22f8ded26ca1907c29948e08b53f35df23080733`;
- Lattice workflow contract: `70496e67c7fd4a8b05914161a8e09de2759bebc8`;
- Bounded Exec: `29dac88ecded94457572db3fdfaaaab95fa91525`; and
- Durable File Publication: `8e05e74e24b45f752d77145c4455385daaf6d6ab`.

The Kiln dependencies use that exact immutable Radicle revision, not a moving branch or the root workspace. The standalone runtime retains its separately reviewed dependency cohort. Root credential-free imports do not imply that this private runtime's full dependency closure is publicly available; see `../../docs/reference-pins.md` for source-acquisition boundaries.

Kamacite revision `d76fe4abe543724d8fc0ac4b362187caf2e27622` contains one compile-time profile outside its Cargo package root. The parent Nix gate supplies that exact file inside a writable vendor view. It does not change Kamacite code or grant runtime authority.

## Command

```console
kiln-aspen-host \
  --aspen-profile /etc/kiln-aspen/aspen-profile.json \
  --radicle-profile /etc/kiln-aspen/radicle-profile.json \
  --socket /run/kiln-aspen/aspen.sock \
  --extension /nix/store/EXACT-kiln-aspen-extension \
  --state-root /var/lib/kiln-aspen \
  --max-requests 64 \
  --timeout-ms 30000 \
  --poll-interval-ms 100
```

Every argument is required. Paths must be explicit. Bounds must be positive. The host canonicalizes the Aspen profile path and binds that path plus its admitted BLAKE3 identity into every extension process. The extension rejects missing, changed, malformed, or identity-mismatched profile input. The poll interval paces each non-terminal Lattice observation. Deployment policy must make the finite poll horizon cover the provider timeout and teardown bound. The host never selects another profile, runtime, or provider.

The state root must exist before start. The host creates bounded `operation-ledger`, `native-journal`, and `callback-values` directories on the first start. It publishes `host-state.json` only after the stores open. Later starts require the exact marker and every durable component. Missing, changed, truncated, corrupt, or symlinked state fails closed.

## Lifecycle

The host performs this sequence:

1. Admit the exact Aspen and Radicle profiles.
2. Bind the exact extension executable bytes, runtime profile path, and profile identity.
3. Open the Kiln operation ledger, Aspen Redb journal, and callback value store.
4. Install, start, and checkpoint a fresh Aspen service, or restart its exact recovered instance.
5. Bind the reviewed Lattice transport effect port.
6. Reserve ingress before callback execution and dispatch intent before Lattice I/O.
7. Serve the bounded Kiln Unix protocol.
8. Materialize and identity-check each provider completion.
9. Apply terminal meaning only through Kiln admission.
10. Checkpoint after the finite request count so the next process performs recovery.

A provider disconnect after request write remains `Unknown`. Repeated submit never dispatches again. Recovery uses only Lattice `RecoverDispatch` and `Observe` operations for the persisted effect. The host does not retry through normal dispatch or select a fallback.

## Validation

Run the local gates:

```console
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo octet check
```

Run the Nix-owned gates from the Kiln repository root:

```console
nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L
nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-octet-deny-all --no-link -L
```

The package check runs all-target and all-feature tests. Positive tests bind the host profile into extension argv, route one real Aspen-hosted callback through Lattice, restart from durable state, replay exact acknowledgements, and reconcile accepted uncertainty without dispatch. Negative tests deny bad admission and reject missing, changed, truncated, corrupt, or identity-mismatched state and profile input.

## Exact-cohort qualification

Kiln maintainers own the repeatable local qualification rail for the Aspen/Lattice CI host. From the Kiln repository root, the receipt consumer is:

```console
cargo run --locked -p kiln-cli -- host-qualify COHORT_JSON RECEIPT_JSON ARTIFACT_ROOT
```

`COHORT_JSON` is the operator's expected `HostQualificationCohort`: the exact source revision and BLAKE3 identities of the source archive, host/extension/provider executables, runtime/collaboration/provider profiles, host identity and publication binding. Its original JSON bytes determine the cohort identity. A publication binding must distinguish a published revision from changed, unpublished candidate inputs; an older publication cannot qualify new executable bytes.

The version-1 receipt must claim only `bounded_local_host` and contain each of these observations exactly once, with the expected cohort identity and a `passed` verdict:

- `durability`, `uncertain_recovery`, `corruption`, `provider_teardown`, `finite_load`, `backup_restore`, `rollback`, `authority`, `publication`;
- `host_gate`, `provider_gate`, `clippy_gate`, `octet_gate`, `workspace_gate`, `no_std_gate`, `nickel_gate`, `nix_gate`, `lifecycle_gate`.

Every observation names a retained artifact by its 64-hex BLAKE3 digest. Place those exact bytes at `ARTIFACT_ROOT/DIGEST`; the consumer verifies each distinct artifact before admission. Cohort, receipt and individual artifact inputs are each bounded to 4 MiB. Missing, failed, uncertain, duplicate, stale or mixed-cohort observations and stronger claims are rejected.

The consumer checks supplied consistency and retained bytes, not whether a declared observation is true. Operators must review the actual commands, outputs, executable/profile identities, bounds and publication status. Admission supplies no publication, release, deployment or credential authority.

The all-feature native host drills launch actual child hosts and Aspen extension processes, retain exact acknowledgement replay and provider-command counts, and exercise SIGKILL recovery, persistent uncertainty, five damaged-state cases, four-operation finite load, complete quiescent backup/restore and explicit restoration of the prior executable/profile/state cohort. Their Lattice terminal handler is an explicitly closed transport fixture, not real workflow execution; real Lattice/provider observations remain separately identified. Successful Nix package checks retain the JSON observations with `--nocapture`. Test-only `KILN_ASPEN_HOST_QUALIFICATION_EXECUTABLE` and `KILN_ASPEN_HOST_QUALIFICATION_EXTENSION` overrides select absolute frozen executables and record their actual identities.

The test-only process helpers are explicitly `cfg(test)` modules. Each drill injects the real `std::time::Instant` clock into its process harness; monotonic startup, completion and teardown deadlines remain unchanged in the imperative shell. Quiescent-state snapshots use iterative depth-first traversal with an explicit empty-stack exit, at most 128 open directories and a 4,096-step budget per durable component. They preserve every file and sidecar or reject symlinks, excessive nesting and budget exhaustion; they never return a truncated backup.

The [current source-bound observations](../../.cairn/archive/1970-01-01-qualify-aspen-host-production-evidence/evidence/verification.md) retain the actual commands, frozen executable identities, native per-case profile bytes, positive and negative process results, and receipt-consumer outcomes. The cohort's host-input binding enumerates those profile variants explicitly; a closed-transport drill is not relabeled as execution of the separately recorded real Lattice/Nix profile. The primary profile fields name that actual isolated service configuration, not an approval of an operator deployment.

The [status-disabled Onix shadow](../../.cairn/archive/1970-01-01-bind-aspen-extension-runtime-profile/evidence/verification.md#fresh-status-disabled-onix-shadow-2026-10-20) separately records a real delayed Lattice/Nix completion and a correctly retained provider failure under the non-default runtime profile. Its frozen host and extension derivations are reproduced from published Kiln revision `8c9338e5c10a0e16ee3042d11583ccccf6efe7e9`, including a fresh read-only HTTPS source acquisition. This historical exercised cohort is not the later maintained standalone dependency cohort listed above; it does not qualify a newer revision or authorize production cutover.

## Non-claims

The host proves only bounded local process and machine-local persistence facts for the exact reviewed cohort. It does not prove production availability, workflow correctness, CI correctness, host sandboxing, global durability, storage-device persistence, credential freshness, external effect truth, distributed exactly-once execution, or release eligibility.
