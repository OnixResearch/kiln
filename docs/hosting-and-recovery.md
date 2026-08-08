# Repository hosting and recovery

Radicle is the planned canonical source and collaboration host. Kiln has no assigned RID and no remote publication authority.

## Local candidate

A publication candidate binds these facts:

- exact source revision;
- source-archive BLAKE3 identity;
- `MIT OR Apache-2.0` license;
- lifecycle evidence identity;
- rollback revision;
- hosting-profile identity.

Candidate validation performs no signing, push, synchronization, announcement, or mirror mutation.

## Publication authority

Publication requires a separate operator action. The operator action must preserve the exact validated candidate.

The publication bundle binds the RID, signed revision, delegates, threshold, sequence, acquisitions, and archive identity.

This implementation does not perform publication. The RID remains unassigned.

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
