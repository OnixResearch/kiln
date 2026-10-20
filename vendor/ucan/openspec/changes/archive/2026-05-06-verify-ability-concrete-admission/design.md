# Design

## Sibling primitive

Add a narrow executable helper in `/home/brittonr/git/verified-logic/src/ucan_ability.rs` that scans `Ability.segments` and returns whether any segment is the vendored glob segment. Its postcondition should connect the executable result to a spec predicate over `segments@.map_values(|s| s@)`.

Concrete admission is the negation of wildcard presence. The primitive is only over already segmented bytes; UCAN remains responsible for parsing/validating capability strings and constructing the segment vectors.

## UCAN integration

After committing/pushing sibling `verified-logic`, run `./scripts/sync-verified-logic.sh`, redact `vendor/verified-logic-revision.txt` source metadata, and replace UCAN-local iterator checks in `VerifiedCapability` with calls into the vendored primitive.

Keep public API shape stable: callers still use `VerifiedCapability::has_wildcard_segment()` and `VerifiedCapability::is_concrete()`.

## Guardrails

Update bridge coverage docs and `scripts/check-ucan-bridge.sh` so the live-path helper is classified and local shell reimplementation is rejected.
