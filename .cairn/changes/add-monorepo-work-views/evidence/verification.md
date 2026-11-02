# Verification evidence

## Result

Work saved views, grouping and boards now understand monorepo CI projects.
`ci_project` and `ci_repository` filters use bounded derived facts from exact
`run` links; `group_by=ci_project` groups each row once; `board.lane_by=ci_project`
projects one lane per admitted project with unlinked issues under Other; and the
planning `project` filter accepts a bounded project set. Signed work records and
existing view bytes are unchanged.

## Evidence

- `cargo test -p kiln-web-service --lib work::` passes the model, board, CI-fact
  and compatibility tests, including unknown links, set bounds, duplicate
  rejection, single-card lane assignment, empty lanes, move columns and old
  view bytes.
- `cargo test -p kiln-web-service --test work` passes 15 native signed-store
  tests; the new monorepo case creates linked and unlinked issues, verifies the
  `kiln.work.ci.v1` listing facts, projects lane cards once, checks Other,
  `move_columns`, and narrows by both `ci_project` and `ci_repository`.
- `cargo test -p kiln-web-service --test engine work_ci_facts` verifies profile
  project facts, repository key/path and scope roots from the served profile.
- The complete `kiln-web-service` library (163 passed) and integration suites
  pass. `auth::tests::feed_token_reloads_after_file_rotation` is environment
  sensitive: `/tmp` on this host is ZFS and repeated same-length writes can keep
  the same `(mtime, len)` fingerprint, so the cache-reload assertion fails. The
  test targets untouched authentication code; a parent-revision run in a
  separate worktree passed, confirming the failure is not introduced here.
- `node --test tests/live-console-ui.test.mjs tests/work-views.test.mjs` passes
  47 tests. The new pure helper cases cover exact CI project/repository matching,
  unlinked non-matching, planning sets, board paths, lane admission, status move
  columns and explicit rejection of malformed lanes.
- A disposable real HTTPS service with two scoped projects sharing one
  repository was driven in Chromium. The saved lane board rendered `API service`
  and `Web client` lanes, all three unlinked issues under `No linked CI project`,
  and only the API project lane after the CI project filter. The multi-project
  planning view showed exactly the two assigned issues, and list grouping showed
  the explicit unlinked group. No provider effects or remote publication ran.
- `cargo clippy -p kiln-web-service --all-targets -- -D warnings`, `cargo fmt
  -p kiln-web-service -- --check`, JavaScript syntax checks, `cairn validate`
  and the active-development traceability profile (no missing requirements)
  pass.

## Non-claims

Derived CI facts are read-only observations over retained runs. They do not
write signed work state, prove CI success, grant execution, merge or release
authority, or make an unresolved link satisfy a filter. The browser exercise used
a disposable local service, not a production deployment.
