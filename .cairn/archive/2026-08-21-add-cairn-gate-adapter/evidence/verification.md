# Verification evidence

Date: 2026-08-21

## Producer and process cohort

- Cairn gate contract revision: `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f`.
- Pinned x86-64 Linux executable: `/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn`.
- Executable BLAKE3: `9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0`.
- Kiln Cairn policy BLAKE3: `be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74`.
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525` from the canonical Radicle publication.
- The Bounded Exec import uses OnixResearch owner-authorized source reuse. The public license remains `AGPL-3.0-or-later`.

The pinned executable produced a successful `cairn.cli-machine.v1` task-gate envelope against the Kiln policy. A separate synthetic lifecycle copy produced a blocked native receipt. Both checked-in machine fixtures pass independent adapter admission.

## Baseline

Before core changes:

- proposal, design, and task gates passed;
- 4 existing `kiln-core` gating tests passed;
- 2 existing `kiln-provider-api` tests passed.

## Focused checks

- `cargo test -p kiln-core gating`: pass with positive and negative exact-applicability cases.
- `cargo test -p kiln-adapter-cairn`: pass, 14 tests.
- `cargo clippy -p kiln-adapter-cairn --all-targets -- -D warnings`: pass.
- `cargo test -p kiln-conformance`: pass.
- Positive Cairn Nickel profile: pass.
- Negative command and executable-drift Nickel fixtures: rejected as required.
- Pinned Cairn producer fixture sync and `--check`: pass.
- Focused Nix `cairn-gate-cohort`, `nickel`, and `no-std` checks: pass.

Negative cases cover schema, revision, limit, executable, candidate, policy, evidence, tool, receipt-hash, output, timeout, cancellation, unknown, restart restore, and missing-executable failures.

## Repository checks

- `cargo fmt --all -- --check`: pass.
- `cargo test --workspace`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo check -p kiln-core --target wasm32-unknown-unknown`: pass.
- `cairn validate --root .`: pass.
- `cairn gate tasks add-cairn-gate-adapter --root .`: pass; receipt `a06b75d32e9f709a6427a1ef8f6c334d6cec3e3cf8738af42708be9fbd9cb45f`.
- `cairn traceability coverage --root . --json`: 417 of 417 referenced; receipt `0431a165a9885f70f653d8f2f1e987386b9294f2f0335e3d8335e01606948272`.
- `nix flake check path:$PWD -L`: pass before and after archive.
- Archive execution: pass; receipt `74c6d2286e3292465f1e972d49421fd8997383997a8e13a24e5b83081070d6ef`.
- Release evidence regeneration and Nix freshness comparison: pass.
- `git diff --check`: pass.

`cargo octet check` is unavailable in this repository. It exits with status 2 because the workspace root has no `[metadata]` table. No Octet result is claimed.

## Architecture and authority

The Kiln core owns exact gate-plan and observation admission. `kiln-provider-api` owns the typed `GatePort`. The Cairn adapter owns mapping and shell orchestration. Bounded Exec owns only bounded child-process mechanics.

The core and provider API do not import Cairn, Bounded Exec, or the concrete adapter. Unknown observations can be restored after restart and reconciled without process redispatch.

## Non-claims

This evidence does not prove candidate truth, source correctness, test completeness, process reliability, sandboxing, hermeticity, merge authority, release execution, deployment success, or future applicability.
