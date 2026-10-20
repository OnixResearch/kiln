# Aeneas `TimeBounds` exact-source pilot

## Result

The pilot is **blocked before theorem work**. It does not report an Aeneas or
Lean proof for `TimeBounds`.

The selected production source is
`crates/ucan-core/src/token.rs` at BLAKE3
`001764bbd4036024de07cb4565fe5044d17531d5e1dac51523efedadbecc0022`.
The intended compiled items are exactly:

- `ucan_core::token::{TimeBounds}::new`;
- `ucan_core::token::{TimeBounds}::not_before_seconds`; and
- `ucan_core::token::{TimeBounds}::expires_at_seconds`.

No copied, simplified, non-const, or proof-only constructor is admitted.
`proofs/aeneas-time-bounds/fixtures/invalid/source-drift.rs` is retained only as
a rejection fixture.

## Reviewed providers

`proofs/aeneas-time-bounds/provider-identity.ncl` pins:

- Octet revision `ef18352bf579bb7c6eb39070d917e3d005385a61`;
- package `octet#octet-aeneas-lean-provider-closure`;
- Octet profile `octet-aeneas-lean-0.1.0-bc2eb6f`, profile BLAKE3
  `cd11b49d1acede2c6a91fa26f0b7a6b4b28db4007b4601a65b3269706a0ce625`;
- Aeneas revision `bc2eb6f941369fa3c695e30598b5758b11e4f0de` and Charon revision
  `cb50ff16b9f1066b8a97dc06da704de2da2fa41c`;
- Octet theorem-adequacy, compatibility-matrix, and Valence-adapter identities;
  and
- Valence revision `8eccac35fcd8aae8ef64b3d1021e7f1a53656180` with profile
  `aeneas-lean/v1`.

UCAN does not vendor, reconstruct, or ambiently discover these providers.

## Exact blocker

The pinned Charon provider compiled the unmodified `ucan-core` crate with
borrow checking preserved and network access denied by bubblewrap. Its exact
selection attempt used the three inherent methods as `--start-from` roots.
Charon rejected each root with:

```text
`--start-from` does not support inherent impls
```

The stable classification is
`charon-start-from-inherent-impl-unsupported` at stage
`charon-item-selection`. The bounded diagnostic is
`proofs/aeneas-time-bounds/evidence/exact-item-selection.log`.

A supplementary `--start-from-pub` probe confirmed that Charon can compile the
production crate and that its LLBC contains all three intended items. That LLBC
was broader than the approved selection, so it is not property evidence.
Pinned Aeneas then raised `Aeneas.Errors.CFailure` while translating the broader
public method surface. The probe is recorded in
`proofs/aeneas-time-bounds/evidence/broad-public-probe.log`; its LLBC was not
retained.

These observations do **not** establish that `const fn`, `Result`, the UCAN
error enum, struct construction, or either getter is unsupported. The exact
selection gate failed before a three-item `translation.json` could exist.
Consequently:

- no generated Lean model is admitted;
- no maintained, false, or vacuous theorem path is selected;
- no theorem map or Octet property receipt is emitted;
- Octet artifact verification is not applicable; and
- no Valence property chain or downstream property handoff is emitted.

## Next discriminating check

Rerun the same production methods with a reviewed Charon provider that supports
inherent impl methods as exact `--start-from` roots, or separately approve
`start_from` attributes on the production methods. Do not begin theorem work
until a non-errorful three-item LLBC and measured `translation.json` bind the
exact Rust items to generated Lean declarations and files.

## Durable evidence and validation

The typed sources of truth are:

- `proofs/aeneas-time-bounds/provider-identity.ncl`;
- `proofs/aeneas-time-bounds/manifest.ncl`;
- `proofs/aeneas-time-bounds/negative-cases.ncl`; and
- `proofs/aeneas-time-bounds/blocked-receipt.ncl`.

Committed JSON under `proofs/aeneas-time-bounds/generated/` is regenerated from
those Nickel sources. The checker validates byte freshness, exact source spans
and markers, BLAKE3 identities, provider and target metadata, non-errorful and
borrow-check-preserving extraction settings, bounded diagnostics, run-local
`def_id` policy, negative-case coverage, runtime separation, and blocked-status
non-claims:

```bash
nix develop -c cargo run -q --bin check-aeneas-time-bounds-proof -- \
  check --root . --nickel nickel
```

The lane is development/CI evidence only. `ucan-core` remains `#![no_std]` and
has no Aeneas, Charon, Lean, Nickel, process, filesystem, clock, network, or
receipt-loading dependency.

## Non-claims

This blocked receipt proves none of the intended constructor/getter properties.
It does not prove token validity, clock correctness, parser or codec behavior,
cryptographic correctness, proof-chain soundness, caveat/revocation/replay
freshness, authorization completeness, interoperability, whole-program
correctness, or whole-system security. BLAKE3 identifies observed bytes; it
does not prove Charon, Aeneas, Lean, model, translation, or verifier soundness.
