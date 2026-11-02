/* Pure Kanban helpers shared by the Work list and board: no DOM, storage, clock or network access. */
(() => {
  "use strict";
  // r[impl kiln.work.kanban.projection] r[impl kiln.work.monorepo.filter]
  // The Work page's URL overrides (also accepted by GET /api/work/board) and their route keys.
  const OVERRIDES = Object.freeze({ kind: "work_kind", status: "work_status", assignee: "work_assignee", project: "work_project", q: "work_q", ci_project: "work_ci_project", ci_repository: "work_ci_repository" });
  const SORTS = ["updated", "created", "priority", "title", "due"];
  const exactSet = value => new Set(String(value || "").split(",").map(part => part.trim()).filter(Boolean));

  // Bounded derived facts from the Work listing. The server derives them from explicit run links;
  // the browser never invents a project or treats an unresolved link as a match.
  function ciIds(ci, id) {
    const rows = ci?.records?.[id];
    return Array.isArray(rows) ? rows.filter(row => typeof row === "string" && row && ciProject(ci, row)) : [];
  }
  function ciProject(ci, id) {
    const projects = Array.isArray(ci?.projects) ? ci.projects : [];
    return projects.find(project => project && project.id === id) || null;
  }
  // One deterministic CI group per record: its first exact linked project, or no group.
  function ciGroup(record, ci) {
    const id = ciIds(ci, record.id)[0] || "";
    const project = id ? ciProject(ci, id) : null;
    return project ? { id: project.id, label: project.label } : null;
  }
  function ciMatches(record, filters, ci) {
    const ids = ciIds(ci, record.id);
    if (filters.ci_project) {
      const selected = exactSet(filters.ci_project);
      if (!ids.some(id => selected.has(id))) return false;
    }
    if (filters.ci_repository) {
      const selected = exactSet(filters.ci_repository);
      if (!ids.some(id => selected.has(ciProject(ci, id)?.repository_key || ""))) return false;
    }
    return true;
  }

  // Filter semantics of the list. `work/board.rs` implements the same rules; one shared fixture checks both.
  function matches(record, filters, ci) {
    const f = record.fields || {};
    if (!ciMatches(record, filters || {}, ci)) return false;
    return Object.entries(filters || {}).every(([key, value]) => {
      if (!value) return true;
      if (key === "q") return [f.title, f.description, f.customer, f.source_url, ...(f.labels || []), record.id].filter(Boolean).join(" ").toLowerCase().includes(String(value).toLowerCase());
      if (key === "kind") return record.kind === value;
      if (key === "assignee") return (f.assignees || []).includes(value);
      if (key === "label") return (f.labels || []).includes(value);
      if (key === "project") return exactSet(value).has(String(f.project || ""));
      if (key === "ci_project" || key === "ci_repository") return true;
      if (!["status", "priority", "team", "cycle", "initiative"].includes(key)) return false;
      return String(f[key] ?? "") === value;
    });
  }
  // Whether the Work page lists a record: private attention records only by explicit kind; without a saved
  // view or kind filter the page lists issues and requests.
  function listed(record, filters, viewActive, ci) {
    const kind = filters?.kind || "";
    if (record.kind === "personal" && kind !== "personal") return false;
    if (!kind && !viewActive && !["issue", "request"].includes(record.kind)) return false;
    return matches(record, filters, ci);
  }
  function boardPath(viewId, route = {}) {
    const params = new URLSearchParams();
    if (viewId) params.set("view", viewId);
    for (const [key, routeKey] of Object.entries(OVERRIDES)) { const value = String(route[routeKey] || "").trim(); if (value) params.set(key, value); }
    if (SORTS.includes(route.work_sort)) params.set("sort", route.work_sort);
    const query = params.toString();
    return query ? `/api/work/board?${query}` : "/api/work/board";
  }
  const isCount = value => Number.isInteger(value) && value >= 0;
  const isCard = card => card && typeof card.id === "string" && typeof card.head === "string" && card.id && card.head && typeof card.status === "string";
  const isProjectFact = project => project && typeof project.id === "string" && project.id
    && typeof project.label === "string" && typeof project.repository === "string"
    && typeof project.repository_key === "string" && project.repository_key
    && (project.scope_root === undefined || project.scope_root === null || typeof project.scope_root === "string");
  const isColumn = lane => lane && typeof lane.status === "string" && Array.isArray(lane.teams) && Array.isArray(lane.cards)
    && isCount(lane.total) && lane.cards.every(isCard)
    && (lane.ci === undefined || isProjectFact(lane.ci));
  const isMoveColumn = column => column && typeof column.status === "string" && Array.isArray(column.teams)
    && typeof column.builtin === "boolean" && typeof column.retired === "boolean"
    && (column.ci === undefined || isProjectFact(column.ci));
  function admit(value, principal) {
    const lanes = Array.isArray(value?.columns) ? value.columns : null;
    const laneBy = value?.lane_by === undefined ? "status" : value.lane_by;
    const moveColumns = value?.move_columns === undefined ? null : value.move_columns;
    if (value?.schema !== "kiln.work.board.v1" || value.bounded !== true || value.principal !== principal || !lanes
      || !["status", "ci_project"].includes(laneBy)
      || ![value.matched_issues, value.shown_cards, value.list_only, value.snapshot_objects, value.object_limit, value.card_limit, value.columns_truncated].every(isCount)
      || typeof value.complete !== "boolean" || !value.other || !Array.isArray(value.other.cards) || !Array.isArray(value.other.statuses)
      || !lanes.every(isColumn) || (moveColumns !== null && (!Array.isArray(moveColumns) || !moveColumns.every(isMoveColumn)))
      || !value.other.cards.every(isCard))
      throw new Error("The board projection is invalid or belongs to a different principal.");
    return value;
  }
  function findCard(projection, id) {
    for (const lane of [...(projection?.columns || []), projection?.other]) { const card = lane?.cards?.find(item => item.id === id); if (card) return card; }
    return null;
  }

  // r[impl kiln.work.kanban.move]
  // Client precheck only; the server revalidates against the issue's team workflow at its exact head.
  function moveTargets(card, columns, now) {
    return (columns || []).map(column => {
      const target = { status: column.status, allowed: false, reason: "" };
      if (column.status === card.status) target.reason = "Current column";
      else if (column.retired) target.reason = "Retired: no team workflow defines this status";
      else if (column.status === "duplicate") target.reason = "Needs a duplicate target: use Edit…";
      else if (column.status === "snoozed" && !(card.due_at_ms != null && card.due_at_ms > now)) target.reason = "Needs a future snooze date: use Edit…";
      else if (!column.builtin && !(card.team && column.teams.includes(card.team))) target.reason = card.team ? "Not in this issue's team workflow" : "Custom status: the issue has no team";
      else target.allowed = true;
      return target;
    });
  }
  function defaultTarget(card, columns, now) {
    const targets = moveTargets(card, columns, now), current = targets.findIndex(target => target.status === card.status);
    return (targets.slice(current + 1).find(target => target.allowed) || targets.find(target => target.allowed))?.status || "";
  }
  function moveCommand(card, status, requestId) {
    return { request_id: requestId, object_id: card.id, expected_head: card.head, action: { type: "set_status", status } };
  }

  // r[impl kiln.work.kanban.interaction]
  // "unknown" is the only retryable class: the outcome may already have applied, so a retry must resend the
  // identical command (same request ID) and the store returns the original result instead of a second move.
  function classify(error) {
    if (error?.name === "AbortError") return "aborted";
    const code = String(error?.message || "").split(":")[0];
    if (error?.status === 409) return code === "work_head_conflict" ? "conflict" : "replay";
    if (error?.status === 400) return "invalid";
    if (error?.status === 403) return "denied";
    if (error?.status === 404) return "missing";
    return "unknown";
  }
  // After a reload, decide what the user may do with the attempted move.
  function resolve(attempt, current) {
    if (!current) return { kind: "missing" };
    if (current.status === attempt.target) return { kind: "applied", card: current };
    if (current.head === attempt.card.head) return { kind: "unchanged", card: current };
    return { kind: "changed", card: current };
  }

  const plural = (count, noun) => `${count} ${noun}${count === 1 ? "" : "s"}`;
  // A filtered or bounded projection is never presented as the whole backlog.
  function completeness(projection) {
    const p = projection, notes = [], lanes = p.lane_by === "ci_project";
    const summary = p.complete ? `${plural(p.matched_issues, "matching issue")}, all shown` : `${p.shown_cards} of ${plural(p.matched_issues, "matching issue")} shown`;
    if (!p.complete) notes.push(`${lanes ? "Lanes" : "Columns"} show at most ${p.card_limit} cards each; ${p.matched_issues - p.shown_cards} matching issue(s) are not shown. Narrow the filters or use the list layout.`);
    if (p.filtered) notes.push("Filtered: issues outside this view's filters are not on this board.");
    if (p.list_only) notes.push(`${p.list_only === 1 ? "1 matching record is not an issue and appears" : `${p.list_only} matching records are not issues and appear`} only in the list layout.`);
    if (lanes) {
      notes.push("Lanes are CI projects derived from explicit run links; a card's status is still its planning status.");
      if (p.columns_truncated) notes.push(`${p.columns_truncated === 1 ? "1 CI project exceeds" : `${p.columns_truncated} CI projects exceed`} the lane bound; their issues are under Other.`);
      if (p.ci_projects_truncated) notes.push(`${p.ci_projects_truncated} admitted CI project fact(s) exceeded the derived-fact bound and are not shown.`);
      if (p.ci_runs_truncated) notes.push(`${p.ci_runs_truncated} retained run fact(s) exceeded the derived-fact bound; those links are unlinked here.`);
    } else {
      if (p.columns_truncated) notes.push(`${p.columns_truncated === 1 ? "1 team status exceeds" : `${p.columns_truncated} team statuses exceed`} the column bound; their issues are under Other statuses.`);
      if (p.ci_projects_truncated || p.ci_runs_truncated) notes.push("Derived CI facts exceeded a hard bound; some CI filters may match fewer records than the admitted profile contains.");
    }
    notes.push(`Bounded snapshot: ${p.snapshot_objects} of at most ${p.object_limit} work objects.`);
    return { summary, notes };
  }

  window.KilnWorkBoard = Object.freeze({ OVERRIDES, matches, listed, boardPath, admit, findCard, moveTargets, defaultTarget, moveCommand, classify, resolve, completeness, ciIds, ciProject, ciGroup });
})();
