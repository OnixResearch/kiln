# Repository hosting and recovery

Radicle is the canonical source and collaboration host. Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` under explicit operator authority.

## Local candidate

A publication candidate binds these facts:

- exact source revision;
- source-archive BLAKE3 identity;
- package and artifact-specific license terms from `LICENSES.md`;
- lifecycle evidence identity;
- rollback revision;
- hosting-profile identity.

Candidate validation performs no signing, push, synchronization, announcement, or mirror mutation.

## Historical publication receipt

The operator published exact candidate `36984e367eafd44c22d2b4f4f47121deb51eece3`. Its source archive BLAKE3 identity is `ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2`.

The publication uses one delegate, threshold one, and parent-feature signed refs. Native acquisition from Rosa and HTTPS acquisition from `seed.radicle.garden` reproduced the exact commit and archive identity.

See `evidence/publication-receipt.ncl` for those historical bounded publication facts and non-claims. `evidence/publication-candidate.ncl`, `evidence/release-input.ncl`, and `evidence/release-evidence.json` belong to that original candidate; their check declarations do not validate later source.

## Current local preparation

Record fresh checks against one exact source revision or explicitly identified source payload. Keep the source manifest and observed results together, distinguish the validated payload from a later evidence-only record, and retain failures rather than copying historical PASS declarations.

The `release-evidence` Nix check verifies deterministic rendering of the historical input. It does not execute those declared checks, rehash current source files, or establish a new candidate's readiness.

A new canonical publication requires a separately authorized operation and a new consistent receipt: candidate/archive, hosting profile, governed signed refs, native and HTTPS acquisition, and required replica observations must all refer to the same revision. Do not rewrite the original receipt to imply that local preparation signed, pushed, synchronized, or published new source.

## Verified local candidate

The local source candidate is `db552f504236b3e330be46618e2fd29fb864f190`,
named by local bookmark `publication-candidate-2026-10-15`. Its deterministic
USTAR source archive BLAKE3 is
`5a9a22c6f642027c77a428835d53082208bcb04287ee0747369c1a47651806c5`.

The [typed candidate binding](../.cairn/changes/prepare-public-release/evidence/candidates/2026-10-15-db552f504236/candidate.ncl)
records the exact license-map, lifecycle, hosting-profile, rollback, and source
identities. [Fresh clean-archive observations](../.cairn/changes/prepare-public-release/evidence/candidates/2026-10-15-db552f504236/verification.json)
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

The [bounded push receipt](../.cairn/changes/prepare-public-release/evidence/push-2026-10-15.json)
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

The [GitHub projection receipt](../.cairn/changes/prepare-public-release/evidence/github-push-2026-10-15.json)
retains the explicit destination, push and authenticated readback commands,
source identities, and access limitations. It is a later local record, not part
of either already-pushed frozen revision. Kiln maintainers own the repeatable
projection verification path.

## Recovery

Keep an independent source archive and signed-reference capture. Keep the rollback revision outside a running Kiln service.

If all seeds fail, report unavailable distribution. Require an explicit governed recovery action.

If a delegate is unavailable, do not lower the threshold automatically. Review any governance change as a new policy action.

If a mirror serves an older sequence, report rollback. Do not accept it as the current publication.

## Non-claims

Matching routes prove only the observed RID, revision, archive, route, and policy facts.

They do not prove future availability, delegate honesty, source correctness, CI correctness, or release eligibility.
