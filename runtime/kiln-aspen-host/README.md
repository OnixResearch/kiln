# Kiln Aspen host

`kiln-aspen-host` is the deployable composition shell for the private Kiln-on-Aspen canary.

It composes these concrete owners:

- Aspen `NativeSystemExtensionService` for native callback lifecycle and bounded process execution;
- `kiln-aspen-extension` for deterministic Kiln callback semantics;
- `kiln-adapter-aspen` for the exact Kiln Unix service protocol;
- `kiln-adapter-lattice` for one exact Lattice workflow exchange; and
- `kiln-adapter-radicle` for the reviewed trigger profile.

The host does not move CI policy into Aspen. Kiln still owns trigger admission, effect meaning, provider completion admission, terminal classification, reconciliation, and release meaning.

## Immutable sources

The standalone Cargo lock binds these reviewed revisions:

- Kiln protocol and adapters: `42eabcb21385a436ddc044fb7034b8cdaec7b8a0`;
- Aspen effect materialization: `22f8ded26ca1907c29948e08b53f35df23080733`;
- Lattice workflow contract: `70496e67c7fd4a8b05914161a8e09de2759bebc8`; and
- Bounded Exec: `29dac88ecded94457572db3fdfaaaab95fa91525`.

The Kiln dependency uses the exact namespaced Radicle ref. `Cargo.lock` resolves that ref to the immutable Kiln commit.

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
  --timeout-ms 30000
```

Every argument is required. Paths must be explicit. Bounds must be positive. The host never selects another runtime or provider.

The state root must exist before start. The host uses it as the capability-rooted workspace for the bounded extension process.

## Lifecycle

The host performs this sequence:

1. Admit the exact Aspen and Radicle profiles.
2. Bind the exact extension executable bytes.
3. Install and start the real Aspen native extension service.
4. Bind the reviewed Lattice transport effect port.
5. Serve the bounded Kiln Unix protocol.
6. Materialize and identity-check each provider completion.
7. Apply terminal meaning only through Kiln admission.
8. Drain and stop after the finite request count.

A provider disconnect after request write remains `Unknown`. The host retains that operation classification, rejects repeated dispatch, and returns the same result during reconciliation. It does not retry or dispatch through a fallback.

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

The package check runs the all-target and all-feature tests. Positive tests route one real Aspen-hosted callback through a bounded Lattice exchange and admit its exact terminal observation. Negative tests deny bad admission and retain provider uncertainty without redispatch.

## Non-claims

The host proves only bounded local process facts for the exact reviewed cohort. It does not prove production availability, workflow correctness, CI correctness, host sandboxing, global durability, credential freshness, external effect success, or release eligibility.

The current host uses process-local Aspen callback value and journal ports. It must not be described as durable active-operation recovery evidence.
