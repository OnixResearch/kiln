## Why

Kiln self-CI has a working single-tenant HTTPS application but lacks a maintained deployment contract. Operators need an independently installable package, preflight/readiness, private runtime credentials, supervised persistent operation and a repeatable native qualification that no longer fails cancellation.

## What Changes

- Ship a dedicated web-service package and a hardened deployment definition using existing OnixOS service-module and systemd conventions; no bespoke deployment control plane.
- Add read-only startup preflight, readiness notification and minimal HTTPS health, retaining bounded shutdown and exact durable recovery.
- Admit read-only private runtime credential files without admitting group/world access; enforce bounded private TLS-key reads.
- Correct native cancellation classification in the owning Lattice producer, record its lifecycle and select an exact measured runtime cohort for consumer evidence.
- Exercise actual supervised HTTPS operation, negative configuration/credential admission, stop/restart and the complete native success/failure/log/artifact/control rail.

## Impact

- **Consumer and owner:** Kiln self-CI; Kiln application/dashboard maintainers own deployment assets, Lattice maintainers own producer cancellation, and OnixOS retains target assembly/activation authority.
- **Files:** service shell/profile admission and tests, deployment assets and examples, flake exports/checks, native exercise, operator documentation and scoped receipts; producer corrections remain repository-local to Lattice.
- **Non-goals:** no actual host deployment, remote publication, lifecycle acceptance/archive, hosted accounts, arbitrary-code sandbox claims, automated state migration, retention pruning or invented provider outcomes.
- **Repeatability:** immutable package/profile/source identities plus real native and service-manager observations; failed checks remain failures.
