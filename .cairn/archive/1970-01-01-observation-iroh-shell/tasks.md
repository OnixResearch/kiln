## Phase 1: Pinned cohort and profile

- [x] [serial] Pin the consumed `iroh` cohort at `0.95.1`, record the `kiln.observe.v1` ALPN identity, and record `iroh-blobs`/`bao-tree` as deferred behind the verified-payload port. r[kiln.observe.iroh.profile]
- [x] [parallel] Evaluate molten (aspen) as the transport home; adopt only on passing evidence, otherwise keep the pinned n0 cohort behind the transport port. r[kiln.observe.iroh.profile]
- [x] [serial] Add typed Nickel iroh observation profiles with positive and negative profile fixtures. r[kiln.observe.iroh.profile]
- [x] [parallel] Reject cohort drift, missing bounds, and unadmitted endpoints before serving. r[kiln.observe.iroh.profile] r[kiln.observe.iroh.serve]

## Phase 2: Serving shell

- [x] [serial] Add `kiln-adapter-iroh-observe` as a tokio shell that consumes the accepted projection unchanged. r[kiln.observe.iroh.serve]
- [x] [depends:iroh-observe-profile] Serve admitted views to peers over the `kiln.observe.v1` ALPN protocol with canonical payloads and identities. r[kiln.observe.iroh.serve]
- [x] [parallel] Classify transport failure as a bounded result that never claims a served view. r[kiln.observe.iroh.serve]

## Phase 3: Identity, trust, and verified streaming

- [x] [depends:iroh-observe-serving] Implement stable node identity, peer alias allowlist, and explicit revocation. r[kiln.observe.iroh.peer]
- [x] [depends:iroh-observe-serving] Align peer principals with capability principals (`did:key` Ed25519) under the basalt observation-authority contract. r[kiln.observe.iroh.peer]
- [x] [depends:iroh-observe-serving] Stream log and record ranges as content-addressed payloads over the bounded iroh stream, carrying their declared BLAKE3 payload identity. r[kiln.observe.iroh.verify]
- [x] [parallel] Verify declared BLAKE3 identities before claiming delivery, and reject or refetch corrupted payloads. r[kiln.observe.iroh.verify]
- [x] [parallel] Deny unknown and revoked peers under the declared policy. r[kiln.observe.iroh.peer]

## Phase 4: Equivalence rail

- [x] [depends:iroh-observe-safety] Assert equal logical views between the HTTP and iroh legs over shared fixtures. r[kiln.observe.iroh.equiv]
- [x] [parallel] Fail conformance when the legs diverge. r[kiln.observe.iroh.equiv]
- [x] [serial] Add relay-fallback observations and confirm they never change canonical state. r[kiln.observe.iroh.nonclaims]

## Phase 5: Conformance fixtures

- [x] [serial] Add deterministic positive fixtures for admitted serving, verified delivery, and HTTP/iroh equality. r[kiln.observe.iroh.conformance]
- [x] [serial] Add negative fixtures for cohort drift, unknown and revoked peers, corrupted payloads, transport failure, relay dependence, and divergence. r[kiln.observe.iroh.conformance]
- [x] [parallel] Add authority-promotion and non-claim fixtures. r[kiln.observe.iroh.nonclaims]

## Phase 6: Evidence

- [x] [serial] Document the cohort pins, ALPN, peer trust, verified streaming, equivalence rail, and non-claims. r[kiln.observe.iroh.profile] r[kiln.observe.iroh.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.observe.iroh.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.observe.iroh.conformance] r[kiln.observe.iroh.nonclaims]
