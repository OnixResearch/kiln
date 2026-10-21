# Repository hosting and recovery

Radicle is the canonical source and collaboration host. Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` under explicit operator authority.

## Local candidate

r[impl kiln.public_release.sources]

A publication candidate binds these facts:

- exact source revision;
- source-archive BLAKE3 identity;
- package and artifact-specific license terms from `LICENSES.md`;
- lifecycle evidence identity;
- rollback revision;
- hosting-profile identity.

Candidate validation performs no signing, push, synchronization, announcement, or mirror mutation.

## Historical publication receipt

r[impl kiln.mantle.cohort_refresh.records]

The operator published exact candidate `36984e367eafd44c22d2b4f4f47121deb51eece3`. Its source archive BLAKE3 identity is `ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2`.

The publication uses one delegate, threshold one, and parent-feature signed refs. Native acquisition from Rosa and HTTPS acquisition from `seed.radicle.garden` reproduced the exact commit and archive identity.

See `evidence/publication-receipt.ncl` for those historical bounded publication facts and non-claims. `evidence/publication-candidate.ncl`, `evidence/release-input.ncl`, and `evidence/release-evidence.json` belong to that original candidate; their check declarations do not validate later source.

## Current local preparation

r[impl kiln.public_release.claims]

Record fresh checks against one exact source revision or explicitly identified source payload. Keep the source manifest and observed results together, distinguish the validated payload from a later evidence-only record, and retain failures rather than copying historical PASS declarations.

The `release-evidence` Nix check verifies deterministic rendering of the historical input. It does not execute those declared checks, rehash current source files, or establish a new candidate's readiness. The input imports `evidence/release-stack-profiles.ncl`, a frozen snapshot of the stack profiles rendered into the historical evidence. Do not refresh that snapshot when current provider cohorts change; a new candidate requires a new input and evidence.

A new canonical publication requires a separately authorized operation and a new consistent receipt: candidate/archive, hosting profile, governed signed refs, native and HTTPS acquisition, and required replica observations must all refer to the same revision. Do not rewrite the original receipt to imply that local preparation signed, pushed, synchronized, or published new source.

## Source integration and production eligibility

r[impl kiln.public_release.gates]
r[related kiln.aspen_host.production_evidence]

Kiln maintainers and the authorized operator own production-eligibility review.
It is separate from source-ref publication. A production receipt must bind its
exact source, executable and profile identities to host, provider, restart,
corruption, finite-load, backup, rollback, authority and publication evidence.
The integration does not introduce an automatic production-eligibility validator.

r[related kiln.aspen_host.production_evidence.accepted]

Accept only the bounded local-host behavior actually supported by the exact
receipt and all required gates. Historical production-host records retain their
original cohort and date; current source checks do not refresh those records.

r[related kiln.aspen_host.production_evidence.rejected]

Reject a release-review claim that promotes machine-local observations into
global durability, exactly-once delivery, CI correctness, provider truth, host
sandboxing, availability, or implied release eligibility. A passing declared
traceability report links requirements to their owners and evidence locations;
it is not evidence that every production obligation was executed.

The existing profile-binding and observation-pacing changes retain their pending
provider-backed Onix shadow and sync/archive tasks. Source integration does not
mark those tasks complete, run a live workload, or promote their delta specs into
accepted requirements. Their implemented local paths remain covered by the
standalone conformance and isolated process checks.

The operator authorized a bounded [source-only publication exception](../.cairn/archive/1970-01-01-integrate-canonical-main/evidence/source-only-exception.ncl)
for checked merge candidate `d11c5592ff990594d3ad53e8dd80240f6ad8e927`.
All 30 x86_64 Nix checks passed and all 619 accepted parent requirement bodies
were preserved. At that candidate, declared traceability was **FAIL**: 622 of 625 referenced,
with the three `kiln.aspen_host.production_evidence` obligations still missing
implementation/verification evidence and no dangling IDs. The six implemented
staged IDs were declared requirement documents, not newly accepted specs.
The [candidate-bound verification record](../.cairn/archive/1970-01-01-integrate-canonical-main/evidence/verification.json)
retains the failure and the isolated runtime scopes. This exception covers only
this source and evidence-only descendants; it does not approve production use,
waive the production obligations, or change the default gate into a PASS.

## Exact-cohort local qualification

`kiln host-qualify COHORT RECEIPT ARTIFACT_ROOT` checks the operator's exact
expected cohort, all required observation kinds, bounded claims and retained
BLAKE3 artifact bytes. It does not attest observation truth or grant production,
publication, release or deployment authority.

The [qualification evidence](../.cairn/archive/1970-01-01-qualify-aspen-host-production-evidence/evidence/verification.md)
binds frozen source candidate `afd06231e8e616c86a6f507fb2a69c41babd1f84`
and frozen native executables; its publication binding was explicitly unpublished
at qualification time, independently of any later authorized source push.
Native child-process drills retain their exact per-case profile variants and
closed Lattice transport-fixture scope. The separately identified real
Lattice/Nix observation retains its successful provider report and exact
acknowledgement replay after SIGKILL with Lattice stopped. Neither scope proves
CI correctness or external provider truth.

Earlier provider failures and a quarantined recovery rejection remain retained;
they are not successful checks. A minimal shell workflow's `completed`
acknowledgement alone is not provider success. All 30 x86_64 Nix checks and the
separate native host/provider gates passed for this frozen input. The actual
receipt consumer admitted only the complete bounded-local-host receipt and
rejected missing, failed, stale, mixed, stronger-claim and damaged-artifact
inputs. Declared traceability at that source was 625/625; this is linkage, not
execution truth or release authority. Exact gate and consumer results belong to
the linked evidence, not the historical source-only exception.

The separately authorized [qualified-candidate publication](../.cairn/archive/1970-01-01-qualify-aspen-host-production-evidence/evidence/publication.json) advanced both declared `main` refs to `c9266e05d3d44f8d0be06aa60c69edd6b3586adb`. Fresh GitHub SSH, namespace-scoped native Radicle and anonymous Garden HTTPS checkouts reproduced all 1441 regular members and the 23767040-byte normalized USTAR archive, BLAKE3 `561e1106ae4c836d1042042818ad181b97b190214a98f92430015ea213826c61`. Iris, Rosa and Garden namespace APIs agreed at read time; public visibility, the delegate and threshold were unchanged. The post-push receipt is retained in a local evidence-only child, outside that published source archive. This source publication grants no deployment, release or credential authority.



## Integrated main publication

The source-plus-verification revision `cd8b810028fafd3bcb531e920f5360e1f52f6fda`
was fast-forwarded to both declared `main` refs. It descends from GitHub main
`1867f611a7968211e29830f4f1a6d5c4ff568138` and canonical Radicle main
`5d99f4e9bb954743443e340baece31256b4c53f1`; neither history was replaced.

Fresh GitHub SSH, native Radicle and anonymous Garden HTTPS clones read back
that exact revision. Each reproduced all 972 entries of the checked source
candidate and its 8,294,400-byte normalized USTAR archive, BLAKE3
`5faa879fa3c23087c37819e802bd6fbd5d5bccdf54a896c7b775cffd840b442a`.
Rosa, Iris and Garden delegate APIs reported the same `main`; governance retained
the original delegate and threshold. These are bounded observations, not seed
trust, indefinite availability or production qualification.

The [publication receipt](../.cairn/archive/1970-01-01-integrate-canonical-main/evidence/publication.json)
binds the checked source, archive, hosting profile, signed refs, acquisitions and
operator exception. It is a later evidence-only descendant of the revision it
records. Current tips remain moving refs; evidence-only descendants do not imply
new executable verification. Historical publication records remain unchanged.

Lattice revision `6339864d3c4e5de72ba4f0899b11b5d5b76704b6` was independently
read back over GitHub SSH and its existing private Radicle namespace. Anonymous
Lattice acquisition is not claimed, and no repository visibility was changed.


## Verified local candidate

The local source candidate is `db552f504236b3e330be46618e2fd29fb864f190`,
named by local bookmark `publication-candidate-2026-10-15`. Its deterministic
USTAR source archive BLAKE3 is
`5a9a22c6f642027c77a428835d53082208bcb04287ee0747369c1a47651806c5`.

The [typed candidate binding](../.cairn/archive/1970-01-01-prepare-public-release/evidence/candidates/2026-10-15-db552f504236/candidate.ncl)
records the exact license-map, lifecycle, hosting-profile, rollback, and source
identities. [Fresh clean-archive observations](../.cairn/archive/1970-01-01-prepare-public-release/evidence/candidates/2026-10-15-db552f504236/verification.json)
retain commands, cleared-environment scope, logs, failures, and non-claims.
Kiln maintainers own this repeatable preparation path.

Unfinished Onix evaluator work remains in the operator workspace and is excluded
from the candidate. Source and evidence are separate: local bookmark
`publication-evidence-2026-10-15` names the later receipt/documentation revision,
not a new tested source candidate. The original publication files remain
unchanged. That preparation stopped before signing, push, synchronization,
announcement, or publication; the later authorized ref push is recorded below.

This archive uses the declared normalized USTAR recipe, not the historical
`git-archive-tar` byte format. Any later native/HTTPS acquisition comparison must
reconstruct this candidate's declared recipe before comparing its archive hash.



## Authorized candidate-ref push

The subsequent operator-authorized push added only these delegate-namespace refs
to the existing canonical `rad` remote:

| Ref | Exact revision |
|---|---|
| `publication-candidate-2026-10-15` | `db552f504236b3e330be46618e2fd29fb864f190` |
| `publication-evidence-2026-10-15` | `9e6f565789744a65fd2173b8800755a2f7061d79` |

Both ref values were observed through the HTTPS namespace APIs on
`iris.radicle.xyz`, `rosa.radicle.xyz`, and `seed.radicle.garden`, and through a
namespace-scoped native clone. A native source checkout matched all 719 manifest
entries and reproduced the exact 6,051,840-byte USTAR archive and BLAKE3 above.
These observations do not establish that three HTTPS origins are independent
physical replicas.

`main` was not selected for the push and remains at
`5d99f4e9bb954743443e340baece31256b4c53f1`. The delegate, threshold, and identity
payload remained unchanged. Parent-feature signed refs advanced from
`33f81572c6f508e8a4475d67ca2d7bba4049d312` to
`3f75ae151e913d09f78b4909bc31d5e7719d9905`.

With the repository already present in local Radicle storage, this command
acquires the candidate through the exercised Jujutsu native-helper route:

```console
jj --config 'signing.behavior="drop"' git clone --no-colocate \
  --branch publication-candidate-2026-10-15 \
  rad::rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE/z6MksnXbFoE8zkCkGWhHc8zuxpnEUhrJHv2KECRV4GSv9gkx kiln
