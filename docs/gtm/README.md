# Kiln launch kit

This directory is the working set for the Kiln `0.1.0` open-source launch and
for design-partner discovery ahead of a hosted-CI pilot. It is public-safe: it
contains no private contact, customer name, price or unverified metric.

The kit is copy and procedure, not evidence. If a sentence here conflicts
with a canonical source below, the source wins.

## Release state

| Item | State |
| --- | --- |
| Product version | `0.1.0`; no tag or release exists |
| Canonical source | Radicle `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` |
| Projections | `https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git` and `https://github.com/OnixResearch/kiln` |
| Observed `main` | `428d03fcfb02478ed79d38453fb44f82c6a2bdd5` on both projections, 2026-11-12 |
| Distribution | Source plus the Nix flake; no crates.io, binary or container publication |
| Exercised platform | `x86_64-linux`; `aarch64-linux` is declared and unchecked |
| License | Mixed; see the [license map](../../LICENSES.md) |
| Hosted service | None. Planned; every gate in [the commercial plan](../commercial-launch.md) is open |
| Launch status | Prepared in the working tree, not published. Publish only after [the checklist](launch-checklist.md) is green |

## Open launch blockers

Observed 2026-11-12. These are owner actions or release-commit checks, not
missing documents:

- the launch preparation is not published; the public tree still shows the
  previous README and has no security policy;
- no `v0.1.0` tag or release exists on either remote;
- the private reporting form was not exercised as an outside reporter;
- secret-scanning push protection is disabled and `main` is unprotected on
  the projection;
- the projection has no description and no topics, and the organization
  profile does not list Kiln;
- the Octet check and the full `nix flake check` were not run, including the
  checks whose package selectors the rename changed, and the packaged
  `nix build path:$PWD#kiln` output was not exercised;
- the workspace catalog outside this repository still names `kiln-public`
  as Kiln's GitHub source;
- the quick start was not repeated from a credential-isolated checkout at
  this revision;
- the product name and domain are not cleared: an unrelated `kiln` crate
  exists on crates.io;
- no approver is named for the release notes and the announcement.

Resolved on 2026-11-12 at the operator's direction:

- private vulnerability reporting and secret scanning are enabled on the
  GitHub projection;
- `OnixResearch/kiln-public`, a stale second public repository, is private
  and no longer an entry point;
- the two packages whose names collided with crates.io are `onix-kiln-core`
  and `onix-kiln-cli`;
- `kiln --help` prints usage.

Hosted CI has its own blockers and stays out of the release: every gate
L0 through L7 is open, no price is approved, and no service exists.

## Assets

| Asset | Use | Primary reader |
| --- | --- | --- |
| [positioning.md](positioning.md) | Messages, audience, proof points with bounds, differentiation, objections, message rules | Anyone writing external copy |
| [launch-plan.md](launch-plan.md) | Two tracks, phases, channels, metrics, risks, go/no-go | Release owner |
| [launch-checklist.md](launch-checklist.md) | Dated observations and open owner-only actions; the publish sequence | Release owner |
| [release-notes-0.1.0.md](release-notes-0.1.0.md) | Draft release body | Release owner, evaluator |
| [announcement.md](announcement.md) | Long-form launch post draft | Content owner |
| [outreach.md](outreach.md) | Channel-specific copy and emails | Community owner |
| [demo.md](demo.md) | Ten-minute demo with exercised commands | Content owner |
| [design-partners.md](design-partners.md) | Discovery guide for the hosted-CI pilot, gate L0 | Commercial owner |

## Canonical sources

Do not invent product claims in launch copy. Use these:

- product entry point, capability map and claim boundary:
  [`README.md`](../../README.md);
- the detailed ledger with every retained receipt: [`docs/status.md`](../status.md);
- trust model and non-claims: [`docs/security-and-non-claims.md`](../security-and-non-claims.md);
- operating procedures and their bounds: [`docs/operator.md`](../operator.md);
- hosted composition and its limits: [`docs/hosted-ci.md`](../hosted-ci.md);
- the commercial offer and gates: [`docs/commercial-launch.md`](../commercial-launch.md);
- licensing: [`LICENSES.md`](../../LICENSES.md).

## Claim discipline

The launch is credible because the boundaries stay visible:

- say "exercised live for one recorded cohort", not "works with GitHub in
  production";
- say "fixture-tested" beside Gitea, Forgejo and Tangled;
- say "not a sandbox" wherever execution is mentioned;
- say "hosted CI is planned and not available" wherever hosted CI is
  mentioned, and quote no price;
- say "open source, mixed-license", not "MIT licensed";
- quote a count only with its scope sentence;
- never present a historical receipt as validation of the current revision.

## How to use this kit

1. Read [positioning.md](positioning.md) before writing any external copy.
2. Work [launch-checklist.md](launch-checklist.md) to green; re-observe every
   dated row.
3. Use [release-notes-0.1.0.md](release-notes-0.1.0.md) for the release,
   after replacing its qualification section.
4. Use [announcement.md](announcement.md) and [outreach.md](outreach.md) for
   posts, adapting the first paragraph per channel.
5. Rehearse [demo.md](demo.md) on the tagged revision.
6. Keep hosted-CI conversations private and follow
   [design-partners.md](design-partners.md).
7. After launch, keep the checklist and release notes current. Do not
   rewrite the announcement into a different claim set.
