# Implementation Work Specification

## ADDED Requirements

### Requirement: Bounded versioned implementation-work contract

r[kiln.implementation.contract] Kiln MUST admit implementation work only through
a versioned bounded contract that binds canonical identity, exact source aliases
and source versions, enrolled repository, project and actor, title, requested
outcome, reviewed acceptance obligations, dependency identities, permitted
repository-relative paths, permitted effect classes, verification profile,
attempt budget, terminal policy, and priority. Unknown schemas, unenrolled
actors or repositories, unsafe paths, unknown effects, missing acceptance,
missing budgets, and fields over their named bounds MUST fail before any version
is retained. Admission MUST NOT claim, execute, verify, publish, or complete
work, and archive readiness MUST NOT be part of the contract.

#### Scenario: Complete contract is admitted

r[kiln.implementation.contract.scenario.admit]
- GIVEN a complete bound contract with an enrolled repository and actor
- WHEN admission runs
- THEN the canonical identity and alias key MUST be deterministic
- AND the work MUST be retained as ready when acceptance and dependencies are satisfiable

#### Scenario: Malformed or unauthorized input is refused

r[kiln.implementation.contract.scenario.refused]
- GIVEN an unknown schema, an unenrolled actor or repository, an escaping path, an empty acceptance set, a zero budget, or an oversized field
- WHEN admission runs
- THEN it MUST fail before retaining a version
- AND no dispatch authority MAY be created

### Requirement: Explicit task-level source mapping

r[kiln.implementation.mapping] Linked issue and Cairn task observations MUST
resolve to one canonical work key only through an explicit source mapping. Every
mapped alias MUST appear in the admitted alias set; repeated, partial, or forged
mappings MUST be refused. Unrelated work MUST NOT merge by title similarity, and
one parent MUST retain several independent task-level work units.

#### Scenario: Linked units merge

r[kiln.implementation.mapping.scenario.linked]
- GIVEN an issue and its explicitly mapped task-level units
- WHEN both are admitted together
- THEN they MUST share one canonical key and identity

#### Scenario: Forged or unrelated mapping is refused

r[kiln.implementation.mapping.scenario.forged]
- GIVEN a mapping that omits an alias, adds an unobserved alias, or repeats one unit
- WHEN admission runs
- THEN it MUST fail
- AND an unrelated task with the same title MUST remain a separate work unit

### Requirement: Visible blocked eligibility and deterministic ranking

r[kiln.implementation.eligibility] Missing dependencies, absent acceptance
obligations, and dependency cycles MUST produce visible blocked reasons.
Blocked work MUST NOT be ranked or dispatched. Only ready work MUST be ranked,
deterministically by reviewed priority and then canonical key.

#### Scenario: Missing dependency or cycle blocks

r[kiln.implementation.eligibility.scenario.blocked]
- GIVEN a work unit with an unadmitted dependency, no acceptance obligation, or a dependency cycle
- WHEN admission or ranking runs
- THEN the unit MUST expose a blocked state with its reasons
- AND it MUST NOT appear in the ready ranking

#### Scenario: Ready ranking is stable

r[kiln.implementation.eligibility.scenario.rank]
- GIVEN several ready work units
- WHEN ranking runs repeatedly
- THEN the same identities MUST appear in the same order

### Requirement: Replay, duplicate and supersession semantics

r[kiln.implementation.replay] Duplicate or out-of-order source observations MUST
produce one retained canonical version and no dispatch by themselves. A lower
source sequence MUST be stale. An equal sequence with different content MUST be
refused as ambiguous. A higher sequence MUST supersede the retained version while
keeping the prior snapshot auditable. Replay MUST produce the same identities and
selection decisions, and an archived or deleted Cairn change MUST NOT prove work
completion.

#### Scenario: Duplicate and out-of-order deliveries

r[kiln.implementation.replay.scenario.duplicate]
- GIVEN repeated or out-of-order observations of one source alias set
- WHEN admission runs
- THEN exactly one current version MUST remain
- AND duplicate or stale observations MUST NOT dispatch work

#### Scenario: Source edit supersedes

r[kiln.implementation.replay.scenario.edit]
- GIVEN an admitted work unit and a later source revision with changed acceptance
- WHEN the later observation is admitted
- THEN it MUST supersede the prior identity
- AND the prior snapshot MUST remain in history

#### Scenario: Lifecycle state is not completion

r[kiln.implementation.replay.scenario.lifecycle]
- GIVEN an implementation-ready Cairn task whose archive readiness is false, or an archived or deleted change
- WHEN work is admitted
- THEN readiness MUST follow the explicit contract only
- AND lifecycle state MUST NOT be treated as implementation completion
