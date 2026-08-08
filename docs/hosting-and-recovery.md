# Repository hosting and recovery

Radicle is the canonical source and collaboration host. Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` under explicit operator authority.

## Local candidate

A publication candidate binds these facts:

- exact source revision;
- source-archive BLAKE3 identity;
- `MIT OR Apache-2.0` license;
- lifecycle evidence identity;
- rollback revision;
- hosting-profile identity.

Candidate validation performs no signing, push, synchronization, announcement, or mirror mutation.

## Publication receipt

The operator published exact candidate `36984e367eafd44c22d2b4f4f47121deb51eece3`. Its source archive BLAKE3 identity is `ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2`.

The publication uses one delegate, threshold one, and parent-feature signed refs. Native acquisition from Rosa and HTTPS acquisition from `seed.radicle.garden` reproduced the exact commit and archive identity.

See `evidence/publication-receipt.ncl` for the bounded publication facts and non-claims.

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
