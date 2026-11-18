## Context

`vendor/heartwood-canonical` carries ten files extracted from heartwood commit `71a91042d96bb994c4cd4311d85f5cd1a6311b4e` plus hand-authored shims. `vendor/heartwood-canonical.source.json` records each derived file's upstream path and SHA-256; the importer reads pinned Git objects, verifies every digest and copies the verified files. The license map links two license texts in that directory that were never imported.

## Decisions

### Decision: Treat the license texts as upstream-derived files

**Choice:** Add `LICENSE-MIT` and `LICENSE-APACHE` to `upstream_paths` and `derived_files` in the source record, and extract both from the pinned commit in the importer. The existing verification and copy loops then cover them.

**Rationale:** One mechanism already verifies byte identity against the pin and refuses drift. A second notice mechanism would duplicate it.

### Decision: Take the texts from the repository root at the pinned commit

**Choice:** Use the root `LICENSE-MIT` and `LICENSE-APACHE` blobs of the pinned commit. The `radicle` crate manifest at that commit inherits its license from the workspace, and the crates.io archive ships no license file.

**Rationale:** The pinned commit is the recorded provenance of every derived file. The repository already takes notices from the exact upstream commit where a crate archive omits them, as for the Tree-sitter grammars.

### Decision: No-spec repair

**Choice:** Use `no-spec-delta` with change-local acceptance and an agentic self-review of the planning inputs.

**Rationale:** `kiln.radicle.canonical.provenance` already requires retained licenses, recorded digests and a drift-refusing importer. This change restores conformance; the self-review is not independent or legal approval.

## Risks / Trade-offs

- The root license texts name the upstream copyright holder for the whole repository; they are retained verbatim and not edited.
- The repair does not assess license compatibility of any combined artifact. `LICENSES.md` keeps that boundary.
- Published history before this change lacks the texts; the repair cannot change that.

## Verification Strategy

Fetch the pinned commit from the public heartwood mirror and confirm the commit object. Run the importer against it and confirm that only the two license files appear as new, with every other derived file unchanged. Copy the importer, record and vendored directory to an isolated root, tamper one license digest, and confirm the importer exits nonzero and writes nothing. Check the `LICENSES.md` links, run the vendored crate's tests, and run the planning gates and `cairn validate`.
