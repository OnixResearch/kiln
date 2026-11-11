## Why

The atomic `run` command provides one receipt file, but a reviewed run still
needs an operator to build profile/intent JSON from live Pijul state and arrange
upload. The existing `scripts/patchgate-review-job.py` already builds those
inputs from a pending inbox, but it called `plan` and `review` separately and
wrote the receipt non-atomically.

## What Changes

- Change the job script to invoke `kiln-patchgate-review run` once against the
  live profile/intent and read the atomically published receipt.
- Add `config/patchgate-review-job.ncl`, a typed worker `Step` builder for
  `config/web-worker.ncl` that runs the script from the checkout against an
  operator-provisioned Pijul target, declares `PATH`, and retains the receipt as
  a `review` report.
- Add positive and negative Nickel fixtures and include them in the focused
  Nickel check.
- Document the worker step and its source/report requirements.

## Impact

- **Files**: `scripts/patchgate-review-job.py`,
  `config/patchgate-review-job.ncl`, `config/fixtures/*patchgate-review-job*.ncl`,
  `flake.nix`, `docs/operator.md`.
- **Testing**: Python syntax/help, Nickel typecheck/export/negative/format, and a
  fake-native + fake-producer smoke through the real adapter `run` command.
- **Authority**: the step uploads review evidence only. It does not accept,
  merge, gate, release, or grant merge authority, and a missing receipt remains
  unavailable rather than passing. The current isolation policy cannot mount an
  external Pijul target, so the step is for an account-authority worker until a
  bounded target mount is added.
