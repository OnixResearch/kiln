## Context

`View.filters` is a `BTreeMap<String, String>` validated by
`work::model::validate_view`; `work::board::project` is a pure projection over
the visible record snapshot. The work COB never sees the service profile, so it
cannot know which CI project owns an exact linked run. The service engine owns
`ProjectProfile` and `RunRecord.project_id`; the `/api/work` route currently
calls the work workspace without engine facts.

## Decisions

### Decision: Derived CI facts, never signed work fields

**Choice:** the web-service shell builds a bounded `work::ci::CiFacts` from the
admitted service profile and retained runs, then passes it to the Work API for
one request. The listing exposes `kiln.work.ci.v1` with project metadata and a
per-object map of exact project ids derived only from `links[kind=run]`.
`CiFacts` is not written into COBs, request bodies or work commands.

**Rationale:** signed work history stays independent of the CI profile, and
profile changes cannot rewrite signed records. Unknown links remain explicit.

### Decision: Set-valued filters in existing filter values

**Choice:** `project`, `ci_project` and `ci_repository` accept a bounded
comma-separated set (at most 64 values). CI project ids and BLAKE3 keys cannot
contain commas, so older single values keep their meaning. Existing values and
absent keys serialize and replay unchanged.

**Rationale:** no schema migration, no second filter map, and one matching rule
for saved views, URL overrides and the board.

### Decision: CI filters are syntax-checked, not existence-checked

**Choice:** `validate_view` checks the shape and bound of `ci_project` and
`ci_repository` values but not whether the profile still admits them.
Planning-project values are still checked against live project records because
they are signed work references. An unknown CI id or repository key matches
nothing.

**Rationale:** a signed view must remain readable after a project is removed
from the service profile; silently rejecting an old signed view would be worse
than an explicit no-match.

### Decision: Lane mode is board configuration

**Choice:** `Board` gains optional `lane_by` (`status` default, `ci_project`
the alternative). A `ci_project` board MUST NOT also declare status `columns`.
Lanes are bounded CI projects in deterministic repository-then-id order,
including empty lanes. Each matching issue is projected once, into the first
lane that contains one of its exact linked project ids; unresolved issues go to
the existing Other lane. `Projection.move_columns` always carries the status
columns so Move keeps targeting statuses.

**Rationale:** the board stays a read-only projection; status remains on each
card; no card is duplicated across lanes; status moves do not need a second
board API.

### Decision: Board/list stay pure

**Choice:** `board::matches`, `board::listed` and `board::project` call a
`CiFacts` value supplied by the shell. Empty facts are the default, preserving
all existing tests and callers.

**Rationale:** pure core rules remain testable without a service, engine, clock
or network.

## Risks / Trade-offs

- Run retention can remove the linked run that proves a project; such a link is
  unknown and cannot satisfy a positive CI filter. This is deliberate and shown
  as unlinked.
- A record linked to runs of several projects appears once in a lane board and
  in the first group of a grouped list; the remaining exact ids remain visible
  in its detail and row chips.
- CI filters add a bounded derived read to each Work request; the hard ceilings
  and truncation flags keep it bounded when profiles are unexpectedly large.
- Board lanes are CI projects, so drag-to-lane does not mean "move to status".
  Lane boards keep the explicit Move dialog (status targets) and disable drag.
