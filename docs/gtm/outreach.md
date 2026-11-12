# Outreach copy

Use these variants only after [launch-checklist.md](launch-checklist.md) is
green. Disclose that you maintain Kiln. Do not ask for votes, stars or upvotes;
do not post the same text repeatedly, and follow each community's rules.

`<PINNED_URL>` becomes
`https://github.com/OnixResearch/kiln/blob/<REVISION>` for the tagged commit;
`<name>` is a recipient or sender. No product site exists, and none may be
referenced; the README is the landing surface.

## Show HN

```text
Title: Show HN: Kiln – exact-revision CI with replayable state

I maintain Kiln, a provider-neutral control plane for continuous integration.
It owns CI meaning and delegates effects to replaceable adapters. It is open
source under a mixed license and self-hosted.

A green check does not say which exact revision it belongs to, and a job whose
connection dropped after dispatch becomes a false failure or a duplicated
effect. Kiln keeps that decision in recorded state. The core is pure
`no_std + alloc` Rust with no filesystem, process, network, clock or credential
dependency, so recorded runs replay. Results bind one exact candidate, and a
transport failure after acceptance stays `Unknown` until reconciliation.

Install from source with the Nix flake, then run the demo:

  GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
    jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
  cd kiln
  nix develop path:$PWD
  cargo run --locked -p kiln-cli -- demo

It is not a sandbox, and no production qualification is claimed. Hosted CI is
planned and not available.
```

First comment, posted separately:

```text
Maintainer here. Boundaries first:

- No production qualification, SLA or support commitment is claimed.
- Kiln is not a sandbox and not qualified for hostile code.
- GitHub CI was exercised live once, three operated cases on one cohort;
  Radicle CI ran same-host and across two owned hosts on a private tailnet.
  Neither is WAN qualification.
- Gitea/Forgejo and Tangled are fixture-tested only.
- Hosted CI is planned and not available.
- Open source, mixed license, self-hosted, installed from source with Nix; read
  LICENSES.md before redistributing a binary.
```

## Nix community post

```text
Title: Kiln 0.1.0: exact-revision CI with Nix discovery and build adapters

Kiln binds every CI result to an exact revision; a moved branch needs a new
evaluation. Its two Nix adapters are a `nix-eval-jobs` discovery adapter and an
exact local derivation build provider with rooted outputs and bounded logs.
Both are exercised locally on x86_64-linux; they claim no network acquisition,
substitution, remote execution or flake evaluation.

Kiln is self-hosted, open source under a mixed license, installed from source
with its flake.

It is not a sandbox, and no production qualification is claimed. Hosted CI is
planned and not available.
```

## Rust community post

```text
Title: Kiln 0.1.0: a pure no_std + alloc CI core with replayable state

`kiln-core` is pure `no_std + alloc` Rust with no filesystem, process, network,
clock or credential dependency. It has one command decision and one event
evolution path:

  state + command -> events + requested effects
  state + event   -> new state

A shell commits events and outbox records together, then dispatches after
capability admission. Provider effects sit behind narrow versioned ports the
core never imports; replay returns no provider effects, and a transport failure
after acceptance produces `Unknown`.

Kiln is self-hosted, open source under a mixed license, installed from source
with Nix.

It is not a sandbox, and no production qualification is claimed. Hosted CI is
planned and not available.
```

## Radicle community note

```text
Kiln 0.1.0 implements the Radicle CI broker protocol: the broker's event
subscription can invoke the controller and worker, and results are published as
exact-revision status COBs. Live exercises were same-host and across two owned
hosts on a private tailnet; not WAN or production qualification.

Kiln is self-hosted, open source under a mixed license, installed from source
with Nix, and not a sandbox; no production qualification is claimed.
```

## Newsletter blurb

```text
Kiln 0.1.0 is a provider-neutral control plane for continuous integration.
Results bind one exact revision or candidate, transport failures stay `Unknown`
until reconciled, and recorded state replays. The core is pure `no_std + alloc`
Rust; adapters cover Git, Jujutsu, Pijul, Radicle, GitHub, Gitea/Forgejo,
Lattice, Nix, a web workspace and change review. Kiln is self-hosted, open
source, mixed-license, installed from source with Nix, and not a sandbox. No
production qualification is claimed.
```

## Short social thread

```text
CI answers "did it pass?" but rarely "which exact revision passed, and what
happened to the job that lost its connection after dispatch?" Kiln 0.1.0 is an
open-source, self-hosted CI control plane.

Kiln's core is pure `no_std + alloc` Rust: state + command -> events +
requested effects; state + event -> new state. Events commit before effects
dispatch, and replay returns no provider effects.

A transport failure after a provider accepts work becomes an explicit `Unknown`,
resolved by reconciliation, not a blind retry. Every result binds one candidate.

Adapters cover Git, Jujutsu, Pijul, Radicle, GitHub, Gitea/Forgejo, Lattice and
Nix, plus a web workspace and change review. Each states its maturity and bound.

Open source, mixed license, self-hosted, installed from source with Nix. Not a
sandbox; no production claim. Hosted CI is planned and not available.
```

