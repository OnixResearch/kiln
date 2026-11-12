## Context

`prepare-go-to-market` observed the public channels read-only and named owner actions. Its acceptance states that it performs no remote setting change. The operator has now named specific actions, so they need their own record.

Observed before any change on 2026-11-12: private vulnerability reporting disabled; secret scanning and push protection disabled; `OnixResearch/kiln-public` public, not archived, last pushed 2026-10-18, one parentless commit `35d10f2e`, no stars, forks or watchers. `OnixResearch/kiln` has been public since the `f748acd0` cutover and carries current `main`.

## Decisions

### Decision: Enable exactly the two named settings

**Choice:** Enable private vulnerability reporting and secret scanning. Leave push protection, Dependabot updates, branch protection, description and topics unchanged.

**Rationale:** The operator named the disabled reporting route and secret scanning. Push protection rejects a push that contains a detected secret pattern and can block the maintainer's projection push if a fixture is secret-shaped; that trade-off was not put to the operator. Description and topics are content the operator chooses.

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
- The workspace catalog outside this repository names `kiln-public` as Kiln's GitHub source, so the workspace wiki links a repository that is now private. Correcting it needs the workspace's documentation generator, which could not run because the Nix store on the preparation host was not accepting writes. The checklist keeps it as an open row.

## Verification Strategy

Read back each setting through the GitHub API before and after. Request both repositories anonymously over HTTPS. List secret-scanning alerts. Rerun the quick-start, demo, fixture-forge, workspace test, Clippy and format commands, traceability coverage, `cairn validate` and release readiness on the current tree. Check relative links and anchors in the changed documents. Run the planning gates.
