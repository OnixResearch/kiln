# Announcing Kiln 0.1.0

> Draft announcement. Publish only after
> [launch-checklist.md](launch-checklist.md) is green. The author must disclose
> that they maintain Kiln. This is a CI control-plane announcement, not a
> replacement-CI pitch.

The usual answer to "did CI check this?" is a job page and a green icon. It
holds until someone asks which exact revision was checked, what happened to a
job that lost its connection after dispatch, or why a rerun told a different
story.

Kiln keeps that record explicit: a provider-neutral control plane for continuous
integration, version 0.1.0, self-hosted, open source under a mixed license.

## The problem

CI meaning is owned by a vendor: pipelines, statuses and history live inside one
forge or runner product.

Results drift from their source: moving branch tips, reruns and retries blur
which exact revision a green check covers.

Unknown is reported as failure, or retried blindly, so a lost connection becomes
a false failure or a duplicated effect.

History is logs, not decisions: operators can read output but cannot replay why
the system acted.

Green checks become stronger claims, repeated as proof of source trust, build
correctness or release eligibility.

## What Kiln is

Kiln is a provider-neutral control plane for continuous integration. It owns CI
meaning (projects, pipeline revisions, runs, attempts, gates and status) and
delegates external effects to replaceable adapters.

It is not a forge, a workflow executor, a build system, a release authority, or
an available hosted service. Kiln records the decision and its evidence
boundary; source history and execution stay with their providers.

## How it works

```text
state + command -> events + requested effects
state + event   -> new state
```

`kiln-core` is pure `no_std + alloc` code with no filesystem, process, network,
clock or credential dependency. A shell commits events and outbox records
together, then dispatches an effect only after capability admission. Replay
returns no provider effects.

Versioned ports cover source, collaboration, discovery, workflow, gates,
triggers and remote execution. Git, Jujutsu, Pijul, Radicle, GitHub,
Gitea/Forgejo, Lattice and Nix adapters implement them; the core imports none.

A transport failure after provider acceptance produces `Unknown`, resolved by
reconciliation rather than a blind retry. Every gate result binds one exact
candidate; a moved target needs a new candidate.

## What runs today

- **GitHub CI.** Authenticated push and pull-request intake, polling recovery,
  exact HTTPS source jobs through Lattice, durable replay, commit statuses.
  Exercised live once: three operated cases (pass, fail, recovery) for a
  recorded cohort; not persistent deployment, whole-workspace certification or
  release readiness.
- **Radicle CI.** Broker compatibility, source-to-build composition with Nix,
  exact-revision status, patch reviews, tag-triggered CI. Exercised live
  same-host on 2026-10-21 and across two owned hosts on a private tailnet; not
  WAN, hostile-network or production qualification.
- **Gitea/Forgejo CI.** Signed webhooks, exact-source jobs, replay, commit
  statuses. Fixture-tested: local tests do not prove a live deployment.
- **Tangled.** A spindle-compatible service. Fixture-tested: no real knot has
  called Kiln, and there is no deployment binary.
- **Nix adapters.** `nix-eval-jobs` discovery and an exact local derivation
  build provider with rooted outputs, bounded logs, read-only cache projection.
  Exercised locally; no claim of network acquisition, substitution, remote
  execution or flake evaluation.
- **Web workspace.** Single-tenant HTTPS service with run search, logs, reruns,
  signed triggers, schedules, filters, notifications, operator-approved
  untrusted pull-request runs, source browser. Exercised locally with TLS across
  success, failure, partial, slow and cancelled runs; isolated local evidence is
  not production deployment, and this is not a hostile-code sandbox.
- **Change review.** Patch sets, exact diffs with interdiff, Jujutsu change-ID
  intake, suggestions, ownership, revision-bound approvals, exact-candidate
  required-CI merge automation, speculative merge queue. Exercised locally with
  disposable Git, SQLite and loopback HTTPS; no deployment, publication or
  release authority.
- **Observation.** Bounded read-only views with `kiln.observe.view.v1`
  identities over HTTPS, iroh for allowlisted peers, and a local dashboard.
  Fixture-tested and exercised locally; views carry no authority, and live
  delivery is not a completeness claim.

## What we do not claim

- production readiness, stability guarantees, an SLA, uptime or support times;
- a sandbox or confinement for hostile code;
- a hosted or paid Kiln service, beta, waitlist or approved prices;
- GitHub Actions workflow compatibility or replacement without a rewrite;
- once-only delivery or execution guarantees;
- any SLSA level, signed provenance or supply-chain certification;
- live interoperability with Tangled, Gitea or Forgejo;
- users, customers, adopters, stars, downloads or benchmarks.

Historical receipts each bind one recorded cohort and date. None validates the
current revision.

## Try it

Install from source with the Nix flake. There is no binary release, no container
image and no crates.io publication.

```console
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
  jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
cd kiln
nix develop path:$PWD
cargo run --locked -p onix-kiln-cli -- demo
```

`demo` commits one `StartRun` command to an in-memory journal and prints the
result. It performs no network, VCS or filesystem write and dispatches nothing:

```json
{"revision":1,"runs":{"demo-run":{"active_attempt":"demo-attempt","outcome":"Pending"}},"attempts":{"demo-attempt":{"run_id":"demo-run","provider_profile":"standalone-build-v1","outcome":"Pending","external_reference":null,"products":[],"metrics":[]}},"holds":[],"seen_commands":["demo-start"]}
```

Render the local dashboard over synthetic sample data and open the printed
`dashboard.html`:

```console
cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview
```

The preview is synthetic rehearsal data, not evidence of real CI execution.

## Hosted CI

Hosted CI is planned and not available. The commercial plan proposes managed CI
for repositories that stay on existing source hosts, charged as execution time
by runner class, starting with an invite-only pilot after its gates pass. Its
status is a bounded local implementation, not a launched paid service. No
numerical price is approved, and no company formation, merchant approval or live
charge exists. Self-hosting carries no license or seat fee. External demand is
not yet validated.

## License

Kiln is a mixed-license source tree. Kiln-owned packages under `crates/` default
to `MIT OR Apache-2.0`; `kiln-adapter-pijul` is `GPL-2.0-only`. Several binaries
link `bounded-exec` (`AGPL-3.0-or-later`) or other copyleft components, so those
combined artifacts are not permissive-only. Read the
[license map](../../LICENSES.md) before redistributing a binary.

## Feedback we want

- Does exact-candidate binding match how you gate merges?
- Where does setup break: clone, development shell, first build, or first
  adapter?
- Which provider adapter would you need first?
- What would make you run Kiln beside an existing CI system rather than instead
  of it?
- Which part of the claim boundary is unclear or wrong?

## Links

- [Repository README](../../README.md)
- [Project status](../status.md)
- [Security and non-claims](../security-and-non-claims.md)
- [Security policy](../../SECURITY.md)
- [License map](../../LICENSES.md)
