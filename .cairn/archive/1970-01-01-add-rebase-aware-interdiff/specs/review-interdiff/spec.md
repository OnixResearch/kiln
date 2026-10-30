## ADDED Requirements

### Requirement: Rebase-aware interdiff

r[kiln.review.interdiff]
The review provider MUST offer an interdiff between two revisions that removes base movement by rebasing the old change onto the new base without writing to the repository, then diffing against the new commit with exact new-side coordinates. When the provider cannot compute it exactly it MUST return an explicit unavailable reason, and the shell MUST fall back to the exact commit comparison with a visible notice. An approximation MUST NOT be presented as exact.

#### Scenario: Rebased change shows only the author's delta

r[kiln.review.interdiff.scenario.rebased]
- GIVEN a change whose second revision is rebased onto a moved target and edits one line
- WHEN the interdiff between the two revisions is requested
- THEN only the author's edited line MUST appear, and the plain commit comparison MUST still show the target movement

#### Scenario: Unavailable interdiff falls back explicitly

r[kiln.review.interdiff.scenario.unavailable]
- GIVEN revisions whose rebase conflicts, or a missing base commit
- WHEN the interdiff is requested
- THEN the provider MUST report unavailable with a reason, and the shell MUST return the exact commit comparison marked as a fallback

### Requirement: Addressable interdiff and stack narrowing

r[kiln.review.interdiff.route]
The console MUST offer "Changes since revision N" in the comparison selector. An interdiff MUST be addressable by `review_interdiff=<from>..<to>` revision ids and an optional `review_interdiff_change=<change-id>`, and a commit range inside one revision by `review_commits=<from>..<to>`. Opening such a URL MUST restore the same view, and back and forward navigation MUST restore earlier views.

#### Scenario: Narrowed interdiff

r[kiln.review.interdiff.scenario.narrowed]
- GIVEN two revisions whose commits carry change-id headers, one change edited between them
- WHEN the interdiff is narrowed to that change ID
- THEN only that change's delta MUST appear; and a stack whose commits lack change IDs MUST report unavailable with the exact fallback

### Requirement: Commit-range view

r[kiln.review.commit_range]
For a revision containing several commits, the service MUST list its commits in order with their change IDs and MUST compare any exact `from..to` range inside the revision. A range outside the revision MUST be rejected. Commenting MUST be offered only where coordinates are exact revision coordinates.

### Requirement: Hunk viewed markers

r[kiln.review.hunks]
Viewed markers MUST be private principal-owned `ReviewedHunk { actor, revision, file, hunk_digest }` preferences whose digest is BLAKE3 over the normalized hunk (path, removed and added lines, no line numbers). A file-level mark MUST write hunk markers for every hunk of the file. Each file MUST show how many of its hunks the principal viewed and which are new since the principal's last viewed revision. Markers MUST NOT count as approval or satisfy ownership.

#### Scenario: Carry-forward on upload

r[kiln.review.hunks.scenario.carry_forward]
- GIVEN a principal who viewed every hunk of a revision
- WHEN a new revision is uploaded where one hunk is unchanged, one is edited, one moved with an edit and one is new
- THEN only the unchanged hunk MUST remain viewed on the new revision

#### Scenario: Legacy file markers replay unchanged

r[kiln.review.hunks.scenario.replay]
- GIVEN a state document written before hunk markers, holding file markers
- WHEN it is loaded and serialized again
- THEN its bytes MUST be identical, and the file markers MUST mean viewed only on their original revision
