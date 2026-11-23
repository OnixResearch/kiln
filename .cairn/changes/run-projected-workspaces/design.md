## Context

`materialization::plan` yields an immutable input and a per-attempt writable overlay copy, pinned by a `WorkspacePin` that the dispatch plan carries. Lattice's implementation request v3 names `workspace_root`, `read_paths` and `write_paths`; its bubblewrap provider mounts a private tmpfs at `/workspace`, read-only binds each read path and read-write binds each write path from below the root, unshares the network and remounts the workspace and root read-only. Every path is relative to the root and every write path must exist.

## Goals and Success Contract

A projected attempt sees exactly its planned entries, writes only its writable origins and creation destinations, and cannot reach the host checkout, its Git database, the store backing its input, host sockets or any other path. Dispatch runs only the route derived for the pinned overlay. Writeback distinguishes every structural change and never overwrites a moved source silently. Verification receives facts bound to one sealed candidate and selects affected consumers outside the view.

## Decisions

### Decision: the route is overlay-relative

**Choice:** the route's workspace root is the overlay, read paths are the top-level components of the planned (and, for a write, creation) destinations, write paths are overlay destinations. The input root is recorded but not mounted.

**Rationale:** Lattice binds paths only from below one root. The overlay holds exactly the planned entries of the input, so reading through read-only binds of the overlay discloses nothing more than the input; the plan's `read_paths: [input_root]` is realized as the overlay's planned entries. The store, the input and the checkout are never below the root.

### Decision: write paths are per origin, collapsed only below a writable creation destination

**Choice:** a write path is each destination whose native origin lies within a writable source path, and each creation destination whose every mounted source path is writable; a path below another write path is dropped. Writable paths are the dispatch plan's permitted paths. A build witness admits none.

**Rationale:** binding a whole directory would let the worker create files outside the creation destinations; a file bind admits in-place edits only, so removing or renaming an origin in the runtime needs a writable creation destination. Both aliases of a writable origin are writable, which writeback requires.

### Decision: dispatch compares, it does not re-derive

**Choice:** `kiln-lattice-dispatch --projected-workspace-route` reads the route, verifies it, requires its pin to equal the admitted plan's pin, requires the request's root, read paths, write paths and network to equal the route, and requires the overlay to be a real directory at its route path. A pinned plan without a route, and a route for an unpinned plan, are refused. The check sits in `worker::run` directly after the existing request-crossing check, before any durable step.

**Rationale:** the dispatcher has no manifest; the pin is bound into the admitted plan identity, the route identity covers its paths, and derivation correctness is the pure core's tested property.

### Decision: structural writeback is declared

**Choice:** `capture` takes declarations (`Rename`, `Fork`, `AliasChange`, `MembershipChange`) and checks each against the overlay; view-definition operations are recorded beside source operations and never applied. Without a declaration the overlay keeps its v1 meaning.

**Rationale:** a directory diff cannot tell a rename from a delete plus an equal-byte create, and equal bytes at two origins must stay independent; only the worker knows its intent, and the overlay must still prove it.

### Decision: a moved source is a conflict, not a rebase

**Choice:** when the current source's revision or tree differs from the manifest's, a capture with no other fault returns `Captured::Conflict` naming, per canonical origin, where the operations and the current source disagree. Fence, pin and other refusals take precedence. Rebase is a re-resolution with a new overlay generation.

**Rationale:** current ownership does not establish freshness of observed content (the Archil reference); normalizing to origins keeps alternate display paths from bypassing detection.

### Decision: v1 records verify read-only

**Choice:** `RecordedCandidate` deserializes a retained candidate and `verify_recorded` recomputes its identity under its own schema's encoding; a v1 record may hold only v1 operations. A v1 fixture recorded by the 90a9fb64 capture is frozen in the tests.

### Decision: facts reuse the affected-project decision

**Choice:** `candidate_facts` runs `monorepo::affected` over the candidate's changed source paths with the full candidate's scopes and graph facts, binds the plan only when it was made for the candidate's manifest and base, and hashes every field.

### Decision: the runtime qualification is an off-by-default example

**Choice:** `examples/projected-runtime.rs` drives the real binaries with a probe program placed in the Nix store in place of OMP; the probe reports its observations on stdout, which Lattice retains as raw events. It is not part of `cargo test`; its receipt is the evidence.

**Rationale:** real bubblewrap through Lattice is the only meaningful confinement evidence; a skipped test is not evidence.

## Risks and Trade-offs

- File binds cannot be unlinked, so removing or renaming an origin inside the runtime requires a writable creation destination around it.
- The route does not re-check permitted paths against authority; S7 adds admission at retrieval and workspace effects.
- The probe replaces OMP; model and tool behavior inside the sandbox is Lattice's qualification.
