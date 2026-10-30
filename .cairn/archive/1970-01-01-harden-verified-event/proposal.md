## Why

`VerifiedEvent` is the proof that a webhook delivery passed HMAC authentication
and event selection, and `plan_event` and the replay `reserve` treat it as such.
Nothing enforces that: every field is `pub`, and the type derives `Deserialize`,
so any crate that depends on `kiln-adapter-github` can build one with a struct
literal or from JSON and pass it to code that assumes authentication happened.
Issue #22 asks for the same treatment `AdmittedScope` and `AdmittedRequest`
already have.

## What Changes

- Make every `VerifiedEvent` field private and expose read-only getters for the
  fields consumers use.
- Remove the `Serialize` and `Deserialize` derives: a survey of the workspace
  found no production path that serializes a `VerifiedEvent`, and a proof type
  should not leave the process as data.
- Keep `admit_event` and `admit_event_with` as the only constructors.
- Prove the seal with `compile_fail` doctests (struct literal and JSON
  deserialization) plus a positive getter signature check.
- Update `kiln-hosted`, `kiln-web-service` and the adapter's own modules to read
  through getters; replace the one module test that serialized the type.
- Record the `Admitted*` proof-versus-record audit requested by the issue. Sealing
  those types is deliberately left to separate changes.

## Impact

- **Files**: `crates/kiln-adapter-github/src/events.rs` (private fields, getters,
  doctests), `crates/kiln-adapter-github/src/events/tests.rs`,
  `crates/kiln-adapter-github/src/service.rs`, `crates/kiln-adapter-github/src/replay.rs`,
  `crates/kiln-adapter-github/src/tests.rs`,
  `crates/kiln-hosted/src/connections.rs`,
  `crates/kiln-web-service/src/engine/automation.rs`.
- **Testing**: `cargo test -p kiln-adapter-github` (including doc tests),
  `cargo test -p kiln-hosted -p kiln-web-service`, workspace clippy and fmt, and
  the `tests`/`clippy`/`format` flake checks. Wire behaviour is unchanged: no
  signature, replay, body-limit or selection logic changes.
