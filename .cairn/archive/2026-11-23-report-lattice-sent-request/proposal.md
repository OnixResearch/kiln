## Why

Kiln #49 live pilot run 6 produced a real candidate, but candidate export
failed. The onix-core composition ran Lattice `implementation-attempt
recover` with the admitted `attempt.json`, while Kiln's worker route had sent
the admitted bytes plus one appended `lineage` member. Lattice binds its
persisted intent to the exact bytes it ran, so it correctly refused.

Kiln records the sent request for the launch (`transport_requests`), and
`worker::sent_request_path` states that `recover`, `cancel`, `inspect` and
`export-candidate` must name that file. Kiln's own campaign export follows the
rule, but the record is reachable only through the Rust API. Callers in
another process, such as the composition, cannot follow it.

## What Changes

- `kiln-lattice-dispatch --database DB --enrollment FILE --worker-request
  ADMITTED --worker-transport-profile FILE --sent-request true` prints one
  `kiln.lattice-sent-request.v1` object with exactly `schema`, `attempt`,
  `fence`, `recorded`, `request`, `request_blake3` and `state_directory`.
  `request` and `state_directory` are exactly as recorded, never
  canonicalized. A launch recorded by an earlier binary, which sent the
  admitted bytes, reports the admitted request frozen in the profile's output
  directory with `recorded: false`.
- Before printing, the mode checks the attempt and fence named by the
  admitted request, the request seal from bind-dispatch, the transport intent
  and the recorded launch. It also checks the file: a bounded ordinary file
  hashing to `request_blake3` that `verify_frozen` accepts. Any refusal exits
  non-zero with nothing on stdout.
- The mode writes nothing. It opens the ledger with a new read-only path
  (`Implementer::open_read_only`) and reads it in one snapshot
  (`Implementer::dispatch_record`). It creates no file, grants no owner,
  reads no credential and starts no Lattice process.
- The campaign export's resolver moves into `kiln_adapter_lattice::worker` as
  `sent_request_and_state`, so the export and the new mode share one rule.

## Impact

- **Files**: `crates/kiln-adapter-lattice/src/{worker.rs,worker_preparation_tests.rs,worker_sent_request_tests.rs,bin/lattice-dispatch.rs}`,
  `crates/kiln-adapter-lattice/tests/sent_request_cli.rs`,
  `crates/kiln-implementer/{src/lib.rs,tests/durable.rs}`,
  `crates/kiln-web-service/src/implementation_campaign.rs`,
  `docs/fair-admission.md`, `docs/implementation-runbook.md`, `CHANGELOG.md`,
  `.cairn/specs/fair-admission/spec.md` (through this change's delta), and this change.
- **Owner**: Kiln implementer and Lattice adapter maintainers. Consumer: the
  onix-core #49 composition, which is coded against this contract.
- **Testing**: focused `kiln-adapter-lattice`, `kiln-implementer` and
  `kiln-web-service` tests, workspace clippy `-D warnings`, fmt, `cairn
  validate` and the change gates, and `nix flake check`.
- **Non-claims**: no live Lattice recover or export runs against a reported
  file here. The report says which file and root the recorded launch ran. It
  does not show that Lattice's retained state still exists.
