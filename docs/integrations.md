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

`kiln-adapter-cdevents` pins CDEvents `0.5.1` at revision `df02ce0b6563a13bf35cc109993b03b18da8a14a`. It pins CloudEvents `1.0.2` with context version `1.0`.

The initial registry supports task-run started and finished events. The pure codec preserves envelope, subject, correlation, and causation facts.

Unknown types, missing links, duplicate identities, context disagreements, and lossy profiles fail before a canonical action. The adapter performs no transport effects.

`kiln-adapter-opentelemetry` pins OTLP `1.11.0` at revision `790608c4d51e6ffc12210b541e8514cbed9e91a4`. It pins semantic conventions `1.43.0` at revision `89aae438b3b3b0a8dd33003c9d70592baf7dbd0d`.

The adapter projects spans, bounded metrics, and redacted logs. Metric names use a configured registry, and provider labels cannot become dynamic metric keys.

The optional OTLP/HTTP JSON shell uses a registered endpoint profile. Telemetry loss, duplication, delay, and backend acceptance do not change canonical state.

## Valence evidence

`kiln-adapter-valence` pins `valence-core` revision `220307a371e7bc4e3309447b09447d0fe6c4db5f`. It records the transitive Artifact Auth revision `e41340bec587b6d049b5cc518ec7db925dde84be`.

The adapter emits `valence.evidence-ir.v1` artifacts with the `recorded_only` verification role. It preserves exact Kiln subjects, source spans, identities, roles, and non-claims.

Kiln cannot promote itself into builder, checker, or signer roles. Imported evidence cannot create runs, outcomes, merge authority, or release readiness.

`valence-core` uses `MPL-2.0`. Kiln consumes it as a pinned dependency and does not copy its source.

## Webhook notifications

`kiln-adapter-webhook` delivers notification intents only through registered HTTPS endpoint profiles. Requests cannot supply destination URLs.

The adapter checks DNS results against address-class policy before it loads secrets. Redirects and private, loopback, link-local, multicast, and unspecified addresses are denied by default.

Bodies bind the run, outcome, policy, endpoint, BLAKE3 identity, and idempotency key. Timeouts after possible acceptance remain `Unknown` unless an idempotent reconciliation contract permits retry.

`bounded-http` revision `db13eb8a0341719e54817ae34451cf310a41033f` was evaluated but not imported. Its minimum Rust version is `1.96.0`; Kiln is pinned to `1.88`.

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
