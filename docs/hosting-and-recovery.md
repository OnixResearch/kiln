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


## Mirrors

A mirror declares its direction and authority. A read-only mirror must reject writes.

Mirror agreement does not transfer Radicle governance. Mirror drift remains an explicit observation.

## Recovery

Keep an independent source archive and signed-reference capture. Keep the rollback revision outside a running Kiln service.

If all seeds fail, report unavailable distribution. Require an explicit governed recovery action.

If a delegate is unavailable, do not lower the threshold automatically. Review any governance change as a new policy action.

If a mirror serves an older sequence, report rollback. Do not accept it as the current publication.

## Non-claims

Matching routes prove only the observed RID, revision, archive, route, and policy facts.

They do not prove future availability, delegate honesty, source correctness, CI correctness, or release eligibility.
