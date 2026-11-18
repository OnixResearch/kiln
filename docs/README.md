# Kiln documentation

Start with the [README](../README.md) for the project overview and quick start.
Each linked document states its own claim boundary, and `status.md` is the
detailed ledger of implemented areas, retained evidence and the publication
record.

The sections below group the maintained documents by the question they answer.

## Start here

- [README](../README.md): project overview, quick start and what runs today.
- [Implementation status and evidence index](status.md): the detailed ledger of implemented areas and evidence.
- [Architecture](architecture.md): Kiln owns CI meaning; providers own external effects.
- [Operator guide](operator.md): local reference run and the operating procedure for each composition.
- [Security and non-claims](security-and-non-claims.md): validates bounded facts, not ambient trust.
- [Integration contracts](integrations.md): source mapping, change gating and fairness rules.
- [Provider authoring](provider-authoring.md): narrow, versioned ports, not one universal interface.

## Sources

- [Local Git source](git-source.md): reads one local repository through pinned `git2`; read-only.
- [Pijul source](pijul-source-adapter.md): bounded, read-only local access preserving native identities.
- [Radicle source contract](radicle-source-contract.md): runtime-independent boundary for exact source observations.
- [Radicle source shell](radicle-source-shell.md): resolves and acquires one exact local Radicle snapshot.

## Forges, triggers and status

