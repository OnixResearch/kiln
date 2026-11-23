# Implementation work delta

## ADDED Requirements

### Requirement: Explicit per-issue intake selection

r[kiln.implementation.intake_selection] A GitHub intake policy MAY carry an explicit selection of issue numbers and exact label names. When present, webhook drain and polling MUST admit an open issue as work only when its number is listed or it carries a selected label, and MUST observe every other open issue as unselected without admitting it. Closed issues MUST still be observed as closed. An empty selection, issue zero or a malformed or unbounded label MUST fail before any provider call. Absent selection MUST preserve the existing repository-wide behaviour.

#### Scenario: Only the selected issue becomes work

r[kiln.implementation.intake_selection.scenario.selected]
- GIVEN open issues #1, #2 and a labelled #3 and a policy selecting issue #1 and that label
- WHEN polling or webhook intake runs, including a repeated poll
- THEN #1 and #3 MUST be admitted once and #2 MUST be reported unselected and never admitted

#### Scenario: Unselected or invalid selection admits nothing

r[kiln.implementation.intake_selection.scenario.refused]
- GIVEN an issue outside the selection, a label differing only in case, an empty selection, issue zero or a malformed label
- WHEN intake runs
- THEN the issue MUST be observed as unselected or the intake MUST fail before admission
