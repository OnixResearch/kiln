## Phase 1: Profile contract

- [ ] [serial] Define endpoint, HTTP cohort, authentication, signature, network, retry, redaction, and non-claim boundaries. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [ ] [serial] Add typed Nickel endpoint profiles with named request, response, redirect, deadline, and retry bounds. r[kiln.webhook.profile]
- [ ] [parallel] Add valid profiles and negative dynamic-destination, blocked-range, redirect, secret, and zero-bound fixtures. r[kiln.webhook.conformance]

## Phase 2: Pure adapter core

- [ ] [serial] Implement pure notification body projection and delivery identity linkage. r[kiln.webhook.request]
- [ ] [depends:webhook-request-core] Implement pure destination, response, retry, ambiguity, and reconciliation decisions. r[kiln.webhook.outcome]
- [ ] [parallel] Add positive and negative pure-core tests. r[kiln.webhook.conformance]

## Phase 3: HTTP shell

- [ ] [depends:webhook-profile] Add `kiln-adapter-webhook` outside core dependency graphs. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [ ] [depends:webhook-outcome-core] Implement bounded DNS, network admission, TLS, authentication, request, response, timeout, and reconciliation behavior. r[kiln.webhook.request] r[kiln.webhook.outcome]
- [ ] [parallel] Add redaction and secret-non-disclosure checks. r[kiln.webhook.conformance] r[kiln.webhook.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add deterministic local receivers for positive and negative transport cases. r[kiln.webhook.conformance]
- [ ] [serial] Document endpoint setup, network authority, retry, ambiguity, and non-claims. r[kiln.webhook.profile] r[kiln.webhook.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.webhook.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.webhook.conformance] r[kiln.webhook.nonclaims]
