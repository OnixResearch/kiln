## Phase 1: Profile and admission

- [ ] [serial] Confirm the host-control wire-contract adoption scope, bind observation authority to the basalt contract with ucan enforcement, and record pins and third-party notices before the wire-format freeze. r[kiln.observe.http.profile]
- [ ] [serial] Define the bounded HTTP observation profile: registered origin, TLS policy, auth admission, content and SSE contracts, and named bounds. r[kiln.observe.http.profile]
- [ ] [serial] Add typed Nickel HTTP observation profiles with positive and negative profile fixtures. r[kiln.observe.http.profile]
- [ ] [parallel] Reject profile drift, unknown view kinds, and non-registered origins before serving. r[kiln.observe.http.profile] r[kiln.observe.http.serve]

## Phase 2: Serving shell

- [ ] [serial] Add `kiln-adapter-http-observe` as a shell that consumes the accepted projection unchanged. r[kiln.observe.http.serve]
- [ ] [depends:http-observe-profile] Serve admitted views as canonical JSON with their `kiln.observe.view.v1` identity. r[kiln.observe.http.serve]
- [ ] [parallel] Return bounded, specific rejections for unknown views, unknown targets, malformed requests, TLS failures, and timeouts. r[kiln.observe.http.serve]

## Phase 3: Live stream and auth

- [ ] [depends:http-observe-serving] Implement the bounded live event stream from committed journal events with a declared drop policy. r[kiln.observe.http.live]
- [ ] [depends:http-observe-live] Admit every request against the profile auth policy before projection. r[kiln.observe.http.auth]
- [ ] [parallel] Add stream-loss and auth-failure observations that never touch canonical state. r[kiln.observe.http.live] r[kiln.observe.http.auth]

## Phase 4: Conformance fixtures

- [ ] [serial] Add deterministic positive fixtures for admitted serving, identity-bearing views, live delivery, and authorized access. r[kiln.observe.http.conformance]
- [ ] [serial] Add negative fixtures for drift, unknown kinds, unknown targets, unauthorized requests, malformed requests, TLS failure, timeouts, and stream loss. r[kiln.observe.http.conformance]
- [ ] [parallel] Add authority-promotion and non-claim fixtures. r[kiln.observe.http.nonclaims]

## Phase 5: Evidence

- [ ] [serial] Document the origin, TLS, auth, content, live, loss, and non-claim contract. r[kiln.observe.http.profile] r[kiln.observe.http.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.observe.http.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.observe.http.conformance] r[kiln.observe.http.nonclaims]
