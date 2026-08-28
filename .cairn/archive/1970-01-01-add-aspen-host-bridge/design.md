# Design: Kiln Aspen host bridge

## Ownership

Kiln owns the Unix request schema, CI callback protocol, provider selection, reconciliation, and terminal classification.

Aspen owns native callback execution, value materialization, effect intent, generation fencing, and bounded process mechanics.

Lattice owns workflow acceptance, execution, observation, and its Unix workflow exchange.

The new binary is a visible Kiln composition root. Core crates do not import Aspen or Lattice implementations.

## Components

### Unix server adapter

`kiln-aspen-host` listens on one admitted absolute Unix socket. It reads one bounded JSON frame per connection and writes one bounded response.

The process serves a configured finite request count. Systemd can restart it after clean exhaustion. Missing or non-socket endpoints never select another runtime.

### Aspen host composition

The binary constructs Aspen's admitted native host from:

- the exact Kiln Aspen profile;
- the exact extension executable bytes and path;
- deterministic role-separated BLAKE3 references;
- positive callback, diagnostic, queue, and teardown bounds;
- an in-memory callback value adapter; and
- an in-memory native journal.

The first service therefore has process-local evidence only. Startup refuses recovery when an earlier process may have unresolved work.

### Trigger registry

The server validates every `AspenServiceIngress` before it stores the trigger under its exact trigger identity.

The effect adapter can resolve only that identity. Missing or conflicting trigger bytes reject provider routing.

### Lattice effect adapter

The effect adapter materializes the exact callback effect request from Aspen's value port. It admits the Kiln effect plan and resolves the linked trigger.

It reuses `plan_trigger`, `LatticeAdapter`, `UnixWorkflowExchange`, and `SqliteReplayStore`. It does not reconstruct workflow policy independently.

The adapter waits within the configured poll bound. It emits a typed provider completion containing:

- the callback effect and operation identities;
- the selected provider profile;
- the trigger identity; and
- the exact admitted Lattice workflow observation.

The BLAKE3 reference and bytes become Aspen's `materialized_output`.

### Hosted completion mapping

The extension decodes only `system-extension-effect-completion-v2`. It requires the materialized provider completion.

It validates all callback, provider, trigger, and observation linkage. Terminal mapping is deterministic:

- Lattice success becomes `Success`;
- domain failure becomes `Failure`;
- denied becomes `Denied`;
- cancelled becomes `Cancelled`;
- timeout becomes `TimedOut`; and
- provider error, pending, or uncertain observations become `Unknown`.

No output reference alone can produce success.

## Request flow

```text
Radicle adapter
  -> Kiln Aspen Unix client
  -> kiln-aspen-host
  -> Aspen NativeSystemExtensionService
  -> bounded kiln-aspen-extension process
  -> Aspen typed transport effect
  -> Kiln Lattice effect adapter
  -> Lattice Unix workflow exchange
  -> exact Lattice observation bytes
  -> Aspen materialized completion callback
  -> Kiln terminal acknowledgement
```

## Reconciliation

The server retains acknowledgements by operation identity during its process lifetime. A repeated submit returns replay without re-running the provider.

A disconnect after write remains unknown to the client. A reconcile request reads the retained acknowledgement.

Process loss with unresolved work remains unknown. The first canary does not infer terminal state from the SQLite or Aspen journals alone.

## Authority

The host binary reads only explicit profile, extension, socket, state-root, and Lattice paths. It reads no ambient credentials or environment policy.

The extension process receives a cleared environment through Aspen. Lattice executes only its own admitted workflow.

## Tests

Positive tests use Aspen's real native service and a controlled Lattice transport fixture. Negative tests cover malformed frames, stale profiles, missing output bytes, identity drift, unavailable Lattice, duplicate conflict, timeout, and post-accept disconnect.

A process test starts the real Unix server and exercises the production client. It does not import Aspen test support.

## Non-claims

The bridge does not prove CI correctness, workflow correctness, callback durability, host sandboxing, provider truth, production availability, or release eligibility.
