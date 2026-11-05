# Verification evidence

## Result

Defelo patch events now preserve their exact native patch ID in the typed
trigger and reject patch events without one. The first patch commit remains the
exact Kiln revision. Existing push mapping is unchanged.

## Evidence

- All 50 Radicle adapter library tests pass.
- Formatting passes for the changed crate.
- `cairn validate --root .` reports zero issues.

## Non-claims

This does not implement patch source acquisition, author-signature trust,
approval gating, or patch runs. The patch ID is producer-owned identity and
grants no CI or review authority. No provider or runtime effect was exercised.
