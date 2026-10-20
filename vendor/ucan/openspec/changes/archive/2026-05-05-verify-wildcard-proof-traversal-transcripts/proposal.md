## Why

Live proof traversal transcript checking now covers no-proof tokens and single-parent chains only when the represented edge uses exact ability equality. UCAN shell semantics already permit wildcard ability attenuation such as `store/*` delegating `store/add`; those chains remain verified by ordinary shell checks but are not represented by the finite transcript primitive.

## What Changes

- Extend the vendored verified proof traversal transcript primitive to model UCAN slash-separated wildcard ability containment.
- Sync the verified-logic snapshot into UCAN.
- Remove the live verifier's exact-ability transcript restriction so eligible single-parent wildcard chains are bridge-checked.
- Keep multi-parent DAG folding, resolver/proof-store/revocation backends, compact parsing, signature crypto, and caveat policy semantics outside this change.

## Impact

- **Files**: `verified-logic/src/ucan_proof_traversal.rs`, `verified-logic/docs/requirements.md`, UCAN `vendor/`, `src/verified.rs`, `src/token.rs`, docs, tests, and bridge guard if needed.
- **APIs**: no new public UCAN API is required; existing proof traversal bridge semantics broaden from exact ability equality to verified wildcard ability containment.
- **Testing**: focused verified-logic proof/tests, UCAN focused transcript tests, full UCAN validation, OpenSpec validation, archive, commit, push.
