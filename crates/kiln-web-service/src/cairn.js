"use strict";
// r[impl kiln.cairn.workspace] Cairn changes (?page=cairn): a register of Cairn changes (list or board) and one change's
// next steps, units, dependencies and campaigns, joined by exact identity from the service's one bounded snapshot. The
// projection is read-only. The only command here is the campaign owner's own decision answer, offered through the shared
// campaign answer path exactly where the campaign inbox lists it. Displayed reasons are never parsed into decisions.
(() => {
  const INDEX_SCHEMA = "kiln.cairn-workspace-index.v2";
  const DETAIL_SCHEMA = "kiln.cairn-workspace-detail.v2";
  const INDEX_ROUTE = "/api/implementation/cairn";
  const DETAIL_ROUTE = "/api/implementation/cairn/detail";
  const HEX = /^[a-f0-9]{64}$/;
  // The service admits a change name of at most this many UTF-8 bytes, without "/" or control characters.
  const CHANGE_MAX_BYTES = 256;
  // A unit is `<change>/<unit>`; the console bounds what the URL may name.
  const UNIT_MAX_BYTES = 1024;
  const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;
  // Typing narrows the register at once and commits it to the URL after this pause, as on the Workers page.
  const FILTER_COMMIT_MS = 350;
  const FILTER_MAX_CHARS = 256;
  // Identities read in this short form; the exact value stays in their title and in the identities disclosure.
  const SHORT_HASH = 12;
  // An unrecognized recorded value is quoted up to this length. It never reads as success.
  const LABEL_TEXT_MAX = 48;

  const word = (label, tone = "", meaning = "") => Object.freeze({ label, tone, meaning });
  const vocabulary = (entries) => new Map(entries.map(([key, label, tone, meaning]) => [key, word(label, tone, meaning)]));

  // r[impl kiln.cairn.workspace.scenario.lanes] Board lanes in display order. A lane is the projection's position for a
  // unit or change; the five facets stay its evidence and are shown beside it.
  const LANES = vocabulary([
    ["attention", "Needs attention", "failure", "A stage reports a stop: blocked or unknown readiness, a blocked, failed, cancelled or superseded latest attempt, or failed verification."],
    ["awaiting", "Awaiting work", "", "No current canonical work: a campaign selection or remaining obligation names it, or the change names no unit yet."],
    ["ready", "Ready to start", "info", "Current work is eligible and no attempt on it has started."],
    ["in_progress", "In progress", "info", "An attempt on current work is claimed, executing, has produced a candidate or is verifying."],
    ["verified", "Verified", "good", "Independent verification of the current work version passed. Integration is not observed yet."],
    ["integrated", "Integrated", "good", "Exact integration of the current work was observed. No lifecycle receipt binds it yet."],
    ["recorded", "Lifecycle recorded", "good", "A native Cairn lifecycle receipt binds an attempt on current work. This is an observation, not lifecycle authority."],
  ]);
  const LANE_ORDER = [...LANES.keys()];
  // Five separately legible facets of a unit, each with explicit not-started, not-published and not-observed values.
  const FACETS = Object.freeze({
    readiness: Object.freeze({ title: "Readiness", values: vocabulary([
      ["ready", "Ready", "good", "Every current version is eligible by canonical admission."],
      ["blocked", "Blocked", "failure", "Canonical eligibility names a reason; next steps list it."],
      ["unknown", "Readiness unknown", "warning", "Canonical eligibility is unavailable. This is not ready."],
      ["not_admitted", "Not admitted", "", "No current canonical work names this unit."],
    ]) }),
    execution: Object.freeze({ title: "Execution", values: vocabulary([
      ["not_started", "Not started", "", "No attempt on the current work version."],
      ["claimed", "Claimed", "info", "An attempt holds the claim on the current work version."],
      ["executing", "Executing", "info", "The attempt's dispatch started."],
      ["candidate_produced", "Candidate produced", "info", "A candidate awaits independent verification."],
      ["verifying", "Verifying", "info", "Independent verification is running."],
      ["verified", "Attempt verified", "good", "The latest attempt on the current version was verified."],
      ["blocked", "Attempt blocked", "failure", "The latest attempt ended blocked; its cause is recorded."],
      ["failed", "Attempt failed", "failure", "The latest attempt failed; its failure class is recorded."],
      ["cancelled", "Attempt cancelled", "warning", "The latest attempt was cancelled."],
      ["superseded", "Attempt superseded", "warning", "A newer work version replaced the attempt's version."],
    ]) }),
    evidence: Object.freeze({ title: "Evidence", values: vocabulary([
      ["current_verified", "Verified for current work", "good", "Independent verification of the current work version passed."],
      ["current_failed", "Verification failed", "failure", "Verification of the current work version did not pass."],
      ["historical_only", "Historical evidence only", "warning", "Only attempts on superseded work were verified. That evidence does not apply to the current version."],
      ["none", "No verification", "", "No attempt recorded independent verification."],
    ]) }),
    integration: Object.freeze({ title: "Integration", values: vocabulary([
      ["not_published", "Not published", "", "The latest attempt on current work has no publication record."],
      ["prepared", "Publication prepared", "info", "A publication plan is recorded; nothing is published yet."],
      ["branch_published", "Branch published", "info", "The candidate branch is published."],
      ["pull_request_published", "Pull request published", "info", "The pull request is published; integration is not observed."],
      ["integrated", "Integrated", "good", "Exact integration was observed."],
      ["source_reconciled", "Source reconciled", "good", "Integration was observed and the source item reconciled."],
    ]) }),
    lifecycle: Object.freeze({ title: "Lifecycle", values: vocabulary([
      ["receipt_observed", "Receipt observed", "good", "A native Cairn lifecycle receipt binds an attempt on the current work."],
      ["not_observed", "No receipt", "", "No native Cairn lifecycle receipt binds an attempt on the current work."],
    ]) }),
  });
  const BINDINGS = vocabulary([
    ["current", "Binds current work", "good", "The member binds the current canonical work version."],
    ["stale", "Binds stale work", "warning", "The member binds a superseded canonical version. The campaign must replan."],
    ["unknown_work", "Binds unknown work", "warning", "The bound identity is not in the canonical ledger."],
    ["not_bound", "Not bound", "", "The member names no canonical work yet."],
  ]);
  const CONDITIONS = vocabulary([
    ["dispatch_recovery", "Unknown dispatch outcome", "failure", "A provider outcome is unknown. Its termination must be observed before another attempt."],
    ["dependency_cycle", "Dependency cycle", "failure", "The dependency graph reaches this work again."],
    ["no_acceptance_obligations", "No acceptance obligations", "failure", "The work admits no acceptance obligation."],
    ["missing_dependency", "Missing dependency", "failure", "A prerequisite work identity is not current canonical work."],
    ["plan_not_accepted", "Plan not accepted", "warning", "Opt-in plan review has no accepted plan for this exact work version."],
    ["stale_work_binding", "Stale work binding", "warning", "A campaign member binds work that is not current."],
    ["pending_effect", "Unapplied campaign effect", "warning", "An owner effect is persisted as intent and its progress is not applied."],
    ["campaign_hold", "Campaign hold", "warning", "The campaign's own selection holds this member."],
  ]);
  const OPERATIONS = vocabulary([
    ["reconcile", "Reconcile the recorded outcome"], ["author_artifact", "Author the artifact"], ["repair_artifact", "Repair the artifact"],
    ["code", "Implement"], ["repair_code", "Repair the implementation"], ["retry_infrastructure", "Retry after an infrastructure failure"],
    ["verify", "Verify independently"], ["publish", "Publish"], ["integrate", "Integrate"], ["lifecycle", "Record the native lifecycle"],
    ["request_decision", "Ask for a decision"], ["wait", "Wait for a prerequisite"], ["replan", "Replan the campaign member"],
    ["blocked", "No automatic step: the source must change"], ["complete", "Complete"],
  ]);
  const SOURCES = vocabulary([
    ["canonical_work", "Canonical work", "", "Current or superseded canonical work names this unit."],
    ["campaign_selection", "Campaign selection", "", "A campaign selects this task by name."],
    ["campaign_remaining_obligation", "Campaign obligation", "", "A campaign still owes this unit."],
  ]);
  const APPLICABILITY = vocabulary([
    ["current", "Current work", "good", "This attempt's work identity is the unit's current canonical version."],
    ["historical", "Historical", "warning", "This attempt ran on a superseded work version. Its records never read as current."],
  ]);
  const DISPATCH = vocabulary([
    ["not_reserved", "Not reserved", ""],
    ["reserved", "Reserved", "info", "A dispatch reservation is recorded; transport has not started."],
    ["transport_started", "Transport started", "info", "Transport intent was persisted; the provider may hold the work."],
  ]);
  const UNKNOWN_OUTCOMES = vocabulary([
    ["unreconciled", "Outcome unknown", "failure", "No termination is observed. Another attempt is refused until it is."],
    ["reconciled", "Termination observed", "warning", "The unknown outcome's termination was observed."],
  ]);
  const FAILURES = vocabulary([
    ["implementation", "Implementation failure", "failure"], ["infrastructure", "Infrastructure failure", "failure"], ["unknown", "Unknown outcome", "failure"],
  ]);
  const DECISION_KINDS = vocabulary([["preference", "Preference"], ["empirical_uncertainty", "Empirical uncertainty"], ["required_review", "Required review"]]);
  const DECISION_STATES = vocabulary([
    ["unissued", "Not issued yet", "info", "Waiting to be issued; the campaign's outstanding-decision bound may hold it."],
    ["outstanding", "Awaiting an answer", "warning", "Issued and waiting for an answer."],
    ["resolved", "Resolved", "good"],
    ["expired", "Expired", "warning"],
  ]);
  const SCOPES = vocabulary([
    ["current", "Scope current", "", "The member's work, policy and candidate still match this decision."],
    ["changed", "Scope changed", "warning", "The member moved on since this decision was asked; it cannot be answered."],
    ["member_unobserved", "Member unobserved", "warning", "The scope cannot be checked, so no answer is offered."],
  ]);
  const STAGES = vocabulary([
    ["selected", "Selected"], ["missing", "Missing", "warning"], ["planning", "Planning", "info"], ["blocked", "Blocked", "failure"],
    ["ready", "Ready", "info"], ["claimed", "Claimed", "info"], ["executing", "Executing", "info"], ["candidate", "Candidate produced", "info"],
    ["verifying", "Verifying", "info"], ["verified_candidate", "Verified candidate", "good"], ["verified_pull_request", "Verified pull request", "good"],
    ["integrated", "Integrated", "good"], ["lifecycle", "Lifecycle", "info"], ["complete", "Complete", "good"], ["cancelled", "Cancelled", "warning"],
  ]);
  const ENUMERATIONS = vocabulary([
    ["not_observed", "Enumeration not observed", "warning"], ["complete", "Enumeration complete"], ["incomplete_limit", "Enumeration incomplete: limit reached", "warning"],
    ["missing_source", "Enumeration source missing", "warning"], ["invalid", "Enumeration invalid", "failure"],
  ]);
  const ADMISSIONS = vocabulary([["open", "Admission open"], ["paused", "Admission paused", "warning"], ["cancelled", "Campaign cancelled", "warning"]]);
  const GOALS = vocabulary([
    ["verified_candidate", "Verified candidate"], ["verified_pull_request", "Verified pull request"], ["integrated_change", "Integrated change"],
    ["native_lifecycle_complete", "Native lifecycle complete"],
  ]);
  const LIFECYCLE_ACTIONS = vocabulary([["observe_reviewed_sync", "Reviewed sync"], ["observe_reviewed_archive", "Reviewed archive"]]);

  // The words for a recorded value. A value Kiln does not know is quoted, never read as success.
  function lookup(table, value) {
    if (typeof value === "string" && table.has(value)) return table.get(value);
    if (typeof value === "string" && value) {
      const quoted = value.length > LABEL_TEXT_MAX ? `${value.slice(0, LABEL_TEXT_MAX - 1)}…` : value;
      return word(`Unrecognized: ${quoted}`, "warning", "Kiln does not recognize this recorded value. It is shown as recorded and never read as success.");
    }
    return word("Not recorded", "warning");
  }
  function facet(name, value) { return lookup(FACETS[name]?.values || new Map(), value); }
  function lane(value) { return lookup(LANES, value); }
  function binding(value) { return lookup(BINDINGS, value); }
  function condition(value) { return lookup(CONDITIONS, value); }
  // An unrecognized lane never reads as progress: it sorts and boards with Needs attention.
  function boardLane(value) { return LANES.has(value) ? value : "attention"; }
  function laneRank(value) { return LANE_ORDER.indexOf(boardLane(value)); }

  function utf8Bytes(text) {
    try { return encodeURIComponent(text).replace(/%[0-9A-F]{2}/g, "x").length; } catch { return Infinity; }
  }
  function validChange(value) {
    return typeof value === "string" && value.trim() !== "" && !value.includes("/") && !CONTROL.test(value) && utf8Bytes(value) <= CHANGE_MAX_BYTES;
  }
  function validUnit(value, change) {
    return typeof value === "string" && validChange(change) && value.startsWith(`${change}/`) && value.length > change.length + 1
      && !CONTROL.test(value) && utf8Bytes(value) <= UNIT_MAX_BYTES;
  }
  // r[impl kiln.cairn.workspace.scenario.reload] The URL's selection, unit, view and filters. A repository and change are
  // selected together or not at all, and a unit only within its own change.
  function normalizeRoute(route) {
    const value = (key) => (typeof route?.[key] === "string" ? route[key] : "");
    const repository = value("cairn_repository");
    const change = value("cairn_change");
    const selected = HEX.test(repository) && validChange(change);
    const unit = value("cairn_unit");
    const unitValid = selected && validUnit(unit, change);
    return Object.freeze({
      repository: selected ? repository : "",
      change: selected ? change : "",
      unit: unitValid ? unit : "",
      q: value("cairn_q").slice(0, FILTER_MAX_CHARS),
      lane: LANES.has(value("cairn_lane")) ? value("cairn_lane") : "",
      blocked: value("cairn_blocked") === "1",
      decisions: value("cairn_decisions") === "1",
      view: value("cairn_view") === "board" ? "board" : "list",
      invalid: Boolean(repository || change) && !selected,
      invalidUnit: Boolean(unit) && !unitValid,
    });
  }
  function filterPatch(filters) {
    return { cairn_q: filters.q, cairn_lane: filters.lane, cairn_blocked: filters.blocked ? "1" : "", cairn_decisions: filters.decisions ? "1" : "", cairn_view: filters.view === "board" ? "board" : "" };
  }
  function detailPath(route) {
    return `${DETAIL_ROUTE}?repository=${route.repository}&change=${encodeURIComponent(route.change)}`;
  }
  const count = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : null);
  const strings = (value) => (Array.isArray(value) ? value.filter((item) => typeof item === "string") : []);
  const list = (value) => (Array.isArray(value) ? value : []);
  function compareText(a, b) { return a < b ? -1 : a > b ? 1 : 0; }
  // Unit counts per known lane; an absent or malformed count is unknown, not zero.
  function laneCounts(value) {
    const counts = new Map();
    for (const key of LANE_ORDER) counts.set(key, value && typeof value === "object" ? count(value[key]) : null);
    return counts;
  }
  function blockerOf(value) {
    if (!value || typeof value !== "object" || typeof value.condition !== "string" || typeof value.subject !== "string") return null;
    return value;
  }
  // r[impl kiln.cairn.workspace.scenario.register] Register rows: needs attention first, then by lane, then the most
  // blockers and open decisions, then by change and repository, so the order never depends on arrival. A repeated or
  // malformed key is not listed twice or guessed.
  function indexRows(index) {
    if (index?.schema !== INDEX_SCHEMA || !Array.isArray(index.changes)) throw new Error(`The Cairn change index is not a ${INDEX_SCHEMA} document.`);
    const rows = [];
    const seen = new Set();
    for (const entry of index.changes) {
      if (!HEX.test(entry?.repository_identity) || !validChange(entry?.change)) continue;
      const key = `${entry.repository_identity}/${entry.change}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const projects = strings(entry.projects);
      const campaigns = strings(entry.campaigns);
      rows.push(Object.freeze({
        key, repository: entry.repository_identity, change: entry.change, projects, campaigns,
        units: count(entry.units), currentWork: count(entry.current_work), attempts: count(entry.attempts),
        activeAttempts: count(entry.active_attempts), blockers: count(entry.blockers), openDecisions: count(entry.open_decisions),
        lane: typeof entry.lane === "string" ? entry.lane : "", lanes: laneCounts(entry.lanes), firstBlocker: blockerOf(entry.first_blocker),
        search: [entry.change, entry.repository_identity, ...projects, ...campaigns].join("\n").toLowerCase(),
      }));
    }
    const most = (value) => (value === null ? Infinity : value);
    rows.sort((a, b) => laneRank(a.lane) - laneRank(b.lane) || most(b.blockers) - most(a.blockers) || most(b.openDecisions) - most(a.openDecisions)
      || compareText(a.change, b.change) || compareText(a.repository, b.repository));
    return rows;
  }
  // An unknown count is never read as zero: it stays in the blocker and decision filters.
  function matches(row, filters = {}) {
    const needle = typeof filters.q === "string" ? filters.q.trim().toLowerCase() : "";
    if (needle && !row.search.includes(needle)) return false;
    if (filters.lane && boardLane(row.lane) !== filters.lane) return false;
    if (filters.blocked && row.blockers === 0) return false;
    if (filters.decisions && row.openDecisions === 0) return false;
    return true;
  }
  // Each chip counts the rows it would show with the other filters kept.
  function filterCounts(rows, filters = {}) {
    const lanes = new Map(LANE_ORDER.map((key) => [key, 0]));
    let all = 0;
    let blocked = 0;
    let decisions = 0;
    for (const row of rows) {
      if (matches(row, { ...filters, lane: "" })) { all += 1; lanes.set(boardLane(row.lane), lanes.get(boardLane(row.lane)) + 1); }
      if (matches(row, { ...filters, blocked: true })) blocked += 1;
      if (matches(row, { ...filters, decisions: true })) decisions += 1;
    }
    return { all, lanes, blocked, decisions };
  }
  // Previous and next follow the filtered register. A change the filters hide steps through every change instead.
  function neighbors(rows, filters, key) {
    const visible = rows.filter((row) => matches(row, filters));
    let order = visible;
    let index = order.findIndex((row) => row.key === key);
    const filtered = index >= 0;
    if (!filtered) { order = rows; index = rows.findIndex((row) => row.key === key); }
    if (index < 0) return { previous: null, next: null, position: 0, total: visible.length, filtered: false, listed: false };
    return { previous: order[index - 1] || null, next: order[index + 1] || null, position: index + 1, total: order.length, filtered, listed: true };
  }
  // r[impl kiln.cairn.workspace.scenario.reload] A detail is shown only when it echoes the exact requested repository and
  // change; anything else is refused, never substituted.
  function admitDetail(value, route) {
    if (value?.schema !== DETAIL_SCHEMA || !HEX.test(value.identity) || value.repository_identity !== route.repository
      || value.change !== route.change || !Array.isArray(value.units) || !Array.isArray(value.memberships)
      || !Array.isArray(value.decisions) || !Array.isArray(value.blockers) || typeof value.overview !== "object" || value.overview === null
      || !["units", "current_work", "attempts", "active_attempts", "campaigns", "blockers", "open_decisions"].every((key) => count(value.overview[key]) !== null)) {
      throw new Error("The service returned a different or malformed Cairn change. No other change was substituted.");
    }
    return value;
  }
  // r[impl kiln.cairn.workspace.scenario.dependencies] The dependency graph of one change: its units, the prerequisites
  // and dependents they name (in this or another change, or work that is not current), layered by dependency depth.
  // Nodes that sit in or after a cycle go to a final column instead of looping.
  function dependencyGraph(detail) {
    const change = detail?.change;
    const repository = detail?.repository_identity;
    const nodes = new Map();
    const edges = new Map();
    // A unit node is keyed by repository and native identity, so the same change name in another repository stays apart.
    const unitNode = (nativeId, owningRepository, unit) => {
      const id = `unit:${owningRepository || "?"}/${nativeId}`;
      if (!nodes.has(id)) {
        const slash = nativeId.indexOf("/");
        const owner = slash > 0 ? nativeId.slice(0, slash) : "";
        const own = owner === change && owningRepository === repository;
        nodes.set(id, { id, kind: own ? "unit" : "external", nativeId, change: owner, repository: owningRepository || "", label: own ? nativeId.slice(slash + 1) : nativeId, lane: unit?.lane ?? "", state: "current" });
      }
      return id;
    };
    const workNode = (identity, state) => {
      const id = `work:${identity}`;
      if (!nodes.has(id)) nodes.set(id, { id, kind: "work", identity, label: short(identity), lane: "", state });
      return id;
    };
    const edge = (from, to) => { edges.set(`${from}\n${to}`, { from, to }); };
    // Each named unit of the view, or the work identity of a view that names none.
    const viewNodes = (view, fallbackState) => {
      const ids = strings(view?.native_ids);
      if (ids.length) return ids.map((nativeId) => unitNode(nativeId, typeof view.repository_identity === "string" ? view.repository_identity : ""));
      return typeof view?.identity === "string" ? [workNode(view.identity, view.state === "current" ? "current" : fallbackState)] : [];
    };
    const units = list(detail?.units).filter((unit) => typeof unit?.native_id === "string");
    for (const unit of units) unitNode(unit.native_id, repository, unit);
    for (const unit of units) {
      const self = unitNode(unit.native_id, repository, unit);
      for (const work of list(unit.current_work)) {
        for (const dependency of list(work?.dependencies)) for (const id of viewNodes(dependency, "not_current")) edge(id, self);
        for (const dependent of list(work?.dependents)) for (const id of viewNodes(dependent, "current")) edge(self, id);
      }
    }
    const incoming = new Map([...nodes.keys()].map((id) => [id, 0]));
    const outgoing = new Map([...nodes.keys()].map((id) => [id, []]));
    for (const { from, to } of edges.values()) { incoming.set(to, incoming.get(to) + 1); outgoing.get(from).push(to); }
    const depth = new Map();
    const queue = [...nodes.keys()].filter((id) => incoming.get(id) === 0).sort(compareText);
    for (const id of queue) depth.set(id, 0);
    for (let head = 0; head < queue.length; head += 1) {
      const id = queue[head];
      for (const next of outgoing.get(id).sort(compareText)) {
        depth.set(next, Math.max(depth.get(next) ?? 0, depth.get(id) + 1));
        incoming.set(next, incoming.get(next) - 1);
        if (incoming.get(next) === 0) queue.push(next);
      }
    }
    const placed = new Set(queue);
    const deepest = Math.max(-1, ...queue.map((id) => depth.get(id)));
    const kindRank = { unit: 0, external: 1, work: 2 };
    const columns = [];
    const result = [];
    for (const node of nodes.values()) {
      const cycle = !placed.has(node.id);
      const level = cycle ? deepest + 1 : depth.get(node.id);
      const placedNode = Object.freeze({ ...node, depth: level, cycle });
      result.push(placedNode);
      (columns[level] ||= []).push(placedNode);
    }
    for (const column of columns) column?.sort((a, b) => kindRank[a.kind] - kindRank[b.kind] || compareText(a.label, b.label));
    return Object.freeze({
      nodes: result, edges: [...edges.values()], columns: columns.filter(Boolean).map((column) => column.map((node) => node.id)),
      cyclic: result.some((node) => node.cycle),
    });
  }

  // ---------- Presentation ----------

  const ui = () => window.KilnUi;
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined && content !== null) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  function plural(value, singular, many = `${singular}s`) { return `${value} ${value === 1 ? singular : many}`; }
  function quantity(value, singular, many) { return value === null ? `unknown ${many || `${singular}s`}` : plural(value, singular, many); }
  function short(value) { const source = String(value ?? ""); return source.length > SHORT_HASH + 1 ? `${source.slice(0, SHORT_HASH)}…` : source; }
  function isAborted(error) { return error?.name === "AbortError"; }
  function errorCode(error) { return /^([a-z_]+):/.exec(typeof error?.message === "string" ? error.message : "")?.[1] || ""; }
  function explain(error) {
    if (error?.name === "TimeoutError") return "The request timed out; its result is not known.";
    if (error instanceof TypeError) return "The service could not be reached.";
    const message = typeof error?.message === "string" ? error.message.trim() : "";
    return message ? (/[.!?]$/.test(message) ? message : `${message}.`) : "The request failed.";
  }
  function plainClick(event) { return event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey; }
  function routeHref(patch) {
    const params = new URLSearchParams(location.search);
    for (const [key, value] of Object.entries(patch)) { if (value) params.set(key, value); else params.delete(key); }
    return `?${params}`;
  }
  function link(patch, label, className, onPlain) {
    const node = element("a", label, className);
    node.href = routeHref(patch);
    node.addEventListener("click", (event) => {
      if (!plainClick(event)) return;
      event.preventDefault();
      if (onPlain) onPlain(); else navigate(patch);
    });
    return node;
  }
  function campaignLink(campaignId, label = campaignId) {
    const node = link({ page: "workers", worker_campaign: campaignId }, label, "cairn-campaign-link");
    node.title = `Open campaign ${campaignId}: its progress, controls and decision inbox`;
    return node;
  }
  function runAnchor(runId, label) {
    if (typeof context?.runLink === "function") return context.runLink(runId, label);
    const node = element("a", label, "row-link");
    node.href = `?${new URLSearchParams({ page: "runs", run: runId, view: "pipeline" })}`;
    return node;
  }
  function badge(entry, prefix) {
    const node = element("span", prefix ? `${prefix}: ${entry.label}` : entry.label, "state-badge");
    if (entry.tone) node.dataset.tone = entry.tone;
    if (entry.meaning) node.title = entry.meaning;
    return node;
  }
  function badges(...nodes) {
    const node = element("div", undefined, "badges");
    node.append(...nodes.filter(Boolean));
    return node;
  }
  function code(value) { return typeof value === "string" && value ? element("code", value) : element("span", "Not recorded", "not-recorded"); }
  // An identity shortened where space is tight; the exact value stays in its title.
  function shortCode(value) {
    if (typeof value !== "string" || !value) return element("span", "Not recorded", "not-recorded");
    const node = element("code", short(value), "cairn-short");
    node.title = value;
    return node;
  }
  function cell(value) {
    if (value === null || value === undefined || value === "") return element("span", "Not recorded", "not-recorded");
    return typeof value === "object" ? value : element("span", String(value));
  }
  function facts(rows, className = "facts") {
    const node = element("dl", undefined, className);
    for (const [label, value] of rows) {
      if (value === undefined) continue;
      const definition = element("dd");
      definition.append(cell(value));
      node.append(element("dt", label), definition);
    }
    return node;
  }
  function bullets(items, empty, className = "cairn-bullets") {
    if (!items.length) return element("span", empty, "small");
    const node = element("ul", undefined, className);
    for (const item of items) node.append(typeof item === "string" ? element("li", item) : item);
    return node;
  }
  function disclosure(summary, key, ...children) {
    const node = element("details", undefined, "cairn-disclosure");
    node.dataset.cairnDisclosure = key;
    node.append(element("summary", summary), ...children.filter(Boolean));
    return node;
  }
  function copyable(value, what) {
    const wrap = element("span", undefined, "cairn-copy");
    wrap.append(code(value));
    if (typeof value === "string" && value && typeof ui()?.copyText === "function") {
      const button = element("button", "Copy", "quiet cairn-copy-button");
      button.type = "button";
      button.setAttribute("aria-label", `Copy ${what}`);
      const status = element("span", undefined, "small");
      status.setAttribute("role", "status");
      button.addEventListener("click", () => ui().copyText(value, status, `${what} copied.`));
      wrap.append(button, status);
    }
    return wrap;
  }
  function when(ms) {
    if (!Number.isSafeInteger(ms) || ms < 0) return element("span", "Not recorded", "not-recorded");
    if (typeof ui()?.timeElement === "function") return ui().timeElement(ms);
    return element("time", new Date(ms).toISOString());
  }
  function section(title, id, ...children) {
    const node = element("section", undefined, "cairn-section");
    const heading = element("h3", title, "cairn-section-title");
    heading.id = id;
    node.setAttribute("aria-labelledby", id);
    node.append(heading, ...children.filter(Boolean));
    return node;
  }
  function emptyState(title, message) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    return node;
  }
  function errorState(message) {
    const node = element("p", message, "warning error-state");
    node.dataset.state = "unavailable";
    node.setAttribute("role", "alert");
    return node;
  }
  function loadingState() {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    return node;
  }
  // How a change's units spread across the lanes, as a proportional bar with the counts in words.
  function laneBar(lanes) {
    const wrap = element("div", undefined, "cairn-lanebar");
    const track = element("div", undefined, "cairn-lanebar-track");
    const words = [];
    let unknown = false;
    for (const [key, entry] of LANES) {
      const value = lanes.get(key);
      if (value === null) { unknown = true; continue; }
      if (!value) continue;
      const segment = element("span", undefined, "cairn-lanebar-seg");
      segment.dataset.lane = key;
      segment.style.flexGrow = String(value);
      segment.title = `${entry.label}: ${value}`;
      track.append(segment);
      words.push(`${value} ${entry.label.toLowerCase()}`);
    }
    const text = words.length ? words.join(" · ") : unknown ? "Unit lanes not recorded" : "No unit named";
    track.setAttribute("role", "img");
    track.setAttribute("aria-label", `Units by lane: ${text}`);
    if (!words.length) track.dataset.empty = "true";
    wrap.append(track, element("span", text, "cairn-lanebar-text"));
    return wrap;
  }

  // ---------- State ----------

  let generation = 0;
  let mounted = null;
  let context = null;
  let filterTimer = null;
  // The data the mounted page last drew, the route it drew, and a route this page applied itself.
  let model = null;
  let shown = null;
  let expected = null;
  // The last decision command outcome, shown once after the reload it caused.
  let notice = "";
  const LOCAL_KEYS = ["unit", "q", "lane", "blocked", "decisions", "view"];
  function sameSelection(a, b) { return Boolean(a && b) && a.repository === b.repository && a.change === b.change; }
  function sameRoute(a, b) { return sameSelection(a, b) && LOCAL_KEYS.every((key) => a[key] === b[key]); }
  function navigate(patch) {
    clearTimeout(filterTimer);
    if (context) context.navigate(patch);
  }
  // A change drawn here already: record the route the page now shows and write it to the URL without a read or redraw.
  function commitLocal(patch) {
    clearTimeout(filterTimer);
    if (!context) return;
    expected = normalizeRoute({ ...context.route, ...patch });
    context.navigate(patch);
  }

  // ---------- Register ----------

  function nextStepCell(row) {
    const node = element("div", undefined, "cairn-next-cell");
    if (row.blockers === null) { node.append(badge(word("Blockers unknown", "warning", "The blocker count was not recorded; nothing is inferred."))); return node; }
    if (!row.blockers || !row.firstBlocker) { node.append(element("span", "No blockers", "small")); return node; }
    const first = row.firstBlocker;
    node.append(badge(condition(first.condition)), element("code", first.subject, "cairn-subject"));
    const next = element("span", `${lookup(OPERATIONS, typeof first.operation === "string" ? first.operation : "").label} · ${typeof first.owner === "string" && first.owner ? first.owner : "owner not named"}`, "small");
    node.append(next);
    if (row.blockers > 1) node.append(element("span", `+${row.blockers - 1} more`, "small cairn-more"));
    return node;
  }
  function decisionsCell(row) {
    if (row.openDecisions === null) return badge(word("Decisions unknown", "warning"));
    if (!row.openDecisions) return element("span", "None open", "small");
    return badge(word(`${plural(row.openDecisions, "open decision")}`, "warning", "Decisions of this change's campaign members that are unissued or awaiting an answer."));
  }
  function changeAnchor(row, route, applied) {
    const anchor = link({ cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "", ...filterPatch(applied) }, row.change, "row-link cairn-change-link",
      () => navigate({ cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "", ...filterPatch(applied) }));
    anchor.dataset.cairnKey = row.key;
    anchor.dataset.focusKey = `change:${row.key}`;
    if (row.repository === route.repository && row.change === route.change) anchor.setAttribute("aria-current", "true");
    return anchor;
  }
  function changeMeta(row) {
    const meta = element("p", undefined, "cairn-row-meta");
    meta.append(element("span", row.projects.length ? row.projects.join(", ") : "No canonical project"), shortCode(row.repository));
    return meta;
  }
  function listRow(row, route, applied) {
    const tr = element("tr");
    tr.dataset.cairnKey = row.key;
    tr.dataset.lane = boardLane(row.lane);
    const change = element("td", undefined, "cairn-change-cell");
    change.dataset.label = "Change";
    change.append(changeAnchor(row, route, applied), changeMeta(row));
    const stage = element("td");
    stage.dataset.label = "Stage";
    stage.append(badge(lane(row.lane)), laneBar(row.lanes));
    const next = element("td");
    next.dataset.label = "Next step";
    next.append(nextStepCell(row));
    const decisions = element("td");
    decisions.dataset.label = "Decisions";
    decisions.append(decisionsCell(row));
    const attempts = element("td");
    attempts.dataset.label = "Attempts";
    attempts.append(element("span", row.attempts === null ? "Not recorded" : `${row.attempts} · ${row.activeAttempts ? `${row.activeAttempts} active` : "none active"}`, "small"));
    tr.append(change, stage, next, decisions, attempts);
    return tr;
  }
  function boardCard(row, route, applied) {
    const card = element("article", undefined, "cairn-card");
    card.dataset.cairnKey = row.key;
    card.dataset.lane = boardLane(row.lane);
    const head = element("h3", undefined, "cairn-card-title");
    head.append(changeAnchor(row, route, applied));
    card.append(head, changeMeta(row));
    if (!LANES.has(row.lane)) card.append(badge(lane(row.lane)));
    card.append(laneBar(row.lanes), nextStepCell(row));
    const foot = element("p", undefined, "cairn-card-foot");
    foot.append(decisionsCell(row), element("span", row.attempts === null ? "Attempts not recorded" : `${plural(row.attempts, "attempt")}${row.activeAttempts ? `, ${row.activeAttempts} active` : ""}`, "small"));
    card.append(foot);
    return card;
  }
  function chip(label, value, pressed, title) {
    const button = element("button", undefined, "chip cairn-chip");
    button.type = "button";
    button.setAttribute("aria-pressed", String(pressed));
    if (title) button.title = title;
    button.append(element("span", label), element("span", "", "cairn-chip-count"));
    button.dataset.value = value;
    return button;
  }
  // Arrow keys move between visible change links: up and down in the list, and across columns on the board.
  function registerKeys(body) {
    body.addEventListener("keydown", (event) => {
      if (!["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].includes(event.key)) return;
      const active = document.activeElement;
      if (!active?.classList?.contains("cairn-change-link")) return;
      const columns = [...body.querySelectorAll(".cairn-board:not([hidden]) .cairn-board-column")];
      let target = null;
      if (columns.length) {
        const visibleLinks = (column) => [...column.querySelectorAll(".cairn-card:not([hidden]) .cairn-change-link")];
        const columnIndex = columns.findIndex((column) => column.contains(active));
        const own = visibleLinks(columns[columnIndex]);
        const index = own.indexOf(active);
        if (event.key === "ArrowDown" || event.key === "ArrowUp") target = own[index + (event.key === "ArrowDown" ? 1 : -1)];
        else {
          const step = event.key === "ArrowRight" ? 1 : -1;
          for (let next = columnIndex + step; next >= 0 && next < columns.length && !target; next += step) {
            const candidates = visibleLinks(columns[next]);
            target = candidates[Math.min(index, candidates.length - 1)];
          }
        }
      } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        const links = [...body.querySelectorAll("tr:not([hidden]) .cairn-change-link")];
        target = links[links.indexOf(active) + (event.key === "ArrowDown" ? 1 : -1)];
      }
      if (target) { event.preventDefault(); target.focus(); }
    });
  }
  // r[impl kiln.cairn.workspace.scenario.register] The register: one row set read as a list or a board, narrowed at once
  // by text, lane, blockers and open decisions, with counts, and committed to the URL without another read.
  function registerPanel(route, rows, index, embedded) {
    const panel = element("section", undefined, "panel cairn-register");
    const titleId = embedded ? "cairn-browse-title" : "cairn-register-title";
    panel.setAttribute("aria-labelledby", titleId);
    const head = element("div", undefined, "cairn-register-head");
    const heading = element("h2", "Changes");
    heading.id = titleId;
    const intro = element("div");
    intro.append(heading, element("p", "Each Cairn change named by canonical work or a selected campaign member, keyed by repository identity and change name. A lane is where the change's least-advanced unit stands; a stop anywhere needs attention.", "small"));
    head.append(intro);
    panel.append(head);
    if (index.error) {
      if (errorCode(index.error) === "cairn_workspace_input_not_configured") {
        panel.append(emptyState("No implementation work database is configured", "An operator supplies one with --implementation-work. This is not proof that no Cairn change exists."));
        return panel;
      }
      const retry = element("button", "Try again");
      retry.type = "button";
      retry.addEventListener("click", () => { if (mounted && context) void load(mounted, context, { force: true }); });
      panel.append(errorState(`Cairn changes unavailable: ${explain(index.error)}${route.change ? " The selected change is still read on its own." : ""}`), retry);
      return panel;
    }
    if (!rows.length) {
      panel.append(emptyState("No Cairn change is named yet", "Canonical work with a Cairn alias, or a selected campaign member, names a change. None is recorded in this store."));
      return panel;
    }
    const applied = { q: route.q, lane: route.lane, blocked: route.blocked, decisions: route.decisions, view: route.view };
    const views = element("div", undefined, "segmented cairn-views");
    views.setAttribute("role", "group");
    views.setAttribute("aria-label", "Register layout");
    const viewButtons = [["list", "List"], ["board", "Board"]].map(([value, label]) => {
      const button = element("button", label);
      button.type = "button";
      button.dataset.view = value;
      button.setAttribute("aria-pressed", String(applied.view === value));
      views.append(button);
      return button;
    });
    head.append(views);
    const form = element("form", undefined, "cairn-search");
    form.setAttribute("role", "search");
    const label = element("label", "Search changes");
    const search = element("input");
    const searchId = embedded ? "cairn-browse-q" : "cairn-q";
    Object.assign(search, { id: searchId, type: "search", maxLength: FILTER_MAX_CHARS, autocomplete: "off", placeholder: "Change, project, campaign or repository identity", value: applied.q });
    search.dataset.focusKey = `search:${embedded ? "browse" : "register"}`;
    label.htmlFor = searchId;
    form.append(label, search);
    const lanesGroup = element("div", undefined, "cairn-chips");
    lanesGroup.setAttribute("role", "group");
    lanesGroup.setAttribute("aria-label", "Lane");
    const allChip = chip("All lanes", "", !applied.lane);
    const laneChips = [allChip, ...[...LANES].map(([key, entry]) => { const node = chip(entry.label, key, applied.lane === key, entry.meaning); node.dataset.lane = key; return node; })];
    lanesGroup.append(...laneChips);
    const flags = element("div", undefined, "cairn-chips cairn-flags");
    flags.setAttribute("role", "group");
    flags.setAttribute("aria-label", "Next step");
    const blockedChip = chip("Has blockers", "blocked", applied.blocked, "Changes with at least one structured blocker, or an unknown count.");
    const decisionsChip = chip("Has open decisions", "decisions", applied.decisions, "Changes whose campaign members have decisions waiting to be issued or answered.");
    flags.append(blockedChip, decisionsChip);
    const tools = element("div", undefined, "cairn-tools");
    const status = element("p", undefined, "small cairn-count");
    status.setAttribute("role", "status");
    const clearButton = element("button", "Clear filters", "quiet");
    clearButton.type = "button";
    tools.append(status, clearButton);

    const body = element("div", undefined, "cairn-register-body");
    const tableWrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack cairn-table");
    table.append(element("caption", "Cairn changes, needs attention first, then by lane, blockers and open decisions.", "sr-only"));
    const thead = element("thead");
    const headRow = element("tr");
    for (const title of ["Change", "Stage", "Next step", "Decisions", "Attempts"]) { const th = element("th", title); th.scope = "col"; headRow.append(th); }
    thead.append(headRow);
    const tbody = element("tbody");
    table.append(thead, tbody);
    tableWrap.append(table);
    const board = element("div", undefined, "cairn-board");
    const columns = new Map();
    for (const [key, entry] of LANES) {
      const column = element("section", undefined, "cairn-board-column");
      column.dataset.lane = key;
      const columnTitle = element("h3", undefined, "cairn-board-title");
      columnTitle.append(element("span", entry.label), element("span", "", "section-count"));
      columnTitle.title = entry.meaning;
      const cards = element("div", undefined, "cairn-board-cards");
      const none = element("p", "None", "small cairn-board-empty");
      column.append(columnTitle, cards, none);
      board.append(column);
      columns.set(key, { column, cards, none, count: columnTitle.lastChild });
    }
    const entries = rows.map((row) => {
      const tr = listRow(row, route, applied);
      tbody.append(tr);
      const card = boardCard(row, route, applied);
      columns.get(boardLane(row.lane)).cards.append(card);
      return { row, tr, card };
    });
    body.append(tableWrap, board);
    registerKeys(body);
    const noMatch = emptyState("No change matches these filters", "Try another search or lane, or clear the filters. A selected change is not replaced.");
    noMatch.classList.add("cairn-no-match");
    const apply = () => {
      const filters = { q: search.value.slice(0, FILTER_MAX_CHARS), lane: applied.lane, blocked: applied.blocked, decisions: applied.decisions };
      applied.q = filters.q;
      let visible = 0;
      const perLane = new Map(LANE_ORDER.map((key) => [key, 0]));
      for (const { row, tr, card } of entries) {
        const match = matches(row, filters);
        tr.hidden = !match;
        card.hidden = !match;
        if (match) { visible += 1; perLane.set(boardLane(row.lane), perLane.get(boardLane(row.lane)) + 1); }
        const href = routeHref({ cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "", ...filterPatch(applied) });
        for (const anchor of [tr, card].map((node) => node.querySelector(".cairn-change-link"))) anchor.href = href;
      }
      for (const [key, entry] of columns) { entry.count.textContent = String(perLane.get(key)); entry.none.hidden = perLane.get(key) > 0; }
      const counts = filterCounts(rows, filters);
      allChip.lastChild.textContent = String(counts.all);
      for (const node of laneChips.slice(1)) { node.lastChild.textContent = String(counts.lanes.get(node.dataset.value)); node.setAttribute("aria-pressed", String(applied.lane === node.dataset.value)); }
      allChip.setAttribute("aria-pressed", String(!applied.lane));
      blockedChip.lastChild.textContent = String(counts.blocked);
      blockedChip.setAttribute("aria-pressed", String(applied.blocked));
      decisionsChip.lastChild.textContent = String(counts.decisions);
      decisionsChip.setAttribute("aria-pressed", String(applied.decisions));
      for (const button of viewButtons) button.setAttribute("aria-pressed", String(applied.view === button.dataset.view));
      tableWrap.hidden = applied.view !== "list" || !visible;
      board.hidden = applied.view !== "board" || !visible;
      const filtered = Boolean(filters.q.trim() || filters.lane || filters.blocked || filters.decisions);
      status.textContent = filtered ? `${visible} of ${plural(rows.length, "change")}` : `${plural(rows.length, "change")}, needs attention first`;
      clearButton.hidden = !filtered;
      noMatch.hidden = Boolean(visible);
    };
    const commit = () => commitLocal(filterPatch(applied));
    search.addEventListener("input", () => {
      apply();
      clearTimeout(filterTimer);
      filterTimer = setTimeout(() => { if (search.isConnected) commit(); }, FILTER_COMMIT_MS);
    });
    form.addEventListener("submit", (event) => { event.preventDefault(); apply(); commit(); });
    for (const node of laneChips) node.addEventListener("click", () => { applied.lane = node.dataset.value === applied.lane ? "" : node.dataset.value; apply(); commit(); });
    blockedChip.addEventListener("click", () => { applied.blocked = !applied.blocked; apply(); commit(); });
    decisionsChip.addEventListener("click", () => { applied.decisions = !applied.decisions; apply(); commit(); });
    for (const button of viewButtons) button.addEventListener("click", () => { applied.view = button.dataset.view; apply(); commit(); });
    clearButton.addEventListener("click", () => {
      search.value = "";
      Object.assign(applied, { q: "", lane: "", blocked: false, decisions: false });
      apply();
      commit();
      search.focus();
    });
    for (const node of [...laneChips, blockedChip, decisionsChip, clearButton, ...viewButtons]) node.dataset.focusKey = `control:${embedded ? "browse" : "register"}:${node.dataset.value ?? node.dataset.view ?? node.textContent}`;
    panel.append(form, lanesGroup, flags, tools, body, noMatch);
    if (index.value?.truncated === true) panel.append(element("p", "Only the first changes are listed; the index reached its bound.", "warning"));
    apply();
    return panel;
  }

  // ---------- Change ----------

  function readout(label, value) {
    const node = element("div", undefined, "side-metric");
    node.append(element("dt", label), element("dd", value));
    return node;
  }
  function changeHeader(detail) {
    const head = element("header", undefined, "cairn-head");
    const titleNode = element("h2", detail.change, "cairn-title");
    titleNode.tabIndex = -1;
    titleNode.dataset.focusKey = "title";
    const overview = detail.overview;
    const metrics = element("dl", undefined, "side-metrics cairn-readouts");
    metrics.append(
      readout("Units", String(overview.units)),
      readout("Attempts", overview.active_attempts ? `${overview.attempts} · ${overview.active_attempts} active` : String(overview.attempts)),
      readout("Campaigns", String(overview.campaigns)),
      readout("Blockers", String(overview.blockers)),
      readout("Open decisions", String(overview.open_decisions)),
    );
    head.append(
      element("p", strings(detail.projects).join(", ") || "No canonical project", "cairn-eyebrow"),
      titleNode,
      badges(badge(lane(overview.lane), "Lane")),
      laneBar(laneCounts(overview.lanes)),
      metrics,
    );
    const snapshot = detail.snapshot || {};
    if (snapshot.campaigns_truncated || snapshot.lifecycle_truncated) head.append(element("p", "The snapshot reached a read bound; campaigns or lifecycle receipts beyond it are not shown.", "warning"));
    if (snapshot.work_recorded === false) head.append(element("p", "No canonical work is recorded in this store. Units come from campaign selections only.", "small"));
    return head;
  }
  function changeNav(route, rows) {
    const nav = element("nav", undefined, "cairn-change-nav");
    nav.setAttribute("aria-label", "Change navigation");
    const back = link({ cairn_repository: "", cairn_change: "", cairn_unit: "" }, "All changes", "button cairn-back");
    back.dataset.focusKey = "back";
    if (typeof ui()?.icon === "function") back.prepend(ui().icon("arrow-left"));
    nav.append(back);
    const filters = { q: route.q, lane: route.lane, blocked: route.blocked, decisions: route.decisions };
    const around = neighbors(rows, filters, `${route.repository}/${route.change}`);
    if (around.listed) {
      const step = (row, rel, text) => {
        if (!row) { const idle = element("span", text, "button cairn-step"); idle.setAttribute("aria-disabled", "true"); return idle; }
        const node = link({ cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "" }, `${text}: ${row.change}`, "button cairn-step");
        node.rel = rel;
        node.dataset.focusKey = `step:${rel}`;
        return node;
      };
      nav.append(step(around.previous, "prev", "Previous"), element("span", `${around.position} of ${around.total}${around.filtered ? " filtered" : ""}`, "small cairn-position"), step(around.next, "next", "Next"));
      if (!around.filtered && (filters.q || filters.lane || filters.blocked || filters.decisions)) nav.append(element("p", "This change is outside the current filters; previous and next step through every change.", "small"));
    }
    return nav;
  }
  // Focus and reveal a campaign member card inside the change, matched by its exact member key and campaign.
  function revealMember(member, campaignId) {
    const target = [...(mounted?.querySelectorAll(".cairn-member") || [])].find((node) => node.dataset.member === member && node.dataset.campaign === campaignId);
    if (!target) return;
    target.scrollIntoView?.({ block: "center" });
    target.focus?.({ preventScroll: true });
  }
  function openUnit(nativeId) {
    const toggle = [...(mounted?.querySelectorAll(".cairn-unit-toggle") || [])].find((node) => node.dataset.unit === nativeId);
    if (!toggle) return;
    if (toggle.getAttribute("aria-expanded") !== "true") toggle.click();
    toggle.scrollIntoView?.({ block: "center" });
    toggle.focus({ preventScroll: true });
  }
  // r[impl kiln.cairn.workspace.scenario.blockers] Each blocker in the reader's order: what stops, which subject, the
  // owner's operation, the display-only detail and where it is resolved. Links come from structured fields only.
  function blockerItem(blocker, detail) {
    const item = element("li", undefined, "cairn-blocker");
    const kind = condition(blocker.condition);
    item.dataset.tone = kind.tone;
    const head = element("div", undefined, "cairn-blocker-head");
    head.append(badge(kind));
    if (blocker.subject_kind === "unit" && detail.units.some((unit) => unit.native_id === blocker.subject)) {
      head.append(link({ cairn_unit: blocker.subject }, blocker.subject, "cairn-subject", () => { openUnit(blocker.subject); }));
    } else if (blocker.subject_kind === "member") {
      const show = element("button", blocker.subject, "quiet cairn-subject");
      show.type = "button";
      show.title = "Show this campaign member";
      show.addEventListener("click", () => revealMember(blocker.subject, blocker.campaign_id || ""));
      head.append(show);
    } else head.append(element("code", blocker.subject, "cairn-subject"));
    item.append(head);
    if (kind.meaning) item.append(element("p", kind.meaning, "cairn-blocker-meaning"));
    const next = element("p", undefined, "cairn-blocker-next");
    next.append(element("span", "Next: ", "kiln-label"), element("strong", lookup(OPERATIONS, blocker.operation).label), document.createTextNode(" · owner "), code(blocker.owner));
    if (typeof blocker.campaign_id === "string" && blocker.campaign_id) next.append(document.createTextNode(" · "), campaignLink(blocker.campaign_id, `Open campaign ${blocker.campaign_id}`));
    item.append(next);
    const recorded = element("p", undefined, "small cairn-blocker-detail");
    recorded.append(element("span", "Recorded detail (display only): "), element("q", typeof blocker.detail === "string" ? blocker.detail : ""));
    item.append(recorded);
    const observation = element("p", undefined, "small");
    observation.append(element("span", "Observation "), shortCode(blocker.observation));
    item.append(observation);
    return item;
  }
  function isOpenDecision(decision) { return decision?.state === "unissued" || decision?.state === "outstanding"; }
  function decisionCard(decision) {
    const card = element("article", undefined, "cairn-decision");
    card.dataset.decision = decision.identity;
    card.append(badges(badge(lookup(DECISION_KINDS, decision.category)), badge(lookup(DECISION_STATES, decision.state)), decision.scope === "current" ? null : badge(lookup(SCOPES, decision.scope))));
    card.append(element("h4", decision.question, "cairn-decision-question"));
    const about = element("p", undefined, "small cairn-decision-about");
    about.append(element("span", "Member "), code(decision.member), element("span", " · campaign "), campaignLink(decision.campaign_id));
    if (typeof decision.recommendation === "string" && decision.recommendation) about.append(element("span", " · recommended "), element("strong", decision.recommendation));
    card.append(about);
    if (typeof decision.pending_reason === "string" && decision.pending_reason) card.append(element("p", `Queue note: ${decision.pending_reason}`, "small"));
    const answer = element("div", undefined, "cairn-decision-answer");
    answer.append(element("p", `Options: ${strings(decision.options).join(", ") || "none supplied"}.`, "small"));
    card.append(answer);
    card.append(disclosure("Decision facts", `decision:${decision.identity}`, facts([
      ["Options", strings(decision.options).join(", ") || "None supplied"],
      ["Recommendation", decision.recommendation ?? "None supplied"],
      ["Scope", lookup(SCOPES, decision.scope).meaning || decision.scope],
      ["Bound work", decision.work_identity ? shortCode(decision.work_identity) : "No work binding"],
      ["Priority", String(decision.priority ?? "Not recorded")],
      ["Resolved answer", decision.resolved_answer ?? "Not resolved"],
      ["Decision identity", copyable(decision.identity, "Decision identity")],
    ])));
    return { card, answer, decision };
  }
  function readOnlyAnswer(reason, campaignId) {
    const node = element("p", undefined, "small cairn-answer-note");
    node.append(element("span", `${reason} `), campaignLink(campaignId, "Open the campaign view"));
    return node;
  }
  // r[impl kiln.cairn.workspace.scenario.decisions] Answer and issue controls come only from the campaign inbox, matched
  // by exact decision identity and restricted to this change's members, through the shared answer path and runner.
  async function attachAnswers(current, campaignId, members, cards, statusHost) {
    const inventory = window.KilnInventory;
    if (typeof inventory?.readInbox !== "function") {
      for (const entry of cards) entry.answer.append(readOnlyAnswer("Campaign controls are not loaded; this decision is read-only here.", campaignId));
      return;
    }
    const reading = cards.map((entry) => { const note = element("p", "Reading the campaign's answer actions…", "small"); entry.answer.append(note); return note; });
    try {
      const inbox = await inventory.readInbox(context, campaignId);
      if (current !== generation || !statusHost.isConnected) return;
      const panel = {};
      const buttons = [];
      const reload = (message) => { if (current === generation) { notice = message; void load(mounted, context, { force: true }); } };
      const runnerUi = inventory.runnerUi(panel, reload, (result, message) => reload(message));
      const runner = inventory.commandRunner(context, runnerUi);
      for (const [index, entry] of cards.entries()) {
        reading[index].remove();
        const listed = inbox.decisions.find((candidate) => candidate.identity === entry.decision.identity && candidate.member === entry.decision.member && members.has(candidate.member));
        if (!listed) { entry.answer.append(readOnlyAnswer("The campaign inbox does not list this decision for this change's member; it is read-only here.", campaignId)); continue; }
        const nodes = inventory.decisionAnswer(inbox, listed, runner.start, buttons);
        if (nodes.length) entry.answer.replaceChildren(...nodes);
        else entry.answer.append(readOnlyAnswer(listed.scope === "current" ? "No answer or issue action is open to you for this decision." : lookup(SCOPES, listed.scope).meaning, campaignId));
      }
      panel.current = { controlButtons: buttons };
      if (buttons.length) {
        const line = element("div", undefined, "cairn-command");
        line.append(element("span", `Campaign ${campaignId} at store version ${inbox.store_version}`, "small"), runnerUi.status, runnerUi.retry);
        statusHost.append(line);
      }
    } catch (error) {
      if (current !== generation || isAborted(error)) return;
      for (const [index, entry] of cards.entries()) {
        reading[index].remove();
        const reason = typeof inventory.controlsUnavailable === "function" ? inventory.controlsUnavailable(error, "Campaign controls").textContent : `Campaign controls unavailable: ${explain(error)}`;
        entry.answer.append(readOnlyAnswer(reason, campaignId));
      }
    }
  }
  function nextSteps(detail, current) {
    const body = element("div", undefined, "cairn-next");
    const commandNotice = element("p", notice || undefined, "small cairn-notice");
    commandNotice.setAttribute("role", "status");
    commandNotice.hidden = !notice;
    body.append(commandNotice);
    const ranked = detail.blockers;
    if (ranked.length) {
      const checklist = element("ol", undefined, "cairn-checklist");
      checklist.setAttribute("aria-label", "Blockers, most urgent first");
      for (const blocker of ranked) checklist.append(blockerItem(blocker, detail));
      body.append(checklist);
    } else body.append(element("p", "No structured blocker is observed. This is not readiness on its own; read each unit's stages.", "small cairn-clear"));
    const open = detail.decisions.filter(isOpenDecision);
    const closed = detail.decisions.filter((decision) => !isOpenDecision(decision));
    const decisions = element("div", undefined, "cairn-decisions");
    decisions.append(element("h4", open.length ? `Open decisions (${open.length})` : "Open decisions", "cairn-subhead"));
    if (!open.length) decisions.append(element("p", "No decision of this change's members waits to be issued or answered.", "small"));
    const statusHost = element("div", undefined, "cairn-command-status");
    const byCampaign = new Map();
    for (const decision of open) {
      const entry = decisionCard(decision);
      decisions.append(entry.card);
      if (!byCampaign.has(decision.campaign_id)) byCampaign.set(decision.campaign_id, []);
      byCampaign.get(decision.campaign_id).push(entry);
    }
    decisions.append(statusHost);
    if (closed.length) decisions.append(disclosure(`Resolved and expired decisions (${closed.length})`, "closed-decisions", ...closed.map((decision) => decisionCard(decision).card)));
    body.append(decisions);
    for (const [campaignId, cards] of byCampaign) {
      const members = new Set(detail.memberships.filter((membership) => membership.campaign_id === campaignId).map((membership) => membership.member));
      void attachAnswers(current, campaignId, members, cards, statusHost);
    }
    return section("Next steps", "cairn-next-title", body);
  }
  // A dependency or dependent opens its unit in place when it belongs to this change, or its own change by exact
  // repository identity. Work that names no Cairn unit stays an identity.
  function dependencyItem(view, direction, detail) {
    const item = element("li");
    const state = view.state === "current" ? null : badge(word("Not current", "failure", "The dependency is absent from current canonical work."));
    const ids = strings(view.native_ids);
    const repository = typeof view.repository_identity === "string" && HEX.test(view.repository_identity) ? view.repository_identity : "";
    if (!ids.length) { item.append(...[state, shortCode(view.identity), element("span", " no Cairn unit", "small")].filter(Boolean)); return item; }
    for (const nativeId of ids) {
      const slash = nativeId.indexOf("/");
      const owner = slash > 0 ? nativeId.slice(0, slash) : "";
      let target;
      if (owner === detail.change && repository === detail.repository_identity) target = link({ cairn_unit: nativeId }, nativeId, "cairn-unit-ref", () => openUnit(nativeId));
      else if (repository && validChange(owner)) target = link({ cairn_repository: repository, cairn_change: owner, cairn_unit: nativeId }, nativeId, "cairn-unit-ref");
      else target = element("code", nativeId);
      target.title = direction === "dependency" ? "Open this prerequisite" : "Open this dependent";
      item.append(...[state, target].filter(Boolean));
    }
    return item;
  }
  function attemptItem(attempt) {
    const item = element("li", undefined, "cairn-attempt");
    item.dataset.applicability = attempt.applicability === "current" ? "current" : "historical";
    const head = element("div", undefined, "cairn-attempt-head");
    head.append(element("strong", `Attempt ${attempt.ordinal ?? "?"} · fence ${attempt.fence ?? "?"}`), badge(lookup(APPLICABILITY, attempt.applicability)), badge(facet("execution", attempt.phase)),
      element("span", `revision ${attempt.work_revision ?? "not recorded"}`, "small"));
    item.append(head);
    const times = element("p", undefined, "small cairn-attempt-times");
    times.append(element("span", "Claimed "), when(attempt.claimed_at_ms));
    if (Number.isSafeInteger(attempt.ended_at_ms)) {
      times.append(element("span", " · ended "), when(attempt.ended_at_ms));
      if (attempt.ended_at_ms >= attempt.claimed_at_ms && typeof ui()?.durationElement === "function") times.append(element("span", " · took "), ui().durationElement(attempt.ended_at_ms - attempt.claimed_at_ms));
    } else if (attempt.active) times.append(element("span", " · still active"));
    item.append(times);
    const rows = [["Dispatch", badges(badge(lookup(DISPATCH, attempt.dispatch)), attempt.unknown_outcome ? badge(lookup(UNKNOWN_OUTCOMES, attempt.unknown_outcome)) : null)]];
    if (attempt.failure) rows.push(["Failure", badges(badge(lookup(FAILURES, attempt.failure)), attempt.cause ? element("span", attempt.cause, "small") : null)]);
    if (Number.isSafeInteger(attempt.superseded_by)) rows.push(["Superseded by", `revision ${attempt.superseded_by}`]);
    const verification = attempt.verification;
    if (verification) {
      const checks = list(verification.checks);
      const passed = checks.filter((check) => check?.outcome === "pass").length;
      const value = badges(badge(word(verification.success ? "Verification passed" : "Verification failed", verification.success ? "good" : "failure")),
        typeof verification.run_id === "string" && verification.run_id ? runAnchor(verification.run_id, `Run ${short(verification.run_id)}`) : null,
        element("span", checks.length ? `${passed} of ${plural(checks.length, "check")} passed` : "No checks recorded", "small"));
      rows.push(["Verification", value]);
    } else rows.push(["Verification", element("span", "Not recorded", "small")]);
    const publication = attempt.publication;
    rows.push(["Publication", publication
      ? badges(badge(facet("integration", publication.phase)), element("span", `${publication.branch || "branch not recorded"} → ${publication.base_branch || "base not recorded"}`, "small"),
        publication.pull_request?.native_id ? element("code", `${publication.pull_request.native_kind || "pull request"} ${publication.pull_request.native_id}`) : null)
      : element("span", "No publication record", "small")]);
    const lifecycle = attempt.lifecycle;
    rows.push(["Lifecycle", lifecycle
      ? badge(lifecycle.binding === "matches_attempt" ? word(`Receipt: ${lookup(LIFECYCLE_ACTIONS, lifecycle.action).label}`, "good") : word("Crossed receipt", "failure", "The receipt binds another work identity or fence. It is not read as this attempt's lifecycle."))
      : element("span", "No receipt", "small")]);
    item.append(facts(rows, "facts cairn-attempt-facts"));
    if (attempt.applicability !== "current") item.append(element("p", "Historical: this attempt ran on a superseded work version, so its records do not apply to the current version.", "small"));
    item.append(disclosure("Exact attempt identities", `attempt:${attempt.identity}`, facts([
      ["Attempt", copyable(attempt.identity, "Attempt identity")],
      ["Work", copyable(attempt.work_identity, "Work identity")],
      ["Candidate", attempt.candidate?.native_id ? code(`${attempt.candidate.native_kind || "candidate"} ${attempt.candidate.native_id}`) : "Not recorded"],
      ["Verification profile", verification?.verification_profile ? shortCode(verification.verification_profile) : undefined],
      ["Lifecycle receipt", lifecycle?.lifecycle_receipt?.native_id ? shortCode(lifecycle.lifecycle_receipt.native_id) : undefined],
    ])));
    return item;
  }
  // r[impl kiln.cairn.workspace.scenario.evidence] Attempts oldest first; a historical attempt is muted and labelled and
  // never reads as current.
  function unitDetail(unit, detail) {
    const node = element("div", undefined, "cairn-unit-detail");
    const current = list(unit.current_work);
    if (!current.length) node.append(element("p", "No current canonical work names this unit. It is listed because a campaign selects it or still owes it.", "small"));
    for (const work of current) {
      const head = element("div", undefined, "cairn-work");
      head.append(element("h4", work.title || "Untitled work", "cairn-work-title"));
      if (work.requested_outcome) head.append(element("p", work.requested_outcome, "small"));
      head.append(facts([
        ["Revision", `${work.revision} · priority ${work.priority ?? "not recorded"}`],
        ["Project", work.project],
        ["Terminal policy", typeof work.terminal_policy === "string" ? work.terminal_policy.replaceAll("_", " ") : "Not recorded"],
        ["Acceptance", bullets(list(work.acceptance).map((obligation) => { const item = element("li"); item.append(element("code", obligation?.id ?? "?"), document.createTextNode(` ${obligation?.statement ?? ""}`)); return item; }), "No acceptance obligation is admitted.")],
        ["Depends on", bullets(list(work.dependencies).map((view) => dependencyItem(view, "dependency", detail)), "Nothing")],
        ["Required by", bullets(list(work.dependents).map((view) => dependencyItem(view, "dependent", detail)), "No current work")],
      ]));
      head.append(disclosure("Exact work identities", `work:${work.identity}`, facts([
        ["Work identity", copyable(work.identity, "Work identity")],
        ["Canonical key", copyable(work.canonical_key, "Canonical key")],
        ["Readiness snapshot", copyable(work.readiness_snapshot, "Readiness snapshot")],
      ])));
      node.append(head);
    }
    const superseded = list(unit.superseded_work);
    if (superseded.length) node.append(element("p", `Superseded versions: ${superseded.map((work) => `revision ${work.revision} (${short(work.identity)})`).join(", ")}`, "small"));
    const attempts = list(unit.attempts);
    node.append(element("h4", attempts.length ? `Attempts (${attempts.length}), oldest first` : "Attempts", "cairn-subhead"));
    if (!attempts.length) node.append(element("p", "No attempt is recorded for this unit.", "small"));
    else {
      const timeline = element("ol", undefined, "cairn-timeline");
      for (const attempt of attempts) timeline.append(attemptItem(attempt));
      node.append(timeline);
    }
    return node;
  }
  // r[impl kiln.cairn.workspace.scenario.stages] One row per unit and one column per facet, each in its own words; the
  // unit opens its work, dependencies and attempt timeline in place, bound to the URL.
  function unitsSection(detail, route) {
    const missing = route.unit && !detail.units.some((unit) => unit.native_id === route.unit)
      ? element("p", `The URL names unit ${route.unit}, which this change does not list. No other unit was opened.`, "warning") : null;
    const invalid = route.invalidUnit ? element("p", "The URL names an invalid unit for this change. No unit was opened.", "warning") : null;
    if (!detail.units.length) return section("Units", "cairn-units-title", missing, invalid, element("p", "No unit is named yet. A whole-change selection names the change, not a unit.", "small"));
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack cairn-units");
    table.append(element("caption", "Units of this change with their five stages. Open a unit for its work, dependencies and attempts.", "sr-only"));
    const thead = element("thead");
    const headRow = element("tr");
    for (const title of ["Unit", ...Object.values(FACETS).map((entry) => entry.title)]) { const th = element("th", title); th.scope = "col"; headRow.append(th); }
    thead.append(headRow);
    const tbody = element("tbody");
    table.append(thead, tbody);
    wrap.append(table);
    const rowsOpen = [];
    for (const [index, unit] of detail.units.entries()) {
      const row = element("tr", undefined, "cairn-unit-row");
      row.dataset.lane = boardLane(unit.lane);
      const head = element("td", undefined, "cairn-unit-cell");
      head.dataset.label = "Unit";
      const toggle = element("button", undefined, "cairn-unit-toggle");
      toggle.type = "button";
      toggle.dataset.unit = unit.native_id;
      toggle.dataset.focusKey = `unit:${unit.native_id}`;
      toggle.setAttribute("aria-controls", `cairn-unit-${index}`);
      const name = element("span", undefined, "cairn-unit-name");
      if (typeof ui()?.icon === "function") name.append(ui().icon("chevron-down"));
      name.append(document.createTextNode(unit.unit || unit.native_id));
      toggle.append(name, element("span", `${lane(unit.lane).label} · ${strings([...list(unit.sources)]).map((source) => lookup(SOURCES, source).label).join(", ")}`, "small"));
      head.append(toggle);
      row.append(head);
      for (const [name, entry] of Object.entries(FACETS)) {
        const td = element("td");
        td.dataset.label = entry.title;
        td.append(badge(facet(name, unit.facets?.[name])));
        row.append(td);
      }
      const detailRow = element("tr", undefined, "cairn-unit-detail-row");
      detailRow.id = `cairn-unit-${index}`;
      const detailCell = element("td");
      detailCell.colSpan = 6;
      detailRow.append(detailCell);
      const setOpen = (open) => {
        toggle.setAttribute("aria-expanded", String(open));
        row.dataset.open = String(open);
        detailRow.hidden = !open;
        if (open && !detailCell.firstChild) detailCell.append(unitDetail(unit, detail));
      };
      setOpen(unit.native_id === route.unit);
      // One unit is open at a time; the URL names it.
      toggle.addEventListener("click", () => {
        const open = toggle.getAttribute("aria-expanded") !== "true";
        for (const other of rowsOpen) if (other.toggle !== toggle && other.toggle.getAttribute("aria-expanded") === "true") other.setOpen(false);
        setOpen(open);
        commitLocal({ cairn_unit: open ? unit.native_id : "" });
      });
      rowsOpen.push({ toggle, setOpen });
      tbody.append(row, detailRow);
    }
    table.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      const buttons = [...table.querySelectorAll(".cairn-unit-toggle")];
      const index = buttons.indexOf(document.activeElement);
      if (index < 0) return;
      const next = buttons[index + (event.key === "ArrowDown" ? 1 : -1)];
      if (next) { event.preventDefault(); next.focus(); }
    });
    return section("Units", "cairn-units-title", missing, invalid, wrap);
  }
  const SVG_NS = "http://www.w3.org/2000/svg";
  function drawEdges(canvas, graph, buttons) {
    if (!canvas.isConnected) return;
    canvas.querySelector("svg")?.remove();
    const svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("width", String(canvas.scrollWidth));
    svg.setAttribute("height", String(canvas.scrollHeight));
    svg.setAttribute("aria-hidden", "true");
    for (const { from, to } of graph.edges) {
      const source = buttons.get(from);
      const target = buttons.get(to);
      if (!source || !target) continue;
      // Nodes sit in positioned stages, so their offsets add the stage's own.
      const startX = source.offsetLeft + source.parentElement.offsetLeft + source.offsetWidth;
      const startY = source.offsetTop + source.parentElement.offsetTop + source.offsetHeight / 2;
      const endX = target.offsetLeft + target.parentElement.offsetLeft;
      const endY = target.offsetTop + target.parentElement.offsetTop + target.offsetHeight / 2;
      const middle = (startX + endX) / 2;
      const path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", `M${startX},${startY} C${middle},${startY} ${middle},${endY} ${endX},${endY}`);
      path.setAttribute("class", "dag-edge");
      const arrow = document.createElementNS(SVG_NS, "path");
      arrow.setAttribute("d", `M${endX - 6},${endY - 4} L${endX},${endY} L${endX - 6},${endY + 4}`);
      arrow.setAttribute("class", "dag-edge");
      svg.append(path, arrow);
    }
    canvas.prepend(svg);
  }
  const LANE_STATE = { attention: "failed", in_progress: "running", verified: "completed", integrated: "completed", recorded: "completed" };
  function graphSection(detail) {
    const graph = dependencyGraph(detail);
    if (!graph.edges.length) return section("Dependencies", "cairn-graph-title", element("p", "No dependency between this change's units, or on other work, is recorded.", "small"));
    const scroller = element("div", undefined, "cairn-graph");
    const canvas = element("div", undefined, "dag-canvas cairn-graph-canvas");
    const buttons = new Map();
    const byId = new Map(graph.nodes.map((node) => [node.id, node]));
    for (const [level, column] of graph.columns.entries()) {
      const nodes = column.map((id) => byId.get(id));
      const stage = element("div", undefined, "dag-stage");
      stage.append(element("h3", nodes.some((node) => node.cycle) ? "In or after a cycle" : level === 0 ? "No prerequisite" : `Level ${level + 1}`));
      for (const node of nodes) {
        let target;
        if (node.kind === "unit") {
          target = element("button", undefined, "dag-node cairn-graph-node");
          target.type = "button";
          target.addEventListener("click", () => openUnit(node.nativeId));
        } else if (node.kind === "external") {
          target = node.repository && validChange(node.change)
            ? link({ cairn_repository: node.repository, cairn_change: node.change, cairn_unit: node.nativeId }, undefined, "button dag-node cairn-graph-node")
            : element("div", undefined, "dag-node cairn-graph-node");
        } else target = element("div", undefined, "dag-node cairn-graph-node");
        target.dataset.kind = node.kind;
        if (LANE_STATE[node.lane]) target.dataset.state = LANE_STATE[node.lane];
        target.append(element("strong", node.label));
        const caption = node.kind === "unit" ? lane(node.lane).label : node.kind === "external" ? `Change ${node.change}` : node.state === "current" ? "Current work, no Cairn unit" : "Not current work";
        target.append(element("span", node.cycle ? `${caption} · cycle` : caption, "small"));
        if (node.kind === "work") target.title = node.identity;
        buttons.set(node.id, target);
        stage.append(target);
      }
      canvas.append(stage);
    }
    scroller.append(canvas);
    const draw = () => drawEdges(canvas, graph, buttons);
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(() => requestAnimationFrame(draw));
    const legend = element("p", `Arrows point from a prerequisite to the work that depends on it. ${graph.cyclic ? "Some nodes sit in or after a dependency cycle and are drawn in the last column." : ""}`.trim(), "small");
    return section("Dependencies", "cairn-graph-title", legend, scroller);
  }
  function membershipCard(membership) {
    const card = element("article", undefined, "cairn-member");
    card.tabIndex = -1;
    card.dataset.member = membership.member;
    card.dataset.campaign = membership.campaign_id;
    card.append(element("h4", membership.member, "cairn-member-title"));
    card.append(badges(badge(binding(membership.work_binding)), membership.observed ? badge(lookup(STAGES, membership.stage), "Stage") : badge(word("Not observed", "warning", "The campaign has not observed this member yet.")),
      membership.enumeration ? badge(lookup(ENUMERATIONS, membership.enumeration)) : null, badge(lookup(ADMISSIONS, membership.admission))));
    const effects = list(membership.pending_effects).map((effect) => `${lookup(OPERATIONS, effect.operation).label} by ${effect.owner}: ${String(effect.state).replaceAll("_", " ")}${effect.progress_applied ? ", progress applied" : ", progress not applied"}`);
    const hold = membership.hold
      ? facts([["Operation", lookup(OPERATIONS, membership.hold.operation).label], ["Owner", membership.hold.owner || "Not named"], ["Reason (display only)", membership.hold.reason]], "facts cairn-hold")
      : "No campaign hold";
    card.append(facts([
      ["Campaign", campaignLink(membership.campaign_id)],
      ["Selection", membership.selector === "tasks" ? `Tasks: ${strings(membership.tasks).join(", ")}` : "Whole change"],
      ["Goal", lookup(GOALS, membership.goal).label],
      ["Hold", hold],
      ["Remaining obligations", bullets(strings(membership.remaining_obligations), "None observed")],
      ["Member blockers (display only)", bullets(strings(membership.blockers), "None observed")],
      ["Pending effects", bullets(effects, "None")],
    ]));
    card.append(disclosure("Exact member bindings", `member:${membership.campaign_id}:${membership.member}`, facts([
      ["Bound work", membership.work_identity ? copyable(membership.work_identity, "Bound work identity") : "Not bound"],
      ["Bound attempt", membership.attempt_identity ? shortCode(membership.attempt_identity) : "None"],
      ["Attempt fence", membership.attempt_fence ?? "None"],
      ["Store version", String(membership.store_version)],
      ["Selection revision", String(membership.selection_revision)],
    ])));
    return card;
  }
  function campaignsSection(detail) {
    if (!detail.memberships.length) return section("Campaigns", "cairn-campaigns-title", element("p", "No selected campaign member names this change.", "small"));
    const grid = element("div", undefined, "cairn-members");
    grid.append(...detail.memberships.map(membershipCard));
    return section("Campaigns", "cairn-campaigns-title", grid);
  }
  function legend() {
    const node = element("dl", undefined, "facts cairn-legend");
    const lanes = element("dd");
    lanes.append(bullets([...LANES.values()].map((entry) => { const item = element("li"); item.append(badge(entry), document.createTextNode(` ${entry.meaning}`)); return item; }), ""));
    node.append(element("dt", "Lanes"), lanes);
    for (const entry of Object.values(FACETS)) {
      const values = element("dd");
      values.append(badges(...[...entry.values.values()].map((value) => badge(value))));
      node.append(element("dt", entry.title), values);
    }
    return node;
  }
  function aboutSection(detail) {
    const snapshot = detail.snapshot || {};
    return disclosure("About this view: exact identities, snapshot and what it does not claim", "about",
      facts([
        ["Repository identity", copyable(detail.repository_identity, "Repository identity")],
        ["View identity", copyable(detail.identity, "View identity")],
        ["Canonical work", snapshot.work_recorded === false ? "Not recorded in this store" : "Recorded"],
        ["Attempts", snapshot.attempts_recorded === false ? "Not recorded in this store" : "Recorded"],
        ["Campaigns read", `${snapshot.campaigns_read ?? "unknown"}${snapshot.campaigns_truncated ? ", truncated at the read bound" : ""}`],
        ["Lifecycle receipts", snapshot.lifecycle_truncated ? "Truncated at the read bound" : "Complete within the read bound"],
      ]),
      element("h4", "What this view does not claim", "cairn-subhead"),
      bullets(strings(detail.non_claims), "No non-claims were supplied."),
      element("h4", "How to read lanes and stages", "cairn-subhead"),
      legend());
  }
  // r[impl kiln.cairn.workspace.scenario.detail] A change reads as next steps, units, dependencies and campaigns before its
  // identities; All changes, previous and next keep the register's filters, and Browse changes keeps the register itself.
  function changePage(route, rows) {
    const page = element("div", undefined, "cairn-page");
    page.append(changeNav(route, rows));
    const sheet = element("article", undefined, "panel cairn-change");
    sheet.setAttribute("aria-label", "Selected Cairn change");
    const detail = model.detail || {};
    if (detail.error) {
      const titleNode = element("h2", route.change, "cairn-title");
      titleNode.tabIndex = -1;
      titleNode.dataset.focusKey = "title";
      const message = detail.error.status === 404
        ? `${route.change} is not named by canonical work or a selected campaign in this store. No other change was substituted.`
        : `${route.change} unavailable: ${explain(detail.error)} No other change was substituted.`;
      sheet.append(titleNode, errorState(message));
      page.append(sheet);
    } else {
      const value = detail.value;
      const current = generation;
      sheet.append(changeHeader(value), nextSteps(value, current), unitsSection(value, route), graphSection(value), campaignsSection(value), aboutSection(value));
      page.append(sheet);
    }
    if (rows.length || model.index.error) page.append(disclosure(rows.length ? `Browse ${plural(rows.length, "change")}` : "Browse changes", "browse", registerPanel(route, rows, model.index, true)));
    return page;
  }
  function registerPage(route, rows) {
    const page = element("div", undefined, "cairn-page");
    if (route.invalid) page.append(element("p", "The URL names an invalid repository identity or change. Nothing was selected.", "warning"));
    page.append(registerPanel(route, rows, model.index, false));
    return page;
  }
  function title(route) {
    document.title = ["Kiln", "Cairn changes", route.change].filter(Boolean).join(" · ");
  }
  function focusKey(root) {
    const active = document.activeElement;
    return active && root.contains?.(active) ? active.dataset?.focusKey || "" : "";
  }
  function render(container, route) {
    const previous = shown;
    const key = focusKey(container);
    const open = new Map([...(container.querySelectorAll?.("details[data-cairn-disclosure]") || [])].map((node) => [node.dataset.cairnDisclosure, node.open]));
    container.replaceChildren(route.change ? changePage(route, model.rows) : registerPage(route, model.rows));
    if (sameSelection(previous, route)) for (const node of container.querySelectorAll?.("details[data-cairn-disclosure]") || []) if (open.has(node.dataset.cairnDisclosure)) node.open = open.get(node.dataset.cairnDisclosure);
    shown = route;
    title(route);
    const find = (wanted) => [...(container.querySelectorAll?.("[data-focus-key]") || [])].find((node) => node.dataset.focusKey === wanted && !node.closest?.("[hidden]"));
    if (key && find(key)) { find(key).focus({ preventScroll: true }); return; }
    if (previous?.change && !route.change) {
      const back = find(`change:${previous.repository}/${previous.change}`);
      if (back) { back.focus({ preventScroll: true }); back.scrollIntoView?.({ block: "center" }); return; }
    }
    if (route.change && !sameSelection(previous, route)) {
      container.scrollIntoView?.({ block: "start" });
      find("title")?.focus({ preventScroll: true });
      if (route.unit) openUnit(route.unit);
    }
  }

  // r[impl kiln.cairn.workspace.scenario.reload] Reads are guarded by a generation: a response for an earlier selection, or
  // after the page was cleared, never draws. A route this page applied itself is neither read nor redrawn.
  async function load(container, ctx, options = {}) {
    if (!container) return;
    const route = normalizeRoute(ctx?.route);
    context = ctx;
    const drawn = mounted === container && model && shown && container.firstChild;
    if (!options.force && drawn && expected && sameRoute(route, expected)) {
      // Browse changes filters move previous and next; nothing else needs a redraw.
      const filtersMoved = route.change && ["q", "lane", "blocked", "decisions"].some((key) => route[key] !== shown[key]);
      shown = route;
      expected = null;
      title(route);
      if (filtersMoved) container.querySelector(".cairn-change-nav")?.replaceWith(changeNav(route, model.rows));
      return;
    }
    expected = null;
    if (!options.force && drawn && sameSelection(route, shown) && !sameRoute(route, shown)) { render(container, route); return; }
    const current = ++generation;
    clearTimeout(filterTimer);
    if (mounted !== container) { mounted?.replaceChildren(); model = null; shown = null; }
    mounted = container;
    const live = () => current === generation && container.isConnected && sameSelection(route, normalizeRoute(context?.route));
    if (!model || !sameSelection(route, shown)) container.replaceChildren(loadingState());
    container.setAttribute?.("aria-busy", "true");
    const [index, detail] = await Promise.all([
      ctx.api(INDEX_ROUTE).then((value) => ({ value }), (error) => ({ error })),
      route.change ? ctx.api(detailPath(route)).then((value) => ({ value }), (error) => ({ error })) : Promise.resolve(null),
    ]);
    if (!live()) return;
    container.removeAttribute?.("aria-busy");
    if (isAborted(index.error) || isAborted(detail?.error)) return;
    let rows = [];
    let indexResult = index;
    if (index.value) {
      try { rows = indexRows(index.value); } catch (error) { indexResult = { error }; }
    }
    let detailResult = detail;
    if (detail?.value) {
      try { detailResult = { value: admitDetail(detail.value, route) }; } catch (error) { detailResult = { error }; }
    }
    model = { index: indexResult, rows, detail: detailResult };
    render(container, normalizeRoute(context?.route));
    notice = "";
  }
  function clear() {
    generation += 1;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted?.removeAttribute?.("aria-busy");
    mounted = null;
    context = null;
    model = null;
    shown = null;
    expected = null;
    notice = "";
  }

  window.KilnCairn = Object.freeze({
    load, clear,
    facets: FACETS, lanes: LANES, facet, lane, binding, condition, normalizeRoute, filterPatch, detailPath, indexRows, matches, filterCounts, neighbors, admitDetail, dependencyGraph,
  });
})();
