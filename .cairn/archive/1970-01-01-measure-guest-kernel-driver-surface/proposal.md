## Why

The guest uses the pinned NixOS-default kernel, and the escape suite only
observes runtime device state. Nothing measured whether unused device classes
are still compiled into the guest `vmlinux` or shipped in its module closure.

Issue: [#40](https://github.com/OnixResearch/kiln/issues/40).

## What Changes

- Add a reviewed deny list in `config/microvm-guest-surface.ncl` with enforced
  and measured-only classes, exact Kconfig symbols and module fragments.
- Add `scripts/microvm-guest-surface.py`, which classifies each class as
  `absent`, `builtin` or `loadable_module_present` from the pinned kernel
  configuration and the guest module closure, and writes a bounded inventory.
- Add `nix build .#microvm-guest-surface` (also a flake check) that measures the
  built guest cohort, fails when an enforced class is compiled in or shipped as
  a module, and compares the inventory with the checked-in fixture.
- Check in `fixtures/microvm/guest-driver-surface.json` for the pinned guest
  nixpkgs revision and kernel version.
- Extend the guest escape suite so enforced classes' sysfs entries must be
  absent, failing closed.
- Document the measured verdicts and residual risk.

## Impact

- **Files**: `config/microvm-guest-surface.ncl`,
  `scripts/microvm-guest-surface.py`, `fixtures/microvm/guest-driver-surface.json`,
  `flake.nix`, `crates/kiln-hosted/tests/microvm.rs`, `docs/hosted-ci.md`.
- **Testing**: `nix build .#checks.x86_64-linux.microvm-guest-surface` measures
  the real pinned guest image and fails on enforced-class presence or fixture
  drift; the escape-suite harness compiles with the new probes.
- **Authority**: measurement only. USB core and display support are recorded
  builtin misses, not denied; a slimmed-kernel redesign is separate work.
  `hostile_isolation_claim` stays `false`.
