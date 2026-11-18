## Context

`kiln_core::projection::resolve` maps every destination of a view to a native origin (source path, object and origin identity) or a generated one. `require_complete(Write)` admits only writable views without generated entries, and `AdmittedPartialView::creation_destinations` lists the destination directories where a writable view may add files. `projection::materialization` prepares an immutable input and per-attempt overlays bound by a `WorkspacePin` (repository, exact source, definition, manifest, profile, input, overlay, attempt and fence). `worker::materialize` writes the overlay as a private writable copy of the input.

## Goals and Success Contract

Every byte that reaches the candidate comes from an overlay file that capture observed and sealing re-read, and every candidate path outside the operations keeps its base object and mode. A moved source, a superseded attempt, a pin for another view and every ambiguous or contradictory mapping are refused before anything is written. A sealed candidate is immutable: later overlay writes produce another capture with another identity and cannot alter a sealed commit.

## Decisions

### Decision: capture maps overlay files through the manifest, not through paths

**Choice:** `capture(definition, complete, pin, observed, current)`:

1. The witness must be for a write, the definition must be the manifest's, and the pin must name the manifest's repository, exact source, definition and manifest and pass `WorkspacePin::validate`. The current fence must equal the pin's fence. The current source facts must be complete and name the manifest's repository, exact revision and canonical tree, or the capture is refused as `source_moved`.
2. Observed files must be unique, canonical regular files or executables within the bound; a symbolic link is refused.
3. Native manifest entries are grouped by origin path. All destinations unchanged produce nothing; all changed to the same object and kind produce one `edit` with the base object as `expected`; all absent produce one `delete`. Any other mix is a `contradictory_alias`.
4. An observed destination that is not in the manifest is a new file. It must lie under a creation destination; every mount whose destination prefix holds it yields a source path, and exactly one must result. That path must be selected by the definition and absent from the source, so a create never overwrites a file the view does not hold. It becomes a `create`.

Operations are sorted by source path; `edit` and `create` also name the overlay destination their bytes come from.

**Rationale:** the manifest is the frozen origin map, so an alias edit, an equal-byte file at another origin and an omitted file are all decided by origin identity, never by the directory layout. Refusing a moved source instead of rebasing keeps a retry from silently overwriting an intervening update.

### Decision: one sealed identity and a pure reconstruction

**Choice:** `SealedCandidate` (`kiln.projection.writeback-candidate.v1`) is built only by capture and serialized for evidence. Its identity is BLAKE3 over the schema, repository, base revision and canonical tree, definition and manifest identities, the pin's overlay, attempt and fence, and every operation. `SealedCandidate::reconstruct(base)` checks that `base` is the candidate's base and returns the base entries with the operations applied. It refuses an edit or delete whose path is absent or holds another object than `expected`, and a create over an existing path.

**Rationale:** verification and later publication bind one identity, and the reconstruction is the reference the shell's tree must equal.

### Decision: the shell re-reads before it writes and checks the tree it built

**Choice:** `worker::writeback::observe` walks an overlay without following links and hashes each regular file with `Oid::hash_object`. Its kind is executable when any execute bit is set. `worker::writeback::seal` re-reads the overlay file of each `edit` and `create`, refuses bytes whose Git blob identity differs from the operation, writes those blobs, and applies the operations to the base tree with `TreeUpdateBuilder`. It checks that a tree diff without rename detection names exactly the operation paths and that the candidate's listing equals `reconstruct`. It then commits with a fixed signature, the base as parent and the candidate identity in the message, and moves no reference.

**Rationale:** a worker can keep writing after capture; re-reading and comparing makes the sealed commit exactly the captured bytes. Diffing the built tree is the independent check that nothing outside the operations changed.

## Risks / Trade-offs

- A rename appears as a delete and a create, so it loses rename identity until a source graph (#81) or an explicit rename operation exists.
- Any source movement refuses the capture; an explicit rebase is later work.
- Capture and plans name hidden paths; they are controller facts, and restricted results stay with #69/#70.

## Verification Strategy

Pure tests cover edit, create and delete operations, each refusal, the candidate identity and reconstruction. Shell tests over a temporary Git repository round-trip a shared edit through an aliased writable view: the candidate changes exactly that path, keeps every other object and mode, appears in a view resolved at the candidate and leaves another attempt's pinned overlay unchanged. They also cover equal-byte files at two origins, a refused overlay that changed after capture and a later capture with another identity. Then Clippy, formatting, Cairn gates, validation and traceability.
