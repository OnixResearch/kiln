"use strict";
// r[impl kiln.cairn.workspace] Cairn changes (?page=cairn): a register of Cairn changes (list or board) and one change's
// next steps, units, dependencies and campaigns, joined by exact identity from one bounded snapshot.
// The projection is read-only. Native Cairn authoring is a separate capability for explicitly enrolled roots.
// Campaign decisions continue to use the shared owner-only campaign answer path.
(() => {
  const INDEX_SCHEMA = "kiln.cairn-workspace-index.v2";
  const DETAIL_SCHEMA = "kiln.cairn-workspace-detail.v4";
  const INDEX_ROUTE = "/api/implementation/cairn";
  const DETAIL_ROUTE = "/api/implementation/cairn/detail";
  const AUTHORING_ROUTE = "/api/implementation/cairn/authoring";
  const AUTHORING_SCHEMA = "kiln.cairn-authoring.v1";
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
  const PLAN_GATES = vocabulary([
    ["no_plan", "No plan", "warning", "No plan has been proposed for this work identity."],
    ["unresolved", "Unresolved annotations", "warning", "Blocking annotations must be resolved before acceptance."],
    ["unreviewed", "Awaiting review", "info", "Proposed plan awaits reviewer decision."],
    ["changes_requested", "Changes requested", "warning", "Reviewer requested changes on this plan revision."],
    ["accepted", "Accepted", "good", "Plan accepted for this exact work identity."],
  ]);
  const RECOVERY_LIFECYCLE_ACTIONS = Object.freeze([
    { action: "prepare-publication", label: "Prepare publication", danger: false, tier: "Tier 2", warning: "Prepares a bounded publication plan for this exact verified candidate. Does not publish or merge." },
    { action: "publish", label: "Publish", danger: true, tier: "Tier 3", warning: "Delegates external publication to the configured owner command for this exact candidate. Requires explicit operator authority." },
    { action: "recover-integration", label: "Recover integration", danger: false, tier: "Tier 2", warning: "Reconciles integration state from retained provider/owner evidence for this exact candidate." },
    { action: "close-source", label: "Close source", danger: true, tier: "Tier 3", warning: "Reconciles and closes the source item after observed integration. Fails closed if integration or authority is missing." },
  ]);
  // r[impl kiln.cairn.workspace.agents.scenario.slots] Revision facts of definitions, worker pins and workflow revisions
  // share one vocabulary. Unmanaged, unknown (with its owner), recorded, none recorded and not published never share words.
  const SLOT_STATES = vocabulary([
    ["recorded", "Recorded", "info", "The producer recorded this revision fact."],
    ["not_recorded", "None recorded", "", "The lifecycle manages this definition and records no value for its projected revision."],
    ["unknown", "Unknown", "warning", "The producer manages this fact but does not know it yet. Its owner can resolve it."],
    ["unmanaged", "Unmanaged", "", "No producer revision lifecycle governs this. It is neither unknown nor unqualified."],
    ["unpublished", "Not published", "warning", "No producer publishes this fact yet."],
  ]);
  const MANAGEMENT = vocabulary([
    ["managed", "Managed lifecycle", "info", "A producer revision lifecycle governs this definition."],
    ["unmanaged", "Unmanaged lifecycle", "", "No producer revision lifecycle governs this definition."],
    ["unpublished", "Lifecycle not published", "warning", "The producer does not say whether a revision lifecycle governs this definition."],
  ]);
  // r[impl kiln.cairn.workspace.agents.scenario.provenance] Fixture evidence never reads as live and never takes a good tone.
  const PROVENANCE = vocabulary([
    ["live", "Live evidence", "good", "A live qualification reference for this revision. It is evidence, never dispatch authority."],
    ["fixture", "Fixture evidence, not live", "warning", "A fixture qualification reference. It never counts as live evidence."],
    ["unqualified", "Unqualified", "", "No current qualification reference names this task family for this revision."],
  ]);
  const CAPABILITIES = vocabulary([
    ["declared", "Declared", ""],
    ["none", "None", "", "The producer states that none is available."],
    ["unknown", "Unknown", "warning", "The producer does not publish this capability. It is not unsupported."],
  ]);
  // r[impl kiln.cairn.workspace.agents.scenario.selection] Selected and pinned revisions stay separate facts.
  const SELECTIONS = vocabulary([
    ["matches_selected", "Pinned to the selected revision", "info", "The session's pin equals the revision selected for new sessions."],
    ["differs_from_selected", "Pinned to another revision", "warning", "New sessions select another revision; this session keeps its own pin."],
    ["no_selection", "No selection recorded", "", "The definition records no revision selected for new sessions."],
    ["definition_not_observed", "Definition not observed", "warning", "The worker's definition is not in this inventory."],
  ]);
  const WORK_BINDINGS = vocabulary([
    ["this_change", "On this change", "info", "Its attempt and fence equal an attempt of this change."],
    ["other_work", "Other work", "", "Its attempt belongs to other work; the identity is withheld here."],
    ["no_attempt", "No attempt", "", "The worker names no attempt."],
  ]);
  const PIN_STATUSES = vocabulary([["live", "Live session"], ["closed", "Session closed"], ["released", "Pin released"]]);
  // r[impl kiln.cairn.workspace.agents.scenario.workflow] A workflow revision names the worker revision it pairs with,
  // compared with the revision the inventory projects for that worker definition.
  const WORKER_PAIRINGS = vocabulary([
    ["profile_revision", "Pairs with the projected worker revision", "info", "The paired worker revision is the revision this inventory projects for the worker definition."],
    ["other_revision", "Pairs with another worker revision", "warning", "The paired worker revision is not the revision this inventory projects for the worker definition."],
    ["definition_not_observed", "Worker definition not observed", "warning", "The paired worker definition is not in this inventory."],
  ]);
  const SELECTION_KINDS = vocabulary([["promotion", "Promotion"], ["rollback", "Rollback", "warning"]]);
  const OBSERVATIONS = vocabulary([
    ["current", "Current"], ["stale", "Stale", "warning", "Observed before the freshness window. Not current."], ["missing", "Missing", "warning"],
    ["future", "From the future", "warning"], ["incomplete", "Incomplete", "warning"], ["out_of_order", "Out of order", "warning"],
    ["unavailable", "Unavailable", "warning"], ["unknown", "Partly unknown", "warning", "The fact lists fields its producer does not publish, or its source state is unknown. It is not current evidence of every field."],
  ]);
  const LIFECYCLES = vocabulary([
    ["configured", "Configured"], ["starting", "Starting", "info"], ["healthy", "Healthy", "info"], ["idle", "Idle", "info"], ["reserved", "Reserved", "info"],
    ["executing", "Executing", "info"], ["stopped", "Stopped"], ["unavailable", "Unavailable", "warning"], ["unknown", "Lifecycle unknown", "warning"],
  ]);
  const PERMISSIONS = vocabulary([["allowed", "Allowed"], ["denied", "Denied", "warning"], ["revoked", "Revoked", "failure"], ["unknown", "Unknown", "warning"]]);
  const AGENT_SOURCES = ["admitted", "not_configured", "unavailable"];

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
  const str = (value) => (typeof value === "string" ? value : "");
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
        activeAssignments: Array.isArray(entry.active_assignments) ? entry.active_assignments : null,
        assignmentsTruncated: entry.active_assignments_truncated === true,
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
      || !["units", "current_work", "attempts", "active_attempts", "campaigns", "blockers", "open_decisions"].every((key) => count(value.overview[key]) !== null)
      || !admitAgents(value.agents)) {
      throw new Error("The service returned a different or malformed Cairn change. No other change was substituted.");
    }
    return value;
  }
  // r[impl kiln.cairn.workspace.agents.scenario.source] The agents section must name its inventory source, so an
  // unconfigured or unavailable inventory is never drawn as an empty one.
  function admitAgents(value) {
    const workflow = value?.workflow_revisions;
    return typeof value === "object" && value !== null && AGENT_SOURCES.includes(value.source)
      && Array.isArray(value.definitions) && Array.isArray(value.workers)
      && typeof workflow === "object" && workflow !== null && AGENT_SOURCES.includes(workflow.source) && Array.isArray(workflow.definitions);
  }
  function slotWord(slot) { return lookup(SLOT_STATES, slot?.state); }
  // One revision fact in words: its state, then its value, or the owner of an unknown or unpublished fact.
  function slotText(slot) {
    const entry = slotWord(slot);
    if (slot?.state === "recorded") return `${entry.label}: ${str(slot.value) || "value not recorded"}`;
    if (slot?.state === "unknown" || slot?.state === "unpublished") return `${entry.label} · ${str(slot.owner) ? `owner ${slot.owner}` : "no owner named"}`;
    return entry.label;
  }
  // A task family reads live only when its reference has live provenance and counts as live; fixture stays fixture.
  function familyProvenance(family) {
    const qualification = family?.qualification;
    if (!qualification || typeof qualification !== "object") return lookup(PROVENANCE, "unqualified");
    if (qualification.provenance === "live" && qualification.counts_as_live === true) return lookup(PROVENANCE, "live");
    if (qualification.provenance === "fixture") return lookup(PROVENANCE, "fixture");
    return lookup(PROVENANCE, `${str(qualification.provenance) || "unrecorded"} provenance`);
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
  let activeController = null;
  let fleetController = null;
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
    intro.append(heading, element("p", "Each lane shows the change’s least-advanced unit. A stop in any unit needs attention.", "small"));
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
  function newRequestId() {
    if (typeof crypto?.randomUUID === "function") return crypto.randomUUID();
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    return `cairn-${[...bytes].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
  }

  async function confirmCairnControl(options) {
    if (typeof context?.confirmControl === "function") {
      return context.confirmControl(options);
    }
    const byId = (id) => (typeof document?.getElementById === "function" ? document.getElementById(id) : null);
    const dialog = byId("control-dialog");
    const form = byId("control-form");
    if (!dialog || !form) {
      return { confirmed: false, reason: "", unavailable: true };
    }
    const title = byId("confirm-title");
    const warning = byId("confirm-warning");
    const factsEl = byId("confirm-facts");
    const confirmBtn = byId("confirm-control");
    const dismissBtn = byId("dismiss-control");
    const reasonField = byId("confirm-reason-field");
    const reasonLabel = byId("confirm-reason-label");
    const reasonInput = byId("confirm-reason");
    const reasonError = byId("confirm-reason-error");
    const cliWrap = byId("confirm-cli-parity");
    const cliCmd = byId("confirm-cli-cmd");
    const cliStatus = byId("confirm-cli-status");
    if (title) title.textContent = options.title || "Confirm action";
    if (warning) warning.textContent = options.warning || "";
    if (confirmBtn) {
      confirmBtn.textContent = options.verb || "Confirm";
      confirmBtn.className = options.danger ? "danger" : "primary";
    }
    if (dismissBtn) dismissBtn.textContent = options.dismiss || "Cancel";
    if (factsEl) {
      factsEl.replaceChildren();
      for (const [term, val] of options.facts || []) {
        if (val === undefined || val === null || val === "") continue;
        factsEl.append(element("dt", term), element("dd", String(val)));
      }
    }
    if (reasonField && reasonInput) {
      if (options.reason) {
        reasonField.hidden = false;
        if (reasonLabel) reasonLabel.textContent = options.reason.label || "Reason";
        reasonInput.placeholder = options.reason.placeholder || "";
        reasonInput.maxLength = options.reason.maxLength || 1024;
        reasonInput.required = Boolean(options.reason.required);
        reasonInput.value = options.reason.initial || "";
      } else {
        reasonField.hidden = true;
        reasonInput.required = false;
        reasonInput.value = "";
      }
      if (reasonError) { reasonError.textContent = ""; reasonError.hidden = true; }
    }
    if (cliWrap && cliCmd) {
      if (options.cliCommand) {
        cliWrap.hidden = false;
        cliCmd.textContent = options.cliCommand;
        if (cliStatus) cliStatus.textContent = "";
      } else {
        cliWrap.hidden = true;
        cliCmd.textContent = "";
      }
    }
    return new Promise((resolve) => {
      let settled = false;
      const cleanup = () => {
        form.removeEventListener("submit", onSubmit);
        dismissBtn?.removeEventListener("click", onCancel);
        dialog.removeEventListener("cancel", onCancel);
        if (dialog.open) dialog.close();
      };
      const finish = (res) => {
        if (settled) return;
        settled = true;
        cleanup();
        options.opener?.focus?.();
        resolve(res);
      };
      const onCancel = () => finish({ confirmed: false, reason: "" });
      const onSubmit = (event) => {
        event.preventDefault();
        const reason = (reasonInput?.value || "").trim().slice(0, 1024);
        if (options.reason?.required && !reason) {
          if (reasonError) { reasonError.textContent = "A reason is required."; reasonError.hidden = false; }
          reasonInput?.focus?.();
          return;
        }
        finish({ confirmed: true, reason, requestId: newRequestId() });
      };
      form.addEventListener("submit", onSubmit);
      dismissBtn?.addEventListener("click", onCancel);
      dialog.addEventListener("cancel", onCancel);
      dialog.showModal();
    });
  }

  function formatAnnotationRegion(ann) {
    const reg = ann?.region;
    if (typeof reg === "string" && reg) return reg;
    if (reg && typeof reg === "object") {
      if (reg.kind === "span" || Number.isSafeInteger(reg.start)) {
        return `step ${str(reg.step || ann.step_id)} [${reg.start ?? 0}..${reg.end ?? "?"})`;
      }
      if (reg.step || ann.step_id) return `step ${str(reg.step || ann.step_id)}`;
    }
    return ann?.step_id ? `step ${str(ann.step_id)}` : "plan";
  }

  function formatAnnotationNote(ann) {
    const note = ann?.note;
    if (typeof note === "string") return note;
    if (note && typeof note === "object") {
      if (note.kind === "delete") return "Delete step";
      if (note.kind === "replace") return `Replace with: ${str(note.text)}`;
      return str(note.text || note.comment || JSON.stringify(note));
    }
    return str(ann?.comment) || "";
  }

  function annotationMatchesStep(ann, stepId) {
    if (!stepId) return false;
    if (str(ann?.step_id) === stepId) return true;
    if (ann?.region && typeof ann.region === "object" && str(ann.region.step) === stepId) return true;
    return false;
  }

  // r[impl kiln.implementation.plan_review_console_surface]
  function renderPlanReview(container, opts = {}) {
    if (!container) return null;
    let workId = str(opts.workId || opts.work_id);
    let planId = str(opts.planId || opts.plan_id || opts.plan?.identity);
    let revision = count(opts.revision ?? opts.plan?.revision);
    let gate = str(opts.gate) || "unreviewed";
    let author = str(opts.author || opts.plan?.author);
    let steps = list(opts.steps ?? opts.plan?.steps);
    let annotations = [...list(opts.annotations ?? opts.items)];
    let comparison = opts.comparison && typeof opts.comparison === "object" ? opts.comparison : null;
    let receiptDigest = str(opts.receiptDigest || opts.receipt_digest || planId);
    const canOp = opts.canOperate !== undefined ? Boolean(opts.canOperate) : (context?.canOperate ? Boolean(context.canOperate()) : true);
    const apiFn = opts.api || context?.api;

    const panel = element("section", undefined, "cairn-plan-review");
    const header = element("div", undefined, "cairn-plan-head");
    const badgesWrap = element("div", undefined, "badges");
    const receiptRow = element("p", undefined, "small cairn-plan-receipt");
    const metaFactsHost = element("div");
    const diffHost = element("div");

    const drawHeader = () => {
      badgesWrap.replaceChildren(
        badge(lookup(PLAN_GATES, gate), "Plan gate"),
        revision !== null ? badge(word(`Revision ${revision}`, "info")) : null,
      );
      receiptRow.replaceChildren(
        element("span", "Receipt BLAKE3 digest: "),
        receiptDigest ? copyable(receiptDigest, "Plan receipt digest") : element("span", "Not recorded", "not-recorded"),
      );
      metaFactsHost.replaceChildren(facts([
        ["Work identity", workId ? copyable(workId, "Work identity") : "Not recorded"],
        ["Plan identity", planId ? copyable(planId, "Plan identity") : "Not recorded"],
        ["Author", author || "Not recorded"],
      ]));
      diffHost.replaceChildren();
      if (comparison) {
        const diffSummary = [
          `kept: ${list(comparison.kept).length}`,
          `changed: ${list(comparison.changed).length}`,
          `added: ${list(comparison.added).length}`,
          `removed: ${list(comparison.removed).length}`,
          comparison.reordered ? "reordered" : "order preserved",
        ].join(" · ");
        diffHost.append(element("p", `Revision comparison vs ${short(comparison.previous)}: ${diffSummary}`, "small cairn-plan-diff"));
      }
    };
    header.append(element("h4", "Implementation plan review", "cairn-subhead"), badgesWrap, metaFactsHost, receiptRow, diffHost);
    drawHeader();
    panel.append(header);

    const statusBanner = element("p", undefined, "small cairn-plan-status");
    statusBanner.setAttribute("role", "status");
    statusBanner.hidden = true;
    const errorBanner = element("p", undefined, "warning error-state cairn-plan-error");
    errorBanner.setAttribute("role", "alert");
    errorBanner.hidden = true;

    let staleLocked = false;
    const decisionButtons = [];
    const setStatus = (msg) => {
      statusBanner.textContent = msg;
      statusBanner.hidden = !msg;
      if (msg) errorBanner.hidden = true;
    };
    const setError = (msg, isStale = false) => {
      errorBanner.textContent = msg;
      errorBanner.hidden = !msg;
      if (isStale) {
        staleLocked = true;
        for (const btn of decisionButtons) btn.disabled = true;
      }
    };

    const applyFeedbackSnapshot = (fb) => {
      if (!fb || typeof fb !== "object") return;
      if (str(fb.work_id)) workId = fb.work_id;
      if (str(fb.plan_id)) planId = fb.plan_id;
      if (count(fb.revision) !== null) revision = fb.revision;
      if (str(fb.gate)) gate = fb.gate;
      if (str(fb.author)) author = fb.author;
      if (Array.isArray(fb.steps)) steps = fb.steps;
      if (Array.isArray(fb.items)) annotations = [...fb.items];
      if (fb.comparison && typeof fb.comparison === "object") comparison = fb.comparison;
      if (str(fb.receipt_digest)) receiptDigest = fb.receipt_digest;
      staleLocked = false;
      for (const btn of decisionButtons) btn.disabled = false;
      drawHeader();
    };

    const stepsList = element("ol", undefined, "cairn-plan-steps");
    const renderSteps = () => {
      stepsList.replaceChildren();
      if (!steps.length) {
        stepsList.append(element("li", "No plan steps recorded for this revision.", "small"));
        return;
      }
      for (const [idx, step] of steps.entries()) {
        const stepId = str(step?.id) || `step-${idx + 1}`;
        const stepText = str(step?.text) || (typeof step === "string" ? step : "");
        const li = element("li", undefined, "cairn-plan-step");
        li.dataset.stepId = stepId;
        const stepHead = element("div", undefined, "cairn-plan-step-head");
        stepHead.append(element("code", stepId, "cairn-short"), element("span", stepText));
        if (comparison) {
          if (list(comparison.added).includes(stepId)) stepHead.append(badge(word("Added", "info")));
          else if (list(comparison.changed).includes(stepId)) stepHead.append(badge(word("Changed", "warning")));
        }
        li.append(stepHead);

        const stepAnns = annotations.filter((a) => annotationMatchesStep(a, stepId));
        if (stepAnns.length) {
          const annList = element("ul", undefined, "cairn-plan-annotations");
          for (const ann of stepAnns) {
            const annLi = element("li", undefined, "cairn-plan-annotation");
            annLi.dataset.blocking = String(Boolean(ann.blocking));
            const badgesNode = badges(
              badge(word(ann.blocking ? "Blocking" : "Note", ann.blocking ? "failure" : "info")),
              ann.resolved_in ? badge(word(`Resolved in r${ann.resolved_in}`, "good")) : null,
            );
            annLi.append(
              badgesNode,
              element("span", `${str(ann.reviewer) || "reviewer"} (r${ann.revision ?? revision ?? "?"} · ${formatAnnotationRegion(ann)}): `, "small"),
              element("span", formatAnnotationNote(ann)),
            );
            if (str(ann.quoted)) annLi.append(element("q", ann.quoted, "small"));
            annList.append(annLi);
          }
          li.append(annList);
        }

        if (canOp && workId && planId && typeof apiFn === "function") {
          const fbDetails = element("details", undefined, "cairn-plan-feedback-box");
          fbDetails.append(element("summary", "Annotate step"));
          const form = element("form", undefined, "cairn-plan-feedback-form");
          const input = element("input");
          Object.assign(input, { type: "text", placeholder: `Feedback on ${stepId}`, maxLength: 2048, required: true, autocomplete: "off" });
          input.setAttribute("aria-label", `Feedback on step ${stepId}`);
          const kindSelect = element("select");
          kindSelect.setAttribute("aria-label", `Feedback kind for ${stepId}`);
          for (const [val, lbl] of [["comment", "Comment"], ["replace", "Suggest replacement"], ["delete", "Request deletion"]]) {
            const opt = element("option", lbl);
            opt.value = val;
            kindSelect.append(opt);
          }
          const blockingLabel = element("label", undefined, "small");
          const blockingCheck = element("input");
          blockingCheck.type = "checkbox";
          blockingLabel.append(blockingCheck, element("span", "Blocking"));
          const submitBtn = element("button", "Add feedback", "quiet");
          submitBtn.type = "submit";
          form.append(kindSelect, input, blockingLabel, submitBtn);
          form.addEventListener("submit", async (event) => {
            event.preventDefault();
            const comment = input.value.trim();
            if (!comment && kindSelect.value !== "delete") return;
            if (staleLocked) return;
            submitBtn.disabled = true;
            try {
              const reqBody = {
                work_id: workId,
                plan_id: planId,
                ...(revision !== null ? { expected_revision: revision } : {}),
                action: "annotate",
                step_id: stepId,
                ...(comment ? { comment } : {}),
                note_kind: kindSelect.value,
                blocking: blockingCheck.checked,
                request_id: newRequestId(),
              };
              const res = await apiFn("/api/implementation/plan-feedback", { method: "POST", body: reqBody, quiet: true });
              if (Array.isArray(res?.items)) {
                applyFeedbackSnapshot(res);
              } else {
                annotations.push({
                  annotation: str(res?.annotation_id) || `local-${Date.now()}`,
                  step_id: stepId,
                  region: { kind: "step", step: stepId },
                  note: kindSelect.value === "delete" ? { kind: "delete" } : { kind: kindSelect.value, text: comment },
                  blocking: blockingCheck.checked,
                  reviewer: str(res?.reviewer) || "operator",
                  revision: res?.revision ?? revision,
                });
                if (str(res?.receipt_digest)) {
                  receiptDigest = res.receipt_digest;
                  drawHeader();
                }
              }
              input.value = "";
              blockingCheck.checked = false;
              setStatus(`Added step feedback on ${stepId}.`);
              renderSteps();
              opts.onFeedback?.(res);
            } catch (error) {
              const isConflict = error?.status === 409 || /implementation_plan_revision|revision|stale|conflict/i.test(str(error?.message));
              if (isConflict) {
                setError(`Stale plan revision mismatch (409): ${explain(error)} Inspect the new plan revision before annotating.`, true);
              } else {
                setError(`Feedback refused: ${explain(error)}`);
              }
            } finally {
              submitBtn.disabled = false;
            }
          });
          fbDetails.append(form);
          li.append(fbDetails);
        }
        stepsList.append(li);
      }
      const unattached = annotations.filter((a) => !steps.some((s, i) => annotationMatchesStep(a, str(s?.id) || `step-${i + 1}`)));
      if (unattached.length) {
        const extraLi = element("li", undefined, "cairn-plan-step");
        extraLi.append(element("strong", "Unresolved / general annotations"));
        const annList = element("ul", undefined, "cairn-plan-annotations");
        for (const ann of unattached) {
          annList.append(element("li", `${str(ann.reviewer) || "reviewer"} (${formatAnnotationRegion(ann)}): ${formatAnnotationNote(ann)}`));
        }
        extraLi.append(annList);
        stepsList.append(extraLi);
      }
    };
    renderSteps();
    panel.append(stepsList, statusBanner, errorBanner);

    if (canOp && workId && planId && (typeof apiFn === "function" || typeof opts.onDecision === "function")) {
      const actions = element("div", undefined, "cluster cairn-plan-actions");
      const submitDecision = async (verdict, btn) => {
        if (staleLocked) return;
        btn.disabled = true;
        try {
          const reqBody = {
            work_id: workId,
            plan_id: planId,
            ...(revision !== null ? { expected_revision: revision } : {}),
            verdict,
            request_id: newRequestId(),
          };
          const res = typeof opts.onDecision === "function"
            ? await opts.onDecision(verdict, reqBody)
            : await apiFn("/api/implementation/plan-review", { method: "POST", body: reqBody, quiet: true });
          const nextDigest = str(res?.receipt_digest || res?.receipt || res?.plan_id || planId);
          if (nextDigest) receiptDigest = nextDigest;
          if (str(res?.gate)) gate = res.gate;
          if (res?.feedback && typeof res.feedback === "object") applyFeedbackSnapshot(res.feedback);
          else drawHeader();
          setStatus(`Recorded plan decision (${verdict})${receiptDigest ? ` · receipt ${short(receiptDigest)}` : ""}.`);
          await opts.onReviewed?.(verdict, res);
        } catch (error) {
          const isConflict = error?.status === 409 || /implementation_plan_revision|revision|stale|conflict/i.test(str(error?.message));
          if (isConflict) {
            setError(`Stale plan revision mismatch (409): plan revision changed concurrently${revision !== null ? ` (expected revision ${revision})` : ""}. Inspect the new plan revision before deciding.`, true);
          } else {
            setError(`Plan decision refused: ${explain(error)}`);
          }
        } finally {
          if (!staleLocked) btn.disabled = false;
        }
      };
      const acceptBtn = element("button", "Accept plan", "primary");
      acceptBtn.type = "button";
      acceptBtn.addEventListener("click", () => { void submitDecision("accepted", acceptBtn); });
      const changesBtn = element("button", "Request changes", "quiet");
      changesBtn.type = "button";
      changesBtn.addEventListener("click", () => { void submitDecision("changes_requested", changesBtn); });
      decisionButtons.push(acceptBtn, changesBtn);
      actions.append(acceptBtn, changesBtn);
      if (HEX.test(workId) && typeof apiFn === "function") {
        const refreshBtn = element("button", "Reload latest plan", "quiet");
        refreshBtn.type = "button";
        refreshBtn.addEventListener("click", async () => {
          refreshBtn.disabled = true;
          try {
            const fb = await apiFn(`/api/implementation/plan-feedback?work=${encodeURIComponent(workId)}`, { quiet: true });
            applyFeedbackSnapshot(fb);
            renderSteps();
            setStatus(`Loaded latest plan revision ${revision ?? "?"}.`);
          } catch (error) {
            setError(`Unable to reload plan: ${explain(error)}`);
          } finally {
            refreshBtn.disabled = false;
          }
        });
        actions.append(refreshBtn);
      }
      panel.append(actions);
    } else if (!canOp) {
      panel.append(element("p", "Read-only: operator authority is required to annotate or decide on this plan.", "small"));
    }
    container.replaceChildren(panel);
    return panel;
  }

  // r[impl kiln.cairn.gate_passport_surface]
  function attemptLifecycleControls(attempt) {
    const wrap = element("div", undefined, "cairn-lifecycle-controls");
    const canOp = context?.canOperate ? Boolean(context.canOperate()) : false;
    const candidateId = attempt.candidate_identity || attempt.candidate || attempt.verification?.candidate || null;
    const candidateDisplay = typeof candidateId === "string"
      ? candidateId
      : str(candidateId?.native_id);
    const workId = str(attempt.work_identity);
    const attemptId = str(attempt.identity);
    const fence = count(attempt.fence);
    const isCurrent = attempt.applicability === "current";
    const crossedLifecycle = attempt.lifecycle && attempt.lifecycle.binding !== "matches_attempt";
    const statusNode = element("p", undefined, "small cairn-lifecycle-status");
    statusNode.setAttribute("role", "status");
    statusNode.hidden = true;
    const errorNode = element("p", undefined, "warning error-state cairn-lifecycle-error");
    errorNode.setAttribute("role", "alert");
    errorNode.hidden = true;

    let failClosedReason = "";
    if (!canOp) failClosedReason = "Operator authority required: publication and integration recovery actions are disabled.";
    else if (!isCurrent) failClosedReason = "Stale/historical attempt: candidate is superseded; publication and integration recovery are blocked.";
    else if (!candidateDisplay) failClosedReason = "Missing candidate identity: no candidate is bound to this attempt, failing closed.";
    else if (!workId || !attemptId || fence === null) failClosedReason = "Missing exact work/attempt/fence identity: failing closed.";
    else if (crossedLifecycle) failClosedReason = "Crossed lifecycle receipt detected: failing closed until reconciled.";

    if (failClosedReason) {
      const diag = element("p", failClosedReason, "small cairn-lifecycle-diagnostic");
      diag.dataset.failClosed = "true";
      wrap.append(diag);
    }

    const row = element("div", undefined, "cluster cairn-lifecycle-buttons");
    for (const spec of RECOVERY_LIFECYCLE_ACTIONS) {
      const btn = element("button", spec.label, spec.danger ? "quiet danger" : "quiet");
      btn.type = "button";
      btn.dataset.lifecycleAction = spec.action;
      if (failClosedReason) btn.disabled = true;
      btn.addEventListener("click", async () => {
        if (failClosedReason || typeof context?.api !== "function") return;
        const cli = `kiln-implementer ${spec.action} --work ${workId} --attempt ${attemptId} --fence ${fence} --candidate ${candidateDisplay}`;
        const choice = await confirmCairnControl({
          title: `${spec.label} (${spec.tier})?`,
          verb: spec.label,
          dismiss: "Keep unchanged",
          danger: spec.danger,
          warning: spec.warning,
          facts: [
            ["Action", spec.action],
            ["Work ID", workId],
            ["Attempt / Fence", `${attemptId} / ${fence}`],
            ["Exact candidate", candidateDisplay],
          ],
          reason: { label: "Operator reason", placeholder: `Why ${spec.action} is requested`, required: true, maxLength: 1024 },
          cliCommand: cli,
          opener: btn,
        });
        if (!choice?.confirmed) {
          if (choice?.unavailable) {
            errorNode.textContent = "Confirmation dialog unavailable; failing closed without mutating.";
            errorNode.hidden = false;
          }
          return;
        }
        const reason = str(choice.reason).trim();
        if (!reason) {
          errorNode.textContent = "An explicit operator reason is required.";
          errorNode.hidden = false;
          return;
        }
        btn.disabled = true;
        errorNode.hidden = true;
        statusNode.textContent = `Sending ${spec.action}…`;
        statusNode.hidden = false;
        try {
          const res = await context.api(`/api/implementation/lifecycle/${spec.action}`, {
            method: "POST",
            body: {
              work_id: workId,
              attempt_id: attemptId,
              fence,
              candidate_identity: candidateId,
              reason,
              request_id: choice.requestId || newRequestId(),
            },
          });
          const dig = str(res?.publication_digest || res?.receipt?.native_id || res?.receipt);
          statusNode.textContent = `${spec.label} ${res?.already_recorded ? "already recorded" : "recorded"}${dig ? ` · ${short(dig)}` : ""}.`;
        } catch (error) {
          statusNode.hidden = true;
          const cliParity = str(error?.cli_parity || error?.payload?.cli_parity);
          errorNode.textContent = `${spec.label} refused (${error?.status || "error"}): ${explain(error)}${cliParity ? ` CLI parity: ${cliParity}` : ""}`;
          errorNode.hidden = false;
        } finally {
          btn.disabled = Boolean(failClosedReason);
        }
      });
      row.append(btn);
    }
    wrap.append(row, statusNode, errorNode);
    return wrap;
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
    if (context?.canOperate !== undefined || attempt.candidate || attempt.candidate_identity) {
      item.append(attemptLifecycleControls(attempt));
    }
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
      const planReviewData = work.plan_review || unit.plan_review || detail.plan_reviews?.[work.identity];
      if (planReviewData && typeof planReviewData === "object") {
        const planBox = element("div", undefined, "cairn-work-plan-review");
        renderPlanReview(planBox, {
          workId: work.identity,
          planId: planReviewData.plan_id || planReviewData.plan,
          revision: planReviewData.revision,
          gate: planReviewData.gate,
          author: planReviewData.author,
          steps: planReviewData.steps,
          annotations: planReviewData.items || planReviewData.annotations,
          comparison: planReviewData.comparison,
          receiptDigest: planReviewData.receipt_digest,
        });
        head.append(planBox);
      } else if (list(work.block_reasons).includes("plan_not_accepted") && typeof context?.api === "function" && HEX.test(str(work.identity))) {
        const planBox = element("div", undefined, "cairn-work-plan-review");
        const loadPlanBtn = element("button", "Inspect & review implementation plan", "quiet");
        loadPlanBtn.type = "button";
        loadPlanBtn.addEventListener("click", async () => {
          loadPlanBtn.disabled = true;
          try {
            const fb = await context.api(`/api/implementation/plan-feedback?work=${encodeURIComponent(work.identity)}`, { quiet: true });
            renderPlanReview(planBox, {
              workId: fb.work_id || work.identity,
              planId: fb.plan_id,
              revision: fb.revision,
              gate: fb.gate,
              author: fb.author,
              steps: fb.steps,
              annotations: fb.items,
              comparison: fb.comparison,
              receiptDigest: fb.receipt_digest,
            });
          } catch (error) {
            loadPlanBtn.disabled = false;
            planBox.replaceChildren(loadPlanBtn, errorState(`Plan review unavailable: ${explain(error)}`));
          }
        });
        planBox.append(loadPlanBtn);
        head.append(planBox);
      }
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
  const ATLAS_MAX_NODES = 64;
  const ATLAS_MAX_ROUTES = 128;
  const ATLAS_MAX_CHAPTERS = 16;
  const ATLAS_MAX_OPEN_QUESTIONS = 64;
  const ATLAS_MAX_PACKETS = 16;
  const ATLAS_MAX_HOPS_PER_PACKET = 32;
  const ATLAS_MAX_PAYLOAD_BYTES = 16 * 1024;
  const ATLAS_DECLARED_QUALIFIER = "Representative payload: design-implied, not measured traffic";

  function admitAtlasDocument(doc) {
    if (!doc || typeof doc !== "object" || Array.isArray(doc) || !Array.isArray(doc.nodes) || !Array.isArray(doc.routes)) {
      return { ok: false, reason: "atlas_document_malformed", message: "Atlas document must declare nodes and routes arrays." };
    }
    if (doc.nodes.length > ATLAS_MAX_NODES) {
      return { ok: false, reason: "atlas_node_bound_exceeded", limit: ATLAS_MAX_NODES, actual: doc.nodes.length, message: `Atlas projection has ${doc.nodes.length} nodes, exceeding the strict ${ATLAS_MAX_NODES}-node cap. Graph rendering refused; showing bounded summary table.` };
    }
    if (doc.routes.length > ATLAS_MAX_ROUTES) {
      return { ok: false, reason: "atlas_route_bound_exceeded", limit: ATLAS_MAX_ROUTES, actual: doc.routes.length, message: `Atlas projection has ${doc.routes.length} routes, exceeding the strict ${ATLAS_MAX_ROUTES}-route cap.` };
    }
    const nodeIds = new Set();
    const nodes = [];
    for (const raw of doc.nodes) {
      const id = str(raw?.id);
      if (!id || nodeIds.has(id)) {
        return { ok: false, reason: "atlas_document_malformed", message: id ? `Duplicate Atlas node id: ${id}` : "Atlas node missing unique id." };
      }
      nodeIds.add(id);
      nodes.push({
        id,
        label: str(raw.label) || id,
        role: str(raw.role) || "node",
        lane: str(raw.lane),
        kind: str(raw.kind) || "unit",
        link: str(raw.link),
        chapter: str(raw.chapter),
      });
    }
    const routeKeys = new Set();
    const routes = [];
    for (const raw of doc.routes) {
      const from = str(raw?.from);
      const to = str(raw?.to);
      if (!nodeIds.has(from) || !nodeIds.has(to)) {
        return { ok: false, reason: "atlas_document_malformed", message: `Atlas route references unknown endpoint (${from || "?"} -> ${to || "?"}).` };
      }
      const provenance = raw.provenance === "observed" ? "observed" : raw.provenance === "declared" ? "declared" : "";
      if (!provenance) {
        return { ok: false, reason: "atlas_document_malformed", message: `Atlas route ${from} -> ${to} has invalid provenance.` };
      }
      const key = `${from}\n${to}\n${str(raw.label)}`;
      routeKeys.add(`${from}\n${to}`);
      routes.push({
        key,
        from,
        to,
        label: str(raw.label) || (provenance === "observed" ? "observed execution" : "declared dependency"),
        provenance,
        evidence_ref: str(raw.evidence_ref),
        digest: str(raw.digest),
      });
    }
    const chapters = list(doc.chapters);
    if (chapters.length > ATLAS_MAX_CHAPTERS) {
      return { ok: false, reason: "atlas_chapter_bound_exceeded", limit: ATLAS_MAX_CHAPTERS, actual: chapters.length, message: `Atlas chapters (${chapters.length}) exceed ${ATLAS_MAX_CHAPTERS}.` };
    }
    for (const ch of chapters) {
      for (const nid of list(ch?.nodes)) if (!nodeIds.has(str(nid))) return { ok: false, reason: "atlas_document_malformed", message: `Chapter references unknown node ${nid}.` };
    }
    const openQuestions = list(doc.open_questions);
    if (openQuestions.length > ATLAS_MAX_OPEN_QUESTIONS) {
      return { ok: false, reason: "atlas_question_bound_exceeded", limit: ATLAS_MAX_OPEN_QUESTIONS, actual: openQuestions.length, message: `Open questions (${openQuestions.length}) exceed ${ATLAS_MAX_OPEN_QUESTIONS}.` };
    }
    const packets = list(doc.packets);
    if (packets.length > ATLAS_MAX_PACKETS) {
      return { ok: false, reason: "atlas_packet_bound_exceeded", limit: ATLAS_MAX_PACKETS, actual: packets.length, message: `Atlas packets (${packets.length}) exceed ${ATLAS_MAX_PACKETS}.` };
    }
    for (const pkt of packets) {
      const hops = list(pkt?.hops);
      if (hops.length > ATLAS_MAX_HOPS_PER_PACKET) {
        return { ok: false, reason: "atlas_hop_bound_exceeded", limit: ATLAS_MAX_HOPS_PER_PACKET, actual: hops.length, message: `Packet hops (${hops.length}) exceed ${ATLAS_MAX_HOPS_PER_PACKET}.` };
      }
      for (const hop of hops) {
        if (!routeKeys.has(`${str(hop?.from)}\n${str(hop?.to)}`)) {
          return { ok: false, reason: "atlas_document_malformed", message: `Packet hop ${str(hop?.from)} -> ${str(hop?.to)} is off declared routes.` };
        }
        const payloadText = typeof hop?.payload === "string" ? hop.payload : JSON.stringify(hop?.payload ?? "");
        if (utf8Bytes(payloadText) > ATLAS_MAX_PAYLOAD_BYTES) {
          return { ok: false, reason: "atlas_payload_bound_exceeded", limit: ATLAS_MAX_PAYLOAD_BYTES, actual: utf8Bytes(payloadText), message: `Packet hop payload exceeds ${ATLAS_MAX_PAYLOAD_BYTES} bytes.` };
        }
        if (hop?.provenance !== "observed" && hop?.provenance !== "declared") {
          return { ok: false, reason: "atlas_document_malformed", message: "Packet hop payload must declare provenance: observed or declared." };
        }
      }
    }
    return {
      ok: true,
      document: {
        identity: str(doc.identity) || "derived-projection",
        title: str(doc.title) || "Atlas topology",
        nodes,
        routes,
        chapters,
        open_questions: openQuestions,
        packets,
      },
    };
  }

  function buildCairnAtlas(detail) {
    const graph = dependencyGraph(detail);
    const nodes = graph.nodes.map((n) => ({
      id: n.id,
      label: n.label,
      role: n.kind === "unit" ? (n.lane || "unit") : n.kind,
      lane: n.lane,
      kind: n.kind,
    }));
    const routes = graph.edges.map((e) => ({
      from: e.from,
      to: e.to,
      label: "depends_on",
      provenance: "declared",
    }));
    const packets = [];
    for (const unit of list(detail?.units)) {
      const unitId = `unit:${detail?.repository_identity || "?"}/${unit?.native_id}`;
      for (const att of list(unit?.attempts)) {
        if (att?.verification?.run_id && att?.work_identity) {
          const runNodeId = `run:${att.verification.run_id}`;
          if (!nodes.some((n) => n.id === runNodeId)) {
            nodes.push({ id: runNodeId, label: `Run ${short(att.verification.run_id)}`, role: att.verification.success ? "verified" : "failed", lane: att.verification.success ? "verified" : "attention", kind: "observed_run" });
          }
          if (nodes.some((n) => n.id === unitId)) {
            routes.push({ from: unitId, to: runNodeId, label: "observed verification", provenance: "observed", evidence_ref: att.verification.run_id, digest: att.identity });
            packets.push({
              id: `verify:${att.identity}`,
              label: `Attempt ${att.ordinal ?? "?"} -> ${short(att.verification.run_id)}`,
              hops: [{
                from: unitId,
                to: runNodeId,
                provenance: "observed",
                observation_id: `run:${att.verification.run_id} / attempt:${att.identity}`,
                digest: att.identity,
                payload: { success: att.verification.success, checks: list(att.verification.checks).length },
              }],
            });
          }
        }
      }
    }
    return {
      identity: str(detail?.identity) || "cairn-change-atlas",
      title: `${str(detail?.change) || "Change"} topology`,
      nodes,
      routes,
      packets: packets.slice(0, ATLAS_MAX_PACKETS),
      chapters: [],
      open_questions: [],
    };
  }

  function renderAtlasBoundedFallback(doc, admission) {
    const wrap = element("section", undefined, "cairn-atlas-bounded-fallback");
    wrap.setAttribute("role", "status");
    const heading = element("h4", `Atlas topology graph refused: ${ATLAS_MAX_NODES}-node bound exceeded`, "cairn-subhead");
    const note = element("p", admission.message, "warning");
    note.dataset.atlasRefusal = admission.reason;
    wrap.append(heading, note);
    const rawNodes = Array.isArray(doc?.nodes) ? doc.nodes : [];
    if (rawNodes.length) {
      const tableWrap = element("div", undefined, "data-table-wrap");
      const tbl = element("table", undefined, "data-table data-table-stack cairn-atlas-summary-table");
      const thead = element("thead");
      const hr = element("tr");
      for (const title of ["Node ID", "Label", "Role"]) {
        const th = element("th", title); th.scope = "col"; hr.append(th);
      }
      thead.append(hr);
      const tbody = element("tbody");
      for (const raw of rawNodes.slice(0, ATLAS_MAX_NODES)) {
        const tr = element("tr");
        tr.append(element("td", str(raw?.id) || "unknown"), element("td", str(raw?.label || raw?.id) || "unknown"), element("td", str(raw?.role || raw?.kind) || "node"));
        tbody.append(tr);
      }
      tbl.append(thead, tbody);
      tableWrap.append(tbl);
      wrap.append(tableWrap);
      if (rawNodes.length > ATLAS_MAX_NODES) {
        wrap.append(element("p", `Bounded summary shows ${ATLAS_MAX_NODES} of ${rawNodes.length} nodes; ${rawNodes.length - ATLAS_MAX_NODES} omitted under the 64-node cap.`, "small"));
      }
    }
    return wrap;
  }

  function renderAtlasTopology(doc, options = {}) {
    const admission = admitAtlasDocument(doc);
    if (!admission.ok) {
      return renderAtlasBoundedFallback(doc, admission);
    }
    const atlas = admission.document;
    const panel = element("section", undefined, "cairn-atlas");
    panel.dataset.atlasIdentity = atlas.identity;
    const head = element("div", undefined, "cairn-atlas-head");
    head.append(
      element("h4", `${atlas.title} · Opt-in Atlas (${atlas.nodes.length}/${ATLAS_MAX_NODES} nodes, ${atlas.routes.length}/${ATLAS_MAX_ROUTES} routes)`, "cairn-subhead"),
      element("p", `Document identity: ${atlas.identity}. Declared topology is context, not runtime evidence or dispatch authority.`, "small"),
    );
    const legend = element("div", undefined, "cairn-atlas-legend");
    const obsBadge = element("span", "Observed (solid)", "state-badge cairn-atlas-provenance-observed");
    const decBadge = element("span", "Declared (dashed)", "state-badge cairn-atlas-provenance-declared");
    legend.append(obsBadge, decBadge);
    const modeGroup = element("div", undefined, "segmented cairn-atlas-modes");
    modeGroup.setAttribute("role", "group");
    modeGroup.setAttribute("aria-label", "Atlas layout mode");
    let viewMode = options.flat ? "flat" : "canvas";
    const canvasBtn = element("button", "Canvas graph"); canvasBtn.type = "button";
    const flatBtn = element("button", "Flat graph"); flatBtn.type = "button";
    modeGroup.append(canvasBtn, flatBtn);
    panel.append(head, legend, modeGroup);

    let pinnedId = "";
    let activeChapter = "";
    const graphHost = element("div", undefined, "cairn-atlas-host-inner");
    const inspectorHost = element("div", undefined, "cairn-atlas-inspector");

    const isRevealed = (nodeId) => {
      if (!activeChapter || !atlas.chapters.length) return true;
      const idx = atlas.chapters.findIndex((c) => str(c?.id || c?.title) === activeChapter);
      if (idx < 0) return true;
      return atlas.chapters.slice(0, idx + 1).some((c) => list(c.nodes).includes(nodeId));
    };

    const drawAtlasEdges = (canvas, buttons) => {
      if (!canvas.isConnected) return;
      canvas.querySelector("svg")?.remove();
      const svg = document.createElementNS(SVG_NS, "svg");
      svg.setAttribute("width", String(canvas.scrollWidth || 800));
      svg.setAttribute("height", String(canvas.scrollHeight || 400));
      svg.setAttribute("aria-hidden", "true");
      for (const route of atlas.routes) {
        const source = buttons.get(route.from);
        const target = buttons.get(route.to);
        if (!source || !target) continue;
        const startX = (source.offsetLeft || 0) + (source.parentElement?.offsetLeft || 0) + (source.offsetWidth || 120);
        const startY = (source.offsetTop || 0) + (source.parentElement?.offsetTop || 0) + (source.offsetHeight || 40) / 2;
        const endX = (target.offsetLeft || 0) + (target.parentElement?.offsetLeft || 0);
        const endY = (target.offsetTop || 0) + (target.parentElement?.offsetTop || 0) + (target.offsetHeight || 40) / 2;
        const middle = (startX + endX) / 2;
        const path = document.createElementNS(SVG_NS, "path");
        path.setAttribute("d", `M${startX},${startY} C${middle},${startY} ${middle},${endY} ${endX},${endY}`);
        path.setAttribute("class", `dag-edge cairn-atlas-edge cairn-atlas-edge-${route.provenance}`);
        path.dataset.provenance = route.provenance;
        if (route.provenance === "declared") path.setAttribute("stroke-dasharray", "6 4");
        svg.append(path);
      }
      canvas.prepend(svg);
    };

    const renderView = () => {
      canvasBtn.setAttribute("aria-pressed", String(viewMode === "canvas"));
      flatBtn.setAttribute("aria-pressed", String(viewMode === "flat"));
      if (viewMode === "flat") {
        const tableWrap = element("div", undefined, "data-table-wrap");
        const tbl = element("table", undefined, "data-table data-table-stack cairn-atlas-flat-table");
        const thead = element("thead");
        const hr = element("tr");
        for (const h of ["From", "To", "Route", "Provenance", "Evidence / Digest"]) {
          const th = element("th", h); th.scope = "col"; hr.append(th);
        }
        thead.append(hr);
        const tbody = element("tbody");
        for (const r of atlas.routes) {
          if (pinnedId && r.from !== pinnedId && r.to !== pinnedId) continue;
          const tr = element("tr");
          tr.dataset.provenance = r.provenance;
          const prov = element("span", r.provenance === "observed" ? "Observed" : "Declared", `state-badge cairn-atlas-provenance-${r.provenance}`);
          const provTd = element("td"); provTd.append(prov);
          tr.append(element("td", r.from), element("td", r.to), element("td", r.label), provTd, element("td", r.provenance === "observed" ? (r.digest || r.evidence_ref || "Unavailable") : ATLAS_DECLARED_QUALIFIER));
          tbody.append(tr);
        }
        if (!atlas.routes.length) {
          const tr = element("tr");
          const td = element("td", "No routes declared in this Atlas document.");
          td.colSpan = 5;
          tr.append(td);
          tbody.append(tr);
        }
        tbl.append(thead, tbody);
        tableWrap.append(tbl);
        graphHost.replaceChildren(tableWrap);
        return;
      }
      // Compute topological depth for canvas columns.
      const incoming = new Map(atlas.nodes.map((n) => [n.id, 0]));
      const outgoing = new Map(atlas.nodes.map((n) => [n.id, []]));
      for (const r of atlas.routes) {
        incoming.set(r.to, (incoming.get(r.to) || 0) + 1);
        outgoing.get(r.from).push(r.to);
      }
      const depth = new Map();
      const q = atlas.nodes.map((n) => n.id).filter((id) => incoming.get(id) === 0);
      for (const id of q) depth.set(id, 0);
      for (let i = 0; i < q.length; i += 1) {
        const id = q[i];
        for (const next of outgoing.get(id)) {
          depth.set(next, Math.max(depth.get(next) || 0, (depth.get(id) || 0) + 1));
          incoming.set(next, incoming.get(next) - 1);
          if (incoming.get(next) === 0) q.push(next);
        }
      }
      const placed = new Set(q);
      const maxD = Math.max(0, ...q.map((id) => depth.get(id) || 0));
      const cols = [];
      for (const n of atlas.nodes) {
        const level = placed.has(n.id) ? (depth.get(n.id) || 0) : maxD + 1;
        (cols[level] ||= []).push(n);
      }
      const scroller = element("div", undefined, "cairn-graph");
      const canvas = element("div", undefined, "dag-canvas cairn-atlas-canvas");
      const buttons = new Map();
      cols.filter(Boolean).forEach((colNodes, idx) => {
        const stage = element("div", undefined, "dag-stage");
        stage.append(element("h3", idx === 0 ? "Roots" : `Hop ${idx + 1}`));
        for (const n of colNodes) {
          const btn = element("button", undefined, "dag-node cairn-graph-node cairn-atlas-node");
          btn.type = "button";
          btn.dataset.nodeId = n.id;
          btn.dataset.revealed = String(isRevealed(n.id));
          if (pinnedId === n.id) btn.setAttribute("aria-current", "true");
          if (LANE_STATE[n.lane]) btn.dataset.state = LANE_STATE[n.lane];
          btn.append(element("strong", n.label), element("span", n.role, "small"));
          btn.addEventListener("click", () => {
            pinnedId = pinnedId === n.id ? "" : n.id;
            renderView();
          });
          buttons.set(n.id, btn);
          stage.append(btn);
        }
        canvas.append(stage);
      });
      scroller.append(canvas);
      graphHost.replaceChildren(scroller);
      if (typeof requestAnimationFrame === "function") requestAnimationFrame(() => drawAtlasEdges(canvas, buttons));
      else drawAtlasEdges(canvas, buttons);
    };

    canvasBtn.addEventListener("click", () => { viewMode = "canvas"; renderView(); });
    flatBtn.addEventListener("click", () => { viewMode = "flat"; renderView(); });

    if (atlas.chapters.length) {
      const chWrap = element("div", undefined, "cairn-chips cairn-atlas-chapters");
      const allCh = element("button", "All chapters", "chip"); allCh.type = "button";
      allCh.addEventListener("click", () => { activeChapter = ""; renderView(); });
      chWrap.append(allCh);
      for (const ch of atlas.chapters) {
        const chId = str(ch?.id || ch?.title);
        const btn = element("button", str(ch?.title || chId), "chip"); btn.type = "button";
        btn.addEventListener("click", () => { activeChapter = chId; renderView(); });
        chWrap.append(btn);
      }
      panel.append(chWrap);
    }

    // Pin list (keyboard/screen-reader index)
    const pinWrap = element("div", undefined, "cairn-atlas-pins");
    pinWrap.append(element("span", "Pin node: ", "small"));
    for (const [i, n] of atlas.nodes.entries()) {
      const pinBtn = element("button", `${String.fromCharCode(65 + (i % 26))} · ${n.label}`, "quiet cairn-atlas-pin");
      pinBtn.type = "button";
      pinBtn.addEventListener("click", () => { pinnedId = pinnedId === n.id ? "" : n.id; renderView(); });
      pinWrap.append(pinBtn);
    }
    panel.append(pinWrap, graphHost, inspectorHost);

    // Packet inspector (declared vs observed)
    if (atlas.packets.length) {
      const pktSection = element("div", undefined, "cairn-atlas-packets");
      pktSection.append(element("h5", "Packets & Hop Payloads"));
      for (const pkt of atlas.packets) {
        const pktCard = element("div", undefined, "cairn-atlas-packet");
        pktCard.append(element("strong", str(pkt.label || pkt.id)));
        const hopList = element("ol", undefined, "cairn-bullets");
        for (const hop of list(pkt.hops)) {
          const li = element("li", undefined, "cairn-atlas-hop");
          li.dataset.provenance = hop.provenance;
          const badgeEl = element("span", hop.provenance === "observed" ? "Observed" : "Declared", `state-badge cairn-atlas-provenance-${hop.provenance}`);
          li.append(badgeEl, document.createTextNode(` ${hop.from} → ${hop.to}: `));
          if (hop.provenance === "declared") {
            li.append(element("em", ATLAS_DECLARED_QUALIFIER), document.createTextNode(` · ${typeof hop.payload === "string" ? hop.payload : JSON.stringify(hop.payload)}`));
          } else {
            const obsId = str(hop.observation_id || hop.identity);
            const dig = str(hop.digest);
            if (!obsId && !dig) {
              li.append(element("span", "Evidence unavailable or expired", "not-recorded"));
            } else {
              li.append(element("code", `${obsId}${dig ? ` (${short(dig)})` : ""}`), document.createTextNode(` · ${typeof hop.payload === "string" ? hop.payload : JSON.stringify(hop.payload)}`));
            }
          }
          hopList.append(li);
        }
        pktCard.append(hopList);
        pktSection.append(pktCard);
      }
      panel.append(pktSection);
    }

    if (atlas.open_questions.length) {
      const qList = element("ul", undefined, "cairn-bullets cairn-atlas-questions");
      for (const q of atlas.open_questions) {
        qList.append(element("li", `[${str(q?.state) || "open"}] ${str(q?.id)}: ${str(q?.text)}`));
      }
      panel.append(element("h5", "Open questions"), qList);
    }

    // Deterministic prose twin
    const proseLines = [
      `Atlas document ${atlas.identity} (${atlas.nodes.length} nodes, ${atlas.routes.length} routes).`,
      ...atlas.routes.map((r) => `- ${r.from} -> ${r.to} (${r.label}) [${r.provenance === "declared" ? `declared: ${ATLAS_DECLARED_QUALIFIER}` : `observed: ${r.digest || r.evidence_ref || "unavailable"}`}]`),
    ];
    panel.append(disclosure("Prose twin (deterministic export)", "atlas-prose-twin", element("pre", proseLines.join("\n"), "cairn-atlas-prose")));
    renderView();
    return panel;
  }

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
    const atlasToggle = element("button", "Open opt-in Atlas topology (off by default)", "quiet cairn-atlas-toggle");
    atlasToggle.type = "button";
    atlasToggle.setAttribute("aria-expanded", "false");
    const atlasHost = element("div", undefined, "cairn-atlas-host");
    atlasHost.hidden = true;
    atlasToggle.addEventListener("click", () => {
      const nextOpen = atlasHost.hidden;
      atlasHost.hidden = !nextOpen;
      atlasToggle.setAttribute("aria-expanded", String(nextOpen));
      atlasToggle.textContent = nextOpen ? "Hide opt-in Atlas topology" : "Open opt-in Atlas topology (off by default)";
      if (nextOpen && !atlasHost.firstChild) {
        atlasHost.replaceChildren(renderAtlasTopology(buildCairnAtlas(detail)));
      }
    });
    if (!graph.edges.length) return section("Dependencies", "cairn-graph-title", element("p", "No dependency between this change's units, or on other work, is recorded.", "small"), atlasToggle, atlasHost);
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
    return section("Dependencies", "cairn-graph-title", legend, scroller, atlasToggle, atlasHost);
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
  // r[impl kiln.cairn.workspace.agents] Agent revisions, capability and wait reasons beside the change, from the resource
  // inventory admitted for this principal. Only fields the projection defines are drawn; anything else is ignored.
  function slotNode(slot) {
    const wrap = element("span", undefined, "cairn-slot");
    wrap.dataset.slotState = str(slot?.state) || "absent";
    wrap.append(badge(slotWord(slot)));
    if (slot?.state === "recorded") wrap.append(shortCode(slot.value));
    else if (slot?.state === "unknown" || slot?.state === "unpublished") wrap.append(element("span", str(slot.owner) ? `Owner ${slot.owner}` : "No owner named", "small cairn-owner"));
    return wrap;
  }
  function waitItems(waits) {
    return list(waits).map((wait) => `${str(wait?.code) || "code not recorded"} · ${str(wait?.subject) || "subject not recorded"} · owner ${str(wait?.owner) || "not named"}`);
  }
  function capabilityText(value) {
    const names = strings(value?.names);
    return value?.state === "declared" && names.length ? names.join(", ") : lookup(CAPABILITIES, value?.state).label;
  }
  function familyItem(family) {
    const item = element("li", undefined, "cairn-family");
    item.dataset.provenance = str(family?.qualification?.provenance) || "none";
    item.append(element("code", str(family?.task_family) || "family not recorded"), badge(familyProvenance(family)));
    if (family?.declared === false) item.append(badge(word("Not declared", "warning", "Qualified, but the definition does not declare this task family.")));
    const qualification = family?.qualification;
    if (qualification && typeof qualification === "object") item.append(shortCode(qualification.reference), element("span", str(qualification.evidence_class), "small"));
    return item;
  }
  function observationBadge(value) {
    return value === "current" ? null : badge(lookup(OBSERVATIONS, value), "Observation");
  }
  function definitionCard(definition) {
    const card = element("article", undefined, "cairn-member cairn-agent");
    const title = element("h4", undefined, "cairn-member-title");
    title.append(shortCode(definition?.reference?.id));
    const families = list(definition?.task_families);
    card.append(title, badges(badge(lookup(MANAGEMENT, definition?.management)), observationBadge(definition?.observation)), facts([
      ["Source", str(definition?.reference?.source)],
      ["Profile revision", shortCode(definition?.profile_revision)],
      ["Desired", slotNode(definition?.desired)],
      ["Prepared", slotNode(definition?.prepared)],
      ["Qualified", slotNode(definition?.qualified)],
      ["Selected for new sessions", slotNode(definition?.selected_for_new_sessions)],
      ["Rollout generation", count(definition?.rollout_generation) ?? "No recorded selection"],
      ["Task families and qualification", families.length ? bullets(families.map(familyItem), "") : lookup(CAPABILITIES, definition?.task_families_state).label],
      ["Tools", capabilityText(definition?.tools)],
      ["Harnesses", capabilityText(definition?.harnesses)],
      ["Output contracts", capabilityText(definition?.output_contracts)],
      ["Models", capabilityText(definition?.models)],
      ["Authority", lookup(PERMISSIONS, definition?.authority).label],
      ["Workers in this inventory", count(definition?.workers) ?? "Not recorded"],
      ["Wait reasons (code · subject · owner)", bullets(waitItems(definition?.wait_reasons), "None")],
    ]));
    return card;
  }
  function pinNode(pin) {
    if (pin?.state !== "recorded") return slotNode(pin);
    const wrap = element("span", undefined, "cairn-slot");
    wrap.dataset.slotState = "recorded";
    wrap.append(badge(lookup(PIN_STATUSES, pin.status)), element("span", "Qualification", "small"), shortCode(pin.qualification),
      element("span", `Rollout generation ${count(pin.rollout_generation) ?? "not recorded"}`, "small"), element("span", "Pin", "small"), shortCode(pin.identity));
    return wrap;
  }
  function workerCard(worker) {
    const card = element("article", undefined, "cairn-member cairn-agent-worker");
    card.dataset.work = str(worker?.work);
    const pin = worker?.pin && typeof worker.pin === "object" ? worker.pin : {};
    const title = element("h4", undefined, "cairn-member-title");
    title.append(shortCode(worker?.reference?.id));
    const rows = [
      ["Definition", shortCode(worker?.definition?.id)],
      ["Lifecycle", lookup(LIFECYCLES, worker?.lifecycle).label],
      ["Pinned revision", shortCode(worker?.pinned_revision)],
      ["Pin", pinNode(pin)],
    ];
    if (worker?.work === "this_change") rows.push(["Attempt on this change", shortCode(worker.attempt)], ["Fence", count(worker.fence) ?? "Not recorded"]);
    rows.push(["Wait reasons (code · subject · owner)", bullets(waitItems(worker?.wait_reasons), "None")]);
    card.append(title, badges(badge(lookup(WORK_BINDINGS, worker?.work)), badge(lookup(SELECTIONS, worker?.selection)), badge(slotWord(pin), "Pin"),
      pin.blocked === true ? badge(word("Pin blocked", "failure", "A further effect or export by this session would be refused.")) : null,
      observationBadge(worker?.observation)), facts(rows));
    return card;
  }
  function sourceLine(source) {
    return `${str(source?.source)} (${str(source?.kind)}): ${lookup(OBSERVATIONS, source?.state).label}${str(source?.reason) ? ` · ${source.reason}` : ""}`;
  }
  // r[impl kiln.cairn.workspace.agents.scenario.workflow] Workflow revisions are their own slot: each names its compiled
  // workflow and the worker revision it pairs with, apart from the agent and worker revisions. Sessions are counted, never named.
  // A slot or pin names a revision of the definition's `revisions`; an unlisted name reads as not listed, never as a match.
  function workflowRevisionNode(name, revisions) {
    const wrap = element("span", undefined, "cairn-slot");
    const revision = revisions.get(str(name));
    wrap.append(shortCode(str(name)));
    if (!revision) wrap.append(badge(word("Revision not listed", "warning", "The projection names a revision it does not list.")));
    else wrap.append(element("span", "Worker", "small"), shortCode(revision.worker?.revision), badge(lookup(WORKER_PAIRINGS, revision.worker?.observed)));
    return wrap;
  }
  function workflowSlotNode(slot, revisions) {
    const wrap = element("span", undefined, "cairn-slot");
    wrap.dataset.slotState = str(slot?.state) || "absent";
    wrap.append(badge(slotWord(slot)));
    if (slot?.state === "recorded" && str(slot.revision)) wrap.append(workflowRevisionNode(slot.revision, revisions));
    return wrap;
  }
  function workflowPinItem(pin, revisions) {
    const item = element("li", undefined, "cairn-workflow-pin");
    item.dataset.status = str(pin?.status);
    const sessions = count(pin?.sessions);
    item.append(element("span", sessions === null ? "Sessions not recorded" : plural(sessions, "session"), "small"),
      badge(lookup(PIN_STATUSES, pin?.status)), badge(lookup(SELECTIONS, pin?.selection)));
    if (pin?.blocked === true) item.append(badge(word("Pin blocked", "failure", "A further effect under this pin would be refused.")));
    if (str(pin?.revision)) item.append(workflowRevisionNode(pin.revision, revisions));
    return item;
  }
  // The listed revisions by identity; anything that is not a revision object is ignored.
  function workflowRevisions(definition) {
    return new Map(list(definition?.revisions).filter((revision) => revision && typeof revision === "object" && str(revision.revision))
      .map((revision) => [revision.revision, revision]));
  }
  // The identities of each listed revision, once each.
  function workflowIdentities(revisions) {
    return [...revisions.values()].map((revision) => {
      const workflow = revision.workflow && typeof revision.workflow === "object" ? revision.workflow : {};
      const worker = revision.worker && typeof revision.worker === "object" ? revision.worker : {};
      return disclosure(`Workflow revision ${short(revision.revision)}: identities`, `workflow-revision-${revision.revision}`, facts([
        ["Workflow revision", copyable(revision.revision, "Workflow revision")],
        ["Compiler", str(workflow.compiler_revision)],
        ["Compiler artifact", shortCode(workflow.compiler_artifact)],
        ["Plan", shortCode(workflow.plan)],
        ["Graph", shortCode(workflow.graph)],
        ["Cairn selection", shortCode(workflow.selection)],
        ["Cairn obligations", shortCode(workflow.obligations)],
        ["Policy", shortCode(workflow.policy)],
        ["Worker definition", shortCode(worker.definition?.id)],
        ["Worker revision", shortCode(worker.revision)],
        ["Worker bundle", shortCode(worker.bundle)],
      ]));
    });
  }
  function workflowCard(definition) {
    const card = element("article", undefined, "cairn-member cairn-workflow");
    const title = element("h4", undefined, "cairn-member-title");
    title.append(shortCode(definition?.reference?.id));
    const recorded = definition?.selected_for_new_sessions?.state === "recorded";
    const revisions = workflowRevisions(definition);
    card.append(title, badges(observationBadge(definition?.observation)), facts([
      ["Source", str(definition?.reference?.source)],
      ["Desired", workflowSlotNode(definition?.desired, revisions)],
      ["Last usable", workflowSlotNode(definition?.last_usable, revisions)],
      ["Selected for new sessions", workflowSlotNode(definition?.selected_for_new_sessions, revisions)],
      ["Rollout generation", recorded ? count(definition.rollout_generation) : "No recorded selection"],
      ["Selected by", recorded ? lookup(SELECTION_KINDS, definition.selection_kind).label : "No recorded selection"],
      ["Qualification reference", recorded ? shortCode(definition.qualification) : "No recorded selection"],
      ["Pinned sessions", definition?.pinned_state === "recorded"
        ? bullets(list(definition.pinned).map((pin) => workflowPinItem(pin, revisions)), "") : badge(slotWord({ state: definition?.pinned_state }))],
    ]), ...workflowIdentities(revisions));
    return card;
  }
  // r[impl kiln.cairn.workspace.agents.scenario.workflow_source] Missing or refused workflow revisions say so with their
  // code; they never hide the agent and worker revisions or read as no workflow.
  function workflowSection(workflow) {
    const children = [element("h4", "Workflow revisions", "cairn-subhead")];
    if (workflow.source !== "admitted") {
      const note = workflow.source === "not_configured"
        ? "No workflow revision source is configured. Workflow revisions are not known here; this is not proof that none exist."
        : `Workflow revisions are unavailable (${str(workflow.reason) || "reason not recorded"}). Nothing was substituted; agent and worker revisions are unaffected.`;
      const node = element("p", note, "warning cairn-workflow-source");
      node.dataset.state = workflow.source;
      children.push(node);
      return children;
    }
    const failing = list(workflow.sources).filter((source) => source?.state !== "current");
    if (failing.length) {
      const node = element("p", `Some workflow revision sources are not current, and their workflow definitions may be missing here: ${failing.map(sourceLine).join("; ")}.`,
        "warning cairn-workflow-source");
      node.dataset.state = "partial";
      children.push(node);
    }
    const cards = element("div", undefined, "cairn-members");
    cards.append(...workflow.definitions.map(workflowCard));
    children.push(workflow.definitions.length ? cards : element("p", "The workflow revision source lists no workflow definition.", "small"));
    return children;
  }
  function agentsSection(detail) {
    const agents = detail.agents;
    const workflow = agents.workflow_revisions;
    const children = [
      element("p", "Read-only revision, capability and wait facts from the resource inventory admitted for you. They are evidence, never dispatch, selection or promotion authority.", "small"),
    ];
    if (agents.source !== "admitted") {
      const note = agents.source === "not_configured"
        ? "No resource inventory is configured for this service. Agent, worker and workflow revisions are not known here; this is not proof that none exist."
        : `The resource inventory is unavailable (${str(agents.reason) || "reason not recorded"}). Agent, worker and workflow revisions are not shown, and nothing was substituted.`;
      const node = element("p", note, "warning cairn-agents-source");
      node.dataset.state = agents.source;
      children.push(node);
      return section("Agents and revisions", "cairn-agents-title", ...children);
    }
    if (agents.synthetic === true) children.push(element("p", "Synthetic inventory: fixture observations, not a live fleet.", "warning cairn-agents-source"));
    children.push(...workflowSection(workflow));
    children.push(element("h4", "Agent definitions", "cairn-subhead"));
    const definitions = element("div", undefined, "cairn-members");
    definitions.append(...list(agents.definitions).map(definitionCard));
    children.push(agents.definitions.length ? definitions : element("p", "This inventory lists no agent definition.", "small"));
    children.push(element("h4", "Worker revisions", "cairn-subhead"));
    const workers = element("div", undefined, "cairn-members");
    workers.append(...list(agents.workers).map(workerCard));
    children.push(agents.workers.length ? workers : element("p", "This inventory lists no worker instance. A definition is not a running worker.", "small"));
    children.push(disclosure("Inventory sources and identity", "agents-sources", facts([
      ["Inventory identity", copyable(agents.inventory_identity, "Inventory identity")],
      ["Evaluated", when(agents.evaluated_at_ms)],
      ["Agents and workers sources", bullets(list(agents.sources).map(sourceLine), "None listed")],
      ["Workflow revision sources", bullets(list(workflow.sources).map(sourceLine), "None configured")],
    ])));
    return section("Agents and revisions", "cairn-agents-title", ...children);
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
  // Read-only fleet context inside the selected change. Attempts join by exact work identity; claims are host-wide
  // because a claim scope names a repository path, not a Cairn native unit. Commands stay on the Fleet page.
  function fleetSection(detail) {
    const panel = element("section", undefined, "panel cairn-fleet");
    panel.append(element("h3", "Fleet coordination"), element("p", "Read-only claims and attempts from this host's fleet coordinator. Attempts join this change by exact work identity; claims are host-wide. Use the Fleet page to change them.", "small"));
    const fleet = model.fleet;
    if (!fleet || fleet.error) {
      panel.append(element("p", fleet?.error ? `Fleet coordination unavailable: ${explain(fleet.error)}` : "Fleet coordination was not read.", "small cairn-fleet-note"));
      return panel;
    }
    const work = new Set();
    for (const unit of detail.units || []) for (const attempt of unit.attempts || []) if (attempt.work_identity) work.add(attempt.work_identity);
    const claims = fleet.claims?.claims || [];
    const attempts = (fleet.attempts?.attempts || []).filter((attempt) => work.has(attempt.work));
    const counts = element("dl", undefined, "cairn-fleet-counts");
    counts.append(readout("Host claims", String(claims.length)), readout("Attempts on this change", String(attempts.length)));
    panel.append(counts);
    if (claims.length) {
      panel.append(element("h4", "Host claims", "cairn-subhead"));
      const listNode = element("ul", undefined, "cairn-fleet-list");
      for (const claim of claims.slice(0, 8)) {
        const item = element("li");
        const identity = str(claim.identity);
        item.append(element("code", identity.slice(0, 12)), element("span", fleetScope(claim.scope), "small"), element("span", `${str(claim.holder?.tool) || "?"} on ${str(claim.holder?.host) || "?"}`, "small"), element("span", str(claim.purpose), "small"));
        listNode.append(item);
      }
      panel.append(listNode);
      if (claims.length > 8) panel.append(element("p", `Showing 8 of ${claims.length} host claims. Open Fleet for the rest.`, "small"));
    }
    if (attempts.length) {
      panel.append(element("h4", "Fleet attempts on this change", "cairn-subhead"));
      const listNode = element("ul", undefined, "cairn-fleet-list");
      for (const attempt of attempts) {
        const item = element("li");
        const session = attempt.session || {};
        item.append(element("code", str(attempt.attempt).slice(0, 12)), element("span", `${str(attempt.state)} · ${str(attempt.phase)}`, "small"), element("span", `${str(session.machine) || "machine not recorded"} ${str(session.agent_name) || ""}`, "small"));
        listNode.append(item);
      }
      panel.append(listNode);
    } else if (!claims.length) {
      panel.append(element("p", "No live fleet claim or attempt is recorded for this host.", "small cairn-fleet-note"));
    }
    const linkNode = element("a", "Open Fleet", "button cairn-fleet-link");
    linkNode.href = routeHref({ page: "fleet" });
    panel.append(linkNode);
    return panel;
  }

  function fleetScope(scope) {
    if (!scope || typeof scope !== "object") return "Scope not recorded";
    const target = typeof scope.target === "string" ? scope.target : scope.target?.repository || scope.target?.machine || "";
    const path = typeof scope.path === "string" ? scope.path : scope.path?.path || "";
    return `${target || "scope not recorded"}${path && path !== "whole" ? ` ${path}` : ""}`;
  }

  let authoringPanel = null;
  const authoringName = (value) => typeof value === "string" && value.length <= 128
    && value !== "archive" && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value);

  function admitAuthoringSnapshot(value, repository) {
    const state = value?.snapshot;
    const graph = state?.graph;
    if (!HEX.test(repository) || value?.schema !== AUTHORING_SCHEMA || value.repository !== repository
      || !HEX.test(state?.revision) || graph?.command !== "change graph" || graph.layout !== "cairn"
      || typeof graph.valid !== "boolean" || !Array.isArray(graph.nodes) || !Array.isArray(graph.archived_changes)
      || graph.active_change_count !== graph.nodes.length) throw new Error("Invalid Cairn authoring readback");
    const nodes = new Map();
    for (const row of graph.nodes) {
      if (!authoringName(row?.name) || nodes.has(row.name) || !Array.isArray(row.dependencies)
        || !row.dependencies.every(authoringName) || new Set(row.dependencies).size !== row.dependencies.length) {
        throw new Error("Invalid Cairn authoring graph");
      }
      nodes.set(row.name, row.dependencies);
    }
    if (!graph.archived_changes.every(authoringName)) throw new Error("Invalid Cairn archive names");
    if (state.change != null && !nodes.has(state.change.name)) throw new Error("Cairn change does not match the graph");
    return { revision: state.revision, valid: graph.valid, nodes, archived: new Set(graph.archived_changes), change: state.change };
  }

  function authoringRequest(repository, snapshot, type, change, dependency) {
    if (!HEX.test(repository) || !HEX.test(snapshot?.revision) || !authoringName(change)) return null;
    const action = { type, change };
    if (type === "create") {
      if (!snapshot.valid || snapshot.nodes.has(change) || snapshot.archived.has(change)) return null;
    } else if (type === "add_dependency" || type === "remove_dependency") {
      const current = snapshot.nodes.get(change);
      if (!current || !authoringName(dependency) || change === dependency) return null;
      if (type === "add_dependency") {
        if (!snapshot.valid || current.includes(dependency) || !snapshot.nodes.has(dependency) && !snapshot.archived.has(dependency)) return null;
        const pending = [dependency];
        const seen = new Set();
        while (pending.length) {
          const node = pending.pop();
          if (node === change) return null;
          if (!seen.has(node)) { seen.add(node); for (const next of snapshot.nodes.get(node) || []) pending.push(next); }
        }
      } else if (!current.includes(dependency)) return null;
      action.dependency = dependency;
    } else return null;
    return { repository, expected_revision: snapshot.revision, action };
  }

  // r[impl kiln.sdlc.cairn_authoring] The canonical workspace selection never authorizes a native root.
  function nativeAuthoringPanel(route) {
    if (!context?.canOperate?.()) { authoringPanel = null; return null; }
    if (authoringPanel && sameSelection(authoringPanel.route, route)) return authoringPanel.node;
    const panel = element("details", undefined, "panel cairn-authoring");
    const status = element("p", "Open to read configured authoring roots.", "small");
    status.setAttribute("role", "status");
    const content = element("div");
    panel.append(element("summary", "Author Cairn changes"), status, content);
    authoringPanel = { route, node: panel };
    const live = () => authoringPanel?.node === panel && panel.isConnected && context?.canOperate?.()
      && sameSelection(route, normalizeRoute(context?.route));
    let enrolled, choice, refresh, snapshot, locked = false, readVersion = 0;
    let editArtifacts = false;
    const field = (label, control) => { const node = element("label", label); node.append(control); return node; };
    const option = (value, label = value) => { const node = element("option", label); node.value = value; return node; };
    const button = (label) => { const node = element("button", label, "button"); node.type = "button"; return node; };
    const editor = element("div");

    async function readGraph() {
      if (locked) return;
      const version = ++readVersion;
      const repository = choice.value;
      snapshot = null;
      editor.replaceChildren();
      if (!repository) { status.textContent = "Choose an explicitly enrolled repository."; return; }
      status.textContent = "Reading the native change graph…";
      try {
        const value = await context.api(`${AUTHORING_ROUTE}?repository=${repository}`, { quiet: true });
        if (!live() || readVersion !== version) return;
        snapshot = admitAuthoringSnapshot(value, repository);
        status.textContent = "Native graph read. Review the revision before writing.";
        renderControls(repository === route.repository ? route.change : "");
      } catch (error) {
        if (live() && readVersion === version) status.textContent = `Native graph unavailable: ${explain(error)}`;
      }
    }

    function artifactControls(repository, change) {
      const surface = element("div");
      const kind = element("select");
      kind.append(...["proposal", "design", "tasks"].map((id) => option(id)));
      const load = button("Open artifact");
      const report = element("p", "", "small");
      report.setAttribute("role", "status");
      const fields = element("div");
      const target = `${AUTHORING_ROUTE}/artifact`;
      const clear = () => { fields.replaceChildren(); report.textContent = ""; };
      change.addEventListener("change", clear);
      kind.addEventListener("change", clear);
      load.addEventListener("click", async () => {
        if (!live() || locked || !authoringName(change.value)) return;
        clear();
        const selected = change.value;
        const artifact = kind.value;
        const current = () => live() && surface.isConnected && repository === choice.value && selected === change.value && artifact === kind.value;
        load.disabled = true;
        try {
          const view = await context.api(`${target}?repository=${repository}&change=${selected}&artifact=${artifact}`, { quiet: true });
          if (!current()) return;
          const source = view?.artifact;
          if (view?.repository !== repository || !HEX.test(view.revision) || source?.change !== selected || source?.artifact !== artifact || !HEX.test(source.content_blake3) || typeof source.content !== "string") throw new Error("Invalid native artifact readback");
          const text = element("textarea");
          text.rows = 16;
          text.value = source.content;
          const baseline = text.value;
          const save = button("Save artifact");
          save.disabled = true;
          text.addEventListener("input", () => { save.disabled = text.value === baseline || new TextEncoder().encode(text.value).length > 48 * 1024; });
          fields.append(field("Artifact content", text), save);
          report.textContent = `Current artifact digest: ${source.content_blake3}`;
          save.addEventListener("click", async () => {
            if (!current() || locked || save.disabled) return;
            const content = text.value;
            locked = true;
            save.disabled = true;
            text.disabled = true;
            let submitted = false;
            try {
              const approval = await context.confirmControl({ title: "Replace Cairn artifact?", verb: "Save artifact", opener: save,
                facts: [["Change", selected], ["Artifact", artifact], ["Expected content", source.content_blake3]],
                warning: "Writes only the selected artifact. Does not approve gates or publish." });
              if (!approval.confirmed || !current()) return;
              submitted = true;
              const reply = await context.api(target, { method: "POST", quiet: true, body: {
                repository, change: selected, artifact, expected_revision: view.revision, expected_content_blake3: source.content_blake3, content,
              } });
              if (!current()) return;
              if (reply?.repository !== repository || reply?.change !== selected || reply?.artifact !== artifact || !["applied", "reconciled"].includes(reply.outcome) || !HEX.test(reply.content_blake3)) throw new Error("Replacement was not confirmed by native readback");
              report.textContent = `Cairn confirmed replacement: ${reply.content_blake3}. Refresh the graph before making other changes.`;
            } catch (error) {
              if (current()) report.textContent = `Artifact not confirmed: ${explain(error)}. Reopen it before writing again.`;
            } finally {
              locked = false;
              if (current()) {
                if (submitted) fields.replaceChildren();
                else { save.disabled = false; text.disabled = false; }
              }
            }
          });
        } catch (error) {
          if (current()) report.textContent = `Artifact unavailable: ${explain(error)}`;
        } finally { load.disabled = false; }
      });
      surface.append(field("Artifact", kind), load, report, fields);
      return surface;
    }

    function renderControls(selected = "") {
      editor.replaceChildren();
      if (!snapshot) return;
      const displayed = snapshot;
      const repository = choice.value;
      const revision = element("p", "Current Cairn revision: ");
      revision.append(code(snapshot.revision));
      const edges = element("ul");
      for (const [dependent, prerequisites] of snapshot.nodes) {
        for (const prerequisite of prerequisites) edges.append(element("li", `${dependent} requires ${prerequisite}${snapshot.archived.has(prerequisite) ? " (archived)" : ""}`));
      }
      const graph = element("details");
      graph.append(element("summary", "Native dependency graph"), edges.firstChild ? edges : element("p", "No dependency edges."));
      editor.append(revision, graph);
      if (!snapshot.valid) editor.append(element("p", "Cairn reports an invalid graph. New changes and prerequisites are disabled.", "warning"));
      if (snapshot.change) editor.append(element("p", `Native readback: ${snapshot.change.name}.`));
      const newName = element("input");
      newName.maxLength = 128;
      const change = element("select");
      change.append(option("", "Choose an active change"), ...[...snapshot.nodes.keys()].map((id) => option(id)));
      change.value = snapshot.nodes.has(selected) ? selected : "";
      const prerequisite = element("input");
      prerequisite.maxLength = 128;
      const actions = [
        [button("Create change"), () => authoringRequest(repository, displayed, "create", newName.value)],
        [button("Add prerequisite"), () => authoringRequest(repository, displayed, "add_dependency", change.value, prerequisite.value)],
        [button("Remove prerequisite"), () => authoringRequest(repository, displayed, "remove_dependency", change.value, prerequisite.value)],
      ];
      const enable = () => { for (const [control, request] of actions) control.disabled = locked || !request(); };
      for (const input of [newName, change, prerequisite]) input.addEventListener("input", enable);
      for (const [opener, request] of actions) opener.addEventListener("click", async () => {
        const body = request();
        if (!body || snapshot !== displayed || locked || !live()) return;
        const { type, change: id, dependency } = body.action;
        const label = opener.textContent;
        locked = true;
        choice.disabled = refresh.disabled = true;
        enable();
        let sent = false;
        try {
          const confirmation = await confirmCairnControl({ title: label, verb: label, opener,
            warning: "This changes the enrolled local Cairn worktree. It does not commit, publish, or start work.",
            facts: [["Repository", enrolled.get(repository)], ["Repository identity", repository], ["Change", id], ["Prerequisite", dependency], ["Expected Cairn revision", body.expected_revision]],
          });
          if (!confirmation?.confirmed || snapshot !== displayed || !live()) return;
          status.textContent = "Waiting for Cairn and native readback…";
          sent = true;
          const result = await context.api(AUTHORING_ROUTE, { method: "POST", body, quiet: true });
          if (!live()) return;
          if (!["applied", "reconciled", "unknown"].includes(result?.outcome) || result.schema !== AUTHORING_SCHEMA
            || result.repository !== repository || result.action?.type !== type || result.action.change !== id
            || result.action.dependency !== dependency) throw new Error("Invalid authoring receipt");
          if (result.outcome === "unknown") {
            snapshot = null;
            status.textContent = "Cairn's write outcome is unknown. Refresh the native graph and inspect the worktree before deciding what to do next.";
            return;
          }
          const observed = admitAuthoringSnapshot(result, repository);
          if (observed.change?.name !== id || observed.revision === body.expected_revision) throw new Error("Receipt does not match the selected change and new revision");
          snapshot = observed;
          status.textContent = result.outcome === "applied"
            ? `Cairn applied the change and confirmed it by native readback. Receipt: ${result.provider?.result?.receipt_hash || "not recorded"}.`
            : "Native readback confirmed the change; Cairn did not return a complete matching receipt.";
        } catch (error) {
          if (!live()) return;
          if (sent) snapshot = null;
          status.textContent = sent
            ? `No authoring success was confirmed: ${explain(error)} Refresh the native graph before any new write.`
            : `Authoring was not sent: ${explain(error)}`;
        } finally {
          locked = false;
          if (live()) {
            choice.disabled = refresh.disabled = false;
            if (sent) renderControls(id); else enable();
          }
        }
      });
      editor.append(field("New change name", newName), actions[0][0], field("Active change", change),
        field("Prerequisite name", prerequisite), actions[1][0], actions[2][0]);
      if (editArtifacts) editor.append(artifactControls(repository, change));
      enable();
    }

    panel.addEventListener("toggle", async () => {
      if (!panel.open || enrolled) return;
      enrolled = new Map();
      try {
        const value = await context.api(AUTHORING_ROUTE, { quiet: true });
        if (!live()) return;
        if (value?.schema !== AUTHORING_SCHEMA || value.capabilities?.create !== true
          || value.capabilities?.edit_dependencies !== true || typeof value.capabilities?.edit_artifacts !== "boolean"
          || !Array.isArray(value.roots)) throw new Error("Invalid Cairn authoring capabilities");
        editArtifacts = value.capabilities.edit_artifacts;
        for (const root of value.roots) {
          if (!HEX.test(root?.repository) || !str(root.label) || enrolled.has(root.repository)) throw new Error("Invalid enrolled Cairn root");
          enrolled.set(root.repository, root.label);
        }
        choice = element("select");
        choice.append(option("", "Choose an enrolled repository"), ...[...enrolled].map(([id, label]) => option(id, label)));
        choice.value = enrolled.has(route.repository) ? route.repository : enrolled.size === 1 ? enrolled.keys().next().value : "";
        refresh = button("Refresh native graph");
        choice.addEventListener("change", () => void readGraph());
        refresh.addEventListener("click", () => void readGraph());
        content.append(element("p", editArtifacts ? "Create a change, edit its prerequisites, or replace a proposal, design, or tasks artifact." : "Cairn can create a spec-driven scaffold and change prerequisites. This provider does not support editing proposal, design, tasks, or other artifact contents.", "small"), field("Enrolled repository", choice), refresh, editor);
        await readGraph();
      } catch (error) {
        if (!live()) return;
        enrolled = null;
        status.textContent = error?.status === 503 || error?.status === 403
          ? "Cairn authoring is not enabled for this console."
          : `Cairn authoring unavailable: ${explain(error)}`;
      }
    });
    return panel;
  }

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
      sheet.append(changeHeader(value), fleetSection(value), nextSteps(value, current), unitsSection(value, route), graphSection(value), campaignsSection(value), agentsSection(value), aboutSection(value));
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
    const page = route.change ? changePage(route, model.rows) : registerPage(route, model.rows);
    const author = nativeAuthoringPanel(route);
    if (author) page.append(author);
    container.replaceChildren(page);
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
    activeController?.abort();
    fleetController?.abort();
    fleetController = null;
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    activeController = controller;
    const apiOpts = controller ? { controller } : undefined;
    clearTimeout(filterTimer);
    if (mounted !== container) { mounted?.replaceChildren(); model = null; shown = null; }
    mounted = container;
    const live = () => current === generation && container.isConnected && sameSelection(route, normalizeRoute(context?.route));
    if (!model || !sameSelection(route, shown)) container.replaceChildren(loadingState());
    container.setAttribute?.("aria-busy", "true");
    const [index, detail] = await Promise.all([
      ctx.api(INDEX_ROUTE, apiOpts).then((value) => ({ value }), (error) => ({ error })),
      route.change ? ctx.api(detailPath(route), apiOpts).then((value) => ({ value }), (error) => ({ error })) : Promise.resolve(null),
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
    model = { index: indexResult, rows, detail: detailResult, fleet: undefined };
    render(container, normalizeRoute(context?.route));
    if (route.change) void loadFleet(route, current, live);
    notice = "";
  }

  // Fleet reads never block the Cairn workspace. They update only the inline panel once both reads settle.
  async function loadFleet(route, current, live) {
    fleetController?.abort();
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    fleetController = controller;
    const apiOpts = controller ? { controller } : undefined;
    const read = (path) => Promise.resolve()
      .then(() => context.api(path, apiOpts))
      .then((value) => ({ value }), (error) => ({ error }));
    const [claims, attempts] = await Promise.all([
      read("/api/fleet/claims"),
      read("/api/fleet/attempts"),
    ]);
    if (current !== generation || !live()) return;
    if (isAborted(claims.error) || isAborted(attempts.error)) return;
    model.fleet = claims.error || attempts.error
      ? { error: claims.error || attempts.error }
      : { claims: claims.value, attempts: attempts.value };
    if (mounted && model.detail?.value) render(mounted, normalizeRoute(context?.route));
  }
  function clear() {
    generation += 1;
    activeController?.abort();
    activeController = null;
    fleetController?.abort();
    fleetController = null;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted?.removeAttribute?.("aria-busy");
    mounted = null;
    context = null;
    model = null;
    shown = null;
    authoringPanel = null;
    expected = null;
    notice = "";
  }

  window.KilnCairn = Object.freeze({
    load, clear,
    facets: FACETS, lanes: LANES, facet, lane, binding, condition, normalizeRoute, filterPatch, detailPath, indexRows, matches, filterCounts, neighbors, admitDetail, dependencyGraph,
    slotStates: SLOT_STATES, provenance: PROVENANCE, selections: SELECTIONS, workerPairings: WORKER_PAIRINGS, slotText, familyProvenance, admitAgents,
    admitAtlasDocument, buildCairnAtlas, renderAtlasTopology, ATLAS_MAX_NODES, ATLAS_DECLARED_QUALIFIER,
    renderPlanReview, attemptLifecycleControls,
    admitAuthoringSnapshot, authoringRequest,
  });
})();
