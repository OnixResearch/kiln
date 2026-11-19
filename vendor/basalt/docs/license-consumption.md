# License Consumption Contract

Basalt uses the `AGPL-3.0-or-later` license. Some downstream consumers,
including Molten and Aspen, use different project licenses. This may be
acceptable. Basalt makes each downstream adoption explicit and reviewable
instead of implicit through Cargo resolution.

`basalt::license_consumption` provides a pure license-consumption contract. It
records a downstream consumer, the Basalt artifact identity, the license
expression, the reviewed license-metadata revision, the reviewer action, and a
non-claim boundary.

## What the contract does

The contract validates that a consumer acknowledges the AGPL boundary before
Basalt treats adoption evidence as complete. A receipt passes only when:

- the schema is `basalt.license-consumption-receipt.v1`
- the artifact identity is the canonical `basalt` package
- the license expression is `AGPL-3.0-or-later`
- the reviewed license-metadata revision is current
- the reviewer records an acknowledged review action
- the complete ordered non-claim boundary is preserved

A receipt fails closed when any of these facts is missing, wrong, stale, or
weakened. The failure returns deterministic issue codes.

## Claim boundary

An accepted receipt states one supported claim: `intentional-license-review`.

The receipt does not grant an alternate license, a compatibility promise, a
license exception, or legal advice. The assumed non-claim statement reads:

> This receipt records review metadata only. It is not legal advice, a license
> exception, or a compatibility promise.

The preserved non-claim boundary is:

```text
legal-advice
license-exception
compatibility-promise
alternate-license-grant
```

This documentation is not legal advice.

## Fixtures

Reviewed Nickel fixtures under `contracts/license-consumption/fixtures/` bind
the receipt shape. Positive and negative fixtures export to generated JSON:

- `positive/acknowledged-agpl.ncl` — acknowledged downstream AGPL consumption
- `negative/missing-review.ncl` — deferred review fails closed
- `negative/wrong-artifact-identity.ncl` — non-canonical artifact identity
- `negative/stale-license-metadata.ncl` — stale license-metadata revision
- `negative/weakened-non-claims.ncl` — incomplete non-claim boundary
- `negative/wrong-license-expression.ncl` — non-AGPL license expression
- `malformed/unsupported-schema.ncl` — rejected at the Nickel contract boundary

## Rust API

```rust
use basalt::license_consumption::{
    LicenseConsumptionReceipt, LicenseReviewAction, license_non_claim_statement,
    validate_license_consumption,
};

let receipt = LicenseConsumptionReceipt {
    schema: "basalt.license-consumption-receipt.v1".to_owned(),
    receipt_id: "example-adoption-001".to_owned(),
    consumer_repo: "github.com/example/app".to_owned(),
    artifact_identity: "basalt".to_owned(),
    license_expression: "AGPL-3.0-or-later".to_owned(),
    license_metadata_revision: "2026-07-31-license-metadata-v1".to_owned(),
    review_action: LicenseReviewAction::Acknowledged,
    non_claims: vec![
        "legal-advice".to_owned(),
        "license-exception".to_owned(),
        "compatibility-promise".to_owned(),
        "alternate-license-grant".to_owned(),
    ],
};

let verdict = validate_license_consumption(&receipt);
assert!(verdict.accepted);
```

## Non-claims

The license-consumption contract does not prove:

- that any project license is compatible with the AGPL
- that a downstream project has complied with the license
- that a reviewer action is accurate or current
- that a license warning or metadata check is complete

Adoption decisions remain with each consumer and its reviewers.
