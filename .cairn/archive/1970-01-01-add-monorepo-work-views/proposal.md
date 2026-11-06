## Why

Work saved views filter planning records by one planning project and group by
status, assignee, project or team. A monorepo has several CI projects sharing
one repository, and a work record reaches one of them only through an explicit
`run` link. Today no saved view can select work by CI project or repository,
group it across the monorepo, or project a board whose lanes are the monorepo's
projects. The four requested behaviors are therefore absent rather than broken
by a defect: filters, grouping, board lanes and the planning-project set all
stop at the single planning project.

## What Changes

- Derive bounded, non-canonical `kiln.work.ci.v1` facts per work record from its
  explicit `run` links: admitted CI project id, label, repository and
  repository key. Signed work records are unchanged.
- Saved views and board requests accept `ci_project` and `ci_repository`
  filters evaluated over those facts; unresolved or absent links never match a
  positive CI filter.
- List grouping accepts `ci_project`; each record appears once under its first
  exact linked project in deterministic order, with an explicit unlinked group.
- A board view may declare `lane_by = ci_project`; lanes become bounded CI
  projects, status stays on each card, and issues without a resolved project
  appear under Other. `move_columns` keeps status moves working.
- The planning-project filter accepts a bounded comma-separated project set.
- The Work console exposes the new filters, grouping, row chips, lane labels
  and saved-view fields; route state restores them.

## Impact

- **Files**: `crates/kiln-web-service/src/work.rs`,
  `work/{model,board,ci}.rs`, `engine.rs`, `server.rs`, Work JavaScript/CSS,
  work tests, `docs/operator.md`, the delta spec and the development coverage
  overlay.
- **Testing**: pure model/board tests, API shell tests, JavaScript contract
  checks, focused Cargo tests, Clippy, rustfmt, Cairn validation and a real
  API/board exercise.
- **Non-claims**: derived CI facts are read-only observation joins. They do not
  write signed work state, prove CI success, grant execution or release
  authority, or make an unresolved link pass a filter.
