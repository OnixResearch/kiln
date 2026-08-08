# Integration contracts

## Source mapping

A source observation preserves the repository, symbolic reference, exact native revision, and canonical tree identity.

Git, Pijul, and Jujutsu identities remain provider facts. Equal trees do not prove equal history, review, signatures, or trust.

The Jujutsu profile admits the Git backend. It rejects ambiguous revsets, conflicted bookmarks, and divergent changes.

## Change gating

A candidate binds ordered changes, dependencies, frozen sources, pipeline revisions, and gate policy. Results apply only to that candidate identity.

Serial mode waits for earlier results. Speculative mode requires an explicit bounded window policy.

A failed prefix invalidates dependent results. Merge preflight rejects changed targets, approvals, capabilities, candidates, or policy.

## Fairness

Weighted-scalar policy applies configured resource weights. Dominant-share policy requires a capacity basis for each resource.

The core compares integer ratios by cross multiplication. Additional positive use cannot improve a tenant priority.

Providers keep worker placement and local preemption. Kiln owns only global tenant admission order.

## Remote execution

The optional adapter supports admitted upload and execute operations through a transport trait. It preserves action, operation, cache, and log identities.

A disconnect after provider acceptance returns `Unknown`. A missing output rejects remote success.

The core imports no generated Remote Execution API types. The standalone profile does not require a remote service.

## Interchange

SLSA and in-toto projections bind the exact producing attempt. Builder, signer, provider, worker, and Kiln identities stay separate.

CDEvents import pins the event and envelope versions. Unknown event types do not create Kiln commands.

OpenTelemetry output is rebuildable from retained events. Telemetry loss does not change canonical state.

## Radicle CI broker

Broker mode reads one JSON line. It returns one `triggered` line and one terminal line.

The adapter writes protocol lines only to standard output. Diagnostics use standard error.

A future direct-node mode requires another capability profile. It cannot silently replace broker mode.

Node-local filtering remains outside Kiln. Third-party status remains an external observation.

Kiln does not execute `.radicle/native.yaml` by default. That file has no execution authority without an admitted import and provider policy.

## Radicle COBs

The profile registry includes Radicle issue, patch, and CI job profiles. It also includes all six pinned Cairn profiles.

Read, event, status, and mutation capabilities stay separate. Every mutation is preview-first and binds expected heads.

A status COB is a projection. A disagreement cannot mutate the Kiln journal.
