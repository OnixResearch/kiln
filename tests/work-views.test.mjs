import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

// Load the served pure helper exactly as the Work page does.
const source = readFileSync(
  new URL("../crates/kiln-web-service/src/work-board.js", import.meta.url),
  "utf8",
);
const context = { window: {}, URLSearchParams };
vm.runInNewContext(source, context);
const kit = context.window.KilnWorkBoard;

const plain = (value) => JSON.parse(JSON.stringify(value));
const repositoryKey = "a".repeat(64);
const ci = {
  schema: "kiln.work.ci.v1",
  projects: [
    { id: "project-a", label: "Alpha", repository: "/repo", repository_key: repositoryKey, scope_root: "crates/a" },
    { id: "project-b", label: "Beta", repository: "/repo", repository_key: repositoryKey, scope_root: "crates/b" },
  ],
  records: { "issue-a": ["project-a"], "issue-b": ["project-b"], "issue-lost": ["removed"] },
};
const issue = (id, project) => ({ id, kind: "issue", fields: { title: id, project } });

// r[verify kiln.work.monorepo.filter.scenario.project]
test("CI project filters match only exact derived ids", () => {
  assert.equal(kit.matches(issue("issue-a"), { ci_project: "project-a" }, ci), true);
  assert.equal(kit.matches(issue("issue-b"), { ci_project: "project-a" }, ci), false);
  // An unknown id and a link the derived facts dropped both match nothing.
  assert.equal(kit.matches(issue("issue-lost"), { ci_project: "removed" }, ci), false);
  assert.equal(kit.matches(issue("unlinked"), { ci_project: "project-a" }, ci), false);
  // A comma set matches any exact member.
  assert.equal(kit.matches(issue("issue-b"), { ci_project: "project-a,project-b" }, ci), true);
});

// r[verify kiln.work.monorepo.filter.scenario.repository]
test("CI repository filters span one monorepo", () => {
  assert.equal(kit.matches(issue("issue-a"), { ci_repository: repositoryKey }, ci), true);
  assert.equal(kit.matches(issue("issue-b"), { ci_repository: repositoryKey }, ci), true);
  assert.equal(kit.matches(issue("unlinked"), { ci_repository: repositoryKey }, ci), false);
  assert.equal(kit.matches(issue("issue-a"), { ci_repository: "b".repeat(64) }, ci), false);
});

// r[verify kiln.work.monorepo.grouping.scenario.grouped]
test("CI grouping selects one exact project and an explicit unlinked group", () => {
  assert.deepEqual(plain(kit.ciGroup(issue("issue-b"), ci)), { id: "project-b", label: "Beta" });
  // The server orders ids repository-then-id; the first one is the group.
  const multi = { ...ci, records: { ...ci.records, "issue-multi": ["project-a", "project-b"] } };
  assert.deepEqual(plain(kit.ciGroup(issue("issue-multi"), multi)), { id: "project-a", label: "Alpha" });
  assert.equal(kit.ciGroup(issue("unlinked"), ci), null);
  assert.equal(kit.ciGroup(issue("issue-lost"), ci), null);
});

// r[verify kiln.work.monorepo.planning.scenario.set]
test("planning project sets match any member and keep single values", () => {
  const record = issue("issue", "project-b");
  assert.equal(kit.matches(record, { project: "project-a,project-b" }, ci), true);
  assert.equal(kit.matches(record, { project: "project-b" }, ci), true);
  assert.equal(kit.matches(record, { project: "project-a" }, ci), false);
});

test("board paths carry CI and project overrides", () => {
  assert.equal(
    kit.boardPath("view", { work_ci_project: "project-a", work_ci_repository: repositoryKey, work_project: "a,b", work_sort: "title" }),
    `/api/work/board?view=view&project=a%2Cb&ci_project=project-a&ci_repository=${repositoryKey}&sort=title`,
  );
});

// r[verify kiln.work.monorepo.board.scenario.lanes] r[verify kiln.work.monorepo.compat.scenario.invalid]
test("lane boards admit project lanes and status move columns", () => {
  const projection = {
    schema: "kiln.work.board.v1",
    principal: "local:operator",
    bounded: true,
    lane_by: "ci_project",
    matched_issues: 2,
    shown_cards: 2,
    list_only: 0,
    snapshot_objects: 2,
    object_limit: 1024,
    card_limit: 100,
    columns_truncated: 0,
    complete: true,
    columns: [
      { status: "project-a", builtin: false, retired: false, teams: [], ci: ci.projects[0], total: 1, cards: [{ id: "issue-a", head: "h", status: "backlog" }] },
      { status: "project-b", builtin: false, retired: false, teams: [], ci: ci.projects[1], total: 1, cards: [{ id: "issue-b", head: "h", status: "backlog" }] },
    ],
    move_columns: [
      { status: "backlog", builtin: true, retired: false, teams: [] },
    ],
    other: { statuses: [], total: 0, cards: [] },
  };
  assert.equal(kit.admit(projection, "local:operator").lane_by, "ci_project");
  assert.match(kit.completeness(projection).notes.join(" "), /Lanes are CI projects/);
  // A status lane masquerading as a card-bearing column is rejected rather than rendered.
  const invalid = { ...projection, move_columns: [{ status: "backlog" }] };
  assert.throws(() => kit.admit(invalid, "local:operator"), /board projection is invalid/);
});
