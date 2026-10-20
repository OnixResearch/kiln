# Protocol Profiles Evidence

This reference records the closed nominal protocol-profile catalog, the
pinned upstream sources, and the bounded conformance report for UCAN `1.0.0`.
It does not rewrite the catalog in `src/protocol_profile.rs`; that module is
the canonical core. This file preserves the review record and non-claims.

## Profiles

| Profile ID | State | Behavior |
|---|---|---|
| `onix-compact-ed25519-v1` | Supported | Three-part unpadded base64url compact token, Ed25519 EdDSA signature, JSON JWT-style claims, BLAKE3 proof references, repository resource and ability model |
| `ucan-wg-1.0.0` | Controlled-integration supported | Canonical DAG-CBOR, Ed25519 Varsig subset, CIDv1 SHA-256, bounded slash commands and policy, ordered CID-bound Delegation chains, exact Invocation admission, caller-owned replay, private live values, and redacted receipts |

No profile ID implies another profile. Unknown profile IDs fail closed and
select no decoder or authority evaluator.

The local compact profile is not labeled as final UCAN `1.0.0`. The final
profile is a separate bounded implementation with positive and negative
execution evidence. See [UCAN 1.0.0 Authorization](ucan-1-authorization.md)
for the orchestration and caller-owned boundaries.

## Pinned sources

| Role | Repository | Revision | Review basis |
|---|---|---|---|
| Normative overview and fixture source | `ucan-wg/spec` | `9955aa1fb7b32897f80b57651f4ee8b22ebf35a7` | Final overview describes DAG-CBOR, Varsig, CIDv1, and the payload model |
| Normative Delegation sub-specification | `ucan-wg/spec` | `1cb32dbc9d4d15a23bf9844a02515d760b81e816` | Delegation payload requirements |
| Normative Invocation sub-specification | `ucan-wg/spec` | `0f4c11325af6aca298b565cc19e90009d10e0197` | Invocation payload requirements |
| Implementation reference (unaudited) | `ucan-wg/rs-ucan` | `59ff9b9c8479b41072688d36709eb762ac9d8ecd` | README declares `1.0.0-rc.1`; behavior guidance only |

The fixture source path is `ucan-wg/spec` at the overview revision. A moving
source (branch, latest release page, unpinned URL) is rejected as
non-reproducible.

The normative final specifications control protocol requirements. `rs-ucan`
agreement cannot override them. Disagreements stay visible in the conformance
report.

## Pinned fixture manifest

`fixtures/1.0.0/manifest.json` binds each selected file to its source URL,
full revision, safe relative path, Git blob identity, BLAKE3 byte identity,
case class, and expected local status.

Inventory:

| File | Git blob | BLAKE3 | Class | Expected status |
|---|---|---|---|---|
| `delegation.json` | `99b84b7c0028edabcae5796bbd0d485f4b7b7b96` | `23ddd201dd206cebacfce06b181916519f5635aa645ab3c71db08703d8ca9db4` | positive | pass |
| `invocation.json` | `2f6e10c8a6016046ad79f0123aa5b6b176bdb2eb` | `01aeb9aa8f82530fd5bc177acd4401aeacd8d72d3301e3df34b78b54efb8c380` | positive | pass |
| `policy.json` | `8f291e0d400e323c3ecdcb91e90e722ca9ea6e46` | `8c2bdedee1be764ee7ef9e5e75fa138287c92979e43c53fc429889f37296e0bc` | positive and negative groups | pass |

Regenerate the manifest-derived report with:

```text
cargo run --locked --example protocol_profile_report
```

The example recomputes BLAKE3 and the Git blob identity of the local files,
validates every manifest entry, and writes `fixtures/1.0.0/report.json`.

## Conformance report

`fixtures/1.0.0/report.json` classifies every case as `pass`, `fail`,
`unsupported`, or `untested`. It binds the profile, implementation revision,
upstream revision, fixture identity, stage, reason code, and non-claims.

All three rows are `pass`. Delegation and Invocation execute canonical
DAG-CBOR, CID construction, and Ed25519 sign/verify round trips. Policy
executes every pinned positive and negative case through the bounded parser
and evaluator.

The pinned upstream `policy.json` has one invalid anonymous object wrapper at
lines 86 and 93. The report generator validates the pinned raw byte identity,
then removes only those two exact lines from its in-memory execution
projection. The `executed-with-pinned-json-normalization` reason code records
this differential. The pinned fixture bytes stay unchanged.

The upstream encoder binary layout is not yet re-derived byte-for-byte.
Therefore, the report does not claim upstream `token` or `cid` byte parity.
The local canonical form is deterministic and self-consistent. The pinned CID
validates as CIDv1 DAG-CBOR SHA-256.

## Non-claims

- Passing fixtures do not prove full interoperability or upstream byte parity.
- `rs-ucan` behavior is advisory, not authority.
- Callers own key truth, proof transport, executor identity, clock truth,
  revocation freshness, replay durability, and backend availability.
- Admission does not prove transport security, global freshness, external
  resource ownership, policy completeness, or effect correctness.
- AWAKE, Promise, registry clients, transport, remote resolution, and storage
  remain outside this bounded profile.
