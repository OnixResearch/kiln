# Kiln operator guide

Kiln is the OnixResearch AI Software Factory control plane: multi-machine
agent fleet coordination (`kiln-fleet` + Herdr + swarms), deterministic
exact-revision CI gating (`kiln-core` + Nix/Lattice/Aspen), and
`jj`/Cairn/Code-Review delivery behind one pure `no_std + alloc` state machine.

Implementation work claims, renewals, dispatch, submission, verification
admission, cancellation, publication, integration and lifecycle completion
require a selected authority profile. See [select implementation
authority](implementation-runbook.md#select-implementation-authority-first) and
[implementation authority](implementation-work.md#implementation-authority).
An unconfigured protected operation denies before any effect.

## Focused operator runbooks

The operator guide is split into four focused runbooks under [`docs/operator/`](operator/):

| Runbook | Covers | Start here |
| --- | --- | --- |
| [Deploy & upgrade](operator/deploy-and-upgrade.md) | `kiln.json` workspace manifest, `kiln-web-service` TLS/auth/tailnet, console navigation & GPUI theme tokens, supervised deployment, internal developer loop, interactive change review & `jj` intake | [Workspace manifest](operator/deploy-and-upgrade.md#workspace-manifest-kilnjson), [Live web service](operator/deploy-and-upgrade.md#single-tenant-live-web-service) |
| [Fleet & Factory](operator/fleet-and-factory.md) | Factory status, read-only observation projection (`kiln-observe-core`, `kiln-dashboard`), logs/wait/capacity/controls, SQLite event store, links to `kiln fleet` and implementation runbooks | [Status](operator/fleet-and-factory.md#status), [Observation projection](operator/fleet-and-factory.md#observation-projection) |
| [CI & providers](operator/ci-and-providers.md) | Local reference verification rail, Patchgate review, Nix job discovery, Aspen native runtime, GitHub/Gitea/Iroh/Radicle providers, experimental adapter tier | [Local reference run](operator/ci-and-providers.md#local-reference-run), [GitHub CI](operator/ci-and-providers.md#github-ci-adapter) |
| [Recovery & qualification](operator/recovery-and-qualification.md) | Journal/outbox recovery, OnixOS pilot, readiness reconciliation, selected campaign progress/blockers, campaign decision inbox, fan-out runbooks, Cairn changes | [Recovery](operator/recovery-and-qualification.md#recovery), [Campaign progress](operator/recovery-and-qualification.md#selected-campaign-progress-and-blockers) |

## Unified CLI quick reference

All local onboarding, diagnostics, service checks, fleet coordination, and CI
commands route through `kiln` (`onix-kiln-cli`) and `kiln.json`
(`config/factory-workspace.ncl`, schema `kiln.factory-workspace.v1`):

```console
kiln init --repo /path/to/repo --out /tmp/my-kiln
kiln doctor --config /tmp/my-kiln/kiln.json
kiln check --config /tmp/my-kiln/kiln.json
kiln serve --config /tmp/my-kiln/kiln.json
kiln fleet status
kiln ci --help
```

- `kiln doctor --config <path>` validates `kiln.json` references, profile loading, and local tool availability (`git`, `jj`, `nickel`, `kiln-fleet`, `kiln-web-service`).
- `kiln check` and `kiln serve` accept `--config <path>` with optional explicit flag overrides (`--profile`, `--access`, `--fleet`, `--repository-root`, etc.).
- See [Deploy & upgrade](operator/deploy-and-upgrade.md#workspace-manifest-kilnjson) for the full `kiln.factory-workspace.v1` schema and flag matrix.

## Experimental adapter tier

The live Factory workspace `Cargo.toml` `default-members` list includes Git,
Jujutsu, GitHub, Radicle, Lattice, Nix, Cairn, Patchgate, Mantle, Aspen, and
Iroh. Four experimental/off-by-default adapters (`kiln-adapter-slurm`,
`kiln-adapter-tangled`, `kiln-adapter-pijul`, and `kiln-adapter-gitea`) stay in
`workspace.members` but are excluded from `default-members`; build or test them
explicitly via `-p <crate>` or `--workspace`. See
[CI & providers](operator/ci-and-providers.md#gitea-and-forgejo-ci-adapter).

## Jujutsu (`intake.jujutsu`) boundary

<!-- r[impl kiln.review.jj_intake.surface] -->
When optional `intake.jujutsu` is configured for change review, Kiln stays
strictly read-only toward Jujutsu and is **not** a `jj` forge or collaboration
authority:

- Kiln reads the single operation head from `.jj/repo/op_heads` on disk and
  invokes the configured `jj` executable with `--ignore-working-copy`,
  `--at-op <head>`, `JJ_CONFIG=/dev/null`, fixed JSON templates, and bounded
  timeout/output limits. It never snapshots, rebases, squashes, describes,
  pushes, merges operations, or moves bookmarks.
- Only full 32-character `jj` change IDs are accepted (prefixes, revsets, and
  the root change are refused), and every request passes fail-closed
  `JujutsuFacts` admission before any provider effect, reservation, or state
  change.
- Recorded `jj` fields (source, change ID, commit, observed operation heads,
  `evolog` predecessors, and commit author) are **provider facts**, never Kiln
  identities, and grant no write, CI, approval, batch-merge, or target-branch
  authority.

See [Jujutsu change-ID intake](operator/deploy-and-upgrade.md#jujutsu-change-id-intake)
for commands, stable error codes, stack rules, and evolog rendering.

## Console theme tokens and Vanilla-JS freeze

`crates/kiln-dashboard/src/render/identity.css` and
`crates/kiln-web-service/src/web.css` share GPUI Kit's semantic theme token
naming (dots written as hyphens, single `light-dark(light, dark)` declarations,
status `-background` paired with status foregrounds, and `--brand-door` /
`--brand-live`). The full token mapping table, Lucide icon import pipeline
(`scripts/import-lucide-icons.sh`), and `tests/kiln-identity.test.mjs` contrast
checks live in
[Navigate the live workspace](operator/deploy-and-upgrade.md#navigate-the-live-workspace).

**Vanilla-JS frontend freeze:** `crates/kiln-web-service/src/*.js` is frozen to
bug-fixes-only while `#67`/`#68` (Rats/Meteorite) remains the target UI
architecture (see [Console port inventory](console-port-inventory.md)). RSS/Atom
repository feeds remain opt-in and hidden from default console navigation
unless explicitly configured.

## Procedure quick links

### Deploy, web console, and review
- [Workspace manifest (`kiln.json`)](operator/deploy-and-upgrade.md#workspace-manifest-kilnjson)
- [Single-tenant live web service](operator/deploy-and-upgrade.md#single-tenant-live-web-service)
- [Explore the website without providers](operator/deploy-and-upgrade.md#explore-the-website-without-providers)
- [Property-based browser exploration](operator/deploy-and-upgrade.md#property-based-browser-exploration) <!-- r[verify kiln.web.topbar_containment.scenario.evidence] r[impl kiln.web.topbar_containment.scenario.host_safety] r[verify kiln.web.topbar_containment.scenario.host_safety] -->
- [Optional private-tailnet access](operator/deploy-and-upgrade.md#optional-private-tailnet-access) <!-- r[impl kiln.web.exploration.scenario.tailnet] -->
- [Navigate the live workspace](operator/deploy-and-upgrade.md#navigate-the-live-workspace) <!-- r[impl kiln.web.design_system.scenario.provenance] -->
- [Extend an existing project registry](operator/deploy-and-upgrade.md#extend-an-existing-project-registry) and [Monorepo projects](operator/deploy-and-upgrade.md#monorepo-projects) <!-- r[related kiln.web.projects.onixresearch] r[verify kiln.web.projects.onixresearch] -->
- [Optional signed COB workspace](operator/deploy-and-upgrade.md#optional-signed-cob-workspace) and [Optional read-only COB browser](operator/deploy-and-upgrade.md#optional-read-only-cob-browser) <!-- r[related kiln.dashboard.cobs.population] r[related kiln.dashboard.cobs.population.scenario.import] r[verify kiln.dashboard.cobs.population] r[verify kiln.dashboard.cobs.population.scenario.import] -->
- [Supervised deployment](operator/deploy-and-upgrade.md#supervised-deployment) and [Internal developer loop](operator/deploy-and-upgrade.md#internal-developer-loop)
- [Optional interactive change review](operator/deploy-and-upgrade.md#optional-interactive-change-review) and [Jujutsu change-ID intake](operator/deploy-and-upgrade.md#jujutsu-change-id-intake) <!-- r[verify kiln.review.keyboard.navigation.scenario.targets] r[verify kiln.review.keyboard.actions.scenario.cancel] -->

### Fleet, Factory, and observation
- [Status](operator/fleet-and-factory.md#status) and [Observation projection](operator/fleet-and-factory.md#observation-projection)
- [Logs and follow](operator/fleet-and-factory.md#logs-and-follow), [Wait](operator/fleet-and-factory.md#wait), [Capacity and readiness](operator/fleet-and-factory.md#capacity-and-readiness), and [Controls](operator/fleet-and-factory.md#controls)
- [SQLite event store](operator/fleet-and-factory.md#sqlite-event-store)

### CI and providers
- [Local reference run](operator/ci-and-providers.md#local-reference-run) <!-- r[verify kiln.toolchain.pin.scenario.shell] r[verify kiln.toolchain.pin.scenario.older] -->
- [Isolated Patchgate review](operator/ci-and-providers.md#isolated-patchgate-review)
- [Nix job discovery](operator/ci-and-providers.md#nix-job-discovery) and [Aspen native runtime](operator/ci-and-providers.md#aspen-native-runtime)
- [GitHub CI adapter](operator/ci-and-providers.md#github-ci-adapter) and [Gitea and Forgejo CI adapter](operator/ci-and-providers.md#gitea-and-forgejo-ci-adapter) <!-- r[impl kiln.lattice.runtime_pin] -->
- [Iroh remote workflow execution](operator/ci-and-providers.md#iroh-remote-workflow-execution)
- [Radicle source-to-build composition](operator/ci-and-providers.md#radicle-source-to-build-composition)

### Recovery, qualification, and campaigns
- [Recovery](operator/recovery-and-qualification.md#recovery)
- [Isolated maintained-cohort OnixOS pilot](operator/recovery-and-qualification.md#isolated-maintained-cohort-onixos-pilot)
- [Readiness reconciliation and qualification (2026-11-06)](operator/recovery-and-qualification.md#readiness-reconciliation-and-qualification-2026-11-06) <!-- r[impl kiln.microvm.confinement.nonclaims] -->
- [Selected campaign progress and blockers](operator/recovery-and-qualification.md#selected-campaign-progress-and-blockers)
- [Campaign controls and decision inbox](operator/recovery-and-qualification.md#campaign-controls-and-decision-inbox)
- [Runbooks and parent fan-out runs](operator/recovery-and-qualification.md#runbooks-and-parent-fan-out-runs) and [Runbook result roll-ups](operator/recovery-and-qualification.md#runbook-result-roll-ups)
- [Native SDLC operator controls](operator/recovery-and-qualification.md#native-sdlc-operator-controls) and [Cairn changes](operator/recovery-and-qualification.md#cairn-changes)
