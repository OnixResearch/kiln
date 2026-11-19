# Security policy

## Supported revisions

Security fixes are made against the current default branch. Reviewed release
tags may receive fixes when their release notes explicitly say so; no other
long-term-support promise is implied.

## Report a vulnerability

Use GitHub private vulnerability reporting for this repository:

<https://github.com/OnixResearch/basalt/security/advisories/new>

Do not include vulnerability details in a public issue, discussion, commit, or
pull request. Include the affected revision, reachable surface, reproduction
steps, expected impact, and any known mitigations in the private report. Avoid
including unrelated credentials, personal data, or production evidence.

Repository administrators must keep private vulnerability reporting enabled
before treating the repository as publicly supported. If the private reporting
interface is unavailable, a public issue may report only that the private
channel is unavailable; it must not contain vulnerability details.

## Scope

A report may concern the Rust authority core, UCAN enforcement adapters, Mantle
operation SDK, OIDC-to-UCAN exchange, durable-authority composition, Nostr
identity bindings, peer-authentication policy, runtime-effect permissions,
world-branch authority policy, the CLI shell, the Nickel policy artifacts, the
reviewed `vendor/ucan/` snapshot, or the build and release tooling.

A passing Basalt check is not a security proof. Each result remains limited to
its declared policy, token, request, and receipt identity. Basalt does not
provide key custody, key discovery, revocation freshness, durable replay
storage, transport security, deployment safety, or whole-system authority.
Reports about those consumer-owned boundaries are out of scope unless Basalt
claims a check it does not perform.
