## Context

`kiln_core::source` retains exact native revisions and computes source identity from path/mode/content manifests. `kiln_core::projection` (#75) admits repositories under a stable `kiln.repository-identity.v1`, admits typed definitions and resolves a frozen `ResolvedProjectionManifest` whose entries record a native or generated origin; visibility there is not access. The Aspen content adapter supplies byte identity only. #79 owns mapping projected edits back to sealed candidates and #80 owns durable manifest publication; #45/#46/#47/#63 keep attempts, verification and publication; #69/#70 keep authority.

## Goals and Success Contract

A reviewer can answer, for any graph-backed source: which logical entity a file is and how it got that identity; whether two equal-byte files are one entity, an alias, a copy or unrelated; which pinned snapshot a workspace or piece of evidence used; which native tree an export produced and from which root; and which single owner may publish the next state. A native-source repository needs none of it.

## Decisions

### Decision: separate logical, byte, revision and snapshot identities

**Choice:** five Kiln-owned BLAKE3 identities, each over a versioned, length-framed canonical encoding:

- `FileId` and `PackageId` are minted once, at the creating operation, over the repository identity, the identity of the operation that created the entity and a per-operation sequence number. They never hash bytes or paths, so a rename keeps them and an equal-byte file never shares them.
- `ContentId` is the existing BLAKE3 of the bytes. Equal bytes share storage, never ownership.
- An entity revision hashes its `FileId`, `ContentId`, mode and parent revision; it is immutable.
- A graph snapshot (root) hashes the schema version, every entity revision, every relation and every pinned dependency selection; mutable latest lookups cannot change a pinned root.
- Projection identities stay #75's manifest identities, now computed over a graph root and a layout definition.

Native provider references (`ExternalRef` commits, trees and blobs) appear only in import and export records.

**Rationale:** identity questions the issue raises (rename continuity, alias versus copy, deduplication without merged ownership) need identities that do not depend on the bytes or the current path.

### Decision: relations are versioned records, not directory structure

**Choice:** membership (`PackageId`, `FileId`, package-relative name), alias (an additional membership of the same `FileId`), copy (a new `FileId` with `copied_from` provenance), fork (a package-level copy that mints new identities with provenance) and dependency (a `PackageId` edge with an exact pinned selection) are records in the snapshot. No entity needs a global display parent.

Outcomes: an edit adds a revision of the same `FileId`; a rename replaces one membership name and keeps the `FileId`; removing a membership or an alias removes only that record, and the entity survives while another membership exists; deleting an entity is an explicit operation that writes a tombstone revision and removes every membership. These match #79's mapped-write outcomes.

### Decision: imports preserve ambiguity instead of inferring identity

**Choice:** an import record binds repository identity, native revision, tree, acquisition provenance, importer version and the resulting path-to-`FileId` mapping. Between two imported native revisions a path keeps its `FileId` only when the path and entry kind are unchanged or an explicit mapping says so. A rename, copy or delete-recreate observed only as native tree changes mints new identities and records an unresolved continuity candidate; equal or similar bytes never decide it. Symbolic links, submodules and special objects are refused until a later qualification admits them.

### Decision: deterministic exports with retained mappings

**Choice:** an export record binds the graph root, the layout (projection) identity, the exporter version and the native tree it produced, with the root-to-tree path mapping. Generated or non-invertible entries are declared by the layout, and an import of a tree containing them is refused unless the layout marks them derived. Exports preserve bytes, executable modes and deletions.

### Decision: one authority mode and one publication owner per repository

**Choice:** a repository is either native-authoritative (the native provider publishes; the graph is a derived import) or explicitly graph-authoritative (the graph store publishes roots; native refs are declared exports). A mode record names the owner. Every publication compares the expected current root or native revision and refuses a mismatch, so independent updates on both sides cannot converge by last writer; the losing side must re-import or re-export explicitly. Changing mode is an explicit migration record binding the last native revision to the first graph root (or the reverse), and rollback exports the final root. Several independently authoritative remote refs remain separate publication effects.

**Rationale:** atomic multi-project changes need one authoritative root followed by consumers.

### Decision: evidence binds exact roots and export subjects

**Choice:** candidates, reviews and gate results bind the exact graph root and, where a native export was built or reviewed, the export record. Any change to the graph root, schema, importer, exporter or layout identity makes that evidence inapplicable.

### Decision: authority stays outside the graph

**Choice:** graph queries, traversal, aggregates and imports or exports run under #69/#70's current authority and #75's disclosure contract. Relations, tags, aliases and imports cannot widen a delegated scope or confer publication or delegation authority.

## Risks / Trade-offs

- Minted identities need durable storage of the creating operation; losing it loses continuity, which the ledger then reports as ambiguous rather than guessing.
- Preserving ambiguity makes imports of busy native histories noisy until explicit mappings are supplied; that is the specified cost of not merging identities silently.
- A graph-authoritative repository needs a native export for every existing tool; exports that cannot be inverted restrict which edits can return.

## Verification Strategy

The design is checked by Cairn proposal, design and tasks gates. The follow-up tasks add a pure model with positive and negative fixtures for each relation outcome, import ambiguity, export round trip, authority-mode conflict, snapshot reproducibility and evidence invalidation, and one bounded prototype with one snapshot, two layouts and one shared entity. No existing repository is converted or published.
