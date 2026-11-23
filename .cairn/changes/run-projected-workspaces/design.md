## Context

`materialization::plan` yields an immutable input and a per-attempt writable overlay copy, pinned by a `WorkspacePin` that the dispatch plan carries. Lattice's implementation request v3 names `workspace_root`, `read_paths` and `write_paths`; its bubblewrap provider mounts a private tmpfs at `/workspace`, read-only binds each read path and read-write binds each write path from below the root, unshares the network and remounts the workspace and root read-only. Every path is relative to the root and every write path must exist.

## Goals and Success Contract

A projected attempt sees exactly its planned entries, writes only its writable origins and creation destinations, and cannot reach the host checkout, its Git database, the store backing its input, host sockets or any other path. Dispatch runs only the route derived for the pinned overlay. Writeback distinguishes every structural change and never overwrites a moved source silently. Verification receives facts bound to one sealed candidate and selects affected consumers outside the view.

## Decisions

### Decision: the route is overlay-relative

**Choice:** the route's workspace root is the overlay, read paths are the top-level components of the planned (and, for a write, creation) destinations, write paths are overlay destinations. The input root is recorded but not mounted.

**Rationale:** Lattice binds paths only from below one root. The overlay holds exactly the planned entries of the input, so reading through read-only binds of the overlay discloses nothing more than the input; the plan's `read_paths: [input_root]` is realized as the overlay's planned entries. The store, the input and the checkout are never below the root.

### Decision: write paths are per origin, collapsed only below a writable creation destination

**Choice:** a write path is each destination whose native origin lies within a writable source path, and each creation destination whose every mounted source path, every native origin planned at or below it and the source prefix of every mount placed below it are writable; a path below another write path is dropped. Writable paths are the dispatch plan's permitted paths. A build witness admits none.

**Rationale:** binding a whole directory would let the worker create files outside the creation destinations; a file bind admits in-place edits only, so removing or renaming an origin in the runtime needs a writable creation destination. Both aliases of a writable origin are writable, which writeback requires. A directory bind exposes everything below it, so an alias of an unwritable origin nested inside a creation destination (mounts `"" → ""` and `private → src/private` with creation destination `src`) refuses the directory grant rather than making `src/private` writable.

### Decision: dispatch derives the route again for the plan

**Choice:** the route carries the authored definition and the resolved manifest it was derived from; `verify` admits both against the pin's definition and manifest identities. `kiln-lattice-dispatch --projected-workspace-route` reads the route, requires its pin to equal the admitted plan's pin, derives its read and write paths again from that definition and manifest for the plan's own permitted paths and requires exact equality (`verify_permitted`), requires the request's root, read paths, write paths and network to equal the route, and requires the overlay to be a real directory at its route path. A pinned plan without a route, and a route for an unpinned plan, are refused. `bind-execution` applies the same derivation check. The check sits in `worker::run` directly after the existing request-crossing check, before any durable step.

**Rationale:** the pin binds the route to a view, not to a permission set: a route derived for the same pin with wider writable paths is well formed. Exact equality, not a subset rule, keeps the derivation the single source of the grant; the plan's permitted paths are what Kiln admitted.

### Decision: the store lies outside every runtime-mounted root

**Choice:** a route's store may not lie at or below `/nix/store`, `/usr`, `/bin`, `/lib`, `/lib64` or `/run/current-system/sw/bin`, the roots Lattice's bubblewrap provider read-only binds into every sandbox (`SYSTEM_BASES` and the `/usr/bin` bind in Lattice `src/candidate_worker/sandbox.rs:1,168-178`, identical at the cohort pin `661499d2` and at `7fb6dc47`). Every other host path the sandbox sees is a fresh `/proc`, `/dev`, `/tmp`, `/workspace` or a route path, so a private state root such as `/var/lib/kiln` is not visible.

**Rationale:** a store under `/usr` would let the worker read every input and overlay backing path outside its route.

### Decision: no VCS metadata in a route

