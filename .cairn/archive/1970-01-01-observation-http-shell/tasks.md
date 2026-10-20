## Phase 1: Profile and admission

- [x] [serial] Confirm the host-control adoption scope, define the versioned observation-authn port and Ed25519 verifier, and record the basalt/UCAN source, license, and third-party-notice prerequisite. r[kiln.observe.http.profile]
- [x] [serial] Record the basalt/UCAN import as a deferred follow-up: an exact pinned cohort, license map, third-party notices, and runtime review remain prerequisites; the Ed25519 request verifier is the admitted reference backend. r[kiln.observe.http.auth]
- [x] [serial] Define the bounded HTTP observation profile: registered origin, TLS policy, auth admission, content and SSE contracts, and named bounds. r[kiln.observe.http.profile]
- [x] [serial] Add typed Nickel HTTP observation profiles with positive and negative profile fixtures. r[kiln.observe.http.profile]
- [x] [parallel] Reject profile drift, unknown view kinds, and non-registered origins before serving. r[kiln.observe.http.profile] r[kiln.observe.http.serve]

## Phase 2: Serving shell

- [x] [serial] Add `kiln-adapter-http-observe` as a shell that consumes the accepted projection unchanged. r[kiln.observe.http.serve]
- [x] [depends:http-observe-profile] Serve admitted views as canonical JSON with their `kiln.observe.view.v1` identity. r[kiln.observe.http.serve]
- [x] [parallel] Return bounded, specific rejections for unknown views, unknown targets, malformed requests, TLS failures, and timeouts. r[kiln.observe.http.serve]

## Phase 3: Live stream and auth

- [x] [depends:http-observe-serving] Implement the bounded live event stream from committed journal events with a declared drop policy. r[kiln.observe.http.live]
- [x] [depends:http-observe-live] Admit every request against the profile auth policy before projection. r[kiln.observe.http.auth]
- [x] [parallel] Add stream-loss and auth-failure observations that never touch canonical state. r[kiln.observe.http.live] r[kiln.observe.http.auth]

## Phase 4: Conformance fixtures

- [x] [serial] Add deterministic positive fixtures for admitted serving, identity-bearing views, live delivery, and authorized access. r[kiln.observe.http.conformance]
- [x] [serial] Add negative fixtures for drift, unknown kinds, unknown targets, unauthorized requests, malformed requests, TLS failure, timeouts, and stream loss. r[kiln.observe.http.conformance]
- [x] [parallel] Add authority-promotion and non-claim fixtures. r[kiln.observe.http.nonclaims]

## Phase 5: Evidence

- [x] [serial] Document the origin, TLS, auth, content, live, loss, and non-claim contract. r[kiln.observe.http.profile] r[kiln.observe.http.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.observe.http.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.observe.http.conformance] r[kiln.observe.http.nonclaims]
