"use strict";

(() => {
  const LIVE_DELAY_MS = 1500;
  const MAX_LIVE_FAILURES = 6;
  const MAX_BACKOFF_MS = 15000;
  const REQUEST_TIMEOUT_MS = 30000;
  const MAX_JSON_BYTES = 4 * 1024 * 1024;
  const LOG_READ_BYTES = 16384;
  const MAX_DISPLAYED_LOG_CHARS = 256 * 1024;
  const LOG_FOLLOW_MS = 2000;
  const CONTROL_FRESHNESS_MS = 30000;
  const $ = (id) => document.getElementById(id);
  const state = {
    token: null, epoch: 0, requests: new Set(), urls: new Set(), urlTimers: new Set(),
    projects: { available: [], enrolled: [] }, runs: [], selected: null, run: null,
    observedAt: 0, connected: false, liveVersion: "0", liveTimer: null,
    liveController: null, liveGeneration: 0, liveFailures: 0,
    logTimer: null, logGeneration: 0, logBusy: false, logNext: "0", logKey: "",
    artifactsGeneration: 0, confirm: null, mutation: false, detailGeneration: 0,
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

  async function api(target, { method = "GET", body, kind = "json", controller = new AbortController() } = {}) {
    if (!state.token) throw new DOMException("Signed out", "AbortError");
    const epoch = state.epoch;
    const headers = { Authorization: `Bearer ${state.token}`, Accept: kind === "sse" ? "text/event-stream" : kind === "blob" ? "application/octet-stream" : "application/json" };
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
        logout("Authentication expired or the operator token was rejected. Sign in again.");
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
      if (kind === "blob") result = await response.blob();
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
    $("enroll-button").disabled = state.mutation || !available;
    $("start-button").disabled = state.mutation || !enrolled;
  }
  async function loadProjects() {
    const epoch = state.epoch;
    const result = await api("/api/projects");
    if (epoch !== state.epoch) throw new DOMException("Session changed", "AbortError");
    if (!Array.isArray(result.available) || !Array.isArray(result.enrolled)) throw new Error("The project response is invalid; repository choices are unavailable.");
    state.projects = result;
    const enrolledIds = new Set(result.enrolled.map((row) => row.id));
    const available = result.available.filter((row) => !enrolledIds.has(row.id));
    options($("available-project"), available, available.length ? "Select an allowlisted repository" : "No unenrolled repositories available", $("available-project").value);
    options($("enrolled-project"), result.enrolled, result.enrolled.length ? "Select an enrolled project" : "No enrolled projects", $("enrolled-project").value);
    options($("run-filter"), result.enrolled, "All projects", $("run-filter").value);
    if (result.enrolled.length === 1) $("enrolled-project").value = result.enrolled[0].id;
    projectDescriptions();
    renderRunList();
  }

  function renderRunList() {
    const focused = document.activeElement?.dataset.runId;
    const filter = $("run-filter").value;
    const runs = state.runs.filter((run) => !filter || run.project_id === filter);
    $("runs-status").textContent = runs.length ? `${runs.length} retained run${runs.length === 1 ? "" : "s"}${state.connected ? "" : " · snapshot may be stale"}` : "No retained runs match this project. Start a run from an enrolled project.";
    $("run-list").replaceChildren();
    for (const run of runs) {
      const button = element("button", undefined, "run-choice");
      button.type = "button";
      button.dataset.runId = run.id;
      if (run.id === state.selected) button.setAttribute("aria-current", "true");
      button.append(element("span", `${text(run.project_id)} · ${text(run.id)}`), element("span", `Source ${text(run.source_revision)}`, "run-source"), element("span", `${text(run.outcome)} · ${text(run.phase)}`, "run-summary"));
      button.addEventListener("click", () => selectRun(run.id));
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
    else if (state.selected) {
      $("run-warning").textContent = "This run is absent from the latest retained snapshot. Displayed details are stale; refresh the run before any control.";
      $("run-warning").hidden = false;
      state.observedAt = 0;
      updateControlButtons();
    }
  }
  async function loadRuns() {
    const epoch = state.epoch;
    const result = await api("/api/runs");
    if (epoch !== state.epoch) throw new DOMException("Session changed", "AbortError");
    adoptRuns(result.runs);
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
      if (state.selected === id) await loadArtifacts();
    } catch (error) {
      if (isAborted(error) || id !== state.selected) return;
      $("empty-run").hidden = Boolean(state.run);
      notice(`Run details unavailable: ${explain(error)}`);
    }
  }
  function renderDetail() {
    const run = state.run;
    if (!run) return;
    $("empty-run").hidden = true;
    $("run-detail").hidden = false;
    $("run-title").textContent = text(run.id);
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
    addFacts($("run-facts"), [["Project", run.project_id], ["Repository", run.repository], ["Source revision", run.source_revision], ["Canonical tree", run.canonical_tree], ["Run revision", run.revision], ["Attempt", run.attempt_id], ["Sequence", run.sequence]]);
    $("execution-identity").textContent = text(run.execution);
    const nodes = Array.isArray(run.nodes) ? run.nodes : [];
    $("node-list").replaceChildren();
    if (!nodes.length) $("node-list").append(element("p", "No workflow nodes have been admitted for this run.", "small"));
    for (const node of nodes) {
      const row = element("div", undefined, "node-row");
      row.append(element("strong", text(node.id)), element("span", text(node.state), "node-state"));
      $("node-list").append(row);
    }
    const previousNode = $("log-node").value;
    options($("log-node"), nodes, nodes.length ? null : "No admitted nodes", previousNode);
    if (previousNode && previousNode !== $("log-node").value) resetLogs();
    $("read-log").disabled = !nodes.length;
    $("follow-logs").disabled = !nodes.length;
    updateControlButtons();
  }
  function exactIdentity(run) {
    return run && run.attempt_id && Number.isSafeInteger(run.revision) && run.revision >= 0 && run.execution && Number.isSafeInteger(run.sequence) && run.sequence >= 0;
  }
  function updateControlButtons() {
    const run = state.run;
    const fresh = state.connected && Date.now() - state.observedAt < CONTROL_FRESHNESS_MS;
    for (const action of ["cancel", "pause", "resume"]) $("control-" + action).disabled = state.mutation || !fresh || !exactIdentity(run) || run.controls?.[action] !== true;
    $("control-note").textContent = !run ? "Select a run to inspect available controls." : !exactIdentity(run) ? "Exact control identity is unavailable or cannot be represented safely. Controls are disabled; no approximate identity will be sent." : !fresh ? "Snapshot is stale or live observation is disconnected. Refresh and reconnect before controlling this execution." : "Only advertised capabilities are enabled. A fresh read and confirmation bind each control to this exact attempt, revision, sequence, and execution.";
  }

  function scheduleLive(delay, generation) {
    clearTimeout(state.liveTimer);
    state.liveTimer = setTimeout(() => live(generation), delay);
  }
  async function live(generation) {
    if (!state.token || generation !== state.liveGeneration) return;
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
        state.liveVersion = String(snapshot.version);
        connection("Live · authenticated", true);
        adoptRuns(snapshot.runs);
        recognized = true;
      }
      if (!recognized) throw new Error("The live endpoint returned no snapshot or heartbeat.");
      state.liveFailures = 0;
      connection("Live · authenticated", true);
      scheduleLive(LIVE_DELAY_MS, generation);
    } catch (error) {
      if (generation !== state.liveGeneration || isAborted(error) || !state.token) return;
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
    await Promise.all([loadProjects(), loadRuns(), state.selected ? loadDetail() : Promise.resolve(), state.selected ? loadArtifacts() : Promise.resolve()]);
  }

  function setMutation(busy) {
    state.mutation = busy;
    projectDescriptions();
    updateControlButtons();
    $("confirm-control").disabled = busy;
  }
  async function mutate(target, payload, success) {
    if (state.mutation) return null;
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
    const id = state.selected;
    try {
      const freshRun = await loadDetail();
      const run = state.run;
      if (!freshRun || state.selected !== id || !exactIdentity(run) || run.controls?.[action] !== true || !state.connected) throw new Error("This exact execution no longer advertises the requested control. Nothing was sent.");
      const identity = structuredClone({ action, attempt_id: run.attempt_id, expected_revision: run.revision, expected_sequence: run.sequence, execution: run.execution });
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
    $("log-status").textContent = "Select a node and read actual retained logs. Worker output arrives on step completion; diagnostics are native provider logs.";
  }
  function cursorValue(value) {
    if (typeof value === "number" && !Number.isSafeInteger(value)) throw new Error("The service returned a cursor that cannot be represented exactly. No approximate cursor was used.");
    const source = String(value);
    if (!/^\d+$/.test(source) || BigInt(source) > 18446744073709551615n) throw new Error("A stream cursor must be an unsigned 64-bit decimal integer.");
    return BigInt(source).toString();
  }
  async function readLog(follow = false) {
    if (state.logBusy || !state.selected || !$("log-node").value) return;
    const id = state.selected;
    const node = $("log-node").value;
    const stream = $("log-stream").value;
    const key = `${id}\0${node}\0${stream}`;
    let cursor;
    try { cursor = cursorValue(follow ? state.logNext : $("log-cursor").value); }
    catch (error) { $("log-status").textContent = explain(error); return; }
    if (!follow || key !== state.logKey) {
      state.logGeneration++;
      state.logKey = key;
      $("log-output").textContent = "";
      $("log-gap").hidden = true;
    }
    const generation = state.logGeneration;
    state.logBusy = true;
    $("read-log").disabled = true;
    $("log-status").textContent = "Reading retained stream cursor…";
    try {
      const maximum = state.run?.log_bounds?.maximum_chunk_bytes;
      if (!Number.isSafeInteger(maximum) || maximum <= 0) throw new Error("The run has no valid admitted log-read bound.");
      const readBytes = Math.min(LOG_READ_BYTES, maximum);
      const result = await api(`${pathFor(id)}/logs?${new URLSearchParams({ node_id: node, stream, cursor, max_bytes: String(readBytes) })}`);
      if (generation !== state.logGeneration || id !== state.selected) return;
      if (result.node_id !== node || result.stream !== stream || typeof result.text !== "string") throw new Error("The returned log identity does not match the selected node and stream.");
      const expectedSource = stream === "diagnostic" ? "native-provider" : "worker-capture";
      if (result.source !== expectedSource) throw new Error("The returned log source does not match the selected stream.");
      const sourceLabel = result.source === "worker-capture" ? "Worker-captured output · byte cursor" : "Native provider diagnostic · native cursor";
      const next = cursorValue(result.next_cursor);
      const returned = cursorValue(result.cursor);
      if (BigInt(next) < BigInt(returned)) throw new Error("The stream cursor moved backwards; automatic following stopped.");
      const gaps = [];
      if (result.gap) gaps.push(`Source reports a log gap: ${text(result.gap)}. Missing bytes are not reconstructed.`);
      if (returned !== cursor) gaps.push(`Source returned cursor ${returned} instead of requested ${cursor}.`);
      if (result.truncated) gaps.push("Captured output or this read is truncated. Missing bytes are not reconstructed; retained bytes may remain at the next cursor.");
      let output = $("log-output").textContent + result.text;
      if (output.length > MAX_DISPLAYED_LOG_CHARS) {
        output = output.slice(-MAX_DISPLAYED_LOG_CHARS);
        gaps.push("Older displayed log text was removed at this page’s memory bound. Re-read an earlier stream cursor to inspect retained bytes.");
      }
      $("log-output").textContent = output;
      if (gaps.length) {
        $("log-gap").textContent = [$("log-gap").hidden ? "" : $("log-gap").textContent, ...gaps].filter(Boolean).join("\n").slice(-8192);
        $("log-gap").hidden = false;
      }
      state.logNext = next;
      $("log-cursor").value = next;
      $("log-position").textContent = `${sourceLabel} · ${returned} → ${next} · retained: ${text(result.retained)}`;
      $("log-status").textContent = result.text ? `${sourceLabel}. Actual retained bytes loaded; text is displayed inertly.` : "No log bytes returned at this cursor. This does not imply execution success or complete logs.";
      if ($("follow-logs").checked) $("log-output").scrollTop = $("log-output").scrollHeight;
    } catch (error) {
      if (!isAborted(error) && generation === state.logGeneration) {
        $("log-status").textContent = `Logs unavailable: ${explain(error)} Following stopped; retained displayed bytes may be stale.`;
        $("follow-logs").checked = false;
      }
    } finally {
      if (generation === state.logGeneration) {
        state.logBusy = false;
        $("read-log").disabled = !$("log-node").value;
        if ($("follow-logs").checked && state.token) state.logTimer = setTimeout(() => readLog(true), LOG_FOLLOW_MS);
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
      if (!isAborted(error) && id === state.selected) $("artifact-status").textContent = `Download unavailable: ${explain(error)}`;
    } finally {
      if (epoch === state.epoch) { button.disabled = false; button.textContent = "Download"; }
    }
  }

  function logout(message = "Signed out. Private views and pending requests were cleared.") {
    state.epoch++;
    state.token = null;
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
    for (const id of ["run-list", "run-title", "run-facts", "run-badges", "run-warning", "execution-identity", "node-list", "artifact-list", "artifact-status", "available-description", "project-description"]) $(id).replaceChildren();
    for (const id of ["available-project", "enrolled-project", "run-filter", "log-node"]) $(id).replaceChildren();
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
  $("login-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const token = $("operator-token").value.trim();
    if (!/^[0-9a-fA-F]{64}$/.test(token)) { notice("Enter exactly 64 hexadecimal characters for the operator token."); return; }
    $("login-button").disabled = true;
    state.token = token;
    state.epoch++;
    const epoch = state.epoch;
    $("operator-token").value = "";
    try {
      await loadProjects();
      if (epoch !== state.epoch || !state.token) return;
      $("login-panel").hidden = true;
      $("workspace").hidden = false;
      $("logout").hidden = false;
      $("auth-status").textContent = "Authenticated operator";
      $("auth-status").dataset.state = "connected";
      notice("");
      startLive();
      try { await loadRuns(); } catch (error) { if (!isAborted(error) && epoch === state.epoch) notice(`Run list unavailable: ${explain(error)}`); }
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
  $("enrolled-project").addEventListener("change", projectDescriptions);
  $("run-filter").addEventListener("change", renderRunList);
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
    const result = await mutate("/api/runs", { project_id: projectId }, "Run request admitted. Inspect phase and outcome for actual execution state.");
    if (!result) return;
    await readAction(loadRuns)();
    if (result.id && state.token) await selectRun(result.id);
  });
  for (const action of ["cancel", "pause", "resume"]) $("control-" + action).addEventListener("click", () => { void prepareControl(action); });
  $("dismiss-control").addEventListener("click", dismissControl);
  $("control-dialog").addEventListener("cancel", dismissControl);
  $("control-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const pending = state.confirm;
    if (!pending || state.mutation) return;
    dismissControl();
    await mutate(`${pathFor(pending.id)}/control`, pending.identity, "Control response received. Observe admitted state for its result; no success is inferred.");
    if (state.token && pending.id === state.selected) await readAction(loadDetail)();
  });
  $("log-form").addEventListener("submit", (event) => { event.preventDefault(); stopLogFollow(); void readLog(false); });
  for (const id of ["log-node", "log-stream"]) $(id).addEventListener("change", resetLogs);
  $("follow-logs").addEventListener("change", () => { stopLogFollow(); if ($("follow-logs").checked) void readLog(state.logKey !== ""); });
  $("clear-log").addEventListener("click", resetLogs);
  // A restored back/forward-cache page must not resurrect a prior operator session.
  window.addEventListener("pagehide", () => logout("Signed out when leaving the live page."));
})();