- [Gitea and Forgejo CI](gitea-ci.md): locally tested shell for signed webhooks and exact revisions.
- [Tangled integration](tangled-integration.md): CI event mapping and open questions; no real knot has called Kiln.
- [Radicle workflow shell](radicle-workflow-shell.md): one bounded broker trigger through a selected runtime path.
- [Radicle status](radicle-status.md): a rebuildable `xyz.radicle.ci-job` projection; no merge authority.
- [Radicle job publication](radicle-jobs.md): a local `xyz.radworks.job` projection for one exact commit.
- [Radicle mutation](radicle-mutation.md): appends only the declared `review` action to a declared patch.
- [GitHub publication](github-publication.md): idempotent verified-PR delivery and opt-in ordered stacks; never force-pushes or merges.
- [GitHub landing observation](github-landing-observation.md): reads one pull request and its check runs into bounded landing evidence, and recovers exact stacked merges; strictly read-only.
- [Repository feeds](repository-feeds.md): read-only Atom and RSS observations, not CI evidence.
- [GitHub CI adapter](operator.md#github-ci-adapter): operator procedure for webhook intake and exact-source jobs.
- [iMessage receiver feasibility](imessage-receiver-feasibility.md): documented no-go for an OpenBubbles receiver; a typed contract kept for a separately licensed one.

## Execution and builds

- [Lattice workflow](lattice-workflow.md): one pinned Lattice contract; state stays a provider observation.
- [Nix evaluation job discovery](nix-eval-jobs-adapter.md): optional discovery only; no builds or state change.
- [Nix build](nix-build-adapter.md): one exact derivation through the pinned store CLI; local, bounded.
- [Hosted Radicle Nix provider](radicle-nix-provider.md): one bounded check for one admitted repository.
- [Mantle build](mantle-build.md): one exact build cohort; Kiln keeps candidate and CI truth.
- [Aspen runtime backend](aspen-runtime-backend.md): workload-neutral substrate; Kiln keeps CI meaning.
- [Kiln Aspen host](../runtime/kiln-aspen-host/README.md): deployable shell for private Kiln-on-Aspen cohorts.
- [Artifact readiness](artifact-readiness.md): opt-in real hosted/Nix phase measurements and matched-cohort analysis; no unmeasured speedup claim.
- [Parallel frontend evaluation](parallel-frontend-evaluation.md): opt-in pinned-nightly `-Zthreads` benchmark under per-job contention limits, with a retained two-level matrix and a quota-bounded recommendation; default builds unchanged.
- [Archil workspace evaluation](archil-workspace-evaluation.md): versioned workspace-provider profile contract and a documentation-review comparison; retained as reference, no provider adopted.
- [celld event store](celld-event-store.md): versioned Worker entry for the pinned celld SQLite API.

## Gates, review and merge checks

- [Cairn gate](cairn-gate.md): evaluates one exact gate plan through Cairn's versioned contracts.
- [Semantic merge orchestration](semantic-merge-orchestration.md): opt-in, fixture-only check of a proposed merge.
- [Isolated Patchgate review](operator.md#isolated-patchgate-review): isolated review evidence; never a source mutation.
- [Optional interactive change review](operator.md#optional-interactive-change-review): human change review in the live workspace, separately admitted.

## Observation

- [Observation projection](observation-projection.md): bounded, deterministic, read-only views; no authority.
- [HTTP observation](http-observation.md): serves accepted views over bounded HTTPS; read-only.
- [Iroh observation](iroh-observation.md): serves accepted views to enrolled iroh peers; read-only.
- [HTTP API contract](http-api-contract.md): GET-only OpenAPI contract for runs, bindings and artifacts, validated against real responses; lockstep only.
- [Operator dashboard](observation-dashboard.md): local terminal or offline HTML report; read only.
- [Resource inventories](operator.md#agent-worker-compute-and-serving-observations): separate read-only definitions, instances, shared compute and serving observations; no dispatch authority.
- [Console port inventory](console-port-inventory.md): checked route, shell and state inventory of the live console, the reference for its Rats/Meteorite port; no route is migrated.
- [Atlas view note](atlas-view.md): planned default-off isometric topology view in which every payload stays labeled declared or observed; nothing is implemented.

## Hosted composition and commercial plan

- [Hosted CI](hosted-ci.md): bounded operator procedure for the optional composition, not a paid service.
- [Commercial launch](commercial-launch.md): charge for compute, not features; not a launched paid service.

## Implementation-work service

- [Implementation work admission](implementation-work.md): exact work, claims, frozen requirements and provider boundaries.
- [Plan review](plan-review.md): opt-in review of an agent's plan before a claim; default admission is unchanged.
- [Implementer service](implementer-service.md): ownership split and deployment prerequisites.
- [Implementation runbook](implementation-runbook.md): admitted Lattice/OMP dispatch, independent canonical CI verification and journaled publication; no unattended GitHub landing qualification.
- [Qualification](implementation-qualification.md): actual-consumer campaign commands, frozen acceptance controls and explicit external prerequisites.
- [Fair admission](fair-admission.md): opt-in queues, reservation accounting, finite parent budgets and conservative unknown-effect reconciliation.
- [Retained sessions](retained-sessions.md): opt-in host-effect journal and paired checkpoints; selected producer capabilities remain prerequisites.
- [Migration bridge](migration-bridge.md): exact handoff/fencing and bounded direct Iroh transport; no two-host or live-migration qualification.

## Hosting, pins, licensing and assurance

- [Hosting and recovery](hosting-and-recovery.md): Radicle as canonical host, publication candidates and recovery.
- [Design reference pins](reference-pins.md): exact review pins that create no runtime dependency.
- [License map](../LICENSES.md): mixed-license tree; the workspace default is not every artifact.
- [Fuzzing](fuzzing.md): fuzz targets outside the workspace; normal builds need no nightly toolchain.

## Launch kit

- [Launch kit index](gtm/README.md): entry point to the launch documents.
- [Positioning](gtm/positioning.md): message source for every external asset; copy, not evidence.
- [Launch plan](gtm/launch-plan.md): phases, channels, metrics, risks and go/no-go criteria.
- [Launch checklist](gtm/launch-checklist.md): dated observations and open owner-only actions.
- [Release notes 0.1.0](gtm/release-notes-0.1.0.md): draft release body for 0.1.0.
- [Announcement](gtm/announcement.md): long-form launch post draft.
- [Outreach](gtm/outreach.md): channel-specific copy.
- [Demo](gtm/demo.md): ten-minute demo script with exercised commands.
- [Design partners](gtm/design-partners.md): discovery guide for the hosted-CI pilot.

## Project files

- [Changelog](../CHANGELOG.md): notable changes per version, with maturity words and non-claims.
- [Contributing](../CONTRIBUTING.md): where work happens, checks, lifecycle and licensing of contributions.
- [Security policy](../SECURITY.md): private vulnerability reporting and the security-relevant surface.
- [Code of conduct](../CODE_OF_CONDUCT.md): participation expectations and enforcement.
- [Agent notes](../AGENTS.md): product boundary and core and shell rules.
