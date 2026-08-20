## Phase 1: Profile contract

- [x] [serial] Define endpoint, HTTP cohort, authentication, signature, network, retry, redaction, and non-claim boundaries. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [x] [serial] Add typed Nickel endpoint profiles with named request, response, redirect, deadline, and retry bounds. r[kiln.webhook.profile]
- [x] [parallel] Add valid profiles and negative dynamic-destination, blocked-range, redirect, secret, and zero-bound fixtures. r[kiln.webhook.conformance]

## Phase 2: Pure adapter core

- [x] [serial] Implement pure notification body projection and delivery identity linkage. r[kiln.webhook.request]
- [x] [depends:webhook-request-core] Implement pure destination, response, retry, ambiguity, and reconciliation decisions. r[kiln.webhook.outcome]
- [x] [parallel] Add positive and negative pure-core tests. r[kiln.webhook.conformance]

## Phase 3: HTTP shell

- [x] [depends:webhook-profile] Add `kiln-adapter-webhook` outside core dependency graphs. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [x] [depends:webhook-outcome-core] Implement bounded DNS, network admission, TLS, authentication, request, response, timeout, and reconciliation behavior. r[kiln.webhook.request] r[kiln.webhook.outcome]
- [x] [parallel] Add redaction and secret-non-disclosure checks. r[kiln.webhook.conformance] r[kiln.webhook.nonclaims]

## Phase 4: Evidence

- [x] [serial] Add deterministic local receivers for positive and negative transport cases. r[kiln.webhook.conformance]
- [x] [serial] Document endpoint setup, network authority, retry, ambiguity, and non-claims. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.webhook.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.webhook.conformance] r[kiln.webhook.nonclaims]
