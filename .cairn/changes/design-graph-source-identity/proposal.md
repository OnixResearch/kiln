## Why

Issue [#81](https://github.com/OnixResearch/kiln/issues/81) asks for the optional source model in which project and package relationships and stable logical files are authoritative independently of one global directory tree, with conventional trees as reproducible projections for tools and VCS interchange. Today `SourceResolution` and the projection contract (#75) identify source by native revision and path/mode/content manifests; nothing gives a file a logical identity that survives a rename, distinguishes an alias from a copy, or says which side owns publication when a graph and a native repository both exist. This is a design and contract milestone: initial native-source views and workspaces must not depend on it.

## What Changes

- Specify the identities: stable logical `FileId` and `PackageId`, byte `ContentId`, immutable entity revisions, graph snapshots (roots), projection identities and native provider references, and how each is derived without inferring logical identity from bytes.
- Specify versioned relations for membership, package-relative names, aliases, copies, forks and dependency selections, and the distinct outcomes of edit, rename, copy, alias, membership removal, alias removal and entity deletion.
- Specify the exact native import ledger and the deterministic tree export, with ambiguity preserved for external rename, copy and delete-recreate, and unsupported objects refused.
- Specify one authority mode per repository (native-authoritative with derived imports, or explicitly enabled graph-authoritative with declared exports), its single publication owner, transitions and conflict refusal.
- Specify evidence binding to the exact graph root and export subject, and invalidation when a graph, schema, importer, exporter or layout identity changes.
- Leave capability-restricted graph surfaces to #69/#70 and the restricted disclosure contract of #75.

## Capabilities

### New Capabilities

- `graph-source`: the optional graph-backed source identity, relation, import/export and authority-mode contract.

## Impact

Design and specification only. The tasks name the fixtures and the bounded prototype that will follow; no code, store, provider adapter or repository conversion is part of this change.

## Non-goals

Converting, mirroring or publishing any existing repository; replacing a VCS; language-level semantic identity for arbitrary source text; making initial native-source views or workspaces depend on this mode.