**Choice:** a planned or creation destination with a `.git` or `.jj` component refuses derivation, and a route path with one fails `verify`, so the dispatcher refuses it before reserving the plan.

**Rationale:** Lattice refuses those components only at run time (`src/candidate_capture.rs:116`, `VcsMetadataRejected`), after Kiln has reserved the plan and sealed the request.

### Decision: projection needs a profile that enables it

**Choice:** `check_route` takes the selected `ProjectionAuthority` and refuses every projected dispatch (a pinned plan or a supplied route) with `projection-requires-operator` unless it is `ProjectionEnabled`. `kiln-lattice-dispatch` on this branch selects at most the local-operator profile (`--local-operator`), which never enables projection, so a projected dispatch cannot run until the verified profile of `compose-verified-implementation-authority` (S1) is integrated and maps a `verified_ucan` configuration with `projection_enabled` to `ProjectionEnabled`.

**Rationale:** the refusal is keyed on the dispatch, not on the `projection_enabled` flag a `LocalOperatorAuthority` was constructed with; C1 gives `projection_enabled` only to `verified_ucan`.

### Decision: structural writeback is declared

**Choice:** `capture` takes declarations (`Rename`, `Fork`, `AliasChange`, `MembershipChange`) and checks each against the overlay; view-definition operations are recorded beside source operations and never applied. Without a declaration the overlay keeps its v1 meaning.

**Rationale:** a directory diff cannot tell a rename from a delete plus an equal-byte create, and equal bytes at two origins must stay independent; only the worker knows its intent, and the overlay must still prove it.

### Decision: a moved source is a conflict, not a rebase

**Choice:** when the current source's revision or tree differs from the manifest's, a capture with no other fault returns `Captured::Conflict` naming, per canonical origin, where the operations and the current source disagree: an origin whose current object or entry kind differs from the frozen one (a mode-only change included), or a new path the source now holds. Fence, pin and other refusals take precedence. Rebase is a re-resolution with a new overlay generation.

**Rationale:** current ownership does not establish freshness of observed content (the Archil reference); normalizing to origins keeps alternate display paths from bypassing detection.

### Decision: v1 records verify read-only

**Choice:** `RecordedCandidate` deserializes a retained candidate and `verify_recorded` recomputes its identity under its own schema's encoding; a v1 record may hold only v1 operations. A v1 fixture recorded by the 90a9fb64 capture is frozen in the tests.

### Decision: facts reuse the affected-project decision

**Choice:** `candidate_facts` runs `monorepo::affected` over the candidate's changed source paths with the full candidate's scopes and graph facts, binds the plan only when it was made for the candidate's manifest and base, and hashes every field.

### Decision: the runtime qualification is an off-by-default example

**Choice:** `examples/projected-runtime.rs` drives the real binaries with a probe program placed in the Nix store in place of OMP; the probe reports its observations on stdout, which Lattice retains as raw events. It requires `--authority-config` selecting `verified_ucan` with `projection_enabled`, and records the same request under `--local-operator` as a refusal. A case qualifies only when the observed probe outcomes equal the complete expected set sent in the prompt, counting repeats, each refusal is observed and names its reason, and the beta Cargo build's failure is observed. It is not part of `cargo test`; its receipt is the evidence.

**Rationale:** real bubblewrap through Lattice is the only meaningful confinement evidence; a skipped test is not evidence, and neither is a probe run that stopped early.

## Risks and Trade-offs

- File binds cannot be unlinked, so removing or renaming an origin inside the runtime requires a writable creation destination around it.
- The route does not re-check permitted paths against current authority; S7 adds admission at retrieval and workspace effects.
- A route carries its manifest, so a route file may be as large as the manifest bound (64 MiB read bound at the dispatcher).
- The restricted-runtime receipt recorded before these rules (local-operator profile, partial probe acceptance) is superseded; the runtime is unqualified until requalified on S1's verified profile.
- The probe replaces OMP; model and tool behavior inside the sandbox is Lattice's qualification.
