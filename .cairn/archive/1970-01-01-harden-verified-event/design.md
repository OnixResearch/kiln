## Context

- `event.rs` defines `VerifiedEvent` with ten `pub` fields, deriving `Clone,
  Debug, Eq, PartialEq, Serialize, Deserialize` and `deny_unknown_fields`.
  `admit_event_with` is the only place it is built, after HMAC verification and
  selection.
- Consumers read: `body_identity` and `delivery_id` (adapter replay, hosted
  connections), `candidate_identity`, `repository`, `revision` (adapter service),
  `selector`, `untrusted`, `pull_request_action` and `revision` (web automation),
  plus `repository_id` and `native_repository` in module tests.
- A workspace survey found exactly one serialization of the type: a module test
  asserting the untrusted marker is omitted. No journal, fixture or receipt
  embeds a `VerifiedEvent`; `plan_event` returns a `RunRecord`, and the replay
  store persists delivery ids and body identities as columns.
- Kiln already seals proofs elsewhere: `AdmittedScope` and `AdmittedRequest`
  keep private fields and expose getters.

## Goals and Success Contract

Outside `kiln_adapter_github::events`, a `VerifiedEvent` cannot be produced by a
struct literal or by deserializing JSON; the only way to obtain one is
`admit_event`/`admit_event_with`. Existing consumers keep working through
getters, and wire behaviour, replay rows and journal formats are unchanged.

False completion includes:
- a remaining public field or public constructor;
- a `Deserialize` impl that lets JSON stand in for authentication;
- a `Serialize` impl kept "for symmetry" with no consumer;
- consumer changes that alter selected fields, identities or order of checks.

## Decisions

### Decision: private fields plus getters, no derives

**Choice:** every field becomes private; getters return `&str`, `&ContentId`,
`u64`, `&NativeRepository`, `&EventSelector` and the two `Copy` option types.
`Serialize` and `Deserialize` are removed.

**Rationale:** the proof should be usable but not forgeable or exfiltratable as
data. Removing both derives also removes `deny_unknown_fields`, which existed
only for the deserializer. The one test that serialized the type is replaced by
its field-level assertion, which already exists.

### Decision: compile_fail doctests instead of a new test dependency

**Choice:** the struct documents two `compile_fail` examples — one deserializing
JSON into the type, one building it as a struct literal with every field name and
type correct, so that the only reason it fails is field privacy — plus a positive
example that type-checks a getter signature.

**Rationale:** the workspace has no `trybuild` dependency, and doctests run in
the existing `cargo test` rail. The literal example names all ten fields, so it
would compile if any field were public.

### Decision: no `cfg(test)` constructor

**Choice:** no test-only constructor is added.

**Rationale:** no existing or new test needs a `VerifiedEvent` that did not come
from `admit_event`; every module, service and replay test signs a real body. An
unused `#[cfg(test)]` constructor would be dead code added only to satisfy a
checklist item, and its absence makes the "not compiled outside tests" criterion
hold trivially. The doctests carry the sealing proof instead.

### Decision: audit, do not seal, the other `Admitted*` types

**Choice:** record one line per type; land sealing separately.

- `AdmittedGithubConfig` (`kiln-adapter-github/src/profile.rs`) — proof of
  reviewed GitHub profile admission (config, identity, pipeline identity, Lattice and
  transport profiles). Seal in a separate change.
- `AdmittedRadicleConfig` (`kiln-adapter-radicle/src/profile.rs`) — proof of
  reviewed Radicle shell profile admission. Seal in a separate change.
- `AdmittedAspenRuntimeProfile` (`kiln-adapter-aspen/src/profile/model.rs`) —
  proof of runtime profile admission with its identity. Seal in a separate change.
- `AdmittedJournal` (`kiln-dashboard/src/journal.rs`) — proof that a journal
  document was admitted; it carries the admitted state and records. Seal in a
  separate change.
- `AdmittedResult` (`kiln-core/src/semantic_merge/state.rs`) — plain record: it
  derives `Deserialize` because it is embedded in persisted semantic-merge state
  and is not the output of a check. Document as a record; do not seal.

**Rationale:** each of those types has its own consumer set and admission
boundary; changing them together would make one review cover five unrelated
proof surfaces and would touch persisted formats for `AdmittedResult`.

## Risks / Trade-offs

- Removing `Serialize`/`Deserialize` is a breaking API change for any external
  consumer; the workspace survey found none, and the crate is not published as a
  stable library.
- Getters expose references to `EventSelector` and `NativeRepository`; those wire
  types stay `pub` and deserializable because they are parsed before
  authentication and carry no proof.
- The doctests prove the seal for the current module path; renaming the type or
  moving it would require updating the examples, which the compiler enforces.
