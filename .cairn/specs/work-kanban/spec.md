# Work Kanban Specification

## Purpose

Defines the `work-kanban` capability.

## Requirements

### Requirement: Board configuration on the view COB

r[kiln.work.kanban.config]
A Kanban board MUST be an optional, bounded configuration of the existing view COB, never a separate task store. Views without board configuration MUST keep reading, replaying and rendering, and MUST use deterministic default columns. Extending the configuration MUST NOT change view ownership, visibility or write admission.

#### Scenario: Old saved view

r[kiln.work.kanban.config.scenario.migration]
- GIVEN a view history written before board configuration existed
- WHEN the store replays it and the board is requested
- THEN the view MUST read unchanged and the board MUST use the default columns

#### Scenario: Invalid configuration

r[kiln.work.kanban.config.scenario.invalid]
- GIVEN a new or changed board configuration that is empty, too long, duplicated or names a status no team defines
- WHEN it is written
- THEN the command MUST be rejected without changing state

### Requirement: Ordered status columns

r[kiln.work.kanban.columns]
Board columns MUST follow the configured order, or else the documented default order from built-in statuses and team workflows. Empty columns MUST remain visible. Mixed-team boards MUST name the teams that define each custom column. Columns whose status no team defines MUST be marked retired, and matching issues whose status has no column MUST appear in an Other statuses lane.

#### Scenario: Unknown status

r[kiln.work.kanban.columns.scenario.unknown]
- GIVEN a configured board that omits a status some matching issue has
- WHEN the board is projected
- THEN that issue MUST appear in the Other statuses lane with its status named

### Requirement: Canonical status move

r[kiln.work.kanban.move]
Moving a card MUST persist exactly one `set_status` operation on that issue through the existing writer admission, request replay and expected-head checks. The target status MUST be valid for the issue's team workflow. Only the issue's status (and a cleared duplicate target when leaving duplicate) MAY change; the view MUST NOT be written.

#### Scenario: Stale head

r[kiln.work.kanban.move.scenario.stale]
- GIVEN an issue changed after a board was read
- WHEN a move with the old head is sent
- THEN it MUST fail with a head conflict and leave the newer state unchanged

#### Scenario: Duplicate retry

r[kiln.work.kanban.move.scenario.retry]
- GIVEN a move whose outcome is unknown to the client
- WHEN the identical command is sent again with the same request ID
- THEN the original result MUST be returned and no second operation stored

### Requirement: Bounded board projection

r[kiln.work.kanban.projection]
The board MUST be a read-only bounded projection of the visible snapshot with the same issue eligibility as the list. It MUST bound cards per column and report matched, shown and list-only counts, the snapshot bound and whether the projection is complete. Another principal's private view MUST be indistinguishable from a missing view.

### Requirement: Accessible move interaction

r[kiln.work.kanban.interaction]
Each card MUST offer a keyboard-reachable Move action; drag-and-drop MUST open the same confirmation. Cancel MUST send nothing. A failed request MUST show its reason; an uncertain outcome MUST be retried only with the same request ID. A head conflict MUST be shown with a reload and an explicit user choice, never silently overwritten.

### Requirement: Planning status boundary

r[kiln.work.kanban.boundary]
Documentation and the board MUST state that column status is planning state. Moving a card, including to completed, MUST NOT acquire execution authority or a worker lease, and MUST NOT be presented as CI or release completion.
