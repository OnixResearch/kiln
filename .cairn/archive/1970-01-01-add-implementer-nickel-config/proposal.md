## Why

The operator command consumed hand-authored JSON. Policy and enrollment are
human-authored, reviewable defaults, so they belong in typed Nickel that bounds
acceptance, edit scope, effects, budget and enrollment before export.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `config/implementer-policy.ncl`, a typed builder for one intake policy.
- Add `config/implementer-enrollment.ncl`, a typed builder for unique
  repositories and actors.
- Add positive and negative fixtures.
- Extend the Nickel check with typecheck, positive export and negative refusal.
- Document the authoring flow.

## Impact

- **Files**: `config/implementer-policy.ncl`,
  `config/implementer-enrollment.ncl`, `config/fixtures/implementer-*.ncl`,
  `flake.nix`, `docs/implementer-service.md`.
- **Testing**: Nickel typecheck, positive export, negative path/enrollment
  fixtures in the flake check.
- **Authority**: configuration only; exported JSON still passes Rust admission
  and the durable ledger.
