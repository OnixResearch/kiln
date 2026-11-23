## Context

The worker route freezes the bytes it sends as `request-<BLAKE3>.json` and
records `FrozenRequest {path, blake3}` for the launch in the transport intent's
transaction. `Implementer::dispatch_request` and `Implementer::dispatch_launch`
return the record and the launch. Recovery reads exactly that file. The
campaign export resolved the file and state root with a private
`exported_request` in `kiln-web-service`. A process outside Kiln has no way
to read the record.

`Implementer::open` opens the SQLite file read-write. It creates the file
when it is missing and runs `CREATE TABLE IF NOT EXISTS`. `attempt`,
`attempts`, `work` and `dispatch_requires_recovery` run in an immediate
transaction that saves the work and attempt documents back, which bumps
their versions. Neither can serve a read-only report.

## Decisions

### Decision: one resolver, two callers

**Choice:** `exported_request` moves to `kiln_adapter_lattice::worker` as
`sent_request_and_state(launch, sent, profile, admitted_bytes)`, next to
`sent_request_path`. The campaign export and the new mode both call it. The
state root is the recorded launch's. Without a recorded launch it falls back
to the profile's.

**Rationale:** the rule (the recorded file, or for an earlier binary's launch
the admitted bytes frozen under their digest) must not drift between the
in-process export and the reported one.

### Decision: a read-only report mode on the existing dispatcher

**Choice:** `--sent-request true` on `kiln-lattice-dispatch`, checked before
every dispatch route, so a report can never fall through to a dispatch.
The mode takes the attempt and fence from the admitted request's
`envelope.attempt_id` and `envelope.fence`, the fields `worker::run` checks
against the plan. It refuses when any of these fails:

- the ledger holds that attempt with that fence;
- the attempt's bind-dispatch seal (`dispatch_request_binding`) names the
  attempt, the fence and the BLAKE3 of the admitted bytes;
- the transport intent is recorded.

The mode then resolves the file through the shared resolver. The file must be
a bounded ordinary file (`MAX_REQUEST_BYTES`, no symlink) that hashes to the
recorded BLAKE3 (or the admitted bytes' for a legacy launch) and that
`verify_frozen` accepts. Only then does the mode print one compact JSON
object. A refusal goes to stderr with exit status 2.

**Rationale:** this applies the recovery checks and the sealed request
binding without a plan, a credential or an owner grant. The caller gets
exactly what Lattice must be given, or nothing.

### Decision: the narrowest read-only ledger path

**Choice:** `Implementer::open_read_only` first reads the existing file's
100-byte SQLite header and refuses a ledger whose file format write or read
version (offsets 18 and 19) is 2, meaning WAL mode. It then opens the file
with `SQLITE_OPEN_READ_ONLY` (never `CREATE`) and sets `query_only`. It skips
the schema statement. `Implementer::dispatch_record` reads the attempt, its
transport intent, its launch and its sent request in one deferred
transaction that saves nothing. It answers as `dispatch_requires_recovery`,
`dispatch_launch` and `dispatch_request` do.

**Rationale:** `open` would create a missing ledger and its table, and the
existing attempt readers save the ledgers back. A read-only connection makes
a write impossible, not merely absent: every saving method fails inside SQLite
before any change. One snapshot means the intent, launch and request
cannot come from different ledger states. That matters because an intent
cleared between separate reads would otherwise look like a legacy launch.
A read-only connection to a WAL database in a writable directory may still
create its `-wal` and `-shm` files
(<https://www.sqlite.org/wal.html#read_only_databases>), and `query_only`
does not prevent that. Kiln creates its ledgers in rollback-journal mode,
so refusing WAL keeps "writes nothing" unconditional without refusing any
ledger Kiln writes.

## Risks / Trade-offs

- A ledger with a hot rollback journal cannot be read read-only. The report
  then refuses instead of recovering the journal, and the dispatcher's next
  read-write open recovers it.
- A ledger an operator switched to WAL mode is refused with a reason naming
  WAL. A switch to WAL between the header read and SQLite's open is not
  excluded; Kiln itself never switches a ledger to WAL.
- The report checks Kiln's record and the file. It does not check that
  Lattice's retained state still exists in the reported root.
