"use strict";

// r[impl kiln.web.interface]
(() => {
  document.documentElement.classList.add("catalog-js");
  const LIVE_DELAY_MS = 1500;
  const MAX_LIVE_FAILURES = 6;
  const MAX_BACKOFF_MS = 15000;
  const REQUEST_TIMEOUT_MS = 30000;
  const MAX_JSON_BYTES = 4 * 1024 * 1024;
  const LOG_READ_BYTES = 16384;
  const MAX_DISPLAYED_LOG_CHARS = 256 * 1024;
  const LOG_FOLLOW_MS = 2000;
  const LOG_TAIL_SLOP_PX = 24;
  const CONTROL_FRESHNESS_MS = 30000;
  const ELAPSED_TICK_MS = 1000;
  const GRAPH_MIN_ZOOM = 0.35;
  const GRAPH_MAX_ZOOM = 1.5;
  const GRAPH_ZOOM_STEP = 0.15;
  const localTimeFormat = new Intl.DateTimeFormat(undefined, {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit", timeZoneName: "short",
  });
  const localTimeZone = localTimeFormat.resolvedOptions().timeZone;
  const $ = (id) => document.getElementById(id);
  const MAX_PANE_POLL_MS = 3000;
  const CHORD_TIMEOUT_MS = 1000;
  const LEASE_EXPIRING_MS = 15 * 60 * 1000;
  const state = {
    session: null, token: null, epoch: 0, requests: new Set(), urls: new Set(), urlTimers: new Set(),
    projects: { available: [], enrolled: [] }, runs: [], selected: null, run: null,
    observedAt: 0, connected: false, liveVersion: "0", liveTimer: null,
    liveController: null, liveGeneration: 0, liveFailures: 0,
    // Running count of the last admitted live snapshot (null before one), and requests in flight outside background polls.
    liveRunning: null, busy: 0,
    logTimer: null, logGeneration: 0, logBusy: false, logNext: "0", logKey: "",
    artifactsGeneration: 0, confirm: null, mutation: false, detailGeneration: 0,
    principal: null, permissions: {}, route: null, routeLoaded: false, nextCursor: null, total: 0,
    runsGeneration: 0, pageGeneration: 0, refsGeneration: 0,
    endpointGeneration: new Map(),
    runsError: "",
    cobsNextCursor: null,
    graphRun: null, graphZoom: 1,
    workCanWrite: false,
    // Client-side order of the loaded runs page only; null keeps the service order.
    runSort: null,
    // Failure-first log defaults for the selected failed run: { run, node, stream, fellBack }.
    failureFirst: null,
    reconcileStatus: null,
    liveTicker: null, logAutoFollow: false, followDeclined: "", startProject: "",
    events: { status: "disconnected", ready: null, ci: null, fleet: null, fleetObservedAt: 0, cairn: null, attention: null },
    eventSubscribers: new Set(),
    factory: { attention: null, machines: null, claims: null, dispatch: null, cairn: null, codeReview: null, repositories: null, showAllClaims: false },
    inspector: { open: false, kind: "", id: "", items: [], paneTimer: null, paneController: null, paneLines: 200, returnFocus: null },
    selectedRuns: new Set(),
    chordPrefix: "", chordTimer: null,
    feedsTimer: null,
    logDigestCache: new Map(),
    logDigestProbed: new Set(),
    digestJump: null,
  };

  function text(value) {
    if (value === null || value === undefined || value === "") return "Unavailable";
    if (typeof value === "number" && Number.isInteger(value) && !Number.isSafeInteger(value)) return "Unavailable (outside exact browser integer range)";
    return typeof value === "object" ? JSON.stringify(value, null, 2) : String(value);
  }
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  // Presentation-only helpers over window.KilnUi; stored values are never rewritten.
  // r[impl kiln.web.readable_observations]
  // r[impl kiln.web.readable_observations.scenario.unknown]
  const ui = window.KilnUi;
  function when(ms) { return ui.timeElement(Number.isSafeInteger(ms) ? ms : null); }
  function elapsed(ms) { return ui.durationElement(Number.isSafeInteger(ms) && ms >= 0 ? ms : null); }
  function relativeText(ms) { return Number.isSafeInteger(ms) ? ui.formatRelative(ms) : "Not recorded"; }
  function statePill(status, label, exact) {
    const pill = element("span", undefined, "status-pill");
    pill.dataset.status = status;
    pill.append(ui.statusIcon(status), element("span", label, "status-pill-label"));
    if (exact && exact !== label) pill.title = exact;
    return pill;
  }
  function phaseChip(phase) {
    const chip = element("span", `Phase: ${text(phase)}`, "chip");
    chip.title = `Recorded phase: ${text(phase)}`;
    return chip;
  }
  // r[impl kiln.web.visual_design]
  function loadingState(label = "Loading admitted records…") {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    node.append(element("p", label, "small"));
    return node;
  }
  function errorState(message) { return element("p", message, "warning error-state"); }
  function emptyState(title, message, action) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    if (action) node.append(action);
    return node;
  }
  function notice(message, kind = "error") {
    $("notice").textContent = message;
    $("notice").dataset.kind = kind;
    $("notice").hidden = !message;
  }
  function isAborted(error) { return error.name === "AbortError"; }
  function explain(error) {
    if (error.name === "TimeoutError") return "The request timed out; its result is not known.";
    if (error instanceof TypeError) return "The service could not be reached. Its current state is unavailable.";
    return error.message || "The service response was unavailable.";
  }
  // r[impl kiln.web.live_activity]
  // r[impl kiln.web.live_activity.scenario.count]
  // r[impl kiln.web.live_activity.scenario.stale]
  // KilnUi decides the count to present; this only draws it. connection() is the one place that states whether
  // observation is current. A heartbeat means service state has not moved since the stored snapshot, so the stored
  // count is current again after a recovered connection.
  function showLiveActivity(current) {
    const running = ui.presentedRunning(state.liveRunning, current);
    $("live-activity-count").textContent = String(running);
    $("live-activity-unit").textContent = running === 1 ? "run" : "runs";
    $("live-activity").hidden = running === 0;
    if ($("global-firing-count")) $("global-firing-count").textContent = String(running);
    document.documentElement.toggleAttribute("data-firing", running > 0);
  }
  function setSseHealth(status, label) {
    state.events.status = status;
    const badge = $("sse-health-badge");
    if (badge) {
      badge.dataset.sse = status;
      badge.textContent = label;
    }
  }
  function updateGlobalLiveStrip() {
    const strip = $("global-live-strip");
    if (!strip) return;
    strip.hidden = !state.session;
    const escalations = state.events.attention?.total ?? state.factory.attention?.total ?? 0;
    const escalationsEl = $("global-escalations-count");
    const escalationsPill = $("global-escalations-pill");
    if (escalationsEl) escalationsEl.textContent = String(escalations);
    if (escalationsPill) escalationsPill.dataset.alert = String(escalations > 0);
    const now = Date.now();
    const claims = state.events.fleet?.claims?.claims || state.factory.claims?.claims || [];
    const foreign = state.factory.claims?.foreign_claims || [];
    const expiring = claims.filter((c) => c && (c.state === "open" || c.state?.state === "open") && Number.isSafeInteger(c.expires_ms) && c.expires_ms - now <= LEASE_EXPIRING_MS).length + foreign.length;
    const claimsEl = $("global-expiring-claims-count");
    const claimsPill = $("global-expiring-claims-pill");
    if (claimsEl) claimsEl.textContent = String(expiring);
    if (claimsPill) claimsPill.dataset.alert = String(expiring > 0);
  }
  function connection(message, healthy) {
    state.connected = healthy;
    $("connection-status").textContent = message;
    $("connection-status").title = message;
    $("connection-status").dataset.state = healthy ? "connected" : "stale";
    if (healthy) setSseHealth("live", "Live: SSE");
    else if (/Disconnected/i.test(message)) setSseHealth("disconnected", "Disconnected");
    else setSseHealth("reconnecting", "Reconnecting");
    showLiveActivity(healthy);
    updateGlobalLiveStrip();
    updateControlButtons();
  }
  // r[impl kiln.web.request_feedback]
  // r[impl kiln.web.request_feedback.scenario.slow]
  // One counter at the fetch boundary: the top-bar line and the busy state follow it and clear when it reaches zero.
  function setBusy(delta) {
    state.busy = Math.max(0, state.busy + delta);
    document.documentElement.toggleAttribute("data-busy", state.busy > 0);
    if (state.busy > 0) $("main").setAttribute("aria-busy", "true"); else $("main").removeAttribute("aria-busy");
  }
  function pathFor(id) { return `/api/runs/${encodeURIComponent(id)}`; }
  function shortIdentity(value) {
    const source = text(value);
    return source.length > 22 ? `${source.slice(0, 12)}…${source.slice(-6)}` : source;
  }
  function projectLabel(id) { return state.projects.enrolled.find((project) => project.id === id)?.label || id; }
  function workspaceLink(patch, label, className) {
    const link = element("a", label, className);
    link.href = `?${new URLSearchParams(Object.entries({ ...state.route, ...patch }).filter(([, value]) => value))}`;
    link.addEventListener("click", (event) => {
      if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
        event.preventDefault(); navigate(patch);
      }
    });
    return link;
  }

  async function boundedText(response, maximum) {
    if (!response.body) throw new Error("The service returned no response body.");
    const reader = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let size = 0;
    let result = "";
    try {
      while (true) {
        const part = await reader.read();
        if (part.done) break;
        size += part.value.byteLength;
        if (size > maximum) throw new Error("The service response exceeds this page’s bounded read limit.");
        result += decoder.decode(part.value, { stream: true });
      }
      return result + decoder.decode();
    } finally {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
  }
  async function boundedBlob(response) {
    if (!response.body) throw new Error("The service returned no download body.");
    const reader = response.body.getReader();
    const chunks = [];
    let bytes = 0;
    try { while (true) { const part = await reader.read(); if (part.done) break; bytes += part.value.byteLength; if (bytes > 256 * 1024 * 1024) throw new Error("Download exceeds this browser’s 256 MiB memory bound. No partial file was offered."); chunks.push(part.value); } return new Blob(chunks, { type: response.headers.get("Content-Type") || "application/octet-stream" }); }
    finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
  }

  // r[impl kiln.dashboard.complete.access]
  // `quiet` marks a background observation poll (the live snapshot poll, log following): it shows no request feedback.
  // A persistent event stream keeps the response body open; the fleet page reads frames incrementally.
  async function apiStream(target, { controller = new AbortController() } = {}) {
    if (!state.session) throw new DOMException("Signed out", "AbortError");
    const headers = { Accept: "text/event-stream" };
    if (state.token) headers.Authorization = `Bearer ${state.token}`;
    const response = await fetch(target, {
      headers, credentials: "omit", cache: "no-store", redirect: "error",
      signal: controller.signal, referrerPolicy: "no-referrer",
    });
    if (response.status === 401) {
      await response.body?.cancel();
      logout(state.session === "network" ? "Network access was rejected. Reopen the page to recheck access." : "Authentication expired or the operator token was rejected. Sign in again.");
      throw new DOMException("Signed out", "AbortError");
    }
    if (!response.ok) {
      let diagnostic;
      try { diagnostic = JSON.parse(await boundedText(response, MAX_JSON_BYTES)); } catch { diagnostic = null; }
      const error = new Error(diagnostic && typeof diagnostic.message === "string" ? `${text(diagnostic.code)}: ${diagnostic.message}` : `Service request rejected (HTTP ${response.status}).`);
      error.status = response.status;
      throw error;
    }
    if (!response.body) throw new Error("The service returned no event stream.");
    return response.body;
  }

  async function api(target, { method = "GET", body, kind = "json", controller = new AbortController(), bootstrap = false, quiet = kind === "sse" } = {}) {
    if (!state.session && !bootstrap) throw new DOMException("Signed out", "AbortError");
    const epoch = state.epoch;
    const headers = { Accept: kind === "sse" ? "text/event-stream" : kind === "blob" ? "application/octet-stream" : "application/json" };
    if (state.token) headers.Authorization = `Bearer ${state.token}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    state.requests.add(controller);
    const timeout = setTimeout(() => controller.abort(new DOMException("Request timed out", "TimeoutError")), REQUEST_TIMEOUT_MS);
    if (!quiet) setBusy(1);
    try {
      const response = await fetch(target, {
        method, headers, body: body === undefined ? undefined : JSON.stringify(body),
        credentials: "omit", cache: "no-store", redirect: "error", signal: controller.signal,
        referrerPolicy: "no-referrer",
      });
      if (epoch !== state.epoch) throw new DOMException("Session changed", "AbortError");
      if (response.status === 401) {
        if (bootstrap) { await response.body?.cancel(); return null; }
        logout(state.session === "network" ? "Network access was rejected. Reopen the page to recheck access." : "Authentication expired or the operator token was rejected. Sign in again.");
        throw new DOMException("Signed out", "AbortError");
      }
      if (!response.ok) {
        let diagnostic;
        try { diagnostic = JSON.parse(await boundedText(response, MAX_JSON_BYTES)); } catch { diagnostic = null; }
        const error = new Error(diagnostic && typeof diagnostic.message === "string" ? `${text(diagnostic.code)}: ${diagnostic.message}` : `Service request rejected (HTTP ${response.status}).`);
        if (diagnostic && typeof diagnostic.code === "string") error.code = diagnostic.code;
        error.status = response.status;
        throw error;
      }
      let result;
      if (kind === "blob") result = await boundedBlob(response);
      else {
        const source = await boundedText(response, MAX_JSON_BYTES);
        if (kind === "sse") result = source;
        else {
          try { result = JSON.parse(source); } catch { throw new Error("The service returned invalid JSON. No state was admitted by this page."); }
        }
      }
      if (epoch !== state.epoch) throw new DOMException("Session changed", "AbortError");
      return result;
    } catch (error) {
      if (controller.signal.aborted && controller.signal.reason?.name === "TimeoutError") throw controller.signal.reason;
      throw error;
    } finally {
      clearTimeout(timeout);
      state.requests.delete(controller);
      // Success, rejection, timeout and abort all pass here, so the indication cannot outlive its request.
      if (!quiet) setBusy(-1);
    }
  }

  function factValue(cell, value) {
    if (value instanceof Node) cell.append(value); else cell.textContent = text(value);
    return cell;
  }
  function addFacts(target, entries) {
    target.replaceChildren();
    for (const [label, value] of entries) target.append(element("dt", label), factValue(element("dd"), value));
  }
  function addFactTiles(target, entries) {
    target.replaceChildren();
    for (const [label, value] of entries) {
      const tile = element("div", undefined, "fact");
      tile.append(element("dt", label), factValue(element("dd"), value));
      target.append(tile);
    }
  }
  function options(select, rows, placeholder, selected) {
    select.replaceChildren();
    if (placeholder !== null) {
      const option = element("option", placeholder);
      option.value = "";
      select.append(option);
    }
    for (const row of rows) {
      const option = element("option", row.label || row.id);
      option.value = row.id;
      select.append(option);
    }
    if ([...select.options].some((option) => option.value === selected)) select.value = selected;
  }
  function projectDescriptions() {
    const available = state.projects.available.find((row) => row.id === $("available-project").value);
    const enrolled = state.projects.enrolled.find((row) => row.id === $("enrolled-project").value);
    $("available-description").textContent = available ? `${text(available.repository)} · ${text(available.reference)}` : "Only service-allowlisted repositories can be enrolled.";
    $("project-description").textContent = enrolled ? `${text(enrolled.repository)} · ${text(enrolled.reference)}\nPipeline: ${text(enrolled.pipeline_identity)}` : "Enroll or select a project before starting a run.";
    $("enroll-button").disabled = !canOperate() || state.mutation || !available;
    $("start-button").disabled = !canOperate() || state.mutation || !enrolled;
  }
  async function loadProjects() {
    const epoch = state.epoch;
    const result = await api("/api/projects");
    if (epoch !== state.epoch) throw new DOMException("Session changed", "AbortError");
    adoptProjects(result);
  }
  function adoptProjects(result) {
    if (!Array.isArray(result.available) || !Array.isArray(result.enrolled)) throw new Error("The project response is invalid; repository choices are unavailable.");
    state.projects = result;
    const enrolledIds = new Set(result.enrolled.map((row) => row.id));
    const available = result.available.filter((row) => !enrolledIds.has(row.id));
    options($("available-project"), available, available.length ? "Select an allowlisted repository" : "No unenrolled repositories available", $("available-project").value);
    options($("enrolled-project"), result.enrolled, result.enrolled.length ? "Select an enrolled project" : "No enrolled projects", $("enrolled-project").value);
    options($("run-filter"), result.enrolled, "All projects", state.route?.project || "");
    if (result.enrolled.length === 1) $("enrolled-project").value = result.enrolled[0].id;
    projectDescriptions();
    renderProjects();
    renderRunList();
    if (activeSubPageFor(state.route) === "runbooks") window.KilnRunbooks?.refreshEnrolledProjects?.();
  }

  // r[impl kiln.web.runs_table] Service filters, cursor paging and URL selection stay authoritative;
  // header sorting reorders only the rows of the loaded page and says so.
  const RUN_SORTS = Object.freeze({
    status: { label: "Status", first: "asc", value: (run) => ui.outcomeStatus(run.outcome, run.phase).label },
    project: { label: "Project", first: "asc", value: (run) => text(projectLabel(run.project_id)) },
    revision: { label: "Revision", first: "asc", value: (run) => (typeof run.source_revision === "string" ? run.source_revision : null) },
    duration: { label: "Duration", first: "desc", value: (run) => (Number.isSafeInteger(run.duration_ms) && run.duration_ms >= 0 ? run.duration_ms : null) },
    started: { label: "Started", first: "desc", value: (run) => (Number.isSafeInteger(run.started_at_ms) ? run.started_at_ms : null) },
  });
  const RUN_COLUMNS = Object.freeze(["status", "project", "revision", "duration", "started"]);
  function compareSortValues(a, b) { return typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b), undefined, { numeric: true }); }
  function sortRunRows(runs) {
    const sort = state.runSort;
    if (!sort) return runs;
    const direction = sort.dir === "asc" ? 1 : -1;
    // Unrecorded values always follow recorded ones; they are never treated as zero.
    return runs.map((run, index) => ({ run, index, value: RUN_SORTS[sort.key].value(run) })).sort((a, b) => {
      if (a.value === null || b.value === null) return a.value === b.value ? a.index - b.index : a.value === null ? 1 : -1;
      return compareSortValues(a.value, b.value) * direction || a.index - b.index;
    }).map((entry) => entry.run);
  }
  function runDayInstant(run) { return Number.isSafeInteger(run.started_at_ms) ? run.started_at_ms : Number.isSafeInteger(run.created_at_ms) ? run.created_at_ms : null; }
  function runDayGroups(runs) {
    const now = Date.now();
    const groups = new Map();
    for (const run of runs) {
      const day = ui.localDay(runDayInstant(run), now);
      const key = day?.key || "";
      if (!groups.has(key)) groups.set(key, { key, label: day?.label || "Start not recorded", runs: [] });
      groups.get(key).runs.push(run);
    }
    const ordered = [...groups.values()];
    if (state.runSort?.key === "started") {
      const direction = state.runSort.dir === "asc" ? 1 : -1;
      ordered.sort((a, b) => (!a.key || !b.key ? (a.key ? -1 : b.key ? 1 : 0) : a.key.localeCompare(b.key) * direction));
    }
    return ordered;
  }
  function setRunSort(key) {
    const current = state.runSort;
    const first = RUN_SORTS[key].first;
    // Cycle: first direction, reversed, then back to the service order.
    state.runSort = current?.key !== key ? { key, dir: first } : current.dir === first ? { key, dir: first === "asc" ? "desc" : "asc" } : null;
    renderRunList();
  }
  function selectRunRow(id) { navigate({ page: "runs", run: id, node: "", stream: "", cursor: state.route?.cursor || "" }); }
  function runTableHead() {
    const head = element("thead");
    const row = element("tr");
    const selectHead = element("th", undefined, "run-col-select");
    selectHead.scope = "col";
    selectHead.append(element("span", "Select", "sr-only"));
    row.append(selectHead);
    for (const key of RUN_COLUMNS) {
      const cell = element("th", undefined, `run-col-${key}${key === "duration" ? " num" : ""}`);
      cell.scope = "col";
      const sorted = state.runSort?.key === key ? state.runSort.dir : "";
      if (sorted) cell.setAttribute("aria-sort", sorted === "asc" ? "ascending" : "descending");
      const button = element("button", RUN_SORTS[key].label, "sort-button");
      button.type = "button";
      button.dataset.sort = key;
      button.title = `Sort the runs on this page by ${RUN_SORTS[key].label.toLowerCase()}`;
      const indicator = ui.icon(sorted === "asc" ? "arrow-up" : sorted === "desc" ? "arrow-down" : "arrow-up-down");
      indicator.classList.add("sort-indicator");
      button.append(indicator);
      button.addEventListener("click", () => setRunSort(key));
      cell.append(button);
      row.append(cell);
    }
    const actions = element("th", undefined, "run-col-actions");
    actions.scope = "col";
    actions.append(element("span", "Actions", "sr-only"));
    row.append(actions);
    head.append(row);
    return head;
  }
  function runTableRow(run) {
    const row = element("tr", undefined, "run-row");
    row.dataset.runId = run.id;
    row.tabIndex = 0;
    row.title = `${run.id}\nSource: ${text(run.source_revision)}`;
    if (run.id === state.selected) row.setAttribute("aria-current", "true");
    const label = text(projectLabel(run.project_id));
    const cell = (key, content, className = "") => { const node = element("td", undefined, `run-col-${key} ${className}`.trim()); node.dataset.label = RUN_SORTS[key]?.label || "Actions"; node.append(...content); row.append(node); return node; };
    const selectCell = element("td", undefined, "run-col-select");
    selectCell.dataset.label = "Select";
    const checkbox = element("input");
    checkbox.type = "checkbox";
    checkbox.dataset.runSelect = run.id;
    checkbox.checked = state.selectedRuns.has(run.id);
    checkbox.setAttribute("aria-label", `Select run ${shortIdentity(run.id)} for bulk actions`);
    checkbox.addEventListener("click", (event) => event.stopPropagation());
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) state.selectedRuns.add(run.id);
      else state.selectedRuns.delete(run.id);
      renderRunsBulkToolbar();
    });
    selectCell.append(checkbox);
    row.append(selectCell);
    const status = [livePill(run.outcome, run.phase)];
    const retry = retryBadge(run);
    if (retry) status.push(retry);
    if (run.superseded_by || run.superseded_revision) {
      const superseded = element("span", "Superseded", "state-badge");
      superseded.dataset.tone = "warning";
      superseded.title = `Superseded${run.superseded_by ? ` by ${run.superseded_by}` : ""}${run.superseded_revision ? ` at ${run.superseded_revision}` : ""}`;
      status.push(superseded);
      if (run.superseded_by) {
        const jump = workspaceLink({ page: "runs", run: run.superseded_by, node: "", stream: "", view: "pipeline" }, `→ ${shortIdentity(run.superseded_by)}`, "row-link");
        jump.title = `Open superseding run ${run.superseded_by}`;
        status.push(jump);
      }
    }
    if (run.dirty_snapshot && typeof run.dirty_snapshot === "object") {
      const dirty = element("span", "Dirty snapshot", "state-badge");
      dirty.dataset.tone = "warning";
      dirty.title = `Dirty working-tree snapshot (base ${text(run.dirty_snapshot.base_revision)}, manifest ${text(run.dirty_snapshot.manifest_id)})`;
      status.push(dirty);
    }
    cell("status", status);
    const name = element("div", undefined, "run-project");
    name.append(projectLink(run.project_id, label, "run-project-link"), element("span", shortIdentity(run.id), "run-id mono"));
    cell("project", [name]);
    const revision = element("span", shortRevision(run.source_revision), "mono");
    revision.title = text(run.source_revision);
    cell("revision", [revision]);
    cell("duration", [ui.runDurationElement(run)], "num");
    cell("started", [ui.runInstantElement(run, "started_at_ms")]);
    const logs = workspaceLink({ page: "runs", run: run.id, view: "logs", node: "", stream: "" }, "Logs", "row-link");
    logs.setAttribute("aria-label", `Logs for ${label} run ${shortIdentity(run.id)}`);
    cell("actions", [logs]);
    row.addEventListener("click", (event) => { if (!(event.target instanceof Element && event.target.closest("a, button"))) selectRunRow(run.id); });
    row.addEventListener("keydown", (event) => {
      if (event.target !== row) return;
      if (event.key.toLowerCase() === "x") {
        event.preventDefault();
        if (state.selectedRuns.has(run.id)) state.selectedRuns.delete(run.id);
        else state.selectedRuns.add(run.id);
        renderRunList();
        return;
      }
      if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectRunRow(run.id); return; }
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      const rows = [...document.querySelectorAll("#run-list tr.run-row")];
      const next = rows[rows.indexOf(row) + (event.key === "ArrowDown" ? 1 : -1)];
      if (next) { event.preventDefault(); next.focus(); }
    });
    return row;
  }
  function renderRunsBulkToolbar() {
    let bar = $("runs-bulk-toolbar");
    const runBrowser = $("run-list")?.parentElement;
    if (!runBrowser) return;
    if (!bar) {
      bar = element("div", undefined, "bulk-toolbar");
      bar.id = "runs-bulk-toolbar";
      bar.setAttribute("role", "region");
      bar.setAttribute("aria-label", "Selected runs bulk actions");
      if (typeof $("run-list").before === "function") $("run-list").before(bar);
      else runBrowser.append(bar);
    }
    const ids = [...state.selectedRuns].filter((id) => state.runs.some((r) => r.id === id));
    if (!ids.length || !state.session) { bar.hidden = true; bar.replaceChildren(); return; }
    bar.hidden = false;
    const countLabel = element("strong", `${ids.length} run${ids.length === 1 ? "" : "s"} selected`);
    const actions = element("div", undefined, "cluster");
    if (canOperate()) {
      const cancelBtn = element("button", "Cancel selected…", "danger");
      cancelBtn.type = "button";
      cancelBtn.addEventListener("click", async () => {
        const choice = await confirmControl({
          title: `Cancel ${ids.length} selected run(s)?`,
          verb: `Cancel ${ids.length} run(s)`,
          dismiss: "Keep runs",
          danger: true,
          warning: "Cancels each selected run sequentially with exact run identities. Partial failures leave remaining runs selected.",
          facts: [["Selected runs", ids.join(", ")]],
          reason: { label: "Bulk cancel reason", placeholder: "Why these runs are being cancelled", required: true },
          cliCommand: ids.map((id) => `kiln runs control --run ${id} --action cancel`).join(" && "),
          opener: cancelBtn,
        });
        if (!choice.confirmed) return;
        for (const id of ids) {
          const run = state.runs.find((r) => r.id === id);
          const queuedCancel = WAITING_PHASES.has(run?.phase);
          if (!run || (!queuedCancel && !exactIdentity(run)) || run.controls?.cancel === false) {
            notice(`Skipped ${shortIdentity(id)}: exact execution identity or cancel capability is unavailable.`);
            continue;
          }
          const payload = queuedCancel
            ? { action: "cancel" }
            : { action: "cancel", attempt_id: run.attempt_id, expected_revision: run.revision, expected_sequence: run.sequence, execution: run.execution };
          const res = await mutate(`${pathFor(id)}/control`, payload, `Cancelled ${shortIdentity(id)}.`);
          if (res) state.selectedRuns.delete(id);
        }
        void readAction(loadRuns)();
      });
      const rerunBtn = element("button", "Rerun selected…", "quiet");
      rerunBtn.type = "button";
      rerunBtn.addEventListener("click", async () => {
        const choice = await confirmControl({
          title: `Rerun ${ids.length} selected run(s) at exact revisions?`,
          verb: `Rerun ${ids.length} run(s)`,
          dismiss: "Cancel",
          warning: "Starts a new candidate at each selected run's exact revision.",
          facts: [["Selected runs", ids.join(", ")]],
          reason: { label: "Bulk rerun reason", placeholder: "Why these runs are being rerun", required: true },
          cliCommand: ids.map((id) => `kiln runs rerun --run ${id} --mode exact`).join(" && "),
          opener: rerunBtn,
        });
        if (!choice.confirmed) return;
        for (const id of ids) {
          const res = await mutate(`${pathFor(id)}/rerun`, { mode: "exact" }, `Rerun requested for ${shortIdentity(id)}.`);
          if (res) state.selectedRuns.delete(id);
        }
        void readAction(loadRuns)();
      });
      actions.append(cancelBtn, rerunBtn);
    }
    const clearBtn = element("button", "Clear", "quiet");
    clearBtn.type = "button";
    clearBtn.addEventListener("click", () => { state.selectedRuns.clear(); renderRunList(); });
    actions.append(clearBtn);
    bar.replaceChildren(countLabel, actions);
  }
  function renderRunList() {
    const focused = document.activeElement?.closest?.("#run-list tr.run-row")?.dataset.runId;
    const focusedSort = document.activeElement?.closest?.("#run-list [data-sort]")?.dataset.sort;
    const runs = state.runs;
    $("runs-status").textContent = state.runsError || `${runs.length} shown of ${text(state.total)} matching retained runs · bounded page${state.connected ? "" : " · snapshot may be stale"}`;
    $("runs-next").disabled = !state.nextCursor;
    const sort = state.runSort;
    $("run-sort-note").textContent = sort ? `Sorted by ${RUN_SORTS[sort.key].label.toLowerCase()} (${sort.dir === "asc" ? "ascending" : "descending"}) within this page` : "Service order · sorting reorders this page only";
    $("run-list").replaceChildren();
    if (!runs.length && !state.runsError) {
      const clear = element("button", "Clear run filters");
      clear.type = "button";
      clear.addEventListener("click", () => navigate({ q: "", project: "", outcome: "", source: "", cursor: "" }));
      $("run-list").append(emptyState("No matching runs", "No runs match these filters. Clear filters or browse another project.", clear));
    }
    if (!runs.length) return;
    const wrap = element("div", undefined, "data-table-wrap run-table-wrap");
    const table = element("table", undefined, "data-table run-table");
    table.append(element("caption", "Retained runs on this page, grouped by local start day. Press Enter on a row to open that run. Column sorting reorders this page only.", "sr-only"), runTableHead());
    for (const group of runDayGroups(runs)) {
      const body = element("tbody", undefined, "run-day-group");
      const heading = element("tr", undefined, "run-day");
      const title = element("th", group.label);
      title.scope = "rowgroup";
      title.colSpan = RUN_COLUMNS.length + 2;
      title.append(element("span", ` · ${group.runs.length} run${group.runs.length === 1 ? "" : "s"}`, "meta"));
      heading.append(title);
      body.append(heading);
      for (const run of sortRunRows(group.runs)) body.append(runTableRow(run));
      table.append(body);
    }
    wrap.append(table);
    $("run-list").append(wrap);
    renderRunsBulkToolbar();
    if (focused) document.querySelector(`#run-list tr.run-row[data-run-id="${CSS.escape(focused)}"]`)?.focus({ preventScroll: true });
    else if (focusedSort) document.querySelector(`#run-list [data-sort="${focusedSort}"]`)?.focus({ preventScroll: true });
  }
  function adoptRun(run) {
    if (!run || run.id !== state.selected) return;
    if (state.run && Number.isSafeInteger(run.revision) && Number.isSafeInteger(state.run.revision) && run.revision < state.run.revision) return;
    if (state.run && run.revision === state.run.revision && Number.isSafeInteger(run.sequence) && Number.isSafeInteger(state.run.sequence) && run.sequence < state.run.sequence) return;
    state.run = run;
    state.observedAt = Date.now();
    renderDetail();
  }
  function adoptRuns(runs) {
    if (!Array.isArray(runs)) throw new Error("Live run data is invalid. The previous snapshot may be stale.");
    const previous = new Map(state.runs.map((run) => [run.id, run]));
    state.runs = runs.map((run) => {
      const known = previous.get(run.id);
      return known && Number.isSafeInteger(known.revision) && Number.isSafeInteger(run.revision) && known.revision > run.revision ? known : run;
    });
    renderRunList();
    const selected = state.runs.find((run) => run.id === state.selected);
    if (selected) adoptRun(selected);
  }
  // r[impl kiln.dashboard.complete.navigation]
  async function loadRuns() {
    const generation = ++state.runsGeneration;
    const route = state.route;
    const query = new URLSearchParams({ limit: "25", order: route.order });
    for (const [key, value] of [["q", route.q], ["project_id", route.project], ["outcome", route.outcome], ["source_revision", route.source], ["cursor", route.cursor]]) if (value) query.set(key, value);
    try {
      const result = await api(`/api/runs?${query}`);
      if (generation !== state.runsGeneration) return;
      state.runsError = "";
      state.nextCursor = result.next_cursor;
      state.total = result.total;
      adoptRuns(result.runs);
    } catch (error) {
      if (!isAborted(error) && generation === state.runsGeneration) {
        state.runs = []; state.nextCursor = null; state.total = 0;
        state.runsError = `Run browser unavailable: ${explain(error)} No other page was substituted. Use First page to discard an invalid cursor.`;
        renderRunList();
      }
      throw error;
    }
  }
  async function loadDetail() {
    const id = state.selected;
    const generation = ++state.detailGeneration;
    if (!id) return;
    const run = await api(pathFor(id));
    if (id !== state.selected || generation !== state.detailGeneration) return;
    adoptRun(run);
    return run;
  }
  async function selectRun(id) {
    state.selected = id;
    state.run = null;
    state.observedAt = 0;
    state.failureFirst = null;
    state.reconcileStatus = null;
    stopLiveTicker();
    dismissControl();
    resetLogs();
    state.artifactsGeneration++;
    $("artifact-list").replaceChildren();
    $("artifact-status").textContent = "Loading retained artifact metadata…";
    $("run-supersession").hidden = true;
    $("run-supersession").replaceChildren();
    $("run-reconcile").hidden = true;
    $("run-reconcile").replaceChildren();
    $("empty-run").hidden = true;
    $("run-detail").hidden = true;
    renderRunList();
    try {
      await loadDetail();
      if (state.selected === id) { await loadArtifacts(); await loadRunView(); }
    } catch (error) {
      if (isAborted(error) || id !== state.selected) return;
      $("empty-run").hidden = Boolean(state.run);
      $("empty-run-title").textContent = error.status === 404 ? "Run not found" : "Run unavailable";
      $("empty-run-message").textContent = `${id}: ${explain(error)} No other run was substituted.`;
      notice(`Run details unavailable: ${explain(error)}`);
    }
  }
  function runStatus(run) { return run ? ui.outcomeStatus(run.outcome, run.phase).status : ""; }
  function isFailedRun(run) { return runStatus(run) === "failure"; }
  function isRunningRun(run) { return runStatus(run) === "running"; }
  const FAILED_NODE_STATE = /^(failed|error)$/i;
  // Phases without a provider execution; cancel removes the exact request before dispatch.
  const WAITING_PHASES = new Set(["queued", "awaiting_approval"]);
  function livePill(outcome, phase) {
    const pill = ui.statusPill(outcome, phase);
    if (pill.dataset.status === "running") pill.classList.add("is-live");
    return pill;
  }
  // r[impl kiln.web.retry_failed]
  // A passing retry child never turns the failed parent green; it is shown as a
  // separate recorded observation and never a clean pass for required CI.
  function retryBadge(run) {
    const retries = Array.isArray(run.retries) ? run.retries : [];
    if (!retries.length || !isFailedRun(run)) return null;
    const badge = element("span", undefined, "state-badge");
    const passed = retries.some((child) => child?.outcome === "Success");
    if (passed) {
      badge.textContent = "Failed, then passed on retry";
      badge.dataset.tone = "warning";
      badge.title = "The parent run remains failed. A retry child passed on the exact same source; a partial-node retry is not a clean full run.";
    } else {
      badge.textContent = `Retried ${retries.length} time${retries.length === 1 ? "" : "s"}; not passed`;
      badge.dataset.tone = "failure";
      badge.title = "The parent run remains failed and no retry child passed.";
    }
    return badge;
  }
  // r[impl kiln.web.failure_first_run] r[impl kiln.web.failure_first_run.scenario.failed] A failed run opened without node/stream parameters defaults once to its
  // first failed node and to stderr; URL-named node and stream always win and are never rewritten.
  function failureDefaults(run) {
    if (!isFailedRun(run)) return null;
    if (state.failureFirst?.run !== run.id) {
      const failed = (Array.isArray(run.nodes) ? run.nodes : []).find((node) => FAILED_NODE_STATE.test(node.state));
      state.failureFirst = { run: run.id, node: failed?.id || "", stream: "stderr", fellBack: false, revealed: false };
    }
    return state.failureFirst;
  }
  function effectiveStream() {
    const defaults = state.failureFirst?.run === state.route?.run ? state.failureFirst : null;
    return state.route?.stream || defaults?.stream || "stdout";
  }
  function logsVisible() { return !document.querySelector('[data-run-panel="logs"]').hidden && !$("run-detail").hidden; }
  // r[impl kiln.web.live_progress] Follow an active run's selected stream unless the operator declined following for it.
  function maybeAutoFollow() {
    const run = state.run;
    if (!run || !isRunningRun(run) || state.followDeclined === run.id || $("follow-logs").checked || !$("log-node").value || !logsVisible()) return false;
    $("follow-logs").checked = true;
    state.logAutoFollow = true;
    return true;
  }
  function stopLiveTicker() {
    clearInterval(state.liveTicker);
    state.liveTicker = null;
  }
  function tickElapsed() {
    const run = state.run;
    if (!state.session || !run || !isRunningRun(run) || activeSubPageFor(state.route) !== "runs" || run.id !== state.selected) { stopLiveTicker(); return; }
    const ms = ui.elapsedSince(Number.isSafeInteger(run.started_at_ms) ? run.started_at_ms : null);
    $("run-elapsed").textContent = ui.formatDuration(ms);
    $("run-elapsed").classList.toggle("not-recorded", ms === null);
    $("run-elapsed").title = ms === null ? "Start time not recorded" : `Since recorded start ${timestamp(run.started_at_ms)}`;
  }
  function renderLiveProgress(run) {
    const running = isRunningRun(run);
    $("run-progress").hidden = !running;
    if (!running) { stopLiveTicker(); return; }
    const { complete, total } = ui.nodeProgress(run.nodes);
    $("run-progress-label").textContent = total ? `${complete} of ${total} node${total === 1 ? "" : "s"} complete` : "Node progress not recorded";
    $("run-progress-bar").hidden = !total;
    $("run-progress-bar").max = Math.max(1, total);
    $("run-progress-bar").value = complete;
    tickElapsed();
    if (!state.liveTicker && state.run === run) state.liveTicker = setInterval(tickElapsed, ELAPSED_TICK_MS);
  }
  function renderDetail() {
    const run = state.run;
    if (!run) return;
    $("empty-run").hidden = true;
    $("run-detail").hidden = false;
    $("run-title").textContent = text(projectLabel(run.project_id));
    const badges = [livePill(run.outcome, run.phase), phaseChip(run.phase)];
    const retry = retryBadge(run);
    if (retry) badges.push(retry);
    if (run.superseded_by || run.superseded_revision) {
      const superseded = element("span", "Superseded", "state-badge");
      superseded.dataset.tone = "warning";
      badges.push(superseded);
    }
    if (run.dirty_snapshot && typeof run.dirty_snapshot === "object") {
      const dirty = element("span", "Dirty snapshot", "state-badge");
      dirty.dataset.tone = "warning";
      badges.push(dirty);
    }
    $("run-badges").replaceChildren(...badges);
    const warnings = [];
    if (run.error_code) warnings.push(`Service diagnostic: ${text(run.error_code)}.`);
    if (run.outcome === "Unknown" || /unknown|recover|unavailable/i.test(text(run.phase))) warnings.push("Execution state is uncertain or recovering. This is not a successful run; do not assume provider effects completed.");
    if (!state.connected) warnings.push("Live observation is disconnected. These details may be stale.");
    $("run-warning").textContent = warnings.join(" ");
    $("run-warning").hidden = !warnings.length;
    const revision = element("span", shortIdentity(run.source_revision), "mono");
    revision.title = text(run.source_revision);
    addFactTiles($("run-facts"), [["Source revision", revision], ["Created", when(run.created_at_ms)], ["Started", ui.runInstantElement(run, "started_at_ms")], ["Completed", ui.runInstantElement(run, "completed_at_ms")], ["Duration", ui.runDurationElement(run)]]);
    renderSupersession(run);
    renderReconcile(run);
    const affected = ui.affectedSummary(run.affected);
    const dirty = run.dirty_snapshot && typeof run.dirty_snapshot === "object" ? run.dirty_snapshot : null;
    addFacts($("run-exact-facts"), [["Run ID", run.id], ["Project", run.project_id], ["Repository", run.repository], ["Source revision", run.source_revision], ["Canonical tree", run.canonical_tree], ["Run revision", run.revision], ["Attempt", run.attempt_id], ["Sequence", run.sequence], ["Parent run", run.parent_run_id], ...(run.superseded_by ? [["Superseded by", workspaceLink({ page: "runs", run: run.superseded_by, node: "", stream: "", view: "pipeline" }, run.superseded_by)]] : []), ...(run.superseded_revision ? [["Superseded revision", run.superseded_revision]] : []), ...(dirty ? [["Dirty snapshot manifest", dirty.manifest_id], ["Snapshot base revision", dirty.base_revision], ["Snapshot synthetic revision", dirty.synthetic_revision]] : []), ...(run.fanout_run_id ? [["Fan-out parent", workspaceLink({ page: "runbooks", runbook: run.runbook?.runbook_id || "", fanout: run.fanout_run_id }, run.fanout_run_id)]] : []), ["Created", timestamp(run.created_at_ms)], ["Started", timestamp(run.started_at_ms)], ["Completed", timestamp(run.completed_at_ms)], ...(affected?.identity ? [["Affected-set BLAKE3", affected.identity]] : [])]);
    renderWhy(affected);
    renderUntrusted(run);
    $("execution-identity").textContent = text(run.execution);
    renderLiveProgress(run);
    const defaults = failureDefaults(run);
    $("run-detail").toggleAttribute("data-failure-first", Boolean(defaults));
    applyRunPanels();
    const nodes = Array.isArray(run.nodes) ? run.nodes : [];
    renderGraph(run);
    renderStepTelemetry(run);
    const previousNode = $("log-node").value;
    const previousStream = $("log-stream").value;
    options($("log-node"), nodes, nodes.length ? null : "No admitted nodes", state.route.node || defaults?.node || previousNode);
    if (state.route.node && !nodes.some((node) => node.id === state.route.node)) {
      options($("log-node"), [], `Node not found: ${state.route.node}`, "");
    }
    $("log-stream").value = effectiveStream();
    const selectionChanged = (previousNode && previousNode !== $("log-node").value) || previousStream !== $("log-stream").value;
    const hadLog = state.logKey !== "" || state.logBusy;
    if (selectionChanged) resetLogs();
    $("read-log").disabled = state.logBusy || !$("log-node").value;
    $("restart-log").disabled = state.logBusy || !$("log-node").value;
    const diagnostic = $("log-stream").value === "diagnostic";
    $("log-search").disabled = diagnostic;
    $("log-search-help").textContent = diagnostic ? "Search is unavailable for native diagnostic streams. Your worker-stream search is kept for stdout/stderr." : "Search retained stdout/stderr from the beginning; changing the search loads matching output.";
    $("follow-logs").disabled = !$("log-node").value;
    $("download-log").disabled = !$("log-node").value;
    renderFailureDigest(run);
    void probeFailureDigestLogs(run);
    updateControlButtons();
    // A live transition (for example to failed or to running) re-reads the newly defaulted stream or starts following it.
    if (selectionChanged && hadLog && logsVisible()) { maybeAutoFollow(); void readLog(false); }
    else if (maybeAutoFollow() && state.logKey && !state.logBusy) void readLog(true);
  }
  // r[impl kiln.monorepo.console] Why this ran: the recorded affected reasons and changed-file facts.
  // Runs recorded without them (older or unscoped manual runs) show nothing here.
  function renderWhy(affected) {
    const target = $("run-why");
    target.hidden = !affected;
    if (!affected) { target.replaceChildren(); return; }
    const heading = element("h3", "Why this ran");
    heading.id = "run-why-title";
    target.replaceChildren(heading);
    if (affected.changed) target.append(element("p", affected.changed, "meta"));
    target.append(affected.reasons.length ? ui.reasonList(affected.reasons) : element("p", "No reason was recorded for this run.", "small"));
  }
  // r[impl kiln.untrusted.console] An untrusted pull-request run's gate: state, reason, approver, approved commit and
  // whether its result counts for required CI. Trusted runs carry no gate and show nothing here.
  function renderUntrusted(run) {
    const target = $("run-untrusted");
    const gate = ui.untrustedSummary(run.untrusted);
    target.hidden = !gate;
    if (!gate) { target.replaceChildren(); return; }
    const heading = element("h3", "Untrusted pull request");
    heading.id = "run-untrusted-title";
    const head = element("div", undefined, "run-untrusted-head");
    head.append(heading, statePill(gate.tone, gate.stateLabel, gate.state));
    const approver = gate.approvedBy === null ? "Not approved" : gate.approvedById && gate.approvedById !== gate.approvedBy ? `${gate.approvedBy} (${gate.approvedById})` : gate.approvedBy;
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [
      ["Why untrusted", gate.reasonText],
      ["Pull request", gate.pullRequest === null ? null : `#${gate.pullRequest}`],
      ["Head repository", gate.headRepository],
      ["Exact commit", gate.sourceCommit],
      ["Approved by", approver],
      ["Approved commit", gate.approvedCommit ?? "Not approved"],
      ["Approved", gate.approvedAtMs === null ? "Not approved" : timestamp(gate.approvedAtMs)],
      ["Counts for required CI", gate.countsForRequiredCi ? "Yes, for this exact commit only" : "No"],
      ...(gate.supersededBy ? [["Superseded by", workspaceLink({ page: "runs", run: gate.supersededBy, node: "", stream: "", view: "pipeline" }, gate.supersededBy)]] : []),
    ]);
    const note = gate.awaitingApproval ? "Nothing runs until an operator approves this exact commit; a new commit needs a new approval." : "Approved untrusted runs and their retries run only on untrusted-capable runners, without secrets, workload token or cache.";
    target.replaceChildren(head, facts, element("p", `${note} Approving a run is not a code-review approval.`, "small"));
  }
  // r[impl kiln.web.run_supersession_and_reconcile] Surface RunView.superseded_by, superseded_revision, and dirty_snapshot with drift notice and direct jump.
  function renderSupersession(run) {
    const target = $("run-supersession");
    const supersededBy = typeof run.superseded_by === "string" && run.superseded_by ? run.superseded_by : "";
    const supersededRevision = typeof run.superseded_revision === "string" && run.superseded_revision ? run.superseded_revision : "";
    const dirty = run.dirty_snapshot && typeof run.dirty_snapshot === "object" ? run.dirty_snapshot : null;
    const active = Boolean(supersededBy || supersededRevision || dirty);
    target.hidden = !active;
    if (!active) { target.replaceChildren(); return; }
    const head = element("div", undefined, "run-supersession-head");
    const heading = element("h3", supersededBy || supersededRevision ? "Target drift · superseded run" : "Dirty working-tree snapshot");
    heading.id = "run-supersession-title";
    head.append(heading);
    const messages = [];
    if (supersededBy || supersededRevision) messages.push("This run was superseded by a newer target revision. Its result does not satisfy the moved target.");
    if (dirty) messages.push("Dirty working-tree snapshot: this run executed against a synthetic snapshot rather than a clean committed revision.");
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [
      ...(supersededBy ? [["Superseded by", workspaceLink({ page: "runs", run: supersededBy, node: "", stream: "", view: "pipeline" }, supersededBy)]] : []),
      ...(supersededRevision ? [["Superseding revision", supersededRevision]] : []),
      ...(dirty ? [["Snapshot manifest", dirty.manifest_id], ["Base revision", dirty.base_revision], ["Synthetic tree revision", dirty.synthetic_revision]] : []),
    ]);
    const actions = element("div", undefined, "run-supersession-actions");
    if (supersededBy) {
      const jump = workspaceLink({ page: "runs", run: supersededBy, node: "", stream: "", view: "pipeline" }, "Open superseding run", "button");
      actions.append(jump);
    }
    const gate = ui.untrustedSummary(run.untrusted);
    if (canOperate() && !gate && !run.runbook && !dirty && (supersededBy || supersededRevision)) {
      const rebuild = element("button", "Run latest admitted reference…", "quiet");
      rebuild.type = "button";
      rebuild.disabled = state.mutation;
      rebuild.addEventListener("click", () => { void prepareRerun("latest"); });
      actions.append(rebuild);
    }
    target.replaceChildren(head, element("p", messages.join(" "), "small"), facts, ...(actions.childElementCount ? [actions] : []));
  }
  // r[impl kiln.web.run_supersession_and_reconcile] First-class Unknown Reconciliation Card calling POST /api/runs/{id}/reconcile without blind reruns.
  async function reconcileRun(runId) {
    const result = await mutate(`${pathFor(runId)}/reconcile`, {}, "Provider reconciliation probed.");
    if (!result || !state.session) return;
    if (result.run && typeof result.run === "object") {
      state.runs = state.runs.map((item) => item.id === result.run.id ? result.run : item);
      renderRunList();
      if (state.selected === result.run.id) {
        state.reconcileStatus = {
          runId: result.run.id,
          stillUnknown: Boolean(result.still_unknown),
          message: typeof result.note === "string" && result.note ? result.note : result.still_unknown ? "Provider remains unreachable; run stays Unknown." : `Reconciled: ${text(result.outcome)} (${text(result.phase)}).`,
        };
        adoptRun(result.run);
        if (result.still_unknown) notice(state.reconcileStatus.message, "warning");
      }
    }
  }
  function renderReconcile(run) {
    const target = $("run-reconcile");
    const isUnknown = run.outcome === "Unknown" || text(run.phase).toLowerCase() === "unknown";
    target.hidden = !isUnknown;
    if (!isUnknown) { target.replaceChildren(); return; }
    const head = element("div", undefined, "run-reconcile-head");
    const heading = element("h3", "Unknown execution · Reconciliation required");
    heading.id = "run-reconcile-title";
    head.append(heading, statePill("unknown", "Unknown", run.outcome));
    const guardrail = element("p", "Execution outcome is Unknown: provider completion or termination could not be confirmed. Do not blindly rerun or assume failure; probe the provider first to reconcile this execution without creating a replacement run.", "small");
    const actions = element("div", undefined, "run-reconcile-actions");
    const probe = element("button", "Reconcile / Probe Provider", "primary");
    probe.id = "reconcile-run";
    probe.type = "button";
    probe.disabled = !canOperate() || state.mutation;
    probe.addEventListener("click", () => { void reconcileRun(run.id); });
    actions.append(probe);
    if (!canOperate()) actions.append(element("span", "Operator access is required to probe the provider.", "small"));
    const statusLine = element("p", state.reconcileStatus?.runId === run.id ? state.reconcileStatus.message : "", "small");
    statusLine.id = "run-reconcile-status";
    statusLine.setAttribute("role", "status");
    statusLine.hidden = !statusLine.textContent;
    target.replaceChildren(head, guardrail, actions, statusLine);
  }
  function renderStepTelemetry(run) {
    const target = $("step-telemetry");
    target.replaceChildren();
    const records = Array.isArray(run.step_telemetry) ? run.step_telemetry : [];
    if (!records.length) { target.append(element("p", "Not recorded: no worker step measurements were recorded for this execution.", "small")); return; }
    const observed = (value, unit) => Number.isSafeInteger(value) && value >= 0 ? `${value.toLocaleString()} ${unit}` : "Not recorded";
    for (const record of records) {
      const observation = record.observation || {};
      const step = observation.step || {};
      const panel = element("article", undefined, "telemetry-step");
      panel.append(element("h4", `${text(observation.binding?.node_id)} · reviewed step ${text(step.step_index)} · sequence ${text(observation.sequence)}`));
      const facts = element("dl", undefined, "facts");
      addFacts(facts, [["Started", when(step.started_at_ms)], ["Completed", when(step.completed_at_ms)], ["Measured elapsed", elapsed(step.duration_ms)], ["Exit code", step.exit_code], ["Observed CPU", observed(step.cpu_time_us, "µs")], ["Observed peak memory", observed(step.peak_memory_bytes, "bytes")]]);
      panel.append(facts);
      for (const cache of step.cache || []) panel.append(element("p", `Shared cache ${text(cache.outcome)} · ${observed(cache.transferred_bytes, "bytes transferred")} · ${duration(cache.duration_ms)} · key ${text(cache.key)}`, "small identity"));
      target.append(panel);
    }
  }
  function exactIdentity(run) {
    return run && run.attempt_id && Number.isSafeInteger(run.revision) && run.revision >= 0 && run.execution && Number.isSafeInteger(run.sequence) && run.sequence >= 0;
  }
  function updateControlButtons() {
    const run = state.run;
    const fresh = state.connected && Date.now() - state.observedAt < CONTROL_FRESHNESS_MS;
    const waiting = WAITING_PHASES.has(run?.phase);
    const gate = ui.untrustedSummary(run?.untrusted);
    for (const action of ["cancel", "pause", "resume"]) $("control-" + action).disabled = !canOperate() || state.mutation || !fresh || !(exactIdentity(run) || (action === "cancel" && waiting)) || run.controls?.[action] !== true;
    // Untrusted and runbook runs retry only their exact approved commit; the service refuses latest-source retries.
    for (const mode of ["exact", "latest", "failed"]) $("rerun-" + mode).disabled = !canOperate() || state.mutation || !run || (mode === "latest" && (Boolean(gate) || Boolean(run.runbook))) || (mode === "failed" && !run.nodes?.some((node) => FAILED_NODE_STATE.test(node.state)));
    $("control-note").textContent = !canOperate() ? "Read-only principal. Execution mutations are disabled." : gate?.awaitingApproval ? "This untrusted pull-request run waits for an operator to approve its exact commit. Nothing has executed. Cancel removes it before dispatch." : run?.phase === "queued" ? "Queued work has no provider execution yet. Cancel removes this exact queued request before dispatch." : !run ? "Select a run to inspect controls." : !exactIdentity(run) ? "Exact control identity unavailable. No approximate identity will be sent." : !fresh ? "Snapshot stale or live observation disconnected. Refresh before control." : "Only advertised capabilities are enabled. Fresh reads and confirmation bind controls to the exact execution.";
    // r[impl kiln.web.failure_first_run] Header shortcuts mirror the disclosure controls: same enablement, same confirmation.
    const operate = canOperate() && Boolean(run);
    // r[impl kiln.untrusted.console] Approval is offered to operators only, while the run still awaits it.
    $("header-approve").hidden = !(operate && gate?.awaitingApproval && gate.sourceCommit);
    $("header-approve").disabled = state.mutation || !fresh;
    $("header-cancel").hidden = !(operate && run.controls?.cancel === true);
    $("header-cancel").disabled = $("control-cancel").disabled;
    $("header-rerun").hidden = !(operate && !["running", "queued"].includes(runStatus(run)));
    $("header-rerun").disabled = $("rerun-exact").disabled;
  }

  function subscribeEvents(listener) {
    if (typeof listener !== "function") return () => {};
    state.eventSubscribers.add(listener);
    return () => { state.eventSubscribers.delete(listener); };
  }
  function emitEvent(eventName, payload) {
    for (const listener of state.eventSubscribers) {
      try { listener(eventName, payload); } catch (_) {}
    }
  }
  function parseSseBlock(block) {
    if (!block || !block.trim()) return null;
    let event = "message";
    let comment = "";
    const data = [];
    for (const line of block.split("\n")) {
      if (line.startsWith(":")) comment = line.slice(1).trim();
      else if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
    }
    if (data.length) return { event, data: data.join("\n") };
    if (comment) return { event: "heartbeat", data: "" };
    return null;
  }
  function applySseFrame(frame) {
    if (!frame) return false;
    if (frame.event === "heartbeat") {
      state.observedAt = Date.now();
      connection("Live · authenticated", true);
      emitEvent("heartbeat", {});
      return true;
    }
    let payload = null;
    try { payload = frame.data ? JSON.parse(frame.data) : {}; } catch { throw new Error("Invalid SSE event payload."); }
    if (frame.event === "ready") {
      state.events.ready = payload;
      state.observedAt = Date.now();
      connection("Live · authenticated", true);
      emitEvent("ready", payload);
      return true;
    }
    if (frame.event === "ci" || frame.event === "snapshot") {
      if (!Number.isSafeInteger(payload.version) || payload.version < 0 || !Array.isArray(payload.runs)) {
        throw new Error("Invalid live CI snapshot identity; the previous view may be stale.");
      }
      if (BigInt(payload.version) < BigInt(state.liveVersion)) {
        throw new Error("The live version moved backwards. Refresh to re-establish service state.");
      }
      const versionChanged = String(payload.version) !== state.liveVersion;
      state.liveVersion = String(payload.version);
      state.liveRunning = ui.liveActivity(payload.runs).running;
      state.events.ci = payload;
      state.observedAt = Date.now();
      connection("Live · authenticated", true);
      const incoming = new Map(payload.runs.map((run) => [run.id, run]));
      const activeSubPage = state.route?.tab || state.route?.page;
      if (!versionChanged) renderRunList();
      else if (activeSubPage === "runs") void readAction(loadRuns)();
      else if (["overview", "workers", "operations", "activity", "project"].includes(activeSubPage)) void readAction(loadPage)();
      if (state.selected && incoming.has(state.selected)) adoptRun(incoming.get(state.selected));
      emitEvent("ci", payload);
      return true;
    }
    if (frame.event === "fleet") {
      state.events.fleet = payload;
      state.events.fleetObservedAt = Date.now();
      updateGlobalLiveStrip();
      emitEvent("fleet", payload);
      if (state.route?.page === "overview" && !payload?.unavailable) void refreshFactoryFromSse("fleet");
      return true;
    }
    if (frame.event === "cairn") {
      state.events.cairn = payload;
      emitEvent("cairn", payload);
      if (state.route?.page === "overview") void refreshFactoryFromSse("cairn");
      else if ((state.route?.tab || state.route?.page) === "cairn") void readAction(loadPage)();
      return true;
    }
    if (frame.event === "attention") {
      state.events.attention = payload;
      updateGlobalLiveStrip();
      emitEvent("attention", payload);
      if (state.route?.page === "overview" || (state.route?.page === "inbox" && (state.route?.tab || "attention") === "attention")) {
        void refreshFactoryFromSse("attention");
      }
      return true;
    }
    return false;
  }
  function scheduleLive(delay, generation) {
    clearTimeout(state.liveTimer);
    state.liveTimer = setTimeout(() => live(generation), delay);
  }
  // r[impl kiln.web.multiplexed_sse_events] Single shared /api/events SSE stream per browser tab.
  async function live(generation) {
    if (!state.session || generation !== state.liveGeneration) return;
    const controller = new AbortController();
    state.liveController = controller;
    try {
      const body = await apiStream("/api/events", { controller });
      if (generation !== state.liveGeneration) { await body.cancel?.().catch(() => {}); return; }
      const reader = body.getReader();
      const decoder = new TextDecoder("utf-8", { fatal: false });
      let buffer = "";
      let recognized = false;
      try {
        while (true) {
          const part = await reader.read();
          if (generation !== state.liveGeneration || !state.session) return;
          if (part.done) break;
          buffer += decoder.decode(part.value, { stream: true });
          buffer = buffer.replace(/\r\n/g, "\n");
          let split;
          while ((split = buffer.indexOf("\n\n")) >= 0) {
            const block = buffer.slice(0, split);
            buffer = buffer.slice(split + 2);
            const parsed = parseSseBlock(block);
            if (applySseFrame(parsed)) {
              recognized = true;
              state.liveFailures = 0;
            }
          }
        }
        buffer += decoder.decode();
        buffer = buffer.replace(/\r\n/g, "\n");
        if (buffer.trim()) {
          for (const block of buffer.split(/\n\n/)) {
            const parsed = parseSseBlock(block);
            if (applySseFrame(parsed)) recognized = true;
          }
        }
      } finally {
        reader.releaseLock();
      }
      if (!recognized) throw new Error("The live endpoint returned no event or heartbeat.");
      state.liveFailures = 0;
      connection("Live · authenticated", true);
      scheduleLive(LIVE_DELAY_MS, generation);
    } catch (error) {
      if (generation !== state.liveGeneration || isAborted(error) || !state.session) return;
      state.liveFailures++;
      const stopped = state.liveFailures >= MAX_LIVE_FAILURES;
      connection(stopped ? "Disconnected · reconnect required" : `Stale · reconnect ${state.liveFailures}/${MAX_LIVE_FAILURES}`, false);
      renderRunList();
      if (state.run) renderDetail();
      notice(`Live observation unavailable: ${explain(error)}${stopped ? " Automatic reconnect stopped. Use Refresh to reconnect." : " Read-only reconnect is bounded; no mutations are retried."}`);
      if (!stopped) scheduleLive(Math.min(MAX_BACKOFF_MS, LIVE_DELAY_MS * 2 ** (state.liveFailures - 1)), generation);
    } finally {
      if (state.liveController === controller) state.liveController = null;
    }
  }
  function startLive() {
    clearTimeout(state.liveTimer);
    state.liveController?.abort();
    state.liveFailures = 0;
    const generation = ++state.liveGeneration;
    connection("Connecting live observations…", false);
    void live(generation);
  }
  async function refreshAll() {
    notice("");
    state.liveVersion = "0";
    state.liveRunning = null;
    startLive();
    await Promise.all([loadProjects(), loadRuns(), state.selected ? loadDetail() : Promise.resolve(), loadPage()]);
  }

  function setMutation(busy) {
    state.mutation = busy;
    projectDescriptions();
    updateControlButtons();
    $("confirm-control").disabled = busy;
  }
  async function mutate(target, payload, success) {
    if (state.mutation || !canOperate()) { if (!canOperate()) notice("This principal has read-only access. Nothing was sent."); return null; }
    const requestId = payload?.request_id || crypto.randomUUID();
    const epoch = state.epoch;
    setMutation(true);
    notice("");
    try {
      const result = await api(target, { method: "POST", body: { ...payload, request_id: requestId } });
      if (epoch !== state.epoch) return null;
      notice(`${success} Request ${requestId}.`, "success");
      return result;
    } catch (error) {
      if (!isAborted(error) && epoch === state.epoch) notice(`${explain(error)} Request ${requestId}. This mutation was not retried. Refresh admitted state before deciding whether to issue a new request; a timeout or connection failure may still have applied.`);
      return null;
    } finally {
      if (epoch === state.epoch) setMutation(false);
    }
  }
  const DEFAULT_CONFIRM_WARNING = "This affects the exact execution/source below. Mutations are never automatically retried. Unknown effects require observation before a new request.";
  function resetConfirmReason() {
    if ($("confirm-reason-field")) $("confirm-reason-field").hidden = true;
    if ($("confirm-reason")) { $("confirm-reason").value = ""; $("confirm-reason").required = false; }
    if ($("confirm-reason-error")) { $("confirm-reason-error").textContent = ""; $("confirm-reason-error").hidden = true; }
    if ($("confirm-cli-parity")) $("confirm-cli-parity").hidden = true;
    if ($("confirm-cli-cmd")) $("confirm-cli-cmd").textContent = "";
    if ($("confirm-cli-status")) $("confirm-cli-status").textContent = "";
    if ($("confirm-warning")) $("confirm-warning").textContent = DEFAULT_CONFIRM_WARNING;
  }
  function dismissControl(resolved = false) {
    const pending = state.confirm;
    state.confirm = null;
    if ($("control-dialog").open) $("control-dialog").close();
    $("confirm-facts").replaceChildren();
    resetConfirmReason();
    if (!resolved && pending?.resolve) pending.resolve({ confirmed: false, reason: "", requestId: pending.requestId || "" });
    if (!resolved) pending?.opener?.focus?.();
  }
  // r[impl kiln.web.design_system] Confirmation copy names the run and the verb: the title asks the decision, the
  // commit repeats the verb (danger only for cancellation) and the dismissal names the state that is kept.
  function confirmWording(title, verb, dismiss, danger) {
    resetConfirmReason();
    $("confirm-title").textContent = title;
    $("confirm-control").textContent = verb;
    $("confirm-control").className = danger ? "danger" : "primary";
    $("dismiss-control").textContent = dismiss;
  }
  function confirmControl({ title, verb, dismiss = "Cancel", danger = false, warning = DEFAULT_CONFIRM_WARNING, facts = [], reason = null, cliCommand = "", requestId = crypto.randomUUID(), opener = null }) {
    dismissControl();
    return new Promise((resolve) => {
      const maxReason = reason?.maxLength || 1024;
      state.confirm = {
        resolve,
        opener: opener || document.activeElement,
        requireReason: Boolean(reason?.required),
        maxReason,
        requestId,
      };
      confirmWording(title, verb, dismiss, danger);
      if ($("confirm-warning")) $("confirm-warning").textContent = warning || DEFAULT_CONFIRM_WARNING;
      const renderedFacts = [...facts];
      if (requestId && !renderedFacts.some(([k]) => k === "Request ID")) renderedFacts.push(["Request ID", requestId]);
      addFacts($("confirm-facts"), renderedFacts);
      if (reason && $("confirm-reason-field") && $("confirm-reason")) {
        $("confirm-reason-field").hidden = false;
        if ($("confirm-reason-label")) $("confirm-reason-label").textContent = reason.label || "Reason";
        $("confirm-reason").placeholder = reason.placeholder || "";
        $("confirm-reason").maxLength = maxReason;
        $("confirm-reason").required = Boolean(reason.required);
        $("confirm-reason").value = reason.initial || "";
      }
      if (cliCommand && $("confirm-cli-parity") && $("confirm-cli-cmd")) {
        $("confirm-cli-parity").hidden = false;
        $("confirm-cli-cmd").textContent = cliCommand;
      }
      $("control-dialog").showModal();
      (reason?.required && $("confirm-reason") ? $("confirm-reason") : $("dismiss-control")).focus();
    });
  }
  async function prepareControl(action) {
    if (!canOperate()) return;
    const id = state.selected;
    try {
      const freshRun = await loadDetail();
      const run = state.run;
      const queuedCancel = action === "cancel" && WAITING_PHASES.has(run?.phase);
      if (!freshRun || state.selected !== id || (!queuedCancel && !exactIdentity(run)) || run.controls?.[action] !== true || !state.connected) throw new Error("This exact execution no longer advertises the requested control. Nothing was sent.");
      const identity = queuedCancel ? { action } : structuredClone({ action, attempt_id: run.attempt_id, expected_revision: run.revision, expected_sequence: run.sequence, execution: run.execution });
      state.confirm = { id, identity };
      const subject = `"${text(projectLabel(run.project_id))}"`;
      const wording = { cancel: [`Cancel run ${subject}?`, "Cancel run", queuedCancel ? (run.phase === "awaiting_approval" ? "Keep awaiting approval" : "Keep queued") : "Keep running"], pause: [`Pause run ${subject}?`, "Pause run", "Keep running"], resume: [`Resume run ${subject}?`, "Resume run", "Keep paused"] }[action];
      confirmWording(...wording, action === "cancel");
      addFacts($("confirm-facts"), [["Run", id], ["Source", run.source_revision], ["Attempt", identity.attempt_id], ["Revision", identity.expected_revision], ["Sequence", identity.expected_sequence], ["Execution", identity.execution]]);
      if (action === "cancel" && $("confirm-cli-parity") && $("confirm-cli-cmd")) {
        $("confirm-cli-parity").hidden = false;
        $("confirm-cli-cmd").textContent = `kiln runs control --run ${id} --action cancel`;
      }
      $("control-dialog").showModal();
      $("dismiss-control").focus();
    } catch (error) { if (!isAborted(error)) notice(explain(error)); }
  }
  // r[impl kiln.untrusted.console] Approval goes through the shared confirmation dialog and binds the exact commit
  // shown; the service rechecks the interactive operator, the commit and the untrusted-capable runner.
  async function prepareApprove() {
    if (!canOperate()) return;
    const id = state.selected;
    try {
      const freshRun = await loadDetail();
      const run = state.run;
      const gate = ui.untrustedSummary(run?.untrusted);
      if (!freshRun || state.selected !== id || !gate?.awaitingApproval || !gate.sourceCommit || !state.connected) throw new Error("This run no longer awaits approval of an exact commit. Nothing was sent.");
      state.confirm = { id, approve: { source_commit: gate.sourceCommit } };
      confirmWording(`Approve untrusted run "${text(projectLabel(run.project_id))}"?`, "Approve and run", "Keep blocked", false);
      addFacts($("confirm-facts"), [["Run", id], ["Pull request", gate.pullRequest === null ? null : `#${gate.pullRequest}`], ["Head repository", gate.headRepository], ["Exact commit", gate.sourceCommit], ["Why untrusted", gate.reasonText], ["Execution", "Untrusted-capable runner only, without secrets, workload token or cache, including retries"], ["Review effect", "None. This is not a code-review approval"]]);
      $("control-dialog").showModal();
      $("dismiss-control").focus();
    } catch (error) { if (!isAborted(error)) notice(explain(error)); }
  }

  function stopLogFollow() {
    clearTimeout(state.logTimer);
    state.logTimer = null;
  }
  function resetLogs() {
    stopLogFollow();
    state.logGeneration++;
    state.logBusy = false;
    state.logNext = "0";
    state.logKey = "";
    $("follow-logs").checked = false;
    state.logAutoFollow = false;
    $("log-output").textContent = "";
    $("log-gap").textContent = "";
    $("log-gap").hidden = true;
    $("log-cursor").value = "0";
    $("log-position").textContent = "No log read yet";
    $("log-status").textContent = "Select a node and read actual retained logs. Live worker bytes can arrive before command completion; diagnostics are native provider logs.";
    $("log-error-bar").hidden = true;
    $("log-error-count").textContent = "";
  }
  // r[impl kiln.web.failure_first_run] Error lines are wrapped in inert <mark> elements built from text nodes;
  // the displayed text (textContent) stays byte-for-byte the retained text.
  function renderLogText(view, output) {
    const matches = ui.errorLines(output);
    if (!matches.length) { view.textContent = output; return; }
    const fragment = document.createDocumentFragment();
    const lines = output.split("\n");
    let pending = "";
    let match = 0;
    lines.forEach((line, index) => {
      const separator = index === lines.length - 1 ? "" : "\n";
      if (matches[match] !== index) { pending += line + separator; return; }
      if (pending) fragment.append(pending);
      const mark = element("mark", line, "log-error-line");
      mark.tabIndex = -1;
      fragment.append(mark);
      pending = separator;
      match++;
    });
    if (pending) fragment.append(pending);
    view.replaceChildren(fragment);
  }
  const ERROR_LINE_CAP = 1000;
  function updateErrorBar() {
    const view = $("log-output");
    const count = view.querySelectorAll("mark.log-error-line").length;
    const failed = isFailedRun(state.run);
    $("log-error-bar").hidden = !view.textContent || (!count && !failed);
    $("log-error-count").textContent = count ? `${count >= ERROR_LINE_CAP ? `${ERROR_LINE_CAP}+` : count} error line${count === 1 ? "" : "s"}` : "No error-pattern lines in the loaded text";
    $("log-jump-error").hidden = !count;
  }
  function scrollLogToLine(mark) {
    const view = $("log-output");
    view.scrollTop = Math.max(0, mark.offsetTop - view.clientHeight / 4);
  }
  function jumpToFirstError() {
    const mark = $("log-output").querySelector("mark.log-error-line");
    if (!mark) return;
    $("log-output").scrollIntoView({ block: "nearest" });
    scrollLogToLine(mark);
    mark.focus({ preventScroll: true });
  }
  function cursorValue(value) {
    if (typeof value === "number" && !Number.isSafeInteger(value)) throw new Error("The service returned a cursor that cannot be represented exactly. No approximate cursor was used.");
    const source = String(value);
    if (!/^\d+$/.test(source) || BigInt(source) > 18446744073709551615n) throw new Error("A stream cursor must be an unsigned 64-bit decimal integer.");
    return BigInt(source).toString();
  }
  // r[impl kiln.dashboard.logs.continuation]
  // r[impl kiln.dashboard.complete.follow]
  async function readLog(follow = false) {
    if (state.logBusy || !state.selected || !$("log-node").value) return;
    const id = state.selected;
    const node = $("log-node").value;
    const stream = $("log-stream").value;
    const search = stream === "diagnostic" ? "" : $("log-search").value.trim();
    const key = `${id}\0${node}\0${stream}\0${search}`;
    let cursor;
    try { cursor = cursorValue(follow ? state.logNext : $("log-cursor").value); }
    catch (error) { $("log-status").textContent = explain(error); return; }
    stopLogFollow();
    const append = key === state.logKey && cursor === state.logNext;
    const generation = ++state.logGeneration;
    state.logBusy = true;
    $("read-log").disabled = true;
    $("restart-log").disabled = true;
    $("log-status").textContent = "Reading retained stream cursor…";
    let fallback = false;
    let emptyRead = false;
    try {
      const maximum = state.run?.log_bounds?.maximum_chunk_bytes;
      if (!Number.isSafeInteger(maximum) || maximum <= 0) throw new Error("The run has no valid admitted log-read bound.");
      const readBytes = Math.min(LOG_READ_BYTES, maximum);
      const query = new URLSearchParams({ node_id: node, stream, cursor, max_bytes: String(readBytes) });
      if (search) query.set("q", search);
      const result = await api(`${pathFor(id)}/logs?${query}`, { quiet: follow });
      if (generation !== state.logGeneration || id !== state.selected) return;
      if (result.node_id !== node || result.stream !== stream || typeof result.text !== "string") throw new Error("The returned log identity does not match the selected node and stream.");
      const expectedSource = stream === "diagnostic" ? "native-provider" : "worker-capture";
      if (result.source !== expectedSource) throw new Error("The returned log source does not match the selected stream.");
      const sourceLabel = result.source === "worker-capture" ? "Worker-captured output · byte cursor" : "Native provider diagnostic · native cursor";
      const next = cursorValue(result.next_cursor);
      const returned = cursorValue(result.cursor);
      if (BigInt(returned) < BigInt(cursor) || BigInt(next) < BigInt(returned)) throw new Error("The stream cursor moved backwards; automatic following stopped.");
      const gaps = [];
      if (result.gap) gaps.push(`Source reports a log gap: ${text(result.gap)}. Missing bytes are not reconstructed.`);
      if (returned !== cursor) gaps.push(search ? `Search advanced from cursor ${cursor} to ${returned}; nonmatching bytes are not displayed.` : `Source returned cursor ${returned} instead of requested ${cursor}.`);
      if (result.truncated) gaps.push("Captured output or this read is truncated. Missing bytes are not reconstructed; retained bytes may remain at the next cursor.");
      const view = $("log-output");
      const scrollTop = view.scrollTop;
      const followTail = $("follow-logs").checked && view.scrollHeight - view.clientHeight - scrollTop <= LOG_TAIL_SLOP_PX;
      const previousOutput = view.textContent;
      let output = (append ? previousOutput : "") + result.text;
      if (output.length > MAX_DISPLAYED_LOG_CHARS) {
        output = output.slice(-MAX_DISPLAYED_LOG_CHARS);
        gaps.push("Older displayed log text was removed at this page’s memory bound. Re-read an earlier stream cursor to inspect retained bytes.");
      }
      if (output !== previousOutput) renderLogText(view, output);
      updateErrorBar();
      if (!append) { $("log-gap").textContent = ""; $("log-gap").hidden = true; }
      if (gaps.length) {
        $("log-gap").textContent = [$("log-gap").hidden ? "" : $("log-gap").textContent, ...gaps].filter(Boolean).join("\n").slice(-8192);
        $("log-gap").hidden = false;
      }
      state.logKey = key;
      state.logNext = next;
      $("log-cursor").value = next;
      $("log-position").textContent = `${sourceLabel} · ${returned} → ${next} · retained: ${text(result.retained)}`;
      if (result.text) $("log-status").textContent = `${sourceLabel}. Actual retained bytes loaded; text is displayed inertly.`;
      else if (search) $("log-status").textContent = "No further matches at this cursor. Previously loaded matching text is retained.";
      else {
        $("log-status").textContent = `No new ${stream} bytes at cursor ${next}. Previously loaded text is retained; an empty read does not imply success or complete logs.`;
        if (stream === "stdout" && !output) $("log-status").textContent += " Check stderr for compiler/build diagnostics.";
      }
      const defaults = state.failureFirst?.run === id && !state.route.stream ? state.failureFirst : null;
      if (defaults?.fellBack && stream === "stdout") $("log-status").textContent += " No stderr bytes were retained for this node, so its stdout is shown; choose stderr to return.";
      // Failure-first: an empty retained stderr falls back to stdout once, only when the URL names no stream.
      const jump = state.digestJump?.run === id && state.digestJump?.node === node ? state.digestJump : null;
      fallback = Boolean((defaults || jump) && stream === "stderr" && (defaults?.stream === "stderr" || jump?.stream === "stderr") && !append && !search && cursor === "0" && next === "0" && !result.text);
      if (fallback && defaults) { defaults.stream = "stdout"; defaults.fellBack = true; }
      if (fallback && jump) { jump.stream = "stdout"; }
      emptyRead = !result.text;
      if (result.text && state.logDigestCache) {
        const lines = output.split("\n");
        const errIdxs = ui.errorLines(output);
        const firstIdx = errIdxs.length ? errIdxs[0] : -1;
        const firstErrLine = firstIdx >= 0 ? lines[firstIdx] : "";
        let runCache = state.logDigestCache.get(id);
        if (!runCache) { runCache = new Map(); state.logDigestCache.set(id, runCache); }
        if (firstErrLine || !runCache.has(node)) {
          runCache.set(node, { stream, firstErrorLine: firstErrLine, lineIndex: firstIdx, errorCount: errIdxs.length });
          if (state.run?.id === id) renderFailureDigest(state.run);
        }
      }
      const firstError = !append && !followTail && (defaults || jump) ? view.querySelector("mark.log-error-line") : null;
      view.scrollTop = followTail ? view.scrollHeight : append ? scrollTop : 0;
      if (firstError) {
        scrollLogToLine(firstError);
        // First read of a failed run: bring the highlighted output into view once, without moving focus.
        if (defaults && !defaults.revealed) { defaults.revealed = true; $("log-output").scrollIntoView({ block: "nearest" }); }
        if (jump) { state.digestJump = null; jumpToFirstError(); }
      } else if (jump && !fallback) {
        state.digestJump = null;
      }
    } catch (error) {
      if (!isAborted(error) && generation === state.logGeneration) {
        $("log-status").textContent = `Logs unavailable: ${explain(error)} Following stopped; retained displayed bytes may be stale.`;
        $("follow-logs").checked = false;
      }
    } finally {
      if (generation === state.logGeneration) {
        state.logBusy = false;
        $("read-log").disabled = !$("log-node").value;
        $("restart-log").disabled = !$("log-node").value;
        // r[impl kiln.web.live_progress] Automatic following ends once the run is terminal and no new bytes arrive.
        if ($("follow-logs").checked && state.logAutoFollow && emptyRead && !isRunningRun(state.run)) { $("follow-logs").checked = false; state.logAutoFollow = false; }
        if (fallback && state.session) { $("log-stream").value = "stdout"; void readLog(false); }
        else if ($("follow-logs").checked && state.session) state.logTimer = setTimeout(() => readLog(true), LOG_FOLLOW_MS);
      }
    }
  }

  async function loadArtifacts() {
    const id = state.selected;
    const generation = ++state.artifactsGeneration;
    if (!id) return;
    $("artifact-status").textContent = "Reading retained artifact metadata…";
    try {
      const result = await api(`${pathFor(id)}/artifacts`);
      if (generation !== state.artifactsGeneration || id !== state.selected) return;
      if (!Array.isArray(result.artifacts)) throw new Error("Invalid artifact metadata response.");
      $("artifact-list").replaceChildren();
      $("artifact-status").textContent = result.artifacts.length ? `${result.artifacts.length} retained artifact${result.artifacts.length === 1 ? "" : "s"}. Downloads require this operator session.` : "No artifacts have been retained for this run. Logs and execution receipts are not substituted for products.";
      for (const artifact of result.artifacts) {
        const card = element("article", undefined, "artifact-card");
        const download = element("button", "Download");
        download.type = "button";
        download.addEventListener("click", () => downloadArtifact(id, artifact, download));
        card.append(element("h3", text(artifact.name)), download, element("p", `ID: ${text(artifact.id)}\nNode: ${text(artifact.node_id)} · Attempt: ${text(artifact.attempt_id)}\nBytes: ${text(artifact.bytes)}\nBLAKE3: ${text(artifact.blake3)}`, "small artifact-meta"));
        $("artifact-list").append(card);
      }
    } catch (error) {
      if (!isAborted(error) && generation === state.artifactsGeneration) {
        $("artifact-list").replaceChildren();
        $("artifact-status").textContent = `Artifact metadata unavailable: ${explain(error)} No artifact presence or absence can be inferred.`;
      }
    }
  }
  async function downloadArtifact(id, artifact, button) {
    const epoch = state.epoch;
    button.disabled = true;
    button.textContent = "Downloading…";
    try {
      // Construct the fixed same-origin route; never follow a provider-supplied path.
      const blob = await api(`${pathFor(id)}/artifacts/${encodeURIComponent(artifact.id)}`, { kind: "blob" });
      if (epoch !== state.epoch) return;
      if (Number.isSafeInteger(artifact.bytes) && blob.size !== artifact.bytes) throw new Error("Downloaded byte count does not match retained metadata. The file was not offered for saving.");
      const url = URL.createObjectURL(blob);
      state.urls.add(url);
      const link = element("a");
      link.href = url;
      link.download = String(artifact.name || "artifact").replace(/[\\/\u0000-\u001f\u007f]/g, "_");
      link.hidden = true;
      document.body.append(link);
      link.click();
      link.remove();
      const timer = setTimeout(() => { URL.revokeObjectURL(url); state.urls.delete(url); state.urlTimers.delete(timer); }, 1000);
      state.urlTimers.add(timer);
      if (id === state.selected) $("artifact-status").textContent = `Authenticated bytes received for ${text(artifact.name)}; download handed to your browser. BLAKE3 is the service’s retained identity, not a claim of product correctness.`;
    } catch (error) {
      if (!isAborted(error) && epoch === state.epoch) {
        if (id === state.selected) $("artifact-status").textContent = `Download unavailable: ${explain(error)}`;
        notice(`Download unavailable for ${text(artifact.name)}: ${explain(error)}`);
      }
    } finally {
      if (epoch === state.epoch) { button.disabled = false; button.textContent = "Download"; }
    }
  }

  function stopFeedsTimer() {
    if (state.feedsTimer !== null) {
      clearTimeout(state.feedsTimer);
      state.feedsTimer = null;
    }
  }

  // r[impl kiln.web.interface.scenario.loss]
  function logout(message = "Signed out. Private views and pending requests were cleared.") {
    state.epoch++;
    state.session = null;
    state.token = null;
    state.workCanWrite = false;
    window.KilnWorkUI?.clear();
    window.KilnWorkInteraction?.clear();
    window.KilnCodeReview?.clear();
    $("code-review-content").replaceChildren();
    // r[impl kiln.web.live_progress] Session clearing stops the elapsed ticker and drops per-run log defaults.
    window.KilnProject?.clear();
    $("project-content").replaceChildren();
    stopFeedsTimer();
    stopLiveTicker();
    stopPanePolling();
    if ($("slide-over-inspector")) $("slide-over-inspector").hidden = true;
    if ($("slide-over-backdrop")) $("slide-over-backdrop").hidden = true;
    if ($("slide-over-body")) $("slide-over-body").replaceChildren();
    state.inspector = { open: false, kind: "", id: "", items: [], paneTimer: null, paneController: null, paneLines: 200, returnFocus: null };
    state.events = { status: "disconnected", ready: null, ci: null, fleet: null, fleetObservedAt: 0, cairn: null, attention: null };
    state.factory = { attention: null, machines: null, claims: null, dispatch: null, cairn: null, codeReview: null, showAllClaims: false };
    state.selectedRuns.clear();
    clearTimeout(state.chordTimer); state.chordPrefix = ""; state.chordTimer = null;
    if ($("global-live-strip")) $("global-live-strip").hidden = true;
    state.runSort = null; state.failureFirst = null; state.reconcileStatus = null; state.followDeclined = ""; state.startProject = "";
    $("run-progress").hidden = true;
    for (const id of ["run-supersession", "run-reconcile"]) { $(id).hidden = true; $(id).replaceChildren(); }
    for (const id of ["header-approve", "header-cancel", "header-rerun"]) $(id).hidden = true;
    for (const id of ["work-content", "planning-content", "inbox-content", "inbox-attention-content", "triage-content", "workviews-content"]) if ($(id)) $(id).replaceChildren();
    window.dispatchEvent(new Event("kiln-session-changed"));
    for (const controller of state.requests) controller.abort();
    state.requests.clear();
    clearTimeout(state.liveTimer);
    state.liveGeneration++;
    state.liveController = null;
    state.liveVersion = "0";
    state.liveRunning = null;
    state.liveFailures = 0;
    stopLogFollow();
    for (const timer of state.urlTimers) clearTimeout(timer);
    state.urlTimers.clear();
    for (const url of state.urls) URL.revokeObjectURL(url);
    state.urls.clear();
    dismissControl();
    state.projects = { available: [], enrolled: [] };
    state.runs = [];
    state.selected = null;
    state.run = null;
    state.observedAt = 0;
    state.mutation = false;
    state.detailGeneration++;
    state.artifactsGeneration++;
    resetLogs();
    for (const id of ["run-list", "run-title", "run-facts", "run-exact-facts", "retained-run-options", "run-badges", "run-warning", "run-why", "run-untrusted", "execution-identity", "node-list", "step-telemetry", "artifact-list", "artifact-status", "available-description", "project-description"]) $(id).replaceChildren();
    for (const id of ["available-project", "enrolled-project", "run-filter", "log-node"]) $(id).replaceChildren();
    state.principal = null;
    state.permissions = {};
    state.pageGeneration++;
    state.runsGeneration++;
    state.refsGeneration++;
    state.endpointGeneration.clear();
    state.logDigestCache.clear();
    state.logDigestProbed.clear();
    state.digestJump = null;
    renderFailureDigest(null);
    state.total = 0; state.nextCursor = null;
    state.runsError = "";
    state.cobsNextCursor = null;
    for (const id of ["cob-list", "cob-detail", "cob-source-status", "cob-status", "cob-repository", "cob-type"]) $(id).replaceChildren();
    $("cob-search").value = "";
    // r[impl kiln.dashboard.source.view] Source reads, caches and selections live in source.js.
    window.KilnSource?.clear();
    $("source-content").replaceChildren();
    // r[impl kiln.dashboard.reviews.view] Receipt reads and filters live in receipts.js.
    window.KilnReceipts?.clear();
    $("receipts-content").replaceChildren();
    // r[impl kiln.cairn.workspace] Cairn change reads and the search live in cairn.js.
    window.KilnCairn?.clear();
    $("cairn-content").replaceChildren();
    // r[impl kiln.dashboard.workers.view.scenario.cleared] Invalidate reads, filters and private DOM in the page module.
    window.KilnWorkers.clear();
    $("workers-content").replaceChildren();
    window.KilnRunbooks?.clear();
    $("runbooks-content").replaceChildren();
    $("source-reference").replaceChildren();
    for (const id of ["source-revision", "log-search"]) $(id).value = "";
    for (const id of ["overview-content", "project-cards", "reports-content", "compare-content", "activity-content", "run-activity-content", "evidence-content", "run-evidence-content", "automation-content", "operations-content", "principal-facts", "node-facts", "pipeline-graph", "source-status", "page-primary-action"]) $(id).replaceChildren();
    for (const id of ["workflow-title", "workflow-caption", "workflow-summary"]) $(id).replaceChildren();
    state.graphRun = null; state.graphZoom = 1;
    state.routeLoaded = false;
    $("operator-token").value = "";
    $("workspace").hidden = true;
    $("run-detail").hidden = true;
    $("empty-run").hidden = false;
    $("login-panel").hidden = false;
    $("logout").hidden = true;
    $("account-settings").hidden = true;
    setAccountMenu(false);
    $("auth-status").textContent = "Signed out";
    $("auth-status").removeAttribute("data-state");
    $("auth-status").removeAttribute("title");
    $("account-avatar").textContent = "?";
    $("account-name").textContent = "Not signed in";
    $("account-role").textContent = "Network identity is checked automatically, or connect with an access token.";
    $("connection-status").hidden = true;
    $("login-button").disabled = false;
    $("confirm-control").disabled = false;
    $("runs-status").textContent = "No private run data loaded.";
    connection("Not connected", false);
    notice(message, "success");
    $("operator-token").focus();
  }
  function readAction(action) {
    return async () => {
      const epoch = state.epoch;
      try { await action(); }
      catch (error) { if (!isAborted(error) && epoch === state.epoch) notice(explain(error)); }
    };
  }
  async function openWorkspace(epoch) {
    if (epoch !== state.epoch || !state.session) return;
    $("login-panel").hidden = true;
    $("workspace").hidden = false;
    $("logout").hidden = false;
    $("account-settings").hidden = false;
    const principalLabel = text(state.principal?.label);
    const admission = state.session === "network" ? "Trusted network identity" : "Manual bearer (memory only)";
    $("auth-status").textContent = `${principalLabel} · ${text(state.principal?.role)}`;
    $("auth-status").title = $("auth-status").textContent;
    $("auth-status").dataset.state = "connected";
    $("account-avatar").textContent = ([...principalLabel.trim()][0] || "?").toUpperCase();
    $("account-name").textContent = principalLabel;
    $("account-role").textContent = `${text(state.principal?.role)} · ${admission}`;
    $("connection-status").hidden = false;
    const principalFacts = [["Principal", state.principal?.label], ["ID", state.principal?.id], ["Role", state.principal?.role], ["Admission", admission]];
    if (state.principal?.agent) principalFacts.push(["Agent", `${text(state.principal.agent.harness)}${state.principal.agent.model ? ` · ${text(state.principal.agent.model)}` : ""} (declared)`]);
    addFacts($("principal-facts"), principalFacts);
    notice("");
    if ($("global-live-strip")) $("global-live-strip").hidden = false;
    updateGlobalLiveStrip();
    projectDescriptions();
    startLive();
    await restoreRoute();
  }
  async function discoverNetworkAccess() {
    const epoch = state.epoch;
    try {
      const result = await api("/api/session", { bootstrap: true });
      if (epoch !== state.epoch || result === null) return;
      adoptSession(result);
      state.session = "network";
      await loadProjects();
      await openWorkspace(epoch);
    } catch (error) {
      if (!isAborted(error) && epoch === state.epoch) notice(`Network access unavailable: ${explain(error)}`);
    }
  }
  // r[impl kiln.web.wayfinding] r[impl kiln.web.console_inventory_seven_destinations]
  // The admitted pages are the 7 consolidated destinations + footer + legacy deep-link aliases.
  const pages = ui.destinations.map((place) => place.page);
  const LEGACY_PAGE_ALIASES = Object.freeze({
    triage: Object.freeze({ page: "inbox", tab: "triage" }),
    work: Object.freeze({ page: "cairn", tab: "work" }),
    planning: Object.freeze({ page: "cairn", tab: "planning" }),
    workviews: Object.freeze({ page: "cairn", tab: "workviews" }),
    projects: Object.freeze({ page: "runs", tab: "projects" }),
    project: Object.freeze({ page: "runs", tab: "project" }),
    runbooks: Object.freeze({ page: "runs", tab: "runbooks" }),
    reviews: Object.freeze({ page: "runs", tab: "reviews" }),
    evidence: Object.freeze({ page: "runs", tab: "evidence" }),
    workers: Object.freeze({ page: "fleet", tab: "workers" }),
    activity: Object.freeze({ page: "source", tab: "activity" }),
    feeds: Object.freeze({ page: "source", tab: "feeds" }),
    automation: Object.freeze({ page: "source", tab: "automation" }),
    operations: Object.freeze({ page: "source", tab: "operations" }),
    cobs: Object.freeze({ page: "source", tab: "cobs" }),
  });
  const DESTINATION_TABS = Object.freeze({
    inbox: Object.freeze([
      Object.freeze({ id: "attention", label: "Escalations", panel: "inbox-attention" }),
      Object.freeze({ id: "inbox", label: "My inbox", panel: "inbox" }),
      Object.freeze({ id: "triage", label: "Requests & triage", panel: "triage" }),
    ]),
    cairn: Object.freeze([
      Object.freeze({ id: "cairn", label: "Cairn changes", panel: "cairn" }),
      Object.freeze({ id: "work", label: "Issues", panel: "work" }),
      Object.freeze({ id: "planning", label: "Planning", panel: "planning" }),
      Object.freeze({ id: "workviews", label: "Saved views", panel: "workviews" }),
    ]),
    runs: Object.freeze([
      Object.freeze({ id: "runs", label: "Retained runs", panel: "runs" }),
      Object.freeze({ id: "projects", label: "CI projects", panel: "projects" }),
      Object.freeze({ id: "runbooks", label: "Runbooks", panel: "runbooks" }),
      Object.freeze({ id: "reviews", label: "Patchgate receipts", panel: "reviews" }),
      Object.freeze({ id: "evidence", label: "Evidence", panel: "evidence" }),
    ]),
    fleet: Object.freeze([
      Object.freeze({ id: "fleet", label: "Fleet coordinator", panel: "fleet" }),
      Object.freeze({ id: "workers", label: "Worker targets", panel: "workers" }),
    ]),
    source: Object.freeze([
      Object.freeze({ id: "source", label: "Source browser", panel: "source" }),
      Object.freeze({ id: "repositories", label: "Repositories", panel: "repositories" }),
      Object.freeze({ id: "activity", label: "Activity", panel: "activity" }),
      Object.freeze({ id: "feeds", label: "Feeds", panel: "feeds" }),
      Object.freeze({ id: "automation", label: "Automation", panel: "automation" }),
      Object.freeze({ id: "operations", label: "Operations", panel: "operations" }),
      Object.freeze({ id: "cobs", label: "Collaboration history", panel: "cobs" }),
    ]),
  });
  const ALLOWED_TABS = Object.freeze({
    inbox: new Set(["attention", "inbox", "triage"]),
    cairn: new Set(["cairn", "work", "planning", "workviews"]),
    runs: new Set(["runs", "projects", "project", "runbooks", "reviews", "evidence"]),
    fleet: new Set(["fleet", "workers"]),
    source: new Set(["source", "repositories", "activity", "feeds", "automation", "operations", "cobs"]),
  });
  const INSPECT_KINDS = new Set(["attempt", "claim", "decision", "attention", "run", "cairn", "review", "repo"]);
  function parseInspectParam(raw) {
    if (typeof raw !== "string" || !raw) return "";
    const idx = raw.indexOf(":");
    if (idx <= 0) return "";
    const kind = raw.slice(0, idx);
    const id = raw.slice(idx + 1);
    if (!INSPECT_KINDS.has(kind)) return "";
    if (!id || id.length > 512 || /[\u0000-\u001f\u007f\s]/.test(id) || id.includes("..")) return "";
    return `${kind}:${id}`;
  }
  const activeSubPageFor = (route) => route?.tab || route?.page || "overview";
  const activePanelFor = (route) => {
    const sub = activeSubPageFor(route);
    return route?.page === "inbox" && sub === "attention" ? "inbox-attention" : sub;
  };
  const views = ["pipeline", "logs", "artifacts", "tests", "reviews", "compare", "activity", "evidence"];
  // r[impl kiln.web.live_activity] The running count opens the pending-run list with no other filter or selection,
  // where each row states whether it is running, queued or awaiting approval.
  const PENDING_RUNS = Object.freeze({ page: "runs", outcome: "Pending", project: "", q: "", source: "", cursor: "", run: "", node: "" });
  // r[impl kiln.web.wayfinding] The brand returns to the workspace's home, the page the bare URL opens.
  const HOME = Object.freeze({ page: "overview" });
  // Each rail destination carries its register number; the number is visual wayfinding and stays out of the link's name.
  for (const link of document.querySelectorAll("#app-sidebar a[data-page]")) {
    const place = ui.destination(link.dataset.page);
    if (!place) continue;
    const number = element("span", place.number, "nav-number");
    number.setAttribute("aria-hidden", "true");
    link.querySelector(".nav-text")?.before(number);
  }
  // r[impl kiln.web.console_layout] Wide screens show pipeline and logs together (side by side when the inspector has room);
  // the tab bar selects the remaining run view below them.
  const splitRunQuery = window.matchMedia("(min-width: 90rem)");
  const PRIMARY_RUN_VIEWS = Object.freeze(["pipeline", "logs"]);
  // r[impl kiln.web.failure_first_run] A failed run keeps its logs on screen beside or above the pipeline at every width.
  function bothPrimaryPanels() {
    const failed = Boolean(state.run) && state.run.id === state.route?.run && isFailedRun(state.run) && PRIMARY_RUN_VIEWS.includes(state.route?.view);
    return splitRunQuery.matches || failed;
  }
  function applyRunPanels() {
    const view = state.route?.view;
    const both = bothPrimaryPanels();
    for (const node of document.querySelectorAll("[data-run-panel]")) node.hidden = !(node.dataset.runPanel === view || (both && PRIMARY_RUN_VIEWS.includes(node.dataset.runPanel)));
    document.querySelector(".run-primary-panels").classList.toggle("is-split", splitRunQuery.matches);
  }
  function setRunList(open) {
    $("run-split").classList.toggle("is-list-collapsed", !open);
    $("run-list-toggle").setAttribute("aria-expanded", String(open));
    $("run-list-toggle-label").textContent = open ? "Hide run list" : "Show run list";
  }
  function canOperate() { return !!state.session && state.permissions.operate === true; }
  function adoptSession(result) {
    if (!result?.principal || result.permissions?.read !== true) throw new Error("No admitted readable principal was returned.");
    state.principal = result.principal;
    state.permissions = result.permissions;
    window.dispatchEvent(new Event("kiln-session-changed"));
  }
  // r[impl kiln.dashboard.time.local]
  // r[impl kiln.dashboard.time.local.scenario.zone]
  // r[impl kiln.dashboard.time.local.scenario.unknown]
  function timestamp(ms) {
    if (!Number.isSafeInteger(ms)) return "Not recorded";
    const date = new Date(ms);
    if (Number.isNaN(date.valueOf())) return "Not recorded";
    return `${localTimeFormat.format(date)} (local · ${localTimeZone}) · ${date.toISOString()} (UTC)`;
  }
  function duration(ms) { return ui.formatDuration(Number.isSafeInteger(ms) && ms >= 0 ? ms : null); }
  const CANONICAL_OUTCOMES = new Set(["Success", "DomainFailure", "Denied", "ProviderError", "TimedOut", "Cancelled", "Unknown", "Pending"]);
  // r[impl kiln.dashboard.source.view.scenario.restore]
  // r[impl kiln.dashboard.complete.navigation]
  function readRoute() {
    const params = new URLSearchParams(location.search);
    // Accept catalog-style fragment links as well, but never read credentials from either location.
    if (location.hash.startsWith("#page=")) for (const [key, value] of new URLSearchParams(location.hash.slice(1))) if (!params.has(key)) params.set(key, value);
    const requestedPage = pages.includes(params.get("page")) ? params.get("page") : "overview";
    const alias = Object.hasOwn(LEGACY_PAGE_ALIASES, requestedPage) ? LEGACY_PAGE_ALIASES[requestedPage] : null;
    const page = alias ? alias.page : requestedPage;
    const rawTab = params.get("tab") || "";
    const allowedTabs = ALLOWED_TABS[page];
    const defaultTab = alias ? alias.tab : (page === "inbox" ? (params.get("work_id") ? "inbox" : "attention") : allowedTabs ? page : "");
    const tab = allowedTabs ? (allowedTabs.has(rawTab) ? rawTab : defaultTab) : "";
    return {
      page,
      tab,
      inspect: parseInspectParam(params.get("inspect") || ""),
      view: views.includes(params.get("view")) ? params.get("view") : "pipeline",
      run: params.get("run") || "", node: params.get("node") || "",
      // An absent stream stays empty so failure-first defaults can apply; a URL-named stream always wins.
      stream: ["stdout", "stderr", "diagnostic"].includes(params.get("stream")) ? params.get("stream") : "",
      q: params.get("q") || "", project: params.get("project") || "", outcome: params.get("outcome") || "", source: params.get("source") || "",
      cursor: params.get("cursor") || "", activity_cursor: params.get("activity_cursor") || "", order: params.get("order") === "asc" ? "asc" : "desc", other: params.get("other") || "",
      cob_repository: params.get("cob_repository") || "", cob_type: params.get("cob_type") || "", cob_q: params.get("cob_q") || "", cob_cursor: params.get("cob_cursor") || "",
      cob: params.get("cob") || "", cob_repo: params.get("cob_repo") || "", cob_kind: params.get("cob_kind") || "",
      src: params.get("src") || "", rev: params.get("rev") || "", spath: params.get("spath") || "", sfile: params.get("sfile") || "", sq: (params.get("sq") || "").slice(0, 256),
      sview: ["history", "changes"].includes(params.get("sview")) ? params.get("sview") : "", sline: (params.get("sline") || "").slice(0, 32),
      repo_focus: params.get("repo_focus") === "import" ? "import" : "",
      receipt: (params.get("receipt") || "").slice(0, 128), receipt_q: (params.get("receipt_q") || "").slice(0, 256), receipt_decision: (params.get("receipt_decision") || "").slice(0, 32),
      worker_q: (params.get("worker_q") || "").slice(0, 256), worker_status: Object.hasOwn(window.KilnWorkers.statuses, params.get("worker_status")) ? params.get("worker_status") : "", worker: params.get("worker") || "",
      worker_campaign: params.get("worker_campaign") || "",
      runbook: (params.get("runbook") || "").slice(0, 128),
      fanout: (params.get("fanout") || "").slice(0, 128),
      dispatch: (params.get("dispatch") || "").slice(0, 128),
      rollup_cursor: (params.get("rollup_cursor") || "").slice(0, 512),
      // r[impl kiln.cairn.workspace.scenario.reload] The selected Cairn change, its open unit, register view and filters are bound to the URL.
      cairn_repository: (params.get("cairn_repository") || "").slice(0, 64), cairn_change: (params.get("cairn_change") || "").slice(0, 1024), cairn_q: (params.get("cairn_q") || "").slice(0, 256),
      cairn_unit: (params.get("cairn_unit") || "").slice(0, 2048), cairn_lane: (params.get("cairn_lane") || "").slice(0, 32),
      cairn_blocked: params.get("cairn_blocked") === "1" ? "1" : "", cairn_decisions: params.get("cairn_decisions") === "1" ? "1" : "", cairn_view: params.get("cairn_view") === "board" ? "board" : "",
      work_id: (params.get("work_id") || "").slice(0, 128), work_kind: (params.get("work_kind") || "").slice(0, 32), work_q: (params.get("work_q") || "").slice(0, 256),
      work_status: (params.get("work_status") || "").slice(0, 128), work_assignee: (params.get("work_assignee") || "").slice(0, 128), work_project: (params.get("work_project") || "").slice(0, 16_384),
      work_ci_project: (params.get("work_ci_project") || "").slice(0, 16_384), work_ci_repository: (params.get("work_ci_repository") || "").slice(0, 16_384),
      work_view: (params.get("work_view") || "").slice(0, 128), work_layout: ["board", "list"].includes(params.get("work_layout")) ? params.get("work_layout") : "", work_group: (params.get("work_group") || "").slice(0, 32), work_sort: (params.get("work_sort") || "").slice(0, 32),
      review_tab: ["changes", "queue"].includes(params.get("review_tab")) ? params.get("review_tab") : "",
      review_change: (params.get("review_change") || "").slice(0, 128), review_revision: (params.get("review_revision") || "").slice(0, 128), review_base: (params.get("review_base") || "").slice(0, 128),
      review_file: (params.get("review_file") || "").slice(0, 1024), review_side: ["base", "revision"].includes(params.get("review_side")) ? params.get("review_side") : "", review_line: (params.get("review_line") || "").slice(0, 16),
      review_whitespace: ["exact", "ignore_all", "ignore_change", "ignore_eol"].includes(params.get("review_whitespace")) ? params.get("review_whitespace") : "",
      // r[impl kiln.review.interdiff.route] Interdiff (<from>..<to> revision ids, optional jj change id), commit range and comparison mode.
      review_interdiff: /^[0-9a-f]{64}\.\.[0-9a-f]{64}$/.test(params.get("review_interdiff") || "") ? params.get("review_interdiff") : "",
      review_interdiff_change: /^[k-z]{32}$/.test(params.get("review_interdiff_change") || "") ? params.get("review_interdiff_change") : "",
      review_commits: /^[0-9a-f]{40}\.\.[0-9a-f]{40}$/.test(params.get("review_commits") || "") ? params.get("review_commits") : "",
      review_mode: params.get("review_mode") === "structural" ? "structural" : "",
      // r[impl kiln.preview.comparison] Exact baseline~candidate observation identities.
      review_preview_compare: /^[0-9a-f]{64}~[0-9a-f]{64}$/.test(params.get("review_preview_compare") || "") ? params.get("review_preview_compare") : "",
      // r[impl kiln.preview.structural.scenario.off] Opt-in structural evidence for that pair.
      review_preview_structural: params.get("review_preview_structural") === "1" ? "1" : "",
      review_q: (params.get("review_q") || "").slice(0, 256),
      feed_q: (params.get("feed_q") || "").slice(0, 256), review_view: ["all", "awaiting_review", "owned_needs_revision", "unresolved"].includes(params.get("review_view")) ? params.get("review_view") : "", review_status: ["open", "abandoned", "submitted"].includes(params.get("review_status")) ? params.get("review_status") : "",
      review_advisory: ["with", "without"].includes(params.get("review_advisory")) ? params.get("review_advisory") : "",
      review_agent: ["with", "without"].includes(params.get("review_agent")) ? params.get("review_agent") : "",
      review_agent_harness: ui.agentText(params.get("review_agent_harness")),
      review_agent_model: ui.agentText(params.get("review_agent_model")),
      review_offset: /^[0-9]{1,8}$/.test(params.get("review_offset") || "") ? params.get("review_offset") : "",
      review_notification_offset: /^[0-9]{1,8}$/.test(params.get("review_notification_offset") || "") ? params.get("review_notification_offset") : "",
      review_operation_offset: /^[0-9]{1,8}$/.test(params.get("review_operation_offset") || "") ? params.get("review_operation_offset") : "",
      theme: ["system", "light", "dark"].includes(params.get("theme")) ? params.get("theme") : "system", density: params.get("density") === "compact" ? "compact" : "comfortable"
    };
  }
  function sameRouteExceptInspect(a, b) {
    if (!a || !b) return false;
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const key of keys) {
      if (key === "inspect") continue;
      if (a[key] !== b[key]) return false;
    }
    return true;
  }
  function navigate(patch) {
    const normalized = { ...patch };
    if (Object.hasOwn(normalized, "page") && Object.hasOwn(LEGACY_PAGE_ALIASES, normalized.page)) {
      const alias = LEGACY_PAGE_ALIASES[normalized.page];
      normalized.page = alias.page;
      if (!Object.hasOwn(normalized, "tab")) normalized.tab = alias.tab;
    } else if (Object.hasOwn(normalized, "page") && normalized.page !== state.route?.page && !Object.hasOwn(normalized, "tab")) {
      normalized.tab = "";
    }
    const route = { ...state.route, ...normalized };
    if (Object.hasOwn(normalized, "run") && normalized.run !== state.route?.run && !Object.hasOwn(normalized, "activity_cursor")) route.activity_cursor = "";
    if ((Object.hasOwn(normalized, "fanout") && normalized.fanout !== state.route?.fanout || Object.hasOwn(normalized, "dispatch") && normalized.dispatch !== state.route?.dispatch) && !Object.hasOwn(normalized, "rollup_cursor")) route.rollup_cursor = "";
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(route)) if (value) params.set(key, value);
    history.pushState(null, "", `${location.pathname}?${params}`);
    void readAction(restoreRoute)();
  }
  function renderDestinationTabs(route, routeHref) {
    const bar = $("destination-tabs");
    if (!bar) return;
    const tabs = DESTINATION_TABS[route.page];
    if (!tabs || !tabs.length) { bar.hidden = true; bar.replaceChildren(); return; }
    const activeTab = activeSubPageFor(route) === "project" ? "projects" : activeSubPageFor(route);
    bar.hidden = false;
    bar.replaceChildren();
    for (const tab of tabs) {
      const link = element("a", tab.label);
      link.dataset.tab = tab.id;
      link.href = routeHref({ page: route.page, tab: tab.id });
      if (tab.id === activeTab) link.setAttribute("aria-current", "page");
      link.addEventListener("click", (event) => {
        if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
          event.preventDefault();
          navigate({ page: route.page, tab: tab.id });
        }
      });
      bar.append(link);
    }
  }
  async function restoreRoute() {
    const previous = state.route;
    state.route = readRoute();
    const route = state.route;
    const prevSub = previous ? activeSubPageFor(previous) : "";
    const nextSub = activeSubPageFor(route);
    const routeHref = (patch) => `?${new URLSearchParams(Object.entries({ ...route, ...patch }).filter(([, value]) => value))}`;
    if (previous && state.session && state.routeLoaded && previous.inspect !== route.inspect && sameRouteExceptInspect(previous, route)) {
      for (const node of document.querySelectorAll("a[data-page]")) node.href = routeHref({ page: node.dataset.page, tab: "" });
      for (const node of document.querySelectorAll("a[data-view]")) node.href = routeHref({ view: node.dataset.view });
      renderDestinationTabs(route, routeHref);
      if (state.session) syncInspectorFromRoute(route);
      return;
    }
    if (prevSub === "code-review" && nextSub !== "code-review") window.KilnCodeReview?.clear();
    if (prevSub === "project" && nextSub !== "project") window.KilnProject?.clear();
    if (prevSub === "source" && nextSub !== "source") window.KilnSource?.clear();
    if (prevSub === "repositories" && nextSub !== "repositories") window.KilnSource?.clearRepositories?.();
    if (prevSub === "reviews" && nextSub !== "reviews") window.KilnReceipts?.clear();
    if (prevSub === "cairn" && nextSub !== "cairn") window.KilnCairn?.clear();
    if (prevSub === "fleet" && nextSub !== "fleet") window.KilnFleet?.clear();
    if (prevSub === "runbooks" && nextSub !== "runbooks") window.KilnRunbooks?.clear();
    // r[impl kiln.web.live_progress] Leaving the run (or the runs page) stops its elapsed ticker.
    if (previous && (nextSub !== "runs" || previous.run !== route.run)) stopLiveTicker();
    document.documentElement.dataset.theme = route.theme;
    document.documentElement.dataset.density = route.density;
    $("app-theme").value = route.theme;
    for (const node of $("theme-switch").children) node.setAttribute("aria-pressed", String(node.dataset.themeChoice === route.theme));
    $("app-density").value = route.density;
    const activePanel = activePanelFor(route);
    for (const node of document.querySelectorAll("[data-panel]")) node.hidden = node.dataset.panel !== activePanel;
    const navPage = ui.destination(route.page).parent || route.page;
    for (const node of document.querySelectorAll("a[data-page]")) node.href = routeHref({ page: node.dataset.page, tab: "" });
    for (const node of document.querySelectorAll("a[data-view]")) node.href = routeHref({ view: node.dataset.view });
    $("live-activity").href = routeHref(PENDING_RUNS);
    $("brand-home").href = routeHref(HOME);
    if ($("global-firing-pill")) $("global-firing-pill").href = routeHref(PENDING_RUNS);
    if ($("global-escalations-pill")) $("global-escalations-pill").href = routeHref({ page: "inbox", tab: "attention" });
    if ($("global-expiring-claims-pill")) $("global-expiring-claims-pill").href = routeHref({ page: "fleet", tab: "fleet" });
    renderDestinationTabs(route, routeHref);
    for (const node of document.querySelectorAll("[data-page]")) { if (node.dataset.page === navPage) node.setAttribute("aria-current", "page"); else node.removeAttribute("aria-current"); }
    for (const group of document.querySelectorAll(".app-navigation details")) {
      if (group.querySelector('[aria-current="page"]')) group.open = true;
    }
    // r[impl kiln.web.wayfinding] When the rail is taller than the window it scrolls; the current destination is
    // brought fully into the rail's own view. Only the rail scrolls: the page keeps its position.
    const rail = document.querySelector(".app-navigation");
    const current = rail?.querySelector('[aria-current="page"]');
    if (rail && current) {
      const frame = rail.getBoundingClientRect();
      const box = current.getBoundingClientRect();
      if (box.top < frame.top) rail.scrollTop -= frame.top - box.top;
      else if (box.bottom > frame.bottom) rail.scrollTop += box.bottom - frame.bottom;
    }
    $("workspace").classList.toggle("page-frame-wide", ["overview", "runs", "code-review"].includes(route.page));
    $("run-split").classList.toggle("has-run", Boolean(route.run));
    applyRunPanels();
    for (const node of document.querySelectorAll("[data-view]")) { if (node.dataset.view === route.view) node.setAttribute("aria-current", "page"); else node.removeAttribute("aria-current"); }
    // r[impl kiln.web.wayfinding] The title block reads the destination register: section number and group, title, purpose.
    const place = ui.destination(route.page);
    const subPlace = nextSub !== route.page ? ui.destination(nextSub) : null;
    const subMeta = !subPlace && nextSub === "repositories" ? { title: "Repositories", purpose: "Multi-forge repository catalog, canonical Radicle/GitHub/Gitea remotes, mirrors, and cross-fleet local working copies." } : null;
    $("workspace-title").textContent = place.title;
    $("app-breadcrumb").textContent = (subPlace || subMeta) ? `${place.number} ${place.title} / ${(subPlace || subMeta).title}` : `${place.number} ${place.title}`;
    document.title = `Kiln · ${place.title}`;
    $("workspace-eyebrow").replaceChildren(element("span", place.number, "page-number"), document.createTextNode(ui.destinationGroup(place.group).name));
    $("observation-note").textContent = (subPlace || subMeta) ? (subPlace || subMeta).purpose : place.purpose;
    if (previous?.page !== route.page || prevSub !== nextSub) {
      notice("");
      $("page-primary-action").replaceChildren();
      $("workspace-title").tabIndex = -1;
      $("workspace-title").focus({ preventScroll: true });
      window.scrollTo(0, 0);
    }
    for (const [id, key] of [["run-search", "q"], ["run-filter", "project"], ["run-outcome", "outcome"], ["run-source", "source"], ["run-order", "order"], ["compare-other", "other"], ["evidence-run", "run"]]) $(id).value = route[key];
    $("log-stream").value = effectiveStream();
    if (route.source || route.order !== "desc") $("run-more-filters").open = true;
    for (const [id, value] of [["run-filter", route.project], ["run-outcome", route.outcome]]) {
      const select = $(id);
      if (value && ![...select.options].some((option) => option.value === value)) { const option = element("option", `Unavailable selection: ${value}`); option.value = value; select.append(option); }
      select.value = value;
    }
    setMenu(false);
    if (!state.session) return;
    state.routeLoaded = true;
    if (previous && (prevSub !== nextSub || previous.run !== route.run || previous.node !== route.node || (previous.view !== route.view && !bothPrimaryPanels()) || previous.stream !== route.stream)) resetLogs();
    state.pageGeneration++;
    if (nextSub === "runs") {
      if (state.selected !== route.run) {
        state.selected = route.run || null; state.run = null; state.observedAt = 0;
        state.detailGeneration++; state.artifactsGeneration++;
        $("run-detail").hidden = true; $("empty-run").hidden = false;
        $("empty-run-title").textContent = route.run ? "Loading selected run" : "Select a run";
        $("empty-run-message").textContent = route.run ? `Reading exact retained identity: ${route.run}` : "Choose a retained run. No run is selected automatically.";
      }
      try { await loadRuns(); } catch (error) { if (isAborted(error)) return; notice(`Run browser unavailable: ${explain(error)} The selected run is fetched independently; no page was substituted.`); }
      if (state.route !== route) return;
      if (route.run && (state.selected !== route.run || !state.run)) await selectRun(route.run);
      else if (route.run) { renderDetail(); await loadRunView(); }
      else { state.selected = null; state.run = null; $("run-detail").hidden = true; $("empty-run").hidden = false; $("empty-run-title").textContent = "Select a run"; $("empty-run-message").textContent = "Choose a retained run. No run is selected automatically."; }
    } else await loadPage();
    if (state.route !== route || !state.session) return;
    syncInspectorFromRoute(route);
    if (nextSub === "runs" && route.run && previous?.run !== route.run && state.run) {
      $("run-title").tabIndex = -1;
      $("run-title").focus({ preventScroll: true });
      // A failed run already revealed its first error line; keep that view instead of jumping back to the title.
      if (!(state.failureFirst?.run === route.run && state.failureFirst.revealed)) $("run-title").scrollIntoView({ block: "start" });
    } else if (nextSub === "runs" && route.run && previous?.view !== route.view) {
      const heading = document.querySelector(`[data-run-panel="${route.view}"] h2`);
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
      heading.scrollIntoView({ block: "start" });
    }
  }
  // r[impl kiln.web.page_consistency] Run links are row actions: .row-link keeps them 44px tall on touch layouts.
  function runLink(id, label = id, view = "pipeline") {
    const link = element("a", text(label), "row-link");
    const params = new URLSearchParams({ page: "runs", run: id, view });
    link.href = `?${params}`;
    link.addEventListener("click", (event) => { if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); navigate({ page: "runs", run: id, view, node: "", stream: "" }); } });
    return link;
  }
  function section(title, parent, className = "panel") { const node = element("section", undefined, className); node.append(element("h3", title)); parent.append(node); return node; }
  // r[impl kiln.web.page_consistency] Boundary and non-claim copy stays reachable behind the shared "About" disclosure.
  function aboutView(label, ...paragraphs) {
    const details = element("details", undefined, "about-view");
    const body = element("div", undefined, "about-view-body");
    for (const paragraph of paragraphs) body.append(element("p", paragraph));
    details.append(ui.disclosureSummary(label, "info"), body);
    return details;
  }
  // r[impl kiln.web.wayfinding] r[impl kiln.web.wayfinding.scenario.orientation] The Guide is the workspace's index: a first path for a new operator, every destination by
  // section number with its purpose, how run states read, Kiln's vocabulary and the keyboard reference. It reads only
  // the destination register, the status rule and the review shortcut table, so it cannot describe a page or a key that
  // the console does not have.
  const GUIDE_STATES = Object.freeze([
    ["Pending", "active", "Dispatched to an execution provider and not finished yet."],
    ["Pending", "queued", "Admitted and waiting for a free project slot. Nothing has executed."],
    ["Pending", "awaiting_approval", "An untrusted pull-request run. Nothing runs until an operator approves its exact commit."],
    ["Success", "", "The workflow completed and every node succeeded for this exact revision."],
    ["DomainFailure", "", "The workflow ran and at least one node failed. Logs open on the first failed node."],
    ["Denied", "", "Refused by policy or authority; the requested work did not run as asked."],
    ["TimedOut", "", "The work exceeded its time bound and was stopped."],
    ["ProviderError", "", "The execution provider reported an error and the work did not complete."],
    ["Cancelled", "", "Stopped on request before it finished."],
    ["Unknown", "", "Kiln cannot confirm what happened, for example after a lost connection. Unknown is neither a pass nor a failure: observe or reconcile before acting."],
  ]);
  const GUIDE_TERMS = Object.freeze([
    ["Project", "A repository reference with a reviewed pipeline that this service may run."],
    ["Run", "One execution of a project's pipeline at one exact source revision. A rerun is a new run."],
    ["Exact revision", "The full commit identity a run is bound to. A branch that moves needs a new run; results never carry over."],
    ["Node", "One step of a run's workflow graph, such as build or test. Each node has its own logs."],
    ["Worker target", "A configured execution target with project admission slots. It is not a discovered machine or a health check."],
    ["Retained", "Kept by this service within its bounds. Views show retained records, not a complete history."],
    ["Evidence", "Recorded source, execution, report, review and provenance facts for a run. Evidence is not acceptance or release authority."],
    ["Receipt", "Patchgate's recorded review of inbox changes: one decision per change and Patchgate's own report, kept byte for byte. A receipt never accepts or merges a change."],
    ["Viewer and operator", "Viewers read. Operators can also enroll projects and start, cancel, approve and rerun runs; the service checks every request again."],
  ]);
  // One Guide sheet: a framed panel with its section title.
  function guideSheet(parent, title) {
    const sheet = element("section", undefined, "panel guide-sheet");
    const heading = element("h2", title, "card-title");
    heading.id = `guide-${title.toLowerCase().replace(/[^a-z]+/g, "-")}`;
    sheet.setAttribute("aria-labelledby", heading.id);
    sheet.append(heading);
    parent.append(sheet);
    return sheet;
  }
  // A numbered destination line: section number, link, purpose.
  function guideEntry(page, text) {
    const place = ui.destination(page);
    const item = element("li");
    item.append(element("span", place.number, "page-number"), workspaceLink({ page }, place.title), element("p", text));
    return item;
  }
  function renderGuide(parent) {
    parent.replaceChildren();
    const start = guideSheet(parent, "Start here");
    const steps = element("ol", undefined, "contents-list guide-steps");
    for (const [page, text] of [
      ["overview", "Failing projects come first. Each card opens the exact failed run at its logs."],
      ["runs", "Every retained run, newest first. Select a run to see its pipeline, logs, artifacts, tests and evidence."],
      ["projects", "Start a run: choose a project, then its latest reference, a named reference or an exact revision."],
      ["code-review", "Review a change at its exact revision; its submit requirements show the CI results that apply to it."],
    ]) steps.append(guideEntry(page, text));
    start.append(steps);
    const contents = element("div", undefined, "contents-groups");
    contents.setAttribute("role", "group");
    contents.setAttribute("aria-label", "Contents");
    for (const group of ui.destinationGroups) {
      const places = ui.destinations.filter((place) => place.group === group.number && !place.parent && place.page !== "guide");
      if (!places.length) continue;
      const box = element("section", undefined, "contents-group panel");
      const heading = element("h2");
      heading.append(element("span", `${group.number}.0`, "page-number"), document.createTextNode(group.name));
      const list = element("ul", undefined, "contents-list");
      for (const place of places) list.append(guideEntry(place.page, place.purpose));
      box.append(heading, list);
      contents.append(box);
    }
    parent.append(contents);
    const states = guideSheet(parent, "How run states read");
    states.append(element("p", "A state is a recorded observation of one run. Colour, icon and words carry the same state; the exact canonical outcome stays available on hover and to assistive technology.", "small"));
    const legend = element("dl", undefined, "guide-legend");
    for (const [outcome, phase, meaning] of GUIDE_STATES) {
      const term = element("dt");
      term.append(ui.statusPill(outcome, phase || undefined));
      legend.append(term, element("dd", meaning));
    }
    states.append(legend);
    const terms = guideSheet(parent, "Words Kiln uses");
    const vocabulary = element("dl", undefined, "guide-terms");
    for (const [term, meaning] of GUIDE_TERMS) vocabulary.append(element("dt", term), element("dd", meaning));
    terms.append(vocabulary);
    const keys = guideSheet(parent, "Keyboard");
    const reference = element("dl", undefined, "contents-keys");
    const keyRow = (list, combo, meaning) => {
      const term = element("dt");
      for (const key of combo) term.append(element("kbd", key));
      list.append(term, element("dd", meaning));
    };
    keyRow(reference, ["Ctrl", "K"], "Search and commands: jump to any destination, Fleet claim, attempt, Cairn change, campaign, worker, review or run, or run > verbs (⌘ K on macOS).");
    keyRow(reference, ["g", "f / a / d / r / c / m"], "Navigation chords: Factory (f), Attention (a), Delivery & Cairn (d), Runs (r), Code Review (c), Fleet & Machines (m).");
    keyRow(reference, ["[", "]"], "Slide-Over Inspector: step to the previous or next item.");
    keyRow(reference, ["Esc"], "Close the Slide-Over Inspector, menu, dialog, command palette or expanded navigation.");
    keyRow(reference, ["↑", "↓", "Enter", "x"], "Run list: move between rows, open the focused run, or select rows for bulk controls.");
    keyRow(reference, ["a", "r", "p", "Shift+X"], "Attention triage: approve/accept (a), renew/reconcile (r), peek/prompt (p), cancel/reject (Shift+X).");
    keyRow(reference, ["j", "k", "Enter", "Space", "x"], "Work lists: move, open, peek and select rows; Shift F10 opens a row's actions.");
    keys.append(reference);
    // The review table is the one the review page binds; it is listed only when that module has loaded.
    const review = window.KilnCodeReview?.keys?.commands || [];
    if (!review.length) return;
    const reviewKeys = element("dl", undefined, "contents-keys");
    for (const command of review) keyRow(reviewKeys, String(command.keys).split(" / ").map((key) => ({ ArrowDown: "↓", ArrowUp: "↑" }[key] || key)), command.description);
    keys.append(element("p", "Code review, while a change is open", "eyebrow"), reviewKeys);
  }
  // r[impl kiln.monorepo.console] A delivery's affected decision: which projects ran and why, and which were skipped.
  function affectedDecisionList(decisions) {
    const box = element("div", undefined, "affected-decisions");
    if (!decisions.rows.length) box.append(element("p", "No project decisions were recorded.", "small"));
    const list = element("ul", undefined, "affected-decision-list");
    for (const row of decisions.rows) {
      const item = element("li");
      item.dataset.outcome = row.affected ? "affected" : "skipped";
      const head = element("p", undefined, "affected-decision-head");
      head.append(element("strong", text(projectLabel(row.projectId))), element("span", row.affected ? "Affected" : "Skipped", "chip"));
      item.append(head);
      if (row.affected) item.append(ui.reasonList(row.reasons));
      else item.append(element("p", row.detail, "small"));
      list.append(item);
    }
    if (decisions.rows.length) box.append(list);
    if (decisions.identity) { const identity = element("p", `Affected-set BLAKE3 ${shortIdentity(decisions.identity)}`, "small mono"); identity.title = decisions.identity; box.append(identity); }
    return box;
  }
  function renderRecorded(parent, value, depth = 0) {
    if (value === null || value === undefined) { parent.append(element("p", "Not recorded", "small")); return; }
    if (Array.isArray(value)) {
      if (!value.length) parent.append(element("p", "No retained records in this bounded view.", "small"));
      for (const row of value) { const card = element("article", undefined, "record-card"); renderRecorded(card, row, depth + 1); parent.append(card); }
      return;
    }
    if (typeof value !== "object") { parent.append(element("span", text(value))); return; }
    const facts = element("dl", undefined, "facts");
    parent.append(facts);
    for (const [key, item] of Object.entries(value)) {
      const label = element("dt", key.replaceAll("_", " "));
      const cell = element("dd");
      facts.append(label, cell);
      const decisions = key === "affected" ? ui.affectedDecisions(item) : null;
      if (decisions) cell.append(affectedDecisionList(decisions));
      else if (item && typeof item === "object") {
        const details = element("details"); details.append(element("summary", `${key.replaceAll("_", " ")} · recorded facts`)); cell.append(details);
        if (depth < 7) renderRecorded(details, item, depth + 1); else details.append(element("pre", text(item), "identity"));
      } else if ((key === "run_id" || key === "parent_run_id") && typeof item === "string") cell.append(runLink(item));
      else if (key.endsWith("_at_ms") || key === "next_due_ms" || key === "reserved_at_ms") cell.append(when(item));
      else if (key === "duration_ms") cell.append(elapsed(item));
      else if ((key === "outcome" || key.endsWith("_outcome")) && CANONICAL_OUTCOMES.has(item)) cell.append(ui.statusPill(item, value.phase));
      else cell.textContent = text(item);
    }
  }
  async function renderEndpoint(target, id, render = renderRecorded) {
    const generation = state.pageGeneration;
    const requestGeneration = (state.endpointGeneration.get(id) || 0) + 1;
    state.endpointGeneration.set(id, requestGeneration);
    const run = state.route.run;
    const parent = $(id);
    parent.replaceChildren(loadingState());
    try {
      const result = await api(target);
      if (generation !== state.pageGeneration || requestGeneration !== state.endpointGeneration.get(id) || run !== state.route.run) return;
      parent.replaceChildren(); render(parent, result); return true;
    } catch (error) {
      if (!isAborted(error) && generation === state.pageGeneration && requestGeneration === state.endpointGeneration.get(id)) parent.replaceChildren(errorState(`Unavailable: ${explain(error)} No presence, absence or success is inferred.`));
    }
  }
  function activityQuery() { const query = new URLSearchParams({ limit: "50" }); if (state.route.activity_cursor) query.set("cursor", state.route.activity_cursor); return query; }
  // r[impl kiln.dashboard.usability]
  async function loadRunChoices() {
    const route = state.route;
    const list = $("retained-run-options");
    list.replaceChildren();
    try {
      const query = new URLSearchParams({ limit: "100", order: "desc" });
      if (route.page === "runs" && state.run?.project_id) query.set("project_id", state.run.project_id);
      const result = await api(`/api/runs?${query}`);
      if (state.route !== route) return;
      for (const run of result.runs || []) {
        if (route.page === "runs" && (run.id === route.run || run.project_id !== state.run?.project_id)) continue;
        const option = element("option");
        option.value = run.id;
        option.label = `${projectLabel(run.project_id)} · ${ui.outcomeStatus(run.outcome, run.phase).label} · ${relativeText(run.created_at_ms)} · ${shortIdentity(run.source_revision)}`;
        list.append(option);
      }
    } catch (error) {
      if (!isAborted(error) && state.route === route) notice(`Run suggestions unavailable: ${explain(error)} You can still enter an exact retained run ID.`);
    }
  }
  function feedDate(value) {
    if (!value) return "Undated";
    return text(value);
  }
  // r[impl kiln.web.feeds.ui]
  // r[impl kiln.web.feeds.config.scenario.console]
  async function loadFeeds() {
    stopFeedsTimer();
    const query = state.route?.feed_q || "";
    const result = await api(`/api/feeds${query ? `?q=${encodeURIComponent(query)}` : ""}`);
    const root = $("feeds-content");
    root.replaceChildren();
    if (!result?.configured) {
      root.append(emptyState("Feeds are not configured", "Add a feeds policy with a dedicated token file to the service profile, then restart the service."));
      return;
    }
    const search = element("form", undefined, "inline-form toolbar");
    const searchLabel = element("label", "Filter feeds");
    searchLabel.htmlFor = "feed-search";
    const searchInput = element("input");
    searchInput.id = "feed-search"; searchInput.type = "search"; searchInput.maxLength = 256;
    searchInput.placeholder = "Label, URL, entry title or summary…";
    searchInput.value = query;
    const searchButton = element("button", "Filter");
    searchButton.type = "submit";
    search.append(searchLabel, searchInput, searchButton);
    search.addEventListener("submit", (event) => {
      event.preventDefault();
      navigate({ page: "feeds", feed_q: searchInput.value.trim() });
    });
    root.append(search);
    const summary = result.summary;
    if (summary) {
      root.append(element(
        "p",
        `${summary.total} subscription${summary.total === 1 ? "" : "s"} · ${summary.pending} pending · ${summary.failing} failing · ${summary.paused} paused`,
        "small"
      ));
    }
    const publication = result.publication?.repositories || [];
    if (publication.length > 0) {
      const section = element("section", undefined, "panel");
      section.append(element("h2", "Repository subscriptions"));
      const cadence = result.bounds?.refresh_interval_ms;
      const note = element("p", `Copy an Atom URL into any feed reader. The URL carries a dedicated read-only feed token, so treat it as a secret. ${cadence ? `External subscriptions refresh every ${Math.max(1, Math.round(cadence / 60000))} minutes.` : "External subscriptions refresh only when you press Refresh."} This page reloads its observations every 60 seconds while open.`, "small");
      section.append(note);
      section.append(element("p", `${publication.length} repositories`, "small"));
      const list = element("div", undefined, "feed-grid");
      for (const repository of publication) {
        const card = element("article", undefined, "feed-card panel");
        card.append(element("h3", text(repository.label)), element("p", `${repository.runs} retained runs · ${repository.key.slice(0, 12)}…`, "small"));
        const actions = element("div", undefined, "cluster");
        const subscribe = repository.subscribe_url;
        if (subscribe) {
          const copy = element("button", undefined, "quiet");
          copy.type = "button";
          copy.textContent = "Copy Atom URL";
          copy.addEventListener("click", async () => {
            try { await navigator.clipboard.writeText(subscribe); notice("Atom URL copied.", "success"); }
            catch { notice("The browser did not allow clipboard access. Open the URL in a new tab and copy it from the address bar."); }
          });
          actions.append(copy);
          const open = element("a", "Open feed", "button-link");
          open.href = subscribe;
          open.target = "_blank";
          open.rel = "noreferrer";
          actions.append(open);
          if (repository.rss_subscribe_url) {
            const rss = element("a", "RSS", "button-link");
            rss.href = repository.rss_subscribe_url;
            rss.target = "_blank";
            rss.rel = "noreferrer";
            actions.append(rss);
          }
        } else {
          actions.append(element("span", "The token-bearing URL is available to operators.", "small"));
        }
        card.append(actions);
        list.append(card);
      }
      section.append(list);
      const opml = result.publication?.opml_subscribe_url;
      if (opml) {
        const exportLink = element("a", "Export OPML for all repositories", "button-link");
        exportLink.href = opml;
        exportLink.target = "_blank";
        exportLink.rel = "noreferrer";
        section.append(exportLink);
      }
      root.append(section);
    }
    if (canOperate()) {
      const form = element("form", undefined, "panel");
      form.append(element("h2", "Subscribe to an external repository feed"));
      const urlLabel = element("label", "Feed URL");
      urlLabel.htmlFor = "feed-url";
      const url = element("input");
      url.id = "feed-url"; url.type = "url"; url.required = true; url.maxLength = 2048; url.placeholder = "https://example.com/repository/releases.atom";
      const labelLabel = element("label", "Label");
      labelLabel.htmlFor = "feed-label";
      const label = element("input");
      label.id = "feed-label"; label.type = "text"; label.maxLength = 128; label.placeholder = "Optional display label";
      const submit = element("button", "Subscribe");
      submit.type = "submit";
      form.append(urlLabel, url, labelLabel, label, submit,
        element("p", "Only HTTPS destinations are fetched by public deployments. Redirects, response size, entries and total subscriptions are bounded.", "small"));
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        submit.disabled = true;
        try {
          await api("/api/feeds", { method: "POST", body: { url: url.value.trim(), label: label.value.trim() || undefined } });
          notice("Feed subscription added.", "success");
          await loadFeeds();
        } catch (error) {
          notice(explain(error));
        } finally { submit.disabled = false; }
      });
      root.append(form);
      const importForm = element("form", undefined, "panel");
      importForm.append(element("h2", "Import OPML"));
      const fileLabel = element("label", "OPML file");
      fileLabel.htmlFor = "feed-opml";
      const file = element("input");
      file.id = "feed-opml"; file.type = "file"; file.accept = ".opml,.xml,text/xml,application/xml,text/x-opml"; file.required = true;
      const importButton = element("button", "Import subscriptions");
      importButton.type = "submit";
      importForm.append(fileLabel, file, importButton,
        element("p", "Imported feeds are retained without blocking this request; background refresh fetches them at the configured cadence, or use Refresh per feed.", "small"));
      const exportButton = element("button", "Export subscriptions", "quiet");
      exportButton.type = "button";
      exportButton.addEventListener("click", async () => {
        exportButton.disabled = true;
        try {
          const blob = await api("/api/feeds/export", { kind: "blob" });
          saveBlob(blob, "kiln-external-feeds.opml");
          notice("Subscription OPML export prepared.", "success");
        } catch (error) {
          notice(explain(error));
        } finally { exportButton.disabled = false; }
      });
      importForm.append(exportButton);
      importForm.addEventListener("submit", async (event) => {
        event.preventDefault();
        const selected = file.files?.[0];
        if (!selected) return;
        importButton.disabled = true;
        try {
          const maxImport = result.bounds?.max_import_bytes || 1_048_576;
          if (selected.size > maxImport) throw new Error(`The OPML file exceeds the ${maxImport}-byte request bound.`);
          const opml = await selected.text();
          const result = await api("/api/feeds/import", { method: "POST", body: { opml } });
          notice(`Imported ${result.added} subscription${result.added === 1 ? "" : "s"}; skipped ${result.skipped}.`, "success");
          await loadFeeds();
        } catch (error) {
          notice(explain(error));
        } finally { importButton.disabled = false; }
      });
      root.append(importForm);
    }
    const scheduleRefresh = () => {
      if (!result.configured) return;
      state.feedsTimer = setTimeout(() => {
        state.feedsTimer = null;
        if (state.session && state.route?.page === "feeds") void readAction(loadFeeds)();
      }, 60_000);
    };
    const subscriptions = result.subscriptions || [];
    if (subscriptions.length === 0) {
      root.append(emptyState(
        query ? "No matching feeds" : "No external feeds",
        query ? "No retained subscription or entry matches this filter." : "Subscriptions added here are fetched on add and on explicit refresh."
      ));
      scheduleRefresh();
      return;
    }
    for (const subscription of subscriptions) {
      const card = element("section", undefined, "panel");
      const heading = element("div", undefined, "section-heading");
      heading.append(element("h2", text(subscription.label || subscription.title || "External feed")));
      heading.append(element(
        "span",
        subscription.enabled === false ? "Paused" : subscription.error ? "Needs attention" : "Observed",
        subscription.enabled === false ? "pill" : subscription.error ? "pill pill-failed" : "pill"
      ));
      card.append(heading);
      const lastSuccess = subscription.last_success_ms ? ` · Last success ${timestamp(subscription.last_success_ms)}` : "";
      card.append(element("p", `${subscription.url || "URL hidden for viewers"} · ${subscription.fetched_at_ms ? `Last attempt ${timestamp(subscription.fetched_at_ms)}` : "Not fetched"}${lastSuccess}`, "small mono"));
      if (subscription.error) card.append(element("p", text(subscription.error), "warning"));
      if (canOperate()) {
        const actions = element("div", undefined, "cluster");
        const refresh = element("button", "Refresh", "quiet");
        refresh.type = "button";
        refresh.addEventListener("click", async () => {
          refresh.disabled = true;
          try { await api(`/api/feeds/${encodeURIComponent(subscription.id)}/refresh`, { method: "POST", body: {} }); await loadFeeds(); }
          catch (error) { notice(explain(error)); await loadFeeds(); }
          finally { refresh.disabled = false; }
        });
        const toggle = element("button", subscription.enabled === false ? "Resume" : "Pause", "quiet");
        toggle.type = "button";
        toggle.addEventListener("click", async () => {
          toggle.disabled = true;
          try {
            await api(`/api/feeds/${encodeURIComponent(subscription.id)}/enabled`, { method: "POST", body: { enabled: subscription.enabled === false } });
            await loadFeeds();
          } catch (error) {
            notice(explain(error));
          } finally { toggle.disabled = false; }
        });
        const rename = element("button", "Rename", "quiet");
        rename.type = "button";
        rename.addEventListener("click", async () => {
          const next = window.prompt("Feed label", subscription.label);
          if (next === null) return;
          rename.disabled = true;
          try {
            await api(`/api/feeds/${encodeURIComponent(subscription.id)}/label`, { method: "POST", body: { label: next.trim() } });
            notice("Feed label updated.", "success");
            await loadFeeds();
          } catch (error) {
            notice(explain(error));
          } finally { rename.disabled = false; }
        });
        const remove = element("button", "Remove", "quiet danger");
        remove.type = "button";
        remove.addEventListener("click", async () => {
          remove.disabled = true;
          try { await api(`/api/feeds/${encodeURIComponent(subscription.id)}/remove`, { method: "POST", body: {} }); await loadFeeds(); }
          catch (error) { notice(explain(error)); }
          finally { remove.disabled = false; }
        });
        actions.append(refresh, toggle, rename, remove);
        card.append(actions);
      }
      const entries = element("ul", undefined, "activity-feed");
      for (const entry of subscription.entries || []) {
        const item = element("li");
        const title = entry.link ? element("a", text(entry.title), "activity-feed-title") : element("span", text(entry.title), "activity-feed-title");
        if (entry.link) { title.href = entry.link; title.target = "_blank"; title.rel = "noreferrer"; }
        item.append(title);
        if (entry.published) item.append(element("span", feedDate(entry.published), "small"));
        if (entry.summary) item.append(element("p", text(entry.summary), "small"));
        entries.append(item);
      }
      if (entries.childElementCount === 0) card.append(element("p", "No entries were retained from this feed.", "small"));
      else card.append(entries);
      root.append(card);
    }
    scheduleRefresh();
  }
  async function loadPage() {
    const route = state.route;
    const subPage = activeSubPageFor(route);
    if (subPage !== "feeds") stopFeedsTimer();
    if (subPage === "code-review") {
      await window.KilnCodeReview.load($("code-review-content"), {
        api,
        navigate: (patch) => navigate({ ...patch, page: "code-review" }),
        toast: (message) => notice(message, "info"),
        principal: state.principal?.id || "",
        role: state.principal?.role || "",
        route: new URLSearchParams(Object.entries(route)),
      });
      return;
    }
    if (route.page === "inbox" && subPage === "attention") {
      await loadAttentionPage();
      return;
    }
    if (["work", "planning", "inbox", "triage", "workviews"].includes(subPage)) {
      const parent = $(subPage + "-content");
      if (!window.KilnWorkUI) { parent.replaceChildren(element("p", "Work interface is loading.", "small")); return; }
      await window.KilnWorkUI.render(parent, { ...route, page: subPage });
      return;
    }
    if (subPage === "overview") await loadOverview();
    if (subPage === "guide") renderGuide($("guide-content"));
    // r[impl kiln.web.project_page] Project history renders through project.js; Run latest reuses the existing start form.
    if (subPage === "project") {
      if (!window.KilnProject) { $("project-content").replaceChildren(element("p", "Project history is loading.", "small")); return; }
      await window.KilnProject.load($("project-content"), { api, navigate, project: route.project, canOperate: canOperate(), startRun });
      return;
    }
    if (subPage === "projects") {
      renderProjects();
      if (state.startProject) { selectProjectForRun(state.startProject); state.startProject = ""; }
      await Promise.all([loadRefs(), loadProjectHealth()]);
    }
    // r[impl kiln.dashboard.workers.view] The Workers module owns its register, local filters and exact detail.
    if (subPage === "workers") await window.KilnWorkers.load($("workers-content"), { api, navigate, route, isCurrent: () => state.route === route && !!state.session, runLink, projectLink });
    if (subPage === "runbooks") {
      if (!window.KilnRunbooks) { $("runbooks-content").replaceChildren(element("p", "Runbooks are loading.", "small")); return; }
      await window.KilnRunbooks.load($("runbooks-content"), {
        api,
        navigate,
        notice,
        confirmControl,
        route,
        canOperate: canOperate(),
        principal: state.principal,
        getEnrolledProjects: () => state.projects.enrolled,
        runLink,
        isCurrent: () => state.route === route && !!state.session,
      });
      return;
    }
    if (subPage === "activity") await renderEndpoint(`/api/activity?${activityQuery()}`, "activity-content", renderActivity);
    if (subPage === "evidence") {
      void loadRunChoices();
      if (route.run) await renderEndpoint(`${pathFor(route.run)}/evidence`, "evidence-content", renderEvidence);
      else $("evidence-content").replaceChildren(element("p", "Enter a retained run ID. No evidence from another attempt is substituted.", "small"));
    }
    if (subPage === "feeds") await loadFeeds();
    if (subPage === "automation") await renderEndpoint("/api/automation", "automation-content", renderAutomation);
    if (subPage === "operations") await renderEndpoint("/api/operations", "operations-content", renderOperations);
    if (subPage === "cobs") await loadCobs();
    // r[impl kiln.dashboard.source.navigation] The Source page renders through source.js.
    if (subPage === "source") {
      if (!window.KilnSource) { $("source-content").replaceChildren(element("p", "Source browser is loading.", "small")); return; }
      await window.KilnSource.load($("source-content"), { api, navigate, route });
    }
    if (subPage === "repositories") {
      if (!window.KilnSource?.loadRepositories) { $("repositories-content").replaceChildren(element("p", "Repositories workbench is loading.", "small")); return; }
      await window.KilnSource.loadRepositories($("repositories-content"), { api, navigate, route, confirmControl, mutate, canOperate, openInspector, notice });
    }
    // r[impl kiln.dashboard.reviews.view] The Patchgate receipts page renders through receipts.js.
    if (subPage === "reviews") {
      if (!window.KilnReceipts) { $("receipts-content").replaceChildren(element("p", "Patchgate receipts are loading.", "small")); return; }
      await window.KilnReceipts.load($("receipts-content"), { api, navigate, route });
    }
    // r[impl kiln.cairn.workspace] The Cairn changes page renders through cairn.js.
    if (subPage === "cairn") {
      if (!window.KilnCairn) { $("cairn-content").replaceChildren(element("p", "Cairn changes are loading.", "small")); return; }
      await window.KilnCairn.load($("cairn-content"), { api, navigate, route, runLink, confirmControl, canOperate });
    }
    // The Fleet page owns its bounded reads and coordinator commands; leaving the
    // page aborts its refresh cycle.
    if (subPage === "fleet") {
      if (!window.KilnFleet) { $("fleet-content").replaceChildren(element("p", "Fleet coordination is loading.", "small")); return; }
      await window.KilnFleet.load($("fleet-content"), {
        api,
        stream: apiStream,
        subscribeEvents,
        openInspector,
        navigate,
        runLink,
        canOperate,
        confirmControl,
        route,
        isCurrent: () => state.route === route && !!state.session,
      });
    }
    if (subPage === "runs") await loadRunView();
  }
  async function loadRunView() {
    const route = state.route;
    if (!route.run || route.page !== "runs") return;
    if (route.view === "tests") await renderEndpoint(`${pathFor(route.run)}/reports`, "reports-content", renderReports);
    if (route.view === "reviews") await renderEndpoint(`${pathFor(route.run)}/reports`, "reviews-content", (parent, result) => {
      if (!window.KilnReceipts) { parent.append(element("p", "Patchgate receipts are loading.", "small")); return; }
      window.KilnReceipts.renderRun(parent, result, {
        download: (artifact, button) => downloadArtifact(result.run_id || route.run, artifact, button),
        candidateId: state.run?.execution?.candidate_id || "",
        sourceRevision: state.run?.source_revision || "",
        cairnCompletion: state.run?.implementation_verification || null,
      });
    });
    if (route.view === "compare") {
      void loadRunChoices();
      if (route.other) await renderEndpoint(`${pathFor(route.run)}/compare?${new URLSearchParams({ other: route.other })}`, "compare-content", renderComparison);
      else $("compare-content").replaceChildren(element("p", "Choose another retained run. Missing reports or historical durations remain unavailable, not zero.", "small"));
    }
    if (route.view === "activity") await renderEndpoint(`${pathFor(route.run)}/activity?${activityQuery()}`, "run-activity-content", renderActivity);
    if (route.view === "evidence") await renderEndpoint(`${pathFor(route.run)}/evidence`, "run-evidence-content", renderEvidence);
    if (route.view === "artifacts") await loadArtifacts();
    if ((route.view === "logs" || bothPrimaryPanels()) && !state.logKey && !state.logBusy) { maybeAutoFollow(); await readLog(false); }
  }
  const browsingLoads = new Map();
  function beginBrowsingLoad(page, route) {
    const generation = (browsingLoads.get(page) || 0) + 1;
    browsingLoads.set(page, generation);
    const epoch = state.epoch;
    return () => state.session && state.epoch === epoch && state.route === route && browsingLoads.get(page) === generation;
  }
  function browsingClear(parent, label, patch) {
    const button = element("button", label);
    button.type = "button";
    button.addEventListener("click", () => navigate(patch));
    parent.append(button);
  }
  // r[impl kiln.dashboard.cobs.view]
  async function loadCobs() {
    const route = state.route;
    const current = beginBrowsingLoad("cobs", route);
    state.cobsNextCursor = null;
    $("cob-next").disabled = true;
    $("cob-first").disabled = !route.cob_cursor;
    $("cob-status").textContent = "Reading configured local COB stores…";
    $("cob-source-status").replaceChildren();
    const list = $("cob-list"); list.replaceChildren(element("p", "Loading objects…", "small"));
    const detail = $("cob-detail"); detail.replaceChildren();
    if (route.cob || route.cob_repo || route.cob_kind) browsingClear(detail, "Clear selected COB", { cob: "", cob_repo: "", cob_kind: "" });
    if (!route.cob) detail.append(element("h2", "Select a COB"), element("p", "Choose an object to inspect its exact identity and recorded operation content. No object is selected automatically.", "small"));
    else detail.append(element("p", `Selected object: ${route.cob}`, "identity"), element("p", "Loading exact object history…", "small"));
    $("cob-search").value = route.cob_q;
    for (const [id, selected, label] of [["cob-repository", route.cob_repository, "All configured repositories"], ["cob-type", route.cob_type, "All types"]]) {
      const select = $(id);
      if (![...select.options].some((option) => option.value === selected)) { const option = element("option", selected ? `Unavailable selection: ${selected}` : label); option.value = selected; select.append(option); }
      select.value = selected;
    }
    const query = new URLSearchParams({ limit: "25" });
    for (const [key, value] of [["repository", route.cob_repository], ["type", route.cob_type], ["q", route.cob_q], ["cursor", route.cob_cursor]]) if (value) query.set(key, value);
    try {
      const inventory = await api(`/api/cobs?${query}`);
      if (!current()) return;
      list.replaceChildren();
      renderCobs(list, inventory);
      if (route.cob && !inventory.objects.some((object) => object.object_id === route.cob && object.repository === route.cob_repo && object.type_name === route.cob_kind)) detail.append(element("p", "The exact selection is outside this filtered inventory page. Its history is inspected independently.", "small"));
    } catch (error) {
      if (!current() || isAborted(error)) return;
      list.replaceChildren(element("p", explain(error), "warning"));
      $("cob-status").textContent = "COB inventory unavailable. Exact selection is inspected independently; no page was substituted.";
    }
    if (!current() || !route.cob) return;
    detail.replaceChildren();
    browsingClear(detail, "Clear selected COB", { cob: "", cob_repo: "", cob_kind: "" });
    if (!route.cob_repo || !route.cob_kind) {
      detail.append(element("p", "The COB link needs its exact repository, type and object ID. No other object was substituted.", "warning"), element("p", route.cob, "identity"));
      return;
    }
    const loading = element("p", "Loading exact object history…", "small"); detail.append(loading);
    try {
      const selection = new URLSearchParams({ repository: route.cob_repo, type: route.cob_kind, object: route.cob });
      const result = await api(`/api/cobs/object?${selection}`);
      if (!current()) return;
      detail.replaceChildren();
      renderCobDetail(detail, result);
      if (!list.querySelector('[aria-current="true"]')) detail.append(element("p", "The exact selection is outside this filtered inventory page. No different object was substituted.", "small"));
    } catch (error) {
      if (!current() || isAborted(error)) return;
      loading.remove();
      detail.append(element("p", `Object history unavailable: ${explain(error)}`, "warning"), element("p", `${route.cob_repo} · ${route.cob_kind} · ${route.cob}`, "identity"));
    }
  }
  let cobTypeLabels = new Map();
  function cobTypeLabel(typeName) { return cobTypeLabels.get(typeName) || typeName; }
  function shortOid(value) { return typeof value === "string" && value.length > 14 ? `${value.slice(0, 12)}…` : text(value); }
  function shortAuthor(value) { const name = String(value ?? "").split(" <")[0]; return name || "Unavailable"; }
  function renderCobs(parent, result) {
    if (!Array.isArray(result.objects) || !Array.isArray(result.repositories) || !Array.isArray(result.types)) throw new Error("COB inventory response is invalid.");
    for (const row of result.types) if (row && typeof row.id === "string") cobTypeLabels.set(row.id, text(row.label));
    for (const [id, rows, label, selected] of [["cob-repository", result.repositories, "All configured repositories", state.route.cob_repository], ["cob-type", result.types, "All types", state.route.cob_type]]) {
      options($(id), rows, label, selected);
      if (selected && ![...$(id).options].some((option) => option.value === selected)) { const option = element("option", `Unavailable selection: ${selected}`); option.value = selected; $(id).append(option); $(id).value = selected; }
    }
    state.cobsNextCursor = result.next_cursor || null;
    $("cob-next").disabled = !state.cobsNextCursor;
    $("cob-status").textContent = `${result.objects.length} shown of ${text(result.total)} matching local objects · bounded snapshot${result.complete === false ? " · incomplete: some stores unavailable" : ""}`;
    const sources = $("cob-source-status");
    if (!result.configured) sources.append(element("p", "No COB stores are configured. The service operator must admit a separate --cobs repository policy. This is not proof that no Onix objects exist.", "warning"));
    for (const source of result.sources || []) sources.append(element("p", source.status === "available" ? `${text(source.label)} · ${text(source.objects)} local objects` : `${text(source.label)} · unavailable or outside read bounds. No absence is inferred.`, source.status === "available" ? "small" : "warning"));
    for (const object of result.objects) {
      const link = element("a", undefined, "run-choice cob-choice");
      const patch = { page: "cobs", cob: object.object_id, cob_repo: object.repository, cob_kind: object.type_name };
      link.href = `?${new URLSearchParams({ ...state.route, ...patch })}`;
      link.title = `${object.type_name} · ${object.object_id}`;
      if (state.route.cob === object.object_id && state.route.cob_repo === object.repository && state.route.cob_kind === object.type_name) link.setAttribute("aria-current", "true");
      const head = element("div", undefined, "cob-head");
      head.append(element("span", cobTypeLabel(object.type_name), "cob-type"), element("span", text(object.repository_label), "cob-repo"));
      const footer = element("div", undefined, "cob-foot");
      const oid = element("code", shortOid(object.object_id), "cob-oid");
      oid.title = text(object.object_id);
      const recorded = when(object.recorded_at_ms);
      recorded.classList.add("cob-when");
      footer.append(oid, recorded, element("span", `${shortAuthor(object.author)} · ${object.ref_tips.length} ref tip(s)`, "cob-when"));
      link.append(head, element("strong", text(object.title), "cob-title"));
      if (object.excerpt) link.append(element("span", text(object.excerpt), "cob-excerpt"));
      link.append(footer);
      link.addEventListener("click", (event) => { if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); navigate(patch); } });
      parent.append(link);
    }
    if (!result.objects.length) parent.append(element("p", result.complete === false ? "No matching objects were read from the available subset. Unavailable stores may contain objects." : "No matching COBs in the configured local snapshot. Remote peers and unconfigured stores were not queried.", "small"));
  }
  // r[impl kiln.dashboard.source.view]
  function patchCommitCandidates(object) {
    const candidates = new Set();
    const accept = (value) => { if (typeof value === "string" && /^[0-9a-f]{40}$/.test(value)) candidates.add(value); };
    for (const operation of object.operations || []) {
      for (const action of operation.actions || []) {
        const value = action.value || {};
        accept(value.oid);
        accept(value.revision);
        accept(value.base);
        const state = value.state;
        if (state && typeof state === "object") { accept(state.oid); accept(state.revision); }
      }
    }
    return [...candidates].slice(0, 4);
  }
  async function attachPatchSources(parent, object) {
    if (object.identity.type_name !== "xyz.radicle.patch") return;
    const candidates = patchCommitCandidates(object);
    if (!candidates.length) return;
    const epoch = state.epoch;
    const route = state.route;
    const generation = browsingLoads.get("cobs");
    const current = () => state.session && state.epoch === epoch && state.route === route && browsingLoads.get("cobs") === generation && card.isConnected;
    const card = section("Recorded patch commits in admitted sources", parent, "record-card");
    card.append(element("p", "A patch commit is linked only when an admitted source actually contains that exact object. Coverage is not inferred from the patch itself.", "small"));
    let links = 0;
    let unavailable = false;
    for (const revision of candidates) {
      let result;
      try { result = await api(`/api/sources/lookup?${new URLSearchParams({ revision })}`); }
      catch (error) { if (!current() || isAborted(error)) return; unavailable = true; card.append(element("p", `Commit lookup unavailable: ${explain(error)}`, "warning")); continue; }
      if (!current()) return;
      for (const match of result.matches || []) {
        const button = element("button", `Open ${match.label} · ${match.kind} · ${revision.slice(0, 12)}…`);
        button.type = "button";
        button.title = revision;
        // r[impl kiln.dashboard.source.navigation.scenario.changes] A recorded patch commit opens that revision's Changes view.
        button.addEventListener("click", () => navigate({ page: "source", src: match.id, rev: revision, sview: "changes", spath: "", sfile: "", sq: "", sline: "", cob: "", cob_repo: "", cob_kind: "", cob_type: "", cob_cursor: "", cob_repository: "", cob_q: "" }));
        card.append(button);
        links += 1;
      }
    }
    if (!links) card.append(element("p", unavailable ? "Commit coverage is unknown because a lookup was unavailable. Nothing was substituted." : "No admitted source contains a commit from this patch. Nothing was substituted.", "small"));
  }
  const cobPresentations = {
    "xyz.onixresearch.cairn-change": { label: "Cairn change", actions: ["init", "set_artifact", "attach_evidence", "set_lifecycle"] },
    "xyz.onixresearch.rfd": { label: "RFD", actions: ["init", "set_artifact", "add_discussion", "add_link", "set_lifecycle", "set_publication"] },
    "xyz.onixresearch.adr": { label: "ADR", actions: ["init", "set_artifact", "add_discussion", "add_link", "set_lifecycle", "set_publication"] },
    "xyz.onixresearch.verification-obligation": { label: "Verification obligation", actions: ["init", "set_field", "attach_evidence", "add_finding", "respond_finding", "set_disposition", "discharge", "reject", "invalidate", "resolve_field"] },
    "xyz.onixresearch.release-decision": { label: "Release decision", actions: ["init", "attach_evidence", "add_finding", "respond_finding", "dispose_finding", "vote", "withdraw", "invalidate", "supersede", "resolve"] },
    "xyz.onixresearch.policy-exception": { label: "Policy exception", actions: ["init", "revise", "add_finding", "resolve_finding", "approve", "reject", "observe_validity", "revoke", "expire", "close", "supersede"] },
    "xyz.radicle.issue": { label: "Issue", actions: ["edit", "comment", "comment.edit", "comment.redact", "comment.react", "lifecycle", "label", "assign"] },
    "xyz.radicle.patch": { label: "Patch", actions: ["edit", "revision", "revision.edit", "revision.redact", "revision.react", "revision.comment", "revision.comment.edit", "revision.comment.redact", "revision.comment.react", "review", "review.edit", "review.redact", "review.comment", "review.comment.edit", "review.comment.redact", "review.comment.react", "merge", "lifecycle", "label", "assign"] },
    "xyz.radworks.job": { label: "CI job", actions: ["Request", "Run", "Finished"] },
  };
  function cobFieldLabel(key) {
    const words = key.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/[_.]/g, " ");
    return words.charAt(0).toUpperCase() + words.slice(1);
  }
  // COB data must not acquire renderRecorded's CI links or timestamp semantics.
  function renderCobFields(parent, value, depth = 0) {
    if (value === null) { parent.append(element("span", "null (recorded)")); return; }
    if (typeof value !== "object") { parent.append(element("span", value === "" ? "(empty text)" : text(value), "cob-text")); return; }
    if (depth >= 4) { parent.append(element("pre", JSON.stringify(value, null, 2), "identity cob-action")); return; }
    if (Array.isArray(value)) {
      if (!value.length) { parent.append(element("span", "Empty list (recorded)", "small")); return; }
      const list = element("ul", undefined, "cob-values");
      for (const item of value) { const row = element("li"); renderCobFields(row, item, depth + 1); list.append(row); }
      parent.append(list); return;
    }
    if (!Object.keys(value).length) { parent.append(element("span", "Empty record (recorded)", "small")); return; }
    const facts = element("dl", undefined, "facts cob-fields"); parent.append(facts);
    for (const [key, item] of Object.entries(value)) {
      const cell = element("dd"); facts.append(element("dt", cobFieldLabel(key)), cell);
      if ((key === "artifacts" || key === "fields") && item && typeof item === "object" && !Array.isArray(item)) {
        for (const [name, content] of Object.entries(item)) {
          const document = element("details", undefined, "cob-document");
          document.open = ["proposal", "title", "body", "document", "statement"].includes(name);
          document.append(element("summary", `${key === "artifacts" ? "Artifact" : "Field"}: ${name}`));
          renderCobFields(document, content, depth + 1); cell.append(document);
        }
        if (!Object.keys(item).length) cell.append(element("span", "No recorded content", "small"));
      } else renderCobFields(cell, item, depth + 1);
    }
  }
  function renderCobAction(parent, typeName, action) {
    const value = action.value;
    const presentation = Object.hasOwn(cobPresentations, typeName) ? cobPresentations[typeName] : null;
    let kind = value?.type;
    let content = value;
    const record = value && typeof value === "object" && !Array.isArray(value);
    if (typeName === "xyz.radworks.job" && record && Object.keys(value).length === 1) {
      kind = Object.keys(value)[0]; content = value[kind];
    }
    const known = record && presentation?.actions.includes(kind) && content && typeof content === "object" && !Array.isArray(content)
      && (kind !== "init" || (content.state && typeof content.state === "object" && !Array.isArray(content.state)));
    const card = element("article", undefined, "cob-content"); parent.append(card);
    if (known) {
      card.append(element("h4", `${presentation.label} · ${kind === "init" ? "Initial record" : cobFieldLabel(kind)}`));
      const fields = kind === "init" ? content.state : Object.fromEntries(Object.entries(content).filter(([key]) => key !== "type"));
      renderCobFields(card, fields);
    } else card.append(element("p", "Unrecognized action shape. Exact recorded content is available below; no meaning was inferred.", "small"));
    const raw = element("details", undefined, "cob-raw"); raw.open = !known;
    raw.append(element("summary", `Raw action ${text(action.index)} · ${text(action.bytes)} bytes · blob ${text(action.blob_id)}`), element("pre", JSON.stringify(value, null, 2), "identity cob-action"));
    card.append(raw);
  }
  function renderCobDetail(parent, result) {
    const object = result.object;
    if (!object?.identity || !Array.isArray(object.operations)) throw new Error("COB history response is invalid.");
    const identity = object.identity;
    parent.append(element("h2", text(identity.title)), element("p", "Recorded root title or import summary; later edits remain separate observations below.", "small"));
    const clear = element("button", "Clear selected COB"); clear.type = "button"; clear.addEventListener("click", () => navigate({ cob: "", cob_repo: "", cob_kind: "" })); parent.append(clear);
    const badges = element("div", undefined, "badges");
    badges.append(element("span", cobTypeLabel(identity.type_name), "cob-type"), element("span", "Signatures not verified", "state-badge"), element("span", text(object.observation), "state-badge"));
    parent.append(badges);
    const exact = element("details", undefined, "cob-exact"); exact.append(element("summary", "Exact object identity & observed ref tips")); parent.append(exact);
    const facts = element("dl", undefined, "facts"); addFacts(facts, [["Repository", identity.repository_label], ["Native repository ID", identity.repository_id], ["Native object ID", identity.object_id], ["Kiln observation key", identity.id], ["Observed operations", object.operation_count], ["Retained action bytes", object.retained_action_bytes]]); exact.append(facts);
    const tips = section("Observed ref tips", exact, "record-card"); renderCobFields(tips, identity.ref_tips);
    // r[impl kiln.web.page_consistency] The history-reading boundary stays reachable without a warning box above every object.
    parent.append(aboutView("How to read this history", "Recorded actions, ordered by operation identity—not a current state or chosen concurrent winner. Authors, status and decision claims are unverified. Embedded text and URLs are inert."));
    for (const operation of object.operations) {
      const card = section(`Operation ${shortOid(operation.id)}`, parent, "record-card cob-operation");
      const byline = element("p", `${shortAuthor(operation.author_name)} · `, "small");
      byline.append(when(operation.recorded_at_ms), document.createTextNode(" · unverified author"));
      card.append(byline);
      for (const action of operation.actions || []) renderCobAction(card, identity.type_name, action);
      const framing = element("details", undefined, "cob-framing"); framing.append(element("summary", "Exact operation metadata"));
      const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["Exact operation ID", operation.id], ["Recorded", timestamp(operation.recorded_at_ms)], ["Git author (unverified)", `${text(operation.author_name)} <${text(operation.author_email)}>`], ["Git parents", operation.git_parents], ["History parents", operation.history_parents], ["Commit message", operation.message]]); framing.append(metadata); card.append(framing);
    }
    for (const boundary of result.non_claims || []) parent.append(element("p", boundary, "small"));
    void attachPatchSources(parent, object);
  }
  // r[impl kiln.web.minimal_console]
  // r[impl kiln.dashboard.complete.overview]
  // r[impl kiln.web.readable_observations.scenario.failed]
  // r[impl kiln.web.console_layout]
  // The dashboard composes existing bounded reads only (/api/overview, /api/workers, /api/activity).
  // Each read owns its sections' loading, empty and unavailable states; one failure never blocks another.
  const OVERVIEW_ACTIVITY_LIMIT = 5;
  const OVERVIEW_SECTIONS = Object.freeze({
    overview: ["overview-health", "overview-attention", "overview-passed", "overview-other", "overview-trend"],
    workers: ["overview-workers"],
    activity: ["overview-activity"],
  });
  const OVERVIEW_UNAVAILABLE = Object.freeze({
    overview: "Project health unavailable",
    workers: "Worker capacity unavailable",
    activity: "Recent activity unavailable",
  });
  function dashboardSection(parent, id, title, className, link) {
    const node = element("section", undefined, className);
    const head = element("div", undefined, "dashboard-section-head");
    const heading = element("h2", undefined, "card-title");
    heading.id = `${id}-title`;
    heading.append(document.createTextNode(title));
    const count = element("span", undefined, "section-count");
    count.id = `${id}-count`;
    head.append(heading, count);
    if (link) head.append(link);
    const body = element("div", undefined, "dashboard-section-body stack stack-sm");
    body.id = id;
    node.setAttribute("aria-labelledby", heading.id);
    node.append(head, body);
    parent.append(node);
    return node;
  }
  function buildOverview(parent) {
    const cockpit = element("div", undefined, "factory-cockpit");
    const ribbon = element("dl", undefined, "factory-telemetry-ribbon");
    ribbon.id = "factory-ribbon";
    const grid = element("div", undefined, "factory-grid");
    const main = element("div", undefined, "factory-main");
    const side = element("div", undefined, "factory-side");
    dashboardSection(main, "factory-attention", "Attention & escalations", "card dashboard-section", workspaceLink({ page: "inbox", tab: "attention" }, "Open full Attention"));
    const deliverySection = dashboardSection(main, "factory-delivery", "Live delivery spine & merge train", "card dashboard-section", workspaceLink({ page: "cairn" }, "Open full Delivery"));
    const mergeTrainStrip = element("div", undefined, "factory-merge-train-strip");
    mergeTrainStrip.id = "factory-merge-train";
    const spineList = element("div", undefined, "delivery-spine-list");
    spineList.id = "factory-spine";
    const deliveryBody = deliverySection.querySelector(".dashboard-section-body");
    deliveryBody?.append(mergeTrainStrip, spineList);
    const ciDetails = element("details", undefined, "panel overview-timing");
    const ciSummary = element("summary", "CI project health & retained runs");
    const ciStack = element("div", undefined, "view-stack");
    dashboardSection(ciStack, "overview-health", "Project health", "card health-hero");
    dashboardSection(ciStack, "overview-attention", "Failing CI projects", "dashboard-section");
    dashboardSection(ciStack, "overview-passed", "Recently passed", "card dashboard-section");
    dashboardSection(ciStack, "overview-other", "In progress & unresolved", "card dashboard-section");
    const trend = element("details", undefined, "panel overview-timing");
    const trendBody = element("div", undefined, "stack stack-sm");
    trendBody.id = "overview-trend";
    trend.append(element("summary", "Recorded duration trend"), trendBody);
    ciStack.append(trend);
    ciDetails.append(ciSummary, ciStack);
    main.append(ciDetails);

    dashboardSection(side, "factory-machines", "Machine & slot census", "card side-card", workspaceLink({ page: "fleet", tab: "fleet" }, "Open Fleet"));
    dashboardSection(side, "factory-claims", "Claims & budget", "card side-card", workspaceLink({ page: "fleet", tab: "fleet" }, "All claims"));
    dashboardSection(side, "overview-workers", "Workers", "card side-card", workspaceLink({ page: "workers", worker_q: "", worker_status: "", worker: "" }, "All workers"));
    dashboardSection(side, "overview-activity", "Recent activity", "card side-card", workspaceLink({ page: "activity", activity_cursor: "" }, "All activity"));
    grid.append(main, side);
    cockpit.append(ribbon, grid);
    parent.replaceChildren(cockpit);
  }
  function setSectionCount(id, value) { const node = $(`${id}-count`); if (node) node.textContent = value; }
  // r[impl kiln.web.console_layout.scenario.unavailable]
  async function overviewRead(target, key, render) {
    const generation = state.pageGeneration;
    const endpointKey = `overview:${key}`;
    const requestGeneration = (state.endpointGeneration.get(endpointKey) || 0) + 1;
    state.endpointGeneration.set(endpointKey, requestGeneration);
    const bodies = () => OVERVIEW_SECTIONS[key].map((id) => $(id)).filter(Boolean);
    // Live refreshes keep the previous observation in place; only an empty section shows a skeleton.
    for (const body of bodies()) if (!body.childElementCount) body.replaceChildren(loadingState("Reading bounded observations…"));
    const current = () => generation === state.pageGeneration && requestGeneration === state.endpointGeneration.get(endpointKey) && state.route.page === "overview";
    try {
      const result = await api(target);
      if (!current()) return;
      render(result);
    } catch (error) {
      if (isAborted(error) || !current()) return;
      for (const body of bodies()) {
        const container = body.closest("section, details");
        if (container) container.hidden = false;
        const unavailable = errorState(`${OVERVIEW_UNAVAILABLE[key]}: ${explain(error)} Nothing is inferred and no sample data is shown.`);
        unavailable.dataset.state = "unavailable";
        body.replaceChildren(unavailable);
        setSectionCount(body.id, "");
      }
    }
  }
  function safeCall(path) {
    return api(path, { quiet: true }).then((value) => ({ ok: true, value }), (error) => ({ ok: false, error }));
  }
  function renderFactoryRibbon() {
    const ribbon = $("factory-ribbon");
    if (!ribbon) return;
    const dispatch = state.factory.dispatch;
    const claims = state.factory.claims;
    const machines = state.factory.machines?.machines || [];
    const attention = state.factory.attention;
    const now = Date.now();
    const openClaims = claims?.claims || [];
    const expiringCount = openClaims.filter((c) => Number.isSafeInteger(c.expires_ms) && c.expires_ms - now <= LEASE_EXPIRING_MS).length;
    const foreignCount = (claims?.foreign_claims || []).length;
    let usedSlots = 0;
    let capSlots = 0;
    let offlineMachines = 0;
    for (const m of machines) {
      if (m.health === "unavailable") { offlineMachines++; continue; }
      if (m.slots && typeof m.slots === "object") {
        for (const s of Object.values(m.slots)) {
          if (Number.isSafeInteger(s?.used)) usedSlots += s.used;
          if (Number.isSafeInteger(s?.capacity)) capSlots += s.capacity;
        }
      }
    }
    const dispatchState = dispatch?.state === "paused" ? "PAUSED" : dispatch?.state === "running" ? "RUNNING" : "UNAVAILABLE";
    const deferred = dispatch?.last_tick?.deferred_count ?? 0;
    const mirror = claims?.foreign_mirror?.status || "unconfigured";
    const escalations = attention?.total ?? 0;

    ribbon.replaceChildren();
    const addCell = (term, buildValue) => {
      const box = element("div", undefined, "factory-telemetry-cell");
      const dt = element("dt", term);
      const dd = element("dd");
      buildValue(dd);
      box.append(dt, dd);
      ribbon.append(box);
    };
    addCell("Dispatch", (dd) => {
      dd.append(statePill(dispatchState === "RUNNING" ? "success" : dispatchState === "PAUSED" ? "warning" : "unknown", dispatchState));
      if (deferred > 0) dd.append(element("span", `${deferred} deferred`, "small"));
      if (canOperate() && (dispatchState === "RUNNING" || dispatchState === "PAUSED")) {
        const paused = dispatchState === "PAUSED";
        const btn = element("button", paused ? "Resume" : "Pause", "quiet");
        btn.type = "button";
        btn.addEventListener("click", async () => {
          const action = paused ? "resume" : "pause";
          const choice = await confirmControl({
            title: paused ? "Resume Fleet dispatch?" : "Pause Fleet dispatch?",
            verb: paused ? "Resume dispatch" : "Pause dispatch",
            dismiss: paused ? "Keep paused" : "Keep running",
            warning: paused ? "Allows new work claims to be dispatched by the coordinator." : "Stops dispatching new work claims; running attempts continue.",
            facts: [["Current state", dispatchState], ["Deferred", String(deferred)]],
            reason: paused ? null : { label: "Pause reason", placeholder: "Why dispatch is being paused", required: false },
            cliCommand: `kiln-fleet dispatch ${action}`,
            opener: btn,
          });
          if (!choice.confirmed) return;
          const res = await mutate(`/api/fleet/dispatch/${action}`, { reason: choice.reason || undefined, request_id: choice.requestId }, `Dispatch ${action}d.`);
          if (res) void loadOverview();
        });
        dd.append(btn);
      }
    });
    addCell("Swarm mirror", (dd) => {
      dd.append(statePill(mirror === "ok" ? "success" : mirror === "unavailable" ? "failure" : "unknown", mirror.toUpperCase()));
    });
    addCell("Escalations", (dd) => {
      dd.append(element("span", String(escalations)), workspaceLink({ page: "inbox", tab: "attention" }, "View ->", "row-link"));
    });
    addCell("Claims <15m / foreign", (dd) => {
      dd.append(element("span", `${expiringCount} expiring · ${foreignCount} foreign`));
    });
    addCell("Host slot occupancy", (dd) => {
      dd.append(element("span", `${usedSlots} / ${capSlots}${offlineMachines ? ` (${offlineMachines} offline)` : ""}`));
    });
    // r[impl kiln.web.repository_import_ui] Compact 1-line Repositories & Fleet Sync summary linking to ?page=source&tab=repositories.
    addCell("Repositories & Fleet Sync", (dd) => {
      const repoData = state.factory.repositories;
      if (!repoData) {
        dd.append(element("span", "Unavailable", "small"), workspaceLink({ page: "source", tab: "repositories" }, "Open ->", "row-link"));
        return;
      }
      const list = Array.isArray(repoData.repositories) ? repoData.repositories : [];
      const sum = repoData.summary || {
        total: list.length,
        diverged: list.filter((r) => r.sync_status === "diverged").length,
        dirty: list.filter((r) => r.sync_status === "dirty").length,
        unknown: list.filter((r) => r.sync_status === "unknown").length,
      };
      const label = `${sum.total ?? 0} repos · ${sum.diverged ?? 0} diverged · ${sum.dirty ?? 0} dirty${sum.unknown ? ` · ${sum.unknown} offline` : ""}`;
      dd.append(workspaceLink({ page: "source", tab: "repositories" }, label, "row-link"));
    });
  }
  function attentionKindLabel(kind) {
    return {
      UntrustedRunApproval: "Untrusted PR",
      FleetSessionDecision: "Fleet decision",
      CampaignDecision: "Campaign decision",
      PlanReviewGate: "Plan review gate",
      UnknownTerminationOrDispatch: "Unknown termination",
      ExpiringOrConflictClaim: "Claim attention",
    }[kind] || String(kind || "Attention");
  }
  function attentionInspectTarget(item) {
    const t = item?.target || {};
    if (item?.kind === "PlanReviewGate" || item?.kind === "UnknownTerminationOrDispatch" || item?.kind === "CampaignDecision" || item?.kind === "UntrustedRunApproval" || item?.kind === "FleetSessionDecision") {
      return { kind: "attention", id: item.id };
    }
    if (item?.kind === "ExpiringOrConflictClaim" && t.claim_id) return { kind: "claim", id: t.claim_id };
    if (t.attempt_id) return { kind: "attempt", id: t.attempt_id };
    if (t.run_id) return { kind: "run", id: t.run_id };
    return { kind: "attention", id: item.id };
  }
  async function executeAttentionAction(item, actionName, opener) {
    const t = item?.target || {};
    if (actionName === "wake" && t.campaign_id) {
      await mutate(`/api/implementation/campaigns/${encodeURIComponent(t.campaign_id)}/wake`, { kind: "operator", reason: "Operator woke from Factory/Attention" }, "Campaign wake admitted.");
    } else if (actionName === "accept_plan" && t.work_id && t.plan_id) {
      await mutate("/api/implementation/plan-review", { work_id: t.work_id, plan_id: t.plan_id, verdict: "accepted" }, "Plan accepted.");
    } else if (actionName === "request_changes" && t.work_id && t.plan_id) {
      await mutate("/api/implementation/plan-review", { work_id: t.work_id, plan_id: t.plan_id, verdict: "changes_requested" }, "Requested plan changes.");
    } else if (actionName === "renew" && t.claim_id) {
      const claim = (state.factory.claims?.claims || []).find((c) => c.identity === t.claim_id);
      if (claim) await mutate(`/api/fleet/claims/${claim.identity}/renew`, { fence: claim.fence, lease_minutes: 30 }, "Claim renewed +30m.");
      else openInspector("claim", t.claim_id, []);
    } else if (actionName === "approve" && t.run_id && t.source_commit) {
      const choice = await confirmControl({
        title: `Approve untrusted run ${shortIdentity(t.run_id)}?`,
        verb: "Approve and run",
        dismiss: "Keep blocked",
        facts: [["Run", t.run_id], ["Exact commit", t.source_commit]],
        cliCommand: `kiln runs approve --run ${t.run_id} --commit ${t.source_commit}`,
        opener,
      });
      if (choice.confirmed) await mutate(`${pathFor(t.run_id)}/approve`, { source_commit: t.source_commit, request_id: choice.requestId }, "Untrusted run approved.");
    } else if (actionName === "reconcile" && t.work_id && t.attempt_id && Number.isSafeInteger(t.fence)) {
      await mutate("/api/implementation/reconcile-termination", { campaign_id: t.campaign_id || undefined, work_id: t.work_id, attempt_id: t.attempt_id, fence: t.fence }, "Termination reconciled.");
    } else if (actionName === "reconcile_run" && t.run_id) {
      await mutate(`${pathFor(t.run_id)}/reconcile`, {}, "Provider reconciliation probed.");
    } else {
      const target = attentionInspectTarget(item);
      openInspector(target.kind, target.id, [item]);
      return;
    }
    if (state.route?.page === "overview") void loadOverview();
    else if (state.route?.page === "inbox") void loadAttentionPage();
  }
  function renderAttentionList(container, items, { compactHealthy = false, emptyTitle = "All clear", emptyMsg = "0 items need operator attention." } = {}) {
    container.replaceChildren();
    if (!items.length) {
      if (compactHealthy) {
        const bar = element("div", undefined, "factory-attention-healthy");
        bar.append(element("strong", "All clear · 0 items need operator attention"), element("span", "All monitored escalations, approvals, and claims are clear.", "small"));
        container.append(bar);
      } else {
        container.append(emptyState(emptyTitle, emptyMsg));
      }
      return;
    }
    const list = element("div", undefined, "factory-attention-list");
    const inspectorList = items.map((item) => attentionInspectTarget(item));
    for (const item of items) {
      const card = element("article", undefined, "factory-attention-item");
      card.tabIndex = 0;
      card.dataset.attentionId = item.id;
      card.dataset.severity = item.severity || "medium";
      const target = attentionInspectTarget(item);
      if (state.inspector.open && state.inspector.kind === target.kind && state.inspector.id === target.id) card.dataset.active = "true";
      const head = element("div", undefined, "factory-attention-head");
      head.append(
        statePill(item.severity === "critical" ? "failure" : item.severity === "high" ? "warning" : "queued", attentionKindLabel(item.kind), item.kind),
        element("span", item.ttl_remaining_ms != null ? `TTL ${ui.formatDuration(item.ttl_remaining_ms)}` : item.age_ms != null ? `Age ${ui.formatDuration(item.age_ms)}` : "", "small mono"),
      );
      const title = element("h3", item.title || item.id, "factory-attention-title");
      const summary = element("p", item.context_preview?.summary || "", "small");
      const actions = element("div", undefined, "factory-attention-actions");
      const inspectBtn = element("button", "Inspect", "primary");
      inspectBtn.type = "button";
      inspectBtn.addEventListener("click", () => openInspector(target.kind, target.id, inspectorList, inspectBtn));
      actions.append(inspectBtn);
      if (canOperate()) {
        if (item.kind === "PlanReviewGate" && item.target?.work_id && item.target?.plan_id) {
          const accept = element("button", "Accept plan", "quiet");
          accept.type = "button";
          accept.addEventListener("click", () => { void executeAttentionAction(item, "accept_plan", accept); });
          actions.append(accept);
        } else if (item.kind === "UntrustedRunApproval" && item.target?.run_id && item.target?.source_commit) {
          const approve = element("button", "Approve…", "quiet");
          approve.type = "button";
          approve.addEventListener("click", () => { void executeAttentionAction(item, "approve", approve); });
          actions.append(approve);
        } else if (item.kind === "UnknownTerminationOrDispatch" && item.target?.attempt_id) {
          const rec = element("button", "Reconcile", "quiet");
          rec.type = "button";
          rec.addEventListener("click", () => { void executeAttentionAction(item, "reconcile", rec); });
          actions.append(rec);
        } else if (item.kind === "ExpiringOrConflictClaim" && item.target?.claim_id) {
          const renew = element("button", "Renew +30m", "quiet");
          renew.type = "button";
          renew.addEventListener("click", () => { void executeAttentionAction(item, "renew", renew); });
          actions.append(renew);
        } else if (item.target?.campaign_id) {
          const wake = element("button", "Wake", "quiet");
          wake.type = "button";
          wake.addEventListener("click", () => { void executeAttentionAction(item, "wake", wake); });
          actions.append(wake);
        }
      }
      card.addEventListener("keydown", (event) => {
        if (event.target !== card && !card.contains(document.activeElement)) return;
        if (event.ctrlKey || event.metaKey || event.altKey) return;
        if (event.key === "p") { event.preventDefault(); openInspector(target.kind, target.id, inspectorList, card); }
        else if (event.key === "a" && canOperate()) {
          event.preventDefault();
          const act = item.kind === "PlanReviewGate" ? "accept_plan" : item.kind === "UntrustedRunApproval" ? "approve" : item.target?.campaign_id ? "wake" : "inspect";
          void executeAttentionAction(item, act, card);
        } else if (event.key === "r" && canOperate()) {
          event.preventDefault();
          const act = item.kind === "ExpiringOrConflictClaim" ? "renew" : item.kind === "UnknownTerminationOrDispatch" ? "reconcile" : "inspect";
          void executeAttentionAction(item, act, card);
        } else if (event.key === "X" && event.shiftKey && canOperate()) {
          event.preventDefault();
          if (item.kind === "PlanReviewGate") void executeAttentionAction(item, "request_changes", card);
          else openInspector(target.kind, target.id, inspectorList, card);
        }
      });
      card.append(head, title, ...(summary.textContent ? [summary] : []), actions);
      list.append(card);
    }
    container.append(list);
  }
  function renderFactoryAttention(result) {
    state.factory.attention = result;
    const container = $("factory-attention");
    if (!container) return;
    const items = Array.isArray(result?.items) ? result.items : [];
    setSectionCount("factory-attention", items.length ? String(items.length) : "0");
    renderAttentionList(container, items, { compactHealthy: true });
    updateGlobalLiveStrip();
    renderFactoryRibbon();
  }
  function renderFactoryMergeTrains(codeReviewSnapshot) {
    const strip = $("factory-merge-train");
    if (!strip) return;
    strip.replaceChildren();
    const trains = typeof window.KilnCodeReview?.summarizeMergeTrains === "function"
      ? window.KilnCodeReview.summarizeMergeTrains(codeReviewSnapshot)
      : [];
    if (!trains.length) {
      strip.append(element("p", "No active speculative merge trains.", "small"));
      return;
    }
    for (const train of trains) {
      const row = element("div", undefined, "factory-merge-train-line");
      const link = workspaceLink({ page: "code-review", review_tab: "queue" }, train.target_ref || "Merge queue", "row-link");
      const meta = element("span", train.summary_text || `${train.mode || "queue"} · ${train.active_count ?? 0} active`, "mono small");
      row.append(link, statePill(train.uncertain_count > 0 ? "warning" : train.blocked_count > 0 ? "failure" : "running", train.status || train.mode || "active"), meta);
      strip.append(row);
    }
  }
  function renderFactoryDeliverySpine() {
    const spine = $("factory-spine");
    if (!spine) return;
    spine.replaceChildren();
    const changes = Array.isArray(state.factory.cairn?.changes) ? state.factory.cairn.changes : [];
    const attempts = Array.isArray(state.factory.attempts) ? state.factory.attempts : [];
    const claims = Array.isArray(state.factory.claims?.claims) ? state.factory.claims.claims : [];
    const reviews = Array.isArray(state.factory.codeReview?.changes) ? state.factory.codeReview.changes : [];
    const spineItems = [];
    for (const ch of changes.slice(0, 8)) {
      const changeId = ch.change || ch.id || "";
      const matchingClaim = claims.find((c) => JSON.stringify(c.scope || "").includes(changeId));
      const matchingAttempt = attempts.find((a) => (a.title || "").includes(changeId) || (a.work || "").includes(changeId));
      const matchingReview = reviews.find((r) => (r.title || "").includes(changeId) || (r.jj_change_id && r.jj_change_id === ch.jj_change_id));
      const inspectTarget = matchingAttempt ? { kind: "attempt", id: matchingAttempt.attempt } : { kind: "cairn", id: `${ch.repository || ""}/${changeId}` };
      spineItems.push({ ch, changeId, matchingClaim, matchingAttempt, matchingReview, inspectTarget });
    }
    setSectionCount("factory-delivery", String(changes.length));
    if (!spineItems.length) {
      spine.append(element("p", "No Cairn changes listed in the active delivery spine.", "small"));
      return;
    }
    const navList = spineItems.map((row) => row.inspectTarget);
    for (const item of spineItems) {
      const row = element("article", undefined, "delivery-spine-row");
      row.tabIndex = 0;
      const head = element("div", undefined, "delivery-spine-head");
      head.append(element("strong", item.changeId || "Unnamed change"), element("span", item.ch.lane || "unknown", "chip"));
      const stages = element("div", undefined, "delivery-spine-stages");
      const stage = (name, tone) => {
        const chip = element("span", name, "delivery-stage-chip");
        if (tone) chip.dataset.stage = tone;
        return chip;
      };
      stages.append(
        stage("Spec", item.ch.blockers > 0 ? "warning" : "good"),
        stage("Claim", item.matchingClaim ? "good" : undefined),
        stage("Attempt", item.matchingAttempt ? (item.matchingAttempt.state === "executing" ? "running" : "good") : undefined),
        stage("Candidate", item.matchingAttempt?.state === "candidate_produced" || item.matchingAttempt?.state === "verifying" ? "good" : undefined),
        stage("CI", item.matchingReview?.ci_summary === "pass" ? "good" : item.matchingReview?.ci_summary === "fail" ? "failure" : item.matchingReview?.ci_summary === "unknown" ? "warning" : undefined),
        stage("Review/Queue", item.matchingReview?.queue_position ? `running` : item.matchingReview ? "good" : undefined),
        stage("Land", item.ch.lane === "integrated" || item.ch.lane === "recorded" ? "good" : undefined),
      );
      row.append(head, stages);
      const openRow = () => openInspector(item.inspectTarget.kind, item.inspectTarget.id, navList, row);
      row.addEventListener("click", openRow);
      row.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); openRow(); } });
      spine.append(row);
    }
  }
  function renderFactoryMachines(result) {
    state.factory.machines = result;
    const container = $("factory-machines");
    if (!container) return;
    container.replaceChildren();
    const machines = Array.isArray(result?.machines) ? result.machines : [];
    setSectionCount("factory-machines", String(machines.length));
    if (!machines.length) {
      container.append(emptyState("No machines configured", "No herdr machines returned by the Fleet coordinator."));
      return;
    }
    const list = element("div", undefined, "factory-machine-list");
    for (const m of machines) {
      const card = element("article", undefined, "factory-machine-card");
      card.dataset.health = m.health || (m.state === "reporting" ? "available" : "unavailable");
      const head = element("div", undefined, "factory-attention-head");
      const statusTone = m.health === "unavailable" ? "failure" : m.draining ? "warning" : "success";
      const statusText = m.health === "unavailable" ? "OFFLINE · Unavailable" : m.draining ? "Draining" : "Available";
      head.append(element("code", m.machine), statePill(statusTone, statusText));
      card.append(head);
      if (m.health === "unavailable") {
        card.append(element("p", m.error?.detail || "Host unreachable (HerdrFailure); slot capacity unknown.", "small warning"));
      } else if (m.slots && typeof m.slots === "object") {
        for (const [kind, counts] of Object.entries(m.slots)) {
          const row = element("div", undefined, "factory-slot-row");
          const meter = element("meter");
          meter.min = 0;
          meter.max = Math.max(1, counts.capacity || 1);
          meter.value = counts.used || 0;
          row.append(element("span", kind, "mono"), meter, element("span", `${counts.used ?? 0}/${counts.capacity ?? 0}`, "mono"));
          card.append(row);
        }
      } else {
        card.append(element("p", `Sessions: ${m.sessions ?? "Not observed"}`, "small"));
      }
      if (canOperate()) {
        const btn = element("button", m.draining ? "Undrain" : "Drain…", "quiet");
        btn.type = "button";
        btn.addEventListener("click", async () => {
          const nextDrain = !m.draining;
          const choice = await confirmControl({
            title: `${nextDrain ? "Drain" : "Undrain"} machine ${m.machine}?`,
            verb: nextDrain ? "Drain machine" : "Undrain machine",
            dismiss: nextDrain ? "Keep active" : "Keep draining",
            danger: nextDrain,
            warning: nextDrain ? "Prevents new attempts from placing onto this machine." : "Re-enables placement on this machine.",
            facts: [["Machine", m.machine], ["Current state", statusText]],
            reason: { label: "Drain reason", placeholder: "Why placement is changing", required: nextDrain },
            cliCommand: `kiln-fleet machines ${nextDrain ? "drain" : "undrain"} ${m.machine}`,
            opener: btn,
          });
          if (!choice.confirmed) return;
          const res = await mutate(`/api/fleet/machines/${encodeURIComponent(m.machine)}/drain`, { draining: nextDrain, reason: choice.reason || undefined, request_id: choice.requestId }, `Machine ${m.machine} ${nextDrain ? "drained" : "undrained"}.`);
          if (res) void loadOverview();
        });
        card.append(btn);
      }
      list.append(card);
    }
    container.append(list);
    renderFactoryRibbon();
  }
  function renderFactoryClaims(result) {
    state.factory.claims = result;
    const container = $("factory-claims");
    if (!container) return;
    container.replaceChildren();
    const now = Date.now();
    const claims = Array.isArray(result?.claims) ? result.claims : [];
    const foreign = Array.isArray(result?.foreign_claims) ? result.foreign_claims : [];
    const expired = Array.isArray(result?.expired_claims) ? result.expired_claims : [];
    const expiring = claims.filter((c) => Number.isSafeInteger(c.expires_ms) && c.expires_ms - now <= LEASE_EXPIRING_MS);
    const healthy = claims.filter((c) => !(Number.isSafeInteger(c.expires_ms) && c.expires_ms - now <= LEASE_EXPIRING_MS));
    setSectionCount("factory-claims", `${claims.length}${foreign.length ? ` + ${foreign.length} foreign` : ""}`);

    const list = element("div", undefined, "factory-claim-list");
    for (const fc of foreign) {
      const card = element("article", undefined, "factory-claim-card");
      card.dataset.kind = "foreign";
      card.append(
        element("strong", `Foreign conflict · ${fc.scope_text || fc.author || "unknown"}`),
        element("p", `Holder: ${fc.holder || fc.author || "unknown"} · Mirror: ${fc.mirror_status || "foreign"}`, "small"),
      );
      list.append(card);
    }
    const renderClaimCard = (c, kind) => {
      const card = element("article", undefined, "factory-claim-card");
      card.dataset.kind = kind;
      const scopeLabel = typeof c.scope?.target === "string" ? `${c.scope.target}${c.scope.path && c.scope.path !== "whole" ? ` ${c.scope.path}` : ""}` : c.identity.slice(0, 12);
      const head = element("div", undefined, "factory-attention-head");
      head.append(element("strong", scopeLabel), when(c.expires_ms));
      const actions = element("div", undefined, "factory-attention-actions");
      const inspectBtn = element("button", "Inspect", "quiet");
      inspectBtn.type = "button";
      inspectBtn.addEventListener("click", () => openInspector("claim", c.identity, claims.map((x) => ({ kind: "claim", id: x.identity })), inspectBtn));
      actions.append(inspectBtn);
      if (canOperate() && kind !== "expired") {
        for (const mins of [30, 60]) {
          const renew = element("button", `+${mins}m`, "quiet");
          renew.type = "button";
          renew.addEventListener("click", async () => {
            const res = await mutate(`/api/fleet/claims/${c.identity}/renew`, { fence: c.fence, lease_minutes: mins }, `Claim renewed +${mins}m.`);
            if (res) void loadOverview();
          });
          actions.append(renew);
        }
        const release = element("button", "Force-Release…", "quiet danger");
        release.type = "button";
        release.addEventListener("click", async () => {
          const choice = await confirmControl({
            title: `Force-release claim ${c.identity.slice(0, 12)}?`,
            verb: "Force-release claim",
            dismiss: "Keep claim",
            danger: true,
            warning: "Releases this coordination claim for the exact fence shown.",
            facts: [["Claim", c.identity], ["Scope", scopeLabel], ["Fence", String(c.fence)]],
            reason: { label: "Release reason", placeholder: "Why this claim is being released", required: true },
            cliCommand: `kiln-fleet release --claim ${c.identity} --fence ${c.fence}`,
            opener: release,
          });
          if (!choice.confirmed) return;
          const res = await mutate(`/api/fleet/claims/${c.identity}/release`, { fence: c.fence, note: choice.reason, request_id: choice.requestId }, "Claim released.");
          if (res) void loadOverview();
        });
        actions.append(release);
      }
      card.append(head, element("p", `${c.holder?.tool || ""} on ${c.holder?.host || ""} · ${c.purpose || ""}`, "small"), actions);
      return card;
    };
    for (const c of expiring) list.append(renderClaimCard(c, "expiring"));
    for (const c of expired.slice(0, 5)) list.append(renderClaimCard(c, "expired"));
    if (healthy.length) {
      if (state.factory.showAllClaims) {
        for (const c of healthy) list.append(renderClaimCard(c, "healthy"));
      }
      const toggle = element("button", state.factory.showAllClaims ? "Collapse healthy claims" : `View All ${healthy.length} Healthy ->`, "quiet");
      toggle.type = "button";
      toggle.addEventListener("click", () => { state.factory.showAllClaims = !state.factory.showAllClaims; renderFactoryClaims(result); });
      list.append(toggle);
    }
    if (!foreign.length && !expiring.length && !expired.length && !healthy.length) {
      list.append(element("p", "No active or recently expired claims.", "small"));
    }
    container.append(list);
    updateGlobalLiveStrip();
    renderFactoryRibbon();
  }
  async function refreshFactoryFromSse(channel) {
    if (!state.session) return;
    if (channel === "attention" || state.route?.page === "inbox") {
      const att = await safeCall("/api/implementation/attention");
      if (att.ok) {
        state.factory.attention = att.value;
        if (state.route?.page === "overview") renderFactoryAttention(att.value);
        if (state.route?.page === "inbox" && activeSubPageFor(state.route) === "attention") {
          renderAttentionPage(att.value);
        }
        updateGlobalLiveStrip();
      }
    }
    if (activeSubPageFor(state.route) === "repositories" && (channel === "repositories" || channel === "fleet")) {
      void readAction(loadPage)();
    }
    if (state.route?.page === "overview") {
      if (channel === "fleet" || channel === "repositories") {
        const [machines, claims, dispatch, repos] = await Promise.all([
          safeCall("/api/fleet/machines"),
          safeCall("/api/fleet/claims?include=foreign,expired"),
          safeCall("/api/fleet/dispatch/status"),
          safeCall("/api/repositories"),
        ]);
        if (machines.ok) renderFactoryMachines(machines.value);
        if (claims.ok) renderFactoryClaims(claims.value);
        if (dispatch.ok) { state.factory.dispatch = dispatch.value; }
        if (repos.ok) state.factory.repositories = repos.value;
        renderFactoryRibbon();
      } else if (channel === "cairn") {
        const cairn = await safeCall("/api/implementation/cairn");
        if (cairn.ok) { state.factory.cairn = cairn.value; renderFactoryDeliverySpine(); }
      }
    }
  }
  async function loadOverview() {
    if (!$("overview-content")?.querySelector?.(".factory-cockpit")) buildOverview($("overview-content"));
    const [overviewRes, workersRes, activityRes, attentionRes, machinesRes, claimsRes, dispatchRes, cairnRes, attemptsRes, reviewRes, reposRes] = await Promise.all([
      safeCall("/api/overview"),
      safeCall("/api/workers"),
      safeCall(`/api/activity?${new URLSearchParams({ limit: String(OVERVIEW_ACTIVITY_LIMIT) })}`),
      safeCall("/api/implementation/attention"),
      safeCall("/api/fleet/machines"),
      safeCall("/api/fleet/claims?include=foreign,expired"),
      safeCall("/api/fleet/dispatch/status"),
      safeCall("/api/implementation/cairn"),
      safeCall("/api/fleet/attempts"),
      safeCall("/api/code-review"),
      safeCall("/api/repositories"),
    ]);
    if (state.route?.page !== "overview") return;
    if (overviewRes.ok && overviewRes.value?.totals) renderOverviewHealth(overviewRes.value);
    else if (overviewRes.ok) $("overview-health")?.replaceChildren(emptyState("Overview unavailable", "Project overview payload is incomplete."));
    else $("overview-health")?.replaceChildren(errorState(`${OVERVIEW_UNAVAILABLE.overview}: ${explain(overviewRes.error)}`));
    if (workersRes.ok && workersRes.value?.summary) renderOverviewWorkers(workersRes.value);
    else if (workersRes.ok) $("overview-workers")?.replaceChildren(emptyState("Workers unavailable", "Worker snapshot is unavailable."));
    else $("overview-workers")?.replaceChildren(errorState(`${OVERVIEW_UNAVAILABLE.workers}: ${explain(workersRes.error)}`));
    if (activityRes.ok && Array.isArray(activityRes.value?.events)) renderOverviewActivity(activityRes.value);
    else if (activityRes.ok) $("overview-activity")?.replaceChildren(emptyState("Activity unavailable", "Activity feed is unavailable."));
    else $("overview-activity")?.replaceChildren(errorState(`${OVERVIEW_UNAVAILABLE.activity}: ${explain(activityRes.error)}`));

    state.factory.dispatch = dispatchRes.ok ? dispatchRes.value : null;
    state.factory.cairn = cairnRes.ok ? cairnRes.value : null;
    state.factory.attempts = attemptsRes.ok ? (attemptsRes.value?.attempts || []) : [];
    state.factory.codeReview = reviewRes.ok ? reviewRes.value : null;
    state.factory.repositories = reposRes.ok ? reposRes.value : null;

    if (attentionRes.ok) renderFactoryAttention(attentionRes.value);
    else $("factory-attention")?.replaceChildren(errorState(`Attention inbox unavailable: ${explain(attentionRes.error)}`));

    if (machinesRes.ok) renderFactoryMachines(machinesRes.value);
    else $("factory-machines")?.replaceChildren(errorState(`Machine census unavailable: ${explain(machinesRes.error)}`));

    if (claimsRes.ok) renderFactoryClaims(claimsRes.value);
    else $("factory-claims")?.replaceChildren(errorState(`Claims unavailable: ${explain(claimsRes.error)}`));

    renderFactoryMergeTrains(state.factory.codeReview);
    renderFactoryDeliverySpine();
    renderFactoryRibbon();
  }
  function renderAttentionPage(result) {
    const root = $("inbox-attention-content");
    if (!root) return;
    state.factory.attention = result;
    const items = Array.isArray(result?.items) ? result.items : [];
    const header = element("div", undefined, "section-heading");
    header.append(element("h2", `Attention & Escalations (${items.length})`), element("p", "Unified human-required approvals, blocked plans, ambiguous terminations, Fleet/Campaign decisions, and expiring or foreign claims.", "small"));
    const listWrap = element("div", undefined, "view-stack");
    renderAttentionList(listWrap, items, { compactHealthy: false, emptyTitle: "No open escalations", emptyMsg: "Zero items currently require operator action." });
    root.replaceChildren(header, listWrap);
    updateGlobalLiveStrip();
  }
  async function loadAttentionPage() {
    const root = $("inbox-attention-content");
    if (!root) return;
    if (!root.childElementCount) root.replaceChildren(loadingState("Loading unified attention escalations…"));
    try {
      const [attention, claims] = await Promise.all([
        api("/api/implementation/attention"),
        safeCall("/api/fleet/claims?include=foreign,expired"),
      ]);
      if (claims.ok) state.factory.claims = claims.value;
      renderAttentionPage(attention);
    } catch (error) {
      if (!isAborted(error)) root.replaceChildren(errorState(`Attention inbox unavailable: ${explain(error)}`));
    }
  }
  function shortRevision(value) { return typeof value === "string" && value.length > 12 ? value.slice(0, 12) : text(value); }
  function finishedAt(ms) { return Number.isSafeInteger(ms) ? ms : -1; }
  // The moment that matters for a project's latest run: started while running, requested while queued or awaiting
  // approval, finished otherwise. Unrecorded instants read as not recorded.
  function runMoment(health, run) {
    const [label, ms, className] = health === "active" ? ["Started ", run.started_at_ms, "running-since"] : health === "queued" ? ["Requested ", run.created_at_ms, "meta"] : ["Finished ", run.completed_at_ms, "meta"];
    const node = element("span", label, className);
    node.append(when(ms));
    return node;
  }
  // r[impl kiln.web.project_page] Project names open the project's history page with a clean run selection.
  const CLEAR_RUN_SELECTION = Object.freeze({ run: "", node: "", stream: "", view: "", q: "", outcome: "", source: "", cursor: "", other: "", activity_cursor: "" });
  function projectLink(id, label, className) {
    const link = workspaceLink({ ...CLEAR_RUN_SELECTION, page: "project", project: id }, text(label), className);
    link.title = `Open the retained history of ${text(label)}`;
    return link;
  }
  // Pipeline/Logs keep their exact accessible names; Logs opens stderr of the exact latest execution for a failing project.
  function projectRunLinks(project, latest) {
    const history = { page: "runs", project: project.id, outcome: "", q: "", source: "", cursor: "", run: "", node: "" };
    const selection = { ...history, run: latest.id, stream: project.health === "failing" ? "stderr" : "stdout" };
    const pipeline = workspaceLink({ ...selection, view: "pipeline" }, "Pipeline");
    const logs = workspaceLink({ ...selection, view: "logs" }, "Logs");
    pipeline.setAttribute("aria-label", `Pipeline for ${text(project.label)}`);
    logs.setAttribute("aria-label", `Logs for ${text(project.label)}`);
    logs.title = `Open ${selection.stream} of this exact execution`;
    return { pipeline, logs };
  }
  function overviewCountChips(totals) {
    const chips = element("div", undefined, "count-chips");
    chips.setAttribute("role", "group");
    chips.setAttribute("aria-label", "Retained run counts");
    chips.append(element("span", "Retained runs", "eyebrow"));
    const outcomes = { runs: "", failed: "failed", active: "", queued: "queued", succeeded: "Success", cancelled: "Cancelled", unknown: "Unknown" };
    const labels = { runs: "Retained", failed: "Failed", active: "Active", queued: "Queued", succeeded: "Passed", cancelled: "Cancelled", unknown: "Unknown" };
    const statuses = { failed: "failure", active: "running", queued: "queued", unknown: "unknown", succeeded: "success", cancelled: "cancelled" };
    for (const key of ["failed", "active", "queued", "unknown", "succeeded", "cancelled", "runs"]) {
      if (!Object.hasOwn(totals, key)) continue;
      const patch = { page: "runs", outcome: outcomes[key], project: "", q: "", source: "", cursor: "", run: "", node: "" };
      if (key === "active") Object.assign(patch, { page: "workers", worker_status: "busy", worker_q: "", worker: "" });
      const link = workspaceLink(patch, undefined, "count-chip");
      link.dataset.outcome = key;
      if (statuses[key]) { link.dataset.status = statuses[key]; link.append(ui.statusIcon(statuses[key])); }
      if (key === "active" && Number(totals[key]) > 0) link.classList.add("is-live");
      if (key === "active") link.title = "Inspect execution targets with active runs";
      link.append(element("strong", text(totals[key]), "metric-value"), element("span", labels[key], "metric-label"));
      chips.append(link);
    }
    return chips;
  }
  // r[impl kiln.web.console_layout.scenario.attention] Pass rate states its denominator; unknown, cancelled and run-less projects are excluded and listed.
  function renderHealthHero(body, groups, total, totals) {
    body.replaceChildren();
    const failing = groups.failing.length;
    const passed = groups.healthy.length;
    const active = groups.active.length + groups.queued.length;
    const denominator = passed + failing + active;
    // r[impl kiln.web.wayfinding.scenario.orientation] An empty workspace names its next step: enroll a project, or read the Guide first.
    if (!total) {
      const next = element("div", undefined, "cluster");
      next.append(workspaceLink({ page: "projects" }, `Open ${ui.destination("projects").number} CI projects`, "button"), workspaceLink({ page: "guide" }, `Read ${ui.destination("guide").number} Guide`, "button"));
      body.append(emptyState("No projects configured", "No project health is recorded. Enroll an allowlisted repository in CI projects to start source-bound runs.", next));
    } else {
      const headline = element("p", undefined, "health-headline");
      headline.dataset.state = failing ? "failing" : "clear";
      headline.append(element("span", String(failing), "health-failing"), document.createTextNode(` failing of ${total} project${total === 1 ? "" : "s"}`));
      const summary = element("p", failing ? "Latest retained run per project. Failing projects are listed first below." : "No project's latest retained run failed.", "health-summary");
      const rate = element("div", undefined, "rate-block");
      const head = element("div", undefined, "rate-head");
      head.append(element("span", "Pass rate of latest project runs"), element("span", denominator ? `${Math.floor((passed / denominator) * 100)}%` : "Unavailable", "rate-value"));
      const bar = element("div", undefined, "rate-bar");
      bar.setAttribute("role", "img");
      const segments = [[passed, "success", "Passed"], [failing, "failure", "Failed"], [active, "running", "Active or queued"]];
      bar.setAttribute("aria-label", denominator ? `Latest project runs: ${segments.map(([count, , label]) => `${count} ${label.toLowerCase()}`).join(", ")}, of ${denominator}` : "No latest project run passed, failed or is in progress");
      const legend = element("ul", undefined, "rate-legend");
      for (const [count, status, label] of segments) {
        if (count) { const segment = element("span", undefined, "rate-seg"); segment.dataset.status = status; segment.style.flexGrow = String(count); segment.title = `${label}: ${count}`; bar.append(segment); }
        const item = element("li");
        const swatch = element("span", undefined, "rate-swatch"); swatch.dataset.status = status; swatch.setAttribute("aria-hidden", "true");
        item.append(swatch, document.createTextNode(`${label} `), element("strong", String(count)));
        legend.append(item);
      }
      const denominatorText = element("p", denominator ? `${passed} passed of ${denominator} latest project runs that passed, failed or are in progress.` : "No latest project run passed, failed or is in progress, so no pass rate is computed.", "small");
      const excluded = [[groups.unknown.length, "unknown"], [groups.cancelled.length, "cancelled"], [groups.unavailable.length, "without a retained run"]].filter(([count]) => count);
      const excludedText = element("p", excluded.length ? `Excluded from the rate: ${excluded.map(([count, label]) => `${count} ${label}`).join(", ")}.` : "No latest project run is excluded from the rate.", "rate-excluded");
      rate.append(head, bar, legend, denominatorText, excludedText);
      body.append(headline, summary, rate);
    }
    body.append(overviewCountChips(totals));
  }
  function attentionCard(project) {
    const latest = project.latest_run;
    const card = element("article", undefined, "attention-card");
    card.dataset.health = project.health;
    const { pipeline, logs } = projectRunLinks(project, latest);
    const head = element("div", undefined, "attention-head");
    const finished = element("span", "Finished ", "meta");
    finished.append(when(latest.completed_at_ms));
    head.append(ui.statusPill(latest.outcome, latest.phase), finished);
    const title = element("h3", undefined, "attention-title");
    title.append(projectLink(project.id, project.label));
    const meta = element("p", undefined, "attention-meta");
    const took = element("span", "Duration ");
    took.append(elapsed(latest.duration_ms));
    const revision = element("span", "Revision ");
    const exact = element("span", shortRevision(latest.source_revision), "mono");
    exact.title = text(latest.source_revision);
    revision.append(exact);
    meta.append(took, revision);
    const actions = element("div", undefined, "attention-actions");
    logs.prepend(ui.icon("triangle-alert"));
    actions.append(logs, pipeline);
    card.append(head, title, meta, actions);
    return card;
  }
  function projectRunTable(projects) {
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack project-table");
    const headings = element("tr");
    for (const [label, className] of [["Status"], ["Project"], ["Duration", "num"], ["Finished"], ["Inspect", "actions-heading"]]) headings.append(element("th", label, className));
    for (const cell of headings.children) cell.scope = "col";
    const head = element("thead"); head.append(headings);
    const body = element("tbody");
    table.append(head, body); wrap.append(table);
    for (const project of projects) {
      const row = element("tr", undefined, "project-row");
      row.dataset.health = project.health;
      const latest = project.latest_run;
      const cell = (label, content, className) => { const node = element("td", undefined, className); node.dataset.label = label; if (content) node.append(content); row.append(node); return node; };
      const name = element("span", undefined, "project-row-name");
      if (latest) {
        const { pipeline, logs } = projectRunLinks(project, latest);
        name.append(projectLink(project.id, project.label));
        cell("Status", livePill(latest.outcome, latest.phase), "project-row-status");
        cell("Project", name);
        cell("Duration", ui.runDurationElement(latest), "num project-row-duration");
        // r[impl kiln.web.live_progress] Running projects show when their recorded run started, and queued projects
        // when they were requested, instead of an absent finish.
        cell("Finished", runMoment(project.health, latest), "project-row-time");
        const actions = element("div", undefined, "project-row-actions");
        actions.append(logs, pipeline);
        cell("Inspect", actions, "project-row-actions-cell");
      } else {
        name.append(projectLink(project.id, project.label));
        cell("Status", statePill("unknown", "No retained runs"), "project-row-status");
        cell("Project", name);
        for (const label of ["Duration", "Finished", "Inspect"]) cell(label, element("span", "—", "meta"), label === "Duration" ? "num" : "");
      }
      body.append(row);
    }
    return wrap;
  }
  function renderDurationTrend(body, result) {
    body.replaceChildren(element("p", `${text(result.timing?.recorded)} / ${text(result.timing?.total)} retained runs have recorded timing. Historical unknowns are excluded, not treated as zero.`, "small"));
    const rows = Array.isArray(result.duration_trend) ? result.duration_trend : [];
    const max = Math.max(1, ...rows.map((row) => Number.isSafeInteger(row.duration_ms) ? row.duration_ms : 0));
    for (const row of rows) {
      const item = element("div", undefined, "trend-row");
      const heading = element("div", undefined, "trend-row-head");
      const meta = element("span", undefined, "meta");
      meta.append(elapsed(row.duration_ms), document.createTextNode(" · "), when(row.completed_at_ms));
      heading.append(ui.statusPill(row.outcome), runLink(row.run_id, text(row.project_id)), meta);
      item.append(heading);
      if (Number.isSafeInteger(row.duration_ms)) { const bar = element("meter"); bar.min = 0; bar.max = max; bar.value = row.duration_ms; bar.setAttribute("aria-label", `${row.run_id}: ${duration(row.duration_ms)}`); item.append(bar); }
      body.append(item);
    }
    if (!rows.length) body.append(element("p", "No recorded duration samples available.", "small"));
  }
  function renderOverviewHealth(result) {
    if (!result || typeof result.totals !== "object" || result.totals === null || !Array.isArray(result.projects)) throw new Error("The overview response is invalid.");
    const groups = { failing: [], healthy: [], active: [], queued: [], unknown: [], cancelled: [], unavailable: [] };
    for (const project of result.projects) {
      // A health claim without its latest run cannot be shown as an exact execution; it stays unresolved.
      const health = Object.hasOwn(groups, project.health) && (project.latest_run || project.health === "unavailable") ? project.health : "unknown";
      groups[health].push(project);
    }
    const newestFirst = (a, b) => finishedAt(b.latest_run?.completed_at_ms) - finishedAt(a.latest_run?.completed_at_ms);
    groups.failing.sort(newestFirst);
    groups.healthy.sort(newestFirst);
    renderHealthHero($("overview-health"), groups, result.projects.length, result.totals);
    const attention = $("overview-attention");
    attention.replaceChildren();
    setSectionCount("overview-attention", groups.failing.length ? String(groups.failing.length) : "");
    if (groups.failing.length) {
      const grid = element("div", undefined, "attention-grid");
      for (const project of groups.failing) grid.append(attentionCard(project));
      attention.append(grid);
    } else attention.append(emptyState("Nothing needs attention", result.projects.length ? "No project's latest retained run failed." : "No project runs are retained yet."));
    const passed = $("overview-passed");
    passed.replaceChildren();
    setSectionCount("overview-passed", groups.healthy.length ? String(groups.healthy.length) : "");
    if (groups.healthy.length) passed.append(projectRunTable(groups.healthy));
    else passed.append(emptyState("No passing projects", "No project's latest retained run passed."));
    const other = $("overview-other");
    // Running work first: the overview highlights in-progress projects before unresolved ones.
    const unresolved = [...groups.active, ...groups.queued, ...groups.unknown, ...groups.unavailable, ...groups.cancelled];
    other.replaceChildren();
    setSectionCount("overview-other", unresolved.length ? String(unresolved.length) : "");
    other.closest("section").hidden = !unresolved.length;
    if (unresolved.length) other.append(projectRunTable(unresolved));
    // r[impl kiln.web.live_progress] While work is running, its section sits directly below Needs attention.
    const otherSection = other.closest("section");
    const passedSection = passed.closest("section");
    const runningFirst = groups.active.length + groups.queued.length > 0;
    // Move only when the order changes, so live refreshes never disturb focus inside either section.
    if (runningFirst && otherSection.nextElementSibling !== passedSection) passedSection.before(otherSection);
    else if (!runningFirst && passedSection.nextElementSibling !== otherSection) passedSection.after(otherSection);
    renderDurationTrend($("overview-trend"), result);
  }
  function renderOverviewWorkers(result) {
    window.KilnWorkers.validate(result);
    const body = $("overview-workers");
    body.replaceChildren();
    setSectionCount("overview-workers", `${result.workers.length} target${result.workers.length === 1 ? "" : "s"}`);
    if (result.storage_failed) body.append(element("p", "Durable storage is failed. Mutating admission is unavailable; no healthy state is inferred.", "warning"));
    if (!result.workers.length) { body.append(emptyState("No targets configured", "No workflow targets are configured in this bounded snapshot. This is not a discovery of physical workers.")); return; }
    const metrics = element("dl", undefined, "side-metrics");
    for (const [label, value] of [["Free project slots", result.summary.free_slots], ["Active / occupied", result.summary.active], ["Configured slots", result.summary.configured_slots], ["Queued runs", result.summary.queued]]) {
      const metric = element("div", undefined, "side-metric");
      metric.append(element("dt", label), element("dd", text(value)));
      metrics.append(metric);
    }
    const counts = new Map();
    for (const worker of result.workers) counts.set(worker.status, (counts.get(worker.status) || 0) + 1);
    const statuses = element("div", undefined, "cluster");
    statuses.setAttribute("aria-label", "Targets by slot status");
    for (const [status, count] of counts) { const badge = window.KilnWorkers.statusBadge(status); badge.append(document.createTextNode(` · ${count}`)); statuses.append(badge); }
    body.append(metrics, statuses, element("p", "Configured targets and admission slots, not discovered machines. Physical liveness is unknown.", "small"));
  }
  // r[impl kiln.web.page_consistency] Readable activity rows: long identity segments are shortened in the title, the exact target stays secondary mono text.
  function compactTarget(target) { return typeof target === "string" ? target.split("/").map((part) => part.length > 22 ? shortIdentity(part) : part).join("/") : text(target); }
  function activityTitle(item) {
    if (item.kind === "audit") { const audit = item.audit || {}; return `${text(audit.method)} ${compactTarget(audit.target)} · ${text(audit.decision)}`; }
    const event = item.event;
    const name = typeof event === "string" ? event : event && typeof event === "object" ? Object.keys(event)[0] : "";
    return name ? name.replace(/([a-z])([A-Z])/g, (_, lower, upper) => `${lower} ${upper.toLowerCase()}`) : "Canonical event";
  }
  function activityExact(item) {
    const target = item.kind === "audit" ? item.audit?.target : null;
    if (typeof target !== "string" || compactTarget(target) === target) return null;
    const node = element("span", target, "mono meta activity-exact");
    node.title = "Exact audit target";
    return node;
  }
  function activityMeta(item) {
    const meta = element("span", undefined, "activity-feed-meta");
    meta.append(element("span", item.kind === "audit" ? "Audit decision" : "Canonical event"));
    if (item.project_id) meta.append(element("span", text(projectLabel(item.project_id))));
    if (item.kind === "audit" && item.audit?.actor_label) meta.append(element("span", text(item.audit.actor_label)));
    if (item.kind === "audit" && item.audit?.agent) { const agent = item.audit.agent; meta.append(element("span", `Agent ${text(agent.harness)}${agent.model ? ` · ${text(agent.model)}` : ""} (declared)`)); }
    meta.append(when(item.recorded_at_ms));
    if (item.run_id) { const link = runLink(item.run_id, `Run ${shortIdentity(item.run_id)}`, "activity"); link.title = item.run_id; meta.append(link); }
    return meta;
  }
  function renderOverviewActivity(result) {
    if (!Array.isArray(result?.items)) throw new Error("Activity response is invalid.");
    const body = $("overview-activity");
    body.replaceChildren();
    const items = result.items.slice(0, OVERVIEW_ACTIVITY_LIMIT);
    setSectionCount("overview-activity", `${items.length} of ${text(result.total)}`);
    if (!items.length) { body.append(emptyState("No recent activity", "No retained canonical events or audit decisions in this bounded timeline.")); return; }
    const list = element("ul", undefined, "activity-feed");
    for (const item of items) {
      const row = element("li");
      row.append(element("span", activityTitle(item), "activity-feed-title"));
      const exact = activityExact(item);
      if (exact) row.append(exact);
      row.append(activityMeta(item));
      list.append(row);
    }
    body.append(list);
  }
  // r[impl kiln.monorepo.console] A path scope as one line: root (or "Whole repository"), graph kinds and
  // any narrowing; `detailed` adds include/exclude and declared dependencies for trigger cards.
  function scopeLine(scope, detailed) {
    const line = element("span", undefined, "scope-line");
    line.append(ui.icon("folder"), element("span", scope.root, scope.whole ? undefined : "mono"));
    for (const graph of scope.graphs) line.append(element("span", `${graph} graph`, "chip"));
    if (detailed) {
      for (const [label, patterns] of [["Only", scope.include], ["Except", scope.exclude], ["Also when", scope.dependsOn]]) {
        if (!patterns.length) continue;
        const part = element("span", `${label} `, "scope-patterns");
        patterns.forEach((pattern, index) => { if (index) part.append(", "); part.append(element("code", pattern)); });
        if (label === "Also when") part.append(" changes");
        line.append(part);
      }
    }
    line.title = scope.scoped ? `Path scope root: ${scope.root}` : "Unscoped: every change runs this project";
    return line;
  }
  // r[impl kiln.monorepo.console] Projects sharing one repository sit under that repository's heading.
  function renderProjects() {
    const parent = $("project-cards"); parent.replaceChildren();
    for (const group of ui.groupProjects(state.projects.enrolled)) {
      let target = parent;
      if (group.shared) {
        const box = element("section", undefined, "project-repo-group");
        const heading = element("div", undefined, "project-repo-heading");
        const title = element("h3", undefined, "project-repo-title");
        title.append(ui.icon("git-branch"), element("span", group.label || "Shared repository", "mono"));
        heading.append(title, element("span", `${group.projects.length} projects share this repository`, "meta"));
        target = element("div", undefined, "project-repo-cards");
        box.append(heading, target);
        parent.append(box);
      }
      for (const project of group.projects) projectCard(project, target, group.shared ? "h4" : "h3");
    }
    if (!state.projects.enrolled.length) parent.append(emptyState("No enrolled projects", "Enroll an allowlisted repository below to start source-bound runs."));
  }
  function projectCard(project, parent, headingLevel) {
    const card = element("section", undefined, "project-card card");
    const heading = element(headingLevel, undefined, "project-card-title");
    heading.append(projectLink(project.id, project.label || project.id));
    card.append(heading);
    parent.append(card);
    card.dataset.projectId = project.id;
    const health = element("div", undefined, "project-card-health");
    health.append(element("span", "Reading latest run…", "meta"));
    card.append(health);
    card.append(element("p", text(project.reference), "meta mono"));
    const scope = ui.scopeSummary(project);
    if (scope) { const line = element("p", undefined, "meta project-scope"); line.append(scopeLine(scope, false)); card.append(line); }
    const actions = element("div", undefined, "project-actions");
    actions.append(workspaceLink({ page: "runs", project: project.id, q: "", outcome: "", source: "", cursor: "", run: "", node: "" }, "Browse runs"));
    const start = element("button", "Select for new run", "quiet");
    start.type = "button";
    start.disabled = !canOperate();
    start.addEventListener("click", () => { selectProjectForRun(project.id); void readAction(loadRefs)(); });
    actions.append(start); card.append(actions);
    const details = element("details");
    details.append(element("summary", "Repository & pipeline identity"));
    const facts = element("dl", undefined, "facts");
    const entries = [["ID", project.id], ["Repository", project.repository], ["Reference", project.reference], ["Pipeline", project.pipeline_identity]];
    if (scope?.scoped) {
      if (scope.include.length) entries.push(["Include", scope.include.join(", ")]);
      if (scope.exclude.length) entries.push(["Exclude", scope.exclude.join(", ")]);
      if (scope.dependsOn.length) entries.push(["Declared dependencies", scope.dependsOn.join(", ")]);
      if (scope.identity) entries.push(["Scope BLAKE3", scope.identity]);
    }
    addFacts(facts, entries);
    details.append(facts); card.append(details);
  }
  // "Select for new run" and the project page's Run latest only preselect the start form; the
  // operator still submits it, so dispatch keeps its existing explicit action and admission.
  function selectProjectForRun(projectId) {
    $("enrolled-project").value = projectId;
    projectDescriptions();
    $("enrolled-project").focus();
    $("start-form").scrollIntoView({ block: "center" });
  }
  function startRun(projectId) {
    state.startProject = typeof projectId === "string" ? projectId : "";
    navigate({ ...CLEAR_RUN_SELECTION, page: "projects", project: "" });
  }
  // Project cards show the latest retained run from the bounded overview read; its failure leaves inventory usable.
  async function loadProjectHealth() {
    const generation = state.pageGeneration;
    const slots = () => [...document.querySelectorAll("#project-cards .project-card")].map((card) => [card, card.querySelector(".project-card-health")]).filter(([, slot]) => slot);
    try {
      const result = await api("/api/overview");
      if (generation !== state.pageGeneration) return;
      if (!Array.isArray(result?.projects)) throw new Error("The overview response is invalid.");
      const byId = new Map(result.projects.map((project) => [project.id, project]));
      for (const [card, slot] of slots()) {
        const project = byId.get(card.dataset.projectId);
        slot.replaceChildren();
        card.querySelector(".project-actions [data-project-logs]")?.remove();
        if (!project) { slot.append(element("span", "Health not reported for this project in the bounded overview.", "meta")); continue; }
        card.dataset.health = project.health;
        const latest = project.latest_run;
        if (!latest) { slot.append(statePill("unknown", "No retained runs")); continue; }
        // r[impl kiln.web.page_consistency] Logs joins the card's shared action row instead of an inline 19px link.
        const { logs } = projectRunLinks(project, latest);
        logs.dataset.projectLogs = "";
        if (project.health === "failing") logs.prepend(ui.icon("triangle-alert"));
        card.querySelector(".project-actions")?.prepend(logs);
        slot.append(ui.statusPill(latest.outcome, latest.phase), runMoment(project.health, latest));
      }
    } catch (error) {
      if (isAborted(error) || generation !== state.pageGeneration) return;
      for (const [, slot] of slots()) slot.replaceChildren(element("span", `Health unavailable: ${explain(error)}`, "meta"));
    }
  }
  async function loadRefs() {
    const id = $("enrolled-project").value;
    const generation = ++state.refsGeneration;
    if (!id) { options($("source-reference"), [], "Select a project first", ""); $("source-status").textContent = "Choose an enrolled project to inspect its admitted source references."; return; }
    $("source-status").textContent = "Reading admitted source references…";
    try { const result = await api(`/api/projects/${encodeURIComponent(id)}/refs`); if (generation !== state.refsGeneration || id !== $("enrolled-project").value) return; options($("source-reference"), result.references.map((row) => ({ id: row.reference, label: `${row.reference} · ${text(row.source_revision)}` })), null, $("source-reference").value); $("source-status").textContent = "Latest resolves the admitted reference when the new run is dispatched; exact revision is reachability-checked by the service."; }
    catch (error) { if (!isAborted(error) && generation === state.refsGeneration) { options($("source-reference"), [], "References unavailable", ""); $("source-status").textContent = explain(error); } }
  }
  const workflowTones = { Pending: "pending", Running: "running", Paused: "paused", Completed: "completed", Failed: "failed", Cancelled: "cancelled", Skipped: "skipped", Unknown: "unknown" };
  const workflowStatuses = { Pending: "queued", Running: "running", Paused: "warning", Completed: "success", Failed: "failure", Cancelled: "cancelled", Skipped: "cancelled", Unknown: "unknown" };
  function workflowBadge(value) {
    const badge = statePill(Object.hasOwn(workflowStatuses, value) ? workflowStatuses[value] : "unknown", text(value));
    badge.classList.add("workflow-state");
    badge.dataset.state = Object.hasOwn(workflowTones, value) ? workflowTones[value] : "unknown";
    return badge;
  }
  // r[impl kiln.dashboard.workflow.presentation]
  function workflowTopology(nodes, edges) {
    const ids = new Set();
    for (const node of nodes) {
      if (!node || typeof node.id !== "string" || !node.id || ids.has(node.id)) return { problem: "Node identities are missing or duplicated. No dependency diagram or ambiguous node selection was substituted." };
      ids.add(node.id);
    }
    if (!Array.isArray(edges)) return { problem: "Dependency graph unavailable for this historical run. Node records do not imply independent stages." };
    const incoming = new Map(nodes.map((node) => [node.id, []]));
    const outgoing = new Map(nodes.map((node) => [node.id, []]));
    const seen = new Set();
    for (const edge of edges) {
      if (!edge || !ids.has(edge.source) || !ids.has(edge.target)) return { problem: "A dependency refers to an unavailable node. No partial dependency diagram was substituted." };
      const key = JSON.stringify([edge.source, edge.target]);
      if (seen.has(key)) return { problem: "Duplicate dependency edges were recorded. No approximate dependency diagram was substituted." };
      seen.add(key); incoming.get(edge.target).push(edge.source); outgoing.get(edge.source).push(edge.target);
    }
    const remaining = new Map(nodes.map((node) => [node.id, incoming.get(node.id).length]));
    const rank = new Map(nodes.map((node) => [node.id, 0]));
    const queue = nodes.filter((node) => !remaining.get(node.id)).map((node) => node.id);
    for (let index = 0; index < queue.length; index++) {
      const id = queue[index];
      for (const target of outgoing.get(id)) {
        rank.set(target, Math.max(rank.get(target), rank.get(id) + 1));
        remaining.set(target, remaining.get(target) - 1);
        if (!remaining.get(target)) queue.push(target);
      }
    }
    if (queue.length !== nodes.length) return { problem: "Cyclic dependencies were recorded. They cannot be presented as a workflow DAG; inspect the retained node records below." };
    const stages = [];
    for (const node of nodes) { const stage = rank.get(node.id); (stages[stage] ||= []).push(node); }
    return { stages, incoming, outgoing };
  }
  function updateGraphZoom(mode) {
    const parent = $("pipeline-graph"); const canvas = parent.querySelector(".dag-canvas");
    if (canvas) {
      if (mode === "fit") state.graphZoom = Math.min(1, (parent.clientWidth - 2) / canvas.offsetWidth);
      else if (mode === "actual") state.graphZoom = 1;
      else if (mode === "in") state.graphZoom += GRAPH_ZOOM_STEP;
      else if (mode === "out") state.graphZoom -= GRAPH_ZOOM_STEP;
      state.graphZoom = Math.max(GRAPH_MIN_ZOOM, Math.min(GRAPH_MAX_ZOOM, state.graphZoom));
      canvas.style.zoom = state.graphZoom;
    }
    $("graph-scale").textContent = `${Math.round(state.graphZoom * 100)}%`;
    for (const id of ["graph-fit", "graph-actual", "graph-in", "graph-out"]) $(id).disabled = !canvas;
    if (canvas) { $("graph-in").disabled = state.graphZoom >= GRAPH_MAX_ZOOM; $("graph-out").disabled = state.graphZoom <= GRAPH_MIN_ZOOM; }
  }
  function workflowNodeButton(node, location, relation) {
    const button = element("button", undefined, location === "graph" ? "dag-node" : "node-row");
    button.type = "button"; button.dataset.nodeId = node.id; button.dataset.workflowFocus = `${location}:${node.id}`;
    button.dataset.state = Object.hasOwn(workflowTones, node.state) ? workflowTones[node.state] : "unknown";
    button.dataset.relation = relation || "";
    const head = element("span", undefined, "workflow-node-head");
    head.append(element("span", text(node.kind), "workflow-kind"), workflowBadge(node.state));
    const timing = element("span", node.duration_ms == null ? "Duration not recorded" : "Recorded duration · ", "small");
    if (node.duration_ms != null) timing.append(elapsed(node.duration_ms));
    button.append(head, element("strong", node.id), timing);
    if (relation) button.append(element("span", relation === "before" ? "Prerequisite of selection" : "Depends on selection", "workflow-relation"));
    if (node.id === state.route.node) button.setAttribute("aria-current", "true");
    button.addEventListener("click", () => navigate({ node: node.id }));
    return button;
  }
  function drawWorkflowEdges(canvas, edges, buttons) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("width", canvas.offsetWidth); svg.setAttribute("height", canvas.offsetHeight); svg.setAttribute("aria-hidden", "true"); canvas.prepend(svg);
    for (const edge of edges) {
      const from = buttons.get(edge.source); const to = buttons.get(edge.target);
      const startX = from.offsetLeft + from.parentElement.offsetLeft + from.offsetWidth;
      const startY = from.offsetTop + from.parentElement.offsetTop + from.offsetHeight / 2;
      const endX = to.offsetLeft + to.parentElement.offsetLeft;
      const endY = to.offsetTop + to.parentElement.offsetTop + to.offsetHeight / 2;
      const mid = (startX + endX) / 2;
      const path = document.createElementNS(svg.namespaceURI, "path");
      path.setAttribute("d", `M${startX},${startY} C${mid},${startY} ${mid},${endY} ${endX},${endY}`);
      path.setAttribute("class", "dag-edge");
      const relation = edge.target === state.route.node ? "before" : edge.source === state.route.node ? "after" : "";
      path.dataset.relation = relation; svg.append(path);
      const arrow = document.createElementNS(svg.namespaceURI, "path");
      arrow.setAttribute("d", `M${endX - 6},${endY - 4} L${endX},${endY} L${endX - 6},${endY + 4}`);
      arrow.setAttribute("class", "dag-edge"); arrow.dataset.relation = relation; svg.append(arrow);
    }
  }
  function renderWorkflowNode(nodes, topology) {
    const parent = $("node-facts"); parent.replaceChildren();
    const matches = nodes.filter((node) => node?.id === state.route.node);
    if (matches.length !== 1) { parent.append(element("p", state.route.node ? `Node unavailable or ambiguous: ${state.route.node}. No other node substituted.` : "Select a node in the diagram or list to inspect its recorded state and relationships.", state.route.node ? "warning" : "small")); return; }
    const node = matches[0];
    const name = element("h4", node.id, "workflow-node-name"); name.tabIndex = -1; name.dataset.workflowFocus = `inspector:${node.id}`;
    parent.append(name, workflowBadge(node.state));
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [["Kind", node.kind], ["Started", when(node.started_at_ms)], ["Completed", when(node.completed_at_ms)], ["Duration", elapsed(node.duration_ms)]]); parent.append(facts);
    if (topology.problem) parent.append(element("p", "Dependency relationships are unavailable; no prerequisites or dependents are inferred.", "small"));
    else for (const [label, ids, location] of [["Prerequisites", topology.incoming.get(node.id), "before"], ["Dependents", topology.outgoing.get(node.id), "after"]]) {
      const group = element("div", undefined, "workflow-relations"); group.append(element("h4", label));
      if (!ids.length) group.append(element("p", "None in the admitted graph.", "small"));
      for (const id of ids) {
        const button = element("button", id, "workflow-node-link"); button.type = "button"; button.dataset.workflowFocus = `${location}:${id}`;
        button.addEventListener("click", () => navigate({ node: id })); group.append(button);
      }
      parent.append(group);
    }
    const actions = element("div", undefined, "workflow-node-actions");
    const logs = element("button", "Inspect node logs"); logs.type = "button"; logs.dataset.workflowFocus = `logs:${node.id}`;
    logs.addEventListener("click", () => navigate({ view: "logs", node: node.id }));
    const clear = element("button", "Clear node selection"); clear.type = "button"; clear.dataset.workflowFocus = "clear"; clear.addEventListener("click", () => navigate({ node: "" }));
    actions.append(logs, clear); parent.append(actions);
  }
  // r[impl kiln.dashboard.workflow.presentation]
  // r[impl kiln.dashboard.complete.pipeline]
  function renderGraph(run) {
    const focused = document.activeElement?.dataset.workflowFocus;
    const parent = $("pipeline-graph"); const list = $("node-list");
    const sameRun = state.graphRun === run.id;
    const scroll = sameRun ? [parent.scrollLeft, parent.scrollTop, list.scrollTop] : [0, 0, 0];
    if (!sameRun) { state.graphRun = run.id; state.graphZoom = 1; }
    parent.replaceChildren(); list.replaceChildren();
    const nodes = Array.isArray(run.nodes) ? run.nodes : [];
    const topology = workflowTopology(nodes, run.edges);
    $("workflow-title").textContent = run.execution?.provider_namespace === "lattice" ? "Lattice workflow" : "Workflow graph";
    $("workflow-caption").textContent = `${nodes.length} admitted node(s) · ${topology.problem ? "dependency topology unavailable" : `${run.edges.length} dependency edge(s) · ${topology.stages.length} stage(s)`}`;
    const summary = $("workflow-summary"); summary.replaceChildren();
    const counts = new Map(); for (const node of nodes) { const key = text(node?.state); counts.set(key, (counts.get(key) || 0) + 1); }
    for (const [value, count] of counts) { const item = workflowBadge(value); item.append(document.createTextNode(` · ${count}`)); summary.append(item); }
    const before = new Set(topology.problem ? [] : topology.incoming.get(state.route.node) || []);
    const after = new Set(topology.problem ? [] : topology.outgoing.get(state.route.node) || []);
    const relation = (id) => before.has(id) ? "before" : after.has(id) ? "after" : "";
    if (topology.problem) parent.append(element("p", topology.problem, "warning"));
    else if (nodes.length) {
      const canvas = element("div", undefined, "dag-canvas"); parent.append(canvas);
      const buttons = new Map();
      topology.stages.forEach((nodes, index) => {
        const column = element("section", undefined, "dag-stage");
        column.append(element("h3", `Stage ${index + 1}`), element("p", `${nodes.length} node(s) · dependency depth ${index}`, "small"));
        for (const node of nodes) { const button = workflowNodeButton(node, "graph", relation(node.id)); buttons.set(node.id, button); column.append(button); }
        canvas.append(column);
      });
      drawWorkflowEdges(canvas, run.edges, buttons);
    } else parent.append(element("p", "No workflow nodes admitted.", "small"));
    if (!nodes.length) list.append(element("p", "No workflow nodes admitted.", "small"));
    const identities = new Map(); for (const node of nodes) if (node?.id) identities.set(node.id, (identities.get(node.id) || 0) + 1);
    for (const node of nodes) {
      if (!node || typeof node.id !== "string" || !node.id) { list.append(element("p", "A retained node record has no usable identity.", "warning")); continue; }
      const button = workflowNodeButton(node, "list", relation(node.id)); button.disabled = identities.get(node.id) !== 1; list.append(button);
    }
    renderWorkflowNode(nodes, topology); updateGraphZoom();
    parent.scrollLeft = scroll[0]; parent.scrollTop = scroll[1]; list.scrollTop = scroll[2];
    if (focused) {
      let target = [...document.querySelectorAll(".workflow-panel [data-workflow-focus]")].find((node) => node.dataset.workflowFocus === focused);
      if (!target && /^(before|after):/.test(focused)) target = $("node-facts").querySelector(".workflow-node-name");
      if (!target && focused === "clear") { target = $("workflow-node-title"); target.tabIndex = -1; }
      target?.focus({ preventScroll: true });
    }
  }
  for (const mode of ["fit", "actual", "in", "out"]) $("graph-" + mode).addEventListener("click", () => updateGraphZoom(mode));
  // r[impl kiln.dashboard.complete.reruns]
  async function prepareRerun(mode) {
    if (!canOperate()) return;
    const id = state.selected;
    try { const run = await loadDetail(); if (!run || id !== state.selected) return; if (mode === "failed" && !run.nodes?.some((node) => /^(failed|error)$/i.test(node.state))) throw new Error("No known failed node set is available. Unknown states cannot be retried blindly."); state.confirm = { id, mode }; const subject = `"${text(projectLabel(run.project_id))}"`; confirmWording(...(mode === "latest" ? [`Run ${subject} on the latest admitted reference?`, "Run latest"] : mode === "failed" ? [`Retry failed nodes of ${subject}?`, "Retry failed nodes"] : [`Rerun ${subject} at the exact revision?`, "Rerun"]), "Cancel", false); addFacts($("confirm-facts"), [["Parent run", id], ["Project", run.project_id], ["Mode", mode], ["Source", mode === "latest" ? "Latest admitted reference (resolved by service)" : run.source_revision], ["Failed selection", mode === "failed" ? run.nodes.filter((node) => /^(failed|error)$/i.test(node.state)).map((node) => node.id) : "Not a node retry"], ["Boundary", "New candidate and attempt; queued durably if project is busy."]]); $("control-dialog").showModal(); $("dismiss-control").focus(); }
    catch (error) { if (!isAborted(error)) notice(explain(error)); }
  }
  // r[impl kiln.dashboard.complete.reports]
  function renderFlaky(parent, rows) {
    const group = section("Repeated-test instability", parent, "view-stack");
    // r[impl kiln.web.page_consistency.scenario.states] Explicit empty state; the detection boundary stays one disclosure away.
    if (!rows.length) group.append(element("p", "No repeated pass/fail instability is recorded in the bounded history. This does not prove all tests are stable.", "small"));
    group.append(aboutView("About instability detection", "Instability requires actual repeated pass/fail observations for the same exact source revision, pipeline, node, report and test identity. It is not inferred from code changes, execution logs or missing reports."));
    for (const row of rows) {
      const card = section(`${text(row.suite)} / ${text(row.class_name)} / ${text(row.name)}`, group, "record-card");
      const facts = element("dl", undefined, "facts"); addFacts(facts, [["Exact source revision", row.source_revision], ["Node", row.node_id], ["Report", row.report_name], ["Passed observations", row.passed], ["Failed observations", row.failed], ["Total observations", row.observations]]); card.append(facts);
      const links = element("div", undefined, "control-bar"); for (const id of row.run_ids || []) links.append(runLink(id, id, "tests")); card.append(links);
    }
  }
  function renderComparison(parent, result) {
    parent.append(element("p", "Only admitted recorded facts are compared. Missing tests, coverage and timing stay unavailable. A changed test verdict is not acceptance authority.", "boundary"));
    const facts = element("dl", undefined, "facts");
    const delta = result.duration_ms?.delta;
    let deltaNode = null;
    if (Number.isSafeInteger(delta)) {
      deltaNode = element("span", `${delta > 0 ? "+" : delta < 0 ? "−" : "±"}${ui.formatDuration(Math.abs(delta))}`, "duration");
      deltaNode.title = `${delta.toLocaleString("en-US")} ms`;
      deltaNode.append(element("span", ` (${delta.toLocaleString("en-US")} ms)`, "sr-only"));
    }
    addFacts(facts, [["Current source", result.source?.current], ["Other source", result.source?.other], ["Current outcome", ui.statusPill(result.outcome?.current)], ["Other outcome", ui.statusPill(result.outcome?.other)], ["Current duration", elapsed(result.duration_ms?.current)], ["Other duration", elapsed(result.duration_ms?.other)], ["Duration delta (current − other)", deltaNode]]);
    parent.append(facts, runLink(result.run_id, "Inspect current run"), runLink(result.other_run_id, "Inspect compared run"));
    for (const key of ["current", "other"]) { const group = section(`${key === "current" ? "Current" : "Other"} structured tests & coverage`, parent, "record-card"); renderRecorded(group, result.reports?.[key]); }
    const changes = section("Recorded test changes", parent, "view-stack");
    changes.append(element("p", `Bounded comparison: ${text(result.test_changes_bounded)}. Tests absent on one side are unavailable, not passing.`, "small"));
    if (!result.test_changes?.length) changes.append(element("p", "No test-status differences recorded in this comparison window.", "small"));
    for (const row of result.test_changes || []) { const card = section(`${text(row.suite)} / ${text(row.class_name)} / ${text(row.name)}`, changes, "record-card"); const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["Node", row.node_id], ["Report", row.report_name], ["Current status", row.current], ["Other status", row.other]]); card.append(metadata); }
    renderFlaky(parent, result.flaky_history || []);
  }
  // r[impl kiln.interchange.provenance] r[impl kiln.interchange.builder] r[impl kiln.interchange.statement.scenario.evidence]
  function isInTotoStatement(value) {
    return Boolean(value) && typeof value === "object" && value._type === "https://in-toto.io/Statement/v1" && typeof value.predicateType === "string";
  }
  // An unsigned identity projection is evidence; it is not a builder, signer, build-level or gate claim.
  function renderInTotoStatement(parent, statement) {
    parent.append(element("p", "Unsigned in-toto statement retained as evidence. Schema validity, product digests and any later signature do not establish builder identity, signer trust, a SLSA build level, gate acceptance or release authority.", "boundary"));
    const predicate = statement.predicate || {};
    const definition = predicate.buildDefinition || {};
    const run = predicate.runDetails || {};
    const external = definition.externalParameters || {};
    const internal = definition.internalParameters || {};
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [
      ["Statement type", statement._type],
      ["Predicate type", statement.predicateType],
      ["Build type", definition.buildType],
      ["Builder identity", run.builder?.id],
      ["Invocation (producing attempt)", run.metadata?.invocationId],
      ["Kiln control plane", external.kilnControlPlane],
      ["Interchange profile", external.interchangeProfile],
      ["Pipeline revision", external.pipelineRevision],
      ["Source or candidate", external.sourceOrCandidate],
      ["Provider", internal.provider],
      ["Worker", internal.worker]
    ]);
    parent.append(facts);
    const subjects = Array.isArray(statement.subject) ? statement.subject : [];
    const subjectGroup = section(`Subjects · ${subjects.length}`, parent, "record-card");
    if (!subjects.length) subjectGroup.append(element("p", "No subject product is recorded.", "warning"));
    else {
      const table = element("table", undefined, "review-table");
      const head = element("tr");
      for (const heading of ["Product", "Digest"]) head.append(element("th", heading));
      const thead = element("thead");
      thead.append(head);
      table.append(thead);
      const body = element("tbody");
      for (const subject of subjects) {
        const row = element("tr");
        const digests = subject?.digest && typeof subject.digest === "object" ? Object.entries(subject.digest) : [];
        row.append(element("td", text(subject?.name), "mono"), element("td", digests.map(([algorithm, value]) => `${text(algorithm)}:${text(value)}`).join(" · ") || "Unavailable", "mono"));
        body.append(row);
      }
      table.append(body);
      subjectGroup.append(table);
    }
    const dependencies = Array.isArray(definition.resolvedDependencies) ? definition.resolvedDependencies : [];
    const dependencyGroup = section(`Resolved dependencies · ${dependencies.length}`, parent, "record-card");
    if (!dependencies.length) dependencyGroup.append(element("p", "No resolved dependency is recorded. Missing dependencies are unavailable, not empty.", "small"));
    else {
      const table = element("table", undefined, "review-table");
      const head = element("tr");
      for (const heading of ["Dependency", "Digest"]) head.append(element("th", heading));
      const thead = element("thead");
      thead.append(head);
      table.append(thead);
      const body = element("tbody");
      for (const dependency of dependencies) {
        const row = element("tr");
        const digests = dependency?.digest && typeof dependency.digest === "object" ? Object.entries(dependency.digest) : [];
        row.append(element("td", text(dependency?.uri ?? dependency?.name), "mono"), element("td", digests.map(([algorithm, value]) => `${text(algorithm)}:${text(value)}`).join(" · ") || "Unavailable", "mono"));
        body.append(row);
      }
      table.append(body);
      dependencyGroup.append(table);
    }
    const details = element("details");
    details.append(element("summary", "Retained canonical statement"));
    details.append(element("pre", JSON.stringify(statement), "identity"));
    parent.append(details);
  }
  function renderReports(parent, result) {
    const records = result.reports;
    const runId = result.run_id || state.route.run;
    if (result.summary) { const summary = section("Recorded report summary", parent, "record-card"); renderRecorded(summary, result.summary); }
    if (Array.isArray(result.flaky_history)) renderFlaky(parent, result.flaky_history);
    if (!Array.isArray(records)) throw new Error("Structured report response unavailable.");
    parent.append(element("p", "Reported tests and coverage are recorded evidence, not execution-success or accepted-gate authority. Missing reports are unavailable, not zero failures or full coverage.", "boundary"));
    if (!records.length) parent.append(emptyState("No declared reports", "No declared reports retained for this run. Structured JUnit, Cobertura or LCOV results appear here once a node retains a declared report."));
    for (const record of records) {
      const report = record.report || record;
      const card = section(`${text(record.name)} · ${text(report.kind)}`, parent, "record-card");
      const facts = element("dl", undefined, "facts"); addFacts(facts, [["Node", record.node_id], ["Attempt", record.attempt_id], ["Source revision", record.source_revision], ["Source BLAKE3", report.source_blake3], ["Source bytes", report.source_bytes], ["Report identity", record.id], ["Recorded", when(record.recorded_at_ms)]]); card.append(facts);
      if (record.unavailable) card.append(element("p", `Report unavailable: ${text(record.unavailable)}. Missing normalized data is not a passing report.`, "warning"));
      if (record.artifact) { const button = element("button", "Download retained report source"); button.type = "button"; button.addEventListener("click", () => downloadArtifact(runId, record.artifact, button)); card.append(button); }
      if (report.tests) {
        const tests = report.tests;
        const summary = element("dl", undefined, "facts"); addFacts(summary, [["Total", tests.total], ["Passed", tests.passed], ["Failed", tests.failed], ["Errors", tests.errors], ["Skipped", tests.skipped], ["Reported duration", reportedSeconds(tests.duration_seconds)]]); card.append(summary);
        for (const test of tests.cases || []) {
          const details = element("details", undefined, "test-case");
          const heading = element("summary");
          heading.append(testStatusPill(test.status), document.createTextNode(` ${text(test.suite)} / ${text(test.class_name)} / ${text(test.name)}`));
          details.append(heading);
          if (test.status === "failed" || test.status === "error") details.open = true;
          const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["File", test.file], ["Line", test.line], ["Reported duration", reportedSeconds(test.duration_seconds)], ["Message", test.message]]); details.append(metadata);
          if (test.details) details.append(element("pre", test.details, "identity"));
          card.append(details);
        }
      }
      if (report.coverage) {
        const coverage = report.coverage;
        const summary = element("dl", undefined, "facts"); addFacts(summary, [["Format", coverage.format], ["Lines hit / found", coverage.lines_hit === null || coverage.lines_found === null ? null : `${coverage.lines_hit} / ${coverage.lines_found}`], ["Line rate", coverage.line_rate === null ? null : `${(coverage.line_rate * 100).toFixed(2)}%`], ["Branches hit / found", coverage.branches_hit === null || coverage.branches_found === null ? null : `${coverage.branches_hit} / ${coverage.branches_found}`], ["Branch rate", coverage.branch_rate === null ? null : `${(coverage.branch_rate * 100).toFixed(2)}%`]]); card.append(summary);
        for (const file of coverage.files || []) { const details = element("details"); details.append(element("summary", text(file.path))); renderRecorded(details, file); card.append(details); }
      }
      if (report.evidence) {
        if (report.kind === "provenance" && isInTotoStatement(report.evidence.document)) renderInTotoStatement(card, report.evidence.document);
        else renderEvidence(card, report.evidence);
      }
    }
  }
  // ---------- Cross-Node Failure Digest (r[impl kiln.web.causal_timeline_and_failure_digest]) ----------
  const MAX_DIGEST_PROBES = 8;
  const DIGEST_PROBE_BYTES = 4096;
  let failureDigestBox = null;

  function normalizeErrorSignature(line) {
    if (typeof line !== "string") return "";
    return line
      .trim()
      .replace(/\b0x[0-9a-f]+\b/gi, "<addr>")
      .replace(/:\d+(?::\d+)?\b/g, ":<line>")
      .replace(/\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?\b/g, "<time>")
      .slice(0, 180);
  }

  function buildFailureDigest(run, observations = new Map()) {
    if (!run || typeof run !== "object") return { failedNodeCount: 0, groups: [] };
    const nodes = Array.isArray(run.nodes) ? run.nodes : [];
    const telemetryByNode = new Map();
    for (const rec of Array.isArray(run.step_telemetry) ? run.step_telemetry : []) {
      const obs = rec?.observation;
      const nodeId = obs?.binding?.node_id;
      if (typeof nodeId === "string" && nodeId && !telemetryByNode.has(nodeId)) telemetryByNode.set(nodeId, obs.step || {});
    }
    const failing = nodes.filter((node) => node && typeof node.id === "string" && FAILED_NODE_STATE.test(node.state));
    if (!failing.length) return { failedNodeCount: 0, groups: [] };
    const groupsMap = new Map();
    for (const node of failing) {
      const obs = observations instanceof Map ? observations.get(node.id) : null;
      const step = telemetryByNode.get(node.id) || null;
      const stepExit = Number.isSafeInteger(step?.exit_code) && step.exit_code !== 0 ? `exit code ${step.exit_code}` : "";
      const rawSig = (obs?.firstErrorLine && obs.firstErrorLine.trim())
        || (typeof node.error === "string" && node.error.trim())
        || (typeof node.message === "string" && node.message.trim())
        || (typeof step?.error === "string" && step.error.trim())
        || (typeof step?.message === "string" && step.message.trim())
        || stepExit
        || (typeof run.error_code === "string" && run.error_code.trim())
        || `${text(node.kind)} · ${text(node.state)}`;
      const signature = normalizeErrorSignature(rawSig) || `${text(node.kind)} · ${text(node.state)}`;
      const stream = obs?.stream || "stderr";
      let group = groupsMap.get(signature);
      if (!group) {
        group = { signature, sampleLine: obs?.firstErrorLine || rawSig, stream, count: 0, nodes: [] };
        groupsMap.set(signature, group);
      }
      group.count += 1;
      group.nodes.push({
        nodeId: node.id,
        kind: node.kind || "task",
        state: node.state || "Failed",
        stream,
        exitCode: Number.isSafeInteger(step?.exit_code) ? step.exit_code : null,
        firstErrorLine: obs?.firstErrorLine || "",
        lineIndex: Number.isSafeInteger(obs?.lineIndex) ? obs.lineIndex : null,
      });
    }
    const groups = [...groupsMap.values()].sort((a, b) => b.count - a.count || a.signature.localeCompare(b.signature));
    return { failedNodeCount: failing.length, groups };
  }

  function ensureFailureDigestContainer() {
    if (!failureDigestBox) {
      failureDigestBox = element("section", undefined, "log-failure-digest");
      failureDigestBox.id = "log-failure-digest";
      failureDigestBox.setAttribute("aria-label", "Cross-node failure digest");
      failureDigestBox.hidden = true;
    }
    const form = $("log-form");
    if (form?.parentNode && failureDigestBox.parentNode !== form.parentNode) {
      form.parentNode.insertBefore(failureDigestBox, form);
    }
    return failureDigestBox;
  }

  function jumpToDigestNode(nodeId, stream = "stderr") {
    if (!nodeId || !state.selected) return;
    const nodeSelect = $("log-node");
    const streamSelect = $("log-stream");
    const nextStream = ["stdout", "stderr", "diagnostic"].includes(stream) ? stream : "stderr";
    state.digestJump = { run: state.selected, node: nodeId, stream: nextStream };
    if (nodeSelect) nodeSelect.value = nodeId;
    if (streamSelect) streamSelect.value = nextStream;
    resetLogs();
    const needsNav = state.route?.view !== "logs" || state.route?.node !== nodeId || state.route?.stream !== nextStream;
    if (needsNav) navigate({ node: nodeId, stream: nextStream, view: "logs" });
    if (!state.logBusy && nodeSelect?.value === nodeId) void readLog(false);
  }

  function renderFailureDigest(run) {
    const box = ensureFailureDigestContainer();
    box.replaceChildren();
    if (!run || typeof run !== "object") { box.hidden = true; return; }
    const observations = state.logDigestCache.get(run.id) || new Map();
    const digest = buildFailureDigest(run, observations);
    if (!digest.failedNodeCount) { box.hidden = true; return; }
    box.hidden = false;
    const head = element("div", undefined, "log-digest-head");
    head.append(
      element("h3", "Cross-Node Failure Digest", "log-digest-title"),
      element("span", `${digest.failedNodeCount} failed node${digest.failedNodeCount === 1 ? "" : "s"} · ${digest.groups.length} unique signature${digest.groups.length === 1 ? "" : "s"}`, "chip"),
    );
    box.append(head);
    const list = element("ul", undefined, "log-digest-list");
    for (const group of digest.groups) {
      const item = element("li", undefined, "log-digest-group");
      const sigRow = element("div", undefined, "log-digest-sig-row");
      const sig = element("code", group.signature, "log-digest-sig");
      sigRow.append(sig);
      if (group.count > 1) sigRow.append(element("span", `${group.count} nodes`, "state-badge"));
      item.append(sigRow);
      const actions = element("div", undefined, "log-digest-actions");
      for (const n of group.nodes) {
        const btn = element("button", `${n.nodeId} (${n.stream})`, "quiet log-digest-jump");
        btn.type = "button";
        btn.dataset.nodeId = n.nodeId;
        btn.dataset.stream = n.stream;
        btn.title = n.firstErrorLine ? `${n.nodeId} · ${n.stream}: ${n.firstErrorLine}` : `Jump to ${n.nodeId} ${n.stream}`;
        btn.addEventListener("click", () => jumpToDigestNode(n.nodeId, n.stream));
        actions.append(btn);
      }
      item.append(actions);
      list.append(item);
    }
    box.append(list);
  }

  async function probeFailureDigestLogs(run) {
    if (!run || !state.session || state.logDigestProbed.has(run.id)) return;
    const maximum = run.log_bounds?.maximum_chunk_bytes;
    if (!Number.isSafeInteger(maximum) || maximum <= 0) return;
    const nodes = Array.isArray(run.nodes) ? run.nodes.filter((n) => n && typeof n.id === "string" && FAILED_NODE_STATE.test(n.state)) : [];
    if (!nodes.length) return;
    state.logDigestProbed.add(run.id);
    const runId = run.id;
    const probeBytes = Math.min(DIGEST_PROBE_BYTES, maximum);
    let runCache = state.logDigestCache.get(runId);
    if (!runCache) { runCache = new Map(); state.logDigestCache.set(runId, runCache); }
    for (const node of nodes.slice(0, MAX_DIGEST_PROBES)) {
      if (runCache.has(node.id) || state.selected !== runId || !state.session) continue;
      for (const stream of ["stderr", "stdout"]) {
        try {
          const res = await api(`${pathFor(runId)}/logs?${new URLSearchParams({ node_id: node.id, stream, cursor: "0", max_bytes: String(probeBytes) })}`, { quiet: true });
          if (state.selected !== runId || !state.session) return;
          if (typeof res?.text === "string" && res.text.trim()) {
            const lines = res.text.split("\n");
            const idxs = ui.errorLines(res.text);
            const firstIdx = idxs.length ? idxs[0] : -1;
            const firstErrorLine = firstIdx >= 0 ? lines[firstIdx] : (lines.findLast((l) => l.trim()) || "");
            runCache.set(node.id, { stream, firstErrorLine, lineIndex: firstIdx, errorCount: idxs.length });
            if (state.run?.id === runId) renderFailureDigest(state.run);
            break;
          }
        } catch {
          // Quiet probe: unavailable logs leave the node-level digest intact.
          break;
        }
      }
    }
  }

  // ---------- Causal Event-Sourced Decision Timeline (r[impl kiln.web.causal_timeline_and_failure_digest]) ----------
  const CAUSAL_STAGES = Object.freeze([
    Object.freeze({ key: "admitted", label: "Admitted", tone: "queued", order: 0 }),
    Object.freeze({ key: "untrusted_gate", label: "Untrusted Gate", tone: "warning", order: 1 }),
    Object.freeze({ key: "dispatched", label: "Dispatched", tone: "running", order: 2 }),
    Object.freeze({ key: "observations", label: "Observations", tone: "info", order: 3 }),
    Object.freeze({ key: "controls_unknown", label: "Controls / Unknown", tone: "warning", order: 4 }),
    Object.freeze({ key: "terminal", label: "Terminal", tone: "success", order: 5 }),
  ]);
  const CAUSAL_STAGE_BY_KEY = new Map(CAUSAL_STAGES.map((s) => [s.key, s]));

  function copyableIdentityChip(value, labelPrefix = "event identity") {
    const id = typeof value === "string" ? value.trim() : "";
    if (!id) return null;
    const wrap = element("span", undefined, "causal-identity");
    const prefix = element("code", id.length > 12 ? id.slice(0, 12) : id, "causal-identity-prefix");
    prefix.title = id;
    prefix.dataset.fullIdentity = id;
    const status = element("span", undefined, "small causal-copy-status");
    status.setAttribute("role", "status");
    const btn = element("button", undefined, "quiet causal-copy-btn");
    btn.type = "button";
    btn.append(ui.icon("copy"), element("span", "Copy"));
    btn.setAttribute("aria-label", `Copy ${labelPrefix} ${id.slice(0, 12)}`);
    btn.addEventListener("click", () => ui.copyText(id, status, "Copied."));
    wrap.append(prefix);
    if (id.length > 12) wrap.append(element("span", "…", "small"));
    wrap.append(btn, status);
    return wrap;
  }

  function classifyCausalEvent(item) {
    if (!item || typeof item !== "object") {
      return { stage: CAUSAL_STAGE_BY_KEY.get("observations"), title: "Recorded item", attribution: "Kiln", identity: "", facts: [], outcome: null };
    }
    if (item.kind === "audit") {
      const audit = item.audit || {};
      const target = String(audit.target || "");
      const method = String(audit.method || "");
      let stageKey = "controls_unknown";
      if (/\/approve\b|untrusted/i.test(target)) stageKey = "untrusted_gate";
      else if (/\/runs\b|\/rerun\b/i.test(target) && method === "POST") stageKey = "admitted";
      else if (/\/reconcile\b|\/control\b|\/cancel\b|\/pause\b|\/resume\b/i.test(target)) stageKey = "controls_unknown";
      const actor = audit.actor_label || audit.principal_id || "Operator";
      const agent = audit.agent ? ` · Agent ${audit.agent.harness}${audit.agent.model ? ` / ${audit.agent.model}` : ""} (declared)` : "";
      const facts = [
        ["Method", method],
        ["Target", target],
        ["Decision", audit.decision],
        ...(item.run_id ? [["Run", runLink(item.run_id)]] : []),
      ];
      return {
        stage: CAUSAL_STAGE_BY_KEY.get(stageKey),
        title: activityTitle(item),
        attribution: `${actor}${agent}`,
        identity: typeof audit.identity === "string" ? audit.identity : typeof audit.request_id === "string" ? audit.request_id : "",
        facts,
        outcome: null,
      };
    }
    const event = item.event;
    const kind = typeof event === "string" ? event : event && typeof event === "object" ? Object.keys(event)[0] || "" : "";
    const payload = kind && event && typeof event === "object" ? event[kind] || {} : {};
    let stageKey = "observations";
    if (kind === "RunStarted") stageKey = "admitted";
    else if (kind === "RetryStarted") stageKey = "dispatched";
    else if (kind === "AttemptUnknown" || kind === "CancellationRequested" || kind === "HoldChanged") stageKey = "controls_unknown";
    else if (kind === "AttemptCompleted" || kind === "AttemptReconciled") stageKey = payload.outcome === "Unknown" ? "controls_unknown" : "terminal";
    else if (item.stage && CAUSAL_STAGE_BY_KEY.has(item.stage)) stageKey = item.stage;

    const attribution = item.principal
      || (payload.command_id ? `Command ${shortIdentity(payload.command_id)}` : "Kiln core");
    const facts = [];
    if (payload.attempt_id) facts.push(["Attempt", payload.attempt_id]);
    if (payload.next_attempt_id) facts.push(["Next attempt", payload.next_attempt_id]);
    if (payload.effect_id) facts.push(["Effect ID", payload.effect_id]);
    if (payload.provider_profile) facts.push(["Provider", payload.provider_profile]);
    if (payload.sequence != null) facts.push(["Sequence", String(payload.sequence)]);
    if (Array.isArray(payload.products)) facts.push(["Products", String(payload.products.length)]);
    if (Array.isArray(payload.metrics)) facts.push(["Metrics", String(payload.metrics.length)]);
    if (item.run_id) facts.push(["Run", runLink(item.run_id)]);
    return {
      stage: CAUSAL_STAGE_BY_KEY.get(stageKey),
      title: activityTitle(item),
      attribution,
      identity: typeof item.event_identity === "string" ? item.event_identity : "",
      facts,
      outcome: payload.outcome || null,
    };
  }

  function renderCausalTimeline(parent, items, emptyMessage = "No retained causal events.") {
    const classified = (Array.isArray(items) ? items : []).map((item, idx) => ({ item, idx, info: classifyCausalEvent(item) }));
    classified.sort((a, b) => {
      const ta = Number.isSafeInteger(a.item?.recorded_at_ms) ? a.item.recorded_at_ms : Number.MAX_SAFE_INTEGER;
      const tb = Number.isSafeInteger(b.item?.recorded_at_ms) ? b.item.recorded_at_ms : Number.MAX_SAFE_INTEGER;
      if (ta !== tb) return ta - tb;
      const oa = Number.isSafeInteger(a.item?.ordinal) ? a.item.ordinal : 0;
      const ob = Number.isSafeInteger(b.item?.ordinal) ? b.item.ordinal : 0;
      if (oa !== ob) return oa - ob;
      const ia = Number.isSafeInteger(a.item?.event_index) ? a.item.event_index : 0;
      const ib = Number.isSafeInteger(b.item?.event_index) ? b.item.event_index : 0;
      if (ia !== ib) return ia - ib;
      return (a.info.stage.order - b.info.stage.order) || (a.idx - b.idx);
    });

    const stageCounts = new Map(CAUSAL_STAGES.map((s) => [s.key, 0]));
    for (const row of classified) stageCounts.set(row.info.stage.key, (stageCounts.get(row.info.stage.key) || 0) + 1);
    const rail = element("ol", undefined, "causal-stage-rail");
    rail.setAttribute("aria-label", "Causal state-machine stages");
    for (const s of CAUSAL_STAGES) {
      const count = stageCounts.get(s.key) || 0;
      const step = element("li", undefined, "causal-stage-step");
      step.dataset.stage = s.key;
      step.dataset.reached = String(count > 0);
      step.append(element("span", s.label, "causal-stage-step-label"));
      if (count > 0) step.append(element("span", String(count), "chip"));
      rail.append(step);
    }
    parent.append(rail);

    if (!classified.length) {
      parent.append(emptyState("No retained activity", emptyMessage));
      return;
    }

    const list = element("ol", undefined, "causal-timeline");
    for (const { item, info } of classified) {
      const card = element("li", undefined, "record-card activity-item causal-event");
      card.dataset.stage = info.stage.key;
      const head = element("div", undefined, "causal-event-head");
      const stageBadge = element("span", info.stage.label, "state-badge causal-stage-badge");
      stageBadge.dataset.tone = info.stage.tone === "success" ? "good" : info.stage.tone === "running" || info.stage.tone === "info" ? "info" : info.stage.tone === "warning" ? "warning" : "neutral";
      head.append(stageBadge);
      if (info.outcome) head.append(ui.statusPill(info.outcome));
      head.append(element("h3", info.title, "causal-event-title"));
      head.append(when(item?.recorded_at_ms));
      card.append(head);

      const meta = element("div", undefined, "causal-event-meta");
      meta.append(element("span", `Attribution: ${info.attribution}`, "small"));
      if (item?.project_id) meta.append(element("span", `Project: ${text(projectLabel(item.project_id))}`, "small"));
      const exact = item ? activityExact(item) : null;
      if (exact) meta.append(exact);
      const idChip = copyableIdentityChip(info.identity, "event_identity");
      if (idChip) meta.append(idChip);
      card.append(meta);

      if (info.facts.length) {
        const dl = element("dl", undefined, "facts");
        addFacts(dl, info.facts);
        card.append(dl);
      }

      const rawDisclosure = element("details", undefined, "activity-fields causal-raw");
      rawDisclosure.append(element("summary", "Raw JSON record"), element("pre", JSON.stringify(item, null, 2), "identity"));
      card.append(rawDisclosure);
      list.append(card);
    }
    parent.append(list);
  }

  function renderStructuredFactCard(title, rows, rawObj, parent) {
    const card = section(title, parent, "record-card");
    const dl = element("dl", undefined, "facts");
    addFacts(dl, rows);
    card.append(dl);
    if (rawObj !== undefined) {
      const raw = element("details", undefined, "causal-raw");
      raw.append(element("summary", "Raw JSON record"), element("pre", JSON.stringify(rawObj, null, 2), "identity"));
      card.append(raw);
    }
    return card;
  }

  // r[impl kiln.dashboard.complete.evidence]
  function renderEvidence(parent, result) {
    parent.append(element("p", "Recorded evidence only. A reported verdict, review, provenance link or retained artifact does not establish accepted gate authority or product correctness.", "warning"));
    const isRunEvidence = Boolean(result && typeof result === "object" && (result.run_id || result.source || result.pipeline || result.execution));
    if (isRunEvidence) {
      // Synthesize causal state-machine timeline from retained events or run evidence linkage.
      const timelineItems = Array.isArray(result.events) && result.events.length ? result.events : [];
      if (!timelineItems.length) {
        const run = state.run?.id === result.run_id ? state.run : null;
        if (result.source || result.pipeline) {
          timelineItems.push({
            kind: "canonical",
            run_id: result.run_id,
            recorded_at_ms: run?.created_at_ms ?? null,
            event_identity: result.pipeline?.identity || result.source?.canonical_tree || "",
            event: { RunStarted: { run_id: result.run_id, attempt_id: result.execution?.attempt_id, provider_profile: result.source?.provider_profile } },
            stage: "admitted",
          });
        }
        if (run?.untrusted || result.untrusted) {
          const gate = run?.untrusted || result.untrusted;
          timelineItems.push({
            kind: "audit",
            run_id: result.run_id,
            recorded_at_ms: gate.approved_at_ms ?? null,
            principal: gate.approved_by || "Untrusted gate",
            audit: { method: "GATE", target: "/untrusted", decision: gate.state || "untrusted", actor_label: gate.approved_by || "Policy" },
          });
        }
        if (result.execution?.effect_id || result.execution?.attempt_id) {
          timelineItems.push({
            kind: "canonical",
            run_id: result.run_id,
            recorded_at_ms: run?.started_at_ms ?? null,
            event_identity: result.execution?.candidate_id || "",
            event: { RetryStarted: { run_id: result.run_id, next_attempt_id: result.execution.attempt_id, effect_id: result.execution.effect_id } },
            stage: "dispatched",
          });
        }
        if (result.execution?.observation) {
          timelineItems.push({
            kind: "canonical",
            run_id: result.run_id,
            recorded_at_ms: run?.started_at_ms ?? null,
            event_identity: result.execution.observation.identity || "",
            event: { Observation: { sequence: result.execution.observation.sequence, state: result.execution.observation.state } },
            stage: "observations",
          });
        }
        if (run?.outcome && run.outcome !== "Pending") {
          timelineItems.push({
            kind: "canonical",
            run_id: result.run_id,
            recorded_at_ms: run.completed_at_ms ?? null,
            event_identity: result.execution?.candidate_id || "",
            event: { AttemptCompleted: { attempt_id: result.execution?.attempt_id || run.attempt_id, outcome: run.outcome } },
          });
        }
      }
      if (timelineItems.length) {
        const timelineGroup = section("Causal Event-Sourced Decision Timeline", parent, "view-stack");
        renderCausalTimeline(timelineGroup, timelineItems);
      }

      if (window.KilnReceipts?.renderGatePassport) {
        const run = state.run?.id === result.run_id ? state.run : null;
        parent.append(window.KilnReceipts.renderGatePassport({
          current_candidate_id: result.execution?.candidate_id || "",
          current_revision: result.source?.exact_revision?.native_id || run?.source_revision || "",
          reports: Array.isArray(result.reports) ? result.reports : [],
          checks: Array.isArray(result.checks) ? result.checks : [],
          cairn_completion: result.cairn_completion || result.authoritative_completion || run?.implementation_verification || null,
          handoff: result.handoff || null,
          plan_review: result.plan_review || result.plan_review_receipt || null,
          semantic_merge: result.semantic_merge || null,
          provenance: result.provenance || result.in_toto || null,
          gate_passport: result.gate_passport || null,
        }, { title: "Run Gate & Receipt Passport" }));
      }

      if (result.source) {
        renderStructuredFactCard("Source linkage", [
          ["Provider profile", result.source.provider_profile],
          ["Repository", result.source.repository?.native_id || result.source.repository],
          ["Reference", result.source.symbolic_reference?.native_id || result.source.symbolic_reference],
          ["Exact revision", copyableIdentityChip(result.source.exact_revision?.native_id || result.source.exact_revision, "source revision") || result.source.exact_revision],
          ["Canonical tree", copyableIdentityChip(result.source.canonical_tree, "canonical tree") || result.source.canonical_tree],
        ], result.source, parent);
      }
      if (result.pipeline) {
        renderStructuredFactCard("Pipeline linkage", [
          ["Pipeline identity", copyableIdentityChip(result.pipeline.identity, "pipeline identity") || result.pipeline.identity],
          ["Revision ID", result.pipeline.revision_id],
          ["Nodes", String(Array.isArray(result.pipeline.nodes) ? result.pipeline.nodes.length : 0)],
          ["Edges", String(Array.isArray(result.pipeline.edges) ? result.pipeline.edges.length : 0)],
        ], result.pipeline, parent);
      }
      if (result.execution) {
        renderStructuredFactCard("Execution linkage", [
          ["Attempt ID", result.execution.attempt_id],
          ["Candidate ID", copyableIdentityChip(result.execution.candidate_id, "candidate") || result.execution.candidate_id],
          ["Effect ID", copyableIdentityChip(result.execution.effect_id, "effect") || result.execution.effect_id],
          ["Observation sequence", result.execution.observation?.sequence != null ? String(result.execution.observation.sequence) : "Not recorded"],
        ], result.execution, parent);
      }
    }
    const runLinks = element("div", undefined, "action-row");
    if (result.run_id) runLinks.append(runLink(result.run_id, "Inspect this exact run", "pipeline"));
    if (result.parent_run_id) runLinks.append(runLink(result.parent_run_id, "Inspect parent run"));
    if (runLinks.childElementCount) parent.append(runLinks);
    if (Array.isArray(result.artifacts)) {
      const group = section("Retained artifact provenance", parent, "view-stack");
      if (!result.artifacts.length) group.append(element("p", "No retained artifacts for this run.", "small"));
      for (const artifact of result.artifacts) {
        const card = renderStructuredFactCard(text(artifact.name), [
          ["Artifact ID", artifact.id],
          ["Node", artifact.node_id],
          ["Attempt", artifact.attempt_id],
          ["Bytes", Number.isSafeInteger(artifact.bytes) ? `${ui.humanBytes(artifact.bytes)} (${artifact.bytes})` : text(artifact.bytes)],
          ["BLAKE3", copyableIdentityChip(artifact.blake3, "artifact BLAKE3") || artifact.blake3],
        ], artifact, group);
        const button = element("button", "Download exact retained artifact");
        button.type = "button";
        button.addEventListener("click", () => downloadArtifact(result.run_id, artifact, button));
        card.append(button);
      }
    }
    if (Array.isArray(result.reports)) { const group = section("Recorded gate, review and report evidence", parent, "view-stack"); renderReports(group, { run_id: result.run_id, reports: result.reports }); }
    if (!isRunEvidence) {
      renderStructuredFactCard("Recorded evidence details", [
        ...(Object.hasOwn(result || {}, "accepted_gate") ? [["Accepted gate", String(result.accepted_gate)]] : []),
        ...(result?.claim_boundary ? [["Claim boundary", result.claim_boundary]] : []),
      ], result, parent);
    }
    if (Array.isArray(result.links) && result.links.length) {
      const group = section("Recorded provenance links", parent, "view-stack");
      for (const recorded of result.links) {
        const row = element("div", undefined, "record-card"); row.append(element("p", text(recorded.path), "small"));
        let link = null;
        if (typeof recorded.value === "string" && /(?:^|[/.])run_id$/.test(recorded.path)) link = runLink(recorded.value);
        else if (typeof recorded.value === "string" && recorded.value.startsWith("https://")) {
          try { const url = new URL(recorded.value); if (url.protocol === "https:" && !url.username && !url.password) { link = element("a", recorded.value); link.href = url.href; link.target = "_blank"; link.rel = "noopener noreferrer"; link.referrerPolicy = "no-referrer"; } } catch { /* Invalid recorded URLs remain inert text. */ }
        }
        row.append(link || element("span", text(recorded.value)));
        if (link?.target === "_blank") row.append(element("p", "Recorded external link; destination is not validated acceptance evidence.", "small"));
        group.append(row);
      }
    }
    if (result.claim_boundary) parent.append(element("p", text(result.claim_boundary), "boundary"));
  }
  function renderActivity(parent, result) {
    if (!Array.isArray(result.items)) throw new Error("Activity response is invalid.");
    parent.append(element("p", `${result.items.length} shown of ${text(result.total)} · chronological causal progression · audit window: ${text(result.audit_window)}`, "small"));
    renderCausalTimeline(parent, result.items, "No retained activity in this bounded page. Audit decisions and canonical events appear here as they are recorded.");
    const controls = element("div", undefined, "control-bar");
    const first = element("button", "First page"); first.type = "button"; first.disabled = !state.route.activity_cursor; first.addEventListener("click", () => navigate({ activity_cursor: "" }));
    const next = element("button", "Next page"); next.type = "button"; next.disabled = !result.next_cursor; next.addEventListener("click", () => navigate({ activity_cursor: result.next_cursor }));
    controls.append(first, next); parent.append(controls);
  }
  // r[impl kiln.dashboard.complete.operations]
  function renderOperations(parent, result) {
    if (result.storage_failed) parent.append(element("p", "Durable storage is failed. Mutating admission is unavailable; no healthy state is inferred.", "warning"));
    // Retained quantities read in human units with their share of the limit; the exact recorded counts stay beside them.
    const quantity = (resource, value) => {
      const bytes = typeof resource === "string" && resource.endsWith("bytes") ? ui.humanBytes(value) : "";
      return bytes ? `${bytes} (${value.toLocaleString("en-US")} bytes)` : text(value);
    };
    const share = (used, maximum) => Number.isSafeInteger(used) && Number.isSafeInteger(maximum) && maximum > 0 ? ` · ${Math.floor((used / maximum) * 100)}% of the limit` : "";
    const resourceName = (resource) => text(resource).replaceAll("_", " ");
    for (const warning of result.warnings || []) {
      const row = element("div", undefined, "warning"); row.append(element("span", `${resourceName(warning.resource)} near its retention limit: ${quantity(warning.resource, warning.used)} of ${quantity(warning.resource, warning.maximum)}${share(warning.used, warning.maximum)}. Retention admission may fail; no canonical data is automatically pruned. Code ${text(warning.code)}.`));
      if (warning.run_id) row.append(document.createTextNode(" "), runLink(warning.run_id, "Inspect affected run")); parent.append(row);
    }
    // r[impl kiln.web.page_consistency.scenario.data] Scalar capacity reads as metric tiles; project labels lead queue and provider rows with exact ids kept secondary.
    const capacity = section("Execution capacity", parent, "record-card");
    const capacityRows = result.capacity && typeof result.capacity === "object" && !Array.isArray(result.capacity) ? Object.entries(result.capacity) : [];
    if (capacityRows.length && capacityRows.every(([, value]) => value === null || typeof value !== "object")) {
      const grid = element("div", undefined, "inventory-grid worker-metrics");
      for (const [key, value] of capacityRows) { const card = element("article", undefined, "inventory-card"); const label = key.replaceAll("_", " "); card.append(element("span", label[0].toUpperCase() + label.slice(1), "metric-label"), element("strong", text(value), "metric-value")); grid.append(card); }
      capacity.append(grid);
    } else renderRecorded(capacity, result.capacity);
    const queue = section("Durable queue", parent, "view-stack");
    if (!result.queue?.length) queue.append(emptyState("Queue is empty", "No queued work in the admitted snapshot."));
    for (const row of result.queue || []) { const card = section(`Position ${text(row.position)} · ${text(projectLabel(row.project_id))}`, queue, "record-card"); const queued = element("p", "Queued ", "small"); queued.append(when(row.created_at_ms)); const link = runLink(row.run_id, `Run ${shortIdentity(row.run_id)}`); link.title = text(row.run_id); card.append(element("p", text(row.project_id), "mono meta"), link, queued); }
    const providers = section("Provider observations", parent, "view-stack");
    if (!result.providers?.length) providers.append(emptyState("No provider observations", "No provider observation is retained in this snapshot. Capacity is not inferred."));
    for (const row of result.providers || []) {
      const card = element("article", undefined, "record-card provider-observation");
      const head = element("div", undefined, "provider-observation-head");
      const badge = element("span", text(row.status), "state-badge");
      if (row.status === "unavailable" || row.status === "observed") badge.dataset.tone = row.status === "observed" ? "good" : "warning";
      head.append(element("h4", text(projectLabel(row.project_id))), badge);
      card.append(head);
      const meta = element("p", undefined, "activity-feed-meta");
      if (row.provider_profile_id) meta.append(element("span", text(row.provider_profile_id), "mono"));
      if (row.sequence !== undefined) meta.append(element("span", `Sequence ${text(row.sequence)}`));
      if (typeof row.run_id === "string") { const link = runLink(row.run_id, `Run ${shortIdentity(row.run_id)}`); link.title = row.run_id; meta.append(link); }
      card.append(meta);
      if (row.status === "unavailable") card.append(element("p", "Provider observation is unavailable, not healthy capacity.", "warning"));
      const fields = element("details"); fields.append(element("summary", "Recorded fields")); renderRecorded(fields, row); card.append(fields);
      providers.append(card);
    }
    const usage = section("Retained usage & limits", parent, "view-stack");
    for (const row of result.usage || []) {
      const card = section(resourceName(row.resource), usage, "record-card"); card.append(element("p", `${quantity(row.resource, row.used)} of ${quantity(row.resource, row.maximum)}${share(row.used, row.maximum)}`, "small"));
      if (row.run_id) card.append(runLink(row.run_id, "Inspect retained run"));
      if (Number.isSafeInteger(row.used) && Number.isSafeInteger(row.maximum) && row.maximum > 0) { const meter = element("meter"); meter.min = 0; meter.max = row.maximum; meter.value = row.used; meter.setAttribute("aria-label", `${row.resource}: ${row.used} of ${row.maximum}`); card.append(meter); }
    }
    const policy = section("Retention policy", parent, "record-card"); renderRecorded(policy, result.retention_policy);
    const access = element("dl", undefined, "facts"); addFacts(access, [["Access policy BLAKE3", result.access_policy_identity]]); parent.append(access);
  }
  function field(form, label, type, value, bounds = {}) {
    const wrapper = element("label", undefined, type === "checkbox" ? "check" : "form-field");
    const input = element("input"); input.type = type;
    if (type === "checkbox") input.checked = value === true;
    else input.value = value ?? "";
    for (const [key, val] of Object.entries(bounds)) input[key] = val;
    if (type === "checkbox") wrapper.append(input, document.createTextNode(label)); else wrapper.append(element("span", label), input);
    input.disabled = !canOperate() || state.permissions.configure !== true;
    form.append(wrapper); return input;
  }
  // r[impl kiln.dashboard.complete.automation]
  function renderAutomation(parent, result) {
    // r[impl kiln.web.page_consistency.scenario.states] Automation status leads as a badge; an unconfigured policy is an explicit state with guidance.
    const status = element("div", undefined, "badges");
    const statusBadge = element("span", `Automation: ${text(result.status)}`, "state-badge");
    if (result.status === "enabled" || result.status === "unconfigured") statusBadge.dataset.tone = result.status === "enabled" ? "good" : "warning";
    status.append(statusBadge, element("span", result.configured ? "Only registered sources and destinations are editable." : "No automation policy is admitted. Register sources/destinations in server policy; browser input cannot create authority.", "small"));
    parent.append(status);
    const policy = element("dl", undefined, "facts"); addFacts(policy, [["Reviewed policy BLAKE3", result.policy_identity]]); parent.append(policy);
    for (const [key, kind] of [["triggers", "trigger"], ["schedules", "schedule"], ["notifications", "notification"]]) {
      const group = section(key[0].toUpperCase() + key.slice(1), parent, "view-stack");
      if (!result[key]?.length) group.append(emptyState(`No registered ${key}`, "No registrations. Registrations are admitted in server policy, not created from this page."));
      for (const row of result[key] || []) {
        const form = element("form", undefined, "record-card automation-form"); form.append(element("h3", text(row.id)));
        const scope = kind === "trigger" ? ui.scopeSummary(row) : null;
        const facts = element("dl", undefined, "facts"), filter = row.filter || {}, filterParts = [];
        for (const [label, values] of [["branches", filter.branches], ["tags", filter.tags], ["types", filter.types], ["paths", filter.paths]]) if (Array.isArray(values) && values.length) filterParts.push(`${label}: ${values.join(", ")}`);
        addFacts(facts, [["Project", projectLabel(row.project_id) === row.project_id ? row.project_id : `${projectLabel(row.project_id)} · ${row.project_id}`], ["Reference", row.reference], ...(filterParts.length ? [["Filter", filterParts.join(" · ")]] : []), ...(scope ? [["Path scope", scopeLine(scope, true)]] : []), ["Last status", CANONICAL_OUTCOMES.has(row.last_status) ? ui.statusPill(row.last_status) : row.last_status]]); form.append(facts);
        const inputs = { enabled: field(form, "Enabled", "checkbox", row.enabled) };
        if (kind === "trigger") {
          inputs.push = field(form, "Signed push events", "checkbox", row.push); inputs.pull_request = field(form, "Signed pull request events", "checkbox", row.pull_request);
          if (row.allow_push === false) inputs.push.disabled = true;
          if (row.allow_pull_request === false) inputs.pull_request.disabled = true;
        }
        if (kind === "schedule") {
          inputs.interval_ms = field(form, "Interval (milliseconds)", "number", row.interval_ms, { min: row.min_interval_ms, max: row.max_interval_ms, step: "1", required: true });
          const next = Number.isSafeInteger(row.next_due_ms) ? new Date(row.next_due_ms).toISOString().slice(0, 23) : "";
          inputs.next_due_ms = field(form, "Next due (UTC, not local time)", "text", next, { placeholder: "YYYY-MM-DDTHH:mm:ss.sss", maxLength: 23 });
          form.append(element("p", `Allowed interval: ${text(row.min_interval_ms)}–${text(row.max_interval_ms)} ms. Last admitted next due: ${timestamp(row.next_due_ms)}.`, "small"));
        }
        if (kind === "notification") {
          inputs.on_success = field(form, "Notify on success", "checkbox", row.on_success); inputs.on_failure = field(form, "Notify on failure", "checkbox", row.on_failure);
          if (row.allow_success === false) inputs.on_success.disabled = true;
          if (row.allow_failure === false) inputs.on_failure.disabled = true;
        }
        const button = element("button", "Save registered automation"); button.type = "submit"; button.disabled = !canOperate() || state.permissions.configure !== true; form.append(button);
        form.addEventListener("submit", async (event) => {
          event.preventDefault(); if (!canOperate() || state.permissions.configure !== true) return;
          const payload = { kind, id: row.id };
          for (const [name, input] of Object.entries(inputs)) {
            if (input.type === "checkbox") payload[name] = input.checked;
            else if (name === "next_due_ms") { if (!input.value.trim()) continue; if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?$/.test(input.value)) { notice("Enter next due as a UTC date and time including seconds."); return; } const ms = Date.parse(input.value + "Z"); if (!Number.isSafeInteger(ms)) { notice("Next due UTC time is invalid."); return; } payload[name] = ms; }
            else { const number = Number(input.value); if (!Number.isSafeInteger(number)) { notice("Automation interval must be an exact integer."); return; } payload[name] = number; }
          }
          button.disabled = true;
          const updated = await mutate("/api/automation", payload, "Automation registration updated.");
          if (updated && state.session && activeSubPageFor(state.route) === "automation") { parent.replaceChildren(); renderAutomation(parent, updated); }
          else button.disabled = !canOperate() || state.permissions.configure !== true;
        });
        group.append(form);
      }
    }
    const intake = section("Signed event intake & schedule outcomes", parent, "view-stack"); renderRecorded(intake, result.sources);
    const deliveries = section("HTTPS notification delivery status", parent, "view-stack"); renderRecorded(deliveries, result.deliveries);
    const retention = section("Automation retention", parent, "record-card"); renderRecorded(retention, result.retention);
  }
  function saveBlob(blob, filename) {
    const url = URL.createObjectURL(blob); state.urls.add(url);
    const link = element("a"); link.href = url; link.download = filename.replace(/[\\/\u0000-\u001f\u007f]/g, "_"); link.hidden = true; document.body.append(link); link.click(); link.remove();
    const timer = setTimeout(() => { URL.revokeObjectURL(url); state.urls.delete(url); state.urlTimers.delete(timer); }, 1000); state.urlTimers.add(timer);
  }
  // r[impl kiln.dashboard.complete.streaming]
  async function downloadLogs() {
    const id = state.selected; const node = $("log-node").value; const stream = $("log-stream").value;
    if (!id || !node) return;
    const epoch = state.epoch; const generation = state.logGeneration; $("download-log").disabled = true;
    try {
      const query = new URLSearchParams({ node_id: node, stream });
      const metadata = await api(`${pathFor(id)}/logs/download?${query}&metadata=true`);
      if (epoch !== state.epoch) return;
      if (!Number.isSafeInteger(metadata.bytes) || metadata.bytes < 0) throw new Error("Exact retained download size is unavailable.");
      const blob = await api(`${pathFor(id)}/logs/download?${query}`, { kind: "blob" });
      if (epoch !== state.epoch) return;
      if (blob.size !== metadata.bytes) throw new Error("Retained stream changed while downloading. Refresh metadata before another explicit download; no partial file was offered.");
      saveBlob(blob, metadata.filename || `${id}-${node}-${stream}.log`);
      if (id === state.selected && generation === state.logGeneration) $("log-status").textContent = `Downloaded ${blob.size} complete retained bytes for ${node}/${stream}. Retained: ${text(metadata.retained)} · truncated: ${text(metadata.truncated)} · BLAKE3: ${text(metadata.blake3)}. Missing source bytes are not reconstructed.`;
    }
    catch (error) { if (!isAborted(error) && epoch === state.epoch && generation === state.logGeneration && id === state.selected) $("log-status").textContent = `Retained stream download unavailable: ${explain(error)}`; }
    finally { if (epoch === state.epoch) $("download-log").disabled = false; }
  }
  // Reported test seconds are producer floats; humanize while keeping the exact value inspectable.
  function reportedSeconds(seconds) {
    if (typeof seconds !== "number" || !Number.isFinite(seconds) || seconds < 0) return "Not recorded";
    const node = element("span", ui.formatDuration(seconds * 1000), "duration");
    node.title = `${seconds} s reported`;
    node.append(element("span", ` (${seconds} s reported)`, "sr-only"));
    return node;
  }
  const testStatuses = { passed: ["success", "Passed"], failed: ["failure", "Failed"], error: ["failure", "Error"], skipped: ["cancelled", "Skipped"] };
  function testStatusPill(status) {
    const [tone, label] = Object.hasOwn(testStatuses, status) ? testStatuses[status] : ["unknown", typeof status === "string" && status ? status : "Not recorded"];
    return statePill(tone, label, typeof status === "string" ? `Reported status: ${status}` : "");
  }
  // Account disclosure: identity and session actions stay subordinate to page actions.
  function setAccountMenu(open, restoreFocus = false) {
    $("account-menu").hidden = !open;
    $("account-button").setAttribute("aria-expanded", String(open));
    if (!open && restoreFocus) $("account-button").focus();
  }
  function setMenu(open, restoreFocus = false) {
    const mobile = window.matchMedia("(max-width: 60rem)").matches;
    const wasOpen = $("app-menu-toggle").getAttribute("aria-expanded") === "true";
    open = open && mobile;
    if (open) setAccountMenu(false);
    $("app-sidebar").classList.toggle("is-open", open);
    $("app-sidebar").inert = mobile && !open;
    $("app-menu-toggle").setAttribute("aria-expanded", String(open));
    $("app-menu-backdrop").hidden = !open;
    document.documentElement.classList.toggle("app-menu-open", open);
    $("main").inert = open;
    for (const node of document.querySelectorAll(".session-tools, .topbar-search")) node.inert = open;
    if (open) $("app-sidebar").querySelector('[aria-current="page"]')?.focus();
    else if (restoreFocus) $("app-menu-toggle").focus();
    else if (wasOpen && state.session) {
      $("workspace-title").tabIndex = -1;
      $("workspace-title").focus({ preventScroll: true });
    }
  }
  state.route = readRoute();
  // Filter values stay canonical; labels are human with the exact outcome alongside.
  const outcomeFilterLabel = (id) => { const label = id === "Pending" ? "Pending (queued or running)" : ui.outcomeStatus(id).label; return label === id || id === "Pending" ? label : `${label} (${id})`; };
  options($("run-outcome"), [{ id: "failed", label: "All failures" }, { id: "queued", label: "Queued" }, ...["Pending", "Success", "DomainFailure", "Denied", "Cancelled", "TimedOut", "ProviderError", "Unknown"].map((id) => ({ id, label: outcomeFilterLabel(id) }))], "All outcomes", state.route.outcome);
  // r[impl kiln.web.design_system] Web deviation: rail destinations and run tabs are links (new tab, history); plain clicks stay in-page.
  const plainClick = (event) => event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey;
  for (const node of document.querySelectorAll("a[data-page]")) node.addEventListener("click", (event) => { if (plainClick(event)) { event.preventDefault(); navigate({ page: node.dataset.page }); } });
  $("brand-home").addEventListener("click", (event) => { if (plainClick(event)) { event.preventDefault(); navigate(HOME); } });
  for (const node of document.querySelectorAll("a[data-view]")) node.addEventListener("click", (event) => { if (plainClick(event)) { event.preventDefault(); navigate({ view: node.dataset.view }); } });
  for (const mode of ["exact", "latest", "failed"]) $("rerun-" + mode).addEventListener("click", () => { void prepareRerun(mode); });
  for (const id of ["app-theme", "app-density"]) $(id).addEventListener("change", () => navigate({ [id === "app-theme" ? "theme" : "density"]: $(id).value }));
  for (const node of $("theme-switch").children) node.addEventListener("click", () => navigate({ theme: node.dataset.themeChoice }));
  $("live-activity").addEventListener("click", (event) => { if (plainClick(event)) { event.preventDefault(); navigate(PENDING_RUNS); } });
  $("app-menu-toggle").addEventListener("click", () => setMenu($("app-menu-toggle").getAttribute("aria-expanded") !== "true"));
  $("app-menu-backdrop").addEventListener("click", () => setMenu(false, true));
  window.matchMedia("(max-width: 60rem)").addEventListener("change", () => setMenu(false));
  document.addEventListener("keydown", (event) => {
    if ($("app-menu-toggle").getAttribute("aria-expanded") !== "true") return;
    if (event.key === "Escape") { event.preventDefault(); setMenu(false, true); }
    if (event.key === "Tab") {
      const stops = [$("app-menu-toggle"), ...$("app-sidebar").querySelectorAll('a[href], button:not(:disabled), summary')].filter((node) => node.getClientRects().length);
      const index = stops.indexOf(document.activeElement);
      event.preventDefault();
      stops[(index + (event.shiftKey ? -1 : 1) + stops.length) % stops.length].focus();
    }
  });
  // r[impl kiln.web.console_layout.scenario.navigation] Desktop rail: pin keeps it expanded (page memory only); Escape collapses an unpinned expansion and returns focus.
  const railQuery = window.matchMedia("(min-width: 60.001rem)");
  let railReturnFocus = null;
  function setRailPinned(pinned) {
    document.documentElement.classList.toggle("rail-pinned", pinned);
    $("rail-pin").setAttribute("aria-pressed", String(pinned));
    $("app-sidebar").classList.remove("rail-suppressed");
  }
  $("rail-pin").addEventListener("click", () => setRailPinned($("rail-pin").getAttribute("aria-pressed") !== "true"));
  $("app-sidebar").addEventListener("mouseenter", () => $("app-sidebar").classList.remove("rail-suppressed"));
  $("app-sidebar").addEventListener("focusin", (event) => {
    $("app-sidebar").classList.remove("rail-suppressed");
    if (!(event.relatedTarget instanceof Node && $("app-sidebar").contains(event.relatedTarget))) railReturnFocus = event.relatedTarget instanceof HTMLElement ? event.relatedTarget : null;
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || event.defaultPrevented || !railQuery.matches || document.documentElement.classList.contains("rail-pinned") || document.querySelector("dialog[open]")) return;
    const sidebar = $("app-sidebar");
    const focused = sidebar.contains(document.activeElement);
    const idle = !document.activeElement || document.activeElement === document.body || document.activeElement === $("main");
    if (!focused && !(idle && sidebar.matches(":hover"))) return;
    event.preventDefault();
    sidebar.classList.add("rail-suppressed");
    if (!focused) return;
    const target = railReturnFocus?.isConnected && !sidebar.contains(railReturnFocus) && !railReturnFocus.closest("[hidden], [inert]") ? railReturnFocus : $("main");
    target.focus({ preventScroll: true });
  });
  $("run-list-toggle").addEventListener("click", () => setRunList($("run-list-toggle").getAttribute("aria-expanded") !== "true"));
  splitRunQuery.addEventListener("change", () => {
    if (!state.route) return;
    applyRunPanels();
    if (bothPrimaryPanels() && state.session && state.route.page === "runs" && state.run && !state.logKey && !state.logBusy) { maybeAutoFollow(); void readLog(false); }
  });
  $("runs-first").addEventListener("click", () => navigate({ cursor: "" }));
  $("runs-next").addEventListener("click", () => { if (state.nextCursor) navigate({ cursor: state.nextCursor }); });
  $("compare-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ other: $("compare-other").value.trim(), view: "compare" }); });
  $("evidence-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ page: "evidence", run: $("evidence-run").value.trim() }); });
  $("cob-search-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ cob_repository: $("cob-repository").value, cob_type: $("cob-type").value, cob_q: $("cob-search").value.trim(), cob_cursor: "" }); });
  $("cob-reset").addEventListener("click", () => navigate({ cob_repository: "", cob_type: "", cob_q: "", cob_cursor: "" }));
  $("cob-first").addEventListener("click", () => navigate({ cob_cursor: "" }));
  $("cob-next").addEventListener("click", () => { if (state.cobsNextCursor) navigate({ cob_cursor: state.cobsNextCursor }); });
  $("source-mode").addEventListener("change", () => { const mode = $("source-mode").value; $("start-button").textContent = mode === "latest" ? "Run latest admitted reference" : mode === "exact" ? "Run exact reachable revision" : "Run selected admitted reference"; $("source-revision").required = mode === "exact"; $("source-revision").disabled = mode !== "exact"; $("source-reference").disabled = mode !== "reference"; });
  $("source-revision").disabled = true; $("source-reference").disabled = true;
  $("download-log").addEventListener("click", () => { void downloadLogs(); });
  window.addEventListener("popstate", () => { void readAction(restoreRoute)(); });
  window.addEventListener("hashchange", () => { void readAction(restoreRoute)(); });
  void restoreRoute();
  $("login-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const token = $("operator-token").value.trim();
    if (!/^[0-9a-fA-F]{64}$/.test(token)) { notice("Enter exactly 64 hexadecimal characters for the operator token."); return; }
    $("login-button").disabled = true;
    state.session = "token";
    state.token = token;
    state.epoch++;
    const epoch = state.epoch;
    $("operator-token").value = "";
    try {
      adoptSession(await api("/api/session"));
      await loadProjects();
      await openWorkspace(epoch);
    } catch (error) {
      if (!isAborted(error) && epoch === state.epoch) { const message = explain(error); logout(""); notice(`Sign-in could not establish operator access: ${message}`); }
    } finally { $("login-button").disabled = false; }
  });
  $("logout").addEventListener("click", () => logout());
  $("refresh-all").addEventListener("click", readAction(refreshAll));
  $("account-button").addEventListener("click", () => setAccountMenu($("account-menu").hidden));
  $("account-settings").addEventListener("click", () => { setAccountMenu(false); navigate({ page: "settings" }); });
  document.addEventListener("click", (event) => { if (!$("account-menu").hidden && !(event.target instanceof Element && event.target.closest(".account"))) setAccountMenu(false); });
  document.addEventListener("focusin", (event) => { if (!$("account-menu").hidden && !(event.target instanceof Element && event.target.closest(".account"))) setAccountMenu(false); });
  document.addEventListener("keydown", (event) => { if (event.key === "Escape" && !$("account-menu").hidden) { event.preventDefault(); setAccountMenu(false, true); } });
  $("available-project").addEventListener("change", projectDescriptions);
  $("enrolled-project").addEventListener("change", () => { projectDescriptions(); void readAction(loadRefs)(); });
  $("run-search-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ q: $("run-search").value.trim(), project: $("run-filter").value, outcome: $("run-outcome").value, source: $("run-source").value.trim(), order: $("run-order").value, cursor: "" }); });
  // r[impl kiln.web.interface.scenario.workflow]
  $("enroll-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const projectId = $("available-project").value;
    if (!projectId) return;
    const result = await mutate("/api/projects", { project_id: projectId }, "Project enrollment admitted.");
    if (!result) return;
    await readAction(async () => { await loadProjects(); $("enrolled-project").value = projectId; projectDescriptions(); })();
  });
  $("start-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const projectId = $("enrolled-project").value;
    if (!projectId) return;
    const mode = $("source-mode").value;
    const payload = { project_id: projectId };
    if (mode === "reference") { if (!$("source-reference").value) { notice("Select an admitted reference."); return; } payload.reference = $("source-reference").value; }
    if (mode === "exact") { if (!$("source-revision").value.trim()) { notice("Enter an exact reachable source revision."); return; } payload.source_revision = $("source-revision").value.trim(); }
    const result = await mutate("/api/runs", payload, "Run request admitted. Inspect queue, phase and outcome for actual execution state.");
    if (!result) return;
    await readAction(loadRuns)();
    if (result.id && state.session) navigate({ page: "runs", run: result.id, node: "", stream: "", view: "pipeline" });
  });
  for (const action of ["cancel", "pause", "resume"]) $("control-" + action).addEventListener("click", () => { void prepareControl(action); });
  // r[impl kiln.web.failure_first_run] Header Rerun/Cancel reuse the exact confirmation flows of the disclosure buttons.
  $("header-cancel").addEventListener("click", () => { void prepareControl("cancel"); });
  $("header-rerun").addEventListener("click", () => { void prepareRerun("exact"); });
  $("header-approve").addEventListener("click", () => { void prepareApprove(); });
  $("log-jump-error").addEventListener("click", jumpToFirstError);
  $("dismiss-control").addEventListener("click", () => dismissControl());
  $("control-dialog").addEventListener("cancel", () => dismissControl());
  $("control-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const pending = state.confirm;
    if (!pending) return;
    if (pending.resolve) {
      const reason = ($("confirm-reason")?.value || "").trim().slice(0, pending.maxReason || 1024);
      if (pending.requireReason && !reason) {
        if ($("confirm-reason-error")) { $("confirm-reason-error").textContent = "A reason is required."; $("confirm-reason-error").hidden = false; }
        $("confirm-reason")?.focus?.();
        return;
      }
      const { resolve, opener, requestId } = pending;
      dismissControl(true);
      opener?.focus?.();
      resolve({ confirmed: true, reason, requestId: requestId || "" });
      return;
    }
    if (state.mutation) return;
    dismissControl();
    if (pending.approve) await mutate(`${pathFor(pending.id)}/approve`, pending.approve, "Approval admitted for this exact commit. Observe the run for dispatch and its outcome; no success is inferred.");
    else if (pending.mode) await mutate(`${pathFor(pending.id)}/rerun`, { mode: pending.mode }, "New source-bound candidate requested.").then((result) => { if (result?.id) navigate({ page: "runs", run: result.id, node: "", stream: "", view: "pipeline" }); });
    else await mutate(`${pathFor(pending.id)}/control`, pending.identity, "Control response received. Observe admitted state for its result; no success is inferred.");
    if (state.session && pending.id === state.selected) await readAction(loadDetail)();
  });
  $("log-form").addEventListener("submit", (event) => { event.preventDefault(); stopLogFollow(); void readLog(false); });
  $("restart-log").addEventListener("click", () => { $("log-cursor").value = "0"; void readLog(false); });
  for (const id of ["log-node", "log-stream", "log-search"]) $(id).addEventListener("change", () => {
    resetLogs();
    if (id === "log-node") navigate({ node: $("log-node").value, view: "logs" });
    else if (id === "log-stream") navigate({ stream: $("log-stream").value });
    else void readLog(false);
  });
  $("follow-logs").addEventListener("change", () => {
    stopLogFollow();
    // An operator choice owns following for this run: unchecking declines automatic following until another run is opened.
    state.logAutoFollow = false;
    state.followDeclined = $("follow-logs").checked ? "" : state.selected || "";
    if ($("follow-logs").checked) { $("log-output").scrollTop = $("log-output").scrollHeight; void readLog(state.logKey !== ""); }
  });
  $("clear-log").addEventListener("click", resetLogs);
  function stopPanePolling() {
    clearTimeout(state.inspector.paneTimer);
    state.inspector.paneTimer = null;
    state.inspector.paneController?.abort();
    state.inspector.paneController = null;
  }
  async function loadAttemptPane(attemptId, panePre, paneStatus) {
    if (!state.inspector.open || state.inspector.kind !== "attempt" || state.inspector.id !== attemptId) return;
    if (document.visibilityState !== "visible") {
      if (paneStatus) paneStatus.textContent = "Pane tail paused (tab hidden).";
      return;
    }
    stopPanePolling();
    const controller = new AbortController();
    state.inspector.paneController = controller;
    try {
      const res = await api(`/api/fleet/attempts/${encodeURIComponent(attemptId)}/pane?lines=${state.inspector.paneLines || 200}`, { quiet: true, controller });
      if (!state.inspector.open || state.inspector.kind !== "attempt" || state.inspector.id !== attemptId) return;
      const output = typeof res?.text === "string" ? res.text : Array.isArray(res?.lines) ? res.lines.join("\n") : "";
      if (panePre) panePre.textContent = output || "(empty pane)";
      if (paneStatus) paneStatus.textContent = `Pane ${text(res?.pane_id)} · ${res?.line_count ?? 0} lines${res?.truncated ? " (truncated)" : ""} · ${text(res?.machine)} (${text(res?.session_state)})`;
    } catch (error) {
      if (isAborted(error) || !state.inspector.open || state.inspector.id !== attemptId) return;
      if (paneStatus) paneStatus.textContent = `Pane unavailable: ${explain(error)}`;
    } finally {
      if (state.inspector.paneController === controller) state.inspector.paneController = null;
      if (state.inspector.open && state.inspector.kind === "attempt" && state.inspector.id === attemptId && document.visibilityState === "visible") {
        state.inspector.paneTimer = setTimeout(() => { void loadAttemptPane(attemptId, panePre, paneStatus); }, MAX_PANE_POLL_MS);
      }
    }
  }
  function openInspector(kind, id, items = [], opener = null) {
    if (!INSPECT_KINDS.has(kind) || !id) return;
    if (Array.isArray(items) && items.length) {
      state.inspector.items = items.map((entry) => entry?.kind && entry?.id ? entry : attentionInspectTarget(entry));
    }
    state.inspector.returnFocus = opener || document.activeElement;
    navigate({ inspect: `${kind}:${id}` });
  }
  function closeInspector(restoreFocus = true) {
    stopPanePolling();
    const returnTarget = state.inspector.returnFocus;
    state.inspector.open = false;
    state.inspector.kind = "";
    state.inspector.id = "";
    if ($("slide-over-inspector")) $("slide-over-inspector").hidden = true;
    if ($("slide-over-backdrop")) $("slide-over-backdrop").hidden = true;
    if ($("slide-over-body")) $("slide-over-body").replaceChildren();
    if (state.route?.inspect) navigate({ inspect: "" });
    if (restoreFocus && returnTarget?.isConnected && !returnTarget.closest?.("[hidden], [inert]")) {
      returnTarget.focus({ preventScroll: true });
    }
  }
  function stepInspector(delta) {
    const items = state.inspector.items;
    if (!items.length || !state.inspector.open) return;
    const idx = items.findIndex((it) => it.kind === state.inspector.kind && it.id === state.inspector.id);
    const nextIdx = idx < 0 ? 0 : (idx + delta + items.length) % items.length;
    const next = items[nextIdx];
    if (next) openInspector(next.kind, next.id, items);
  }
  async function renderSlideOver(kind, id) {
    const body = $("slide-over-body");
    const titleEl = $("slide-over-title");
    const eyebrowEl = $("slide-over-eyebrow");
    const openFull = $("slide-over-open-full");
    if (!body || !titleEl) return;
    stopPanePolling();
    eyebrowEl.textContent = kind.toUpperCase();
    titleEl.textContent = id;
    body.replaceChildren(loadingState(`Loading ${kind}…`));
    const items = state.inspector.items;
    const idx = items.findIndex((it) => it.kind === kind && it.id === id);
    if ($("slide-over-prev")) $("slide-over-prev").disabled = items.length < 2 || idx < 0;
    if ($("slide-over-next")) $("slide-over-next").disabled = items.length < 2 || idx < 0;

    const fullLinks = {
      attempt: { page: "fleet", tab: "fleet" },
      claim: { page: "fleet", tab: "fleet" },
      decision: { page: "fleet", tab: "fleet" },
      attention: { page: "inbox", tab: "attention" },
      run: { page: "runs", tab: "runs", run: id },
      cairn: { page: "cairn", tab: "cairn", cairn_change: id.includes("/") ? id.split("/").slice(1).join("/") : id, cairn_repository: id.includes("/") ? id.split("/")[0] : "" },
      review: { page: "code-review", review_change: id },
      repo: { page: "source", tab: "repositories" },
    };
    if (openFull && fullLinks[kind]) {
      openFull.hidden = false;
      openFull.href = `?${new URLSearchParams(Object.entries(fullLinks[kind]).filter(([, v]) => v))}`;
      openFull.onclick = (e) => { if (plainClick(e)) { e.preventDefault(); closeInspector(false); navigate(fullLinks[kind]); } };
    } else if (openFull) openFull.hidden = true;

    try {
      if (kind === "attempt") {
        const attemptsRes = await safeCall("/api/fleet/attempts");
        if (!state.inspector.open || state.inspector.id !== id) return;
        const attempt = (attemptsRes.value?.attempts || []).find((a) => a.attempt === id);
        if (!attempt) { body.replaceChildren(errorState(`Attempt ${id} not found in live Fleet snapshot.`)); return; }
        titleEl.textContent = attempt.title || shortIdentity(id);
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [
          ["Attempt", attempt.attempt],
          ["State", `${text(attempt.state)} · ${text(attempt.phase)}`],
          ["Work", attempt.work || attempt.title],
          ["Fence", attempt.fence],
          ["Machine", attempt.session?.machine || "Not observed"],
          ["Session", `${text(attempt.session?.agent_name)} · ${text(attempt.session?.state)}`],
        ]);
        const paneBox = element("div", undefined, "slide-over-pane-viewer");
        const paneStatus = element("p", "Reading live herdr pane…", "small");
        paneStatus.id = "slide-over-pane-status";
        const panePre = element("pre", "Loading pane output…", "slide-over-pane-output");
        panePre.id = "slide-over-pane-output";
        paneBox.append(panePre);
        const controls = element("div", undefined, "view-stack");
        if (canOperate()) {
          const promptForm = element("form", undefined, "inline-form");
          const promptLabel = element("label", "Steer / Prompt agent");
          promptLabel.htmlFor = "slide-over-prompt-input";
          const promptInput = element("input");
          promptInput.id = "slide-over-prompt-input";
          promptInput.type = "text";
          promptInput.maxLength = 4096;
          promptInput.required = true;
          promptInput.placeholder = "Send answer or prompt to live herdr pane…";
          const promptBtn = element("button", "Send prompt", "primary");
          promptBtn.type = "submit";
          promptForm.append(promptLabel, promptInput, promptBtn);
          promptForm.addEventListener("submit", async (e) => {
            e.preventDefault();
            const txt = promptInput.value.trim();
            if (!txt) return;
            const res = await mutate(`/api/fleet/attempts/${encodeURIComponent(id)}/prompt`, { text: txt }, "Prompt sent to agent pane.");
            if (res) { promptInput.value = ""; void loadAttemptPane(id, panePre, paneStatus); }
          });
          const btnRow = element("div", undefined, "cluster");
          const interruptBtn = element("button", "Interrupt (Ctrl-C)", "quiet");
          interruptBtn.type = "button";
          interruptBtn.addEventListener("click", async () => {
            const res = await mutate(`/api/fleet/attempts/${encodeURIComponent(id)}/interrupt`, { reason: "Operator interrupt from Slide-Over" }, "Interrupt sent to agent pane.");
            if (res) void loadAttemptPane(id, panePre, paneStatus);
          });
          const cancelBtn = element("button", "Cancel Attempt…", "danger");
          cancelBtn.type = "button";
          cancelBtn.addEventListener("click", async () => {
            const choice = await confirmControl({
              title: `Cancel attempt ${shortIdentity(id)}?`,
              verb: "Cancel attempt",
              dismiss: "Keep running",
              danger: true,
              facts: [["Attempt", id], ["Work", attempt.title || attempt.work]],
              reason: { label: "Cancel reason", placeholder: "Why this attempt is being cancelled", required: true },
              cliCommand: `kiln-fleet cancel ${id}`,
              opener: cancelBtn,
            });
            if (!choice.confirmed) return;
            await mutate(`/api/fleet/attempts/${encodeURIComponent(id)}/cancel`, { reason: choice.reason, request_id: choice.requestId }, "Attempt cancelled.");
            closeInspector();
          });
          btnRow.append(interruptBtn, cancelBtn);
          controls.append(promptForm, btnRow);
        }
        body.replaceChildren(facts, element("h3", "Live agent pane"), paneStatus, paneBox, controls);
        void loadAttemptPane(id, panePre, paneStatus);
        return;
      }
      if (kind === "attention") {
        const attRes = state.factory.attention || (await api("/api/implementation/attention", { quiet: true }));
        if (!state.inspector.open || state.inspector.id !== id) return;
        const item = (attRes?.items || []).find((x) => x.id === id);
        if (!item) { body.replaceChildren(errorState(`Attention item ${id} is no longer active.`)); return; }
        titleEl.textContent = item.title || id;
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [
          ["Kind", attentionKindLabel(item.kind)],
          ["Severity", item.severity],
          ["Summary", item.context_preview?.summary],
          ["Run", item.target?.run_id],
          ["Campaign", item.target?.campaign_id],
          ["Work ID", item.target?.work_id],
          ["Attempt", item.target?.attempt_id],
          ["Commit", item.target?.source_commit],
        ]);
        const nodes = [facts];
        if (item.kind === "PlanReviewGate" && Array.isArray(item.context_preview?.steps)) {
          if (typeof window.KilnCairn?.renderPlanReview === "function" && item.target?.work_id && item.target?.plan_id) {
            const prBox = element("div", undefined, "view-stack");
            window.KilnCairn.renderPlanReview(prBox, {
              workId: item.target.work_id,
              planId: item.target.plan_id,
              revision: item.context_preview.revision ?? null,
              gate: item.context_preview.gate || "unreviewed",
              author: item.context_preview.author || "",
              steps: item.context_preview.steps,
              annotations: item.context_preview.annotations || [],
              comparison: item.context_preview.comparison || null,
              receiptDigest: item.context_preview.receipt_digest || null,
              canOperate: canOperate(),
              api,
              onReviewed: () => { closeInspector(); void refreshAll(); },
            });
            nodes.push(prBox);
          } else {
            nodes.push(element("h3", "Proposed plan steps"));
            const ol = element("ol", undefined, "slide-over-plan-steps");
            for (const step of item.context_preview.steps) ol.append(element("li", step.text || step.id || String(step)));
            nodes.push(ol);
            if (canOperate() && item.target?.work_id && item.target?.plan_id) {
              const row = element("div", undefined, "cluster");
              const accept = element("button", "Accept plan", "primary");
              accept.type = "button";
              accept.addEventListener("click", async () => { await executeAttentionAction(item, "accept_plan", accept); closeInspector(); });
              const changes = element("button", "Request changes", "quiet");
              changes.type = "button";
              changes.addEventListener("click", async () => { await executeAttentionAction(item, "request_changes", changes); closeInspector(); });
              row.append(accept, changes);
              nodes.push(row);
            }
          }
        } else if (item.kind === "UnknownTerminationOrDispatch" && canOperate()) {
          const recBtn = element("button", "Reconcile ambiguous termination", "primary");
          recBtn.type = "button";
          recBtn.addEventListener("click", async () => { await executeAttentionAction(item, "reconcile", recBtn); closeInspector(); });
          nodes.push(recBtn);
        } else if (item.kind === "UntrustedRunApproval" && canOperate()) {
          const appBtn = element("button", "Approve untrusted run…", "primary");
          appBtn.type = "button";
          appBtn.addEventListener("click", async () => { await executeAttentionAction(item, "approve", appBtn); closeInspector(); });
          nodes.push(appBtn);
        } else if (item.target?.campaign_id && canOperate()) {
          const wakeBtn = element("button", "Wake campaign", "primary");
          wakeBtn.type = "button";
          wakeBtn.addEventListener("click", async () => { await executeAttentionAction(item, "wake", wakeBtn); closeInspector(); });
          nodes.push(wakeBtn);
        }
        body.replaceChildren(...nodes);
        return;
      }
      if (kind === "claim") {
        const claimsRes = await api("/api/fleet/claims?include=foreign,expired", { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        const claim = (claimsRes?.claims || []).find((c) => c.identity === id) || (claimsRes?.expired_claims || []).find((c) => c.identity === id);
        if (!claim) { body.replaceChildren(errorState(`Claim ${id} not found.`)); return; }
        titleEl.textContent = typeof claim.scope?.target === "string" ? claim.scope.target : shortIdentity(id);
        const facts = element("dl", undefined, "facts");
        const claimState = typeof claim.state === "string" ? claim.state : claim.state && typeof claim.state === "object" ? Object.keys(claim.state)[0] || "unknown" : "unknown";
        addFacts(facts, [["Identity", claim.identity], ["Fence", claim.fence], ["State", claimState], ["Holder", `${claim.holder?.tool || ""} on ${claim.holder?.host || ""}`], ["Purpose", claim.purpose], ["Expires", timestamp(claim.expires_ms)]]);
        const nodes = [facts];
        if (canOperate()) {
          const row = element("div", undefined, "cluster");
          for (const mins of [30, 60]) {
            const btn = element("button", `Renew +${mins}m`, "quiet");
            btn.type = "button";
            btn.addEventListener("click", async () => { await mutate(`/api/fleet/claims/${claim.identity}/renew`, { fence: claim.fence, lease_minutes: mins }, `Claim renewed +${mins}m.`); void renderSlideOver("claim", id); });
            row.append(btn);
          }
          const rel = element("button", "Force-Release…", "danger");
          rel.type = "button";
          rel.addEventListener("click", async () => {
            const choice = await confirmControl({
              title: `Force-release claim ${claim.identity.slice(0, 12)}?`,
              verb: "Force-release claim",
              danger: true,
              facts: [["Claim", claim.identity], ["Fence", String(claim.fence)]],
              reason: { label: "Release reason", placeholder: "Why this claim is being released", required: true },
              cliCommand: `kiln-fleet release --claim ${claim.identity} --fence ${claim.fence}`,
              opener: rel,
            });
            if (!choice.confirmed) return;
            await mutate(`/api/fleet/claims/${claim.identity}/release`, { fence: claim.fence, note: choice.reason, request_id: choice.requestId }, "Claim released.");
            closeInspector();
          });
          row.append(rel);
          nodes.push(row);
        }
        body.replaceChildren(...nodes);
        return;
      }
      if (kind === "run") {
        const run = await api(pathFor(id), { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        titleEl.textContent = projectLabel(run.project_id);
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [["Run", run.id], ["Outcome", run.outcome], ["Phase", run.phase], ["Revision", run.source_revision], ["Created", timestamp(run.created_at_ms)]]);
        const nodes = [facts];
        if ((run.outcome === "Unknown" || String(run.phase || "").toLowerCase() === "unknown") && canOperate()) {
          const rec = element("button", "Reconcile / Probe Provider", "primary");
          rec.type = "button";
          rec.addEventListener("click", async () => { await reconcileRun(run.id); void renderSlideOver("run", id); });
          nodes.push(rec);
        }
        body.replaceChildren(...nodes);
        return;
      }
      if (kind === "cairn") {
        const [repo, ...rest] = id.split("/");
        const change = rest.join("/") || id;
        const q = new URLSearchParams({ repository: repo, change });
        const detail = await api(`/api/implementation/cairn/detail?${q}`, { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        titleEl.textContent = detail?.change || change;
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [["Repository", detail?.repository || repo], ["Lane", detail?.lane], ["Units", Array.isArray(detail?.units) ? detail.units.length : 0]]);
        body.replaceChildren(facts);
        return;
      }
      if (kind === "review") {
        const detail = await api(`/api/code-review/changes/${encodeURIComponent(id)}`, { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        titleEl.textContent = detail?.change?.title || id;
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [["Change", id], ["Status", detail?.change?.status], ["Target", detail?.change?.target_ref]]);
        body.replaceChildren(facts);
        return;
      }
      if (kind === "decision") {
        const decRes = await api("/api/fleet/decisions", { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        const dec = (decRes?.decisions || []).find((d) => d.identity === id);
        if (!dec) { body.replaceChildren(errorState(`Decision ${id} not found.`)); return; }
        titleEl.textContent = dec.question || id;
        const facts = element("dl", undefined, "facts");
        addFacts(facts, [["Decision", dec.identity], ["Question", dec.question], ["Raised", timestamp(dec.raised_ms)], ["Answer", dec.answer]]);
        body.replaceChildren(facts);
        return;
      }
      if (kind === "repo") {
        const repoRes = await api("/api/repositories", { quiet: true });
        if (!state.inspector.open || state.inspector.id !== id) return;
        const repos = Array.isArray(repoRes?.repositories) ? repoRes.repositories : [];
        const repo = repos.find((r) => r.identity === id || r.slug === id);
        if (!repo) { body.replaceChildren(errorState(`Repository ${id} not found in catalog.`)); return; }
        titleEl.textContent = repo.label || repo.slug || shortIdentity(repo.identity);

        const syncMap = { synced: "success", diverged: "failure", dirty: "warning", unknown: "unknown", local_only: "queued" };
        const syncTone = syncMap[repo.sync_status] || "unknown";
        const adm = repo.admission || {};
        const admLabels = [
          adm.catalog_imported ? "Catalog Imported" : null,
          adm.ci_configured ? "CI Configured" : null,
          adm.fleet_enrolled ? "Fleet Enrolled" : null,
          adm.source_configured ? "Source Configured" : null,
          (!adm.ci_configured && !adm.fleet_enrolled) ? "Observed" : null,
        ].filter(Boolean);

        const facts = element("dl", undefined, "facts");
        addFacts(facts, [
          ["Canonical ID", copyableIdentityChip(repo.identity, "repository") || repo.identity],
          ["Slug", repo.slug],
          ["Cross-fleet sync", statePill(syncTone, String(repo.sync_status || "unknown").toUpperCase())],
          ["Active Cairns", String(repo.total_cairns ?? 0)],
          ["Admission", admLabels.join(" · ")],
        ]);
        const nodes = [facts];

        if (Array.isArray(repo.non_claims) && repo.non_claims.length) {
          const ncBox = element("div", undefined, "warning slide-over-repo-note");
          for (const nc of repo.non_claims) ncBox.append(element("p", nc, "small"));
          nodes.push(ncBox);
        }

        // Remotes & Mirrors table
        nodes.push(element("h3", "Remotes & mirrors"));
        const remotes = Array.isArray(repo.remotes) && repo.remotes.length ? repo.remotes : repo.canonical ? [repo.canonical] : [];
        if (!remotes.length) {
          nodes.push(element("p", "No external remotes declared (local checkout only).", "small"));
        } else {
          const rWrap = element("div", undefined, "data-table-wrap");
          const rTable = element("table", undefined, "data-table slide-over-repo-table");
          const rHead = element("thead");
          const rHeadRow = element("tr");
          for (const h of ["Provider", "Role", "Endpoint", "Default ref", "Observed rev", "Status"]) rHeadRow.append(element("th", h));
          rHead.append(rHeadRow);
          const rBody = element("tbody");
          for (const ep of remotes) {
            const tr = element("tr");
            tr.append(
              element("td", ep.provider || "unknown"),
              element("td", ep.role === "canonical" ? "Canonical" : "Mirror"),
              element("td", ep.native_id || ep.url || "", "mono"),
              element("td", ep.default_ref || "—", "mono"),
              element("td", ep.observed_revision ? shortIdentity(ep.observed_revision) : "Unobserved", "mono"),
              element("td", ep.status || "declared"),
            );
            rBody.append(tr);
          }
          rTable.append(rHead, rBody);
          rWrap.append(rTable);
          nodes.push(rWrap);
        }

        // Fleet Local Copies table
        nodes.push(element("h3", "Fleet local working copies"));
        const copies = Array.isArray(repo.local_copies) ? repo.local_copies : [];
        if (!copies.length) {
          nodes.push(element("p", "No local working copies observed across the fleet.", "small"));
        } else {
          const cWrap = element("div", undefined, "data-table-wrap");
          const cTable = element("table", undefined, "data-table slide-over-repo-table");
          const cHead = element("thead");
          const cHeadRow = element("tr");
          for (const h of ["Machine", "Path", "VCS", "HEAD", "Branch", "State", "Cairns"]) cHeadRow.append(element("th", h));
          cHead.append(cHeadRow);
          const cBody = element("tbody");
          for (const cp of copies) {
            const tr = element("tr");
            const offline = cp.health === "unavailable";
            const stateCell = element("td");
            if (offline) stateCell.append(statePill("failure", "OFFLINE", cp.error || "Host unavailable"));
            else if (cp.dirty === true) stateCell.append(statePill("warning", "DIRTY"));
            else if (cp.dirty === false) stateCell.append(statePill("success", "CLEAN"));
            else stateCell.append(statePill("unknown", "UNKNOWN"));
            if (cp.error) stateCell.append(element("p", cp.error, "small"));
            tr.append(
              element("td", cp.machine || "unknown", "mono"),
              element("td", cp.path || "—", "mono"),
              element("td", cp.vcs || "—", "mono"),
              element("td", cp.head_commit ? shortIdentity(cp.head_commit) : "—", "mono"),
              element("td", cp.branch_or_bookmark || "—", "mono"),
              stateCell,
              element("td", cp.cairn_count != null ? String(cp.cairn_count) : "—", "num"),
            );
            cBody.append(tr);
          }
          cTable.append(cHead, cBody);
          cWrap.append(cTable);
          nodes.push(cWrap);
        }

        // Inline Add Mirror & Tier-3 Remove
        if (canOperate()) {
          nodes.push(element("h3", "Attach remote or mirror"));
          const mirrorForm = element("form", undefined, "view-stack slide-over-mirror-form");
          const row = element("div", undefined, "inline-form");
          const provSel = element("select");
          provSel.id = "slide-over-mirror-provider";
          provSel.setAttribute("aria-label", "Mirror provider");
          for (const [val, lbl] of [["github", "GitHub"], ["gitea", "Gitea / Forgejo"], ["radicle", "Radicle"]]) {
            provSel.append(Object.assign(element("option", lbl), { value: val }));
          }
          const roleSel = element("select");
          roleSel.id = "slide-over-mirror-role";
          roleSel.setAttribute("aria-label", "Mirror role");
          for (const [val, lbl] of [["mirror", "Mirror"], ["canonical", "Canonical"]]) {
            roleSel.append(Object.assign(element("option", lbl), { value: val }));
          }
          const urlInp = element("input");
          urlInp.id = "slide-over-mirror-url";
          urlInp.type = "text";
          urlInp.required = true;
          urlInp.placeholder = "owner/repo, https://..., or rad:z...";
          urlInp.setAttribute("aria-label", "Mirror URL or Radicle RID");
          const radNotice = element("p", "Read-only declaration/observation only: importing a Radicle RID never runs rad init, signing, push, sync, or delegate mutation.", "warning");
          radNotice.hidden = true;
          const mirrorErr = element("p", "", "warning");
          mirrorErr.hidden = true;
          provSel.addEventListener("change", () => { radNotice.hidden = provSel.value !== "radicle"; });
          const addBtn = element("button", "Add mirror…", "primary");
          addBtn.type = "submit";
          row.append(provSel, roleSel, urlInp, addBtn);
          mirrorForm.append(row, radNotice, mirrorErr);
          mirrorForm.addEventListener("submit", async (e) => {
            e.preventDefault();
            const provider = provSel.value;
            const role = roleSel.value;
            const url = urlInp.value.trim();
            if (typeof window.KilnSource?.validateRemoteInput === "function") {
              const check = window.KilnSource.validateRemoteInput({ provider, role, url, attachRepo: repo });
              if (!check.ok) { mirrorErr.textContent = check.error; mirrorErr.hidden = false; return; }
            }
            mirrorErr.hidden = true;
            const choice = await confirmControl({
              title: `Attach ${provider} ${role} to ${repo.label || repo.slug}?`,
              verb: "Attach mirror",
              warning: provider === "radicle" ? "Read-only declaration/observation only: importing a Radicle RID never runs rad init, signing, push, sync, or delegate mutation." : DEFAULT_CONFIRM_WARNING,
              facts: [["Repository", repo.identity], ["Provider", provider], ["Role", role], ["Endpoint", url]],
              cliCommand: `kiln repositories add-mirror ${repo.identity} --provider ${provider} --role ${role} ${url}`,
              opener: addBtn,
            });
            if (!choice.confirmed) return;
            const res = await mutate(`/api/repositories/${encodeURIComponent(repo.identity)}/mirrors`, {
              provider, role, url, request_id: choice.requestId,
            }, "Mirror attached to repository.");
            if (res) {
              void renderSlideOver("repo", id);
              if (activeSubPageFor(state.route) === "repositories") void readAction(loadPage)();
              else if (state.route?.page === "overview") void loadOverview();
            }
          });
          nodes.push(mirrorForm);

          if (adm.catalog_imported) {
            const remBtn = element("button", "Remove from catalog…", "danger");
            remBtn.type = "button";
            remBtn.addEventListener("click", async () => {
              const choice = await confirmControl({
                title: `Remove ${repo.label || repo.slug} from catalog?`,
                verb: "Remove from catalog",
                dismiss: "Keep in catalog",
                danger: true,
                warning: "Removes this repository registration from Kiln's durable catalog only. Local files on disk are NEVER deleted.",
                facts: [["Repository", repo.label || repo.slug], ["Canonical ID", repo.identity], ["Disk files", "Untouched"]],
                reason: { label: "Removal reason", placeholder: "Why this catalog entry is being removed", required: true },
                cliCommand: `kiln repositories remove ${repo.identity}`,
                opener: remBtn,
              });
              if (!choice.confirmed) return;
              const res = await mutate(`/api/repositories/${encodeURIComponent(repo.identity)}/remove`, {
                reason: choice.reason, request_id: choice.requestId,
              }, "Repository removed from catalog (disk checkouts untouched).");
              if (res) {
                closeInspector();
                if (activeSubPageFor(state.route) === "repositories") void readAction(loadPage)();
                else if (state.route?.page === "overview") void loadOverview();
              }
            });
            nodes.push(remBtn);
          }
        } else {
          nodes.push(element("p", "Read-only principal: mirror attachment and catalog removal require operator access.", "small"));
        }

        body.replaceChildren(...nodes);
        return;
      }
    } catch (error) {
      if (state.inspector.open && state.inspector.id === id && !isAborted(error)) {
        body.replaceChildren(errorState(`Unavailable: ${explain(error)}`));
      }
    }
  }
  function syncInspectorFromRoute(route) {
    const raw = route?.inspect || "";
    const panel = $("slide-over-inspector");
    const backdrop = $("slide-over-backdrop");
    if (!panel || !backdrop) return;
    if (!raw) {
      stopPanePolling();
      state.inspector.open = false;
      state.inspector.kind = "";
      state.inspector.id = "";
      panel.hidden = true;
      backdrop.hidden = true;
      $("slide-over-body")?.replaceChildren();
      return;
    }
    const idx = raw.indexOf(":");
    const kind = raw.slice(0, idx);
    const id = raw.slice(idx + 1);
    if (state.inspector.open && state.inspector.kind === kind && state.inspector.id === id) return;
    state.inspector.open = true;
    state.inspector.kind = kind;
    state.inspector.id = id;
    panel.hidden = false;
    backdrop.hidden = false;
    void renderSlideOver(kind, id);
  }
  $("slide-over-close")?.addEventListener("click", () => closeInspector());
  $("slide-over-backdrop")?.addEventListener("click", () => closeInspector());
  $("slide-over-prev")?.addEventListener("click", () => stepInspector(-1));
  $("slide-over-next")?.addEventListener("click", () => stepInspector(1));
  $("confirm-cli-copy")?.addEventListener("click", () => {
    const cmd = $("confirm-cli-cmd")?.textContent || "";
    if (cmd && $("confirm-cli-status")) ui.copyText(cmd, $("confirm-cli-status"), "CLI command copied.");
  });
  $("global-command-trigger")?.addEventListener("click", () => { $("work-command-open")?.click(); });
  document.addEventListener("visibilitychange", () => {
    if (!state.inspector.open || state.inspector.kind !== "attempt" || !state.inspector.id) return;
    if (document.visibilityState === "visible") {
      const pre = $("slide-over-pane-output");
      const status = $("slide-over-pane-status");
      if (pre && status) void loadAttemptPane(state.inspector.id, pre, status);
    } else {
      stopPanePolling();
      if ($("slide-over-pane-status")) $("slide-over-pane-status").textContent = "Pane tail paused (tab hidden).";
    }
  });
  const CHORD_ROUTES = Object.freeze({
    f: { page: "overview", tab: "" },
    a: { page: "inbox", tab: "attention" },
    d: { page: "cairn", tab: "cairn" },
    r: { page: "runs", tab: "runs" },
    c: { page: "code-review", tab: "" },
    m: { page: "fleet", tab: "fleet" },
  });
  document.addEventListener("keydown", (event) => {
    if (!state.session || event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (document.querySelector("dialog[open]")) return;
    const target = event.target;
    const isEditable = target instanceof Element && Boolean(target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']"));
    if (state.inspector.open) {
      if (event.key === "Escape") { event.preventDefault(); closeInspector(); return; }
      if (!isEditable && event.key === "[") { event.preventDefault(); stepInspector(-1); return; }
      if (!isEditable && event.key === "]") { event.preventDefault(); stepInspector(1); return; }
    }
    if (isEditable) return;
    if (state.chordPrefix === "g") {
      clearTimeout(state.chordTimer);
      state.chordPrefix = "";
      state.chordTimer = null;
      const dest = CHORD_ROUTES[event.key.toLowerCase()];
      if (dest) { event.preventDefault(); navigate(dest); }
      return;
    }
    if (event.key === "g" && !event.shiftKey) {
      state.chordPrefix = "g";
      clearTimeout(state.chordTimer);
      state.chordTimer = setTimeout(() => { state.chordPrefix = ""; state.chordTimer = null; }, CHORD_TIMEOUT_MS);
    }
  });
  window.KilnFactory = Object.freeze({
    openInspector,
    closeInspector,
    stepInspector,
    confirmControl,
    mutate,
    api,
    subscribeEvents,
    eventsState: () => state.events,
    factoryState: () => state.factory,
    canOperate,
    navigate,
    route: () => state.route,
    refreshOverview: () => (state.route?.page === "overview" ? loadOverview() : Promise.resolve()),
    timeline: Object.freeze({ classifyCausalEvent, renderCausalTimeline, renderActivity, renderEvidence }),
    failureDigest: Object.freeze({ build: buildFailureDigest, normalizeSignature: normalizeErrorSignature, render: renderFailureDigest, jump: jumpToDigestNode }),
  });
  // r[impl kiln.work.authority] r[impl kiln.work.change]
  window.KilnWork = Object.freeze({
    navigate, element, notice, runLink,
    api: async (target, options) => {
      if (target === "/api/work") state.workCanWrite = false;
      const result = await api(target, options);
      if (target === "/api/work") state.workCanWrite = result?.can_write === true;
      return result;
    },
    route: () => state.route,
    session: () => state.session ? { principal: state.principal?.id || "", can_write: state.workCanWrite } : null,
    changeEvidence: (record) => window.KilnWorkEvidence.read(record),
    command: async (command) => {
      if (!state.session || !state.workCanWrite) throw new Error("This principal has no admitted work-write capability. Nothing was sent.");
      if (state.mutation) throw new Error("Another admitted mutation is in progress. Nothing was sent.");
      const epoch = state.epoch;
      setMutation(true);
      try { return await api("/api/work/commands", { method: "POST", body: command }); }
      catch (error) {
        if (!isAborted(error) && epoch === state.epoch) notice(`${explain(error)} Request ${command.request_id}. No retry was sent; a timeout may still have applied. Refresh exact native state before another action.`);
        throw error;
      } finally { if (epoch === state.epoch) setMutation(false); }
    },
  });
  window.addEventListener("kiln-work-ready", () => {
    if (state.session && ["work", "planning", "inbox", "triage", "workviews"].includes(activeSubPageFor(state.route))) void readAction(loadPage)();
  });
  // Restoration rechecks network authorization when signed out; pagehide preserves active sessions across sleep/navigation.
  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) return;
    if (!state.session) void discoverNetworkAccess();
    else void readAction(refreshAll)();
  });
  void discoverNetworkAccess();
})();
