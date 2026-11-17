"use strict";
// r[impl kiln.cairn.workspace] Cairn changes (?page=cairn): one read-only view per Cairn change. It joins canonical
// work, attempts with their verification, publication and native lifecycle records, and selected campaign members by
// exact identity, from the service's one bounded snapshot. Nothing here claims, dispatches, mutates lifecycle or
// grants authority, and displayed reasons are never parsed into decisions.
(() => {
  const INDEX_SCHEMA = "kiln.cairn-workspace-index.v1";
  const DETAIL_SCHEMA = "kiln.cairn-workspace-detail.v1";
  const INDEX_ROUTE = "/api/implementation/cairn";
  const DETAIL_ROUTE = "/api/implementation/cairn/detail";
  const HEX = /^[a-f0-9]{64}$/;
  // The service admits a change name of at most this many UTF-8 bytes, without "/" or control characters.
  const CHANGE_MAX_BYTES = 256;
  const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;
  // Typing in the filter narrows the list at once and commits it to the URL after this pause, as on the receipts page.
  const FILTER_COMMIT_MS = 350;
  const FILTER_MAX_CHARS = 256;
  // Identities read in this short form in the list; the full value stays in the detail.
  const SHORT_HASH = 12;
  // An unrecognized recorded value is quoted up to this length. It never reads as success.
  const LABEL_TEXT_MAX = 48;

  const word = (label, tone, meaning = "") => Object.freeze({ label, tone, meaning });
  const vocabulary = (entries) => new Map(entries.map(([key, label, tone, meaning]) => [key, word(label, tone, meaning)]));
  // Five separately legible facets of a unit, each with explicit not-started, not-published and not-observed values.
  // Tones are the shared state-badge tones.
  const FACETS = Object.freeze({
    readiness: Object.freeze({ title: "Readiness", values: vocabulary([
      ["ready", "Ready", "good", "Every current version is eligible by canonical admission."],
      ["blocked", "Blocked", "failure", "Canonical eligibility names a reason; the blockers list it."],
      ["unknown", "Readiness unknown", "warning", "Canonical eligibility is unavailable. This is not ready."],
      ["not_admitted", "Not admitted", "info", "No current canonical work names this unit."],
    ]) }),
    execution: Object.freeze({ title: "Execution", values: vocabulary([
      ["not_started", "Not started", "info", "No attempt on the current work version."],
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
      ["none", "No verification", "info", "No attempt recorded independent verification."],
    ]) }),
    integration: Object.freeze({ title: "Integration", values: vocabulary([
      ["not_published", "Not published", "info", "The latest attempt on current work has no publication record."],
      ["prepared", "Publication prepared", "info", "A publication plan is recorded; nothing is published yet."],
      ["branch_published", "Branch published", "info", "The candidate branch is published."],
      ["pull_request_published", "Pull request published", "info", "The pull request is published; integration is not observed."],
      ["integrated", "Integrated", "good", "Exact integration was observed."],
      ["source_reconciled", "Source reconciled", "good", "Integration was observed and the source item reconciled."],
    ]) }),
    lifecycle: Object.freeze({ title: "Lifecycle", values: vocabulary([
      ["receipt_observed", "Lifecycle receipt observed", "good", "A native Cairn lifecycle receipt binds an attempt on the current work."],
      ["not_observed", "No lifecycle receipt", "info", "No native Cairn lifecycle receipt binds an attempt on the current work."],
    ]) }),
  });
  const BINDINGS = vocabulary([
    ["current", "Current work", "good", "The member binds the current canonical work version."],
    ["stale", "Stale work version", "warning", "The member binds a superseded canonical version. The campaign must replan."],
    ["unknown_work", "Unknown work", "warning", "The bound identity is not in the canonical ledger."],
    ["not_bound", "Not bound", "info", "The member names no canonical work yet."],
  ]);
  const CONDITIONS = vocabulary([
    ["missing_dependency", "Missing dependency", "failure", "A prerequisite work identity is not current canonical work."],
    ["no_acceptance_obligations", "No acceptance obligations", "failure", "The work admits no acceptance obligation."],
    ["dependency_cycle", "Dependency cycle", "failure", "The dependency graph reaches this work again."],
    ["campaign_hold", "Campaign hold", "warning", "The campaign's own selection holds this member."],
    ["pending_effect", "Unapplied campaign effect", "warning", "An owner effect is persisted as intent and its progress is not applied."],
    ["dispatch_recovery", "Unknown dispatch outcome", "failure", "A provider outcome is unknown. Its termination must be observed before another attempt."],
    ["stale_work_binding", "Stale work binding", "warning", "A campaign member binds work that is not current."],
  ]);
  const SOURCES = vocabulary([
    ["canonical_work", "Canonical work", "info"],
    ["campaign_selection", "Campaign selection", "info"],
    ["campaign_remaining_obligation", "Campaign remaining obligation", "info"],
  ]);
  const APPLICABILITY = vocabulary([
    ["current", "Current work", "good", "This attempt's work identity is the unit's current canonical version."],
    ["historical", "Historical", "warning", "This attempt ran on a superseded work version. Its records never read as current."],
  ]);
  const DISPATCH = vocabulary([
    ["not_reserved", "Not reserved", "info"],
    ["reserved", "Reserved", "info", "A dispatch reservation is recorded; transport has not started."],
    ["transport_started", "Transport started", "info", "Transport intent was persisted; the provider may hold the work."],
  ]);
  const UNKNOWN_OUTCOMES = vocabulary([
    ["unreconciled", "Outcome unknown", "failure", "No termination is observed. Another attempt is refused until it is."],
    ["reconciled", "Termination observed", "warning", "The unknown outcome's termination was observed."],
  ]);

  // The words for a recorded value. A value Kiln does not know is quoted, never read as success.
  function lookup(table, value) {
    if (typeof value === "string" && table.has(value)) return table.get(value);
    if (typeof value === "string" && value) {
      const quoted = value.length > LABEL_TEXT_MAX ? `${value.slice(0, LABEL_TEXT_MAX - 1)}…` : value;
      return word(`Unrecognized: ${quoted}`, "warning", "Kiln does not recognize this recorded value. It is shown as recorded and never read as success.");
    }
    return word("Not recorded", "info");
  }
  function facet(name, value) { return lookup(FACETS[name]?.values || new Map(), value); }
  function binding(value) { return lookup(BINDINGS, value); }
  function condition(value) { return lookup(CONDITIONS, value); }

  function utf8Bytes(text) {
    try { return encodeURIComponent(text).replace(/%[0-9A-F]{2}/g, "x").length; } catch { return Infinity; }
  }
  function validChange(value) {
    return typeof value === "string" && value.trim() !== "" && !value.includes("/") && !CONTROL.test(value) && utf8Bytes(value) <= CHANGE_MAX_BYTES;
  }
  // The URL's selection. A repository and change are selected together or not at all.
  function normalizeRoute(route) {
    const value = (key) => (typeof route?.[key] === "string" ? route[key] : "");
    const repository = value("cairn_repository");
    const change = value("cairn_change");
    const selected = HEX.test(repository) && validChange(change);
    return Object.freeze({
      repository: selected ? repository : "",
      change: selected ? change : "",
      q: value("cairn_q").slice(0, FILTER_MAX_CHARS),
      invalid: Boolean(repository || change) && !selected,
    });
  }
  function detailPath(route) {
    return `${DETAIL_ROUTE}?repository=${route.repository}&change=${encodeURIComponent(route.change)}`;
  }
  const count = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : null);
  const strings = (value) => (Array.isArray(value) ? value.filter((item) => typeof item === "string") : []);
  function compareText(a, b) { return a < b ? -1 : a > b ? 1 : 0; }
  // List rows, attention first: unknown blocker counts, then the most blockers, then by change and repository, so the
  // order never depends on arrival. A repeated or malformed key is not listed twice or guessed.
  function indexRows(index) {
    if (index?.schema !== INDEX_SCHEMA || !Array.isArray(index.changes)) throw new Error("The Cairn change index is not a kiln.cairn-workspace-index.v1 document.");
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
        activeAttempts: count(entry.active_attempts), blockers: count(entry.blockers),
        search: [entry.change, entry.repository_identity, ...projects, ...campaigns].join("\n").toLowerCase(),
      }));
    }
    const attention = (row) => (row.blockers === null ? Infinity : row.blockers);
    rows.sort((a, b) => attention(b) - attention(a) || compareText(a.change, b.change) || compareText(a.repository, b.repository));
    return rows;
  }
  function matches(row, query) {
    const needle = typeof query === "string" ? query.trim().toLowerCase() : "";
    return !needle || row.search.includes(needle);
  }
  // r[impl kiln.cairn.workspace.scenario.reload] A detail is shown only when it echoes the exact requested
  // repository and change; anything else is refused, never substituted.
  function admitDetail(value, route) {
    if (value?.schema !== DETAIL_SCHEMA || !HEX.test(value.identity) || value.repository_identity !== route.repository
      || value.change !== route.change || !Array.isArray(value.units) || !Array.isArray(value.memberships)
      || !Array.isArray(value.decisions) || !Array.isArray(value.blockers) || typeof value.overview !== "object" || value.overview === null
      || !["units", "current_work", "attempts", "active_attempts", "campaigns", "blockers"].every((key) => count(value.overview[key]) !== null)) {
      throw new Error("The service returned a different or malformed Cairn change. No other change was substituted.");
    }
    return value;
  }

  // ---------- Presentation ----------

  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined && content !== null) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  function plural(value, singular, many = `${singular}s`) { return `${value} ${value === 1 ? singular : many}`; }
  function short(value) { const source = String(value ?? ""); return source.length > SHORT_HASH + 1 ? `${source.slice(0, SHORT_HASH)}…` : source; }
  function quantity(value, singular, many) { return value === null ? `unknown ${many || `${singular}s`}` : plural(value, singular, many); }
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
  function link(ctx, patch, label, className) {
    const node = element("a", label, className);
    node.href = routeHref(patch);
    node.addEventListener("click", (event) => { if (!plainClick(event)) return; event.preventDefault(); ctx.navigate(patch); });
    return node;
  }
  function badge(entry, prefix) {
    const node = element("span", prefix ? `${prefix}: ${entry.label}` : entry.label, "state-badge");
    node.dataset.tone = entry.tone;
    if (entry.meaning) node.title = entry.meaning;
    return node;
  }
  function badges(...nodes) {
    const node = element("div", undefined, "badges");
    node.append(...nodes.filter(Boolean));
    return node;
  }
  function code(value) { return typeof value === "string" && value ? element("code", value) : element("span", "Not recorded", "not-recorded"); }
  function cell(value) {
    if (value === null || value === undefined || value === "") return element("span", "—", "not-recorded");
    return typeof value === "object" ? value : element("span", String(value));
  }
  function facts(rows) {
    const list = element("dl", undefined, "facts");
    for (const [label, value] of rows) {
      if (value === undefined) continue;
      const definition = element("dd");
      definition.append(cell(value));
      list.append(element("dt", label), definition);
    }
    return list;
  }
  // A labelled table that stacks by the width of its own region, as the other console tables do.
  function table(label, heads, rows, className) {
    const wrap = element("div", undefined, "data-table-wrap");
    const node = element("table", undefined, `data-table data-table-stack ${className}`);
    node.setAttribute("aria-label", label);
    const head = element("thead");
    const headRow = element("tr");
    for (const title of heads) { const th = element("th", title); th.setAttribute("scope", "col"); headRow.append(th); }
    head.append(headRow);
    const body = element("tbody");
    for (const cells of rows) {
      const row = element("tr");
      cells.forEach((value, index) => { const td = element("td"); td.dataset.label = heads[index]; td.append(cell(value)); row.append(td); });
      body.append(row);
    }
    node.append(head, body);
    wrap.append(node);
    return wrap;
  }
  // An identity shortened where space is tight; the exact value stays in its title.
  function shortCode(value) {
    if (typeof value !== "string" || !value) return element("span", "Not recorded", "not-recorded");
    const node = element("code", short(value));
    node.title = value;
    return node;
  }
  function section(title, ...children) {
    const node = element("section", undefined, "cairn-section");
    node.append(element("h3", title, "cairn-section-title"), ...children.filter(Boolean));
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
    return node;
  }
  function loadingState() {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    return node;
  }
  function list(items, empty) {
    if (!items.length) return element("p", empty, "small");
    const node = element("ul", undefined, "cairn-list-items");
    for (const item of items) node.append(typeof item === "string" ? element("li", item) : item);
    return node;
  }
  function nonClaims(values) {
    const details = element("details", undefined, "cairn-about");
    details.append(element("summary", "What this view does not claim"), list(strings(values), "No non-claims were supplied."));
    return details;
  }

  let generation = 0;
  let mounted = null;
  let filterTimer = null;
  // What the mounted page last drew from data, and the filter the reader has applied since.
  let shown = null;
  let applied = null;

  // A row link carries the search the reader has applied, even before it is written to the URL.
  function changeRow(ctx, row, route) {
    const item = element("li");
    const anchor = element("a", undefined, "cairn-row");
    const patch = { cairn_repository: row.repository, cairn_change: row.change };
    anchor.href = routeHref(patch);
    anchor.addEventListener("click", (event) => {
      if (!plainClick(event)) return;
      event.preventDefault();
      clearTimeout(filterTimer);
      ctx.navigate({ ...patch, ...(applied ? { cairn_q: applied.q } : {}) });
    });
    anchor.dataset.cairnKey = row.key;
    if (row.repository === route.repository && row.change === route.change) anchor.setAttribute("aria-current", "true");
    const meta = element("p", undefined, "cairn-row-meta");
    meta.append(
      element("span", quantity(row.units, "unit")),
      element("span", quantity(row.attempts, "attempt")),
      element("span", row.activeAttempts ? `${row.activeAttempts} active` : "none active"),
      element("span", row.projects.length ? row.projects.join(", ") : "no canonical project"),
      element("code", short(row.repository)),
    );
    anchor.append(
      element("span", row.change, "cairn-row-title"),
      badges(
        row.blockers === null ? badge(word("Blockers unknown", "warning")) : badge(word(row.blockers ? plural(row.blockers, "blocker") : "No blockers", row.blockers ? "warning" : "good")),
        row.campaigns.length ? badge(word(plural(row.campaigns.length, "campaign"), "info", row.campaigns.join(", "))) : null,
      ),
      meta,
    );
    item.append(anchor);
    return { item, anchor };
  }
  function listPanel(ctx, route, index, rows, rowNodes) {
    const panel = element("section", undefined, "panel split-list cairn-list-panel");
    panel.setAttribute("aria-labelledby", "cairn-list-title");
    const title = element("h2", "Cairn changes");
    title.id = "cairn-list-title";
    panel.append(title, element("p", "Changes named by canonical work or a selected campaign member, keyed by repository identity and change name.", "small"));
    if (index.error) {
      if (errorCode(index.error) === "cairn_workspace_input_not_configured") {
        panel.append(emptyState("No implementation work database is configured", "An operator supplies one with --implementation-work. This is not proof that no Cairn change exists."));
        return panel;
      }
      const retry = element("button", "Try again");
      retry.type = "button";
      retry.addEventListener("click", () => { void load(mounted, ctx); });
      panel.append(errorState(`Cairn changes unavailable: ${explain(index.error)} A selected change is still read on its own.`), retry);
      return panel;
    }
    if (!rows.length) {
      panel.append(emptyState("No Cairn change is named yet", "Canonical work with a Cairn alias, or a selected campaign member, names a change. None is recorded in this store."));
      return panel;
    }
    const form = element("form", undefined, "cairn-filters");
    form.setAttribute("role", "search");
    form.addEventListener("submit", (event) => event.preventDefault());
    const label = element("label", "Search");
    const search = element("input");
    Object.assign(search, { id: "cairn-q", type: "search", maxLength: FILTER_MAX_CHARS, autocomplete: "off", placeholder: "Change, project or campaign", value: route.q });
    label.htmlFor = "cairn-q";
    form.append(label, search);
    const status = element("p", undefined, "small");
    status.setAttribute("role", "status");
    const listNode = element("ol", undefined, "cairn-list");
    const entries = rows.map((row) => { const { item, anchor } = changeRow(ctx, row, route); rowNodes.set(row.key, anchor); listNode.append(item); return { row, item }; });
    const noMatch = element("p", "No change matches this search.", "small");
    const apply = () => {
      const q = search.value.slice(0, FILTER_MAX_CHARS);
      let visible = 0;
      for (const { row, item } of entries) { const match = matches(row, q); item.hidden = !match; if (match) visible += 1; }
      const filtered = Boolean(q.trim());
      status.textContent = filtered ? `${visible} of ${plural(rows.length, "change")}` : `${plural(rows.length, "change")}, most blockers first`;
      noMatch.hidden = Boolean(visible) || !filtered;
      applied = { q };
    };
    search.addEventListener("input", () => {
      apply();
      clearTimeout(filterTimer);
      filterTimer = setTimeout(() => { if (search.isConnected) ctx.navigate({ cairn_q: applied.q }); }, FILTER_COMMIT_MS);
    });
    panel.append(form, status, listNode, noMatch);
    if (index.value.truncated === true) panel.append(element("p", "Only the first changes are listed; the index reached its bound.", "warning"));
    apply();
    return panel;
  }

  function howToRead(route) {
    const panel = element("div", undefined, "cairn-howto");
    panel.append(
      element("h2", "How to read a Cairn change"),
      element("p", "Choose a change to see every unit named by canonical work, a campaign task selection or a campaign's remaining obligations, with the attempts on each work version and the campaign members that select it."),
      element("p", "Each unit reads in five separate facets. Evidence from an attempt on a superseded work version stays historical; it never makes changed work look accepted."),
    );
    const legend = element("dl", undefined, "facts");
    for (const [name, entry] of Object.entries(FACETS)) {
      const values = element("dd");
      values.append(badges(...[...entry.values.values()].map((value) => badge(value))));
      legend.append(element("dt", entry.title), values);
    }
    panel.append(legend);
    if (route.invalid) panel.append(element("p", "The URL names an invalid repository identity or change. Nothing was selected.", "warning"));
    return panel;
  }
  function facetBadges(facets) {
    return badges(...Object.keys(FACETS).map((name) => badge(facet(name, facets?.[name]), FACETS[name].title)));
  }
  function workFacts(work) {
    const dependencies = Array.isArray(work.dependencies) ? work.dependencies.map((dependency) => {
      const item = element("li");
      const state = dependency.state === "current" ? word("Current", "good") : word("Not current", "failure", "The dependency is absent from current canonical work.");
      item.append(badge(state), element("span", ` ${strings(dependency.native_ids).join(", ") || "no Cairn unit"} `), code(dependency.identity));
      return item;
    }) : [];
    const acceptance = Array.isArray(work.acceptance) ? work.acceptance.map((obligation) => `${obligation?.id}: ${obligation?.statement}`) : [];
    return facts([
      ["Title", work.title],
      ["Project", work.project],
      ["Revision", String(work.revision)],
      ["Work identity", code(work.identity)],
      ["Readiness snapshot", code(work.readiness_snapshot)],
      ["Terminal policy", work.terminal_policy],
      ["Acceptance", list(acceptance, "No acceptance obligation is admitted.")],
      ["Dependencies", list(dependencies, "None")],
    ]);
  }
  function attemptsTable(attempts) {
    return table("Attempts", ["Attempt", "Applies to", "Phase", "Dispatch", "Verification", "Publication", "Lifecycle"], attempts.map((attempt) => {
      const which = element("div", undefined, "cairn-attempt-id");
      which.append(element("span", `${attempt.ordinal} · fence ${attempt.fence}`), shortCode(attempt.identity));
      const verification = attempt.verification
        ? badge(word(attempt.verification.success ? `Passed · ${attempt.verification.run_id}` : `Failed · ${attempt.verification.run_id}`, attempt.verification.success ? "good" : "failure"))
        : null;
      const lifecycle = attempt.lifecycle
        ? badge(attempt.lifecycle.binding === "matches_attempt" ? word(`Receipt · ${attempt.lifecycle.action}`, "good") : word("Crossed receipt", "failure", "The receipt binds another work identity or fence. It is not read as this attempt's lifecycle."))
        : null;
      const outcome = attempt.unknown_outcome ? badge(lookup(UNKNOWN_OUTCOMES, attempt.unknown_outcome)) : null;
      return [
        which,
        badges(badge(lookup(APPLICABILITY, attempt.applicability)), element("span", `revision ${attempt.work_revision}`, "small")),
        badges(badge(lookup(FACETS.execution.values, attempt.phase)), attempt.failure ? element("span", `${attempt.failure}${attempt.cause ? `: ${attempt.cause}` : ""}`, "small") : null),
        badges(badge(lookup(DISPATCH, attempt.dispatch)), outcome),
        verification,
        attempt.publication ? badge(facet("integration", attempt.publication.phase)) : null,
        lifecycle,
      ];
    }), "cairn-attempts-table");
  }
  function unitNode(unit) {
    const node = element("article", undefined, "cairn-unit");
    node.append(element("h4", unit.native_id, "cairn-unit-title"), badges(...strings([...(Array.isArray(unit.sources) ? unit.sources : [])]).map((source) => badge(lookup(SOURCES, source)))), facetBadges(unit.facets));
    const current = Array.isArray(unit.current_work) ? unit.current_work : [];
    if (!current.length) node.append(element("p", "No current canonical work names this unit. It is listed because a campaign selects it or still owes it.", "small"));
    for (const work of current) node.append(workFacts(work));
    const superseded = Array.isArray(unit.superseded_work) ? unit.superseded_work : [];
    if (superseded.length) node.append(element("p", `Superseded versions: ${superseded.map((work) => `revision ${work.revision} (${short(work.identity)})`).join(", ")}`, "small"));
    const attempts = Array.isArray(unit.attempts) ? unit.attempts : [];
    node.append(attempts.length ? attemptsTable(attempts) : element("p", "No attempt is recorded for this unit.", "small"));
    return node;
  }
  function membershipNode(ctx, membership) {
    const node = element("article", undefined, "cairn-member");
    const campaign = link(ctx, { page: "workers", worker_campaign: membership.campaign_id }, membership.campaign_id);
    const hold = membership.hold
      ? facts([["Hold", membership.hold.operation], ["Owner", membership.hold.owner || "Not named"], ["Reason (display only)", membership.hold.reason]])
      : element("p", "No campaign hold.", "small");
    const effects = Array.isArray(membership.pending_effects) ? membership.pending_effects.map((effect) => `${effect.operation} by ${effect.owner}: ${effect.state}${effect.progress_applied ? ", progress applied" : ", progress not applied"}`) : [];
    node.append(
      element("h4", membership.member, "cairn-unit-title"),
      badges(badge(binding(membership.work_binding), "Work binding"), badge(word(membership.observed ? `Stage: ${membership.stage}` : "Not observed", membership.observed ? "info" : "warning"))),
      facts([
        ["Campaign", campaign],
        ["Selection", membership.selector === "tasks" ? `Tasks: ${strings(membership.tasks).join(", ")}` : "Whole change"],
        ["Goal", membership.goal],
        ["Admission", membership.admission],
        ["Enumeration", membership.enumeration],
        ["Bound work", code(membership.work_identity)],
        ["Remaining obligations", list(strings(membership.remaining_obligations), "None observed")],
        ["Member blockers", list(strings(membership.blockers), "None observed")],
        ["Pending effects", list(effects, "None")],
      ]),
      hold,
    );
    return node;
  }
  function blockerNode(blocker) {
    const node = element("article", undefined, "cairn-blocker");
    node.append(
      badges(badge(condition(blocker.condition)), element("code", blocker.subject, "cairn-subject")),
      facts([
        ["Proposed operation", blocker.operation],
        ["Owner", blocker.owner],
        ["Observation", code(blocker.observation)],
        ["Detail (display only)", blocker.detail],
      ]),
    );
    return node;
  }
  function blockersSection(blockers) {
    if (!blockers.length) return section("Blockers", element("p", "No structured blocker is observed. This is not readiness on its own; read each unit's facets.", "small"));
    return section("Blockers", ...blockers.map(blockerNode));
  }
  // Decisions read as in the campaign inbox: category and state, the question, then its facts. Nothing here answers one.
  function decisionNode(decision) {
    const node = element("article", undefined, "cairn-decision");
    const scope = window.KilnInventory?.scopes?.[decision.scope] || decision.scope;
    node.append(
      element("h4", `${String(decision.category).replaceAll("_", " ")} · ${decision.state}`, "cairn-item-title"),
      element("p", decision.question),
      facts([
        ["Member", decision.member],
        ["Options", strings(decision.options).join(", ") || "None supplied"],
        ["Recommendation", decision.recommendation ?? "None supplied"],
        ["Scope", scope],
        ["Bound work", code(decision.work_identity)],
        ["Queue note", decision.pending_reason ?? "None"],
        ["Resolved answer", decision.resolved_answer ?? "Not resolved"],
      ]),
    );
    return node;
  }
  function decisionsSection(decisions) {
    if (!decisions.length) return null;
    return section("Decisions", ...decisions.map(decisionNode), element("p", "Answer decisions in the campaign's own view; this page never answers one.", "small"));
  }
  function sheet(ctx, detail) {
    const node = element("article", undefined, "cairn-sheet");
    const head = element("header", undefined, "cairn-head");
    const titleNode = element("h2", detail.change, "cairn-title");
    titleNode.tabIndex = -1;
    head.append(titleNode, facts([
      ["Projects", strings(detail.projects).join(", ") || "No canonical project"],
      ["Repository identity", code(detail.repository_identity)],
      ["View identity", code(detail.identity)],
    ]));
    const overview = detail.overview;
    const totals = badges(
      badge(word(plural(overview.units, "unit"), "info")),
      badge(word(plural(overview.current_work, "current work version"), "info")),
      badge(word(`${plural(overview.attempts, "attempt")}, ${overview.active_attempts} active`, "info")),
      badge(word(plural(overview.campaigns, "campaign"), "info")),
      badge(word(overview.blockers ? plural(overview.blockers, "blocker") : "No blockers", overview.blockers ? "warning" : "good")),
    );
    node.append(head, totals);
    const snapshot = detail.snapshot || {};
    if (snapshot.campaigns_truncated || snapshot.lifecycle_truncated) node.append(element("p", "The snapshot reached a read bound; campaigns or lifecycle receipts beyond it are not shown.", "warning"));
    if (snapshot.work_recorded === false) node.append(element("p", "No canonical work is recorded in this store. Units come from campaign selections only.", "small"));
    node.append(blockersSection(detail.blockers));
    node.append(section("Units", ...(detail.units.length ? detail.units.map(unitNode) : [element("p", "No unit is named yet. A whole-change selection names the change, not a unit.", "small")])));
    node.append(section("Campaign members", ...(detail.memberships.length ? detail.memberships.map((membership) => membershipNode(ctx, membership)) : [element("p", "No selected campaign member names this change.", "small")])));
    const decisions = decisionsSection(detail.decisions);
    if (decisions) node.append(decisions);
    node.append(nonClaims(detail.non_claims));
    return node;
  }
  function detailPanel(ctx, route, detail) {
    const panel = element("section", undefined, "panel split-detail cairn-detail");
    if (!route.change) {
      panel.setAttribute("aria-label", "How to read a Cairn change");
      panel.append(howToRead(route));
      return panel;
    }
    panel.setAttribute("aria-label", "Selected Cairn change");
    panel.append(link(ctx, { cairn_repository: "", cairn_change: "" }, "All changes", "button cairn-back"));
    if (detail.error) {
      const message = detail.error.status === 404
        ? `${route.change} is not named by canonical work or a selected campaign in this store. No other change was substituted.`
        : `${route.change} unavailable: ${explain(detail.error)} No other change was substituted.`;
      panel.append(errorState(message));
      return panel;
    }
    try {
      panel.append(sheet(ctx, admitDetail(detail.value, route)));
    } catch (error) {
      panel.append(errorState(explain(error)));
    }
    return panel;
  }

  async function load(container, ctx) {
    if (!container) return;
    const route = normalizeRoute(ctx?.route);
    // A search the reader already applied is only being written to the URL: nothing to read or redraw.
    if (mounted === container && shown && applied && route.repository === shown.repository && route.change === shown.change
      && route.q !== shown.q && route.q === applied.q && container.firstChild) {
      shown = { ...shown, q: route.q };
      return;
    }
    const current = ++generation;
    clearTimeout(filterTimer);
    if (mounted !== container) { mounted?.replaceChildren(); shown = null; }
    mounted = container;
    const live = () => current === generation && container.isConnected;
    if (!shown) container.replaceChildren(loadingState());
    const [index, detail] = await Promise.all([
      ctx.api(INDEX_ROUTE).then((value) => ({ value }), (error) => ({ error })),
      route.change ? ctx.api(detailPath(route)).then((value) => ({ value }), (error) => ({ error })) : Promise.resolve(null),
    ]);
    // r[impl kiln.cairn.workspace.scenario.reload] A response for an earlier selection never draws over a newer one.
    if (!live() || isAborted(index.error) || isAborted(detail?.error)) return;
    let rows = [];
    let indexResult = index;
    if (index.value) {
      try { rows = indexRows(index.value); } catch (error) { indexResult = { error }; }
    }
    const focused = typeof document.activeElement?.dataset?.cairnKey === "string" ? document.activeElement.dataset.cairnKey : "";
    const rowNodes = new Map();
    const layout = element("div", undefined, "split-view cairn-layout");
    layout.dataset.selected = route.change ? "true" : "false";
    applied = null;
    layout.append(listPanel(ctx, route, indexResult, rows, rowNodes), detailPanel(ctx, route, detail || {}));
    container.replaceChildren(layout);
    shown = { repository: route.repository, change: route.change, q: route.q };
    if (focused) rowNodes.get(focused)?.focus?.({ preventScroll: true });
    document.title = ["Kiln", "Cairn changes", route.change].filter(Boolean).join(" · ");
  }
  function clear() {
    generation += 1;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted = null;
    shown = null;
    applied = null;
  }

  window.KilnCairn = Object.freeze({
    load, clear,
    facets: FACETS, facet, binding, condition, normalizeRoute, detailPath, indexRows, matches, admitDetail,
  });
})();
