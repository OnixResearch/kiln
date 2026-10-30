# Radicle job publication

Kiln can publish a local `xyz.radworks.job` collaborative object for one exact
source commit. The object is the Tangled/Radicle CI job projection and is
separate from the `xyz.radicle.ci-job` status projection.

Kiln remains the source of CI state. A job object does not complete a run,
authorize a merge, or change a release decision.

## Wire schema

The adapter uses the `radicle-job` action schema:

```json
{"Request":{"oid":"<40 lowercase hex>"}}
{"Run":{"uuid":"<uuid>","log":"<http URL>"}}
{"Finished":{"uuid":"<uuid>","reason":"Succeeded"}}
```

`reason` is `Succeeded` or `Failed`. Actions are stored under the pinned
cohort's signed COB refs:

```text
refs/namespaces/<kiln-node-id>/refs/cobs/xyz.radworks.job/<object-id>
```

## Discovery and extension

When no object id is supplied, the shell scans at most the profile's
`max_objects_scanned` job objects, reduces each one, and matches the root
commit. No match creates a new object. One match is extended. More than one
match fails as `job_identity_ambiguous`; name the exact object id in the plan
instead. A named object bound to another commit fails as `job_commit_mismatch`.

The shell never refuses an object because another node published a head. The
pinned `radicle-cob` graph supplies every validated tip as a parent of the new
Kiln entry, and Kiln advances only its own signer namespace. Other namespaces
are not rewritten, removed, or overwritten. Entries with invalid signatures are
removed by the pinned change graph before reduction.

## Run identity and replay

The run UUID is the first 16 bytes of

```text
BLAKE3("kiln-radicle-job-v1" || repository || revision || run_id)
```

rendered as a canonical UUID. Publishing the same state again returns the
existing observation without a write. A started run can append one terminal
action. Replaying a different log, a different terminal reason, or a start
after terminal finish fails closed.

## Profile

`RadicleJobProfile` pins the job type (`xyz.radworks.job`), profile id
(`radicle-job-v1`), cohort versions, executable identity, repository allowlist,
signer identity, payload bound, log URL bound and object scan bound. It grants
no network peer, merge, identity, delegation, cache or canonical-state
authority. The resolved SecretSpec signer must equal the declared public node
identity before the first effect.

## Errors

- `job_repository_identity`, `job_repository_not_admitted`
- `job_revision_identity`
- `job_object_identity`, `job_object_identity_invalid`, `job_object_missing`
- `job_commit_mismatch`, `job_identity_ambiguous`
- `job_payload_bound`, `job_log_url_bound`, `job_log_url_invalid`
- `radicle_object_scan_bound`
- `job_run_identity_invalid`, `job_run_duplicate`, `job_run_bound`
- `job_finish_without_start`, `job_finish_duplicate`
- `job_replay_log_conflict`, `job_replay_reason_conflict`, `job_replay_start_after_finish`
- `job_action_invalid`, `job_action_encoding`, `job_root_invalid`
- `radicle_job_signer_mismatch`
- `radicle_store_create_failed`, `radicle_store_update_failed`, `radicle_store_error`

## Evidence boundary

Native fixtures call `RadicleJobShell::publish` and `.reconcile` against
temporary bare Git repositories with fixture-only signers. They cover fresh
creation, replay, start then finish, extension of another node's job, divergent
two-head joins, explicit object mismatch, signer/repository refusal, malformed
history and serialized observations. The read-only COB browser fixture shows a
job object with `Request`, `Run` and `Finished` actions.

These checks prove bounded local writes under one profile. They do not prove
peer convergence, remote visibility, node availability, CI correctness, build
or release authority.
