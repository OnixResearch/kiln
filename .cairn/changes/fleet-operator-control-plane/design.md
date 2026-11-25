# Fleet operator control plane design

## Context

Cairn detail, inventory, campaign controls and Fleet events already exist but use different scopes and observation rules.

## Decisions

Compose existing owner views and route commands to owners. Match attempt/fence/work, provider execution and campaign revision exactly. Requested, acknowledged and observed effects remain separate. A recovery request uses retained original intent and must not submit a replacement. Worker, model and agent facts keep producer identity and freshness. Pause applies only where the owning route supports it.

Web recovery is disabled unless the service is configured with the original reviewed Lattice route and byte-sealed request and a grant for one authenticated principal. The browser reviews an exact attempt/fence and digest before issuing a command. A lost reply remains unknown and requires fresh evidence before another command. Native worker inspection is a separate read of the Lattice owner export for the canonical sealed campaign request, not a runtime census.

The native inspector consumes Lattice inventory v3. It displays the model label only for a completed response that passed the producer's model check. Deployment and replica are admission facts, not response-level replica proof. Unattributed broker coverage is displayed separately. Parent/child lineage requires a harness-owned export bound to the same attempt; a single-shot OMP stream is not such an export.

For the opt-in Animus profile, Fleet admits only the Lattice readback of the Animus lineage receipt. It matches the sealed native request, work, attempt, fence, worker incarnation and profile revision, then projects bounded direct children and the producer's coverage. Default OMP remains unchanged. Missing or rejected coverage is not an empty tree.

## Risks / Trade-offs

Some providers do not emit per-attempt runtime or topology observations. Incomplete evidence remains visible as incomplete. Controls refuse crossed identities and never substitute CLI snippets or unreviewed provider access for authority.
