# Blocker: Lattice workflow application handler

Recorded: 2026-08-20
Updated: 2026-08-20

## Completed prerequisites

Lattice publishes `lattice-workflow-contract` and `lattice-workflow-transport` at revision `e3148ec59fa12081c270d9800a060d94214b9405`.

The transport provides bounded Linux Unix framing, peer admission, deadlines, every exchange operation, explicit ambiguity, and dispatch-acceptance recovery.

Kiln publishes its application-owned `WorkflowPort` and `WorkflowExchangePort`. The `add-lattice-workflow-transport-client` change adds the concrete Unix client.

## Exact blocker

The Lattice executable does not provide a production `WorkflowExchangeHandler` that binds the transport service to Lattice durable workflow state and runtime execution.

No supported Lattice command starts the Unix service with durable dispatch, observe, control, log-read, reconciliation, and dispatch-recovery behavior.

The Radicle executable cannot target a deployed Lattice handler without inventing runtime behavior outside Lattice.

The local uncommitted Radicle draft invokes Nix directly. That behavior conflicts with the accepted Radicle design and remains unchanged outside remote `main`.

## Unblock criteria

Lattice must publish and test an application handler plus composition command for the existing transport contract.

The handler must preserve durable acceptance, exact request lookup, lifecycle observations, control outcomes, log bounds, and unknown outcomes without automatic redispatch.

After that input exists, Kiln can compose the concrete client with Radicle trigger intake and complete this change.

## Non-claims

The transport protocol and client do not prove a deployed handler, workflow execution, provider availability, or Radicle CI completion.
