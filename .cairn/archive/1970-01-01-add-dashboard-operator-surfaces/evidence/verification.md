# Operator surfaces verification

Date: 2026-10-22
Scope: the dashboard's second input, the operator CLI command, the operator CLI
property rail, and a shell-written journal fixture.
Base revision: `141ec4d2` plus the uncommitted change working tree.

## What was added

- `kiln-adapter-sqlite`: `open_read_only`, which opens a store without
  writing to it, admits the stored schema version instead of initializing one,
  and refuses a missing store; `load_stream_records`, which reads one stream's
  retained records; and `canonical_profile`, which mirrors
  `config/sqlite-profile.ncl` so a reader admits the profile the store was
  created with.
- `kiln-dashboard`: a store input that loads a named stream, verifies its
  event identities with the store's own admission, and replays it canonically;
  a shared shell module; a `sample_store` example.
- `kiln-cli`: the `kiln dashboard` subcommand over the same shell.
- `checks/bombadil`: three operator CLI specifications, two evidence
  fixtures, a journal written by the OnixOS pilot's own writer, and a fourth
  dashboard case over that journal.
- The conformance boundary audit now names the dashboard's shell entries and
  forbids the SQLite runtime reach in the pure core.

## Results

- `cargo test -p kiln-dashboard`: 23 passed, including the store admission
  test, the read-only store test, and the tampered-journal rejection.
- `cargo test --workspace`: 99 test binaries passed, no failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass.
- `cargo fmt -- --check`: pass.
- `nix build path:$PWD#checks.x86_64-linux.bombadil -L`: pass. The rail drives
  the packaged `kiln-dashboard` and `kiln` binaries under a pseudo terminal
  over four journal cases and three CLI cases, and keeps the captured traces.
- Every property can fail. The negative controls reported a violation for the
  wrong expected revision, an understated run bound, a success expectation on
  the rejected journal, a wrong demo run identity, a shortened evidence
  identity, and a success expectation on the rejected evidence input.
- `cairn gate proposal|design|tasks add-dashboard-operator-surfaces --root .`:
  PASS.
- `cairn validate --root .`: `valid: true`.
- `cairn traceability coverage --root .`: pass, 855 of 855 requirements
  referenced.
- `cairn release-readiness --root .`: `valid: true`, verdict pass.

## The pilot journal fixture

`checks/bombadil/journal-pilot.json` was written by the OnixOS pilot's own
writer through `cargo run -p kiln-onixos-pilot --example write_sample_journal`.
The pilot stores its durable `StartRun` intent before it reaches the Aspen
host, so the example produces a real pilot-written journal without a host. The
fixture carries the pilot's own schema, its own `pilot-event-blake3` event
identity convention, and its own recorded identity. Its request carries the
timestamp of the moment it was written, so the fixture is a recorded artifact
rather than a byte-reproducible sequence; the dashboard admits it by
recomputing the event identity, not by trusting a recorded one.

## The store input

The store is opened with a read-only flag. The test proves that observing a
store does not change the file and that a missing store is refused instead of
created. The dashboard reads the store through the adapter's own API, so the
schema stays owned by the adapter.

The Celld worker event store is not a dashboard input. It is a remote
authenticated HTTP service, and reading it would give the dashboard a network
path that its read-only local boundary forbids.

## Limits and non-claims

- The property rail covers four journal cases and three CLI commands. It does
  not claim to have explored every journal, store, profile, argument, or
  terminal input.
- The canonical store profile mirrors the Nickel contract by hand. It is not a
  byte comparison against the exported Nickel document, and the two shapes
  differ.
- The pilot fixture is a recorded artifact, not a production journal, and the
  dashboard does not claim it observes a live shell.
- The operator surfaces check the rendered report and the operator CLI only.
  They do not prove CI success, provider correctness, source trust, journal or
  store completeness, release readiness, or that the observed revision is the
  newest one.
