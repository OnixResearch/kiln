## Phase 1: Investigation

- [x] [serial] Write `docs/tangled-integration.md` from the Tangled documentation and core source, with sources, dates and a "Not established" list. r[kiln.tangled.note] r[kiln.tangled.note.scenario.unverified]

## Phase 2: Pure core

- [x] [serial] Add identity validation, profile admission and the typed Nickel profile with positive and negative fixtures. r[kiln.tangled.identity] r[kiln.tangled.profile] r[kiln.tangled.profile.scenario.invalid]
- [x] [serial] Add service-auth JWT verification (ES256K and ES256), DID-document key parsing and webhook HMAC verification. r[kiln.tangled.auth] r[kiln.tangled.auth.scenario.bad-signature]
- [x] [serial] Add notifyEvent, manual and pull request event admission and replay keys. r[kiln.tangled.events] r[kiln.tangled.events.scenario.malformed] r[kiln.tangled.identity.scenario.unknown-repo]
- [x] [serial] Add the reviewed workflow mapping and `when` evaluation. r[kiln.tangled.workflows] r[kiln.tangled.workflows.scenario.unsupported]
- [x] [serial] Add pipeline planning, Kiln runs, status mapping, approval and retry. r[kiln.tangled.source.scenario.exact] r[kiln.tangled.status] r[kiln.tangled.untrusted] r[kiln.tangled.untrusted.scenario.held]

## Phase 3: Shell

- [x] [serial] Add the SQLite replay store, the Git CLI exact acquisition, the HMAC status publisher and the DID resolver. r[kiln.tangled.replay] r[kiln.tangled.replay.scenario.push] r[kiln.tangled.source]
- [x] [serial] Add the XRPC router, the service and the bounded HTTP server. r[kiln.tangled.xrpc] r[kiln.tangled.xrpc.scenario.retry] r[kiln.tangled.status.scenario.published]

## Phase 4: Verification

- [x] [serial] Run focused fmt, clippy and tests, the end-to-end loopback fixture and the Nickel fixtures; write the evidence. r[kiln.tangled.note] r[kiln.tangled.identity] r[kiln.tangled.auth] r[kiln.tangled.events] r[kiln.tangled.replay] r[kiln.tangled.source] r[kiln.tangled.workflows] r[kiln.tangled.xrpc] r[kiln.tangled.status] r[kiln.tangled.untrusted] r[kiln.tangled.profile]
