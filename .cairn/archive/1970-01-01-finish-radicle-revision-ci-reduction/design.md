## Design

Use the existing process boundary around radicle 0.25.1 rather than linking it into core. Bound the reachable Git DAG before calling native reducers. Return exact heads with typed native projections; compare the observed set again before any append. Native reducers alone decide actor authorization. CI derives the exact revision from this projection and verifies author signed refs before reserving a durable per-revision record. Approval binds patch, revision, base and oid, and untrusted execution must omit privileged resources. Existing push serialization is unchanged.

## Verification

Local synthetic signer/repository fixtures cover concurrent native reviews, denied merge, stale head refusal, exact two-revision CI, new-revision reapproval, duplicate events and unchanged push replay. Parent runs integrated checks once after edits; no live remote mutation is authorized.
