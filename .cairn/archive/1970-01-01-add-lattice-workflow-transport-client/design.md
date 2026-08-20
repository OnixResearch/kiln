# Design: Concrete Lattice workflow transport client

## Success contract

Completion requires exact producer pinning, a concrete local client, bounded transport profiles, dispatch recovery, positive and negative socket tests, and full Kiln verification.

False completion includes raw JSON sockets, direct process execution, automatic redispatch, ambient peer policy, lost unknown outcomes, or a fixture-only transport.

## Architecture

`UnixWorkflowExchange` implements Kiln's application-owned `WorkflowExchangePort`. It owns socket I/O mapping and a bounded map of requests accepted in this process.

`LatticeAdapter` retains CI lowering, observation policy, and canonical Kiln state admission.

The producer crate owns framing, peer policy, request identity, response identity, and disconnect classification.

## Dispatch recovery

A post-write unknown outcome triggers only `RecoverDispatch` with the original admitted request. A recovered exact acceptance resumes normal observation.

Not-found remains unknown. Kiln never converts it into safe retry authority.

## Tests

Positive tests cover dispatch, observe, controls, logs, reconciliation, and recovered acceptance.

Negative tests cover missing sockets, incomplete writes, post-write disconnects, not-found, malformed responses, stale linkage, and response limits.

## Non-claims

The client does not prove a deployed Lattice handler, durable lookup, provider availability, workflow correctness, or external effect success.
