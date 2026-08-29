# Verification evidence

Date: 2026-08-29

## Live defect

Onix activated Aspen route closure `/nix/store/6abs8ks8fd4bs928za37xw8p3ddjqir1-nixos-system-britton-desktop-26.11.20260819.afe3d8a`. The broker config named the explicit Aspen adapter and had an empty environment.

Canonical Seaglass commit `ee03ebc2d4315f8c1931767c19e17083aa125549` produced one real default-branch broker event. Broker run `5175bab4-cac9-467a-8d2d-b19eeb5e29ce` passed repository, `flake.nix`, and default-branch filters. The adapter then exited with code `2` and diagnostic:

`radicle_json: broker input is not a supported deployed broker request`

No Aspen host operation or provider report was created. The empty queue boundary allowed immediate explicit rollback to legacy closure `/nix/store/q89zgccfjbr5r7j4g73x0mi7nfg0v82p-nixos-system-britton-desktop-26.11.20260819.afe3d8a`.

## Root cause

Radicle CI broker `0.31.0` publishes `trigger.json` with request kind `trigger`, version `1`, full repository metadata, pusher, `before`, `after`, branch, and commits. Kiln's synthetic fixture omitted request kind, version, and producer metadata. Its parser also rejected every unknown field.

The producer source commit is `88076f8679e3114e30975aa824bf75d83caaca85`. The fixture BLAKE3 is `527431291b5da59b39aeb9ea6e8314367dd4ce352938ee865c7444df4f7b485b`.

## Repair

Kiln now requires request kind `trigger` and protocol version `1`. It accepts producer-owned descriptive metadata but projects only event kind, repository ID, first push or patch commit, and branch. The mapped actor remains the explicit `broker` boundary, and push `before` remains absent because broker `0.31` can project the tip as both `before` and `after`.

The process shell now admits protocol input before it reads the runtime profile, observes a socket, opens replay state, or constructs Aspen or Lattice transport. Invalid input therefore returns its protocol diagnostic without backend effects.

## Verification

- `cargo fmt --all -- --check`: PASS.
- `cargo test -p kiln-adapter-radicle --all-targets --all-features`: PASS, including 39 library tests, 2 binary tests, and positive and negative broker-wire cases.
- `cargo test --workspace --all-targets --all-features --exclude kiln-adapter-pijul`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `nix build path:$PWD#packages.x86_64-linux.kiln --no-link -L --builders ''`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L --builders ''`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.radicle-broker-wire-octet-deny-all --no-link -L --builders ''`: PASS with the pinned full Octet catalog, workspace scope, all targets, and all features. The check copies the exact production `defelo.rs` and tests into a focused workspace. It reported zero findings.
- Strict Cairn validation and proposal, design, and tasks gates: PASS.

Repository-wide traceability coverage still fails on unrelated accepted-spec gaps. The two new scenario markers remain dangling until this change is synced. No complete traceability claim is made before sync.

## Non-claims

Fixture admission does not prove broker availability, source trust, provider truth, CI correctness, status publication, distributed exactly-once execution, cutover readiness, or release eligibility. A second cutover requires all static gates and another real event.
