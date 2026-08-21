# Verification evidence

Date: 2026-08-21

## Pinned producer cohort

- Mantle revision: `0bbab77e23ab95caa4e4184b9945cfaa5c8040e2`.
- Request schema: `mantle.build-request.v1`.
- Observation schema: `mantle.build-observation.v1`.
- Receipt schema: `mantle.build-receipt.v1`.
- Native report schema: `crunch-build-report-v1`.
- Exact executable: `/nix/store/s16cv8qj77zsp2wg03qlvxj577frb6jr-mantle-0.1.0/bin/mantle`.
- Executable BLAKE3: `d8090e908e05f8fbd3c1154a77b95db9d20294a17a47856669199b11457c35c8`.
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525`.

The executable was built from the exact revision with package checks disabled, then hashed independently. Its `--help` output includes the selected build command.

Mantle's default package build at this revision reports four pre-existing producer test failures. Kiln does not claim Mantle repository or release readiness from this executable pin.

## Adapter behavior

- The typed `BuildPort` binds one effect, attempt, candidate, pipeline revision, plan, policy, platform, provider, and idempotency key.
- The shell verifies executable, plan, and native store-identity bytes before dispatch.
- Bounded Exec clears the environment and owns process-group timeout, cancellation, output bounds, and teardown.
- Successful completion requires every requested product exactly once.
- Cache and substitution facts cannot bypass product admission.
- Timeout, cancellation, provider error, and unknown remain distinct.
- Unknown observations have no terminal receipt and reconcile without redispatch.
- Log reads require the exact recorded report-bound identity and named byte bounds.

## Checks

- `cargo test --workspace`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo check -p kiln-core --target wasm32-unknown-unknown`: pass.
- `cargo fmt --all -- --check`: pass.
- Mantle adapter tests: 9 passed, including positive and negative cases.
- Positive typed Nickel profiles and authority fixtures: pass.
- Negative revision, capability, bound, and digest Nickel fixtures: rejected as required.
- Pinned producer fixture and contract byte comparison: pass.
- Independent wire consumer admission: pass.
- Tracey: 417 requirements, 417 referenced, no missing or dangling IDs.
- Tracey receipt: `8d09aa907d54925b7dca9b2835e7fd44cf620919fad73d3aa58de11cf9dd718f`.
- Cairn task gate: pass; receipt `c0f77f6eb7d7601ed3045a98b9c4f4a90705f0ac839e8c2ab68b4c301df8bac3`.
- Cairn archive: pass; receipt `3bc8f3c4e9473090e86a3a0b4b7954e752cd161afe3a2622b37534722007528b`.
- Post-archive validation and Tracey: pass.
- `nix flake check path:$PWD -L --option builders ''`: pass.
- Release evidence identity: `4d712e4ade8f4460dd0a297e281e67bb438085c871620da73dd48dd9cb4be28c`.

Kiln has no root Octet metadata profile. No Octet result is claimed.

## Boundaries and non-claims

`kiln-core` owns deterministic build linkage and completion admission. It imports no Mantle, Bounded Exec, filesystem, process, network, clock, or credential code.

`kiln-adapter-mantle` owns serialization, native-report mapping, bounded execution, file identity checks, and log reads.

Mantle retains derivation evaluation, build, cache, store, and native evidence authority. Kiln retains candidate truth, required-product meaning, CI state, retry policy, and release authority.

Passing checks do not prove build correctness, sandbox completeness, cache truth, product semantics, reproducibility, runtime availability, or release readiness.
