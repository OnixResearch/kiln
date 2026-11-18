## Context

`prepare-go-to-market` observed the public channels read-only and named owner actions. Its acceptance states that it performs no remote setting change. The operator has now named specific actions, so they need their own record.

Observed before any change on 2026-11-12: private vulnerability reporting disabled; secret scanning and push protection disabled; `OnixResearch/kiln-public` public, not archived, last pushed 2026-10-18, one parentless commit `35d10f2e`, no stars, forks or watchers. `OnixResearch/kiln` has been public since the `f748acd0` cutover and carries current `main`.

## Decisions

### Decision: Change only the settings the operator named

**Choice:** First enable private vulnerability reporting and secret scanning. After the operator's second authorization, set the description to the Radicle project description, set seven topics, and enable push protection. After the third authorization, create one ruleset on the projection's `main` that blocks deletion and non-fast-forward pushes, and append one row for Kiln to the organization profile. Leave Dependabot updates and the homepage unchanged.

**Rationale:** The operator's first instruction named the disabled reporting route and secret scanning. Description, topics and push protection were put to the operator as explicit choices with their exact values, and both were ticked. Push protection rejects a push that contains a detected secret pattern; secret scanning had listed no alert for four hours, so existing content is unlikely to trip it. The branch rule and the organization-profile row were put to the operator at close-out with their trade-offs: a projection rollback needs the ruleset disabled first, and the profile link reaches the previous README until the launch preparation is published. Both were ticked. The ruleset requires no review, status check or signature, so the maintainer's fast-forward projection push still passes.

### Decision: Make the stale repository private, do not delete it

**Choice:** Change `OnixResearch/kiln-public` to private.

**Rationale:** The goal is one public GitHub entry point. A visibility change achieves it and is reversible. Deletion is irreversible, would remove the object the 2026-10-17 publication receipts describe, and needs a `delete_repo` token scope the session does not hold. Nothing in this workspace depends on the repository: sibling manifests pin `OnixResearch/kiln` or the Radicle projection.

### Decision: Keep the historical record, add a dated withdrawal

**Choice:** Leave the 2026-10-17 projection section and its receipts unchanged and add a dated withdrawal subsection after it.

**Rationale:** The earlier section describes what was observed then. Rewriting it would falsify a retained record.

### Decision: Replace kit figures with results on the current tree

**Choice:** Rerun the documented commands after the rename and the help fix and replace the test and coverage figures in the checklist, release-notes draft and positioning.

**Rationale:** The help fix adds two tests and three requirement identifiers. A figure quoted from the earlier tree would no longer match the tree it is quoted for.

### Decision: No-spec classification

**Choice:** Use `no-spec-delta` with change-local acceptance and an agentic self-review.

**Rationale:** Repository settings and launch documents do not amend accepted software behavior. The self-review is not independent approval.

## Risks / Trade-offs

- Secret scanning's first pass over existing history may not have finished when the alert list was read. The checklist tells the operator to read it again.
- The private reporting form cannot be exercised as an outside reporter from the maintainer's session. It stays an open checklist row.
- Anyone who cloned `kiln-public` keeps that copy.
- The workspace catalog outside this repository named `kiln-public` as Kiln's GitHub source, so the workspace wiki linked a repository that is now private. It is corrected in the workspace repository, which has its own history. The workspace's Nix generator could not run on the preparation host, so its renderer was built with Cargo from the same source and the guide was checked with an existing asciidoctor build of the pinned version. The rendered site under `public/` there was not refreshed.
- The Nix gates are run at a pinned local revision with auto-GC disabled for those invocations only, because the host's garbage collector held its lock for hours. A watchdog stops a build before free space on the store falls below a floor.
- The ruleset was read back, not provoked: no deletion or non-fast-forward push was attempted against it. A recovery that must rewind the projection needs it disabled or deleted first.
- The organization profile links the projection before the launch preparation is published, so a visitor reaches the previous README until then. The profile commit was made through the contents API and is unsigned.

## Verification Strategy

Read back each setting through the GitHub API before and after. Read back the active rules for `main`, and the profile file and its commit. Request both repositories and the organization page anonymously over HTTPS. List secret-scanning alerts. Rerun the quick-start, demo, fixture-forge, workspace test, Clippy and format commands, traceability coverage, `cairn validate` and release readiness on the current tree. Check relative links and anchors in the changed documents. Run the planning gates.
