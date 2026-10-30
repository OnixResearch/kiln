## Context

`xyz.radworks.job` is the Tangled/Radicle CI job COB. Its root `Request` binds
one Git commit; each node appends `Run` and `Finished` actions under its own
namespace for a UUID. Heartwood reduces all valid changes from all namespaces.

The repository already pins `radicle-cob 0.21.1` and the vendored COB Git store.
That cohort provides:

- `object::Storage::objects` returning every observed head for one object
  across `refs/namespaces/*/refs/cobs/<type>/<object>`;
- `ChangeGraph` traversal that drops entries whose signatures do not verify;
- `collaboration::update` creating the new signed entry with the reduced
  history's tips as parents, so appends extend every observed head;
- `collaboration::get` reducing the same graph under an `Evaluate`
  implementation.

No node-crate reducer is required for this schema. The node reducer is only
needed for Radicle patch/issue authorization, which remains out of scope.

## Decisions

### Wire compatibility

Actions serialize exactly as `radicle-job 0.6.0` derives them:

```json
{"Request":{"oid":"<40-hex>"}}
{"Run":{"uuid":"<uuid>","log":"<url>"}}
{"Finished":{"uuid":"<uuid>","reason":"Succeeded"}}
```

`reason` is `Succeeded` or `Failed`. The reducer starts one run per author and
UUID, rejects a duplicate start, and rejects a finish without a started run or
with a conflicting terminal reason. A later `Request` with the same commit is
ignored; a later `Request` with another commit fails the object.

### Job identity

A job object is identified by its native root commit id. Discovery reduces
every job in the bounded scan and matches the requested commit. One match is
extended. Multiple matches are ambiguous and require an explicit object id;
Kiln never guesses. An unreducible job fails the scan instead of risking a
duplicate for the same commit. An explicit object id must reduce to the same commit.

### Run identity

The run UUID is deterministic: the first 16 bytes of
`BLAKE3("kiln-radicle-job-v1" || repository || revision || run_id)`, rendered
as a UUID string. Replay therefore addresses the same run. Replay with the
same log and status returns the current observation without a write. A log or
status conflict fails closed. Starting after a terminal finish fails closed.

### Multi-writer append

Kiln never refuses an object because another node has a head. `update` observes
all validated tips, creates the Kiln entry with all of them as parents, and
advances only `refs/namespaces/<kiln-node>/...`. Other namespaces are never
written, deleted or overwritten. The vendored store's expected-ref guard is set
to Kiln's own namespace head when it exists, so concurrent Kiln writers cannot
silently fork Kiln's namespace.

### Bounds and authority

Admission checks the pinned profile id/type/cohort/executable, the repository
allowlist, exact lowercase 40-hex revision, bounded log URL, bounded payload,
bounded object scan and a non-zero maximum run count. The signer must equal the
profile's declared public identity before the first write. No network, merge,
identity, cache or canonical-state authority is granted.

## Risks

- The reducer must not treat an unsigned or invalid-signature entry as valid;
  that is delegated to the pinned `ChangeGraph` and covered by fixtures.
- An object with a conflicting reducer state fails closed rather than silently
  dropping Kiln's run.
- Creating a root and appending the first run is two writes. If the append
  fails after creation, replay discovers the root and appends the run; a root
  without a run is not reported as a successful publication.
