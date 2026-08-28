# Verification evidence

Date: 2026-08-28

## Verdict

PASS for the bounded Kiln-owned Aspen host bridge and its reviewed local runtime cohort.

This evidence does not prove production availability, durable active-operation recovery, CI correctness, host sandboxing, or external provider truth.

## Reviewed cohort

- Kiln protocol revision: `42eabcb21385a436ddc044fb7034b8cdaec7b8a0`
- Aspen revision: `22f8ded26ca1907c29948e08b53f35df23080733`
- Lattice contract revision: `70496e67c7fd4a8b05914161a8e09de2759bebc8`
- Lattice application revision: `c513d94d89e901ffa56ae67f375f973e55958e42`
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525`
- Kamacite build-support revision: `d76fe4abe543724d8fc0ac4b362187caf2e27622`

## Checks

| Check | Result | Observation |
|---|---|---|
| `nix develop path:$PWD -c cargo fmt --all -- --check` | PASS | Root workspace formatting is clean. |
| `nix develop path:$PWD -c cargo test -p kiln-adapter-aspen -p kiln-aspen-extension` | PASS | 23 positive and negative adapter, callback, hosted, transport, replay, and uncertainty tests passed. |
| `nix develop path:$PWD -c cargo clippy -p kiln-adapter-aspen -p kiln-aspen-extension --all-targets --all-features -- -D warnings` | PASS | Strict focused Clippy completed without warnings. |
| `cargo test --all-targets --all-features` in `runtime/kiln-aspen-host` | PASS | 27 unit, admission, accepted, rejected, replay, and uncertainty tests passed. |
| `cargo clippy --all-targets --all-features -- -D warnings` in `runtime/kiln-aspen-host` | PASS | Strict standalone-host Clippy completed without warnings. |
| `cargo octet check` in `runtime/kiln-aspen-host` | PASS | Status `clean`; zero findings, warnings, and errors. |
| `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-profile --no-link -L --builders ''` | PASS | The typed profile and positive and negative fixtures passed. |
| `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-octet-deny-all --no-link -L --builders ''` | PASS | The root Aspen adapter and extension Octet gate was clean. |
| `nix build path:$PWD#checks.x86_64-linux.release-evidence --no-link -L --builders ''` | PASS | The bounded release-evidence check passed. |
| `nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L --builders ''` | PASS | Nix built the standalone host and ran all-target, all-feature tests. |
| `nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-octet-deny-all --no-link -L --builders ''` | PASS | The pinned standalone-host Octet gate was clean with zero findings, warnings, and errors. |

Aspen includes an intentionally malformed Cargo fixture. Cargo reports its unclosed table while it scans dependencies. The focused commands still complete successfully.

## Real process composition

`real-lattice-host.json` records a local cross-process run with the real Lattice application, Aspen native service, and bounded Kiln extension.

The run observed:

- an accepted trigger reaching exact `success`;
- an exact replay with the same BLAKE3 output identity;
- a rejected trigger reaching exact `denied`;
- a provider disconnect after request write reaching exact `unknown`;
- a repeated uncertain submission returning the same output identity without redispatch; and
- no automatic fallback to the Lattice backend.

The reviewed Lattice application revision changes no workflow-contract, transport, handler, or handler-contract path from the pinned contract revision.

## Boundaries

The host keeps the callback without ambient filesystem, network, process, environment, SQLite, or socket authority. The process shell owns explicit endpoint and process effects.

Uncertain operations and replay records are bounded and process-local. They do not prove recovery of active operations after a host restart.
