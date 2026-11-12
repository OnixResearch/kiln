# Kiln 0.1.0 launch plan

This plan is a proposal for the operator. It authorizes no tag, publication,
repository setting, announcement, offer or charge. Dates are relative to the
tag day `T-0`; no calendar date is promised.

## Objective

Publish one tagged source release of Kiln `0.1.0`, make the first run easy,
learn where self-hosted evaluation breaks, and start design-partner discovery
for the hosted-CI pilot without presenting hosted CI as available.

Success is not a star count. Success is:

- a new operator goes from the README to the `demo` output and the dashboard
  preview without private help;
- a few operators attempt a real adapter setup on their own repository and
  report where it breaks, naming revision and command;
- three qualified design-partner interviews are retained privately, which is
  the proposed discovery gate L0 of [the commercial plan](../commercial-launch.md#6-dependency-ordered-launch-backlog);
- no launch claim outruns the retained evidence.

## Non-goals

- a hosted or paid service, a price, a waitlist or a beta;
- crates.io publication, a binary release or a container image;
- a production, isolation, compliance or SLA claim;
- GitHub Actions compatibility;
- a benchmark or performance claim.

## Two tracks

| Track | What becomes public | Gate |
| --- | --- | --- |
| A. Open-source release | The tagged `0.1.0` source, the README, the documentation and this kit | Hard gates in [the launch checklist](launch-checklist.md#10-gono-go-review) |
| B. Hosted-CI discovery | Nothing beyond the README section [Hosted CI direction](../../README.md#hosted-ci-direction). Conversations are private | Gate L0 of the commercial plan; follow [the design-partner guide](design-partners.md) |

The tracks share one rule: track A copy never presents track B as available.

## Launch thesis

Teams that operate their own CI lack a control plane whose meaning survives a
change of forge or executor and whose results name their exact source. Kiln
fits that job: a deterministic core, exact-candidate results, explicit
`Unknown` outcomes and replaceable adapters. Kiln is early. The launch asks
for evaluation and specific feedback, not production adoption.

## Message map

| Audience | Message | Proof to show | Call to action |
| --- | --- | --- | --- |
| Platform or infrastructure engineer running CI on Linux | "Your CI decisions are recorded, replayable and bound to exact revisions." | `demo` output, dashboard preview, [architecture](../architecture.md) | Run the quick start |
| Team with source on Radicle, Forgejo or Gitea, or using Jujutsu or Pijul | "One CI meaning while source and forge providers change." | Ports table, capability map with evidence words | Read the adapter procedure for your host |
| Reviewer of a growing change volume | "Approvals and required CI bind to the exact candidate." | [Change review](../operator.md#optional-interactive-change-review) and its bounded receipt | Read the review procedure |
| Nix user | "Nix job discovery and exact derivation builds are providers, not the whole system." | [Discovery](../nix-eval-jobs-adapter.md) and [build](../nix-build-adapter.md) adapters, exercised locally | Try the local rails |
| Skeptical CI operator | "It is not a sandbox and claims no production qualification; here is what was actually exercised." | [Proof points](positioning.md#proof-points) with bounds | Read the claim boundary |
| Candidate design partner | "Keep your source where it is; we are testing whether managed CI for it is worth building." | Status sentence and the commercial plan | A 30-minute discovery conversation |

## Phases

### Phase 0: readiness (T-7 to T-1)

Exit criteria are the hard gates of the checklist:

- this change is published through the canonical procedure and both
  projections resolve the same `main`;
- the documented repository gates were run on the release commit and their
  results, including failures and gates not run, are recorded;
- the quick-start and demo commands pass on the release commit from a
  credential-isolated checkout;
- the vendored license texts named by the license map are present;
- private vulnerability reporting is enabled on the GitHub projection and the
  link in `SECURITY.md` was tested;
- the second public repository `kiln-public` no longer competes as an entry
  point;
- release notes and announcement have a named approver.

### Phase 1: soft launch (T-0 to T+7)

- Tag `v0.1.0` on the verified commit and publish the release notes.
- Invite a small number of self-hosting operators individually. Ask for a
  bounded evaluation: run the quick start, attempt one adapter, report where
  setup breaks. Do not ask for a testimonial.
- Open the pinned launch-feedback issue.
- Begin hosted-CI discovery conversations in private.
- Do not post to broad channels until one person outside the maintainer group
  has reproduced the quick start.

### Phase 2: public launch (T+7 to T+14)

- Publish the announcement.
- Post once each to the channels in the plan below, adapting the first
  paragraph and disclosing maintainer status.
- Publish one terminal and browser recording of [the demo](demo.md).
- Name no evaluator or design partner without written permission.

### Phase 3: sustain (T+14 to T+30)

- Triage issues by revision, platform, command and adapter.
- Ship one small change that removes a reported onboarding blocker.
- Hold the T+30 review below and decide the next public artifact.
- Decide from recorded demand which fixture-tested integration to exercise
  live next. Default to none until an operator asks for it.
- Record the L0 outcome in private business records and update the
  commercial plan only with facts.

## Channel plan

| Channel | Asset | Timing | Notes |
| --- | --- | --- | --- |
| Canonical Radicle repository and GitHub projection | Tag and [release notes](release-notes-0.1.0.md) | T-0 | Primary source of truth |
| Repository README | Landing surface | T-0 | No product domain exists; reference none |
| Direct invitations to self-hosting operators | [Outreach](outreach.md) evaluator email | T-0 onward | Personal, bounded ask |
| Show HN | [Outreach](outreach.md) | T+7 | Disclose maintainer status; answer with evidence and bounds |
| Nix community forum | [Outreach](outreach.md) | T+7 | Lead with the Nix adapters and their local-exercise bound |
| Rust community channels and newsletters | [Outreach](outreach.md) | T+7 to T+14 | Submit once; follow each channel's self-promotion rules |
| Radicle community | [Outreach](outreach.md) | T+7 | State the same-host and private-tailnet bounds |
| Design-partner email | [Outreach](outreach.md) and [guide](design-partners.md) | T-0 onward, private | Carries the not-a-launched-service sentence |
| Meetups and conferences | Talk abstract in [outreach](outreach.md) | T+14 onward | Prefer the demo and a boundary discussion |

## Content calendar

| Day | Asset | Owner |
| --- | --- | --- |
| T-7 | Run and record repository gates on the release candidate | Release owner |
| T-5 | Approve positioning, release notes and announcement | Maintainer |
| T-3 | Rehearse the demo on the release candidate | Content owner |
| T-1 | Credential-isolated quick-start smoke; final checklist | Release owner |
| T-0 | Tag, release notes, repository metadata, feedback issue | Release owner |
| T-0 | First evaluator and design-partner invitations | Community and commercial owners |
| T+7 | Announcement and community posts | Community owner |
| T+10 | First metrics review | Maintainer |
| T+30 | Retrospective and next-artifact decision | Maintainer |

## Metrics

These are proposed measurements with proposed first targets. Replace each
target with a measured baseline after 30 days. This plan relies on no product
telemetry: every number comes from issues and direct follow-up. Do
not publish a metric without its scope and source.

| Metric | Definition | Source | Proposed first target |
| --- | --- | --- | --- |
| Activation | An operator reports the `demo` output and the dashboard preview on their own machine | Feedback issue, direct follow-up | 10 independent reports |
| Real adapter attempt | An operator reaches a first exact-revision status on their own repository | Issue reports, follow-up | 3 operators |
| Setup friction | The step where an evaluation stopped | Issue reports | Recorded for every stopped evaluation |
| Issue quality | The issue names revision, platform, command, expected and observed result | Issue tracker | 80% of launch issues |
| Support load | Maintainer minutes per evaluator per week | Manual log | Under 60 minutes |
| Claim quality | Public copy with an unsupported claim | Review against [positioning](positioning.md#message-rules) | Zero |
| Discovery | Qualified hosted-CI interviews retained privately | Private records | 3, the proposed L0 gate |

Stars, forks and impressions are awareness signals only. Do not use them, or
commit, task or archive counts, as evidence of product value.

## Roles

| Role | Responsibility |
| --- | --- |
| Release owner | Gates, tag, canonical publication, projection read-back, rollback |
| Maintainer | Claim review, security and licensing decisions, issue triage |
| Content owner | Announcement, demo rehearsal and recording |
| Community owner | Channel posts, evaluator follow-up |
| Security owner | Private vulnerability reporting and disclosure handling |
| Commercial owner | Design-partner interviews, private records, L0 decision |

One person can hold several roles. Name the people in a private launch
tracker, not in this document.

## Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Copy implies a hosted service exists | False availability claim; breaks the commercial plan's rule | README and kit state planned and not available; the hosted email carries the status sentence |
| "Production-ready" misreading | Credibility damage; unsafe deployments | Non-claims in every asset; evidence words in the capability map |
| Kiln used as a sandbox for untrusted code | Security incident for an adopter | Objection handling; adapters refuse fork pull requests by default; security policy states the trust model |
| Setup needs Nix, a reviewed Lattice runtime, SecretSpec and typed profiles | Low activation beyond the demo | Lead with `demo` and the dashboard; ask evaluators for one adapter only; record the stopping step |
| First build compiles the workspace from source | Slow first impression | State it in the quick start; decide on a published build cache only from recorded demand |
| License misread as permissive-only or as AGPL-only | Wrong adoption decision | License row in the README table; link the license map in every asset |
| Two public GitHub repositories | Evaluators land on a stale tree | Resolve `kiln-public` before launch |
| The name collides with unrelated `kiln` crates on crates.io | A reader installs a different project; name clearance is an open commercial gate | Show no `cargo install` command; send readers to the repository; the operator clears the name |
| Vulnerability reported in a public issue | Disclosure risk | Enable and test private vulnerability reporting first |
| Garden HTTPS clone fails for a client | Broken first command | Quick start also gives the GitHub projection |
| Projections disagree on `main` | Evaluators test different source | Read back both routes before announcing |
| A fixture-tested integration is assumed live | Failed evaluation, lost trust | Evidence words beside Gitea, Forgejo and Tangled in every asset |
| A historical receipt is read as current validation | False claim | Proof points carry cohort, date and bound |
| Support load exceeds one maintainer's capacity | Slow or no answers | Bounded asks; no response-time promise; issue template fields |

## Go/no-go criteria

Launch only when every hard gate in
[the checklist](launch-checklist.md#10-gono-go-review) is checked. If one
fails, delay. A smaller launch with accurate claims is better than a broad
launch that teaches readers the evidence is unreliable.

## Rollback and correction

- If a documented command is broken, correct the README and release notes
  first, then publish a fixed revision. Do not ask readers to trust a moving
  `main` without saying so.
- If a claim is wrong, correct it in the original asset and add a dated note.
  Do not silently rewrite it.
- If a vulnerability is reported, follow `SECURITY.md` and keep details out of
  public channels until coordinated.
- If private material is published, stop promotion, rotate the affected
  credential and record the incident before resuming. Published history
  cannot be assumed removable.

## Post-launch review

At T+30, answer:

1. How many independent operators reached the `demo` output and the dashboard
   preview?
2. How many attempted a real adapter, and at which step did each stop?
3. Which claims were misunderstood?
4. Did any launch copy overstate the evidence?
5. Which adapter or provider did evaluators ask for first?
6. What did the discovery interviews say about workload, privacy and
   acceptable spend? Record the answer privately.
7. What is the next public artifact: a setup guide, a recorded live exercise
   or a packaging improvement?
