# Context

`crates/kiln-adapter-webhook/src/lib.rs` defines `WEBHOOK_BODY_SCHEMA =
"kiln.webhook-notification.v1"` and the `WebhookProfile`/`AuthPolicy`/network
policy machinery. `crates/kiln-web-service/src/automation.rs` and
`code_review/delivery.rs` consume that payload and enforce destination identity,
address admission, and BLAKE3 body identity. A receiver would be a new
destination class, not a change to these types.

Pinned sources inspected:
- rustpush `b3c96c23e1aae78d1229ebaccb12027624f1eaff` (SSPL v1, plus an
  OpenBubbles-only exception in `LICENSE.exceptions`)
- openbubbles-app `eed1b6332efbb17adbf5ebfa2263ad770169f75e` (Apache-2.0)
- OpenAbsinthe-Stub `1f8dc73a311e7b4d94a868972a6816c8a2c14e44` (stub only,
  `todo!()`/`panic!()` implementation)
- apple-private-apis `b8598d2c1656ae73fbe0f1be1b18222ea7cadb0a` (MPL-2.0)
- OpenBubbles quickstart and renewal documentation

## Decision

Feasibility result: **no-go for a Kiln-distributed or Kiln-operated receiver
under the pinned public sources.** Phase 2 is not performed.

Reasons:
1. No compatible cohort: openbubbles-app pins rustpush at `a7fab473`, while the
   issue pins rustpush at `b3c96c23`; the two do not establish a tested
   compatible app/library pair at the pinned commits.
2. OpenAbsinthe is not publicly available: only a stub is public, and its
   validation functions panic/`todo!()`. Mac hardware validation cannot be
   performed from public sources.
3. Anisette validation requires either macOS AOSKit (macOS host) or a remote
   anisette service; the `remote-clearadi` path depends on an uninitialized
   `clearadi` submodule. Credential storage and an Apple ID account are
   required and were not created.
4. Licensing: rustpush is SSPL v1 with an exception granted only to OpenBubbles.
   Kiln is not OpenBubbles and cannot rely on that exception; distributing or
   offering rustpush functionality would trigger SSPL Section 13 service-source
   disclosure. apple-private-apis is MPL-2.0, so a combined artifact is not
   permissive-only, conflicting with Kiln's default `MIT OR Apache-2.0` posture.
5. Hardware/account/renewal requirements: genuine Apple hardware identifiers
   (serial, board ID, model, etc.) are required for activation and periodic
   renewal; a Mac cannot register an Android number.

Evidence classes and performance status:
- Build: not performed (no build of OpenBubbles/rustpush attempted).
- Offline tests: not performed.
- Activation: not performed (no account created, no hardware validation).
- Renewal: not performed.
- Live delivery: not performed (no message sent).

The receiver contract is still defined so a future, separately licensed and
validated implementation could conform without changing Kiln's notification
payload.

## Verification

Typed Nickel contract in `config/imessage-receiver.ncl` with positive and
negative fixtures under `fixtures/imessage-receiver/`. BLAKE3 is named as the
Kiln identity algorithm; Apple-native identifiers are carried as opaque
strings. The message definition permits only run ID, observed outcome, and a
configured trusted run URL.

## Non-claims

This change is not an implementation, not a build, not an activation, and not a
delivery. It does not create an account, does not send messages, and does not
grant merge or release authority. The contract does not make OpenBubbles or
rustpush a Kiln dependency.
