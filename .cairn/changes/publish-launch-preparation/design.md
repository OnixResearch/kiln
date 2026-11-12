## Context

Before the push, `main` named `428d03fc` on the local bookmark, in local Radicle storage, on the Garden seed and on GitHub. The local head `bd5edde8` was eleven commits ahead with `428d03fc` as an ancestor. The repository's operation log shows the established publication pattern: push bookmark `main` to remote `rad`, then to remote `github`.

The projection's `main` carries a ruleset that blocks deletion and non-fast-forward pushes, and secret-scanning push protection is enabled.

## Decisions

### Decision: Push `main` only, canonical first

**Choice:** Run `jj git push --remote rad --bookmark main`, then `jj git push --remote github --bookmark main`.

**Rationale:** Radicle is canonical and GitHub is a projection. The operator's instruction was one word, so it is read as the repository's routine publication of `main` and nothing wider.

### Decision: No tag, release or workspace push

**Choice:** Leave the `v0.1.0` tag, the release, the announcement and the workspace repository's catalog commit as owner actions.

**Rationale:** The checklist gates the release on a go/no-go review with named approvers. A push of commits does not pass that review. The workspace repository has its own history and remote and was not named.

### Decision: Scan before the push

**Choice:** Search the added lines of the eleven commits for credential and private-detail patterns, with a synthetic positive control.

**Rationale:** Published history cannot be assumed removable. The scan is a bounded pattern search, not privacy clearance. The checklist keeps the release owner's redacting scan open.

### Decision: Exercise the published route, not only its refs

**Choice:** Clone both documented routes without credentials, compare their trees, and run the quick start from the GitHub clone with an empty home.

**Rationale:** A matching ref shows that the push arrived. It does not show that a new reader can acquire and build the source. The checklist kept this run open because nothing was published.

### Decision: Record after the fact

**Choice:** Record the publication in a local commit on top of the published head.

**Rationale:** A commit cannot name its own publication. That record commit is not published by this change.

### Decision: No-spec classification

**Choice:** Use `no-spec-delta` with change-local acceptance and an agentic self-review.

**Rationale:** Publication and launch documents do not amend accepted software behavior. The self-review is not independent approval.

## Risks / Trade-offs

- The published kit describes itself as unpublished until this record is published.
- The Nix gates ran on local revisions before publication and were not rerun on the published head. The commits after `0495510a` change only documents and lifecycle evidence.
- The quick start ran with three host accommodations a reader does not need: auto-GC disabled for the `nix` invocation, the Cargo target directory outside the clone, and debug information off to fit a memory-backed directory.
- The two runtime workspaces still name a source host that returned HTTP 530. The quick start does not build them.
- The pushed commits are unsigned, as earlier commits on `main` are. Radicle signs the reference update, not the commits.

## Verification Strategy

Before the push: read `main` on both public routes and in local storage, confirm the previous head is an ancestor of the local head, and scan the added lines. After each push: keep the command output. Then read both routes back anonymously, compare the projection's history with the previous head through the GitHub API, read the ruleset, the tag list, the release list and the secret-scanning alert list, clone both routes without credentials, and run the quick start. Check links in the changed documents, run traceability coverage, `cairn validate` and the planning gates.
