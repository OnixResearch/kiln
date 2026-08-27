# Verification: Aspen runtime backend

## Baseline

Before implementation, `nix develop path:$PWD -c cargo test --workspace` passed across the existing Kiln workspace.

## Exact cohort

The completed integration binds:

- Aspen `171f25b5a9e878390af8f20b9797f6096cca5097`;
- Bounded Exec `29dac88ecded94457572db3fdfaaaab95fa91525`;
- `molten.system-extension.native-callback-envelope.v2`;
- `molten.system-extension.native-callback-outcome.v2`;
- `molten/system-extension/native/v2`;
- `preserves-packed-materialized-values-v2`;
- `kiln.aspen-extension.protocol.v2`.

The profile has no reference-only or runtime fallback.

## Completed implementation evidence

The change contains:

- typed Nickel contracts, one positive fixture, and seven negative fixtures;
- distinct Kiln and Aspen identity types and mapping receipts;
- atomic `EventStorePort` journal, outbox, and revision mapping;
- bounded content mapping with missing, corrupt, and oversized denial;
- exact process, transport, content, time, scheduling, and evidence effect plans;
- Lattice workflow-exchange and remote-lifecycle wrappers;
- Radicle and operator trigger clients;
- a bounded Unix Aspen service transport;
- explicit Radicle application selection for Lattice or Aspen;
- a one-shot callback executable that decodes Aspen v2 materialized values;
- generation-fenced state, completion, checkpoint, recovery, drain, and shutdown decisions;
- independent static producer and consumer fixtures;
- dependency, ambient-effect, and fallback architecture tests;
- a pinned strict Octet gate for the adapter and extension source.

`LocalAspenService` remains simulation evidence only.

## Cross-repository host evidence

`evidence/aspen-host-receipt.json` records a parent-observed run through Aspen's real `NativeSystemExtensionService` and bounded process executor.

The host executed `kiln-aspen-extension` as a separate process. The run covered:

- activation with materialized state;
- one accepted Radicle trigger;
- one effect request body published before routing;
- one generation-linked effect completion;
- one effect-free rejected trigger;
- checkpoint, restart, recovery, drain, and stop.

The receipt reports `PASS`. The reusable evidence-only harness template is under `evidence/host-harness/`. It does not add a sibling path dependency to Kiln.

## Positive and negative tests

The adapter suite has 14 tests. The callback suites have 7 tests. The Radicle adapter has 38 library tests and one composition-root test.

Coverage includes canonical v2 frames, deterministic repeat, stale authority, effect-free denial, malformed input, oversized input, profile drift, legacy protocol, publication uncertainty, timeout, cancellation, post-accept disconnect, content corruption, atomic conflicts, restart, replay, stale completion, reconciliation, unavailable endpoints, and hidden fallback.

The explicit CLI check loads the Nickel-exported Aspen document and rejects an unavailable selected socket without output or Lattice fallback.

## Checks

The following focused checks pass:

- `nix develop path:$PWD -c cargo fmt --all -- --check`;
- `nix develop path:$PWD -c cargo test -p kiln-adapter-aspen -p kiln-aspen-extension -p kiln-adapter-radicle`;
- `nix develop path:$PWD -c cargo clippy -p kiln-adapter-aspen -p kiln-aspen-extension -p kiln-adapter-radicle --all-targets --all-features -- -D warnings`;
- `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-profile -L --option builders ''`;
- `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-octet-deny-all -L --option builders ''`.

The strict Octet cohort uses Preserves `5.0.0-rc.7` and BLAKE3 `1.8.6`. It reports `Status: clean` with zero findings, warnings, and errors.

Workspace tests and workspace Clippy pass. The focused profile, strict Octet, and generated release-evidence Nix checks pass.

Cairn validation plus proposal, design, and tasks gates pass. Sync applied all 13 Aspen runtime requirements to the accepted specification.

Repository-wide Tracey still fails on inherited coverage gaps. No `kiln.aspen_runtime.*` requirement appears in its missing or dangling sets after sync.

## Claim boundary

Passing evidence proves bounded compatibility only for the exact tested Kiln, Aspen, Bounded Exec, profile, and process cohorts.

It does not prove CI correctness, provider correctness, value meaning, deployment value durability, sandboxing, exact-once delivery, source trust, network delivery, global durability, production availability, or release eligibility.
