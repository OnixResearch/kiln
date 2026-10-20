# Verify ability concrete/wildcard admission

## Motivation

`VerifiedCapability::has_wildcard_segment` and `VerifiedCapability::is_concrete` still compute wildcard/concrete ability facts in the UCAN shell adapter even though the facts are deterministic over already-validated ability segments. This leaves a small executable predicate in UCAN that should live in the sibling `verified-logic` UCAN ability module.

## Scope

- Add a sibling `verified_logic::ucan_ability` executable helper for wildcard-segment presence / concrete-operation admission over an already segmented ability.
- Sync the vendored snapshot into UCAN.
- Route `VerifiedCapability::{has_wildcard_segment,is_concrete}` through the vendored helper.
- Update tests, docs, coverage inventory, and bridge guard checks.

## Non-goals

- No new ability parser, resource parser, or capability semantic model.
- No claim about caller intent, authorization policy truth, proof-chain truth, caveat semantics, crypto, backend, clock, or token provenance.
- No change to wildcard syntax: `*` remains valid only as a full path segment and concrete operation requests must not contain wildcard segments.
