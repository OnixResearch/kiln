# Why

Issue #71 asks whether an optional OpenBubbles iMessage receiver can be used for
Kiln CI notifications. The receiver would consume the existing
`kiln.webhook-notification.v1` payload and deliver a short CI notification over
iMessage. Before any implementation, the compatible public source cohort,
validation/anisette and hardware/account requirements, and licensing must be
established.

## What Changes

- Publish a feasibility decision (no-go for a distributed/operated Kiln receiver
  under the pinned public sources) and the evidence classes that were and were
  not performed.
- Define a versioned receiver contract (`imessage-receiver`) that consumes the
  existing `kiln.webhook-notification.v1` payload, with endpoint
  authentication and destination policy inherited from the existing webhook
  profile contract, BLAKE3 Kiln identities, and Apple-native identifiers
  preserved as opaque values.
- Define the message body from authenticated available fields only: run ID,
  observed outcome, and a configured trusted run URL. A moving branch head is
  never part of the message.
- Provide typed Nickel positive and negative fixtures for the contract.

## Impact

Documentation, one typed Nickel profile/contract, and fixtures only. No Rust
changes, no accounts, no activation, no OpenBubbles build, no messages sent.
No provider, source or lifecycle authority is created.

## Authority

Feasibility and contract work only. Operator authorization is still required for
any future activation, deployment, or message delivery.
