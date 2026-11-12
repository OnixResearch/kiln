# Positioning and messaging

This is the message source for every external Kiln asset. It is copy, not
evidence: if a sentence here conflicts with a repository source it cites, the
source wins.

## Category

Kiln is a provider-neutral control plane for continuous integration. It owns
CI meaning (projects, pipeline revisions, runs, attempts, gates and status) and
delegates external effects to replaceable adapters.

Kiln is not a forge or source host, a workflow executor, a build system, a
release authority, or an available hosted service. It claims no production or
hostile-isolation qualification.

## One-liners

| Length | Copy |
| --- | --- |
| 8 words | Exact-revision CI with replayable state and replaceable providers. |
| 20 words | Kiln is an open-source CI control plane with deterministic state, exact-revision results and replaceable source, forge and execution adapters. |
| 50 words | Kiln is a provider-neutral control plane for continuous integration. A pure core turns commands into events and requested effects; adapters for Git, Jujutsu, Pijul, Radicle, GitHub, Gitea, Lattice and Nix perform them. Every result binds to an exact revision, unknown outcomes stay explicit, and recorded state can be replayed. |
| 100 words | Kiln is a provider-neutral control plane for continuous integration. It owns CI meaning (runs, attempts, gates and status) and delegates external effects to replaceable adapters. A pure core turns commands into events and requested effects, so recorded state can be replayed. Results bind to an exact revision or candidate; a moved branch needs a new evaluation. Transport failures after dispatch stay `Unknown` until reconciled. Adapters cover Git, Jujutsu, Pijul, Radicle, GitHub, Gitea/Forgejo, Lattice workflows and Nix builds, alongside a single-tenant web workspace, change review and read-only observation. Kiln is open source and self-hosted; it claims no production or hostile-isolation qualification. |

## Audience

