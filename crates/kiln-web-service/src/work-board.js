/* Pure Kanban helpers shared by the Work list and board: no DOM, storage, clock or network access. */
(() => {
  "use strict";
  // r[impl kiln.work.kanban.projection]
  // The Work page's URL overrides (also accepted by GET /api/work/board) and their route keys.
  const OVERRIDES = Object.freeze({ kind: "work_kind", status: "work_status", assignee: "work_assignee", project: "work_project", q: "work_q" });
  const SORTS = ["updated", "created", "priority", "title", "due"];

  // Filter semantics of the list. `work/board.rs` implements the same rules; one shared fixture checks both.
  function matches(record, filters) {
    const f = record.fields || {};
    return Object.entries(filters || {}).every(([key, value]) => {
      if (!value) return true;
      if (key === "q") return [f.title, f.description, f.customer, f.source_url, ...(f.labels || []), record.id].filter(Boolean).join(" ").toLowerCase().includes(String(value).toLowerCase());
      if (key === "kind") return record.kind === value;
      if (key === "assignee") return (f.assignees || []).includes(value);
      if (key === "label") return (f.labels || []).includes(value);
      if (!["status", "priority", "team", "project", "cycle", "initiative"].includes(key)) return false;
      return String(f[key] ?? "") === value;
    });
  }
  // Whether the Work page lists a record: private attention records only by explicit kind; without a saved
  // view or kind filter the page lists issues and requests.
  function listed(record, filters, viewActive) {
    const kind = filters?.kind || "";
    if (record.kind === "personal" && kind !== "personal") return false;
    if (!kind && !viewActive && !["issue", "request"].includes(record.kind)) return false;
    return matches(record, filters);
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
  function admit(value, principal) {
    const lanes = Array.isArray(value?.columns) ? value.columns : null;
    if (value?.schema !== "kiln.work.board.v1" || value.bounded !== true || value.principal !== principal || !lanes
      || ![value.matched_issues, value.shown_cards, value.list_only, value.snapshot_objects, value.object_limit, value.card_limit, value.columns_truncated].every(isCount)
      || typeof value.complete !== "boolean" || !value.other || !Array.isArray(value.other.cards) || !Array.isArray(value.other.statuses)
      || !lanes.every(lane => typeof lane.status === "string" && Array.isArray(lane.teams) && Array.isArray(lane.cards) && isCount(lane.total) && lane.cards.every(isCard))
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
    const p = projection, notes = [];
    const summary = p.complete ? `${plural(p.matched_issues, "matching issue")}, all shown` : `${p.shown_cards} of ${plural(p.matched_issues, "matching issue")} shown`;
    if (!p.complete) notes.push(`Columns show at most ${p.card_limit} cards each; ${p.matched_issues - p.shown_cards} matching issue(s) are not shown. Narrow the filters or use the list layout.`);
    if (p.filtered) notes.push("Filtered: issues outside this view's filters are not on this board.");
    if (p.list_only) notes.push(`${p.list_only === 1 ? "1 matching record is not an issue and appears" : `${p.list_only} matching records are not issues and appear`} only in the list layout.`);
    if (p.columns_truncated) notes.push(`${p.columns_truncated === 1 ? "1 team status exceeds" : `${p.columns_truncated} team statuses exceed`} the column bound; their issues are under Other statuses.`);
    notes.push(`Bounded snapshot: ${p.snapshot_objects} of at most ${p.object_limit} work objects.`);
    return { summary, notes };
  }

  window.KilnWorkBoard = Object.freeze({ OVERRIDES, matches, listed, boardPath, admit, findCard, moveTargets, defaultTarget, moveCommand, classify, resolve, completeness });
})();
