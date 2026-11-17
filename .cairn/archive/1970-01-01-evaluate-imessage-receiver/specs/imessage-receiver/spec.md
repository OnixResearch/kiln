# iMessage Receiver Delta

## ADDED Requirements

### Requirement: Receiver feasibility

r[kiln.imessage.receiver] Kiln MUST NOT bundle, distribute, or operate an
OpenBubbles iMessage receiver unless every prerequisite recorded in
`docs/imessage-receiver-feasibility.md` is satisfied. A documented no-go is a
valid result of this evaluation.

### Requirement: Receiver contract

r[kiln.imessage.receiver.contract] A receiver, if ever implemented, MUST consume the
existing `kiln.webhook-notification.v1` payload without changing its schema or
Kiln-owned BLAKE3 identities. Its destination MUST satisfy the existing webhook
profile contract (HTTPS, POST, JSON, redirects disabled, no inline secrets,
positive bounds). Its message MUST be built only from authenticated available
fields: run ID, observed outcome, and a configured trusted run URL. A moving
branch head MUST NOT appear in the message.

#### Scenario: Feasible prerequisites

- GIVEN all pinned sources, licenses, hardware, account, and validation
  requirements are satisfied
- WHEN a receiver is proposed
- THEN implementation may proceed under a separately recorded decision.

#### Scenario: Infeasible prerequisites

- GIVEN any prerequisite in `docs/imessage-receiver-feasibility.md` is unsatisfied
- WHEN this evaluation runs
- THEN the result is a documented no-go and no receiver is implemented.

#### Scenario: Contract rejects invalid receiver

- GIVEN a receiver profile with a wrong payload schema, an inline secret, a
  dynamic destination, a zero or over-limit bound, an invalid recipient handle,
  a non-HTTPS run URL base, or a message field other than run ID, outcome and
  run URL, such as a moving branch head
- WHEN Nickel validates it
- THEN validation fails.

#### Scenario: Contract accepts valid receiver

- GIVEN a receiver profile with the pinned payload schema, BLAKE3 identity
  algorithm, a valid destination and message fields limited to run ID, outcome
  and trusted run URL
- WHEN Nickel validates it
- THEN validation succeeds.
