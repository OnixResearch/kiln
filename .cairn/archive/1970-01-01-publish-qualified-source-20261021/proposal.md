# Publish the self-CI-qualified source

## Why

The canonical Radicle `main` is `305f85d8...`. The self-CI-qualified source `c955334f...` is a fast-forward descendant that adds the real fork self-CI qualification, flake-alias task mapping, and native-host disconnect isolation. The operator requested publication (`begin`) after the publication recommendation.

## What Changes

- Fast-forward the canonical Radicle `main` from `305f85d8...` to `c955334f...`.
- Source-only: no GitHub mirror mutation, no governance/delegate/threshold change, no force push or history rewrite, no deployment, activation, binary release or release authority.
- Bind the source archive (`28fec0fa...`, 4,439 members), sorted source paths (`8001e6b5...`), hosting profile (`31c16d1a...`), license map (`a099bfe5...`), self-CI verification (`5cd60bda...`) and local gate results to the candidate.
- Record signed refs, governance, native Radicle and anonymous Garden HTTPS readbacks, publication receipt, rollback and non-claims; update README and hosting publication facts.

## Non-goals

- No claim of production eligibility, combined-binary distribution clearance, or release readiness beyond the recorded gates.
- No mirror, governance, announcement, delegate-threshold, or recovery mutation.
- No reuse of historical publication receipts as validation of this source.
