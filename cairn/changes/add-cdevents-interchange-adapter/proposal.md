# Add a CDEvents interchange adapter

## Why

Kiln models CDEvents and CloudEvents admission, but it has no concrete encoder, decoder, or transport-neutral adapter crate.

A versioned adapter can exchange CI observations while preserving Kiln event and identity authority.

## What Changes

- Add `kiln-adapter-cdevents` for bounded CDEvents import and export.
- Pin supported CDEvents, subject, event, and CloudEvents binding versions.
- Map each supported type through explicit pure conversions.
- Preserve native event, source, subject, correlation, causation, timestamp-observation, and extension facts.
- Reject unknown mandatory fields, unsupported types, missing links, duplicates, and undeclared lossy mappings.
- Keep imported success and failure as external observations.
- Add typed Nickel profiles, canonical fixtures, and independent round-trip tests.
- Keep transport outside the initial adapter unless a separate profile declares it.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted interchange-projections contract.

## Impact

- **Adapters**: adds `kiln-adapter-cdevents`.
- **Core**: reuses provider-neutral interchange admission.
- **Configuration**: adds version, event, field, loss, bound, and transport-neutral profiles.
- **Testing**: covers exact mappings, duplicates, correlations, schema drift, and authority promotion.