## LinkedIn

```text
Most CI conversations start with a green check. The harder question is which
exact revision it covers, and what happened to a job that lost its connection.

Kiln 0.1.0 is an open-source, self-hosted control plane for continuous
integration. It records CI meaning as deterministic state, binds results to
exact candidates, keeps `Unknown` explicit until reconciliation, and replays
recorded runs. Adapters for Git, Jujutsu, Pijul, Radicle, GitHub, Gitea/Forgejo,
Lattice and Nix implement the effects.

It is a mixed-license source tree, installed from source with Nix, not a
sandbox, with no production qualification claimed. Hosted CI is planned and
not available.
```

## Self-hosted evaluator email

```text
Subject: Try Kiln 0.1.0 on one repository?

Hi <name>,

I maintain Kiln, an open-source, self-hosted CI control plane that binds results
to exact revisions and replays recorded state. I am looking for operators to try
the 0.1.0 quick start and report where setup breaks.

The ask is bounded: run the quick start, then try one adapter against one
repository of your choice, and report the exact command and first failure.
This is not a migration.

  GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
    jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
  cd kiln
  nix develop path:$PWD
  cargo run --locked -p kiln-cli -- demo

Open source, mixed license, installed from source with Nix. Not a sandbox; no
production qualification is claimed. No commitment on either side.

<name>
<PINNED_URL>
```

## Hosted-CI design-partner email

The pilot is proposed, not launched. Read
[the design-partner guide](design-partners.md) before sending; it records what
a conversation may and may not promise.

```text
Subject: A 30-minute conversation about managed CI?

Hi <name>,

I maintain Kiln, the open-source CI control plane. We are testing whether an
invite-only pilot for managed CI is worth running. It would connect to
repositories already hosted elsewhere, so a team keeps its source host and
gains exact-revision status.

To be clear: hosted CI is a bounded local implementation and not a launched paid
service, and it is not available today. There is no price to quote and no
waitlist.

Could we take 30 minutes to discuss your workload, source host, privacy
requirements and an acceptable spend cap? A wrong fit is a useful answer too.

Kiln is self-hosted today: open source, mixed license, installed from source
with Nix, not a sandbox, with no production qualification claimed.

<name>
```

## Warm-introduction note

```text
<name> suggested I reach out. I maintain Kiln, an open-source, self-hosted,
mixed-license CI control plane, installed from source with Nix and not a
sandbox. It binds results to exact revisions and replays recorded state. I am
looking for a few operators to run the 0.1.0 quick start, try one adapter and
report where setup breaks. No production claim; would 30 minutes be useful?
```

## Talk abstract

```text
Title: Exact Revisions and Replayable State: A Provider-Neutral CI Control Plane

A green check is a weak claim. It rarely says which revision was checked, what
the provider did, or what happened to a job that lost its connection.

This talk describes Kiln, an open-source CI control plane that keeps CI meaning
in deterministic state and delegates external effects to replaceable adapters.
The core is pure `no_std + alloc` Rust: commands become events and requested
effects, events evolve state, and replay returns no provider effects. Every gate
result binds one exact candidate, and a transport failure after provider
acceptance stays `Unknown` until reconciliation. Kiln is open source, mixed
license, self-hosted, installed from source with Nix, and not a sandbox;
no production qualification is claimed. The talk covers the state model, the
provider ports, adapter maturity, and where the project refuses stronger claims.
```

**Takeaways**

- A CI result should name an exact revision or candidate, not a branch.
- Unknown is a state, not a failure; reconciliation is part of the design.
- A pure core with narrow ports lets providers change without changing CI
  meaning.
- Bounded claims are a feature: every surface states what a pass does not prove.

## Launch feedback issue prompt

Pin this as a GitHub issue on the projection for the launch window.

```text
Thanks for trying Kiln 0.1.0. To make a report reproducible, include:

- the exact revision you built, and where you acquired it;
- platform and architecture;
- the exact command and the directory you ran it in;
- the expected and observed result, including the first error line;
- whether the failure was in the clone, the development shell, the build, or a
  provider adapter.

Report vulnerabilities through the security policy, not public issues.
```

## Follow-up cadence

| When | Action |
| --- | --- |
| T+2 | Thank the contact and ask for the exact command and revision |
| T+7 | Fix or document the first reproducible setup blocker, then reply |
| T+14 | Ask whether a second session is useful; do not ask for a testimonial |
| T+30 | Close the loop with every contact, including a not-a-fit outcome |
