# Blocker: Lattice workflow transport service

Recorded: 2026-08-20

## Completed prerequisites

Lattice publishes `lattice-workflow-contract` at revision `66fe7b9b43567490caacc797ebda307e734c4419`.

Kiln publishes `kiln-adapter-lattice` and its application-owned `WorkflowPort` and `WorkflowExchangePort` at commit `63cd107d0f59f8ac9c3236dd18eedad0fae41960`.

## Exact blocker

Lattice does not publish a bounded shell transport service for the workflow contract.

No supported Unix socket, HTTP, RPC, or process endpoint implements dispatch, observe, control, log-read, and reconciliation exchange operations.

The Radicle executable cannot instantiate a production `WorkflowPort` without inventing an unreviewed transport protocol or bypassing Lattice.

The local uncommitted Radicle draft invokes Nix directly. That behavior conflicts with the accepted Radicle design and remains unchanged outside remote `main`.

## Unblock criteria

One of these inputs must become available:

1. Lattice publishes a versioned bounded transport service and positive plus negative fixtures for the workflow contract.
2. The Kiln owner selects another application-owned transport contract with equivalent lifecycle, ambiguity, control, and log semantics.

After that input exists, Kiln can implement the concrete `WorkflowExchangePort`, compose it with Radicle trigger intake, and complete this change.

## Non-claims

The completed Lattice contract and Kiln adapter do not prove a deployed transport, workflow execution, provider availability, or Radicle CI completion.
