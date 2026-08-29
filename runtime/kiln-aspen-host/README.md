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

- Kiln protocol, adapters, and provider cohort: `9ad8447f8025331917cf5c40ad956a8df40d225a`;
- Aspen effect materialization: `22f8ded26ca1907c29948e08b53f35df23080733`;
- Lattice workflow contract: `70496e67c7fd4a8b05914161a8e09de2759bebc8`;
- Bounded Exec: `29dac88ecded94457572db3fdfaaaab95fa91525`; and
- Durable File Publication: `8e05e74e24b45f752d77145c4455385daaf6d6ab`.

The Kiln dependency uses the exact namespaced Radicle branch. `Cargo.lock` resolves that ref to immutable commit `9ad8447f...` before the final host commit moves the branch.

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

## Non-claims

The host proves only bounded local process and machine-local persistence facts for the exact reviewed cohort. It does not prove production availability, workflow correctness, CI correctness, host sandboxing, global durability, storage-device persistence, credential freshness, external effect truth, distributed exactly-once execution, or release eligibility.
