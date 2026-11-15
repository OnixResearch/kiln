# OpenBubbles iMessage Receiver Feasibility

Issue #71, Phase 1 only. No accounts were created, no OpenBubbles was built,
no activation performed, no messages sent.

## Pinned source cohort

| Source | Pinned commit | Public? | Role |
|---|---|---|---|
| rustpush | `b3c96c23e1aae78d1229ebaccb12027624f1eaff` | yes (SSPL v1 + OpenBubbles-only exception) | iMessage/Apple push library |
| openbubbles-app | `eed1b6332efbb17adbf5ebfa2263ad770169f75e` | yes (Apache-2.0) | Flutter client |
| OpenAbsinthe-Stub | `1f8dc73a311e7b4d94a868972a6816c8a2c14e44` | yes (stub) | Mac hardware validation |
| apple-private-apis | `b8598d2c1656ae73fbe0f1be1b18222ea7cadb0a` | yes (MPL-2.0) | anisette + iCloud auth |

Submodule state verified in rustpush: every submodule is uninitialized
(`open-absinthe`, `third_party/apple-private-apis`, `third_party/quinn`,
`third_party/rtc`, `third_party/rustls`, `third_party/rustls-platform-verifier`).

## Compatible cohort result

No established compatible cohort. The openbubbles-app `.gitmodules` entry pins
the `rustpush` submodule at `a7fab473e7a33325a760635285db2860de8e1cb0`, not the
issue-pinned `b3c96c23`. The two pinned commits do not form a tested
app/library pair.

## Component status

- Public: rustpush, openbubbles-app, OpenAbsinthe-Stub, apple-private-apis.
- Stubbed: OpenAbsinthe-Stub `src/nac.rs` panics in `from_validation_data`
  (line 87) and leaves the `ValidationCtx` methods as `todo!()` (lines 99, 103
  and 108). The working validation provider is not part of the public sources.
- Separately supplied: a remote anisette service (for non-macOS hosts) or a
  macOS host for AOSKit anisette; genuine Apple hardware identifiers; an Apple ID.

## Validation / anisette / hardware / account / credential / external-service requirements

- Anisette: macOS AOSKit (macOS host only) or remote anisette v3 / ClearADI.
  The `remote-clearadi` feature depends on an uninitialized `clearadi`
  submodule and cannot build from the pinned tree.
- Hardware: genuine Apple hardware identifiers (serial number, board ID,
  model) are sent to Apple for registration and periodic renewal
  ([renewal](https://openbubbles.app/docs/renewal.html)). Per the
  [quickstart](https://openbubbles.app/quickstart.html), a Mac cannot register
  an Android number, and an iPhone used for registration must stay online.
- Account: an Apple ID is required; credentials must be stored by the
  receiver's secret mechanism. None were created or stored for this evaluation.
- External services: Apple GSA/iMessage servers; optional remote anisette
  service.
- Renewal: registration renews periodically and Apple can force
  re-registration at any time
  ([renewal](https://openbubbles.app/docs/renewal.html)).

## Licensing decision

No-go for distribution or operation by Kiln under the pinned public sources.

- rustpush `LICENSE` is SSPL v1. Its `LICENSE.exceptions` grants a special
  exception only to OpenBubbles. Kiln is not OpenBubbles and cannot rely on it.
  Offering rustpush functionality as a service would trigger SSPL section 13
  service-source obligations for all supporting software.
- apple-private-apis is MPL-2.0. That alone is not the blocker: Kiln already
  distributes combined artifacts containing MPL-2.0 and AGPL-3.0-or-later
  components (see [the license map](../LICENSES.md)).
- OpenAbsinthe-Stub ships no license file, and the functional validation
  provider is closed.

## Evidence classes

| Class | Performed? |
|---|---|
| Build | No |
| Offline tests | No |
| Activation | No |
| Renewal | No |
| Live delivery | No |

## Decision

No-go. Phase 2 is not performed. The receiver contract in
`config/imessage-receiver.ncl` is provided for a future, separately licensed and
validated implementation and does not make OpenBubbles or rustpush a Kiln
dependency.

## Non-claims

This document is not an implementation, build, activation, or delivery. It does
not create accounts, build OpenBubbles, or send messages. It does not grant
merge or release authority.
