# Verification evidence

## Result

Firecracker receipts now include the observed host preflight facts, and
untrusted dispatch refuses unsafe facts unless a reviewed runner supplies a
non-empty override. The override is retained separately in the receipt while
the unsafe observations remain visible.

## Evidence

- All 62 hosted library tests pass.
- All 33 microVM library tests pass.
- Workspace `--all-targets` check passes.
- Typed hosted Nickel profiles typecheck and format.
- `cairn validate` reports zero issues.

## Non-claims

Host facts are observations, not a patched-host proof. Trust-domain scheduling,
core/cache partitioning and covert-channel measurement remain open. The override
is operator authority, not evidence of isolation.
