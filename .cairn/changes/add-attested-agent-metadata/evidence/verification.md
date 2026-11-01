# Verification evidence

## Result

Attested agent revisions and submission receipts now store the exact principal
plus optional declared harness/model metadata. Review profile metadata is bounded and restricted to
declared agents; access declarations merge with deduplication and conflict
rejection. The review Nickel contract exports both optional agent fields instead
of silently dropping them, and the identity view shows the declared identity
beside the attested principal.

## Evidence

- Core tests cover structured attestation, legacy bare-principal
  deserialization and human-byte compatibility.
- Review profile tests cover metadata admission, merge deduplication and conflict
  rejection.
- `check` and `serve` share one review-profile loader, so startup validation
  catches access/review agent metadata conflicts and non-writer agents before
  listener creation.
- Full core and web-service suites pass.
- Clippy, rustfmt, JavaScript syntax, Nickel typecheck/export and Cairn
  validation pass.

## Non-claims

Harness/model are operator-declared configuration, not verified execution
metadata. Signed per-request agent credentials remain future work. Historical
revisions are not backfilled, and metadata grants no review, execution or merge
authority.
