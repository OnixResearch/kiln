## Why

Kiln uses the legacy `cairn/` lifecycle directory. Current Cairn commands reject that legacy-only layout before validation.

## What Changes

- Copy the complete legacy lifecycle tree into `.cairn/` with Cairn's hash-bound migration command.
- Rebase the project policy on Cairn revision `695124d459574ba7aeba6097310d237f393c243c`.
- Update active repository paths to use `.cairn/` and remove the duplicate legacy tree.
- Preserve migration receipts and bounded validation evidence.

## Impact

- **Files**: lifecycle content, Cairn policy snapshots, documentation, and generated policy output
- **Testing**: migration parity, stale-plan rejection, policy freshness, strict Cairn validation, traceability, workspace tests, and Nix checks

## Out of Scope

This change does not alter accepted requirement meaning, CI behavior, provider authority, release authority, or historical evidence claims.
