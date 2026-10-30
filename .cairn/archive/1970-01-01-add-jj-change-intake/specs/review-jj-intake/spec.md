## ADDED Requirements

### Requirement: Exact jj change-id intake

r[kiln.review.jj_intake]
When the operator enables `intake.jujutsu`, an authenticated write principal MUST be able to submit one exact jj change ID through the console or `kiln-web-service ci review-submit --jj-change <id>`. Kiln MUST map one change ID to exactly one Kiln change with a deterministic BLAKE3 identity bound to the repository, and MUST append each newly observed commit for that ID as the next revision of that change. A commit that is already a revision of the change MUST append nothing. Only full change IDs SHALL be accepted; revsets, prefixes and the root change MUST be refused. A change ID MUST NOT grant target-write, approval, CI or submission authority, and branch intake MUST remain available unchanged.

#### Scenario: One change rewritten three times

r[kiln.review.jj_intake.scenario.rewritten]
- GIVEN a real jj repository where one change is created, submitted, rewritten, submitted, rewritten and submitted again
- WHEN Kiln admits each submission
- THEN exactly one Kiln change MUST exist with three revisions in order, and the recorded jj predecessors of revisions 2 and 3 MUST contain the commit of the previous revision

#### Scenario: Duplicate submissions

r[kiln.review.jj_intake.scenario.replay]
- GIVEN a change ID whose current commit is already the change's current revision
- WHEN the same change ID is submitted again, with a new or the same request id
- THEN no revision, provider operation or dependency change MUST be appended

### Requirement: Fail-closed jj admission

r[kiln.review.jj_intake.admission]
Every jj observation MUST go through `kiln_core::source::JujutsuFacts` admission before any provider effect. A divergent change, a conflicted bookmark on the resolved commit, a conflicted commit, a missing or divergent operation head, a non-Git backend, a jj Git store other than the review repository, a commit absent from the review repository or whose `change-id` header disagrees, a missing change, and stack members out of ancestor order MUST each be refused with a stable `review_jj_*` diagnostic. A refusal of any stack member MUST refuse the whole request before any provider effect, journal reservation or state change.

#### Scenario: Divergent change refused

r[kiln.review.jj_intake.admission.scenario.divergent]
- GIVEN a real jj repository where one change ID resolves to two visible commits
- WHEN the change ID is submitted
- THEN the request MUST fail with `review_jj_divergent_change` and the review state and provider operations MUST be unchanged

#### Scenario: Conflicted inputs refused

r[kiln.review.jj_intake.admission.scenario.conflicted]
- GIVEN a real jj repository where the resolved commit carries a conflicted bookmark, or is itself a conflicted commit
- WHEN the change ID is submitted
- THEN the request MUST fail with `review_jj_conflicted_bookmark` or `review_jj_conflicted_commit` respectively and nothing MUST be appended

### Requirement: Kiln stays read-only toward jj

r[kiln.review.jj_intake.read_only]
Kiln MUST read the jj operation head from the repository's operation-head store without invoking jj, and MUST invoke only the configured absolute jj executable with `--ignore-working-copy` and `--at-op` pinned to that head, a cleared environment and named time and output bounds. Kiln MUST NOT rebase, squash, describe, push, snapshot, merge operations or move bookmarks.

#### Scenario: Repository unchanged by observation

r[kiln.review.jj_intake.read_only.scenario.unchanged]
- GIVEN a real jj repository
- WHEN Kiln observes changes, including a refused divergent change
- THEN the operation heads, operation store, Git refs and working-copy files MUST be byte-identical before and after

### Requirement: jj observations are provider facts

r[kiln.review.jj_intake.observations]
For every revision admitted through change-ID intake Kiln MUST record, as provider observations keyed by the Kiln revision, the source (`jj` or `git_change_id_header`), the change ID, the commit, the observed operation, the creating operation and jj predecessor commits when available, and the commit author. These observations MUST NOT become Kiln identities. Every new profile, request and state field MUST be omitted when default so existing journals, profiles and review state replay byte-identically.

#### Scenario: Legacy state replays byte-identically

r[kiln.review.jj_intake.observations.scenario.compatibility]
- GIVEN review state and a profile serialized before this change
- WHEN they are deserialized and serialized again
- THEN the bytes MUST be identical

### Requirement: Stack submission by change ID

r[kiln.review.jj_intake.stack]
A stack request of ordered exact change IDs (bottom to top) MUST create or update one Kiln change per change ID and MUST set dependency edges in ancestor order, each binding the previous member's current revision. On resubmission Kiln MUST map members by change ID, append a revision only to members whose commit changed, leave unchanged and already-submitted members untouched, and update dependency edges when the order or a bound revision changes. A change ID that disappears from the stack MUST be flagged on its change and MUST NOT be abandoned. Each change MUST keep its own approvals, CI and compare-and-swap submission; stack intake MUST NOT grant batch-merge authority.

#### Scenario: Four-change stack and replay

r[kiln.review.jj_intake.stack.scenario.create]
- GIVEN a real jj stack of four changes
- WHEN it is submitted as a stack and then submitted again unchanged
- THEN four Kiln changes MUST exist where each member above the bottom depends only on the member below it, and the second submission MUST create nothing and append nothing

#### Scenario: Two edited members

r[kiln.review.jj_intake.stack.scenario.partial]
- GIVEN the four-change stack already submitted
- WHEN exactly two changes are rewritten in jj and the stack is resubmitted
- THEN exactly those two Kiln changes MUST gain one revision each and the others MUST keep their revision count

#### Scenario: Reorder

r[kiln.review.jj_intake.stack.scenario.reorder]
- GIVEN a submitted stack
- WHEN the changes are reordered in jj and resubmitted in the new order
- THEN every dependency edge MUST follow the new ancestor order

#### Scenario: Removed change flagged

r[kiln.review.jj_intake.stack.scenario.removed]
- GIVEN a submitted stack
- WHEN it is resubmitted without one of its change IDs
- THEN that change MUST stay open and MUST be reported as removed from the stack on its change detail

### Requirement: Git change-id header intake

r[kiln.review.jj_intake.header]
When `intake.jujutsu.change_id_header` is enabled, Kiln MUST accept an admitted branch whose commits carry the jj `change-id` commit header as change-ID intake, without invoking jj, using the same change-ID mapping as jj intake. Missing or malformed headers, merge commits, duplicate change IDs and jj conflict trees MUST be refused before any provider effect.

#### Scenario: Header stack

r[kiln.review.jj_intake.header.scenario.stack]
- GIVEN a pushed branch of commits written by jj with `change-id` headers
- WHEN it is submitted with the header option as a stack
- THEN one Kiln change per header change ID MUST exist with ancestor-order dependencies, sharing identities with jj intake

### Requirement: Evolog view

r[kiln.review.jj_intake.evolog]
Change detail MUST render the revision log as an evolog: each revision with its jj predecessor commits, operation, commit author, declared authorship and a link to the interdiff from the previous revision, plus stack membership and the removed flag.

### Requirement: Operator surface

r[kiln.review.jj_intake.surface]
The optional `intake.jujutsu` profile record MUST be typed in Nickel with positive and negative fixtures, MUST be omitted when absent, and `docs/operator.md` MUST state the jj intake boundary and that jj observations are provider facts.
