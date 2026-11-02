# Work Monorepo Views Specification

## Purpose

Defines monorepo-aware saved views, grouping and boards over the signed work
workspace. Every CI fact here is a derived, read-only join through an explicit
exact `run` link; none of it is written into signed work records or grants CI,
execution, merge or release authority.

## ADDED Requirements

### Requirement: Derived exact CI facts

r[kiln.work.monorepo.facts]
The Work API MUST derive `kiln.work.ci.v1` facts from the admitted service
profile and retained runs: admitted project id, label, canonical repository,
BLAKE3 repository key, optional scope root, and each visible record's exact
linked project ids. It MUST derive a record's project ids only from that
record's explicit `run` links whose retained run resolves to an admitted
project. It MUST NOT write these facts into signed work records, infer a
project from titles or repository text, or claim CI success. Project facts and
run facts MUST obey named hard ceilings and report truncation.

#### Scenario: Linked run resolves

r[kiln.work.monorepo.facts.scenario.resolved]
- GIVEN an admitted CI project and a retained run of that project
- AND a work record with an exact `run` link to that run
- WHEN the Work listing is projected
- THEN the derived facts name that exact project for the record.

#### Scenario: Link cannot be resolved

r[kiln.work.monorepo.facts.scenario.unknown]
- GIVEN a work record whose `run` link has no retained run or no admitted project
- WHEN the Work listing is projected
- THEN the record has no derived project for that link and no project is invented.

### Requirement: CI project and repository filters

r[kiln.work.monorepo.filter]
Saved views and board requests MUST accept bounded `ci_project` and
`ci_repository` filters over derived exact facts. Each value is a comma-separated
set of at most 64 exact CI project ids or 64 lowercase BLAKE3 repository keys.
A record MUST match when any derived project id or repository key is in the
selected set. An unresolvable link, an unknown id, an unknown repository key or
an absent fact MUST NOT match a positive CI filter. These filters MUST NOT
change work records, CI state or authority.

#### Scenario: Monorepo project filter

r[kiln.work.monorepo.filter.scenario.project]
- GIVEN two CI projects sharing one repository and issues linked to runs of each
- WHEN a saved view filters by one project
- THEN only issues with an exact link to that project match.

#### Scenario: Repository filter spans the monorepo

r[kiln.work.monorepo.filter.scenario.repository]
- GIVEN two CI projects sharing one repository
- WHEN a view filters by the shared repository key
- THEN issues linked to either project match.

### Requirement: Planning-project set

r[kiln.work.monorepo.planning]
The planning `project` filter MUST accept a comma-separated set of at most 64
admitted planning project ids. A record MUST match when its canonical planning
project is any member of the set. Every named id MUST be an admitted planning
project when the view is written. A single value MUST keep its previous meaning
and absent keys MUST serialize unchanged.

#### Scenario: Several planning projects

r[kiln.work.monorepo.planning.scenario.set]
- GIVEN issues on planning projects A and B
- WHEN a saved view names `A,B`
- THEN both issues match and an issue on C does not.

### Requirement: CI-project grouping and row facts

r[kiln.work.monorepo.grouping]
The Work list MUST accept `group_by = ci_project`. Each record MUST appear once,
under its first exact linked project in deterministic project order, or under an
explicit unlinked group. Rows and detail MUST show the derived CI project and
repository labels without making them signed fields or CI outcomes.

#### Scenario: Grouped monorepo work

r[kiln.work.monorepo.grouping.scenario.grouped]
- GIVEN issues linked to runs of two CI projects and one issue with no run link
- WHEN the list groups by CI project
- THEN two project groups and one unlinked group appear, with no duplicate row.

### Requirement: CI-project board lanes

r[kiln.work.monorepo.board]
A board view MAY declare `lane_by = ci_project` (absent means status). In that
mode lanes MUST be the admitted CI projects matching the view's CI filters, in
deterministic repository-then-id order, including empty lanes, bounded by the
board column bound with the omitted count reported. Each matching issue MUST be
a card in exactly one lane: the first lane containing an exact linked project
id, or the Other lane when none resolves. Card status MUST remain on the card;
status columns MUST NOT also be configured. The projection MUST expose the
status columns used by Move so moving a card never targets a project lane.
Drag-to-lane MUST remain a status-board behavior only.

#### Scenario: Lanes per project and unlinked work

r[kiln.work.monorepo.board.scenario.lanes]
- GIVEN two CI projects and issues linked to each plus an issue with no run link
- WHEN the view's board is projected with `lane_by = ci_project`
- THEN each project lane contains its linked issue once and the unlinked issue is in Other.

#### Scenario: Empty lane stays visible

r[kiln.work.monorepo.board.scenario.empty]
- GIVEN an admitted CI project with no matching issue
- WHEN the lane board is projected
- THEN its lane is present with zero cards.

### Requirement: Bounded compatibility

r[kiln.work.monorepo.compat]
Saved views and boards without the new fields MUST read, replay and serialize
with their previous bytes and default status columns. Unknown filter keys,
grouping values, lane modes, malformed sets, over-bound sets, duplicate ids and
non-canonical repository keys MUST fail closed without changing state. The
derived facts, filters, grouping and lanes MUST remain read-only.

#### Scenario: Existing status board

r[kiln.work.monorepo.compat.scenario.old]
- GIVEN a board view written before lane modes existed
- WHEN it is read and projected
- THEN it has status lanes, no `lane_by` bytes and the previous card membership.

#### Scenario: Invalid set

r[kiln.work.monorepo.compat.scenario.invalid]
- GIVEN a view with an empty, duplicate, over-bound or malformed project set
- WHEN the view is written
- THEN the command fails without changing state.
