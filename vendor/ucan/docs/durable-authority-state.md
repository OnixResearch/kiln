# Durable Authority State pilot

UCAN consumes two separate cores from canonical Radicle RID `rad:z3kGDT73ja8rewvGLwJFLF5MTx1ih`.
Cargo and Nix resolve exact object `a0de79303f3f45c18a34fff535b690a85cb3cf59` through the admitted HTTPS adapter.
There is no GitHub, sibling-path, branch, tag, or path fallback.

## Replay mapping

| UCAN value or owner | Shared role | Mapping |
|---|---|---|
| consumer | tenant | supplied at adapter construction |
| verified audience DID | actor | BLAKE3 with `ucan/durable-authority/actor/v1` |
| consumer operation profile | operation profile | supplied at adapter construction |
| holder canonical request digest | request | exact fixed digest |
| holder nonce | nonce | BLAKE3 with `ucan/durable-authority/nonce/v1` |
| consumer authority policy | generation | supplied positive generation |
| consumer persistence domain | ledger namespace | supplied at adapter construction |
| consumer protected effect | attempt | supplied at adapter construction |

`DurableHolderReplayAdmission` loads one slot and calls the pure shared consumption planner.
First use must apply the exact compare-and-swap reservation before the UCAN trait returns success.
The consumer selects commit-only or synchronized durability as an explicit minimum.

Exact duplicates and active reservations map to `Duplicate`.
Cancelled records map to `Stale`.
Conflicting nonce reuse maps to `Denied`.
Unavailable, uncertain, stale-write, failed, and unknown commit outcomes map to `Backend`.
Missing mapped fields map to `Malformed`.

UCAN still calls replay once after token, signature, profile, capability, and caveat checks.
UCAN performs no later work after replay succeeds.
The consumer owns downstream effect completion, commit, cancellation, and uncertain recovery.

## Revocation mapping

| UCAN value or owner | Shared role | Mapping |
|---|---|---|
| consumer registry selection | source | supplied at adapter construction |
| consumer freshness policy | minimum generation | supplied positive generation |
| caller clock observation | current time | supplied Unix seconds |
| proof reference | revocation entry | BLAKE3 with `ucan/durable-authority/revocation-entry/v1` |
| consumer verifier | authenticity | stored caller-supplied observation |
| registry ingestion | completeness | stored caller-supplied observation |

`DurableRevocationChecker` answers only from a current, complete, authenticated snapshot.
A revoked entry returns `true`, and an admitted absent entry returns `false`.
Temporal or generation failures map to `Stale`.
Wrong-source, partial, or unauthenticated views map to `Malformed`.
Missing or unavailable storage maps to `Backend`.

## Authority boundary

The shared cores do not verify tokens, signatures, keys, proof chains, capabilities, caveats, or operation profiles.
They do not grant authorization or execute protected effects.
They do not read clocks, select tenants, authenticate revocation data, set generations, or choose outage policy.

State digests and receipts do not prove global replay exclusion, revocation truth, backend honesty, effect completion, consumer compatibility, or release eligibility.
UCAN and its callers retain these decisions.
