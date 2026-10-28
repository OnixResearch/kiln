"use strict";

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
  const GRAPH_MIN_ZOOM = 0.35;
  const GRAPH_MAX_ZOOM = 1.5;
  const GRAPH_ZOOM_STEP = 0.15;
  const localTimeFormat = new Intl.DateTimeFormat(undefined, {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit", timeZoneName: "short",
  });
  const localTimeZone = localTimeFormat.resolvedOptions().timeZone;
  const compactTimeFormat = new Intl.DateTimeFormat(undefined, {
    year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZoneName: "short",
  });
  const $ = (id) => document.getElementById(id);
  const state = {
    session: null, token: null, epoch: 0, requests: new Set(), urls: new Set(), urlTimers: new Set(),
    projects: { available: [], enrolled: [] }, runs: [], selected: null, run: null,
    observedAt: 0, connected: false, liveVersion: "0", liveTimer: null,
    liveController: null, liveGeneration: 0, liveFailures: 0,
    logTimer: null, logGeneration: 0, logBusy: false, logNext: "0", logKey: "",
    artifactsGeneration: 0, confirm: null, mutation: false, detailGeneration: 0,
    principal: null, permissions: {}, route: null, nextCursor: null, total: 0,
    runsGeneration: 0, pageGeneration: 0, refsGeneration: 0,
    endpointGeneration: new Map(),
    runsError: "",
    cobsNextCursor: null,
    graphRun: null, graphZoom: 1,
    workCanWrite: false,
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
  function connection(message, healthy) {
    state.connected = healthy;
    $("connection-status").textContent = message;
    $("connection-status").dataset.state = healthy ? "connected" : "stale";
    updateControlButtons();
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

  async function api(target, { method = "GET", body, kind = "json", controller = new AbortController(), bootstrap = false } = {}) {
    if (!state.session && !bootstrap) throw new DOMException("Signed out", "AbortError");
    const epoch = state.epoch;
    const headers = { Accept: kind === "sse" ? "text/event-stream" : kind === "blob" ? "application/octet-stream" : "application/json" };
    if (state.token) headers.Authorization = `Bearer ${state.token}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    state.requests.add(controller);
    const timeout = setTimeout(() => controller.abort(new DOMException("Request timed out", "TimeoutError")), REQUEST_TIMEOUT_MS);
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
    }
  }

  function addFacts(target, entries) {
    target.replaceChildren();
    for (const [label, value] of entries) target.append(element("dt", label), element("dd", text(value)));
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
  }

  function renderRunList() {
    const focused = document.activeElement?.dataset.runId;
    const runs = state.runs;
    $("runs-status").textContent = state.runsError || `${runs.length} shown of ${text(state.total)} matching retained runs · bounded page${state.connected ? "" : " · snapshot may be stale"}`;
    $("runs-next").disabled = !state.nextCursor;
    $("run-list").replaceChildren();
    if (!runs.length && !state.runsError) {
      const empty = element("p", "No runs match these filters. Clear filters or browse another project.", "small");
      const clear = element("button", "Clear run filters");
      clear.type = "button";
      clear.addEventListener("click", () => navigate({ q: "", project: "", outcome: "", source: "", cursor: "" }));
      $("run-list").append(empty, clear);
    }
    for (const run of runs) {
      const button = element("button", undefined, "run-choice");
      button.type = "button";
      button.dataset.runId = run.id;
      if (run.id === state.selected) button.setAttribute("aria-current", "true");
      button.append(element("strong", text(projectLabel(run.project_id)), "run-name"), element("span", `${text(run.outcome)} · ${compactTimestamp(run.created_at_ms)}`, "run-summary"), element("span", `Revision ${shortIdentity(run.source_revision)} · ${duration(run.duration_ms)}`, "run-source"), element("span", shortIdentity(run.id), "run-id"));
      button.title = `${run.id}\nSource: ${run.source_revision}`;
      button.addEventListener("click", () => navigate({ page: "runs", run: run.id, node: "", cursor: state.route.cursor }));
      $("run-list").append(button);
      if (focused === run.id) button.focus({ preventScroll: true });
    }
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
    dismissControl();
    resetLogs();
    state.artifactsGeneration++;
    $("artifact-list").replaceChildren();
    $("artifact-status").textContent = "Loading retained artifact metadata…";
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
  function renderDetail() {
    const run = state.run;
    if (!run) return;
    $("empty-run").hidden = true;
    $("run-detail").hidden = false;
    $("run-title").textContent = text(projectLabel(run.project_id));
    $("run-badges").replaceChildren();
    for (const [label, value] of [["Outcome", run.outcome], ["Phase", run.phase]]) {
      const badge = element("span", `${label}: ${text(value)}`, "state-badge");
      badge.dataset.tone = value === "Success" ? "good" : /unknown|error|fail|denied|unavailable/i.test(text(value)) ? "warning" : "neutral";
      $("run-badges").append(badge);
    }
    const warnings = [];
    if (run.error_code) warnings.push(`Service diagnostic: ${text(run.error_code)}.`);
    if (run.outcome === "Unknown" || /unknown|recover|unavailable/i.test(text(run.phase))) warnings.push("Execution state is uncertain or recovering. This is not a successful run; do not assume provider effects completed.");
    if (!state.connected) warnings.push("Live observation is disconnected. These details may be stale.");
    $("run-warning").textContent = warnings.join(" ");
    $("run-warning").hidden = !warnings.length;
    addFacts($("run-facts"), [["Source revision", shortIdentity(run.source_revision)], ["Created", compactTimestamp(run.created_at_ms)], ["Duration", duration(run.duration_ms)]]);
    addFacts($("run-exact-facts"), [["Run ID", run.id], ["Project", run.project_id], ["Repository", run.repository], ["Source revision", run.source_revision], ["Canonical tree", run.canonical_tree], ["Run revision", run.revision], ["Attempt", run.attempt_id], ["Sequence", run.sequence], ["Parent run", run.parent_run_id], ["Created", timestamp(run.created_at_ms)], ["Started", timestamp(run.started_at_ms)], ["Completed", timestamp(run.completed_at_ms)]]);
    $("execution-identity").textContent = text(run.execution);
    const nodes = Array.isArray(run.nodes) ? run.nodes : [];
    renderGraph(run);
    renderStepTelemetry(run);
    const previousNode = $("log-node").value;
    options($("log-node"), nodes, nodes.length ? null : "No admitted nodes", state.route.node || previousNode);
    if (state.route.node && !nodes.some((node) => node.id === state.route.node)) {
      options($("log-node"), [], `Node not found: ${state.route.node}`, "");
    }
    if (previousNode && previousNode !== $("log-node").value) resetLogs();
    $("read-log").disabled = state.logBusy || !$("log-node").value;
    $("restart-log").disabled = state.logBusy || !$("log-node").value;
    const diagnostic = $("log-stream").value === "diagnostic";
    $("log-search").disabled = diagnostic;
    $("log-search-help").textContent = diagnostic ? "Search is unavailable for native diagnostic streams. Your worker-stream search is kept for stdout/stderr." : "Search retained stdout/stderr from the beginning; changing the search loads matching output.";
    $("follow-logs").disabled = !$("log-node").value;
    $("download-log").disabled = !$("log-node").value;
    updateControlButtons();
  }
  function renderStepTelemetry(run) {
    const target = $("step-telemetry");
    target.replaceChildren();
    const records = Array.isArray(run.step_telemetry) ? run.step_telemetry : [];
    if (!records.length) { target.append(element("p", "Unknown: no worker step measurements were recorded for this execution.", "small")); return; }
    const observed = (value, unit) => Number.isSafeInteger(value) && value >= 0 ? `${value.toLocaleString()} ${unit}` : "Unknown (not recorded)";
    for (const record of records) {
      const observation = record.observation || {};
      const step = observation.step || {};
      const panel = element("article", undefined, "telemetry-step");
      panel.append(element("h4", `${text(observation.binding?.node_id)} · reviewed step ${text(step.step_index)} · sequence ${text(observation.sequence)}`));
      const facts = element("dl", undefined, "facts");
      addFacts(facts, [["Started", timestamp(step.started_at_ms)], ["Completed", timestamp(step.completed_at_ms)], ["Measured elapsed", duration(step.duration_ms)], ["Exit code", step.exit_code], ["Observed CPU", observed(step.cpu_time_us, "µs")], ["Observed peak memory", observed(step.peak_memory_bytes, "bytes")]]);
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
    for (const action of ["cancel", "pause", "resume"]) $("control-" + action).disabled = !canOperate() || state.mutation || !fresh || !(exactIdentity(run) || (action === "cancel" && run?.phase === "queued")) || run.controls?.[action] !== true;
    for (const mode of ["exact", "latest", "failed"]) $("rerun-" + mode).disabled = !canOperate() || state.mutation || !run || (mode === "failed" && !run.nodes?.some((node) => /^(failed|error)$/i.test(node.state)));
    $("control-note").textContent = !canOperate() ? "Read-only principal. Execution mutations are disabled." : run?.phase === "queued" ? "Queued work has no provider execution yet. Cancel removes this exact queued request before dispatch." : !run ? "Select a run to inspect controls." : !exactIdentity(run) ? "Exact control identity unavailable. No approximate identity will be sent." : !fresh ? "Snapshot stale or live observation disconnected. Refresh before control." : "Only advertised capabilities are enabled. Fresh reads and confirmation bind controls to the exact execution.";
  }

  function scheduleLive(delay, generation) {
    clearTimeout(state.liveTimer);
    state.liveTimer = setTimeout(() => live(generation), delay);
  }
  async function live(generation) {
    if (!state.session || generation !== state.liveGeneration) return;
    const controller = new AbortController();
    state.liveController = controller;
    try {
      const source = await api(`/api/live?after=${encodeURIComponent(state.liveVersion)}`, { kind: "sse", controller });
      if (generation !== state.liveGeneration) return;
      let recognized = false;
      for (const frame of source.replace(/\r\n/g, "\n").split(/\n\n/)) {
        let event = "message";
        const data = [];
        for (const line of frame.split("\n")) {
          if (line.startsWith("event:")) event = line.slice(6).trim();
          if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
          if (line.startsWith(":")) recognized = true;
        }
        if (event === "heartbeat") recognized = true;
        if (event !== "snapshot") continue;
        const snapshot = JSON.parse(data.join("\n"));
        if (!Number.isSafeInteger(snapshot.version) || snapshot.version < 0 || !Array.isArray(snapshot.runs)) throw new Error("Invalid live snapshot identity; the previous view may be stale.");
        if (BigInt(snapshot.version) < BigInt(state.liveVersion)) throw new Error("The live version moved backwards. Refresh to re-establish service state.");
        const versionChanged = String(snapshot.version) !== state.liveVersion;
        state.liveVersion = String(snapshot.version);
        connection("Live · authenticated", true);
        const incoming = new Map(snapshot.runs.map((run) => [run.id, run]));
        if (!versionChanged) renderRunList();
        else if (state.route.page === "runs") void readAction(loadRuns)();
        else if (["overview", "workers", "operations", "activity"].includes(state.route.page)) void readAction(loadPage)();
        if (state.selected && incoming.has(state.selected)) adoptRun(incoming.get(state.selected));
        recognized = true;
      }
      if (!recognized) throw new Error("The live endpoint returned no snapshot or heartbeat.");
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
      notice(`Live observation unavailable: ${explain(error)}${stopped ? " Automatic reconnect stopped. Use Refresh & reconnect." : " Read-only reconnect is bounded; no mutations are retried."}`);
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
    const requestId = crypto.randomUUID();
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
  function dismissControl() {
    state.confirm = null;
    if ($("control-dialog").open) $("control-dialog").close();
    $("confirm-facts").replaceChildren();
  }
  async function prepareControl(action) {
    if (!canOperate()) return;
    const id = state.selected;
    try {
      const freshRun = await loadDetail();
      const run = state.run;
      const queuedCancel = action === "cancel" && run?.phase === "queued";
      if (!freshRun || state.selected !== id || (!queuedCancel && !exactIdentity(run)) || run.controls?.[action] !== true || !state.connected) throw new Error("This exact execution no longer advertises the requested control. Nothing was sent.");
      const identity = queuedCancel ? { action } : structuredClone({ action, attempt_id: run.attempt_id, expected_revision: run.revision, expected_sequence: run.sequence, execution: run.execution });
      state.confirm = { id, identity };
      $("confirm-title").textContent = `Confirm ${action} run`;
      $("confirm-control").textContent = `Confirm ${action}`;
      addFacts($("confirm-facts"), [["Run", id], ["Source", run.source_revision], ["Attempt", identity.attempt_id], ["Revision", identity.expected_revision], ["Sequence", identity.expected_sequence], ["Execution", identity.execution]]);
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
    $("log-output").textContent = "";
    $("log-gap").textContent = "";
    $("log-gap").hidden = true;
    $("log-cursor").value = "0";
    $("log-position").textContent = "No log read yet";
    $("log-status").textContent = "Select a node and read actual retained logs. Live worker bytes can arrive before command completion; diagnostics are native provider logs.";
  }
  function cursorValue(value) {
    if (typeof value === "number" && !Number.isSafeInteger(value)) throw new Error("The service returned a cursor that cannot be represented exactly. No approximate cursor was used.");
    const source = String(value);
    if (!/^\d+$/.test(source) || BigInt(source) > 18446744073709551615n) throw new Error("A stream cursor must be an unsigned 64-bit decimal integer.");
    return BigInt(source).toString();
  }
  // r[impl kiln.dashboard.logs.continuation]
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
    try {
      const maximum = state.run?.log_bounds?.maximum_chunk_bytes;
      if (!Number.isSafeInteger(maximum) || maximum <= 0) throw new Error("The run has no valid admitted log-read bound.");
      const readBytes = Math.min(LOG_READ_BYTES, maximum);
      const query = new URLSearchParams({ node_id: node, stream, cursor, max_bytes: String(readBytes) });
      if (search) query.set("q", search);
      const result = await api(`${pathFor(id)}/logs?${query}`);
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
      if (output !== previousOutput) view.textContent = output;
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
      view.scrollTop = followTail ? view.scrollHeight : append ? scrollTop : 0;
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
        if ($("follow-logs").checked && state.session) state.logTimer = setTimeout(() => readLog(true), LOG_FOLLOW_MS);
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

  function logout(message = "Signed out. Private views and pending requests were cleared.") {
    state.epoch++;
    state.session = null;
    state.token = null;
    state.workCanWrite = false;
    window.KilnWorkUI?.clear();
    window.KilnWorkInteraction?.clear();
    window.KilnCodeReview?.clear();
    $("code-review-content").replaceChildren();
    for (const id of ["work-content", "planning-content", "inbox-content", "triage-content", "workviews-content"]) $(id).replaceChildren();
    window.dispatchEvent(new Event("kiln-session-changed"));
    for (const controller of state.requests) controller.abort();
    state.requests.clear();
    clearTimeout(state.liveTimer);
    state.liveGeneration++;
    state.liveController = null;
    state.liveVersion = "0";
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
    for (const id of ["run-list", "run-title", "run-facts", "run-exact-facts", "retained-run-options", "run-badges", "run-warning", "execution-identity", "node-list", "step-telemetry", "artifact-list", "artifact-status", "available-description", "project-description"]) $(id).replaceChildren();
    for (const id of ["available-project", "enrolled-project", "run-filter", "log-node"]) $(id).replaceChildren();
    state.principal = null;
    state.permissions = {};
    state.pageGeneration++;
    state.runsGeneration++;
    state.refsGeneration++;
    state.endpointGeneration.clear();
    state.total = 0; state.nextCursor = null;
    state.runsError = "";
    state.cobsNextCursor = null;
    for (const id of ["cob-list", "cob-detail", "cob-source-status", "cob-status", "cob-repository", "cob-type"]) $(id).replaceChildren();
    $("cob-search").value = "";
    for (const id of ["source-browser-status", "source-log", "source-tree", "source-file", "source-diff"]) $(id).replaceChildren();
    $("source-log-status").textContent = "";
    $("source-log-next").disabled = true;
    $("reviews-list").replaceChildren();
    $("reviews-detail").replaceChildren();
    $("reviews-status").textContent = "";
    $("source-filter").value = "";
    // r[impl kiln.dashboard.workers.view.scenario.cleared]
    $("workers-content").replaceChildren();
    $("worker-search").value = "";
    $("worker-status").value = "";
    $("source-reference").replaceChildren();
    for (const id of ["source-revision", "log-search"]) $(id).value = "";
    for (const id of ["overview-content", "project-cards", "reports-content", "compare-content", "activity-content", "run-activity-content", "evidence-content", "run-evidence-content", "automation-content", "operations-content", "principal-facts", "node-facts", "pipeline-graph", "source-status"]) $(id).replaceChildren();
    for (const id of ["workflow-title", "workflow-caption", "workflow-summary"]) $(id).replaceChildren();
    state.graphRun = null; state.graphZoom = 1;
    $("operator-token").value = "";
    $("workspace").hidden = true;
    $("run-detail").hidden = true;
    $("empty-run").hidden = false;
    $("login-panel").hidden = false;
    $("logout").hidden = true;
    $("auth-status").textContent = "Signed out";
    $("auth-status").removeAttribute("data-state");
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
    $("auth-status").textContent = `${text(state.principal?.label)} · ${text(state.principal?.role)}`;
    $("auth-status").title = $("auth-status").textContent;
    $("auth-status").dataset.state = "connected";
    addFacts($("principal-facts"), [["Principal", state.principal?.label], ["ID", state.principal?.id], ["Role", state.principal?.role], ["Admission", state.session === "network" ? "Trusted network identity" : "Manual bearer (memory only)"]]);
    notice("");
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
  const pages = ["overview", "projects", "workers", "runs", "cobs", "source", "reviews", "code-review", "activity", "evidence", "automation", "operations", "settings", "work", "planning", "inbox", "triage", "workviews"];
  const views = ["pipeline", "logs", "artifacts", "tests", "reviews", "compare", "activity", "evidence"];
  const workerStatuses = { slots_available: "Slots available", busy: "Busy", unknown: "Unknown execution", not_enrolled: "Not enrolled" };
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
    if (!Number.isSafeInteger(ms)) return "Unavailable (not recorded)";
    const date = new Date(ms);
    if (Number.isNaN(date.valueOf())) return "Unavailable (not recorded)";
    return `${localTimeFormat.format(date)} (local · ${localTimeZone}) · ${date.toISOString()} (UTC)`;
  }
  function compactTimestamp(ms) {
    if (!Number.isSafeInteger(ms)) return "Unavailable (not recorded)";
    const date = new Date(ms);
    return Number.isNaN(date.valueOf()) ? "Unavailable (not recorded)" : compactTimeFormat.format(date);
  }
  function duration(ms) { return Number.isSafeInteger(ms) && ms >= 0 ? `${(ms / 1000).toFixed(3)} s` : "Unavailable (not recorded)"; }
  // r[impl kiln.dashboard.source.view.scenario.restore]
  function readRoute() {
    const params = new URLSearchParams(location.search);
    // Accept catalog-style fragment links as well, but never read credentials from either location.
    if (location.hash.startsWith("#page=")) for (const [key, value] of new URLSearchParams(location.hash.slice(1))) if (!params.has(key)) params.set(key, value);
    return {
      page: pages.includes(params.get("page")) ? params.get("page") : "overview",
      view: views.includes(params.get("view")) ? params.get("view") : "pipeline",
      run: params.get("run") || "", node: params.get("node") || "",
      stream: ["stdout", "stderr", "diagnostic"].includes(params.get("stream")) ? params.get("stream") : "stdout",
      q: params.get("q") || "", project: params.get("project") || "", outcome: params.get("outcome") || "", source: params.get("source") || "",
      cursor: params.get("cursor") || "", activity_cursor: params.get("activity_cursor") || "", order: params.get("order") === "asc" ? "asc" : "desc", other: params.get("other") || "",
      cob_repository: params.get("cob_repository") || "", cob_type: params.get("cob_type") || "", cob_q: params.get("cob_q") || "", cob_cursor: params.get("cob_cursor") || "",
      cob: params.get("cob") || "", cob_repo: params.get("cob_repo") || "", cob_kind: params.get("cob_kind") || "",
      src: params.get("src") || "", rev: params.get("rev") || "", spath: params.get("spath") || "", sfile: params.get("sfile") || "", sq: (params.get("sq") || "").slice(0, 256),
      receipt: (params.get("receipt") || "").slice(0, 128),
      worker_q: (params.get("worker_q") || "").slice(0, 256), worker_status: Object.hasOwn(workerStatuses, params.get("worker_status")) ? params.get("worker_status") : "", worker: params.get("worker") || "",
      work_id: (params.get("work_id") || "").slice(0, 128), work_kind: (params.get("work_kind") || "").slice(0, 32), work_q: (params.get("work_q") || "").slice(0, 256),
      work_status: (params.get("work_status") || "").slice(0, 128), work_assignee: (params.get("work_assignee") || "").slice(0, 128), work_project: (params.get("work_project") || "").slice(0, 128),
      work_view: (params.get("work_view") || "").slice(0, 128), work_layout: ["board", "list"].includes(params.get("work_layout")) ? params.get("work_layout") : "", work_group: (params.get("work_group") || "").slice(0, 32), work_sort: (params.get("work_sort") || "").slice(0, 32),
      review_change: (params.get("review_change") || "").slice(0, 128), review_revision: (params.get("review_revision") || "").slice(0, 128), review_base: (params.get("review_base") || "").slice(0, 128),
      review_file: (params.get("review_file") || "").slice(0, 1024), review_side: ["base", "revision"].includes(params.get("review_side")) ? params.get("review_side") : "", review_line: (params.get("review_line") || "").slice(0, 16),
      review_whitespace: ["exact", "ignore_all", "ignore_change", "ignore_eol"].includes(params.get("review_whitespace")) ? params.get("review_whitespace") : "",
      review_q: (params.get("review_q") || "").slice(0, 256), review_view: ["all", "awaiting_review", "owned_needs_revision", "unresolved"].includes(params.get("review_view")) ? params.get("review_view") : "", review_status: ["open", "abandoned", "submitted"].includes(params.get("review_status")) ? params.get("review_status") : "",
      review_offset: /^[0-9]{1,8}$/.test(params.get("review_offset") || "") ? params.get("review_offset") : "",
      review_notification_offset: /^[0-9]{1,8}$/.test(params.get("review_notification_offset") || "") ? params.get("review_notification_offset") : "",
      review_operation_offset: /^[0-9]{1,8}$/.test(params.get("review_operation_offset") || "") ? params.get("review_operation_offset") : "",
      theme: ["system", "light", "dark"].includes(params.get("theme")) ? params.get("theme") : "system", density: params.get("density") === "compact" ? "compact" : "comfortable"
    };
  }
  function navigate(patch) {
    const route = { ...state.route, ...patch };
    if (Object.hasOwn(patch, "run") && patch.run !== state.route.run && !Object.hasOwn(patch, "activity_cursor")) route.activity_cursor = "";
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(route)) if (value) params.set(key, value);
    history.pushState(null, "", `${location.pathname}?${params}`);
    void readAction(restoreRoute)();
  }
  async function restoreRoute() {
    const previous = state.route;
    state.route = readRoute();
    if (previous?.page === "code-review" && state.route.page !== "code-review") window.KilnCodeReview?.clear();
    const route = state.route;
    document.documentElement.dataset.theme = route.theme;
    document.documentElement.dataset.density = route.density;
    $("app-theme").value = route.theme;
    $("app-density").value = route.density;
    for (const node of document.querySelectorAll("[data-panel]")) node.hidden = node.dataset.panel !== route.page;
    for (const node of document.querySelectorAll("[data-page]")) { if (node.dataset.page === route.page) node.setAttribute("aria-current", "page"); else node.removeAttribute("aria-current"); }
    for (const group of document.querySelectorAll(".app-navigation details")) {
      if (group.querySelector('[aria-current="page"]')) group.open = true;
    }
    for (const node of document.querySelectorAll("[data-run-panel]")) node.hidden = node.dataset.runPanel !== route.view;
    for (const node of document.querySelectorAll("[data-view]")) { if (node.dataset.view === route.view) node.setAttribute("aria-current", "page"); else node.removeAttribute("aria-current"); }
    const title = { work: "Issues", inbox: "My inbox", planning: "Planning", triage: "Requests & triage", workviews: "Saved views", projects: "CI projects", cobs: "Raw collaboration history", overview: "CI overview", "code-review": "Code review", reviews: "Patchgate receipts" }[route.page] || route.page[0].toUpperCase() + route.page.slice(1);
    $("workspace-title").textContent = title;
    $("app-breadcrumb").textContent = title;
    document.title = `Kiln · ${title}`;
    $("observation-note").textContent = {
      overview: "Latest per project, failures first. Counts cover retained runs only.",
      projects: "Choose a project to start a source-bound run, or browse its retained history.",
      workers: "Inspect configured execution targets and admission slots. Availability is not a health check.",
      runs: "Filter retained runs, select an execution, then inspect its pipeline, logs and evidence.",
      cobs: "Browse local collaboration records. Raw operation history is not verified lifecycle state.",
      source: "Choose a repository and revision to browse files and changes without modifying source.",
      reviews: "Inspect retained review decisions. Producer verdicts do not grant acceptance authority.",
      "code-review": "Review exact revisions, publish discussions and approvals, and inspect every submit requirement. CI evidence alone is not merge authority.",
      activity: "Browse retained canonical events and attributable audit decisions.",
      evidence: "Choose a retained run to inspect its source, execution and recorded evidence.",
      automation: "Inspect registered triggers, schedules and delivery outcomes. Only admitted registrations are editable.",
      operations: "Inspect capacity, queued work and retention limits. No automatic pruning is performed.",
      work: "Capture an issue, decide what happens next, and follow its discussion and linked CI results.",
      planning: "Organize issues into projects, milestones and cycles. Track larger goals with initiatives.",
      inbox: "Assignments, mentions, watched updates and due reminders. Opening an item does not mark it read.",
      triage: "Review incoming requests, keep their original source and decide what to take on next.",
      workviews: "Return to private or shared filters, boards and favorites saved in this workspace.",
      settings: "Review your access and adjust appearance. Credentials are never saved by this page."
    }[route.page];
    if (previous?.page !== route.page) {
      notice("");
      $("workspace-title").tabIndex = -1;
      $("workspace-title").focus({ preventScroll: true });
      window.scrollTo(0, 0);
    }
    for (const [id, key] of [["run-search", "q"], ["run-filter", "project"], ["run-outcome", "outcome"], ["run-source", "source"], ["run-order", "order"], ["compare-other", "other"], ["evidence-run", "run"], ["log-stream", "stream"]]) $(id).value = route[key];
    if (route.source || route.order !== "desc") $("run-more-filters").open = true;
    $("worker-search").value = route.worker_q;
    $("worker-status").value = route.worker_status;
    for (const [id, value] of [["run-filter", route.project], ["run-outcome", route.outcome]]) {
      const select = $(id);
      if (value && ![...select.options].some((option) => option.value === value)) { const option = element("option", `Unavailable selection: ${value}`); option.value = value; select.append(option); }
      select.value = value;
    }
    setMenu(false);
    if (!state.session) return;
    if (previous && (previous.page !== route.page || previous.run !== route.run || previous.node !== route.node || previous.view !== route.view || previous.stream !== route.stream)) resetLogs();
    state.pageGeneration++;
    if (route.page === "runs") {
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
    if (route.page === "runs" && route.run && previous?.run !== route.run && state.run) {
      $("run-title").tabIndex = -1;
      $("run-title").focus({ preventScroll: true });
      $("run-title").scrollIntoView({ block: "start" });
    } else if (route.page === "runs" && route.run && previous?.view !== route.view) {
      const heading = document.querySelector(`[data-run-panel="${route.view}"] h2`);
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
      heading.scrollIntoView({ block: "start" });
    }
  }
  function runLink(id, label = id, view = "pipeline") {
    const link = element("a", text(label));
    const params = new URLSearchParams({ page: "runs", run: id, view });
    link.href = `?${params}`;
    link.addEventListener("click", (event) => { if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); navigate({ page: "runs", run: id, view, node: "" }); } });
    return link;
  }
  function section(title, parent, className = "panel") { const node = element("section", undefined, className); node.append(element("h3", title)); parent.append(node); return node; }
  function renderRecorded(parent, value, depth = 0) {
    if (value === null || value === undefined) { parent.append(element("p", "Unavailable (not recorded)", "small")); return; }
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
      if (item && typeof item === "object") {
        const details = element("details"); details.append(element("summary", `${key.replaceAll("_", " ")} · recorded facts`)); cell.append(details);
        if (depth < 7) renderRecorded(details, item, depth + 1); else details.append(element("pre", text(item), "identity"));
      } else if ((key === "run_id" || key === "parent_run_id") && typeof item === "string") cell.append(runLink(item));
      else if (key.endsWith("_at_ms") || key === "next_due_ms" || key === "reserved_at_ms") cell.textContent = timestamp(item);
      else if (key === "duration_ms") cell.textContent = duration(item);
      else cell.textContent = text(item);
    }
  }
  async function renderEndpoint(target, id, render = renderRecorded) {
    const generation = state.pageGeneration;
    const requestGeneration = (state.endpointGeneration.get(id) || 0) + 1;
    state.endpointGeneration.set(id, requestGeneration);
    const run = state.route.run;
    const parent = $(id);
    parent.replaceChildren(element("p", "Loading admitted records…", "small"));
    try {
      const result = await api(target);
      if (generation !== state.pageGeneration || requestGeneration !== state.endpointGeneration.get(id) || run !== state.route.run) return;
      parent.replaceChildren(); render(parent, result); return true;
    } catch (error) {
      if (!isAborted(error) && generation === state.pageGeneration && requestGeneration === state.endpointGeneration.get(id)) parent.replaceChildren(element("p", `Unavailable: ${explain(error)} No presence, absence or success is inferred.`, "warning"));
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
        option.label = `${projectLabel(run.project_id)} · ${run.outcome} · ${compactTimestamp(run.created_at_ms)} · ${shortIdentity(run.source_revision)}`;
        list.append(option);
      }
    } catch (error) {
      if (!isAborted(error) && state.route === route) notice(`Run suggestions unavailable: ${explain(error)} You can still enter an exact retained run ID.`);
    }
  }
  async function loadPage() {
    const route = state.route;
    if (route.page === "code-review") {
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
    if (["work", "planning", "inbox", "triage", "workviews"].includes(route.page)) {
      const parent = $(route.page + "-content");
      if (!window.KilnWorkUI) { parent.replaceChildren(element("p", "Work interface is loading.", "small")); return; }
      await window.KilnWorkUI.render(parent, route);
      return;
    }
    if (route.page === "overview") await renderEndpoint("/api/overview", "overview-content", renderOverview);
    if (route.page === "projects") { renderProjects(); await loadRefs(); }
    if (route.page === "workers") await renderEndpoint("/api/workers", "workers-content", renderWorkers);
    if (route.page === "activity") await renderEndpoint(`/api/activity?${activityQuery()}`, "activity-content", renderActivity);
    if (route.page === "evidence") {
      void loadRunChoices();
      if (route.run) await renderEndpoint(`${pathFor(route.run)}/evidence`, "evidence-content", renderEvidence);
      else $("evidence-content").replaceChildren(element("p", "Enter a retained run ID. No evidence from another attempt is substituted.", "small"));
    }
    if (route.page === "automation") await renderEndpoint("/api/automation", "automation-content", renderAutomation);
    if (route.page === "operations") await renderEndpoint("/api/operations", "operations-content", renderOperations);
    if (route.page === "cobs") await loadCobs();
    if (route.page === "source") await loadSource();
    if (route.page === "reviews") await loadReviewsPage();
    if (route.page === "runs") await loadRunView();
  }
  async function loadRunView() {
    const route = state.route;
    if (!route.run || route.page !== "runs") return;
    if (route.view === "tests") await renderEndpoint(`${pathFor(route.run)}/reports`, "reports-content", renderReports);
    if (route.view === "reviews") await renderEndpoint(`${pathFor(route.run)}/reports`, "reviews-content", renderReviews);
    if (route.view === "compare") {
      void loadRunChoices();
      if (route.other) await renderEndpoint(`${pathFor(route.run)}/compare?${new URLSearchParams({ other: route.other })}`, "compare-content", renderComparison);
      else $("compare-content").replaceChildren(element("p", "Choose another retained run. Missing reports or historical durations remain unavailable, not zero.", "small"));
    }
    if (route.view === "activity") await renderEndpoint(`${pathFor(route.run)}/activity?${activityQuery()}`, "run-activity-content", renderActivity);
    if (route.view === "evidence") await renderEndpoint(`${pathFor(route.run)}/evidence`, "run-evidence-content", renderEvidence);
    if (route.view === "artifacts") await loadArtifacts();
    if (route.view === "logs" && !state.logKey && !state.logBusy) await readLog(false);
  }
  // r[impl kiln.dashboard.workers.view]
  function workerMetrics(parent, capacity, targets) {
    const grid = element("div", undefined, "inventory-grid worker-metrics");
    const entries = [["Configured project slots", capacity.configured_slots], ["Enrolled project slots", capacity.enrolled_slots], ["Active / occupied slots", capacity.active], ["Free project slots", capacity.free_slots], ["Queued runs", capacity.queued]];
    if (targets !== undefined) entries.unshift(["Configured targets", targets]);
    for (const [label, value] of entries) {
      const card = element("article", undefined, "inventory-card");
      card.append(element("span", label, "metric-label"), element("strong", text(value), "metric-value"));
      grid.append(card);
    }
    parent.append(grid);
  }
  function workerStatusBadge(status) {
    const badge = element("span", workerStatuses[status] || "Unknown slot status", "state-badge");
    if (status === "unknown") badge.dataset.tone = "warning";
    return badge;
  }
  // r[impl kiln.dashboard.workers.view.scenario.restore]
  function renderWorkers(parent, result) {
    if (result?.schema !== "kiln.web-workers.v1" || result.bounded !== true || typeof result.storage_failed !== "boolean" || !result.summary || !Array.isArray(result.workers) || result.workers.some((worker) => !worker.capacity || !Array.isArray(worker.projects) || worker.projects.some((project) => !Array.isArray(project.active_run_ids)))) throw new Error("Worker inventory response is invalid. Target capacity is unavailable.");
    const route = state.route;
    const summary = section("Bounded target snapshot", parent);
    summary.append(element("p", `Snapshot ${text(result.version)} · complete configured target inventory within service limits · liveness unknown`, "small"));
    workerMetrics(summary, result.summary, result.summary.targets);
    const query = route.worker_q.toLowerCase();
    const matches = result.workers.filter((worker) => (!route.worker_status || worker.status === route.worker_status) && (!query || [worker.id, worker.provider_profile_id, worker.platform, ...worker.projects.flatMap((project) => [project.id, project.label])].some((value) => String(value ?? "").toLowerCase().includes(query))));
    const workspace = element("div", undefined, "workspace-grid workers-grid");
    parent.append(workspace);
    const inventory = element("section", undefined, "panel");
    inventory.setAttribute("aria-label", "Configured worker target inventory");
    inventory.append(element("h2", "Target inventory"), element("p", `${matches.length} of ${result.workers.length} configured targets match · filters do not change exact selection`, "small"));
    const list = element("div", undefined, "run-list worker-list");
    inventory.append(list);
    workspace.append(inventory);
    for (const worker of matches) {
      const choice = element("a", undefined, "run-choice cob-choice worker-choice");
      const patch = { page: "workers", worker: worker.id };
      choice.href = `?${new URLSearchParams({ ...route, ...patch })}`;
      if (route.worker === worker.id) choice.setAttribute("aria-current", "true");
      choice.append(element("strong", text(worker.provider_profile_id)), element("span", text(worker.platform), "small"), element("span", text(worker.id), "run-source"), workerStatusBadge(worker.status), element("span", `${text(worker.capacity.free_slots)} configured project slots free · ${text(worker.capacity.active)} occupied · ${text(worker.capacity.queued)} queued`, "small"), element("span", `${worker.projects.length} assigned projects · heartbeat ${text(worker.liveness || "unknown")}`, "small"));
      choice.addEventListener("click", (event) => { if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); navigate(patch); } });
      list.append(choice);
    }
    if (!matches.length) list.append(element("p", result.workers.length ? "No targets match these metadata and slot-status filters. Clear filters to see the full bounded inventory; the selected target remains independent." : "No workflow targets are configured in this bounded snapshot. This is not a discovery of physical workers.", "small"));
    const inspector = element("section", undefined, "panel worker-detail");
    inspector.setAttribute("aria-label", "Exact worker target inspector");
    const heading = element("div", undefined, "section-heading");
    heading.append(element("h2", route.worker ? "Selected target" : "Select a target"));
    inspector.append(heading);
    workspace.append(inspector);
    if (!route.worker) { inspector.append(element("p", "Choose a configured target to inspect its assigned projects and exact execution observations. No target is selected automatically.", "small")); return; }
    const clear = element("button", "Clear selection");
    clear.type = "button";
    clear.addEventListener("click", () => navigate({ worker: "" }));
    heading.append(clear);
    const selected = result.workers.find((worker) => worker.id === route.worker);
    if (!selected) { inspector.append(element("p", "Target not found in this configured snapshot. No other target was substituted.", "warning"), element("p", route.worker, "worker-identity")); return; }
    if (!matches.includes(selected)) inspector.append(element("p", "This exact target is outside the current list filters. Its inspection remains selected.", "small"));
    renderWorkerDetail(inspector, selected);
  }
  function renderWorkerDetail(parent, worker) {
    const badges = element("div", undefined, "badges");
    badges.append(workerStatusBadge(worker.status), element("span", `Heartbeat liveness: ${text(worker.liveness || "unknown")}`, "state-badge"));
    parent.append(badges);
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [["Kiln target ID", worker.id], ["Provider profile", worker.provider_profile_id], ["Platform", worker.platform], ["Assigned projects", worker.projects.length], ["Enrolled projects", worker.projects.filter((project) => project.enrolled).length]]);
    parent.append(facts);
    workerMetrics(parent, worker.capacity);
    parent.append(element("p", "Each enrolled project contributes one admission slot. Ambiguous nonterminal executions retain their occupied slots. Availability is not a heartbeat or a provider concurrency guarantee.", "small"));
    if (worker.observed_capacity) {
      const observed = element("dl", undefined, "facts");
      addFacts(observed, [["Heartbeat observed slots", worker.observed_capacity.observed_slots], ["Heartbeat active slots", worker.observed_capacity.active_slots], ["Heartbeat free slots", worker.observed_capacity.free_slots], ["Last heartbeat", timestamp(worker.observed_capacity.last_seen_at_ms)]]);
      parent.append(observed, element("p", "Expired observations contribute no free slots. Heartbeats observe availability, not physical capacity guarantees.", "small"));
    }
    const projects = section("Assigned projects & current runs", parent, "view-stack");
    for (const project of worker.projects) {
      const card = section(text(project.label || project.id), projects, "record-card");
      const metadata = element("dl", undefined, "facts");
      addFacts(metadata, [["Project ID", project.id], ["Enrollment", project.enrolled ? "Enrolled" : "Not enrolled"], ["Queued runs", project.queued]]);
      card.append(metadata);
      const runs = element("div", undefined, "view-stack worker-run-links");
      for (const id of project.active_run_ids) runs.append(runLink(id, `Active / occupying: ${id}`));
      if (!project.active_run_ids.length) runs.append(element("p", "No active execution is retained for this project slot. Physical liveness remains unknown.", "small"));
      card.append(runs);
    }
    if (!worker.projects.length) projects.append(element("p", "No assigned projects in this snapshot.", "small"));
    const latest = section("Latest execution observation", parent, "record-card");
    const run = worker.latest_run;
    if (!run) { latest.append(element("p", "Unknown: no retained execution observation. Queued requests are not execution evidence; timing, sequence and outcome are unknown.", "small")); return; }
    latest.append(runLink(run.id, `Inspect exact run: ${run.id}`));
    const observation = element("dl", undefined, "facts");
    const unknown = (value) => value === null || value === undefined ? "Unknown (not recorded)" : value;
    const recordedTime = (value) => value === null || value === undefined ? "Unknown (not recorded)" : timestamp(value);
    const elapsed = Number.isSafeInteger(run.started_at_ms) && Number.isSafeInteger(run.completed_at_ms) && run.completed_at_ms >= run.started_at_ms ? duration(run.completed_at_ms - run.started_at_ms) : "Unknown (not recorded)";
    addFacts(observation, [["Project ID", run.project_id], ["Phase", unknown(run.phase)], ["Outcome", unknown(run.outcome)], ["Sequence", unknown(run.sequence)], ["Error code", unknown(run.error_code)], ["Created", recordedTime(run.created_at_ms)], ["Started", recordedTime(run.started_at_ms)], ["Completed", recordedTime(run.completed_at_ms)], ["Execution duration", elapsed]]);
    latest.append(observation, element("p", "Latest retained nonqueued execution, not necessarily an active run. A newer queued request never replaces this execution evidence.", "small"));
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
      const when = element("span", compactTimestamp(object.recorded_at_ms), "cob-when");
      when.title = timestamp(object.recorded_at_ms);
      footer.append(oid, when, element("span", `${shortAuthor(object.author)} · ${object.ref_tips.length} ref tip(s)`, "cob-when"));
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
        button.addEventListener("click", () => navigate({ page: "source", src: match.id, rev: revision, spath: "", sfile: "", sq: "", cob: "", cob_repo: "", cob_kind: "", cob_type: "", cob_cursor: "", cob_repository: "", cob_q: "" }));
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
    parent.append(element("p", "Recorded actions, ordered by operation identity—not a current state or chosen concurrent winner. Authors, status and decision claims are unverified. Embedded text and URLs are inert.", "warning"));
    for (const operation of object.operations) {
      const card = section(`Operation ${shortOid(operation.id)}`, parent, "record-card cob-operation");
      card.append(element("p", `${shortAuthor(operation.author_name)} · ${timestamp(operation.recorded_at_ms)} · unverified author`, "small"));
      for (const action of operation.actions || []) renderCobAction(card, identity.type_name, action);
      const framing = element("details", undefined, "cob-framing"); framing.append(element("summary", "Exact operation metadata"));
      const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["Exact operation ID", operation.id], ["Recorded", timestamp(operation.recorded_at_ms)], ["Git author (unverified)", `${text(operation.author_name)} <${text(operation.author_email)}>`], ["Git parents", operation.git_parents], ["History parents", operation.history_parents], ["Commit message", operation.message]]); framing.append(metadata); card.append(framing);
    }
    for (const boundary of result.non_claims || []) parent.append(element("p", boundary, "small"));
    void attachPatchSources(parent, object);
  }

  // r[impl kiln.dashboard.source.view]
  function sourceQuery(values) {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(values)) if (value) query.set(key, value);
    return query;
  }
  function sourceBreadcrumb(parent, route) {
    const crumb = element("div", undefined, "source-breadcrumb");
    if (route.spath) {
      const up = element("button", "↑ up"); up.type = "button";
      up.addEventListener("click", () => navigate({ spath: route.spath.split("/").slice(0, -1).join("/"), sfile: "", sq: "" }));
      crumb.append(up);
    }
    const root = element("button", "root"); root.type = "button";
    root.addEventListener("click", () => navigate({ spath: "", sfile: "", sq: "" }));
    crumb.append(root);
    let prefix = "";
    for (const part of route.spath.split("/").filter(Boolean)) {
      prefix = prefix ? `${prefix}/${part}` : part;
      const separator = element("span", "/", "source-separator");
      const button = element("button", part); button.type = "button";
      const target = prefix;
      button.addEventListener("click", () => navigate({ spath: target, sfile: "", sq: "" }));
      crumb.append(separator, button);
    }
    parent.append(crumb);
  }
  function renderSourceEntries(parent, route, tree) {
    const heading = element("p", `${text(tree.label)} · ${text(tree.kind)} · ${tree.revision_scope === "revision" ? `revision ${text(tree.revision)}` : `channel ${text(tree.channel)}`}`, "small");
    parent.append(heading);
    sourceBreadcrumb(parent, route);
    const needle = route.sq.toLowerCase();
    const entries = needle ? tree.entries.filter((entry) => entry.path.toLowerCase().includes(needle)) : tree.entries;
    parent.append(element("p", `${entries.length} of ${tree.entries.length} entr${tree.entries.length === 1 ? "y" : "ies"} at ${route.spath || "root"}${needle ? ` matching “${route.sq}”` : ""} · bounded listing`, "small"));
    const list = element("div", undefined, "source-entries");
    for (const entry of entries) {
      const button = element("button", undefined, "source-entry");
      button.type = "button";
      if (entry.path === route.sfile) button.setAttribute("aria-current", "true");
      button.append(element("span", entry.kind === "directory" ? "dir" : entry.kind, "source-kind"), element("span", entry.name, "source-name"), element("span", entry.bytes === null || entry.bytes === undefined ? "" : text(entry.bytes), "source-size"));
      const patch = entry.kind === "directory" ? { spath: entry.path, sfile: "", sq: "" } : { sfile: entry.path };
      button.addEventListener("click", () => navigate(patch));
      list.append(button);
    }
    parent.append(list);
    if (tree.entries.length && !entries.length) parent.append(element("p", "No entries match this directory filter. Clear the filter to see this bounded listing; this is not a repository-wide search.", "small"));
    if (route.sfile && !entries.some((entry) => entry.path === route.sfile)) parent.append(element("p", "The selected file is outside this filtered directory listing. Its exact content is inspected independently.", "small"));
    if (!tree.entries.length) parent.append(element("p", "This directory is empty in the selected revision.", "small"));
  }
  function renderSourceFile(parent, file) {
    const heading = section(`${text(file.path)}`, parent, "record-card");
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [["Repository", file.label], ["Kind", file.kind], ["Scope", file.revision_scope], ["Revision", file.revision], ["Object", file.oid], ["Bytes", file.bytes], ["Binary", file.binary ? "yes" : "no"]]);
    heading.append(facts);
    if (file.binary) heading.append(element("p", "This object is binary or not valid UTF-8. No text is rendered and nothing is decoded.", "small"));
    else heading.append(sourceLines(file.text, "source-content"));
  }
  function sourceLines(textValue, className) {
    const pre = element("pre", undefined, className);
    const lines = String(textValue ?? "").split("\n");
    if (lines.length && lines[lines.length - 1] === "") lines.pop();
    for (const line of lines) pre.append(element("span", line, "line"), document.createTextNode("\n"));
    return pre;
  }
  function renderSourceDiff(parent, diff) {
    const card = section(`${text(diff.files_changed)} file(s) changed · +${text(diff.insertions)} −${text(diff.deletions)}`, parent, "record-card");
    card.append(element("p", `Revision ${text(diff.revision)} against ${text(diff.parent)} · ${diff.truncated ? "bounded preview, not the complete diff" : "complete diff within bounds"}`, "small"));
    const pre = element("pre", undefined, "source-content diff");
    for (const line of text(diff.diff).split("\n")) {
      const kind = line.startsWith("+++") || line.startsWith("---") ? "meta" : line.startsWith("@@") ? "hunk" : line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : line.startsWith("diff --git") ? "file" : "context";
      pre.append(element("span", line || " ", `diff-${kind}`), document.createTextNode("\n"));
    }
    card.append(pre);
  }
  async function loadSourceDiff(selected, route, current) {
    const parent = $("source-diff");
    if (!route.rev || selected.revision_scope !== "revision") {
      parent.replaceChildren(element("p", selected.kind === "pijul" ? "Pijul changes are listed from the current channel; historical diffs are not readable." : "Select a revision to see its changes against its first parent.", "small"));
      return;
    }
    parent.replaceChildren(element("p", "Loading selected revision changes…", "small"));
    try {
      const diff = await api(`/api/sources/${encodeURIComponent(selected.id)}/diff?${sourceQuery({ revision: route.rev })}`);
      if (!current()) return;
      parent.replaceChildren(); renderSourceDiff(parent, diff);
    } catch (error) { if (!isAborted(error) && current()) parent.replaceChildren(element("p", `Changes unavailable: ${explain(error)}`, "warning")); }
  }
  async function loadSource() {
    const route = state.route;
    const current = beginBrowsingLoad("source", route);
    const status = $("source-log-status");
    const log = $("source-log");
    for (const id of ["source-log", "source-tree", "source-file", "source-diff"]) $(id).replaceChildren();
    $("source-log-next").disabled = true;
    $("source-log-next").onclick = null;
    $("source-filter").value = route.sq;
    $("source-filter").disabled = !route.src;
    $("source-up").disabled = true;
    status.textContent = "Reading configured sources…";
    $("source-browser-status").replaceChildren(element("p", "Loading admitted records…", "small"));
    let inventory;
    try { inventory = await api("/api/sources"); }
    catch (error) { if (!isAborted(error) && current()) { $("source-browser-status").replaceChildren(element("p", `Unavailable: ${explain(error)}`, "warning")); status.textContent = "Source inventory unavailable."; } return; }
    if (!current()) return;
    const sources = Array.isArray(inventory.sources) ? inventory.sources : [];
    const parent = $("source-browser-status"); parent.replaceChildren();
    if (!inventory.configured) parent.append(element("p", "No source repositories are configured. The service operator must admit a separate --sources policy. This is not proof that no repository is readable.", "warning"));
    for (const row of sources) {
      const details = element("details");
      details.append(element("summary", `${text(row.label)} · ${text(row.kind)} · ${text(row.status)}${row.id === route.src ? " · selected" : ""}`));
      const facts = element("dl", undefined, "facts");
      addFacts(facts, [["Source ID", row.id], ["Scope", row.revision_scope], ["Channel", row.channel], ["Exact head", row.head], ["Unavailable reason", row.reason]]);
      details.append(facts); parent.append(details);
    }
    options($("source-select"), sources.map((row) => ({ id: row.id, label: `${row.label} · ${row.kind}${row.status === "available" ? "" : " · unavailable"}` })), "Select a configured source", route.src);
    const selected = sources.find((row) => row.id === route.src);
    if (!selected) {
      $("source-filter").disabled = true;
      if (route.src) { const option = element("option", `Unavailable selection: ${route.src}`); option.value = route.src; $("source-select").append(option); $("source-select").value = route.src; }
      const message = route.src ? `Source not found in the configured inventory: ${route.src}. No repository was substituted.` : sources.length ? "Select a configured source above to browse revisions and files. No repository is selected automatically." : "No source repository is configured.";
      status.textContent = message;
      $("source-tree").append(element("p", message, route.src ? "warning" : "small"));
      if (route.src) browsingClear(parent, "Clear source selection", { src: "", rev: "", spath: "", sfile: "", sq: "" });
      return;
    }
    browsingClear(parent, "Clear source selection", { src: "", rev: "", spath: "", sfile: "", sq: "" });
    if (selected.status !== "available") {
      $("source-filter").disabled = true;
      status.textContent = `Selected source unavailable: ${text(selected.reason)}. No repository was substituted.`;
      $("source-tree").append(element("p", status.textContent, "warning"));
      return;
    }
    $("source-filter").disabled = false;
    $("source-up").disabled = !route.spath;
    status.textContent = "Reading revisions…";
    const tree = $("source-tree"); tree.append(element("p", "Loading selected directory…", "small"));
    const file = $("source-file");
    const content = element("div", undefined, "view-stack");
    if (route.sfile) {
      browsingClear(file, "Clear selected file", { sfile: "" });
      file.append(element("p", `Selected file: ${route.sfile}`, "identity"), content);
      content.append(element("p", "Loading exact file content…", "small"));
    } else file.append(element("p", "Select a file entry to read its exact content. No file is selected automatically.", "small"));
    await Promise.all([
      (async () => {
        try {
          const history = await api(`/api/sources/${encodeURIComponent(selected.id)}/log?${sourceQuery({ limit: "30" })}`);
          if (current()) renderSourceLog(log, route, selected, history, current);
        } catch (error) { if (!isAborted(error) && current()) status.textContent = `Revisions unavailable: ${explain(error)}`; }
      })(),
      (async () => {
        try {
          const listing = await api(`/api/sources/${encodeURIComponent(selected.id)}/tree?${sourceQuery({ revision: route.rev, path: route.spath })}`);
          if (!current()) return;
          tree.replaceChildren(); renderSourceEntries(tree, route, listing);
        } catch (error) { if (!isAborted(error) && current()) { tree.replaceChildren(); sourceBreadcrumb(tree, route); tree.append(element("p", `Listing unavailable: ${explain(error)}`, "warning")); } }
      })(),
      loadSourceDiff(selected, route, current),
      (async () => {
        if (!route.sfile) return;
        try {
          const result = await api(`/api/sources/${encodeURIComponent(selected.id)}/file?${sourceQuery({ revision: route.rev, path: route.sfile })}`);
          if (!current()) return;
          content.replaceChildren(); renderSourceFile(content, result);
        } catch (error) { if (!isAborted(error) && current()) content.replaceChildren(element("p", `File unavailable: ${explain(error)}`, "warning")); }
      })()
    ]);
  }
  function renderSourceLog(parent, route, selected, history, current) {
    const entries = Array.isArray(history.entries) ? history.entries : [];
    parent.replaceChildren();
    const historical = selected.kind !== "pijul" && selected.revision_scope === "revision";
    for (const entry of entries) {
      const button = element("button", undefined, "run-choice source-revision");
      button.type = "button";
      button.title = text(entry.id);
      if (historical && entry.id === route.rev) button.setAttribute("aria-current", "true");
      button.append(element("strong", text(entry.summary || entry.id)), element("span", text(entry.author), "small"), compactTimestampSpan(entry.recorded_at_ms), element("span", historical ? `select revision ${shortOid(entry.id)}` : `change ${shortOid(entry.id)} · historical trees are not readable`, "small"));
      button.addEventListener("click", () => { if (historical) navigate({ rev: entry.id, spath: "", sfile: "" }); });
      if (!historical) button.disabled = true;
      parent.append(button);
    }
    if (!entries.length) parent.append(element("p", "No recorded revision is readable for this source.", "small"));
    $("source-log-status").textContent = `${entries.length} revision${entries.length === 1 ? "" : "s"} observed${history.next_cursor ? " · bounded page" : ""}${historical ? " · select one to pin the tree and file reads" : " · Pijul listings cover the current channel state only"}`;
    if (route.rev) {
      parent.append(element("p", `Selected revision: ${route.rev}${entries.some((entry) => entry.id === route.rev) ? "" : " · not in this bounded history page; exact reads are independent"}`, "identity"));
      browsingClear(parent, "Use current head", { rev: "", spath: "", sfile: "" });
    }
    const index = entries.findIndex((entry) => entry.id === route.rev);
    const steps = element("div", undefined, "source-steps");
    for (const [label, target] of [["← newer", index > 0 ? entries[index - 1] : null], ["older →", index >= 0 ? entries[index + 1] : null]]) {
      const button = element("button", label); button.type = "button";
      button.disabled = !historical || !target;
      if (target) button.addEventListener("click", () => navigate({ rev: target.id, spath: "", sfile: "" }));
      steps.append(button);
    }
    parent.append(steps);
    const next = $("source-log-next");
    next.disabled = !history.next_cursor;
    next.onclick = async () => {
      if (!history.next_cursor) return;
      next.disabled = true;
      let more;
      try { more = await api(`/api/sources/${encodeURIComponent(selected.id)}/log?${sourceQuery({ limit: "30", cursor: history.next_cursor })}`); }
      catch (error) { if (!isAborted(error) && current()) { $("source-log-status").textContent = `More revisions unavailable: ${explain(error)}`; next.disabled = false; } return; }
      if (!current()) return;
      renderSourceLog(parent, route, selected, { ...history, entries: [...entries, ...(more.entries || [])], next_cursor: more.next_cursor }, current);
    };
  }
  function compactTimestampSpan(ms) {
    const span = element("span", compactTimestamp(ms), "small");
    span.title = timestamp(ms);
    return span;
  }
  // r[impl kiln.dashboard.reviews.view]
  function retainedDecisionLabel(decision) {
    if (typeof decision !== "string" || !decision) return "Unknown (not recorded)";
    return ["accepted", "rejected", "escalated", "blocked"].includes(decision) ? decision : `Unrecognized decision: ${decision}`;
  }
  function reviewDecisionBadges(parent, counts) {
    const badges = element("div", undefined, "badges");
    for (const [decision, count] of Object.entries(counts || {}).sort()) {
      const badge = element("span", `${retainedDecisionLabel(decision)} · ${count}`, "state-badge");
      badge.dataset.tone = reviewDecisionTone(decision);
      badges.append(badge);
    }
    if (!badges.childElementCount) badges.append(element("span", "No review rows recorded", "state-badge"));
    parent.append(badges);
  }
  async function loadReviewsPage() {
    const route = state.route;
    const current = beginBrowsingLoad("reviews", route);
    $("reviews-list").replaceChildren();
    const detail = $("reviews-detail"); detail.replaceChildren();
    if (route.receipt) {
      browsingClear(detail, "Clear selected receipt", { receipt: "" });
      detail.append(element("p", `Selected receipt: ${route.receipt}`, "identity"), element("p", "Loading retained receipt…", "small"));
    } else detail.append(element("h2", "Select a receipt"), element("p", "Choose a retained receipt. No receipt is selected automatically.", "small"));
    $("reviews-status").textContent = "Reading retained receipts…";
    let inventory = null;
    try { inventory = await api("/api/reviews"); }
    catch (error) { if (!current() || isAborted(error)) return; $("reviews-status").textContent = `Receipt inventory unavailable: ${explain(error)}. Exact selection is inspected independently.`; }
    if (!current()) return;
    const list = $("reviews-list");
    const receipts = Array.isArray(inventory?.receipts) ? inventory.receipts : [];
    if (inventory) $("reviews-status").textContent = inventory.configured
      ? `${receipts.length} retained receipt${receipts.length === 1 ? "" : "s"} in ${text(inventory.label)}${(inventory.skipped || []).length ? ` · ${inventory.skipped.length} entry(ies) skipped` : ""}`
      : "No receipt directory is configured. The operator must admit a separate --reviews policy; this is not proof that no receipt exists.";
    for (const skipped of inventory?.skipped || []) list.append(element("p", `Skipped retained entry: ${text(typeof skipped === "string" ? skipped : JSON.stringify(skipped))}. No verdict is inferred.`, "warning"));
    for (const receipt of receipts) {
      const link = element("a", undefined, "run-choice cob-choice");
      link.href = `?${new URLSearchParams({ ...state.route, page: "reviews", receipt: receipt.name })}`;
      if (receipt.name === route.receipt) link.setAttribute("aria-current", "true");
      const head = element("div", undefined, "cob-head");
      head.append(element("span", text(receipt.provider_profile), "cob-type"), element("span", `${text(receipt.bytes)} bytes`, "cob-repo"));
      link.append(head, element("strong", text(receipt.name), "cob-title"));
      const summary = element("span", `${Object.entries(receipt.decision_counts || {}).map(([d, c]) => `${retainedDecisionLabel(d)} ${c}`).join(" · ") || "no decisions"} · acceptance authority ${receipt.authorizes_acceptance === true ? "producer claim, not granted" : receipt.authorizes_acceptance === false ? "none" : "unknown (not recorded)"}`, "small");
      link.append(summary);
      link.addEventListener("click", (event) => { if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); navigate({ page: "reviews", receipt: receipt.name }); } });
      list.append(link);
    }
    if (inventory && !receipts.length) list.append(element("p", inventory.configured ? "No receipts are retained in the configured directory." : "No receipt directory is configured.", "small"));
    if (!route.receipt) return;
    const content = element("div", undefined, "view-stack");
    detail.replaceChildren();
    browsingClear(detail, "Clear selected receipt", { receipt: "" });
    detail.append(element("p", `Selected receipt: ${route.receipt}`, "identity"), content);
    if (inventory && !receipts.some((receipt) => receipt.name === route.receipt)) detail.append(element("p", "This exact receipt is not in the current inventory. No other receipt was substituted.", "warning"));
    content.append(element("p", "Loading retained receipt…", "small"));
    let record;
    try { record = await api(`/api/reviews/${encodeURIComponent(route.receipt)}`); }
    catch (error) { if (!isAborted(error) && current()) content.replaceChildren(element("p", `Receipt unavailable: ${explain(error)}`, "warning")); return; }
    if (!current()) return;
    content.replaceChildren();
    const document = record.document;
    const heading = section(text(record.name), content, "record-card");
    const facts = element("dl", undefined, "facts");
    const observation = document.observation || {};
    addFacts(facts, [["Retained bytes", record.bytes], ["BLAKE3", record.blake3], ["Effect", observation.effect_id], ["Candidate", observation.candidate_id], ["Report identity", observation.report_identity], ["Acceptance authority", observation.authorizes_acceptance === true ? "producer claims authority (not granted)" : observation.authorizes_acceptance === false ? "none" : "Unknown (not recorded)"], ["Isolated", observation.isolated === true ? "yes" : observation.isolated === false ? "no" : "Unknown (not recorded)"]]);
    heading.append(facts);
    const reviews = Array.isArray(observation.reviews) ? observation.reviews : [];
    reviewDecisionBadges(heading, reviews.reduce((counts, row) => { const decision = typeof row?.decision === "string" ? row.decision : ""; counts[decision] = (counts[decision] || 0) + 1; return counts; }, Object.create(null)));
    if (reviews.length) {
      const table = element("table", undefined, "review-table");
      const head = element("tr");
      for (const label of ["Change", "Decision"]) head.append(element("th", label));
      const thead = element("thead"); thead.append(head); table.append(thead);
      const body = element("tbody");
      for (const row of reviews) {
        const line = element("tr");
        const decision = element("td");
        const badge = element("span", retainedDecisionLabel(row?.decision), "state-badge");
        badge.dataset.tone = reviewDecisionTone(row?.decision);
        decision.append(badge);
        line.append(element("td", renderChangeRef(row?.change), "review-change"), decision);
        body.append(line);
      }
      table.append(body); heading.append(table);
    }
    if (typeof document.native_report === "string") { const details = element("details"); details.append(element("summary", "Retained native report · verbatim text"), element("pre", document.native_report, "identity")); heading.append(details); }
    const raw = element("details"); raw.append(element("summary", "Complete retained receipt fields"), element("pre", JSON.stringify(document, null, 2), "identity")); heading.append(raw);
    for (const boundary of record.non_claims || []) detail.append(element("p", boundary, "small"));
  }
  // r[impl kiln.web.minimal_console]
  function renderOverview(parent, result) {
    const grid = element("div", undefined, "overview-totals");
    grid.setAttribute("aria-label", "Retained run counts");
    parent.append(grid);
    const outcomes = { runs: "", failed: "failed", active: "", queued: "queued", succeeded: "Success", cancelled: "Cancelled", unknown: "Unknown" };
    const labels = { runs: "Retained", failed: "Failed", active: "Active", queued: "Queued", succeeded: "Succeeded", cancelled: "Cancelled", unknown: "Unknown" };
    for (const key of ["failed", "active", "queued", "unknown", "succeeded", "cancelled", "runs"]) {
      if (!Object.hasOwn(result.totals || {}, key)) continue;
      const patch = { page: "runs", outcome: outcomes[key], project: "", q: "", source: "", cursor: "", run: "", node: "" };
      if (key === "active") Object.assign(patch, { page: "workers", worker_status: "busy", worker_q: "", worker: "" });
      const link = workspaceLink(patch, undefined, "overview-total");
      link.dataset.outcome = key;
      if (key === "active") link.title = "Inspect execution targets with active runs";
      link.append(element("strong", text(result.totals[key]), "metric-value"), element("span", labels[key], "metric-label"));
      grid.append(link);
    }
    const health = section("Latest project runs", parent);
    health.classList.add("project-health");
    const priority = { failing: 0, unknown: 1, active: 2, queued: 3, unavailable: 4, cancelled: 5, healthy: 6 };
    for (const project of [...(result.projects || [])].sort((a, b) => (priority[a.health] ?? 4) - (priority[b.health] ?? 4))) {
      const row = element("article", undefined, "project-row");
      row.dataset.health = project.health;
      const name = element("div", undefined, "project-row-name");
      const latest = project.latest_run;
      const history = { page: "runs", project: project.id, outcome: "", q: "", source: "", cursor: "", run: "", node: "" };
      name.append(workspaceLink(history, text(project.label)));
      if (latest) {
        const badge = element("span", text(latest.outcome), "state-badge");
        badge.dataset.tone = project.health === "failing" ? "warning" : project.health === "healthy" ? "good" : "neutral";
        name.append(badge);
        row.append(name, element("span", duration(latest.duration_ms), "project-row-duration"), element("span", compactTimestamp(latest.completed_at_ms), "project-row-time"));
        const actions = element("div", undefined, "project-row-actions");
        const selection = { ...history, run: latest.id, stream: project.health === "failing" ? "stderr" : "stdout" };
        const pipeline = workspaceLink({ ...selection, view: "pipeline" }, "Pipeline");
        const logs = workspaceLink({ ...selection, view: "logs" }, "Logs");
        pipeline.setAttribute("aria-label", `Pipeline for ${text(project.label)}`);
        logs.setAttribute("aria-label", `Logs for ${text(project.label)}`);
        actions.append(pipeline, logs); row.append(actions);
      } else {
        row.append(name, element("span", "No retained runs", "small"));
      }
      health.append(row);
    }
    if (!result.projects?.length) health.append(element("p", "No enrolled project health is recorded.", "small"));
    const trend = element("details", undefined, "panel overview-timing");
    trend.append(element("summary", "Recorded duration trend"));
    parent.append(trend);
    trend.append(element("p", `${text(result.timing?.recorded)} / ${text(result.timing?.total)} retained runs have recorded timing. Historical unknowns are excluded, not treated as zero.`, "small"));
    const rows = result.duration_trend || [];
    const max = Math.max(1, ...rows.map((row) => Number.isSafeInteger(row.duration_ms) ? row.duration_ms : 0));
    for (const row of rows) { const item = element("div", undefined, "trend-row"); item.append(runLink(row.run_id, `${row.project_id} · ${row.outcome}`), element("span", `${duration(row.duration_ms)} · ${timestamp(row.completed_at_ms)}`, "small")); if (Number.isSafeInteger(row.duration_ms)) { const bar = element("meter"); bar.min = 0; bar.max = max; bar.value = row.duration_ms; bar.setAttribute("aria-label", `${row.run_id}: ${duration(row.duration_ms)}`); item.append(bar); } trend.append(item); }
    if (!rows.length) trend.append(element("p", "No recorded duration samples available.", "small"));
  }
  function renderProjects() {
    const parent = $("project-cards"); parent.replaceChildren();
    for (const project of state.projects.enrolled) {
      const card = section(text(project.label || project.id), parent, "project-card surface");
      card.append(element("p", text(project.reference), "small"));
      const actions = element("div", undefined, "project-actions");
      actions.append(workspaceLink({ page: "runs", project: project.id, q: "", outcome: "", source: "", cursor: "", run: "", node: "" }, "Browse runs"));
      const start = element("button", "Select for new run");
      start.type = "button";
      start.disabled = !canOperate();
      start.addEventListener("click", () => { $("enrolled-project").value = project.id; projectDescriptions(); void readAction(loadRefs)(); $("enrolled-project").focus(); $("start-form").scrollIntoView({ block: "center" }); });
      actions.append(start); card.append(actions);
      const details = element("details");
      details.append(element("summary", "Repository & pipeline identity"));
      const facts = element("dl", undefined, "facts");
      addFacts(facts, [["ID", project.id], ["Repository", project.repository], ["Reference", project.reference], ["Pipeline", project.pipeline_identity]]);
      details.append(facts); card.append(details);
    }
    if (!state.projects.enrolled.length) parent.append(element("p", "No enrolled projects.", "small"));
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
  function workflowBadge(value) {
    const badge = element("span", text(value), "workflow-state");
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
    button.append(head, element("strong", node.id), element("span", node.duration_ms == null ? "Timing not observed" : `Recorded duration · ${duration(node.duration_ms)}`, "small"));
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
    addFacts(facts, [["Kind", node.kind], ["Started", timestamp(node.started_at_ms)], ["Completed", timestamp(node.completed_at_ms)], ["Duration", duration(node.duration_ms)]]); parent.append(facts);
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
  async function prepareRerun(mode) {
    if (!canOperate()) return;
    const id = state.selected;
    try { const run = await loadDetail(); if (!run || id !== state.selected) return; if (mode === "failed" && !run.nodes?.some((node) => /^(failed|error)$/i.test(node.state))) throw new Error("No known failed node set is available. Unknown states cannot be retried blindly."); state.confirm = { id, mode }; $("confirm-title").textContent = mode === "latest" ? "Run latest admitted reference" : mode === "failed" ? "Retry known failed nodes" : "Rerun exact source revision"; $("confirm-control").textContent = "Create new run"; addFacts($("confirm-facts"), [["Parent run", id], ["Project", run.project_id], ["Mode", mode], ["Source", mode === "latest" ? "Latest admitted reference (resolved by service)" : run.source_revision], ["Failed selection", mode === "failed" ? run.nodes.filter((node) => /^(failed|error)$/i.test(node.state)).map((node) => node.id) : "Not a node retry"], ["Boundary", "New candidate and attempt; queued durably if project is busy."]]); $("control-dialog").showModal(); $("dismiss-control").focus(); }
    catch (error) { if (!isAborted(error)) notice(explain(error)); }
  }
  function renderFlaky(parent, rows) {
    const group = section("Repeated-test instability", parent, "view-stack");
    group.append(element("p", "Instability requires actual repeated pass/fail observations for the same exact source revision, pipeline, node, report and test identity. It is not inferred from code changes, execution logs or missing reports.", "small"));
    if (!rows.length) group.append(element("p", "No repeated pass/fail instability is recorded in the bounded history. This does not prove all tests are stable.", "small"));
    for (const row of rows) {
      const card = section(`${text(row.suite)} / ${text(row.class_name)} / ${text(row.name)}`, group, "record-card");
      const facts = element("dl", undefined, "facts"); addFacts(facts, [["Exact source revision", row.source_revision], ["Node", row.node_id], ["Report", row.report_name], ["Passed observations", row.passed], ["Failed observations", row.failed], ["Total observations", row.observations]]); card.append(facts);
      const links = element("div", undefined, "control-bar"); for (const id of row.run_ids || []) links.append(runLink(id, id, "tests")); card.append(links);
    }
  }
  function renderComparison(parent, result) {
    parent.append(element("p", "Only admitted recorded facts are compared. Missing tests, coverage and timing stay unavailable. A changed test verdict is not acceptance authority.", "boundary"));
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [["Current source", result.source?.current], ["Other source", result.source?.other], ["Current outcome", result.outcome?.current], ["Other outcome", result.outcome?.other], ["Current duration", duration(result.duration_ms?.current)], ["Other duration", duration(result.duration_ms?.other)], ["Duration delta (current − other)", Number.isSafeInteger(result.duration_ms?.delta) ? `${(result.duration_ms.delta / 1000).toFixed(3)} s` : null]]);
    parent.append(facts, runLink(result.run_id, "Inspect current run"), runLink(result.other_run_id, "Inspect compared run"));
    for (const key of ["current", "other"]) { const group = section(`${key === "current" ? "Current" : "Other"} structured tests & coverage`, parent, "record-card"); renderRecorded(group, result.reports?.[key]); }
    const changes = section("Recorded test changes", parent, "view-stack");
    changes.append(element("p", `Bounded comparison: ${text(result.test_changes_bounded)}. Tests absent on one side are unavailable, not passing.`, "small"));
    if (!result.test_changes?.length) changes.append(element("p", "No test-status differences recorded in this comparison window.", "small"));
    for (const row of result.test_changes || []) { const card = section(`${text(row.suite)} / ${text(row.class_name)} / ${text(row.name)}`, changes, "record-card"); const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["Node", row.node_id], ["Report", row.report_name], ["Current status", row.current], ["Other status", row.other]]); card.append(metadata); }
    renderFlaky(parent, result.flaky_history || []);
  }
  function renderReports(parent, result) {
    const records = result.reports;
    const runId = result.run_id || state.route.run;
    if (result.summary) { const summary = section("Recorded report summary", parent, "record-card"); renderRecorded(summary, result.summary); }
    if (Array.isArray(result.flaky_history)) renderFlaky(parent, result.flaky_history);
    if (!Array.isArray(records)) throw new Error("Structured report response unavailable.");
    parent.append(element("p", "Reported tests and coverage are recorded evidence, not execution-success or accepted-gate authority. Missing reports are unavailable, not zero failures or full coverage.", "boundary"));
    if (!records.length) parent.append(element("p", "No declared reports retained for this run.", "small"));
    for (const record of records) {
      const report = record.report || record;
      const card = section(`${text(record.name)} · ${text(report.kind)}`, parent, "record-card");
      const facts = element("dl", undefined, "facts"); addFacts(facts, [["Node", record.node_id], ["Attempt", record.attempt_id], ["Source revision", record.source_revision], ["Source BLAKE3", report.source_blake3], ["Source bytes", report.source_bytes], ["Report identity", record.id], ["Recorded", timestamp(record.recorded_at_ms)]]); card.append(facts);
      if (record.unavailable) card.append(element("p", `Report unavailable: ${text(record.unavailable)}. Missing normalized data is not a passing report.`, "warning"));
      if (record.artifact) { const button = element("button", "Download retained report source"); button.type = "button"; button.addEventListener("click", () => downloadArtifact(runId, record.artifact, button)); card.append(button); }
      if (report.tests) {
        const tests = report.tests;
        const summary = element("dl", undefined, "facts"); addFacts(summary, [["Total", tests.total], ["Passed", tests.passed], ["Failed", tests.failed], ["Errors", tests.errors], ["Skipped", tests.skipped], ["Reported seconds", tests.duration_seconds]]); card.append(summary);
        for (const test of tests.cases || []) {
          const details = element("details", undefined, "test-case"); details.append(element("summary", `${text(test.status)} · ${text(test.suite)} / ${text(test.class_name)} / ${text(test.name)}`));
          if (test.status === "failed" || test.status === "error") details.open = true;
          const metadata = element("dl", undefined, "facts"); addFacts(metadata, [["File", test.file], ["Line", test.line], ["Reported seconds", test.duration_seconds], ["Message", test.message]]); details.append(metadata);
          if (test.details) details.append(element("pre", test.details, "identity"));
          card.append(details);
        }
      }
      if (report.coverage) {
        const coverage = report.coverage;
        const summary = element("dl", undefined, "facts"); addFacts(summary, [["Format", coverage.format], ["Lines hit / found", coverage.lines_hit === null || coverage.lines_found === null ? null : `${coverage.lines_hit} / ${coverage.lines_found}`], ["Line rate", coverage.line_rate === null ? null : `${(coverage.line_rate * 100).toFixed(2)}%`], ["Branches hit / found", coverage.branches_hit === null || coverage.branches_found === null ? null : `${coverage.branches_hit} / ${coverage.branches_found}`], ["Branch rate", coverage.branch_rate === null ? null : `${(coverage.branch_rate * 100).toFixed(2)}%`]]); card.append(summary);
        for (const file of coverage.files || []) { const details = element("details"); details.append(element("summary", text(file.path))); renderRecorded(details, file); card.append(details); }
      }
      if (report.evidence) renderEvidence(card, report.evidence);
    }
  }
  // r[impl kiln.dashboard.reviews.view]
  function patchgateReceipts(result) {
    const records = Array.isArray(result.reports) ? result.reports : [];
    return records.filter((record) => {
      const report = record.report || record;
      return report?.kind === "review" && report?.evidence?.document?.schema === "kiln.patchgate.review.v1";
    });
  }
  function reviewDecisionTone(decision) {
    if (decision === "accepted") return "good";
    if (decision === "rejected" || decision === "escalated" || decision === "blocked") return "warning";
    return "";
  }
  function renderChangeRef(change) {
    if (!change || typeof change !== "object") return text(change);
    return `${text(change.provider_namespace)}/${text(change.native_kind)}/${text(change.native_id)}`;
  }
  function renderPatchgateReceipt(parent, runId, record, report, document) {
    const observation = document.observation || {};
    const reviews = Array.isArray(observation.reviews) ? observation.reviews : [];
    const counts = new Map();
    for (const row of reviews) counts.set(row.decision, (counts.get(row.decision) || 0) + 1);
    const card = section(`Patchgate review · ${reviews.length} change${reviews.length === 1 ? "" : "s"}`, parent, "record-card");
    const badges = element("div", undefined, "badges");
    for (const [decision, count] of [...counts.entries()].sort()) {
      const badge = element("span", `${decision} · ${count}`, "state-badge");
      badge.dataset.tone = reviewDecisionTone(decision);
      badges.append(badge);
    }
    if (!counts.size) badges.append(element("span", "No review rows recorded", "state-badge"));
    badges.append(element("span", observation.authorizes_acceptance === true ? "Producer claims acceptance authority (not granted)" : "Acceptance authority: none", "state-badge"));
    badges.append(element("span", observation.isolated === true ? "Isolated execution recorded" : "Isolation not recorded", "state-badge"));
    card.append(badges);
    const facts = element("dl", undefined, "facts");
    addFacts(facts, [
      ["Effect", observation.effect_id],
      ["Candidate", observation.candidate_id],
      ["Candidate root", observation.candidate_root],
      ["Pipeline revision", observation.pipeline_revision],
      ["Policy identity", observation.policy_identity],
      ["Provider profile", observation.provider_profile],
      ["Native binding", observation.native_binding_identity],
      ["Report identity", observation.report_identity],
      ["Request identity", document.request_identity],
      ["Retained native report bytes", typeof document.native_report === "string" ? document.native_report.length : null],
      ["Recorded", timestamp(record.recorded_at_ms)],
      ["Source revision", record.source_revision],
      ["Report record", record.id]
    ]);
    card.append(facts);
    if (reviews.length) {
      const table = element("table", undefined, "review-table");
      const head = element("tr");
      for (const heading of ["Change", "Decision"]) head.append(element("th", heading));
      const thead = element("thead");
      thead.append(head);
      table.append(thead);
      const body = element("tbody");
      for (const row of reviews) {
        const line = element("tr");
        const decision = element("td");
        const badge = element("span", text(row.decision), "state-badge");
        badge.dataset.tone = reviewDecisionTone(row.decision);
        decision.append(badge);
        line.append(element("td", renderChangeRef(row.change), "review-change"), decision);
        body.append(line);
      }
      table.append(body);
      card.append(table);
    }
    if (typeof document.native_report === "string") {
      const details = element("details");
      details.append(element("summary", `Retained native Patchgate report · ${document.native_report.length} bytes · preserved verbatim`));
      details.append(element("pre", document.native_report, "identity"));
      card.append(details);
    }
    if (report.evidence?.claim_boundary) card.append(element("p", report.evidence.claim_boundary, "small"));
    if (runId) card.append(runLink(runId, "Inspect this exact run", "pipeline"));
  }
  function renderReviews(parent, result) {
    parent.append(element("p", "A retained receipt proves only that Kiln admitted and preserved that review evidence for this run. Rejected, escalated, blocked, waiting, pending, removed, reverted and unknown decisions stay distinct; absence of a receipt is not a clean review.", "warning"));
    const receipts = patchgateReceipts(result);
    if (!receipts.length) {
      parent.append(element("p", "No retained Patchgate review receipt for this run. Upload a kiln.patchgate.review.v1 document as a review report to retain one.", "small"));
      return;
    }
    const runId = result.run_id || state.route.run;
    for (const record of receipts) {
      const report = record.report || record;
      renderPatchgateReceipt(parent, runId, record, report, report.evidence.document);
    }
    const other = (result.reports || []).filter((record) => !receipts.includes(record));
    if (other.length) parent.append(element("p", `${other.length} other retained report(s) are shown under Tests & coverage.`, "small"));
  }
  function renderEvidence(parent, result) {
    parent.append(element("p", "Recorded evidence only. A reported verdict, review, provenance link or retained artifact does not establish accepted gate authority or product correctness.", "warning"));
    for (const key of ["source", "pipeline", "execution"]) if (Object.hasOwn(result, key)) { const group = section(key[0].toUpperCase() + key.slice(1) + " linkage", parent, "record-card"); renderRecorded(group, result[key]); }
    if (result.run_id) parent.append(runLink(result.run_id, "Inspect this exact run", "pipeline"));
    if (result.parent_run_id) parent.append(runLink(result.parent_run_id, "Inspect parent run"));
    if (Array.isArray(result.artifacts)) {
      const group = section("Retained artifact provenance", parent, "view-stack");
      if (!result.artifacts.length) group.append(element("p", "No retained artifacts for this run.", "small"));
      for (const artifact of result.artifacts) { const card = section(text(artifact.name), group, "record-card"); renderRecorded(card, artifact); const button = element("button", "Download exact retained artifact"); button.type = "button"; button.addEventListener("click", () => downloadArtifact(result.run_id, artifact, button)); card.append(button); }
    }
    if (Array.isArray(result.reports)) { const group = section("Recorded gate, review and report evidence", parent, "view-stack"); renderReports(group, { run_id: result.run_id, reports: result.reports }); }
    if (!result.source && !result.pipeline && !result.execution) renderRecorded(parent, result);
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
    parent.append(element("p", `${result.items.length} shown of ${text(result.total)} · bounded retained timeline · audit window: ${text(result.audit_window)}`, "small"));
    for (const item of result.items) {
      const card = section(`${item.kind === "audit" ? "Audit decision" : "Canonical event"} · ${timestamp(item.recorded_at_ms)}`, parent, "record-card");
      if (item.run_id) card.append(runLink(item.run_id));
      if (item.kind === "audit") renderRecorded(card, item.audit); else renderRecorded(card, { event_identity: item.event_identity, ordinal: item.ordinal, event_index: item.event_index, event: item.event });
    }
    if (!result.items.length) parent.append(element("p", "No retained activity in this bounded page.", "small"));
    const controls = element("div", undefined, "control-bar");
    const first = element("button", "First page"); first.type = "button"; first.disabled = !state.route.activity_cursor; first.addEventListener("click", () => navigate({ activity_cursor: "" }));
    const next = element("button", "Next page"); next.type = "button"; next.disabled = !result.next_cursor; next.addEventListener("click", () => navigate({ activity_cursor: result.next_cursor }));
    controls.append(first, next); parent.append(controls);
  }
  function renderOperations(parent, result) {
    if (result.storage_failed) parent.append(element("p", "Durable storage is failed. Mutating admission is unavailable; no healthy state is inferred.", "warning"));
    for (const warning of result.warnings || []) {
      const row = element("div", undefined, "warning"); row.append(element("span", `${text(warning.code)}: ${text(warning.resource)} uses ${text(warning.used)} / ${text(warning.maximum)}. Retention admission may fail; no canonical data is automatically pruned.`));
      if (warning.run_id) row.append(document.createTextNode(" "), runLink(warning.run_id, "Inspect affected run")); parent.append(row);
    }
    const capacity = section("Execution capacity", parent, "record-card"); renderRecorded(capacity, result.capacity);
    const queue = section("Durable queue", parent, "view-stack");
    if (!result.queue?.length) queue.append(element("p", "No queued work in the admitted snapshot.", "small"));
    for (const row of result.queue || []) { const card = section(`Position ${text(row.position)} · ${text(row.project_id)}`, queue, "record-card"); card.append(runLink(row.run_id), element("p", `Queued: ${timestamp(row.created_at_ms)}`, "small")); }
    const providers = section("Provider observations", parent, "view-stack");
    for (const row of result.providers || []) { const card = section(`${text(row.project_id)} · ${text(row.status)}`, providers, "record-card"); if (row.status === "unavailable") card.append(element("p", "Provider observation is unavailable, not healthy capacity.", "warning")); renderRecorded(card, row); }
    const usage = section("Retained usage & limits", parent, "view-stack");
    for (const row of result.usage || []) {
      const card = section(text(row.resource), usage, "record-card"); card.append(element("p", `${text(row.used)} / ${text(row.maximum)}`, "small"));
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
    if (type === "checkbox") wrapper.append(input, document.createTextNode(label)); else wrapper.append(document.createTextNode(label), input);
    input.disabled = !canOperate() || state.permissions.configure !== true;
    form.append(wrapper); return input;
  }
  function renderAutomation(parent, result) {
    parent.append(element("p", `Automation: ${text(result.status)}. ${result.configured ? "Only registered sources and destinations are editable." : "No automation policy is admitted. Register sources/destinations in server policy; browser input cannot create authority."}`, "small"));
    const policy = element("dl", undefined, "facts"); addFacts(policy, [["Reviewed policy BLAKE3", result.policy_identity]]); parent.append(policy);
    for (const [key, kind] of [["triggers", "trigger"], ["schedules", "schedule"], ["notifications", "notification"]]) {
      const group = section(key[0].toUpperCase() + key.slice(1), parent, "view-stack");
      if (!result[key]?.length) group.append(element("p", "No registrations.", "small"));
      for (const row of result[key] || []) {
        const form = element("form", undefined, "record-card automation-form"); form.append(element("h3", text(row.id)));
        const facts = element("dl", undefined, "facts"); addFacts(facts, [["Project", row.project_id], ["Reference", row.reference], ["Last status", row.last_status]]); form.append(facts);
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
          if (updated && state.session && state.route.page === "automation") { parent.replaceChildren(); renderAutomation(parent, updated); }
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
  function setMenu(open, restoreFocus = false) {
    const mobile = window.matchMedia("(max-width: 60rem)").matches;
    const wasOpen = $("app-menu-toggle").getAttribute("aria-expanded") === "true";
    open = open && mobile;
    $("app-sidebar").classList.toggle("is-open", open);
    $("app-sidebar").inert = mobile && !open;
    $("app-menu-toggle").setAttribute("aria-expanded", String(open));
    $("app-menu-backdrop").hidden = !open;
    document.documentElement.classList.toggle("app-menu-open", open);
    $("main").inert = open;
    document.querySelector(".session-tools").inert = open;
    if (open) $("app-sidebar").querySelector('[aria-current="page"]')?.focus();
    else if (restoreFocus) $("app-menu-toggle").focus();
    else if (wasOpen && state.session) {
      $("workspace-title").tabIndex = -1;
      $("workspace-title").focus({ preventScroll: true });
    }
  }
  state.route = readRoute();
  options($("run-outcome"), [{ id: "failed", label: "All failures" }, { id: "queued", label: "Queued" }, ...["Pending", "Success", "DomainFailure", "Denied", "Cancelled", "TimedOut", "ProviderError", "Unknown"].map((id) => ({ id }))], "All outcomes", state.route.outcome);
  for (const node of document.querySelectorAll("[data-page]")) node.addEventListener("click", () => navigate({ page: node.dataset.page }));
  for (const node of document.querySelectorAll("[data-view]")) node.addEventListener("click", () => navigate({ view: node.dataset.view }));
  for (const mode of ["exact", "latest", "failed"]) $("rerun-" + mode).addEventListener("click", () => { void prepareRerun(mode); });
  for (const id of ["app-theme", "app-density"]) $(id).addEventListener("change", () => navigate({ [id === "app-theme" ? "theme" : "density"]: $(id).value }));
  $("app-menu-toggle").addEventListener("click", () => setMenu($("app-menu-toggle").getAttribute("aria-expanded") !== "true"));
  $("app-menu-backdrop").addEventListener("click", () => setMenu(false, true));
  window.matchMedia("(max-width: 60rem)").addEventListener("change", () => setMenu(false));
  document.addEventListener("keydown", (event) => {
    if ($("app-menu-toggle").getAttribute("aria-expanded") !== "true") return;
    if (event.key === "Escape") { event.preventDefault(); setMenu(false, true); }
    if (event.key === "Tab") {
      const stops = [$("app-menu-toggle"), ...$("app-sidebar").querySelectorAll('a[href], button:not(:disabled)')];
      const index = stops.indexOf(document.activeElement);
      event.preventDefault();
      stops[(index + (event.shiftKey ? -1 : 1) + stops.length) % stops.length].focus();
    }
  });
  $("runs-first").addEventListener("click", () => navigate({ cursor: "" }));
  $("runs-next").addEventListener("click", () => { if (state.nextCursor) navigate({ cursor: state.nextCursor }); });
  $("compare-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ other: $("compare-other").value.trim(), view: "compare" }); });
  $("evidence-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ page: "evidence", run: $("evidence-run").value.trim() }); });
  for (const name of ["activity", "automation", "operations"]) $("refresh-" + name).addEventListener("click", readAction(loadPage));
  $("refresh-cobs").addEventListener("click", readAction(loadPage));
  $("refresh-sources").addEventListener("click", readAction(loadPage));
  $("refresh-reviews").addEventListener("click", readAction(loadPage));
  $("source-select").addEventListener("change", (event) => { if (state.session) navigate({ src: event.target.value, rev: "", spath: "", sfile: "", sq: "" }); });
  $("source-filter").addEventListener("input", (event) => { if (state.session) navigate({ sq: event.target.value }); });
  $("source-up").addEventListener("click", () => { if (state.session && state.route.spath) navigate({ spath: state.route.spath.split("/").slice(0, -1).join("/"), sfile: "", sq: "" }); });
  $("source-form").addEventListener("submit", (event) => { event.preventDefault(); if (state.session) navigate({ src: $("source-select").value, rev: "", spath: "", sfile: "", sq: "" }); });
  $("refresh-workers").addEventListener("click", readAction(loadPage));
  $("worker-search-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ worker_q: $("worker-search").value.trim(), worker_status: $("worker-status").value }); });
  $("worker-reset").addEventListener("click", () => navigate({ worker_q: "", worker_status: "" }));
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
  $("refresh-projects").addEventListener("click", readAction(loadProjects));
  $("refresh-runs").addEventListener("click", readAction(loadRuns));
  $("refresh-detail").addEventListener("click", readAction(loadDetail));
  $("refresh-artifacts").addEventListener("click", () => { void loadArtifacts(); });
  $("available-project").addEventListener("change", projectDescriptions);
  $("enrolled-project").addEventListener("change", () => { projectDescriptions(); void readAction(loadRefs)(); });
  $("run-search-form").addEventListener("submit", (event) => { event.preventDefault(); navigate({ q: $("run-search").value.trim(), project: $("run-filter").value, outcome: $("run-outcome").value, source: $("run-source").value.trim(), order: $("run-order").value, cursor: "" }); });
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
    if (result.id && state.session) navigate({ page: "runs", run: result.id, node: "", view: "pipeline" });
  });
  for (const action of ["cancel", "pause", "resume"]) $("control-" + action).addEventListener("click", () => { void prepareControl(action); });
  $("dismiss-control").addEventListener("click", dismissControl);
  $("control-dialog").addEventListener("cancel", dismissControl);
  $("control-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const pending = state.confirm;
    if (!pending || state.mutation) return;
    dismissControl();
    if (pending.mode) await mutate(`${pathFor(pending.id)}/rerun`, { mode: pending.mode }, "New source-bound candidate requested.").then((result) => { if (result?.id) navigate({ page: "runs", run: result.id, node: "", view: "pipeline" }); });
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
    if ($("follow-logs").checked) { $("log-output").scrollTop = $("log-output").scrollHeight; void readLog(state.logKey !== ""); }
  });
  $("clear-log").addEventListener("click", resetLogs);
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
    if (state.session && ["work", "planning", "inbox", "triage", "workviews"].includes(state.route?.page)) void readAction(loadPage)();
  });
  // Restoration rechecks network authorization; a cleared bearer is never restored.
  window.addEventListener("pagehide", () => logout("Disconnected when leaving the live page."));
  window.addEventListener("pageshow", (event) => { if (event.persisted) void discoverNetworkAccess(); });
  void discoverNetworkAccess();
})();
