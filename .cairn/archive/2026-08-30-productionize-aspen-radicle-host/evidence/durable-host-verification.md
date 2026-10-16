# Durable Aspen host verification

Date: 2026-08-28

## Composition

The nested host lock resolves all Kiln protocol and adapter crates to published provider milestone `9ad8447f8025331917cf5c40ad956a8df40d225a`. It continues to pin Aspen `22f8ded26ca1907c29948e08b53f35df23080733`, Lattice contract `70496e67c7fd4a8b05914161a8e09de2759bebc8`, Bounded Exec `29dac88ecded94457572db3fdfaaaab95fa91525`, and Durable File Publication `8e05e74e24b45f752d77145c4455385daaf6d6ab`.

The imperative shell composes these concrete stores:

- a SQLite operation ledger with full synchronization and row BLAKE3 checks;
- Aspen's Redb `DurableNativeHostJournal`;
- capability-relative, no-replace callback value files with required parent synchronization.

## Ordering and recovery

Ingress reservation commits before native callback execution. The exact callback receipt and effect intent commit before Lattice dispatch. Terminal provider observations commit before completion delivery. Callback output publication can reconstruct a missing acknowledgement after a crash.

The ledger persists logical ticks. Restart restores the Aspen instance, callback state, trigger facts, effect intent, provider observation, acknowledgement, and uncertainty. Pending recovery uses Lattice `RecoverDispatch` and `Observe`; it never calls normal dispatch.

## Checks

- all-target, all-feature host tests: PASS;
- strict all-target, all-feature Clippy: PASS;
- direct Octet deny-all: PASS, zero findings;
- Nix host package with release-mode all-target and all-feature tests: PASS;
- Nix Octet deny-all: PASS, zero findings;
- accepted hosted callback, exact replay, and restart replay: PASS;
- accepted-but-unacknowledged Lattice recovery without dispatch: PASS;
- missing Lattice uncertainty retention: PASS;
- missing journal, changed marker, truncated ledger, and corrupt callback value: PASS;
- real Lattice and process fixture with 16 no-Lattice restart replays: PASS;
- full state-root backup and restore with byte-equal acknowledgement: PASS;
- corrupt copied SQLite ledger startup rejection: PASS.

The known Aspen malformed-Cargo fixture diagnostic remains benign: `unclosed table, expected \`]\'`.

## Claim boundary

The checks establish bounded machine-local recovery for the exact fixture cohort. They do not prove global durability, storage-device persistence, distributed exactly-once execution, workflow correctness, provider truth, host sandboxing, production availability, or release eligibility.
