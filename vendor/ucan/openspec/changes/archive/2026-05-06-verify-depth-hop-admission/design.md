## Design

The sibling `verified-logic` module `ucan_delegation_depth` already models chain length and budget admission. This change adds a narrow runtime helper for the shell representation:

- shell input `depth` means proof hops from the leaf token;
- verified budget input `chain_len` means finite chain length with leaf-only length one;
- `depth + 1` overflow is rejected as not representable as a finite executable chain length;
- successful conversion delegates to `check_depth_budget`.

UCAN keeps the existing public `proof_depth_within_budget(depth, max_depth) -> bool` API and simply routes its implementation through the vendored helper. The bridge guard rejects reintroduced local checked-add translation in `src/verified.rs`.