```

The explicit `rad::` helper prefix avoids the observed Jujutsu 0.45.1 clone
behavior that lowercased the case-sensitive RID. The generic `rad://<rid>` fetch
view and plain HTTPS Git projection expose canonical refs, not these
delegate-only heads; both continued to advertise only `main`. Use the delegate
namespace for readback rather than interpreting absent canonical heads as a
failed push. See the upstream [canonical-reference model](https://radicle.dev/2025/08/12/canonical-references).

The [bounded push receipt](../.cairn/archive/1970-01-01-prepare-public-release/evidence/push-2026-10-15.json)
retains authorization, exact refs, commands, observed failures, readback excerpts,
and non-claims. Kiln maintainers own this repeatable verification path. This
receipt is a later local record, not part of the already-pushed immutable
evidence revision. No default-branch promotion, governance change, binary
release, or fresh product-check run is claimed. Historical receipts retain
their original meaning.

## Mirrors

A mirror declares its direction and authority. A read-only mirror must reject writes.

Mirror agreement does not transfer Radicle governance. Mirror drift remains an explicit observation.

The operator authorized a one-way projection to
`git@github.com:OnixResearch/kiln.git`. Only the two frozen publication refs above
were pushed; no GitHub `main` ref update or automatic mirroring was requested.
Radicle remains canonical. A fresh authenticated SSH clone matched both exact
ref identities, all 719 source-manifest entries, and the normalized USTAR archive
identity above.

Anonymous HTTPS cloning requested credentials. Public repository metadata
returned HTTP 404, while the available `gh` metadata lookup returned HTTP 401.
Public access and GitHub visibility were not established; visibility settings
were not changed. This is not a claim of anonymous GitHub source availability.

The [GitHub projection receipt](../.cairn/archive/1970-01-01-prepare-public-release/evidence/github-push-2026-10-15.json)
retains the explicit destination, push and authenticated readback commands,
source identities, and access limitations. It is a later local record, not part
of either already-pushed frozen revision. Kiln maintainers own the repeatable
projection verification path.

### Public historyless source projection

The separate [public GitHub source repository](https://github.com/OnixResearch/kiln-public)
contains parentless commit `35d10f2ebb3a6901d2aab273a467940ad8581bf6`.
Its source bytes are the frozen candidate
`db552f504236b3e330be46618e2fd29fb864f190`, not the later working tree.
The new projection commit has a different Git identity; it is not a
revision-preserving mirror or a dependency fallback.

A fresh HTTPS clone with operator credentials disabled reproduced all 719
source-manifest members, file bytes, executable flags, symlink facts, and the
normalized USTAR and manifest BLAKE3 identities above. The public branch has one
reachable commit. Private development history was not uploaded, and the original
`OnixResearch/kiln` repository remains private. Actions are disabled in the new
repository.

The [source-publication receipt](../.cairn/archive/1970-01-01-prepare-public-release/evidence/github-source-publication-2026-10-17.json)
records this projection and eight producer source repositories, their exact
payload selections, anonymous acquisition observations, and disabled Actions.
The [bounded scan review](../.cairn/archive/1970-01-01-prepare-public-release/evidence/github-source-projections-2026-10-17/scan-review.json)
retains the original scanner exit codes and evidence for known matches; it is
not exhaustive privacy, rights, or legal-compatibility clearance. Trellis has
only an empty private staging repository pending its required qualified
Corresponding Source review.

Kiln maintainers own this projection's repeatable acquisition and source-identity
checks; each producer owns its source and license boundary. No automatic
mirroring, canonical Radicle operation, governance change, consumer-pin update,
binary release, runtime qualification, or fresh product-check run is claimed.
The earlier private-projection observations above retain their original scope.

For Durable File Publication, the operator's GitHub-hosting request is an explicit
exception to the snapshot's historical Radicle-only public-distribution prose.
The GitHub repository metadata declares a noncanonical source view; its canonical
Radicle identity and immutable Radicle release-channel contract remain unchanged.
This is not authorization to substitute GitHub objects in dependency admission.

## Source-only publication of candidate `f748acd0`

The operator authorized one exact source-only cutover
([`operator-authority.ncl`](../.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/operator-authority.ncl)).
Both declared `main` refs were fast-forwarded from
`2c529b154d625735c43246cad2178d70813a01c3` to
`f748acd0cc239f48bf3565ccb89d46ce88cbee2f` (three local commits; no history was
replaced and no force push was used). The GitHub default branch changed from
`publication-candidate-2026-10-15` to `main`, and the GitHub projection changed
from private to public. No governance, delegate or threshold change was made.

The candidate's deterministic normalized USTAR source archive is BLAKE3
`00d59178898714dbe136ad451d88b418771cb1fcc525d3dd152b1885c85fe9dd`
(30,709,760 bytes, 1,560 entries); its compact sorted-key source manifest is
BLAKE3 `c2da897ba6c01a070916feb55fbda9e45c07ab598519d8718f3552d110831100`
(330,268 bytes, 1,560 entries). The same export, manifest and archive recipe
reproduced the earlier `publication-candidate-2026-10-15` identities exactly, so
the recipe itself is not new. Every tracked path, mode and BLAKE3 of the checked
copy matched the candidate revision, and extracting the archive reproduced that
copy.

Fresh anonymous clones with cleared HOME, no credentials, no SSH and no global
Git configuration reproduced the revision and the archive identity: GitHub HTTPS
`https://github.com/OnixResearch/kiln.git` and the read-only canonical projection
`https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git` (Git smart-HTTP
protocol v0) both read back `f748acd0`, all 1,560 entries, manifest
`c2da897b…` and archive `00d59178…`. `iris`, `rosa` and `seed.radicle.garden`
namespace APIs agreed on `main` and on the delegate namespace head, governance
remained one delegate at threshold one, and the local node reported the revision
in sync with 19 peers after the push.

All 24 credential-isolated root-workspace `checks.x86_64-linux` attributes passed
with cleared HOME/CARGO_HOME/XDG paths, disabled SSH, credential helper, askpass,
Nix tokens and netrc, no remote builders, and only `cache.nixos.org` as a
substituter; the packaged CLI accepted `demo-start`. Cold flake-source acquisition
into an isolated store without substituters and a pinned `cargo 1.88.0` fetch with
zero Git-sourced workspace packages passed separately.

The six optional runtime and Octet attributes (`aspen-runtime-octet-deny-all`,
`kiln-aspen-host-octet-deny-all`, `kiln-aspen-host-package`,
`kiln-radicle-nix-provider-octet-deny-all`, `kiln-radicle-nix-provider-package`,
`radicle-broker-wire-octet-deny-all`) were **not exercised** in this cutover.
Instantiating them stalled in the host's shared Nix daemon: the client blocked on
the daemon socket with effectively no client CPU for hours, with remote builders
disabled and the evaluation cache disabled, while every other attribute of the
same flake evaluated in seconds. They were cancelled rather than reported as
passing; no source, pin, gate, policy or cohort was changed to work around the
host behavior. These attributes select exactly the optional producer cohorts whose
private upstream sources are already declared outside the credential-free
contributor build. Earlier "all 30 `x86_64-linux` checks passed" results belong to
their own frozen inputs and are not reused here.

[`verification.json`](../.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/verification.json)
retains the commands, environments, identities, the non-exercised attributes and
the failed or cancelled attempts;
[`publication.json`](../.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/publication.json)
is the later evidence-only receipt for the published revision, together with the
anonymous readbacks, GitHub projection settings and post-push governance
observations in the same directory. Rollback for `main` is
`2c529b154d625735c43246cad2178d70813a01c3`; the previous GitHub default branch and
visibility are recorded in that receipt. Changing visibility back cannot retract
already copied public data.

## Source-only publication of the qualified source `c955334f`

r[impl kiln.hosting.source_only_publication]

The operator authorized one source-only publication of the self-CI-qualified
source. Canonical Radicle `main` was fast-forwarded from
`305f85d80b9ea05fbd91db73de8a6662a55650ee` to
`c955334f54bdfe6e155e56c00573ca7ba77c4fb1` with no force push, no history
rewrite, no governance, delegate, threshold, visibility or announcement change,
and no mirror publication. The GitHub projection was observed but not mutated
and retains its own revision.

The candidate binds the deterministic normalized USTAR source archive
(BLAKE3 `28fec0fa2b3a71c4c4e725d4640080a07d18d4a5929ada05d7cd9b81eff38004`,
60,989,440 bytes, 4,439 tracked members), the lexically sorted path manifest
(`8001e6b5cbc334cdb1d46611e71d824a4deee82d399937983a19743c75a45383`), the
hosting profile export, the license map and the self-CI qualification.

Fresh native Radicle and anonymous Garden HTTPS acquisitions both reproduced
`c955334f`, all 4,439 tracked paths, that path manifest and that archive
identity. The delegate set and threshold one were unchanged, signed refs
advanced from `e9d43a1cbe8befa6a1fc08dbe5630a99e23599f7` to
`27455a353402ecaa0bd9c6500a953c6d341c1575`, and the public visibility was
preserved. A credential-isolated anonymous clone with a cleared home, disabled
global and system Git configuration, no credential helper, no askpass, no
inherited SSH agent and Git protocol v0 ran the workspace test suite to
completion.

[`publication.json`](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/publication.json)
is the later evidence-only receipt for the published revision, with the
readbacks, governance observation, repository checks and replica observation
in the same directory. Rollback for `main` is
`305f85d80b9ea05fbd91db73de8a6662a55650ee`. This publication grants no
deployment, activation, binary release or release authority, and does not
qualify changed executables of any later revision.

## Source-only publication of the evidence revision `acc359e7`

The operator authorized a second source-only publication of the local evidence
revision `acc359e769bb3a056279bf5db8ba21e95c294a4c`. Canonical Radicle `main`
was fast-forwarded from `c955334f54bdfe6e155e56c00573ca7ba77c4fb1` and the
read-only GitHub projection was advanced from
`305f85d80b9ea05fbd91db73de8a6662a55650ee` to that same revision. Neither push
used force, rewrote history, or changed governance, delegates, threshold,
visibility, announcements, or deployment state.

That revision's deterministic normalized USTAR source archive is BLAKE3
`1f72af29b005a7162c45b4fbd60ba94c19698451663e1285c91f6b2496e69a9a`
(61,480,960 bytes, 4,457 tracked members) and its lexically sorted path
manifest is `e97163d1367f5c76e6541683f06266f9866f28b2261ecb9ee204e19f4a4e7a83`.
Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS all
reproduced that revision, every tracked path, the path manifest, and the
archive identity. A credential-isolated anonymous clone with a cleared home,
disabled global and system Git configuration, no credential helper, no askpass,
no inherited SSH agent and Git protocol v0 ran the workspace test suite with
527 tests passed and 0 failed. Signed refs advanced from
`27455a353402ecaa0bd9c6500a953c6d341c1575` to
`0bae3cd81083e8c24a6b1a926c96a409bd584040`; one delegate, threshold one and
public visibility were unchanged.

[`publication.json`](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/evidence-revision-publication-2026-10-21/publication.json)
is the later evidence-only receipt for that revision, with the readbacks,
governance observation, repository checks, replica observation, command record
and source path manifest in the same directory. Rollback for canonical `main`
is `c955334f54bdfe6e155e56c00573ca7ba77c4fb1` and for the GitHub projection is
`305f85d80b9ea05fbd91db73de8a6662a55650ee`. The GitHub projection remains a
read-only projection and never carries Radicle governance. This publication
grants no deployment, activation, binary release or release authority.

## Recovery

Keep an independent source archive and signed-reference capture. Keep the rollback revision outside a running Kiln service.

If all seeds fail, report unavailable distribution. Require an explicit governed recovery action.

If a delegate is unavailable, do not lower the threshold automatically. Review any governance change as a new policy action.

If a mirror serves an older sequence, report rollback. Do not accept it as the current publication.

## Non-claims

Matching routes prove only the observed RID, revision, archive, route, and policy facts.

They do not prove future availability, delegate honesty, source correctness, CI correctness, or release eligibility.
