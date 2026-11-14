## Why

Issue #58 needs an opt-in reviewer launch of one exact already-built Linux CLI artifact. A moving branch, unsigned cache payload or unqualified browser engine is not execution authority.

## What Changes

- Add provider-neutral, bounded descriptor admission from existing exact build observations and a complete runtime-only closure.
- Add a shell that verifies exact cached NARs/signatures and a selected immutable TryNix qemu-wasm site, exports only admitted runtime payloads, and enforces finite sessions and retention.
- Add a real review-page consumer and isolated Linux browser launcher using the existing TryNix boot/store/agent protocol, with separate discovery, hydration, boot and readiness observations.
- Fail closed on stale candidates, missing members, wrong hashes/signatures, unavailable engine and exhausted limits. Never mutate CI/review/merge evidence.

## Impact

Consumer: authenticated change reviewers and local qualification operators. Owner: Kiln web-service maintainers. Files: new core preview module; dedicated web preview shell/assets/example; code-review.js; focused tests and this native change. Shared registration/routes belong to integration owner. Verification: pure Rust and Node behavioral tests plus exact-built local browser cohort commands, coordinated by main agent. No deployment, remote mutation, build, cache publication or Redox/browser-VMM implementation.

## Out of Scope

No build latest HEAD, implicit trust of cache possession, production qualification, compiler, new VMM or repository-secret export.