These segments are hypotheses. [The commercial plan](../commercial-launch.md#1-offer-and-customer-validation)
states that external demand is not yet validated; Kiln's own self-CI is the
only current consumer.

| Priority | Segment | Trigger | Job to be done |
| --- | --- | --- | --- |
| Primary | Platform and infrastructure engineers who operate their own CI on Linux, often with Nix | A review, incident or audit asks which exact source a result belongs to and what happened to a lost job | Operate CI whose decisions are recorded, replayable and bound to exact revisions |
| Secondary | Teams whose source lives outside one forge: Radicle, Forgejo or Gitea, Jujutsu or Pijul workflows, or several hosts at once | CI definitions and history are tied to a forge they do not want to depend on | Keep one CI meaning while source, forge and execution providers change |
| Secondary | Teams reviewing a growing volume of changes, including agent-submitted ones | Review and merge decisions need to bind to the exact candidate that CI checked | Use change review with revision-bound approvals and exact-candidate required CI |
| Later, gated | Small teams that want managed CI for repositories they already host elsewhere | They want source-bound execution and a usage-based compute bill | Join an invite-only hosted-CI pilot once [its gates](../commercial-launch.md#6-dependency-ordered-launch-backlog) pass |
| Not a fit | Teams that need a turnkey hosted service today, a hostile multi-tenant isolation guarantee, GitHub Actions compatibility, a support contract or an SLA | Any of those is a hard requirement | Do not adopt Kiln for that requirement |

## Problems

1. **CI meaning is owned by a vendor.** Pipelines, statuses and history live
   inside one forge or runner product. Changing a provider means redefining
   what a run is.
2. **Results drift from the source they describe.** Moving branch tips,
   reruns and retries blur which exact revision a green check refers to.
3. **Unknown is reported as failure, or retried blindly.** A connection lost
   after dispatch becomes a false failure or a duplicated effect.
4. **History is logs, not decisions.** Operators can read output but cannot
   replay why the system did what it did.
5. **Green checks get promoted into stronger claims.** A passing run is
   repeated as proof of source trust, build correctness or release
   eligibility.

## Value pillars

| Pillar | What the reader gets | Proof surface |
| --- | --- | --- |
| Deterministic core | `state + command -> events + requested effects`, then `state + event -> new state`. The core has no filesystem, process, network, clock or credential dependency, so recorded state replays | [Architecture](../architecture.md), `cargo run --locked -p kiln-cli -- demo` |
| Exact-candidate results | Every gate result binds to one exact candidate identity; a moved target or failed dependency needs a new candidate and evaluation | [Change gating](../integrations.md#change-gating) |
| Replaceable providers | Source, collaboration, discovery, workflow, gate and trigger ports are narrow and versioned; adapters implement them and the core imports none | [Provider authoring](../provider-authoring.md), [README ports table](../../README.md#how-it-works) |
| Explicit uncertainty | A transport failure after provider acceptance produces `Unknown`; reconciliation resolves it without a blind retry | [Provider effects](../security-and-non-claims.md#provider-effects), [Recovery](../operator.md#recovery) |
| A workspace, not only a queue | A single-tenant web workspace with run search, logs, reruns, signed triggers and schedules, plus change review with exact-candidate required CI | [Web service](../operator.md#single-tenant-live-web-service), [Change review](../operator.md#optional-interactive-change-review) |
| Observation without control | Bounded read-only views with stable identities over HTTPS or iroh, and a local terminal or HTML dashboard | [Observation projection](../observation-projection.md), [Dashboard](../observation-dashboard.md) |
| Typed configuration and identity | Human-authored configuration is typed Nickel; Kiln-owned identities use BLAKE3 over packed Preserves encoding | [README](../../README.md#configuration-and-identity) |
| Bounded claims | Each surface states what a passing check does not prove | [Security and non-claims](../security-and-non-claims.md) |

## Proof points

Use these only with their bounds. Each receipt binds one recorded cohort and
date; none validates the current revision.

| Proof point | Bound |
| --- | --- |
| GitHub deliveries ran exact-source jobs on a separate Iroh worker and published matching commit statuses: three operated cases (pass, fail, recovery), verdict `PASS` ([receipt](../../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json)) | "Only the three genuine operated GitHub-to-separate-Iroh-worker-to-GitHub cases; not persistent deployment, whole-workspace remote CI certification, or release readiness" |
| A native Radicle peer update drove the controller and worker to an exact-revision status: success, source-controlled failure, host kill without redispatch, duplicate-fetch suppression, observed 2026-10-21 ([receipt](../../.cairn/archive/1970-01-01-qualify-live-radicle-ci/evidence/verification.json)) | "One fresh same-host native Radicle publisher/CI/observer topology on trusted reviewed canary source; not WAN or distinct-host delivery, hostile-worker confinement, production deployment" |
| Radicle source delivery crossed a private tailnet between two owned hosts ([receipt](../../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-ci/evidence/verification.json)) | "This is not WAN, hostile-network, hostile-worker confinement, production deployment, activation or release authority." |
| Kiln ran self-CI on a dedicated Radicle fork with success, genuine failure and unavailable-provider cases ([record](../../.cairn/archive/1970-01-01-qualify-kiln-self-ci/evidence/verification.md)) | Fork-based; the canonical repository was untouched; no hostile-node isolation |
| The web service was exercised natively: TLS, worker capacity, isolation denials, success, failure, partial, slow and cancelled runs ([receipt](../../.cairn/archive/1970-01-01-operate-single-tenant-web-service/evidence/native-final/verification.json)) | Isolated local evidence, not production deployment; bounds are not hostile confinement |
| Change review ran required CI to success and genuine failure with restart recovery ([receipt](../../.cairn/archive/1970-01-01-complete-review-workflow/evidence/qualification.json)) | Disposable local Git, SQLite and loopback HTTPS; no deployment, publication or release authority |
| The Firecracker backend produced the same canonical outcome as the namespace backend across cold and warm runs ([receipt](../../.cairn/archive/1970-01-01-add-firecracker-microvm-backend/evidence/verification.json)) | "No independent adversarial/hostile multi-tenant qualification; none is claimed." Timings are single-host observations, not capacity promises |
| `cargo test --locked --workspace` passed with 1608 tests, 0 failed, 5 ignored at `428d03fcfb02478ed79d38453fb44f82c6a2bdd5` on 2026-11-12 ([checklist](launch-checklist.md#3-install-and-demo-smoke)) | One `x86_64-linux` host in the development shell; tests do not prove provider behavior |

Do not quote any other count, duration or percentage. Do not combine counts
from different receipts.

## Differentiation

For Hydra, Zuul, Buildbot and the Radicle CI broker, the "does well" column
restates how the [README references](../../README.md#references) describe
each system. The forge-native and bespoke-script rows describe a category,
not a product. No adapter exists for Hydra, Zuul or Buildbot; they are design
references.

| Alternative | What it does well | Where Kiln differs | Use together? |
| --- | --- | --- | --- |
| Forge-native CI such as GitHub Actions | CI integrated with the forge that hosts the source | Kiln keeps CI meaning outside any one forge and binds results to exact candidates. It does not run Actions workflows; only [a fail-closed migration subset](../hosted-ci.md#5-repository-owned-pipelines-and-fail-closed-migration) exists in the optional hosted composition | Yes. Source stays on GitHub; Kiln receives webhooks and publishes commit statuses |
| Hydra | CI vocabulary, evaluation model, fair-share scheduling and operator surfaces | Nix is one provider among several. Kiln adds source-host intake, gating, review and a provider-neutral journal | Not today |
| Zuul | Exact candidate sets, speculative gates, dependency queues and frozen repository state | Kiln applies exact-candidate gating inside a small deterministic core with replaceable source and execution providers | Not today |
| Buildbot | Separation of change sources, schedulers, build requests, workers and reporters | Kiln records decisions as canonical events with stable identities and keeps provider effects behind versioned ports | Not today |
| Radicle CI broker | Node-local event filtering, CI-engine adapters and decentralized operation | Kiln is a CI engine the broker can invoke; it adds canonical state, recovery and exact-revision status | Yes. Kiln implements the broker protocol |
| Bespoke CI scripts | Full local control | Kiln supplies recorded state, replay, recovery rules and typed configuration | Yes, as reviewed job steps |

## Objection handling

### "Is it production-ready?"

No production qualification is claimed. The retained evidence is a set of
bounded local and live exercises, each for one recorded cohort. The most
recent retained readiness record, dated 2026-11-08 in the
`reconcile-readiness-and-qualification` lifecycle package, still lists open
qualification blockers. Evaluate Kiln in your own
environment under your own review.

### "Can it run untrusted pull requests safely?"

Kiln is not a sandbox. The standalone GitHub and Gitea adapters refuse fork
pull requests. The web service holds untrusted pull-request runs in a blocked
state until an operator approves them, and confines them to declared
untrusted-capable runners without secrets. The optional hosted composition's
namespace backend is for trusted jobs only; its Firecracker backend moves a
job off the host kernel, and every receipt still reports
`hostile_isolation_claim:false`. See [isolation and limits](../hosted-ci.md#6-explicit-isolation-resource-limits-and-execution-recovery).

### "Why not stay on forge-native CI?"

Stay if it meets your needs. Kiln is for teams that want CI meaning to
outlive a forge or executor choice, results bound to exact candidates,
explicit unknown outcomes, and a control plane they operate.

### "Do I need Radicle, Nix or Lattice?"

Radicle is where Kiln's own source is hosted; it is not a runtime dependency,
and core builds do not link it. Nix supplies the supported development shell
and packages. Workflow execution currently goes through the Lattice adapter,
so the GitHub, Gitea and web-service paths need a reviewed Lattice runtime.

### "What is the license?"

Kiln is a mixed-license source tree. Kiln-owned packages default to
`MIT OR Apache-2.0`. `kiln-adapter-pijul` is `GPL-2.0-only`. Several binaries
link `AGPL-3.0-or-later` or `MPL-2.0` components, so those combined artifacts
are not permissive-only. Read the [license map](../../LICENSES.md) before
redistributing a binary.

### "Is there a hosted version, and what does it cost?"

No hosted service is available. The [commercial plan](../commercial-launch.md)
proposes paid hosted CI for repositories that stay on their existing hosts,
starting with an invite-only pilot after its gates pass. No price is
approved. Self-hosting carries no software license or seat fee.

### "Will it run my GitHub Actions workflows?"

No. Pipelines are reviewed and typed. The optional hosted composition
compiles a bounded subset of Actions syntax and rejects everything else.

### "How do I install it?"

From source with the Nix flake. There is no binary release and no crates.io
publication; the `kiln-cli` and `kiln-core` crates on crates.io belong to an
unrelated project. The first build compiles the workspace. Start with the
[quick start](../../README.md#quick-start).

### "Who supports it?"

Use the issue tracker on the GitHub projection. No response time or support
commitment is offered. Report vulnerabilities
through [the security policy](../../SECURITY.md).

### "Why does every page list what it does not claim?"

Because that boundary is part of the product. Kiln records what was checked
and refuses to promote it into source trust, build correctness or release
eligibility.

## Message rules

Say:

- "provider-neutral CI control plane";
- "exact revision" and "exact candidate";
- "deterministic, replayable state";
- "replaceable adapters";
- "explicit `Unknown` outcomes and reconciliation";
- "retained receipt for a recorded cohort";
- "self-hosted";
- "open source, mixed-license";
- "hosted CI is planned and not available".

Do not say:

- "production-ready", "battle-tested" or "enterprise-grade";
- "secure sandbox", "safe for untrusted code" or "tenant isolation";
- "Actions-compatible" or "drop-in replacement";
- "exactly-once";
- "SLSA compliant" or any SLSA level: emitted statements are unsigned;
- "works with Tangled", "works with Gitea" or "works with Forgejo" without
  "fixture-tested";
- "MIT licensed" or "permissively licensed";
- "decentralized CI" as a blanket claim;
- "agent-safe" or "AI-verified": authorship metadata is self-declared and
  unverified;
- a hosted product name, a price, a waitlist, a customer, a user count or a
  benchmark.

## Naming

| Surface | Name |
| --- | --- |
| Product | Kiln |
| Operator CLI | `kiln` (crate `kiln-cli`) |
| Web workspace | `kiln-web-service` |
| Local dashboard | `kiln-dashboard` |
| Optional hosted composition | `kiln-hosted` |
| Canonical repository | `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` |
| GitHub projection | `https://github.com/OnixResearch/kiln` |
| HTTPS projection | `https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git` |

Do not coin other product names. The repository records no product domain;
do not reference one. Unrelated crates named `kiln`, `kiln-core` and
`kiln-cli` exist on crates.io: never show a `cargo install` command, and send
readers to the repository.

## Hosted-CI track

The commercial plan's proposed line is: "Keep your source where it is;
connect it to open-source Kiln for managed CI." Use it only in design-partner
conversations, with the status sentence beside it: bounded local
implementation, not a launched paid service. Never use it as an availability
claim in a release, announcement or post. Follow the
[design-partner guide](design-partners.md).

## Landing-page outline

The repository README is the landing surface. Use this order:

1. **Hero.** "Exact-revision CI with replayable state and replaceable
   providers." Subhead: the 20-word line. Status line: version, platform,
   license map, no production claim.
2. **Why.** The five problems, one line each.
3. **How it works.** The two core transitions and the ports table.
4. **Quick start.** Clone, development shell, tests, `demo`, dashboard
   preview.
5. **What runs today.** The capability map with maturity words and links.
6. **Hosted CI direction.** Planned, invite-only, not available.
7. **What Kiln does not claim.** The claim boundary.
8. **License, contributing, security.**
9. **References.**
