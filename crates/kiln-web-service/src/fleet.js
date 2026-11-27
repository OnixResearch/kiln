// The Fleet page projects the fleet coordinator's loopback API into the console.
// It is a read view plus the same explicit coordinator commands an operator uses
// from the fleet CLI: claim, renew, release, answer a decision and cancel an
// attempt. The fleet service remains the authority for every command.
(() => {
  "use strict";
  const STREAM_RECONNECT_MS = 1_000;
  const CAIRN_REFRESH_MS = 15_000;
  const CAIRN_INDEX = "/api/implementation/cairn";
  const MACHINES = "/api/fleet/machines";
  const HERDR_MACHINE = /^[a-z0-9][a-z0-9-]{0,62}$/;
  const HERDR_PANE = /^(?!-)[a-zA-Z0-9_-]+:p[a-zA-Z0-9]+$/;
  const HERDR_CAPTURE_LINES = 200;
  const MAX_HERDR_WINDOWS = 256;
  // The read-only viewer asks for the whole visible screen, which Kiln bounds.
  const HERDR_VIEWER_LINES = 1_000;
  const HERDR_VIEWER_REFRESH_MS = 2_000;
  const HERDR_VIEWER_MAX_ANSI = 64 * 1_024;
  const HERDR_VIEWER_MAX_CELLS = 500;
  const HERDR_VIEWER_MODULE = "/herdr-viewer.js";
  const CLAIMS_INCLUDE = "/api/fleet/claims?include=foreign,expired";
  const DISPATCH_STATUS = "/api/fleet/dispatch/status";
  const NATIVE_WORK = "/api/work";
  const MACHINE_STATES = new Map([
    ["reporting", ["Reporting", "good"]], ["unreachable", ["Unreachable", "failure"]],
    ["not_reporting", ["Not reporting", ""]], ["stale", ["Stale", "warning"]],
  ]);
  const CORE_STATUS_LABELS = Object.entries({
    idle: "Idle", working: "Working", done: "Done", blocked: "Blocked", error: "Error",
    unknown: "Unknown (program not reporting)",
  });
  const CORE_FIELDS = ["state", "observed_at_ms", "last_seen_at_ms", "terminals", "statuses"];
  const SOURCE_KINDS = new Map([
    ["github_issue", ["GitHub issue", "github", "issue"]],
    ["cairn_task", ["Cairn task", "cairn", "task"]], ["cairn_artifact", ["Cairn artifact", "cairn", "artifact"]],
  ]);
  const AGGREGATE_REFRESH_MS = CAIRN_REFRESH_MS;
  const STREAM_BACKOFF_MAX_MS = 30_000;
  const CONTROL_FRESHNESS_MS = 30_000;
  const LEASE_NOTICE_MS = 15 * 60_000;
  const ATTENTION_LIMIT = 8;
  const IDENTITY = /^[0-9a-f]{64}$/;
  const WORK_OBJECT = /^[0-9a-f]{40}$/;
  const VERSION = /^[0-9a-f]{64}$/;
  const TOOL = "kiln-web";
  const SESSION_KEY = "kiln-fleet-session";
  const QUERY_MAX = 256;
  const PURPOSE_MAX = 1024;
  const ANSWER_MAX = 4096;
  const REASON_MAX = 1024;
  const ATTEMPT_STATES = new Map([
    ["claimed", "Claimed"], ["executing", "Executing"],
    ["candidate_produced", "Candidate produced"], ["verifying", "Verifying"],
    ["terminal", "Terminal"], ["unknown", "Unknown"],
  ]);

  const ui = () => window.KilnUi;
  const element = (tag, value, className) => {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = String(value);
    if (className) node.className = className;
    return node;
  };
  const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const count = (value) => Number.isSafeInteger(value) && value >= 0 ? value : null;

  function sessionId() {
    try {
      const existing = sessionStorage.getItem(SESSION_KEY);
      if (existing && /^web-[a-z0-9]{6,16}$/.test(existing)) return existing;
      const bytes = crypto.getRandomValues(new Uint8Array(8));
      const created = `web-${Array.from(bytes, (byte) => (byte % 36).toString(36)).join("")}`;
      sessionStorage.setItem(SESSION_KEY, created);
      return created;
    } catch (_) {
      return "web-session";
    }
  }

  function stateText(value) {
    if (typeof value === "string") return value;
    if (record(value)) {
      const keys = Object.keys(value);
      return keys.length ? keys[0] : "Unknown";
    }
    return "Unknown";
  }

  function label(value) {
    const words = stateText(value).replaceAll("_", " ");
    return words.charAt(0).toUpperCase() + words.slice(1);
  }

  function blockReason(reason) {
    const kind = record(reason) ? reason.state : reason;
    if (kind === "missing_dependency" && IDENTITY.test(text(reason?.identity))) {
      return `Missing dependency: ${reason.identity}`;
    }
    return label(kind);
  }

  function scopeText(scope) {
    if (!record(scope)) return "Not recorded";
    const target = typeof scope.target === "string" ? scope.target : record(scope.target) ? text(scope.target.repository) || text(scope.target.machine) : "";
    const path = typeof scope.path === "string" ? scope.path : record(scope.path) ? text(scope.path.path) : "";
    return `${target || "Scope not recorded"}${path && path !== "whole" ? ` · ${path}` : ""}`;
  }

  function holderText(holder) {
    if (!record(holder)) return "Not recorded";
    const tool = text(holder.tool) || "unknown tool";
    const host = text(holder.host) || "unknown host";
    const session = text(holder.session);
    return `${tool} on ${host}${session ? ` · ${session}` : ""}`;
  }

  function empty(title, message) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    return node;
  }

  function table(headers, rows, emptyTitle, emptyMessage) {
    if (!rows.length) return empty(emptyTitle, emptyMessage);
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack fleet-table");
    const head = element("thead");
    const headings = element("tr");
    for (const label of headers) {
      const cell = element("th", label);
      cell.scope = "col";
      headings.append(cell);
    }
    head.append(headings);
    const body = element("tbody");
    for (const cells of rows) {
      const row = element("tr");
      cells.forEach((value, index) => {
        const cell = element("td");
        cell.dataset.label = headers[index] || "";
        if (value instanceof Node) cell.append(value);
        else cell.textContent = text(value) || "Not recorded";
        row.append(cell);
      });
      body.append(row);
    }
    table.append(head, body);
    wrap.append(table);
    return wrap;
  }

  function identityCell(identity) {
    const wrap = element("span", undefined, "fleet-identity");
    const short = element("code", text(identity).slice(0, 12) || "Not recorded");
    const full = element("span", text(identity), "sr-only");
    wrap.title = text(identity);
    wrap.append(short, full);
    return wrap;
  }

  function when(ms) {
    return count(ms) === null ? element("span", "Not recorded", "not-recorded") : ui().timeElement(ms);
  }

  function validateClaims(payload) {
    if (!record(payload) || !Array.isArray(payload.claims)) throw new Error("Fleet claims response is invalid.");
    for (const claim of payload.claims) {
      if (!record(claim) || !IDENTITY.test(text(claim.identity)) || !Number.isSafeInteger(claim.fence) || claim.fence < 0) throw new Error("Fleet claims response is invalid.");
    }
  }

  function validateWork(payload) {
    if (!record(payload) || count(payload.ready_unclaimed) === null || !Array.isArray(payload.ranked) || !Array.isArray(payload.blocked)) throw new Error("Fleet work response is invalid.");
  }

  function validateAttempts(payload) {
    if (!record(payload) || !Array.isArray(payload.attempts)) throw new Error("Fleet attempts response is invalid.");
    for (const attempt of payload.attempts) {
      if (!record(attempt) || !IDENTITY.test(text(attempt.attempt))) throw new Error("Fleet attempts response is invalid.");
    }
  }

  function validateDecisions(payload) {
    if (!record(payload) || !Array.isArray(payload.decisions)) throw new Error("Fleet decisions response is invalid.");
    for (const decision of payload.decisions) {
      if (!record(decision) || !IDENTITY.test(text(decision.identity))) throw new Error("Fleet decisions response is invalid.");
    }
  }

  function validateHealth(payload) {
    if (!record(payload) || payload.api !== "v1" || !text(payload.machine) || !text(payload.version)) throw new Error("Fleet health response is invalid.");
  }

  function metric(label, value) {
    const cell = element("div");
    cell.append(element("dt", label), element("dd", value === null ? "–" : String(value)));
    return cell;
  }

  function attemptState(attempt) {
    const reported = stateText(attempt.state);
    return ATTEMPT_STATES.has(reported) ? reported : "unknown";
  }

  const attemptMachine = (attempt) => text(attempt.session?.machine);
  const claimMachine = (claim) => text(claim.holder?.host);

  function hasExactFields(value, fields) {
    return record(value) && Object.keys(value).length === fields.length
      && fields.every((field) => Object.hasOwn(value, field));
  }

  function validCore(core) {
    return hasExactFields(core, CORE_FIELDS) && MACHINE_STATES.has(core.state)
      && [core.observed_at_ms, core.last_seen_at_ms, core.terminals].every((value) => value === null || count(value) !== null)
      && (core.statuses === null || (record(core.statuses) && Object.keys(core.statuses).length === CORE_STATUS_LABELS.length
        && CORE_STATUS_LABELS.every(([key]) => Object.hasOwn(core.statuses, key) && count(core.statuses[key]) !== null)));
  }

  function admitMachines(payload) {
    if (!Array.isArray(payload?.machines)) throw new Error("Machine roster response is invalid.");
    const seen = new Set();
    for (const row of payload.machines) {
      if (!text(row?.machine) || seen.has(row.machine) || typeof row.placement_configured !== "boolean"
        || !MACHINE_STATES.has(row.state) || [row.observed_at_ms, row.last_seen_at_ms, row.sessions]
          .some((value) => value !== null && count(value) === null)
        || (Object.hasOwn(row, "core") && !validCore(row.core))) throw new Error("Machine roster response is invalid.");
      seen.add(row.machine);
    }
    return payload.machines;
  }

  function machineCounts(rows, keyOf, roster) {
    if (!rows) return null;
    const byMachine = new Map();
    let missing = 0, unmatched = 0;
    for (const row of rows) {
      const key = keyOf(row);
      if (!key) missing += 1;
      else if (!roster.has(key)) unmatched += 1;
      else byMachine.set(key, (byMachine.get(key) || 0) + 1);
    }
    return { byMachine, missing, unmatched };
  }

  function machineStateBadge(state) {
    const [name, tone] = MACHINE_STATES.get(state);
    const badge = element("span", name, "state-badge");
    if (tone) badge.dataset.tone = tone;
    badge.dataset.state = state;
    return badge;
  }

  function coreObservation(core) {
    if (!core) return "Not selected";
    const wrap = element("div", undefined, "fleet-core-observation");
    const lastReported = core.state === "reporting" ? "" : " (last reported)";
    wrap.append(machineStateBadge(core.state), element("div", core.terminals === null
      ? "Terminals: not observed" : `Terminals: ${core.terminals}${lastReported}`));
    if (core.statuses === null) {
      wrap.append(element("div", "Terminal statuses: not observed", "fleet-observation-status"));
    } else {
      wrap.append(element("div", `Terminal statuses${lastReported}`));
      const statuses = element("dl", undefined, "fleet-core-statuses");
      for (const [key, name] of CORE_STATUS_LABELS) {
        statuses.append(element("dt", name), element("dd", core.statuses[key]));
      }
      wrap.append(statuses);
    }
    const times = element("div", undefined, "fleet-core-times");
    for (const [caption, value] of [["Observed at", core.observed_at_ms], ["Last seen", core.last_seen_at_ms]]) {
      const line = element("div", `${caption}: `);
      line.append(when(value));
      times.append(line);
    }
    wrap.append(times);
    return wrap;
  }

  function machineSlotsCell(row) {
    if (row.health === "unavailable") {
      const wrap = element("div", undefined, "fleet-machine-unavailable");
      const badge = element("span", "OFFLINE · Unavailable", "state-badge");
      badge.dataset.tone = "failure";
      wrap.append(badge);
      const detail = text(row.error?.detail) || text(row.error?.code) || text(row.error?.class);
      if (detail) wrap.append(element("div", detail, "small warning"));
      return wrap;
    }
    if (record(row.slots) && Object.keys(row.slots).length) {
      const entries = Object.entries(row.slots).map(([kind, s]) => `${kind}: ${s?.used ?? 0}/${s?.capacity ?? 0}`);
      return entries.join(" · ");
    }
    return "Not reported";
  }

  function closeHerdrWindows(state) {
    stopHerdrViewer(state);
    state.herdrWindowRequest?.abort();
    state.herdrWindowRequest = null;
    state.herdrWindowsHost = "";
    state.herdrWindowsPanel.hidden = true;
    state.herdrWindowsPanel.replaceChildren();
  }

  function herdrRequestCurrent(state, controller) {
    return active === state && state.context.isCurrent() && currentPrincipal() === state.principal
      && state.herdrWindowRequest === controller;
  }

  function windowRequest(state) {
    stopHerdrViewer(state);
    state.herdrWindowRequest?.abort();
    state.herdrWindowRequest = new AbortController();
    return state.herdrWindowRequest;
  }

  const sameWindow = (observed, windowId) => observed?.pane_id === windowId.pane_id
    && observed?.workspace_id === windowId.workspace_id && observed?.tab_id === windowId.tab_id;

  async function captureHerdrWindow(state, host, path, windowId, result, note = "") {
    if (active !== state || state.herdrWindowsHost !== host || state.context.canOperate?.() !== true) return;
    const controller = windowRequest(state);
    const current = () => herdrRequestCurrent(state, controller) && state.context.canOperate?.() === true;
    result.replaceChildren(observationStatus(`Capturing ${host} / ${windowId.pane_id}…`));
    const noted = (...nodes) => result.replaceChildren(...(note ? [observationStatus(note)] : []), ...nodes);
    try {
      const value = await state.context.api(`${path}/${windowId.pane_id}/capture?lines=${HERDR_CAPTURE_LINES}`, {
        method: "POST", body: {}, quiet: true, controller,
      });
      if (!current()) return;
      if (value?.machine !== host || !sameWindow(value.window, windowId)
        || value.requested_lines !== HERDR_CAPTURE_LINES || count(value.line_count) === null
        || value.line_count > HERDR_CAPTURE_LINES || typeof value.truncated !== "boolean"
        || typeof value.text !== "string") throw new Error("Capture identity or bounds mismatch");
      noted(
        observationStatus(`${host} / ${windowId.pane_id} · ${value.line_count} lines${value.truncated ? " · Limited by Kiln" : ""}`),
        value.text ? element("pre", value.text, "mono") : element("p", "No terminal output returned.", "small"),
      );
    } catch (error) {
      if (!current()) return;
      noted(observationStatus(`${host} / ${windowId.pane_id} · ${captureFailure(error)}`));
    }
  }

  function captureFailure(error) {
    return error?.code === "fleet_herdr_window_unknown"
      ? "This window is no longer in the host inventory. Refresh windows to select again."
      : error?.status === 403 ? "Only an operator can capture terminal output." : "Terminal capture unavailable.";
  }

  // r[impl kiln.sdlc.herdr_viewer.scenario.view]
  // The viewer is read-only: it renders bounded visible-screen snapshots and
  // never sends input. The generated module loads only when first opened.
  async function herdrViewerModule() {
    if (!window.KilnHerdrViewer) await import(HERDR_VIEWER_MODULE);
    if (typeof window.KilnHerdrViewer?.load !== "function") throw new Error("Herdr viewer module unavailable");
    return window.KilnHerdrViewer.load();
  }

  function stopHerdrViewer(state) {
    const viewer = state.herdrViewer;
    if (!viewer) return;
    state.herdrViewer = null;
    stopViewerRefresh(viewer);
    try {
      viewer.terminal?.dispose();
    } catch {
      // The canvas is already detached.
    }
  }

  function stopViewerRefresh(viewer) {
    viewer.stopped = true;
    clearTimeout(viewer.timer);
    viewer.timer = null;
    viewer.controller?.abort();
    viewer.controller = null;
  }

  function admitAnsiCapture(value, host, windowId) {
    const side = (cells) => cells === null || (Number.isSafeInteger(cells) && cells >= 1 && cells <= HERDR_VIEWER_MAX_CELLS);
    return value?.machine === host && sameWindow(value.window, windowId) && value.format === "ansi"
      && value.source === "visible" && value.requested_lines === HERDR_VIEWER_LINES
      && count(value.line_count) !== null && value.line_count <= HERDR_VIEWER_LINES
      && typeof value.truncated === "boolean" && typeof value.ansi === "string"
      && value.ansi.length <= HERDR_VIEWER_MAX_ANSI && side(value.columns) && side(value.rows);
  }

  async function openHerdrViewer(state, host, path, windowId, result) {
    if (active !== state || state.herdrWindowsHost !== host || state.context.canOperate?.() !== true) return;
    windowRequest(state);
    state.herdrWindowRequest = null;
    const label = `${host} / ${windowId.pane_id}`;
    const status = observationStatus(`Opening viewer for ${label}…`);
    const close = element("button", "Close viewer", "quiet");
    close.type = "button";
    close.setAttribute("aria-label", `Close viewer for ${label}`);
    const surface = element("div", undefined, "fleet-herdr-viewer");
    result.replaceChildren(status, close, surface);
    const viewer = { label, timer: null, controller: null, terminal: null, stopped: false, paused: false };
    state.herdrViewer = viewer;
    const current = () => active === state && state.herdrViewer === viewer && state.context.isCurrent()
      && currentPrincipal() === state.principal && state.context.canOperate?.() === true;
    close.addEventListener("click", () => {
      if (state.herdrViewer !== viewer) return;
      stopHerdrViewer(state);
      result.replaceChildren(observationStatus(`${label} · Viewer closed.`));
    });
    const fallBack = (note) => {
      stopHerdrViewer(state);
      return captureHerdrWindow(state, host, path, windowId, result, note);
    };
    try {
      const module = await herdrViewerModule();
      if (!current()) return;
      viewer.terminal = module.mount(surface);
    } catch {
      if (current()) await fallBack("The terminal viewer is unavailable in this browser; showing plain text.");
      return;
    }
    viewer.refresh = async () => {
      viewer.timer = null;
      if (!current() || viewer.stopped) return;
      if (document.visibilityState === "hidden") {
        viewer.paused = true;
        status.textContent = `${label} · Paused while the page is hidden`;
        return;
      }
      viewer.paused = false;
      const controller = new AbortController();
      viewer.controller = controller;
      let value;
      try {
        value = await state.context.api(
          `${path}/${windowId.pane_id}/capture?lines=${HERDR_VIEWER_LINES}&format=ansi&source=visible`,
          { method: "POST", body: {}, quiet: true, controller },
        );
        if (!current() || viewer.controller !== controller) return;
        if (!admitAnsiCapture(value, host, windowId)) throw new Error("Viewer capture identity or bounds mismatch");
      } catch (error) {
        if (!current() || viewer.controller !== controller) return;
        stopViewerRefresh(viewer);
        status.textContent = `${label} · Refresh stopped · ${captureFailure(error)}`;
        return;
      }
      viewer.controller = null;
      let clipped = false;
      try {
        clipped = viewer.terminal.render(new TextEncoder().encode(value.ansi), value.columns ?? 0, value.rows ?? 0) === true;
      } catch {
        await fallBack("The terminal viewer could not draw this window; showing plain text.");
        return;
      }
      const observed = new Date();
      status.textContent = `${label} · Visible screen${value.truncated ? " · Limited by Kiln" : ""}${clipped ? " · Clipped to viewer grid" : ""} · Read ${observed.toLocaleTimeString()} · Refreshing every ${HERDR_VIEWER_REFRESH_MS / 1000}s while open and visible`;
      viewer.timer = setTimeout(() => void viewer.refresh(), HERDR_VIEWER_REFRESH_MS);
    };
    await viewer.refresh();
  }

  async function openHerdrWindows(state, host) {
    if (active !== state || !state.context.isCurrent() || !state.listedMachines?.some(
      (row) => row.machine === host && row.herdr_windows_configured === true,
    )) return;
    const controller = windowRequest(state);
    const current = () => herdrRequestCurrent(state, controller);
    state.herdrWindowsHost = host;
    const panel = state.herdrWindowsPanel;
    const path = `${MACHINES}/${host}/windows`;
    const refresh = element("button", "Refresh windows", "quiet");
    refresh.type = "button";
    refresh.addEventListener("click", () => void openHerdrWindows(state, host));
    const close = element("button", "Close windows", "quiet");
    close.type = "button";
    close.addEventListener("click", () => closeHerdrWindows(state));
    const inventory = element("div");
    inventory.append(observationStatus("Reading current Herdr windows…"));
    const result = element("div");
    panel.hidden = false;
    panel.replaceChildren(element("h3", `Herdr windows · ${host}`), refresh, close, inventory, result);
    try {
      const value = await state.context.api(path, { quiet: true, controller });
      if (!current()) return;
      if (value?.machine !== host || !Array.isArray(value.windows) || value.windows.length > MAX_HERDR_WINDOWS) {
        throw new Error("Window inventory invalid");
      }
      const seen = new Set();
      for (const row of value.windows) {
        if (typeof row?.pane_id !== "string" || row.pane_id.length > 64 || !HERDR_PANE.test(row.pane_id)
          || !text(row.workspace_id) || !text(row.tab_id) || seen.has(row.pane_id)) throw new Error("Window identity invalid");
        seen.add(row.pane_id);
      }
      if (!value.windows.length) {
        inventory.replaceChildren(observationStatus("No Herdr windows observed on this host."));
        return;
      }
      const operator = state.context.canOperate?.() === true;
      const rows = value.windows.map((row) => {
        const cells = [row.workspace_id, row.tab_id, element("code", row.pane_id)];
        if (operator) {
          const capture = element("button", "Capture output", "quiet");
          capture.type = "button";
          capture.setAttribute("aria-label", `Capture output for ${host} / ${row.pane_id}`);
          capture.addEventListener("click", () => void captureHerdrWindow(state, host, path, row, result));
          const view = element("button", "Open viewer", "quiet");
          view.type = "button";
          view.setAttribute("aria-label", `Open viewer for ${host} / ${row.pane_id}`);
          view.addEventListener("click", () => void openHerdrViewer(state, host, path, row, result));
          const actions = element("div", undefined, "fleet-herdr-actions");
          actions.append(capture, view);
          cells.push(actions);
        }
        return cells;
      });
      const headers = ["Workspace", "Tab", "Window", ...(operator ? ["Terminal"] : [])];
      inventory.replaceChildren(table(headers, rows));
      if (!operator) result.append(observationStatus("Only operators can capture or view terminal output."));
    } catch {
      if (current()) inventory.replaceChildren(observationStatus("Herdr windows unavailable on this host."));
    }
  }

  function renderMachines(state) {
    if (!state.listedMachines) return;
    const roster = new Set(state.listedMachines.map(({ machine }) => machine));
    const attempts = machineCounts(state.listedAttempts, attemptMachine, roster);
    const claims = machineCounts(state.listedClaims, claimMachine, roster);
    const signature = JSON.stringify([state.listedMachines, attempts ? [...attempts.byMachine] : null,
      claims ? [...claims.byMachine] : null, attempts?.missing, attempts?.unmatched, claims?.missing, claims?.unmatched,
      state.context.canOperate?.() !== false]);
    if (signature === state.machineSignature) return;
    state.machineSignature = signature;
    const showCore = state.listedMachines.some((row) => Object.hasOwn(row, "core"));
    const rows = state.listedMachines.map((row) => {
      const sessions = row.sessions === null ? "Not observed" : `${row.sessions}${row.state === "reporting" ? "" : " (last reported)"}`;
      const obsCell = element("div", undefined, "fleet-actions");
      obsCell.append(machineStateBadge(row.state));
      if (row.draining) {
        const drainBadge = element("span", "Draining", "state-badge");
        drainBadge.dataset.tone = "warning";
        obsCell.append(drainBadge);
      }
      const actions = element("div", undefined, "fleet-actions");
      if (row.herdr_windows_configured === true && HERDR_MACHINE.test(row.machine)) {
        const open = element("button", "Browse Herdr windows", "quiet");
        open.type = "button";
        open.setAttribute("aria-label", `Browse Herdr windows on ${row.machine}`);
        open.addEventListener("click", () => void openHerdrWindows(state, row.machine));
        actions.append(open);
      }
      if (state.context.canOperate?.() !== false) {
        const draining = Boolean(row.draining);
        const drainBtn = element("button", draining ? "Undrain" : "Drain", "quiet");
        drainBtn.type = "button";
        drainBtn.addEventListener("click", async () => {
          const nextDraining = !draining;
          const choice = await confirmGuardedFleetMutation(state, {
            title: `${nextDraining ? "Drain" : "Undrain"} machine ${row.machine}?`,
            verb: nextDraining ? "Drain machine" : "Undrain machine",
            dismiss: nextDraining ? "Keep active" : "Keep draining",
            danger: nextDraining,
            warning: nextDraining
              ? "Stops new attempt placement onto this machine. Existing running work continues unless separately cancelled."
              : "Allows new attempt placement onto this machine again.",
            facts: [
              ["Machine", row.machine],
              ["Current state", row.health === "unavailable" ? "Unavailable" : label(row.state)],
              ["Drain status", draining ? "Draining" : "Active"],
            ],
            reason: nextDraining ? { label: "Drain reason", placeholder: "Why this machine is being drained", required: true, maxLength: REASON_MAX } : null,
            cliCommand: `kiln-fleet machines ${nextDraining ? "drain" : "undrain"} ${row.machine}`,
            opener: drainBtn,
          });
          if (!choice.confirmed) return;
          const reqId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `drain-${Date.now()}`;
          void mutate(state, drainBtn, `/api/fleet/machines/${encodeURIComponent(row.machine)}/drain`, {
            draining: nextDraining,
            ...(choice.reason ? { reason: choice.reason } : {}),
            request_id: reqId,
          });
        });
        actions.append(drainBtn);
      } else {
        actions.append(element("span", "Read only", "small"));
      }
      return [element("code", row.machine), row.placement_configured ? "Configured" : "Not configured", obsCell,
        machineSlotsCell(row), when(row.observed_at_ms), when(row.last_seen_at_ms), sessions,
        attempts ? String(attempts.byMachine.get(row.machine) ?? 0) : "Not available",
        claims ? String(claims.byMachine.get(row.machine) ?? 0) : "Not available",
        ...(showCore ? [coreObservation(row.core)] : []),
        actions];
    });
    const headers = ["Machine", "Placement", "Observation", "Slots / Health", "Observed at", "Last seen", "Sessions", "Assigned attempts", "Held claims"];
    if (showCore) headers.push("Herdr Core");
    headers.push("Actions");
    state.machineRows.replaceChildren(table(headers, rows, "No enrolled machines listed", "No machine rows were returned by the roster endpoint."));
    const gaps = [[attempts, "Attempts", "session machine"], [claims, "Claims", "holder machine"]].flatMap(([counts, name, field]) => {
      if (!counts) return [`${name} not yet available.`];
      return counts.missing || counts.unmatched ? [`${name} not attributed: ${counts.missing} without a ${field}, ${counts.unmatched} outside this roster.`] : [];
    });
    notice(state.machineCoverage, gaps.join(" "));
  }

  function renderNativeWork(state, snapshot) {
    if (snapshot?.schema !== "kiln.work.v1" || snapshot.bounded !== true || typeof snapshot.configured !== "boolean"
      || !Array.isArray(snapshot.objects) || snapshot.principal !== state.principal) throw new Error("Native work response refused.");
    const work = snapshot.configured ? snapshot.objects.filter((row) => row?.kind === "issue" || row?.kind === "request") : [];
    const statusText = snapshot.configured ? "Bounded to native records visible to this principal." : "Native work workspace not configured.";
    const signature = JSON.stringify([snapshot.configured, snapshot.truncated, work.map((row) => [row.id, row.kind, row.fields?.title, row.fields?.status])]);
    if (signature === state.issueSignature) return;
    state.issueSignature = signature;
    let incomplete = snapshot.truncated === true;
    const ids = new Map();
    for (const row of work) ids.set(row.id, (ids.get(row.id) || 0) + 1);
    const rows = [];
    for (const row of work) {
      if (!WORK_OBJECT.test(text(row.id)) || ids.get(row.id) !== 1) { incomplete = true; continue; }
      const title = text(row.fields?.title), status = text(row.fields?.status);
      if (!title || !status) incomplete = true;
      const page = row.kind === "request" ? "triage" : "work";
      const patch = { page, work_id: row.id, work_kind: "", work_view: "", work_q: "", work_status: "",
        work_assignee: "", work_project: "", work_ci_project: "", work_ci_repository: "", work_layout: "list" };
      const link = exactLink(state, title || "Title not recorded", patch, row.id);
      link.dataset.workId = row.id;
      rows.push([link, label(row.kind), status ? label(status) : "Not recorded", identityCell(row.id)]);
    }
    state.issueRows.replaceChildren(snapshot.configured
      ? table(["Title", "Kind", "State", "Object ID"], rows, "No issues or requests listed", incomplete
        ? "The native work response was incomplete." : "The authorized snapshot returned no issues or requests.")
      : empty("Native issues unavailable", "Configure the native work workspace to read issues and requests."));
    notice(state.issueStatus, `${statusText}${incomplete ? " Some records or fields are missing or could not be read." : ""}`);
  }

  function renderWorkSources(state, work, attempts) {
    const references = new Map();
    let incomplete = false;
    for (const [bucket, entries, identityField] of [["Ranked", work.ranked, "identity"], ["Blocked", work.blocked, "identity"],
      ["Attempts", attempts.attempts, "work"]]) {
      for (const entry of entries) {
        const refs = entry?.sources, identity = text(entry?.[identityField]);
        if (!Array.isArray(refs) || !IDENTITY.test(identity)) { incomplete = true; continue; }
        for (const ref of refs) {
          const kind = SOURCE_KINDS.get(ref?.kind);
          const source = ref?.source, id = text(ref?.native_id), version = text(ref?.version);
          if (!kind || !id || !VERSION.test(version) || source?.contract_version !== 1 || source.provider_namespace !== kind[1]
            || source.native_kind !== kind[2] || source.native_id !== id) { incomplete = true; continue; }
          const key = JSON.stringify([identity, ref.kind, id, version]);
          const known = references.get(key);
          if (known) { known.buckets.add(bucket); continue; }
          const github = ref.kind === "github_issue" && /^([a-z0-9-]+)\/([a-z0-9_.-]+)#([1-9][0-9]*)$/.exec(id);
          const linkable = github && github[2] !== "." && github[2] !== ".." && `${github[1]}/${github[2]}` === entry.repository;
          const reference = element(linkable ? "a" : "code", id, linkable ? "row-link" : undefined);
          if (linkable) reference.href = `https://github.com/${github[1]}/${github[2]}/issues/${github[3]}`;
          references.set(key, { identity, kind: kind[0], reference, version, buckets: new Set([bucket]) });
        }
      }
    }
    const truncated = work.truncated === true || attempts.truncated === true;
    const rows = [...references.values()].map(({ identity, kind, reference, version, buckets }) =>
      [identityCell(identity), [...buckets].join(" · "), kind, reference, identityCell(version)]);
    state.sourceRows.replaceChildren(table(["Work ID", "Listed in", "Source", "Native reference", "Source version"], rows,
      "No work source references listed", incomplete || truncated
        ? "Some work or source records are missing." : "No references were returned for the listed work."));
    notice(state.sourceCoverage, [truncated ? "Fleet response is truncated." : "",
      incomplete ? "Some work entries lack readable source references." : ""].filter(Boolean).join(" "));
  }

  function attemptFacet(title, keyOf, labels) {
    const panel = element("div", undefined, "fleet-mix");
    const list = element("ul", undefined, "fleet-mix-list");
    panel.append(element("h3", title), list);
    return { panel, list, keyOf, labels, selected: null, buttons: new Map() };
  }

  function renderFacet(state, facet, attempts) {
    const counts = new Map();
    for (const attempt of attempts) {
      const key = facet.keyOf(attempt);
      counts.set(key, (counts.get(key) || 0) + 1);
    }
    if (facet.selected !== null && !counts.has(facet.selected)) counts.set(facet.selected, 0);
    const keys = facet.labels ? [...facet.labels.keys()].filter((key) => counts.has(key)) : [...counts.keys()].sort();
    const previous = facet.buttons;
    facet.buttons = new Map();
    let restoreFocus;
    const rows = keys.map((key) => {
      const value = counts.get(key);
      const name = facet.labels?.get(key) ?? (key || "No machine recorded");
      const row = element("li");
      const button = element("button", undefined, "quiet fleet-mix-toggle");
      button.type = "button";
      button.setAttribute("aria-controls", "fleet-attempt-register");
      const bar = element("meter", undefined, "fleet-mix-bar");
      bar.max = Math.max(1, attempts.length);
      bar.value = value;
      bar.setAttribute("aria-hidden", "true");
      button.append(element("span", name), bar, element("strong", value));
      button.addEventListener("click", () => {
        facet.selected = facet.selected === key ? null : key;
        applyAttemptFilters(state);
      });
      if (previous.has(key) && previous.get(key) === document.activeElement) restoreFocus = button;
      facet.buttons.set(key, button);
      row.append(button);
      return row;
    });
    facet.list.replaceChildren(...rows);
    facet.panel.hidden = !rows.length;
    return restoreFocus;
  }

  function applyAttemptFilters(state) {
    const filtering = state.facets.some((facet) => facet.selected !== null);
    for (const facet of state.facets) {
      for (const [key, button] of facet.buttons) button.setAttribute("aria-pressed", String(key === facet.selected));
    }
    const matches = filtering ? state.listedAttempts.filter((attempt) => state.facets.every((facet) => facet.selected === null || facet.keyOf(attempt) === facet.selected)) : state.listedAttempts;
    const summary = `Showing ${matches.length} of ${state.listedAttempts.length} listed attempts`;
    if (state.result.textContent !== summary) state.result.textContent = summary;
    state.clearFilters.setAttribute("aria-disabled", String(!filtering));
    renderAttemptRows(state, matches, filtering);
  }

  function renderAttention(state) {
    const now = Date.now();
    const leases = (state.listedClaims || []).filter((claim) => stateText(claim.state) === "open"
      && count(claim.expires_ms) !== null && claim.expires_ms > now && claim.expires_ms - now <= LEASE_NOTICE_MS);
    const changes = (state.cairnWorkerCells || []).map(({ row }) => row).filter((row) => row.lane === "attention" || row.blockers > 0 || row.openDecisions > 0);
    const signature = JSON.stringify([leases, changes, !!state.listedClaims, !!state.cairnWorkerCells]);
    if (signature === state.attentionSignature) return;
    state.attentionSignature = signature;
    const list = element("ul", undefined, "fleet-attention-list");
    for (const row of changes.slice(0, ATTENTION_LIMIT)) {
      const item = element("li");
      const condition = row.firstBlocker && window.KilnCairn.condition(row.firstBlocker.condition).label;
      item.append(cairnLink(state, row), element("span", condition || `${row.blockers ?? "Unknown"} blockers · ${row.openDecisions ?? "unknown"} decisions`, "small"), inspectButton(state, row));
      list.append(item);
    }
    for (const claim of leases.slice(0, ATTENTION_LIMIT)) {
      const item = element("li");
      const link = element("a", scopeText(claim.scope), "row-link");
      link.href = `#fleet-claim-${claim.identity}`;
      item.append(link, element("span", "Claim expires ", "small"), when(claim.expires_ms));
      list.append(item);
    }
    if (changes.length > ATTENTION_LIMIT || leases.length > ATTENTION_LIMIT) list.append(element("li", "More items in the Cairn and claim registers.", "small"));
    state.attention.replaceChildren(changes.length || leases.length ? list : element("p",
      state.listedClaims && state.cairnWorkerCells ? "No listed Cairn changes or claim leases need attention." : "Waiting for Cairn or Fleet observations.", "small"));
  }

  function isSnapshotFresh(state, now = Date.now()) {
    return state.connection?.dataset?.state === "live"
      && Number.isFinite(state.observedAt)
      && state.observedAt > 0
      && (now - state.observedAt) < CONTROL_FRESHNESS_MS;
  }

  async function openConfirmDialog(state, options) {
    if (typeof state.context?.confirmControl === "function") {
      return state.context.confirmControl(options);
    }
    const dialog = typeof document?.getElementById === "function" ? document.getElementById("control-dialog") : null;
    const form = typeof document?.getElementById === "function" ? document.getElementById("control-form") : null;
    if (!dialog || !form) {
      notice(state.command, "Confirmation dialog is unavailable; no command was sent.");
      return { confirmed: false, reason: "" };
    }
    const byId = (id) => document.getElementById(id);
    const title = byId("confirm-title");
    const warning = byId("confirm-warning");
    const facts = byId("confirm-facts");
    const confirmBtn = byId("confirm-control");
    const dismissBtn = byId("dismiss-control");
    const reasonField = byId("confirm-reason-field");
    const reasonLabel = byId("confirm-reason-label");
    const reasonInput = byId("confirm-reason");
    const reasonError = byId("confirm-reason-error");
    if (title) title.textContent = options.title || "Confirm action";
    if (warning) warning.textContent = options.warning || "";
    if (confirmBtn) {
      confirmBtn.textContent = options.verb || "Confirm";
      confirmBtn.className = options.danger ? "danger" : "primary";
    }
    if (dismissBtn) dismissBtn.textContent = options.dismiss || "Cancel";
    if (facts) {
      facts.replaceChildren();
      for (const [term, value] of options.facts || []) {
        if (value === undefined || value === null || value === "") continue;
        facts.append(element("dt", term), element("dd", value));
      }
    }
    if (reasonField && reasonInput) {
      if (options.reason) {
        reasonField.hidden = false;
        if (reasonLabel) reasonLabel.textContent = options.reason.label || "Reason";
        reasonInput.placeholder = options.reason.placeholder || "";
        reasonInput.maxLength = options.reason.maxLength || REASON_MAX;
        reasonInput.required = Boolean(options.reason.required);
        reasonInput.value = options.reason.initial || "";
      } else {
        reasonField.hidden = true;
        reasonInput.required = false;
        reasonInput.value = "";
      }
      if (reasonError) { reasonError.textContent = ""; reasonError.hidden = true; }
    }
    const cliWrap = byId("confirm-cli-parity");
    const cliCmd = byId("confirm-cli-cmd");
    const cliStatus = byId("confirm-cli-status");
    if (cliWrap && cliCmd) {
      if (options.cliCommand) {
        cliWrap.hidden = false;
        cliCmd.textContent = options.cliCommand;
        if (cliStatus) cliStatus.textContent = "";
      } else {
        cliWrap.hidden = true;
        cliCmd.textContent = "";
        if (cliStatus) cliStatus.textContent = "";
      }
    }
    return new Promise((resolve) => {
      let settled = false;
      const cleanup = () => {
        form.removeEventListener("submit", onSubmit);
        dismissBtn?.removeEventListener("click", onCancel);
        dialog.removeEventListener("cancel", onCancel);
        if (dialog.open) dialog.close();
        if (reasonField) reasonField.hidden = true;
        if (reasonInput) { reasonInput.value = ""; reasonInput.required = false; }
        if (reasonError) { reasonError.textContent = ""; reasonError.hidden = true; }
        if (cliWrap) cliWrap.hidden = true;
        if (cliCmd) cliCmd.textContent = "";
        if (cliStatus) cliStatus.textContent = "";
      };
      const finish = (result) => {
        if (settled) return;
        settled = true;
        cleanup();
        options.opener?.focus?.();
        resolve(result);
      };
      const onCancel = () => finish({ confirmed: false, reason: "" });
      const onSubmit = (event) => {
        event.preventDefault();
        const max = options.reason?.maxLength || REASON_MAX;
        const reason = (reasonInput?.value || "").trim().slice(0, max);
        if (options.reason?.required && !reason) {
          if (reasonError) { reasonError.textContent = "A reason is required."; reasonError.hidden = false; }
          reasonInput?.focus?.();
          return;
        }
        finish({ confirmed: true, reason, requestId: typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `fleet-${Date.now()}` });
      };
      form.addEventListener("submit", onSubmit);
      dismissBtn?.addEventListener("click", onCancel);
      dialog.addEventListener("cancel", onCancel);
      dialog.showModal();
      (options.reason?.required ? reasonInput : dismissBtn)?.focus?.();
    });
  }

  async function confirmGuardedFleetMutation(state, options) {
    if (!isSnapshotFresh(state)) {
      notice(state.command, "Fleet observation snapshot is stale or disconnected (older than 30s). Wait for a live snapshot before mutating.");
      return { confirmed: false, reason: "" };
    }
    const expectedVersion = state.version;
    const choice = await openConfirmDialog(state, options);
    if (!choice?.confirmed) return { confirmed: false, reason: "" };
    if (!state.context?.isCurrent?.() || !isSnapshotFresh(state) || (expectedVersion && state.version !== expectedVersion)) {
      notice(state.command, "Fleet observation became stale or changed before confirmation completed. Nothing was sent.");
      return { confirmed: false, reason: "" };
    }
    return choice;
  }

  function renderClaims(state, claims) {
    state.listedClaims = claims;
    renderAttention(state);
    const now = Date.now();
    const open = claims.filter((claim) => stateText(claim.state) === "open" && (claim.expires_ms || 0) > now).length;
    state.metrics.claims.textContent = String(open);
    const known = new Set(claims.map((c) => c.identity));
    for (const id of [...state.selectedClaims]) if (!known.has(id)) state.selectedClaims.delete(id);
    const bulkWrap = state.claimsBulk;
    const renderBulk = () => {
      if (!bulkWrap) return;
      const countSelected = state.selectedClaims.size;
      if (!countSelected) { bulkWrap.replaceChildren(); bulkWrap.hidden = true; return; }
      bulkWrap.hidden = false;
      const labelNode = element("span", `${countSelected} claim${countSelected === 1 ? "" : "s"} selected`, "small");
      const bulkRenew = element("button", "Renew selected (+30m)", "quiet");
      bulkRenew.type = "button";
      bulkRenew.addEventListener("click", async () => {
        const selected = (state.listedClaims || []).filter((c) => state.selectedClaims.has(c.identity));
        if (!selected.length) return;
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Renew ${selected.length} selected claim${selected.length === 1 ? "" : "s"}?`,
          verb: `Renew ${selected.length}`,
          dismiss: "Keep current leases",
          danger: false,
          warning: "Renews each selected claim sequentially using its recorded fence.",
          facts: [["Count", String(selected.length)], ["Leases", selected.map((c) => c.identity.slice(0, 10)).join(", ")]],
          cliCommand: selected.map((c) => `kiln-fleet claims renew ${c.identity} --fence ${c.fence}`).join(" && "),
          opener: bulkRenew,
        });
        if (!choice.confirmed) return;
        for (const claim of selected) {
          await state.context.api(`/api/fleet/claims/${claim.identity}/renew`, { method: "POST", body: { fence: claim.fence } });
        }
        state.selectedClaims.clear();
        restart(state);
      });
      const bulkRelease = element("button", "Release selected", "danger");
      bulkRelease.type = "button";
      bulkRelease.addEventListener("click", async () => {
        const selected = (state.listedClaims || []).filter((c) => state.selectedClaims.has(c.identity));
        if (!selected.length) return;
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Release ${selected.length} selected claim${selected.length === 1 ? "" : "s"}?`,
          verb: `Release ${selected.length}`,
          dismiss: "Keep claims",
          danger: true,
          warning: "Releases all selected coordination claims at their exact fences. Requires an explicit reason.",
          facts: [["Count", String(selected.length)], ["Claims", selected.map((c) => c.identity.slice(0, 10)).join(", ")]],
          reason: { label: "Bulk release reason", placeholder: "Why these claims are being released", required: true, maxLength: REASON_MAX },
          cliCommand: selected.map((c) => `kiln-fleet claims release ${c.identity} --fence ${c.fence}`).join(" && "),
          opener: bulkRelease,
        });
        if (!choice.confirmed) return;
        const bounded = text(choice.reason).trim().slice(0, REASON_MAX);
        if (!bounded) return;
        for (const claim of selected) {
          await state.context.api(`/api/fleet/claims/${claim.identity}/release`, { method: "POST", body: { fence: claim.fence, note: bounded } });
        }
        state.selectedClaims.clear();
        restart(state);
      });
      const clearSel = element("button", "Clear", "quiet");
      clearSel.type = "button";
      clearSel.addEventListener("click", () => { state.selectedClaims.clear(); renderClaims(state, state.listedClaims || []); });
      bulkWrap.replaceChildren(labelNode, bulkRenew, bulkRelease, clearSel);
    };
    const rows = claims.map((claim) => {
      const check = document.createElement("input");
      check.type = "checkbox";
      check.checked = state.selectedClaims.has(claim.identity);
      check.setAttribute("aria-label", `Select claim ${claim.identity.slice(0, 12)}`);
      check.addEventListener("change", () => {
        if (check.checked) state.selectedClaims.add(claim.identity);
        else state.selectedClaims.delete(claim.identity);
        renderBulk();
      });
      const actions = element("div", undefined, "fleet-actions");
      actions.id = `fleet-claim-${claim.identity}`;
      if (typeof state.context.openInspector === "function") {
        const inspect = element("button", "Inspect", "quiet");
        inspect.type = "button";
        inspect.addEventListener("click", () => state.context.openInspector("claim", claim.identity, { opener: inspect }));
        actions.append(inspect);
      }
      const renew = element("button", "Renew", "quiet");
      renew.type = "button";
      renew.addEventListener("click", async () => {
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Renew claim ${claim.identity.slice(0, 12)}?`,
          verb: "Renew claim",
          dismiss: "Keep current lease",
          danger: false,
          warning: "Renews this coordination claim lease for the exact fence shown. Requires a fresh snapshot (within 30s).",
          facts: [
            ["Claim", claim.identity],
            ["Scope", scopeText(claim.scope)],
            ["Holder", holderText(claim.holder)],
            ["Fence", String(claim.fence)],
          ],
          reason: { label: "Renewal note (optional)", placeholder: "Optional context for this renewal", required: false, maxLength: REASON_MAX },
          cliCommand: `kiln-fleet claims renew ${claim.identity} --fence ${claim.fence}`,
          opener: renew,
        });
        if (!choice.confirmed) return;
        void mutate(state, renew, `/api/fleet/claims/${claim.identity}/renew`, { fence: claim.fence });
      });
      const release = element("button", "Release", "quiet");
      release.type = "button";
      release.addEventListener("click", async () => {
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Release claim ${claim.identity.slice(0, 12)}?`,
          verb: "Release claim",
          dismiss: "Keep claim",
          danger: true,
          warning: "Releases this coordination claim for the exact fence shown. Requires a fresh snapshot (within 30s) and explicit reason.",
          facts: [
            ["Claim", claim.identity],
            ["Scope", scopeText(claim.scope)],
            ["Holder", holderText(claim.holder)],
            ["Fence", String(claim.fence)],
          ],
          reason: { label: "Release reason", placeholder: "Why this claim is being released", required: true, maxLength: REASON_MAX },
          cliCommand: `kiln-fleet claims release ${claim.identity} --fence ${claim.fence} --note "${text(scopeText(claim.scope)).replace(/"/g, "'")}"`,
          opener: release,
        });
        if (!choice.confirmed) return;
        const bounded = text(choice.reason).trim().slice(0, REASON_MAX);
        if (!bounded) { notice(state.command, "A release reason is required."); return; }
        void mutate(state, release, `/api/fleet/claims/${claim.identity}/release`, { fence: claim.fence, note: bounded });
      });
      actions.append(renew, release);
      return [check, identityCell(claim.identity), scopeText(claim.scope), holderText(claim.holder), text(claim.purpose), when(claim.expires_ms), actions];
    });
    renderBulk();
    state.claims.replaceChildren(table(["", "ID", "Scope", "Holder", "Purpose", "Expires", "Actions"], rows, "No live claims", "The coordinator reports no live claims."));
  }

  function renderWork(state, work) {
    state.metrics.ready.textContent = String(work.ready_unclaimed);
    const ready = work.ranked.map((item) => [
      identityCell(item.identity), text(item.repository), text(item.title),
      item.interactive ? element("span", "Interactive", "state-badge fleet-tag") : "Not interactive",
    ]);
    state.ready.replaceChildren(table(["ID", "Repository", "Title", "Mode"], ready, "No ranked work", "The coordinator reports no ranked work."));
    const blocked = work.blocked.map((item) => [
      identityCell(item.identity), text(item.repository), text(item.title),
      Array.isArray(item.reasons) ? item.reasons.map(blockReason).join(" · ") : "Not recorded",
    ]);
    state.blocked.replaceChildren(table(["ID", "Repository", "Title", "Reasons"], blocked, "Nothing blocked", "The coordinator reports no blocked work."));
  }

  // r[impl kiln.fleet.console_drilldown]
  function renderAttempts(state, attempts) {
    state.metrics.attempts.textContent = String(attempts.length);
    state.listedAttempts = attempts;
    if (!state.runtimeStarted) loadRuntime(state);
    renderCairnWorkers(state);
    const focus = state.facets.map((facet) => renderFacet(state, facet, attempts)).find(Boolean);
    applyAttemptFilters(state);
    focus?.focus();
  }

  function renderAttemptRows(state, attempts, filtering) {
    const known = new Set(attempts.map((a) => a.attempt));
    for (const id of [...state.selectedAttempts]) if (!known.has(id)) state.selectedAttempts.delete(id);
    const bulkWrap = state.attemptsBulk;
    const renderBulk = () => {
      if (!bulkWrap) return;
      const countSelected = state.selectedAttempts.size;
      if (!countSelected) { bulkWrap.replaceChildren(); bulkWrap.hidden = true; return; }
      bulkWrap.hidden = false;
      const labelNode = element("span", `${countSelected} attempt${countSelected === 1 ? "" : "s"} selected`, "small");
      const bulkCancel = element("button", "Cancel selected", "danger");
      bulkCancel.type = "button";
      bulkCancel.addEventListener("click", async () => {
        const selected = (state.listedAttempts || []).filter((a) => state.selectedAttempts.has(a.attempt));
        if (!selected.length) return;
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Cancel ${selected.length} selected attempt${selected.length === 1 ? "" : "s"}?`,
          verb: `Cancel ${selected.length}`,
          dismiss: "Keep attempts",
          danger: true,
          warning: "Cancels all selected attempts sequentially at the fleet coordinator. Requires an explicit reason.",
          facts: [["Count", String(selected.length)], ["Attempts", selected.map((a) => a.attempt.slice(0, 10)).join(", ")]],
          reason: { label: "Bulk cancel reason", placeholder: "Why these attempts are being cancelled", required: true, maxLength: REASON_MAX },
          cliCommand: selected.map((a) => `kiln-fleet attempts cancel ${a.attempt}`).join(" && "),
          opener: bulkCancel,
        });
        if (!choice.confirmed) return;
        const bounded = text(choice.reason).trim().slice(0, REASON_MAX);
        if (!bounded) return;
        for (const attempt of selected) {
          await state.context.api(`/api/fleet/attempts/${attempt.attempt}/cancel`, { method: "POST", body: { reason: bounded } });
        }
        state.selectedAttempts.clear();
        restart(state);
      });
      const clearSel = element("button", "Clear", "quiet");
      clearSel.type = "button";
      clearSel.addEventListener("click", () => { state.selectedAttempts.clear(); renderAttemptRows(state, attempts, filtering); });
      bulkWrap.replaceChildren(labelNode, bulkCancel, clearSel);
    };
    const rows = attempts.map((attempt) => {
      const check = document.createElement("input");
      check.type = "checkbox";
      check.checked = state.selectedAttempts.has(attempt.attempt);
      check.setAttribute("aria-label", `Select attempt ${attempt.attempt.slice(0, 12)}`);
      check.addEventListener("change", () => {
        if (check.checked) state.selectedAttempts.add(attempt.attempt);
        else state.selectedAttempts.delete(attempt.attempt);
        renderBulk();
      });
      const session = record(attempt.session) ? attempt.session : {};
      const actions = element("div", undefined, "fleet-actions");
      if (typeof state.context.openInspector === "function") {
        const inspect = element("button", "Inspect", "quiet");
        inspect.type = "button";
        inspect.addEventListener("click", () => state.context.openInspector("attempt", attempt.attempt, { opener: inspect }));
        actions.append(inspect);
      }
      const cancel = element("button", "Cancel", "quiet");
      cancel.type = "button";
      cancel.addEventListener("click", async () => {
        const choice = await confirmGuardedFleetMutation(state, {
          title: `Cancel attempt ${attempt.attempt.slice(0, 12)}?`,
          verb: "Cancel attempt",
          dismiss: "Keep attempt",
          danger: true,
          warning: "Cancels this attempt at the fleet coordinator. Requires a fresh snapshot (within 30s) and explicit reason.",
          facts: [
            ["Attempt", attempt.attempt],
            ["Work", text(attempt.title) || text(attempt.work) || "Not recorded"],
            ["State", label(attempt.state)],
            ["Session", `${text(session.machine) || "Machine not recorded"} ${text(session.agent_name) || ""}`.trim()],
          ],
          reason: { label: "Cancel reason", placeholder: "Why this attempt is being cancelled", required: true, maxLength: REASON_MAX },
          cliCommand: `kiln-fleet attempts cancel ${attempt.attempt} --reason "Cancelled via console"`,
          opener: cancel,
        });
        if (!choice.confirmed) return;
        const bounded = text(choice.reason).trim().slice(0, REASON_MAX);
        if (!bounded) { notice(state.command, "A cancel reason is required."); return; }
        void mutate(state, cancel, `/api/fleet/attempts/${attempt.attempt}/cancel`, { reason: bounded });
      });
      actions.append(cancel);
      return [
        check,
        identityCell(attempt.attempt),
        `${label(attempt.state)}\nPhase: ${label(attempt.phase)}`,
        `${text(attempt.repository) || "Repository not recorded"}\n${text(attempt.title) || "Title not recorded"}`,
        `${text(session.machine) || "Machine not recorded"} ${text(session.agent_name) || ""}\n${label(session.state)}`,
        when(attempt.lease_expires_ms),
        actions,
      ];
    });
    renderBulk();
    state.attempts.replaceChildren(table(["", "ID", "State", "Work", "Session", "Lease", "Actions"], rows,
      filtering ? "No matching attempts" : "No attempts",
      filtering ? "Clear the filters to see all listed attempts." : "The coordinator reports no attempts."));
  }

  function renderDecisions(state, decisions) {
    const open = decisions.filter((decision) => decision.answer == null && decision.closed_ms == null).length;
    state.metrics.decisions.textContent = String(open);
    const previousInputs = state.answerInputs;
    state.answerInputs = new Map();
    const rows = decisions.map((decision) => {
      let action;
      if (decision.answer != null || decision.closed_ms != null) {
        action = element("span", decision.answer != null ? "Answered" : "Closed without answer", "state-badge");
      } else if (state.context.canOperate?.() === false) {
        action = element("span", "Operator access required", "small");
      } else {
        const form = element("form", undefined, "fleet-answer");
        const input = element("input");
        Object.assign(input, { type: "text", maxLength: ANSWER_MAX, placeholder: "Answer", required: true, autocomplete: "off" });
        input.setAttribute("aria-label", "Decision answer");
        input.value = previousInputs.get(decision.identity)?.value ?? "";
        state.answerInputs.set(decision.identity, input);
        const button = element("button", "Answer");
        button.type = "submit";
        form.append(input, button);
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          const answer = input.value.trim().slice(0, ANSWER_MAX);
          if (!answer) return;
          void mutate(state, button, `/api/fleet/decisions/${decision.identity}/answer`, { text: answer }, form);
        });
        action = form;
      }
      return [identityCell(decision.identity), text(decision.question), when(decision.raised_ms), action];
    });
    state.decisions.replaceChildren(table(["ID", "Question", "Raised", "Action"], rows, "No decisions", "The coordinator reports no decisions."));
  }

  function formatMention(mention) {
    if (!record(mention)) return "";
    if (mention.kind === "all") return "@all";
    if (mention.kind === "role") return `@role:${text(mention.target)}`;
    if (mention.kind === "member") return `@${text(mention.target)}`;
    return "";
  }

  function renderSwarmGraph(container, graph) {
    const wrap = element("div", undefined, "fleet-swarm-graph");
    if (!record(graph) || !Array.isArray(graph.edges) || !graph.edges.length) {
      wrap.append(empty("No mention edges", "No directed mentions recorded at or before this replay cursor."));
      container.replaceChildren(wrap);
      return;
    }
    const rows = graph.edges.map((edge) => [
      text(edge.from),
      text(edge.to),
      String(edge.weight ?? 0),
      `#${edge.last_seq ?? 0}`,
      Array.isArray(edge.kinds) ? edge.kinds.join(", ") : "",
    ]);
    wrap.append(table(["From", "To", "Weight", "Last Seq", "Kinds"], rows, "No mention edges", "No directed mentions recorded."));
    container.replaceChildren(wrap);
  }

  function renderSwarmDetail(state, run, graph) {
    const detail = element("div", undefined, "fleet-swarm-detail");
    const head = element("div", undefined, "fleet-section-head");
    const title = element("h3", `${text(run?.config?.title) || "Swarm run"} · ${stateText(run?.phase)}`);
    head.append(title);
    const runId = text(run?.identity);
    if (IDENTITY.test(runId)) {
      const link = element("a", "Open standalone read-only view");
      link.href = `/fleet/swarms/${runId}`;
      link.className = "small";
      head.append(link);
    }

    const members = record(run.members) ? Object.values(run.members) : [];
    const rosterRows = members.map((member) => [
      text(member.name),
      text(member.role),
      stateText(member.state),
      `${text(member.host)} · ${text(member.harness)} · ${text(member.model)}`,
      member.scope ? scopeText(member.scope) : "read-only",
      member.claim_fence != null ? String(member.claim_fence) : "–",
      String(member.budget_draw ?? 0),
      member.last_post_seq != null ? `#${member.last_post_seq}` : "–",
    ]);

    const headSeq = Number(graph?.head_seq ?? run?.board?.posts?.length ?? 0);
    const cutoff = Number(graph?.up_to_seq ?? headSeq);
    const replay = element("div", undefined, "fleet-swarm-replay");
    const sliderLabel = element("label", `Replay mention graph up to seq ${cutoff} / ${headSeq}`);
    const slider = element("input");
    Object.assign(slider, { type: "range", min: "0", max: String(headSeq), value: String(cutoff) });
    const graphBox = element("div");
    renderSwarmGraph(graphBox, graph);
    const boardBox = element("div");
    const renderBoardAt = (seqLimit) => {
      const posts = Array.isArray(run?.board?.posts) ? run.board.posts.filter((p) => Number(p.seq) <= seqLimit) : [];
      const postRows = posts.map((post) => [
        `#${post.seq}`,
        text(post.sender),
        text(post.sender_role),
        text(post.kind),
        Array.isArray(post.mentions) && post.mentions.length ? post.mentions.map(formatMention).join(", ") : "–",
        text(post.body),
      ]);
      boardBox.replaceChildren(table(["Seq", "Sender", "Role", "Kind", "Mentions", "Body"], postRows, "No board posts", "No posts at or before this cursor."));
    };
    renderBoardAt(cutoff);
    slider.addEventListener("change", async () => {
      const targetSeq = Number(slider.value);
      sliderLabel.textContent = `Replay mention graph up to seq ${targetSeq} / ${headSeq}`;
      renderBoardAt(targetSeq);
      try {
        const res = await state.context.api(`/api/fleet/swarms/${run.identity}/graph?up_to_seq=${targetSeq}`, { quiet: true });
        if (record(res) && record(res.graph)) renderSwarmGraph(graphBox, res.graph);
      } catch (_) {}
    });
    replay.append(sliderLabel, slider);

    detail.append(
      head,
      element("h4", "Roster"),
      table(["Member", "Role", "State", "Host · Harness · Model", "Scope", "Fence", "Budget", "Last Post"], rosterRows, "No members", "No members spawned in this swarm."),
      element("h4", "Replayable Mention Graph"),
      replay,
      graphBox,
      element("h4", "Board Feed"),
      boardBox,
    );
    state.swarmDetail.replaceChildren(detail);
  }

  async function inspectSwarm(state, id) {
    if (!IDENTITY.test(id)) return;
    state.selectedSwarm = id;
    try {
      const [runRes, graphRes] = await Promise.all([
        state.context.api(`/api/fleet/swarms/${id}`, { quiet: true }),
        state.context.api(`/api/fleet/swarms/${id}/graph`, { quiet: true }),
      ]);
      if (record(runRes) && record(runRes.swarm)) {
        renderSwarmDetail(state, runRes.swarm, record(graphRes) ? graphRes.graph : null);
      }
    } catch (_) {}
  }

  async function refreshSwarms(state) {
    try {
      const payload = await state.context.api("/api/fleet/swarms", { quiet: true });
      const swarms = record(payload) && Array.isArray(payload.swarms) ? payload.swarms : [];
      const rows = swarms.map((run) => {
        const id = text(run.identity);
        const actions = element("div", undefined, "fleet-actions");
        const inspect = element("button", "Inspect", "quiet");
        inspect.type = "button";
        inspect.addEventListener("click", () => { void inspectSwarm(state, id); });
        actions.append(inspect);
        if (IDENTITY.test(id)) {
          const link = element("a", "HTML", "small");
          link.href = `/fleet/swarms/${id}`;
          actions.append(link);
        }
        const members = record(run.members) ? Object.keys(run.members).length : 0;
        const headSeq = Array.isArray(run?.board?.posts) && run.board.posts.length ? run.board.posts[run.board.posts.length - 1].seq : 0;
        return [
          identityCell(id),
          text(run?.config?.title),
          stateText(run.phase),
          String(members),
          `#${headSeq}`,
          `${run?.budget?.drawn_units ?? 0} / ${run?.budget?.ceiling_units ?? 0}`,
          actions,
        ];
      });
      state.swarms.replaceChildren(table(["ID", "Title", "Phase", "Members", "Board", "Budget", "View"], rows, "No swarm runs", "The coordinator reports no swarm runs."));
      if (state.selectedSwarm && swarms.some((r) => text(r.identity) === state.selectedSwarm)) {
        void inspectSwarm(state, state.selectedSwarm);
      }
    } catch (_) {}
  }

  // r[impl kiln.sdlc.workload_placement]
  function movePanel(state) {
    const panel = element("details", undefined, "panel");
    const status = element("p", "Open to inspect eligible reservations.", "small");
    status.setAttribute("role", "status");
    const content = element("div");
    panel.append(element("summary", "Move queued workload"), status, content);
    const base = "/api/fleet/placement/move";
    const current = () => active === state && state.context.isCurrent() && state.context.canOperate?.() === true;
    const button = (label) => { const b = element("button", label); b.type = "button"; return b; };
    const option = (select, value, label = value) => { const o = element("option", label); o.value = value; select.append(o); };
    const label = (name, control) => { const l = element("label", name); l.append(control); return l; };
    const eligibleMoves = () => (state.listedAttempts || []).filter((a) => a.queued_move_eligible === true && /^dispatch-[0-9a-f]{64}$/.test(a.reservation) && IDENTITY.test(a.attempt) && count(a.fence) > 0);
    const reportingHosts = () => (state.listedMachines || []).filter((m) => m.placement_configured === true && m.state === "reporting");
    panel.addEventListener("toggle", async () => {
      if (!panel.open || !current()) return;
      const result = element("div");
      content.replaceChildren(result);
      const visible = () => current() && panel.open && result.isConnected;
      try {
        const capabilities = await state.context.api("/api/fleet/placement/capabilities", { quiet: true });
        if (!visible()) return;
        status.textContent = capabilities.active_move_reason || "Running workloads cannot be moved.";
        if (!capabilities.move) { content.prepend(element("p", capabilities.move_reason || "Queued moves are unavailable.")); return; }
        const attempts = eligibleMoves();
        if (!attempts.length) { content.prepend(element("p", "No movable reservation in the current Fleet snapshot. Reopen after Fleet refreshes.")); return; }
        const attempt = element("select");
        const host = element("select");
        option(host, "", "Choose a destination");
        attempts.forEach((a) => option(attempt, a.attempt, `${a.title || "Attempt"} · ${a.attempt.slice(0, 12)}`));
        reportingHosts().forEach((m) => option(host, m.machine));
        const review = button("Review queued move");
        const allowReview = () => { review.disabled = false; };
        attempt.addEventListener("change", allowReview);
        host.addEventListener("change", allowReview);
        content.prepend(label("Queued attempt", attempt), label("New destination", host), review);
        review.addEventListener("click", async () => {
          const item = eligibleMoves().find((a) => a.attempt === attempt.value);
          const hostAllowed = reportingHosts().some((m) => m.machine === host.value);
          if (!visible() || review.disabled || !host.value) return;
          if (!item || !hostAllowed) {
            result.replaceChildren(element("p", "The Fleet snapshot changed. Reopen to review against the current reservation."));
            return;
          }
          const request = { attempt: item.attempt, fence: item.fence, reservation: item.reservation, destination: host.value };
          review.disabled = attempt.disabled = host.disabled = true;
          let submitted;
          const show = ({ placement }, id) => {
            if (placement?.plan?.identity !== id || placement.plan.request?.attempt !== item.attempt) throw new Error("Move receipt did not match the exact plan.");
            result.replaceChildren(element("p", `Move ${id}: ${placement.state} · target ${placement.plan.request.destination}${placement.session ? ` · session ${placement.session}` : ""}`));
          };
          try {
            const { plan } = await state.context.api(`${base}/plan`, { method: "POST", body: request, quiet: true });
            if (!visible()) return;
            if (!IDENTITY.test(plan?.identity) || !plan.source || !Object.keys(request).every((key) => plan.request?.[key] === request[key])) throw new Error("Move preview did not match the selected reservation.");
            const approval = await openConfirmDialog(state, { title: "Rebind queued reservation?", verb: "Move reservation", opener: review,
              facts: [["Attempt", request.attempt], ["Fence", String(request.fence)], ["Reservation", request.reservation], ["From", plan.source], ["To", request.destination]],
              warning: "This changes a queued reservation only. It does not move a running agent or confirm a launch." });
            if (!approval.confirmed || !visible()) return;
            submitted = plan.identity;
            const reply = await state.context.api(base, { method: "POST", body: { plan: submitted, ...request }, quiet: true });
            if (visible()) show(reply, submitted);
          } catch (error) {
            if (visible()) result.replaceChildren(element("p", `${submitted ? "Move outcome not confirmed" : "No move submitted"}: ${error?.message || "request failed"}.`));
          } finally {
            if (visible() && submitted) {
              const read = button("Read move outcome");
              read.addEventListener("click", async () => {
                read.disabled = true;
                try { const reply = await state.context.api(`${base}/${submitted}`, { quiet: true }); if (visible()) show(reply, submitted); }
                catch (error) { if (visible()) result.replaceChildren(element("p", `Move outcome unavailable: ${error?.message || "read failed"}.`)); }
                finally { read.disabled = false; if (visible()) result.append(read); }
              });
              result.append(read);
            }
            review.disabled = Boolean(submitted);
            attempt.disabled = host.disabled = false;
          }
        });
      } catch (error) {
        if (visible()) status.textContent = `Move capabilities unavailable: ${error?.message || "read failed"}.`;
      }
    });
    return panel;
  }

  function placementPanel(state) {
    const section = element("section", undefined, "panel");
    const open = element("button", "Place independent work units", "quiet");
    open.type = "button";
    const content = element("div");
    section.append(open, content);
    const current = () => active === state && state.context.isCurrent() && state.context.canOperate?.() === true;
    const base = "/api/fleet/placement";
    open.addEventListener("click", async () => {
      if (!current()) return;
      open.disabled = true;
      const status = element("p", "Checking placement capabilities…", "small");
      status.setAttribute("role", "status");
      content.replaceChildren(status);
      try {
        const capabilities = await state.context.api(`${base}/capabilities`, { quiet: true });
        if (!current()) return;
        status.textContent = capabilities.split ? `Select independently claimed, unstarted attempts. ${capabilities.move_reason || "Existing execution cannot be moved by this provider."}` : "This coordinator has no qualified split or move provider.";
        if (!capabilities.split) return;
        const form = element("form");
        const fields = element("fieldset");
        fields.append(element("legend", "Attempts and destination machines"));
        const eligibleChoices = () => (state.listedAttempts || []).filter((a) => a.state === "claimed" && !a.session && IDENTITY.test(a.attempt) && count(a.fence) > 0);
        const reportingHosts = () => (state.listedMachines || []).filter((m) => m.placement_configured === true && m.state === "reporting");
        const choices = eligibleChoices();
        const hosts = reportingHosts();
        const rows = [];
        const add = element("button", "Add another unit", "quiet");
        add.type = "button";
        const addRow = () => {
          const attempt = element("select");
          const destination = element("select");
          for (const select of [attempt, destination]) {
            select.required = true;
            const option = element("option", "Select…");
            option.value = "";
            select.append(option);
          }
          for (const a of choices) {
            const option = element("option", `${a.title || "Attempt"} · ${a.attempt.slice(0, 12)}`);
            option.value = a.attempt;
            attempt.append(option);
          }
          for (const h of hosts) {
            const option = element("option", h.machine);
            option.value = h.machine;
            destination.append(option);
          }
          const row = element("div");
          for (const [name, input] of [["Attempt", attempt], ["Destination", destination]]) {
            const label = element("label", name);
            label.append(input);
            row.append(label);
          }
          if (rows.length >= 2) {
            const remove = element("button", "Remove unit", "quiet");
            remove.type = "button";
            remove.addEventListener("click", () => { rows.splice(rows.findIndex((x) => x.attempt === attempt), 1); row.remove(); add.disabled = false; });
            row.append(remove);
          }
          fields.append(row);
          rows.push({ attempt, destination });
          add.disabled = rows.length >= 16;
        };
        add.addEventListener("click", addRow);
        addRow(); addRow();
        const preview = element("button", "Review split", "primary");
        preview.type = "submit";
        const result = element("div");
        const show = (placement) => {
          const list = element("ul");
          for (const unit of placement.units || []) {
            const destination = placement.plan.units.find((planned) => planned.attempt === unit.attempt)?.destination;
            list.append(element("li", `${unit.attempt}: ${String(unit.state).replaceAll("_", " ")}${destination ? ` · target ${destination}` : ""}${unit.session ? ` · session ${unit.session}` : ""}`));
          }
          result.replaceChildren(element("p", `Placement ${placement.plan.identity}`, "small"), list);
        };
        const reconcile = (plan) => {
          const refresh = element("button", "Read placement outcome", "quiet");
          refresh.type = "button";
          refresh.addEventListener("click", async () => {
            if (!current()) return;
            refresh.disabled = true;
            try {
              const reply = await state.context.api(`${base}/split/${plan}`, { quiet: true });
              if (current()) { show(reply.placement); status.textContent = "Read the current Fleet placement outcome."; }
            } catch (error) {
              if (current()) status.textContent = `Placement outcome unavailable: ${error?.message || "read failed"}.`;
            } finally { refresh.disabled = false; if (current()) result.append(refresh); }
          });
          result.append(refresh);
        };
        form.append(fields, add, preview);
        content.append(form, result);
        form.addEventListener("submit", async (event) => {
          event.preventDefault();
          if (!current() || preview.disabled) return;
          const currentChoices = eligibleChoices();
          const currentHosts = reportingHosts();
          const units = rows.map(({ attempt, destination }) => ({ attempt: attempt.value, fence: currentChoices.find((a) => a.attempt === attempt.value)?.fence, destination: destination.value }));
          if (units.some((u) => count(u.fence) === null || !currentHosts.some((m) => m.machine === u.destination))) {
            status.textContent = "The Fleet snapshot changed. Reopen to place against the current claims and hosts.";
            return;
          }
          if (new Set(units.map((u) => u.attempt)).size !== units.length || new Set(units.map((u) => u.destination)).size < 2) {
            status.textContent = "Select distinct attempts and at least two distinct destination machines.";
            return;
          }
          preview.disabled = fields.disabled = add.disabled = true;
          let submitted;
          try {
            const { plan } = await state.context.api(`${base}/split/plan`, { method: "POST", body: { units }, quiet: true });
            if (!current()) return;
            if (!IDENTITY.test(plan?.identity)) throw new Error("Fleet returned no exact plan identity.");
            if (!Array.isArray(plan.units) || plan.units.length !== units.length || !units.every((u) => plan.units.some((p) => p.attempt === u.attempt && p.fence === u.fence && p.destination === u.destination && Array.isArray(p.scopes) && Array.isArray(p.dependencies)))) throw new Error("Fleet preview does not match the requested assignments.");
            const choice = await openConfirmDialog(state, {
              title: "Reserve the split placements?", verb: "Reserve placements", opener: preview,
              warning: "This reserves exact hosts for already-admitted attempts. Reservations do not confirm sessions have started.",
              facts: [["Plan", plan.identity], ...plan.units.flatMap((u) => [
                [u.attempt, `${u.destination} · fence ${u.fence}`],
                ["Scopes", u.scopes.map(scopeText).join(", ")],
                ["Prerequisites", u.dependencies.join(", ") || "None"],
              ])],
            });
            if (!choice.confirmed || !current()) return;
            submitted = plan.identity;
            const reply = await state.context.api(`${base}/split`, { method: "POST", body: { plan: submitted, units }, quiet: true });
            if (current()) { show(reply.placement); status.textContent = "Placement reserved. Wait for Fleet to observe each session."; }
          } catch (error) {
            if (current()) status.textContent = submitted ? `Placement not confirmed: ${error?.message || "request failed"}. Read the plan outcome before retrying.` : `No split reserved: ${error?.message || "preview failed"}.`;
          } finally {
            if (current() && submitted) reconcile(submitted);
            preview.disabled = fields.disabled = false;
            add.disabled = rows.length >= 16;
          }
        });
      } catch (error) {
        if (current()) status.textContent = `Placement capabilities unavailable: ${error?.message || "read failed"}.`;
      } finally { open.disabled = false; }
    });
    return section;
  }

  function renderAcquire(state) {
    const form = element("form", undefined, "fleet-acquire");
    const scope = element("input");
    Object.assign(scope, { type: "text", maxLength: QUERY_MAX, placeholder: "owner/repo path", required: true, autocomplete: "off" });
    const purpose = element("input");
    Object.assign(purpose, { type: "text", maxLength: PURPOSE_MAX, placeholder: "What are you working on?", required: true, autocomplete: "off" });
    const minutes = element("input");
    Object.assign(minutes, { type: "number", min: "1", step: "1", placeholder: "Use default" });
    const button = element("button", "Acquire claim");
    button.type = "submit";
    for (const [name, input] of [["Scope", scope], ["Purpose", purpose], ["Lease minutes", minutes]]) {
      const field = element("label", name);
      field.append(input);
      form.append(field);
    }
    form.append(button);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const scopeValue = scope.value.trim().slice(0, QUERY_MAX);
      const purposeValue = purpose.value.trim().slice(0, PURPOSE_MAX);
      if (!scopeValue || !purposeValue) return;
      const body = { scope: scopeValue, tool: TOOL, session: sessionId(), purpose: purposeValue };
      if (minutes.value !== "" && Number.isSafeInteger(Number(minutes.value)) && Number(minutes.value) > 0) body.lease_minutes = Number(minutes.value);
      void mutate(state, button, "/api/fleet/claims", body, form);
    });
    state.acquire.replaceChildren(form);
  }

  function notice(node, message = "") {
    node.textContent = message;
    node.hidden = !message;
  }

  function connection(state, kind, label) {
    if (state.connection.dataset.state === kind) return;
    state.connection.dataset.state = kind;
    state.connection.textContent = label;
    notice(state.assignmentStatus, kind === "live" ? "" : (state.listedAttempts || state.listedClaims)
      ? "Fleet assignments disconnected or reconnecting · last counts shown." : "Waiting for Fleet assignment observations.");
  }

  function live(state) {
    connection(state, "live", "Live");
    if (!state.transport.hidden) notice(state.transport);
    state.failures = 0;
  }

  async function mutate(state, button, path, body, form) {
    if (state.mutating) return;
    state.mutating = true;
    notice(state.command);
    button.disabled = true;
    try {
      await state.context.api(path, { method: "POST", body });
      if (form) form.reset();
      restart(state);
    } catch (error) {
      if (error?.name !== "AbortError") notice(state.command, error?.message || "The fleet coordinator rejected the command.");
    } finally {
      state.mutating = false;
      button.disabled = false;
    }
  }

  function eventFrames(source) {
    const frames = [];
    for (const block of source.replace(/\r\n/g, "\n").split("\n\n")) {
      if (!block.trim()) continue;
      let event = "message";
      let comment = "";
      const data = [];
      for (const line of block.split("\n")) {
        if (line.startsWith(":")) comment = line.slice(1).trim();
        else if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
      }
      if (data.length) frames.push({ event, data: data.join("\n") });
      else if (comment === "heartbeat" || comment === "unavailable") frames.push({ event: comment, data: "" });
    }
    return frames;
  }

  // r[impl kiln.fleet.console_snapshot]
  function applySnapshot(state, snapshot) {
    if (!record(snapshot) || typeof snapshot.version !== "string" || !VERSION.test(snapshot.version)) throw new Error("Fleet event response is invalid.");
    validateHealth(snapshot.health);
    validateClaims(snapshot.claims);
    validateWork(snapshot.work);
    validateAttempts(snapshot.attempts);
    validateDecisions(snapshot.decisions);
    const parts = [snapshot.claims, snapshot.work, snapshot.attempts, snapshot.decisions];
    const render = [renderClaims, renderWork, renderAttempts, renderDecisions];
    const values = [snapshot.claims.claims, snapshot.work, snapshot.attempts.attempts, snapshot.decisions.decisions];
    let sourcesChanged = false;
    parts.forEach((part, index) => {
      const signature = JSON.stringify(part);
      if (signature === state.signatures[index]) return;
      if (index === 1 || index === 2) sourcesChanged = true;
      render[index](state, values[index]);
      state.signatures[index] = signature;
    });
    void refreshSwarms(state);
    if (sourcesChanged) renderWorkSources(state, snapshot.work, snapshot.attempts);
    renderMachines(state);
    state.machine.textContent = `${snapshot.health.machine} · fleet ${snapshot.health.version}`;
    state.observedAt = Date.now();
    const received = new Date(state.observedAt);
    state.updated.dateTime = received.toISOString();
    state.updated.textContent = `Last snapshot · ${received.toLocaleString()}`;
  }

  function applyFrame(state, frame) {
    if (frame.event === "unavailable") throw new Error("The coordinator did not answer.");
    if (frame.event === "snapshot") {
      const snapshot = JSON.parse(frame.data);
      applySnapshot(state, snapshot);
      state.version = String(snapshot.version);
      live(state);
      return true;
    }
    if (frame.event === "heartbeat") {
      if (!state.version) throw new Error("Fleet has not supplied a snapshot.");
      state.observedAt = Date.now();
      live(state);
      return true;
    }
    return false;
  }

  async function stream(state) {
    if (state.inFlight || !state.context.isCurrent()) return;
    const epoch = state.epoch;
    state.inFlight = true;
    const controller = new AbortController();
    state.controller = controller;
    try {
      const query = state.version ? `?after=${encodeURIComponent(state.version)}` : "";
      const target = `/api/fleet/events${query}`;
      let recognized = false;
      if (state.context.stream) {
        const body = await state.context.stream(target, { controller });
        const reader = body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";
        let watchdog = 0;
        const arm = () => {
          clearTimeout(watchdog);
          watchdog = setTimeout(() => controller.abort(new DOMException("Stream idle", "TimeoutError")), 30_000);
        };
        arm();
        try {
          while (true) {
            const { value, done } = await reader.read();
            if (epoch !== state.epoch || !state.context.isCurrent()) return;
            if (done) break;
            arm();
            buffer += decoder.decode(value, { stream: true });
            let split;
            while ((split = buffer.indexOf("\n\n")) >= 0) {
              const block = buffer.slice(0, split);
              buffer = buffer.slice(split + 2);
              for (const frame of eventFrames(block)) recognized = applyFrame(state, frame) || recognized;
            }
          }
        } finally {
          clearTimeout(watchdog);
          controller.abort();
          reader.releaseLock();
        }
      } else {
        const source = await state.context.api(target, { kind: "sse", quiet: true, controller });
        if (epoch !== state.epoch || !state.context.isCurrent()) return;
        for (const frame of eventFrames(source)) recognized = applyFrame(state, frame) || recognized;
      }
      if (!recognized) throw new Error("The fleet event endpoint returned no snapshot or heartbeat.");
      connection(state, "reconnecting", "Reconnecting · last snapshot shown");
      if (epoch === state.epoch && state.context.isCurrent()) {
        clearTimeout(state.timer);
        state.timer = setTimeout(() => { void stream(state); }, STREAM_RECONNECT_MS);
      }
    } catch (error) {
      if (epoch !== state.epoch || !state.context.isCurrent()) return;
      state.failures += 1;
      connection(state, state.version ? "stale" : "unavailable", state.version ? "Stale · last snapshot shown" : "Unavailable");
      notice(state.transport, error?.message || "The coordinator did not answer.");
      if (state.context.isCurrent()) {
        clearTimeout(state.timer);
        const delay = Math.min(STREAM_BACKOFF_MAX_MS, 1_000 * 2 ** (state.failures - 1));
        state.timer = setTimeout(() => { void stream(state); }, delay);
      }
    } finally {
      if (epoch === state.epoch) state.inFlight = false;
    }
  }

  function restart(state) {
    if (typeof state.context.subscribeEvents === "function") {
      for (const poll of state.polls || []) void pollProjection(state, poll);
      return;
    }
    state.epoch += 1;
    state.controller?.abort();
    state.inFlight = false;
    clearTimeout(state.timer);
    if (state.version && state.connection.dataset.state === "live") connection(state, "reconnecting", "Reconnecting · last snapshot shown");
    void stream(state);
  }

  function exactLink(state, name, patch, title) {
    const link = element("a", name, "row-link");
    link.href = `?${new URLSearchParams({ ...patch, theme: state.context.route?.theme || "system" })}`;
    link.title = title;
    link.addEventListener("click", (event) => {
      if (!state.context.navigate || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      state.context.navigate(patch);
    });
    return link;
  }

  function cairnLink(state, row) {
    const patch = { page: "cairn", cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "" };
    const name = row.projects.join(", ") || row.repository.slice(0, 12);
    const cell = element("div");
    const link = exactLink(state, row.change, patch, row.repository);
    cell.append(link, element("div", name, "small"));
    return cell;
  }

  function cairnBadge(lane, suffix = "") {
    const badge = element("span", `${lane.label}${suffix}`, "state-badge");
    badge.dataset.tone = lane.tone;
    return badge;
  }

  function cairnUnitLanes(row, cairn) {
    const counts = element("div", undefined, "fleet-cairn-lanes");
    for (const [key, value] of row.lanes) {
      if (value > 0) counts.append(cairnBadge(cairn.lane(key), ` ${value}`));
    }
    if (row.units === 0) return "No units listed";
    const known = [...row.lanes.values()];
    if (known.some((value) => value === null) || known.reduce((sum, value) => sum + value, 0) !== row.units) {
      counts.append(element("span", "Unit stages incomplete", "small"));
    }
    return counts;
  }

  // r[impl kiln.fleet.cairn_execution_context]
  function appendDispatchContext(item, assignment) {
    const model = text(assignment.requested_model);
    const services = (Array.isArray(assignment.model_services) ? assignment.model_services : [])
      .filter((ref) => text(ref?.source) && text(ref?.id)).map((ref) => `${ref.source}/${ref.id}`);
    const profile = text(assignment.dispatch_profile);
    const harness = text(assignment.harness);
    if (profile) item.append(element("div", `Dispatch profile · ${profile}`));
    if (harness) item.append(element("div", `Planned harness · ${harness}`));
    if (model) item.append(element("div", `Requested model artifact · ${model}`));
    if (services.length) item.append(element("div", `Bound model services · ${services.join(", ")}${assignment.model_services_truncated === true ? " · more not listed" : ""}`));
    if (!model && !services.length) item.append(element("div", "Model binding not recorded"));
  }

  function renderCairnWorkers(state) {
    if (!state.cairnWorkerCells) return;
    const sessions = new Map((state.listedAttempts || []).map((item) => [item.attempt, item]));
    for (const { row, cell } of state.cairnWorkerCells) {
      if (!row.activeAssignments) { cell.textContent = "Worker attribution not recorded"; continue; }
      if (!row.activeAssignments.length) {
        cell.textContent = row.assignmentsTruncated ? "Active assignments omitted from this snapshot" : "No active assignments listed";
        continue;
      }
      const list = element("ul", undefined, "fleet-cairn-workers");
      for (const assignment of row.activeAssignments) {
        const attempt = text(assignment?.attempt);
        if (!IDENTITY.test(attempt) || count(assignment.fence) === null) continue;
        const item = element("li");
        item.append(identityCell(attempt));
        const observed = sessions.get(attempt);
        const session = observed?.fence === assignment.fence ? observed.session : null;
        const worker = assignment.worker;
        if (text(worker?.source) && text(worker?.id)) {
          item.append(element("div", `Dispatch binding · ${worker.source}/${worker.id}`));
        }
        if (text(session?.machine)) {
          const sessionState = record(session.state) ? session.state.state : session.state;
          const agent = text(session.agent_name);
          item.append(element("div", `Fleet session · ${session.machine}${agent ? ` / ${agent}` : ""} · ${label(sessionState)}`));
          if (text(session.kind)) item.append(element("div", `Observed agent kind · ${session.kind}`));
          if (count(session.observed_ms) !== null) {
            const seen = element("div");
            seen.append("Session observed · ", when(session.observed_ms));
            item.append(seen);
          }
        } else {
          item.append(element("div", "Machine not observed"));
          if (!text(worker?.source) || !text(worker?.id)) item.append(element("div", "Worker not recorded"));
        }
        appendDispatchContext(item, assignment);
        item.append(element("div", "Swarm / subagents not observed"));
        list.append(item);
      }
      if (row.assignmentsTruncated) list.append(element("li", "Additional assignments not listed"));
      cell.replaceChildren(list);
    }
  }

  // r[impl kiln.fleet.cairn_progress]
  const externalRef = (value) => text(value?.native_id)
    ? [value.provider_namespace, value.native_kind, value.native_id].filter(text).join(" · ") : "Not recorded";

  function inspectorRun(state, run, title, view) {
    if (!text(run)) return "Not recorded";
    if (state.context.runLink) return state.context.runLink(run, title, view);
    const node = element("a", title);
    node.href = `?${new URLSearchParams({ page: "runs", run, view })}`;
    return node;
  }

  function fleetedAttempt(state, attempt) {
    const match = state.listedAttempts?.find((item) => item.attempt === attempt.identity && item.fence === attempt.fence);
    const session = match?.session;
    return text(session?.machine) ? [session.machine, session.agent_name].filter(text).join(" / ") : "Not observed";
  }

  function recoveryReadout(state, attempt) {
    const panel = element("div", undefined, "fleet-recovery");
    panel.append(element("p", "Recovery uses the original retained request. It does not submit a replacement or infer termination.", "small"));
    if (state.context.canOperate?.() === false) return panel;
    const button = element("button", "Read recovery evidence", "quiet");
    button.type = "button";
    const reconcile = element("button", "Reconcile original dispatch…", "quiet");
    reconcile.type = "button";
    reconcile.hidden = true;
    let reviewedDigest = "";
    const graph = element("div", undefined, "small");
    const target = `/api/implementation/dispatch-recovery?attempt=${attempt.identity}&fence=${attempt.fence}`;
    function applyEvidence(value) {
      if (value?.schema !== "kiln.dispatch-recovery-observation.v1" || value.attempt !== attempt.identity
        || value.fence !== attempt.fence || value.work_identity !== attempt.work_identity) throw new Error("Crossed recovery evidence refused.");
      const digest = value.request_binding?.request_digest;
      const exactDigest = IDENTITY.test(text(digest));
      reviewedDigest = value.can_recover === true && exactDigest ? digest : "";
      reconcile.hidden = !reviewedDigest;
      status.textContent = [label(value.recovery_status), exactDigest ? `Request digest ${digest}` : "Exact request not recorded",
        value.workflow ? `Provider execution ${externalRef(value.workflow)}` : "Provider execution not observed",
        reviewedDigest ? "Operator recovery available" : "Web recovery not admitted for this observation"].join(" · ");
      const plan = value.planned_graph;
      graph.replaceChildren();
      if (Array.isArray(plan?.nodes) && Array.isArray(plan?.edges)) {
        const details = element("details");
        details.append(element("summary", "Planned Lattice graph (not live state)"));
        const dependencies = new Map();
        for (const edge of plan.edges) {
          if (!text(edge?.source) || !text(edge?.target)) continue;
          const parents = dependencies.get(edge.target) || [];
          parents.push(edge.source);
          dependencies.set(edge.target, parents);
        }
        const nodes = element("ul");
        for (const node of plan.nodes) {
          if (!text(node?.id)) continue;
          const parents = dependencies.get(node.id);
          nodes.append(element("li", `${node.id} · ${label(node.kind)}${parents?.length ? ` · depends on ${parents.join(", ")}` : ""}`));
        }
        details.append(nodes);
        graph.append(details);
      } else if (value.planned_graph_status === "exceeds_response_bound") {
        graph.textContent = "Planned graph exceeds the response bound.";
      }
    }
    reconcile.addEventListener("click", async () => {
      const controller = state.inspectorController;
      if (!reviewedDigest || !controller || reconcile.disabled || !state.context.isCurrent()) return;
      const choice = await openConfirmDialog(state, {
        title: `Reconcile original dispatch for attempt ${attempt.identity.slice(0, 12)}?`,
        verb: "Reconcile original dispatch",
        dismiss: "Keep unconfirmed",
        danger: false,
        warning: "Probes/reconciles the original retained dispatch request. This does not start a replacement.",
        facts: [
          ["Attempt", attempt.identity],
          ["Fence", String(attempt.fence)],
          ["Work", attempt.work_identity],
          ["Request digest", reviewedDigest],
        ],
        opener: reconcile,
      });
      if (!choice.confirmed || state.inspectorController !== controller || !state.context.isCurrent()) return;
      const request_digest = reviewedDigest;
      reviewedDigest = "";
      reconcile.hidden = true;
      button.disabled = true;
      status.textContent = "Reconciling original dispatch…";
      const current = () => state.inspectorController === controller && state.context.isCurrent();
      try {
        const value = await state.context.api(target, { method: "POST", body: { request_digest }, quiet: true, controller });
        if (!current()) return;
        if (value?.schema !== "kiln.dispatch-recovery-result.v1" || value.attempt !== attempt.identity
          || value.fence !== attempt.fence || value.work_identity !== attempt.work_identity
          || value.outcome !== "accepted" || !text(value.workflow?.native_id)) throw new Error("Crossed or unknown recovery result refused.");
        status.textContent = `Original execution recovered · ${externalRef(value.workflow)}. No replacement was submitted.`;
      } catch (error) {
        if (current() && !controller.signal.aborted) status.textContent = `Recovery not confirmed: ${error?.message || "request did not return evidence"}. Read the exact evidence again before acting.`;
      } finally { if (current()) button.disabled = false; }
    });
    const status = element("p", undefined, "small");
    status.setAttribute("role", "status");
    button.addEventListener("click", async () => {
      const controller = state.inspectorController;
      if (!controller) return;
      const current = () => state.inspectorController === controller && state.context.isCurrent();
      button.disabled = true;
      reviewedDigest = "";
      reconcile.hidden = true;
      graph.replaceChildren();
      status.textContent = "Reading recovery evidence…";
      try {
        const value = await state.context.api(target, { quiet: true, controller });
        if (!current()) return;
        applyEvidence(value);
      } catch (error) {
        if (current() && !controller.signal.aborted) status.textContent = `Recovery evidence unavailable: ${error?.message || "read failed"}`;
      } finally {
        button.disabled = false;
      }
    });
    panel.append(button, reconcile, status, graph);
    return panel;
  }

  function nativeRuntimeReadout(state, attempt) {
    const panel = element("div");
    const button = element("button", "Inspect native worker", "quiet");
    button.type = "button";
    const result = element("div", undefined, "small");
    result.setAttribute("role", "status");
    button.addEventListener("click", async () => {
      const controller = state.inspectorController;
      if (!controller || button.disabled || !state.context.isCurrent()) return;
      const current = () => state.inspectorController === controller && state.context.isCurrent();
      const query = new URLSearchParams({ attempt: attempt.identity, fence: String(attempt.fence), work: attempt.work_identity });
      button.disabled = true;
      result.textContent = "Reading exact native runtime evidence…";
      try {
        const evidence = await state.context.api(`/api/implementation/runtime-evidence?${query}`, { quiet: true, controller });
        if (!current()) return;
        if (evidence?.schema !== "kiln.native-runtime-evidence.v1" || evidence.attempt_identity !== attempt.identity
          || evidence.work_identity !== attempt.work_identity || evidence.fence !== attempt.fence) throw new Error("Crossed native worker evidence refused.");
        const worker = evidence.worker;
        if (worker && (worker.schema !== "lattice.worker-instance-observation.v3" || worker.attempt_id !== attempt.identity
          || worker.work_id !== attempt.work_identity || worker.fence !== attempt.fence)) throw new Error("Crossed native worker observation refused.");
        const lines = worker ? [
          element("p", `Native worker · ${label(worker.lifecycle?.state)} · cancellation ${label(worker.cancellation)}`),
          element("p", `Incarnation ${text(worker.incarnation) || "not recorded"} · profile ${text(worker.profile_revision) || "not recorded"}`),
          when(worker.observed_at_epoch_ms),
        ] : [element("p", "No native worker observation was retained for this attempt.")];
        const models = evidence.model_requests;
        if (models?.state === "serving_owner") {
          const requests = element("details");
          requests.append(element("summary", "Retained model request observations"));
          const list = element("ul");
          for (const { service, expected_model, requests: entries } of models.services ?? []) {
            for (const observation of entries) {
              const row = [`${service} · ${label(observation.class)}`, observation.request_id];
              if (observation.class === "completed") row.push(`Response model matched ${expected_model}`);
              if (observation.deployment_id) row.push(`Admission deployment ${observation.deployment_id} · generation ${observation.deployment_generation ?? "unknown"}`);
              if (observation.replica) row.push(`Admission replica ${observation.replica}`);
              row.push(label(observation.evidence || "unobserved"));
              list.append(element("li", row.join(" · ")));
            }
          }
          requests.append(list);
          if (!models.services?.length) requests.append(element("p", "No serving-owner requests retained."));
          if (!models.usage_complete) requests.append(element("p", "Provider usage coverage is incomplete."));
          if (models.omitted > 0) requests.append(element("p", `${models.omitted} additional records omitted by the display bound.`));
          lines.push(requests);
        } else if (models?.state === "brokered_unattributed") {
          lines.push(element("p", `${models.dispatched_requests} broker dispatches recorded${models.count_complete ? "" : " (incomplete count)"}. No serving-owner model or replica attribution.`));
        } else if (models?.state === "no_model") {
          lines.push(element("p", "Native binding declares no model."));
        } else {
          lines.push(element("p", "No native model request evidence retained."));
        }
        const lineage = evidence.agent_lineage;
        if (lineage?.state === "recorded") {
          const receipt = lineage.receipt;
          if (!worker || receipt?.schema !== "animus.implementation-child-lineage.v1"
            || receipt.work_identity !== attempt.work_identity || receipt.attempt_identity !== attempt.identity
            || receipt.fence !== attempt.fence || receipt.request_identity !== evidence.request_identity
            || receipt.worker_incarnation !== worker.incarnation || receipt.profile_revision !== worker.profile_revision) {
            throw new Error("Crossed agent lineage refused.");
          }
          const topology = element("details");
          topology.append(element("summary", "Retained Animus child lineage"),
            element("p", `Parent ${receipt.root_id} · ${label(receipt.coverage)} coverage`));
          if (receipt.edges.length) {
            const children = element("ul");
            for (const edge of receipt.edges) children.append(element("li", `Child ${edge.child_id} · ${label(edge.state)}`));
            topology.append(children);
          } else {
            topology.append(element("p", receipt.coverage === "complete" ? "The producer recorded no children." : "No children observed in the retained partial record."));
          }
          lines.push(topology);
        } else if (lineage?.state === "unobserved") {
          lines.push(element("p", "No attempt-bound Animus child lineage retained."));
        } else {
          lines.push(element("p", "This harness does not export attempt-bound child lineage."));
        }
        lines.push(element("p", "Retained state does not prove physical host liveness."));
        result.replaceChildren(...lines);
      } catch (error) {
        if (current() && !controller.signal.aborted) result.textContent = `Native runtime evidence unavailable: ${error?.message || "read failed"}`;
      } finally { if (current()) button.disabled = false; }
    });
    panel.append(button, result);
    return panel;
  }

  // r[impl kiln.fleet.operator_plane.scenario.stuck]
  function executionArtifactsReadout(state, attempt) {
    const panel = element("div", undefined, "fleet-execution-artifacts");
    const button = element("button", "Read execution artifacts", "quiet");
    button.type = "button";
    const result = element("div", undefined, "small");
    result.setAttribute("role", "status");
    const matchesExecution = (observed) => {
      if (!attempt.workflow) return !observed;
      return record(observed)
        && observed.provider_namespace === attempt.workflow.provider_namespace
        && observed.native_kind === attempt.workflow.native_kind
        && observed.native_id === attempt.workflow.native_id;
    };
    const renderGraph = (graph) => {
      if (graph?.status === "available" && Array.isArray(graph.nodes) && Array.isArray(graph.edges)) {
        const details = element("details");
        details.append(element("summary", "Observed provider graph"));
        const dependencies = new Map();
        for (const edge of graph.edges) {
          if (!text(edge?.source) || !text(edge?.target)) continue;
          const parents = dependencies.get(edge.target) || [];
          parents.push(edge.source);
          dependencies.set(edge.target, parents);
        }
        const list = element("ul");
        for (const node of graph.nodes) {
          if (!text(node?.id)) continue;
          const parents = dependencies.get(node.id);
          const parts = [node.id, label(node.kind), label(node.state)];
          if (parents?.length) parts.push(`depends on ${parents.join(", ")}`);
          list.append(element("li", parts.join(" · ")));
        }
        if (!graph.nodes.length) details.append(element("p", "Observed provider graph has no nodes."));
        else details.append(list);
        return details;
      }
      if (graph?.status === "unsupported") {
        return element("p", `Observed provider graph unsupported (${text(graph.reason) || "route does not support graph"}).`);
      }
      return element("p", `Observed provider graph unavailable (${text(graph?.reason) || "not observed"}).`);
    };
    const renderLogs = (logs) => {
      if (logs?.status === "available" && Array.isArray(logs.entries)) {
        const details = element("details");
        const markers = [
          logs.redacted ? "Redacted" : null,
          logs.truncated ? `Truncated · ${logs.omitted_entries ?? 0} entries omitted` : null,
        ].filter(Boolean);
        details.append(element("summary", ["Observed provider logs", ...markers].join(" · ")));
        if (logs.redacted) details.append(element("p", "Provider log output includes redacted entries."));
        if (logs.truncated) details.append(element("p", `Provider logs truncated to ${logs.retained_bytes ?? 0} bytes (${logs.omitted_entries ?? 0} omitted).`));
        if (!logs.entries.length) {
          details.append(element("p", "No provider log entries retained."));
        } else {
          const list = element("ul");
          for (const entry of logs.entries) {
            const item = element("li");
            const meta = [`${text(entry?.node_id) || "unknown"} · ${label(entry?.stream)} · cursor ${count(entry?.cursor) ?? 0}`];
            if (entry?.redacted) meta.push("Redacted");
            if (entry?.truncated) meta.push("Truncated");
            item.append(element("div", meta.join(" · ")));
            item.append(element("pre", typeof entry?.text === "string" ? entry.text : ""));
            list.append(item);
          }
          details.append(list);
        }
        return details;
      }
      if (logs?.status === "unsupported") {
        return element("p", `Observed provider logs unsupported (${text(logs.reason) || "route does not support logs"}).`);
      }
      return element("p", `Observed provider logs unavailable (${text(logs?.reason) || "not observed"}).`);
    };
    button.addEventListener("click", async () => {
      const controller = state.inspectorController;
      if (!controller || button.disabled || !state.context.isCurrent()) return;
      const current = () => state.inspectorController === controller && state.context.isCurrent();
      const query = new URLSearchParams({ attempt: attempt.identity, fence: String(attempt.fence), work: attempt.work_identity });
      button.disabled = true;
      result.textContent = "Reading exact execution artifacts…";
      try {
        const value = await state.context.api(`/api/implementation/execution-artifacts?${query}`, { quiet: true, controller });
        if (!current()) return;
        if (value?.schema !== "kiln.execution-artifacts-observation.v1"
          || value.attempt !== attempt.identity || value.fence !== attempt.fence
          || value.work_identity !== attempt.work_identity
          || !matchesExecution(value.execution)) {
          throw new Error("Crossed execution artifacts refused.");
        }
        if (value.status === "unsupported") {
          result.replaceChildren(
            element("p", `Execution artifact route unsupported (${text(value.reason) || "not admitted"}).`),
            renderGraph(value.graph),
            renderLogs(value.logs),
          );
          return;
        }
        if (value.status !== "available") {
          result.replaceChildren(
            element("p", `Execution artifacts unavailable (${text(value.reason) || "not recorded"}).`),
            renderGraph(value.graph),
            renderLogs(value.logs),
          );
          return;
        }
        const summary = [
          `Execution ${externalRef(value.execution)}`,
          value.execution_state ? `State ${label(value.execution_state)}` : null,
          `Freshness ${label(value.freshness)}`,
        ].filter(Boolean).join(" · ");
        const lines = [element("p", summary)];
        if (count(value.observed_at_ms) !== null) {
          const seen = element("p");
          seen.append("Observed ", when(value.observed_at_ms));
          lines.push(seen);
        }
        lines.push(renderGraph(value.graph), renderLogs(value.logs), element("p", "Observed provider artifacts do not establish verification or completion authority."));
        result.replaceChildren(...lines);
      } catch (error) {
        if (current() && !controller.signal.aborted) result.textContent = `Execution artifacts unavailable: ${error?.message || "read failed"}`;
      } finally {
        if (current()) button.disabled = false;
      }
    });
    panel.append(button, result);
    return panel;
  }

  // r[impl kiln.fleet.operator_plane]
  function attemptInspection(state, unit, attempt) {
    const entry = element("details", undefined, "fleet-attempt-inspection");
    entry.append(element("summary", `${text(unit.native_id)} · attempt ${attempt.ordinal ?? "?"} · ${label(attempt.phase)}`));
    const verification = attempt.verification;
    const lifecycle = attempt.lifecycle;
    const facts = element("dl", undefined, "facts");
    const rows = [
      ["Attempt", identityCell(attempt.identity)], ["Fence", String(attempt.fence)],
      ["Work revision", `${label(attempt.applicability)} · ${attempt.work_revision ?? "Not recorded"}`],
      ["Work identity", identityCell(attempt.work_identity)], ["Claimed", when(attempt.claimed_at_ms)],
      ["Recorded lease deadline", when(attempt.lease_until_ms)],
      ["Fleet session", fleetedAttempt(state, attempt)],
      ["Dispatch", [label(attempt.dispatch), attempt.unknown_outcome && label(attempt.unknown_outcome)].filter(Boolean).join(" · ")],
      ["Provider execution", externalRef(attempt.workflow)],
      ["Candidate", externalRef(attempt.candidate)],
      ["Independent verification", verification ? verification.success === true ? "Passed" : verification.success === false ? "Failed" : "Unknown" : "Not recorded"],
      ["CI pipeline", inspectorRun(state, verification?.run_id, "Open exact verification run", "pipeline")],
      ["CI logs", inspectorRun(state, verification?.run_id, "Open exact verification logs", "logs")],
      ["Publication", attempt.publication ? label(attempt.publication.phase) : "Not recorded"],
      ["Cairn receipt", lifecycle?.binding === "matches_attempt" ? externalRef(lifecycle.lifecycle_receipt) : lifecycle ? "Crossed receipt (not applied)" : "Not recorded"],
    ];
    if (attempt.failure) rows.push(["Failure", [label(attempt.failure), text(attempt.cause)].filter(Boolean).join(" · ")]);
    for (const [key, value] of rows) {
      const dd = element("dd");
      dd.append(value);
      facts.append(element("dt", key), dd);
    }
    entry.append(facts);
    if (Array.isArray(verification?.checks) && verification.checks.length) {
      const checks = element("ul");
      for (const check of verification.checks) checks.append(element("li", `${text(check?.name) || "Unnamed check"} · ${label(check?.outcome)}`));
      entry.append(element("h4", "Verification checks"), checks);
    }
    entry.append(element("p", "The CI links are for independent verification. Provider graph and logs require an explicitly admitted execution artifact route.", "small"));
    if (attempt.unknown_outcome === "unreconciled" || ["reserved", "transport_started"].includes(attempt.dispatch) || attempt.workflow?.provider_namespace === "lattice") entry.append(recoveryReadout(state, attempt));
    if (state.context.canOperate?.() !== false) {
      entry.append(nativeRuntimeReadout(state, attempt));
      entry.append(executionArtifactsReadout(state, attempt));
    }
    return entry;
  }

  async function inspectChange(state, row) {
    state.inspectorController?.abort();
    const controller = new AbortController();
    state.inspectorController = controller;
    const route = { repository: row.repository, change: row.change };
    state.inspectorTitle.textContent = `Inspect ${row.change}`;
    state.inspector.hidden = false;
    state.inspectorBody.replaceChildren(element("p", "Reading exact Cairn change…", "small"));
    state.inspectorTitle.focus();
    try {
      const cairn = window.KilnCairn;
      const value = await state.context.api(cairn.detailPath(route), { quiet: true, controller });
      if (state.inspectorController !== controller || !state.context.isCurrent()) return;
      const detail = cairn.admitDetail(value, route);
      const entries = [];
      for (const unit of detail.units) {
        for (const attempt of Array.isArray(unit?.attempts) ? unit.attempts : []) {
          if (IDENTITY.test(text(attempt?.identity)) && IDENTITY.test(text(attempt?.work_identity)) && count(attempt.fence) !== null) {
            entries.push(attemptInspection(state, unit, attempt));
          }
        }
      }
      state.inspectorBody.replaceChildren(cairnLink(state, row), ...(entries.length ? entries : [empty("No attempts recorded", "No attempt was admitted for this exact change.")]));
    } catch (error) {
      if (state.inspectorController === controller && state.context.isCurrent() && !controller.signal.aborted) {
        state.inspectorBody.replaceChildren(empty("Inspector unavailable", error?.message || "The exact Cairn change could not be read."));
      }
    }
  }

  function inspectButton(state, row) {
    const button = element("button", "Inspect attempts", "quiet");
    button.type = "button";
    button.setAttribute("aria-controls", "fleet-inspector");
    button.addEventListener("click", () => { state.inspectorTrigger = button; void inspectChange(state, row); });
    return button;
  }

  function renderCairnProgress(state, index) {
    const cairn = window.KilnCairn;
    const rows = cairn.indexRows(index);
    const partial = index.truncated || index.snapshot?.campaigns_truncated || index.snapshot?.lifecycle_truncated || rows.length !== index.changes.length;
    const missing = index.snapshot?.work_recorded !== true || index.snapshot?.attempts_recorded !== true;
    const signature = JSON.stringify([index.changes, partial, missing]);
    if (signature === state.cairnSignature) return;
    const observedCount = (value) => value === null ? "Not recorded" : String(value);
    state.cairnWorkerCells = rows.map((row) => ({ row, cell: element("div") }));
    const values = state.cairnWorkerCells.map(({ row, cell }) => [
      cairnLink(state, row),
      cairnBadge(cairn.lane(row.lane)),
      cairnUnitLanes(row, cairn),
      `${observedCount(index.snapshot?.attempts_recorded === true ? row.activeAttempts : null)} active attempts · ${observedCount(row.blockers)} blockers · ${observedCount(row.openDecisions)} open decisions`,
      cell,
      inspectButton(state, row),
    ]);
    const emptyMessage = partial || missing ? "The canonical index is incomplete." : "The canonical index lists no changes.";
    state.cairnRows.replaceChildren(table(["Change", "Stage", "Unit stages", "Activity", "Workers", "Inspect"], values, "No changes listed", emptyMessage));
    renderCairnWorkers(state);
    renderAttention(state);
    state.cairnCoverage.textContent = [partial ? "Partial index." : "", missing ? "Work or attempt observations missing." : ""].filter(Boolean).join(" ");
    state.cairnCoverage.hidden = !(partial || missing);
    state.cairnSignature = signature;
  }

  function loadRuntime(state) {
    if (!window.KilnFleetRuntime || !state.listedAttempts || state.connection.dataset.state === "stale" || state.connection.dataset.state === "unavailable") return;
    state.runtimeStarted = true;
    void window.KilnFleetRuntime.load(state.runtime, state.context, state.listedAttempts);
  }

  async function pollCairn(state) {
    if (state !== active || !state.context.isCurrent() || document.visibilityState === "hidden" || state.cairnController) return;
    clearTimeout(state.cairnTimer);
    const controller = new AbortController();
    state.cairnController = controller;
    const current = () => state === active && state.context.isCurrent();
    try {
      const index = await state.context.api(CAIRN_INDEX, { quiet: true, controller });
      if (!current()) return;
      renderCairnProgress(state, index);
      renderAttention(state);
      const label = `Refreshing every ${CAIRN_REFRESH_MS / 1000}s while visible`;
      if (state.cairnStatus.textContent !== label) state.cairnStatus.textContent = label;
      const observed = new Date();
      state.cairnTime.dateTime = observed.toISOString();
      state.cairnTime.textContent = `Last read · ${observed.toLocaleTimeString()}`;
    } catch (error) {
      if (!current()) return;
      renderAttention(state);
      const fallback = error?.message?.startsWith("cairn_workspace_input_not_configured:") ? "Cairn workspace not configured" : "Cairn index unavailable";
      state.cairnStatus.textContent = state.cairnSignature ? "Stale · last Cairn snapshot shown" : fallback;
    } finally {
      state.cairnController = null;
      if (current() && state.runtimeStarted) loadRuntime(state);
      if (current() && document.visibilityState !== "hidden") state.cairnTimer = setTimeout(() => { void pollCairn(state); }, CAIRN_REFRESH_MS);
    }
  }

  const currentPrincipal = () => window.KilnWork?.session()?.principal || "";

  function onSessionChanged() {
    if (active && currentPrincipal() !== active.principal) clear();
    else if (active) closeHerdrWindows(active);
  }

  function observationStatus(message) {
    const status = element("p", message, "small fleet-observation-status");
    status.setAttribute("role", "status");
    return status;
  }

  async function pollProjection(state, poll) {
    const current = () => active === state && state.context.isCurrent() && currentPrincipal() === state.principal;
    if (!current() || document.visibilityState === "hidden" || poll.controller) return;
    clearTimeout(poll.timer);
    const controller = new AbortController();
    poll.controller = controller;
    try {
      const value = await state.context.api(poll.path, { quiet: true, controller });
      if (current()) poll.apply(value);
    } catch (error) {
      if (current() && error?.name !== "AbortError") poll.fail();
    } finally {
      poll.controller = null;
      if (current() && document.visibilityState !== "hidden") poll.timer = setTimeout(() => { void pollProjection(state, poll); }, AGGREGATE_REFRESH_MS);
    }
  }

  function onVisible() {
    if (active && document.visibilityState === "visible" && active.context.isCurrent()) {
      restart(active);
      void pollCairn(active);
      for (const poll of active.polls) void pollProjection(active, poll);
      if (active.herdrViewer?.paused) void active.herdrViewer.refresh();
    }
  }

  function handleMultiplexedEvent(state, eventName, payload) {
    if (active !== state || !state.context.isCurrent()) return;
    try {
      if (eventName === "fleet") {
        if (payload?.unavailable) {
          connection(state, state.version ? "stale" : "unavailable", state.version ? "Stale · last snapshot shown" : "Unavailable");
          notice(state.transport, text(payload.error) || "The coordinator did not answer.");
          return;
        }
        applySnapshot(state, payload);
        state.version = String(payload.version);
        live(state);
      } else if (eventName === "heartbeat") {
        if (!state.version) return;
        state.observedAt = Date.now();
        live(state);
      } else if (eventName === "cairn") {
        void pollCairn(state);
      }
    } catch (error) {
      connection(state, state.version ? "stale" : "unavailable", state.version ? "Stale · last snapshot shown" : "Unavailable");
      notice(state.transport, error?.message || "Invalid fleet event payload.");
    }
  }

  function renderDispatchBar(state, status) {
    if (!state.dispatchBar) return;
    if (!record(status)) { state.dispatchBar.hidden = true; return; }
    state.dispatchBar.hidden = false;
    const paused = status.paused === true || text(status.state).toLowerCase() === "paused";
    const deferred = Number.isSafeInteger(status.deferred_count) ? status.deferred_count : 0;
    const badge = element("span", paused ? `DISPATCH PAUSED · ${deferred} deferred` : "DISPATCH RUNNING", "state-badge");
    badge.dataset.tone = paused ? "warning" : "good";
    const detail = element("span", status.reason ? `Reason: ${text(status.reason)}` : `${deferred} deferred attempt${deferred === 1 ? "" : "s"}`, "small");
    const items = [badge, detail];
    if (state.context.canOperate?.() !== false) {
      const btn = element("button", paused ? "Resume dispatch" : "Pause dispatch", paused ? "primary" : "quiet");
      btn.type = "button";
      btn.addEventListener("click", async () => {
        const nextPause = !paused;
        const choice = await confirmGuardedFleetMutation(state, {
          title: nextPause ? "Pause fleet dispatch?" : "Resume fleet dispatch?",
          verb: nextPause ? "Pause dispatch" : "Resume dispatch",
          dismiss: "Keep current dispatch state",
          danger: nextPause,
          warning: nextPause
            ? "Halts new attempt admissions across the factory while letting running attempts finish."
            : "Resumes queued and deferred attempt admissions.",
          facts: [["Current state", paused ? "PAUSED" : "RUNNING"], ["Deferred count", String(deferred)]],
          reason: nextPause ? { label: "Pause reason", placeholder: "Why dispatch is being paused", required: true, maxLength: REASON_MAX } : null,
          cliCommand: `kiln-fleet dispatch ${nextPause ? "pause" : "resume"}`,
          opener: btn,
        });
        if (!choice.confirmed) return;
        const reqId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `dispatch-${Date.now()}`;
        await state.context.api(`/api/fleet/dispatch/${nextPause ? "pause" : "resume"}`, {
          method: "POST",
          body: { ...(choice.reason ? { reason: choice.reason } : {}), request_id: reqId },
        });
        restart(state);
      });
      items.push(btn);
    }
    state.dispatchBar.replaceChildren(...items);
  }

  function buildFleetAtlas(state) {
    const nodes = new Map();
    const routes = [];
    for (const m of state.listedMachines || []) {
      const id = `machine:${m.machine}`;
      if (!nodes.has(id)) nodes.set(id, { id, label: m.machine, role: m.state || "machine", kind: "machine" });
    }
    for (const entry of state.cairnWorkerCells || []) {
      const row = entry.row;
      const cid = `change:${row.change}`;
      if (!nodes.has(cid)) nodes.set(cid, { id: cid, label: row.change, role: row.lane || "change", lane: row.lane, kind: "unit" });
      for (const a of row.activeAssignments || []) {
        const aid = `attempt:${text(a.attempt).slice(0, 12)}`;
        if (!nodes.has(aid)) nodes.set(aid, { id: aid, label: aid, role: "attempt", kind: "attempt" });
        routes.push({ from: cid, to: aid, label: "declared assignment", provenance: "declared" });
        const session = (state.listedAttempts || []).find((x) => x.attempt === a.attempt && x.fence === a.fence)?.session;
        if (text(session?.machine)) {
          const mid = `machine:${session.machine}`;
          if (!nodes.has(mid)) nodes.set(mid, { id: mid, label: session.machine, role: "observed host", kind: "machine" });
          routes.push({ from: aid, to: mid, label: "observed session", provenance: "observed", evidence_ref: a.attempt, digest: a.attempt });
        }
      }
    }
    return {
      identity: text(state.version) || "fleet-atlas-projection",
      title: "Fleet & Cairn execution topology",
      nodes: [...nodes.values()],
      routes,
      packets: [],
      chapters: [],
      open_questions: [],
    };
  }

  let active = null;
  function load(root, context) {
    clear();
    const state = {
      context, controller: null, timer: 0, inFlight: false, mutating: false, signatures: [],
      root, principal: currentPrincipal(), observedAt: 0,
      version: "", epoch: 0, failures: 0, answerInputs: new Map(),
      selectedClaims: new Set(), selectedAttempts: new Set(), unsubscribeEvents: null,
    };
    const page = element("div", undefined, "fleet-page");
    const head = element("section", undefined, "panel fleet-summary");
    const headRow = element("div", undefined, "fleet-summary-head");
    const headline = element("div");
    headline.append(element("h2", "Fleet coordination"), element("p", "A snapshot of claims, work and decisions from the fleet coordinator.", "small"));
    const status = element("div", undefined, "fleet-status");
    const machine = element("span", "Coordinator not yet known", "mono");
    const updated = element("time", "No snapshot received", "small");
    const statusChip = element("span", "Connecting", "fleet-connection");
    statusChip.setAttribute("role", "status");
    status.append(statusChip, machine, updated);
    headRow.append(headline, status);
    const dispatchBar = element("div", undefined, "fleet-dispatch-bar");
    dispatchBar.hidden = true;
    state.dispatchBar = dispatchBar;
    const metrics = element("dl", undefined, "fleet-metrics");
    const claimMetric = metric("Open claims", null), readyMetric = metric("Ready & unclaimed", null), attemptMetric = metric("Listed attempts", null), decisionMetric = metric("Listed open decisions", null);
    metrics.append(claimMetric, readyMetric, attemptMetric, decisionMetric);
    const transport = element("p", undefined, "fleet-banner");
    const command = element("p", undefined, "fleet-banner");
    for (const banner of [transport, command]) {
      banner.setAttribute("role", "alert");
      banner.hidden = true;
    }
    head.append(headRow, dispatchBar, transport, command, metrics);
    // r[impl kiln.fleet.aggregate_workspace]
    const machinesPanel = element("section", undefined, "panel fleet-section");
    machinesPanel.id = "fleet-machines";
    state.machineStatus = observationStatus("Reading machine observations…");
    state.assignmentStatus = observationStatus("Waiting for Fleet assignment observations.");
    state.machineCoverage = observationStatus();
    state.machineCoverage.hidden = true;
    state.machineRows = element("div");
    state.herdrWindowsPanel = element("section");
    state.herdrWindowsPanel.hidden = true;
    machinesPanel.append(element("h2", "Machines"),
      element("p", "Enrolled peers and observed Herdr sessions. Counts cover listed attempts by exact session machine and live claims by holder machine, not claim scope. These counts do not establish host availability.", "small"),
      state.machineStatus, state.assignmentStatus, state.machineCoverage, state.machineRows, state.herdrWindowsPanel);
    const issuesPanel = element("section", undefined, "panel fleet-section");
    issuesPanel.id = "fleet-issues";
    state.issueStatus = observationStatus("Reading native issues and requests…");
    state.issueRows = element("div");
    issuesPanel.append(element("h2", "Native issues and requests"),
      element("p", "Open native issues in Issues and requests in Triage by exact object ID. Imported GitHub and Cairn work references are listed separately below.", "small"),
      state.issueStatus, state.issueRows);
    const sourcesPanel = element("section", undefined, "panel fleet-section");
    sourcesPanel.id = "fleet-work-sources";
    state.sourceCoverage = observationStatus("Waiting for Fleet work observations.");
    state.sourceRows = element("div");
    sourcesPanel.append(element("h2", "Implementation work sources"),
      element("p", "Imported GitHub issues and Cairn references attached to ranked or blocked Fleet work or an attempt’s exact work ID. This is provenance, not a match to a native issue; freshness follows Fleet status above.", "small"),
      state.sourceCoverage, state.sourceRows);
    const claimsPanel = element("section", undefined, "panel fleet-section");
    const claimsHead = element("div", undefined, "fleet-section-head");
    claimsHead.append(element("h2", "Claims"), element("p", "Live coordination claims with their current lease. Renew or release uses the claim's recorded fence.", "small"));
    const acquire = element("div");
    claimsHead.append(acquire);
    const claimsBulk = element("div", undefined, "bulk-toolbar");
    claimsBulk.hidden = true;
    state.claimsBulk = claimsBulk;
    const claimsForeignStatus = observationStatus("");
    claimsForeignStatus.hidden = true;
    state.claimsForeignStatus = claimsForeignStatus;
    const claims = element("div");
    claimsPanel.append(claimsHead, claimsBulk, claimsForeignStatus, claims);
    const work = element("div", undefined, "fleet-work");
    const readyPanel = element("section", undefined, "panel fleet-section");
    readyPanel.append(element("h2", "Ranked work"), element("p", "Coordinator ranking. This list can include claimed work and is separate from the ready-and-unclaimed count.", "small"));
    const ready = element("div");
    readyPanel.append(ready);
    const blockedPanel = element("section", undefined, "panel fleet-section");
    blockedPanel.append(element("h2", "Blocked work"), element("p", "Canonical work with a recorded eligibility reason. The console does not infer a next action.", "small"));
    const blocked = element("div");
    blockedPanel.append(blocked);
    work.append(readyPanel, blockedPanel);
    const attemptsPanel = element("section", undefined, "panel fleet-section");
    attemptsPanel.append(element("h2", "Attempts"), element("p", "Select a bar to filter by state or recorded session machine. Counts cover all listed attempts.", "small"));
    const facets = [attemptFacet("By state", attemptState, ATTEMPT_STATES), attemptFacet("By session machine", attemptMachine)];
    const charts = element("div", undefined, "fleet-charts");
    for (const facet of facets) {
      facet.panel.hidden = true;
      charts.append(facet.panel);
    }
    const filterBar = element("div", undefined, "fleet-filter-bar");
    const result = element("p", "Waiting for a snapshot.", "small");
    result.setAttribute("role", "status");
    const clearFilters = element("button", "Clear filters", "quiet");
    clearFilters.type = "button";
    clearFilters.setAttribute("aria-disabled", "true");
    clearFilters.setAttribute("aria-controls", "fleet-attempt-register");
    clearFilters.addEventListener("click", () => {
      if (!facets.some((facet) => facet.selected !== null)) return;
      for (const facet of facets) facet.selected = null;
      for (const facet of facets) renderFacet(state, facet, state.listedAttempts);
      applyAttemptFilters(state);
    });
    filterBar.append(result, clearFilters);
    const attemptsBulk = element("div", undefined, "bulk-toolbar");
    attemptsBulk.hidden = true;
    state.attemptsBulk = attemptsBulk;
    const attempts = element("div");
    attempts.id = "fleet-attempt-register";
    attemptsPanel.append(charts, filterBar, attemptsBulk, attempts);
    if (state.context.canOperate?.() === true) attemptsPanel.append(placementPanel(state), movePanel(state));
    const decisionsPanel = element("section", undefined, "panel fleet-section");
    state.attention = element("div");
    decisionsPanel.append(element("h2", "Operator inbox"), element("p", "Listed Cairn blockers, claims due within 15 minutes, and Fleet decisions. Check the source status before acting on a retained observation.", "small"), state.attention, element("h3", "Fleet decisions"));
    const decisions = element("div");
    decisionsPanel.append(decisions);
    const swarmsPanel = element("section", undefined, "panel fleet-section");
    swarmsPanel.append(element("h2", "Swarm runs"), element("p", "Read-only inspection of Kiln-governed swarm runs: roster, replayable mention graph, and untrusted board feed.", "small"));
    const swarms = element("div");
    const swarmDetail = element("div");
    swarmsPanel.append(swarms, swarmDetail);
    const cairnPanel = element("section", undefined, "panel fleet-section");
    state.cairnStatus = element("span", "Reading Cairn change index…");
    state.cairnStatus.setAttribute("role", "status");
    state.cairnTime = element("time");
    const cairnStatusBar = element("div", undefined, "fleet-filter-bar small");
    cairnStatusBar.append(state.cairnStatus, state.cairnTime);
    state.cairnCoverage = element("p", undefined, "small");
    state.cairnCoverage.hidden = true;
    state.cairnRows = element("div", undefined, "fleet-cairn-register");
    state.inspector = element("section", undefined, "panel fleet-section");
    Object.assign(state.inspector, { id: "fleet-inspector", hidden: true });
    const inspectorHead = element("div", undefined, "fleet-filter-bar");
    state.inspectorTitle = element("h2");
    state.inspectorTitle.tabIndex = -1;
    const close = element("button", "Close inspector", "quiet");
    close.type = "button";
    close.addEventListener("click", () => {
      state.inspectorController?.abort();
      state.inspectorController = null;
      state.inspector.hidden = true;
      state.inspectorTrigger?.focus();
    });
    inspectorHead.append(state.inspectorTitle, close);
    state.inspectorBody = element("div", undefined, "fleet-inspector-body");
    state.inspector.append(inspectorHead, state.inspectorBody);
    const fleetAtlasToggle = element("button", "Open opt-in Atlas topology (off by default)", "quiet fleet-atlas-toggle");
    fleetAtlasToggle.type = "button";
    fleetAtlasToggle.setAttribute("aria-expanded", "false");
    const fleetAtlasHost = element("div", undefined, "fleet-atlas-host");
    fleetAtlasHost.hidden = true;
    fleetAtlasToggle.addEventListener("click", () => {
      const nextOpen = fleetAtlasHost.hidden;
      fleetAtlasHost.hidden = !nextOpen;
      fleetAtlasToggle.setAttribute("aria-expanded", String(nextOpen));
      fleetAtlasToggle.textContent = nextOpen ? "Hide opt-in Atlas topology" : "Open opt-in Atlas topology (off by default)";
      if (nextOpen) {
        const doc = buildFleetAtlas(state);
        if (typeof window.KilnCairn?.renderAtlasTopology === "function") {
          fleetAtlasHost.replaceChildren(window.KilnCairn.renderAtlasTopology(doc));
        } else if (doc.nodes.length > 64) {
          fleetAtlasHost.replaceChildren(element("p", `Atlas topology graph refused: ${doc.nodes.length} nodes exceeds 64-node cap.`, "warning"));
        } else {
          fleetAtlasHost.replaceChildren(table(["From", "To", "Route", "Provenance"], doc.routes.map((r) => [r.from, r.to, r.label, r.provenance]), "No Atlas routes", "No active Cairn/Fleet routes observed."));
        }
      }
    });
    cairnPanel.append(element("h2", "Cairn progress"), cairnStatusBar,
      element("p", "Dispatch bindings describe the plan, not the running host or model that served a request. Session freshness follows Fleet status above.", "small"),
      state.cairnCoverage, state.cairnRows, fleetAtlasToggle, fleetAtlasHost);
    state.runtime = element("div");
    state.campaign = element("div", undefined, "fleet-work");
    state.capacity = element("div");
    page.append(head, machinesPanel, decisionsPanel, cairnPanel, state.inspector, issuesPanel, attemptsPanel, state.runtime,
      claimsPanel, window.KilnFleetCapacity ? state.capacity : work, sourcesPanel, state.campaign, swarmsPanel);
    if (state.context.canOperate?.() === true) page.append(guestMoveSection(state));
    root.replaceChildren(page);
    state.machine = machine;
    state.updated = updated;
    Object.assign(state, { connection: statusChip, transport, command, facets, result, clearFilters });
    state.metrics = { claims: claimMetric.querySelector("dd"), ready: readyMetric.querySelector("dd"), attempts: attemptMetric.querySelector("dd"), decisions: decisionMetric.querySelector("dd") };
    state.acquire = acquire;
    state.claims = claims;
    state.ready = ready;
    state.blocked = blocked;
    state.attempts = attempts;
    state.decisions = decisions;
    state.swarms = swarms;
    state.swarmDetail = swarmDetail;
    state.selectedSwarm = "";
    state.polls = [{
      path: MACHINES,
      apply(payload) {
        state.listedMachines = admitMachines(payload);
        renderMachines(state);
        notice(state.machineStatus, payload.truncated === true ? "Machine roster is incomplete; additional records were omitted." : "");
      },
      fail() {
        notice(state.machineStatus, state.listedMachines
          ? "Machine observation read failed · last roster snapshot shown."
          : "Machine observation read failed · no roster snapshot available.");
      },
    }, {
      path: NATIVE_WORK,
      apply: (payload) => renderNativeWork(state, payload),
      fail() {
        state.issueSignature = undefined;
        state.issueRows.replaceChildren();
        notice(state.issueStatus, "Native issues and requests disconnected · no authorized snapshot available.");
      },
    }];
    if (typeof context.subscribeEvents === "function") {
      state.polls.push({
        path: DISPATCH_STATUS,
        apply: (payload) => renderDispatchBar(state, payload),
        fail() {},
      }, {
        path: CLAIMS_INCLUDE,
        apply(payload) {
          const foreign = Array.isArray(payload?.foreign_claims) ? payload.foreign_claims.length : 0;
          const expired = Array.isArray(payload?.expired_claims) ? payload.expired_claims.length : 0;
          const mirrorErr = payload?.foreign_mirror?.error ? ` · Mirror: ${text(payload.foreign_mirror.error)}` : "";
          const parts = [];
          if (foreign > 0) parts.push(`${foreign} foreign claim conflict${foreign === 1 ? "" : "s"}`);
          if (expired > 0) parts.push(`${expired} expired lease${expired === 1 ? "" : "s"}`);
          notice(state.claimsForeignStatus, parts.length || mirrorErr ? `${parts.join(" · ")}${mirrorErr}` : "");
        },
        fail() {},
      });
    }
    renderAcquire(state);
    active = state;
    renderAttention(state);
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("kiln-session-changed", onSessionChanged);
    if (typeof context.subscribeEvents === "function") {
      state.unsubscribeEvents = context.subscribeEvents((eventName, payload) => handleMultiplexedEvent(state, eventName, payload));
      const initialFleet = window.KilnFactory?.eventsState?.()?.fleet;
      if (initialFleet) handleMultiplexedEvent(state, "fleet", initialFleet);
    } else {
      void stream(state);
    }
    void pollCairn(state);
    for (const poll of state.polls) void pollProjection(state, poll);
    void window.KilnFleetCapacity?.mount(state.capacity, {
      ...context,
      isFresh: () => isSnapshotFresh(state),
      confirmControl: (opts) => confirmGuardedFleetMutation(state, opts),
    });
    if (window.KilnInventory?.loadFleetCampaign) {
      void window.KilnInventory.loadFleetCampaign(state.campaign, context, context.route?.worker_campaign || "", (campaign) => {
        if (context.isCurrent() && campaign !== (context.route?.worker_campaign || "")) context.navigate?.({ worker_campaign: campaign });
      });
    }
  }

  function clear() {
    if (active) {
      const state = active;
      active = null;
      state.epoch += 1;
      clearTimeout(state.timer);
      state.controller?.abort();
      closeHerdrWindows(state);
      if (typeof state.unsubscribeEvents === "function") {
        state.unsubscribeEvents();
        state.unsubscribeEvents = null;
      }
      clearTimeout(state.cairnTimer);
      state.cairnController?.abort();
      state.inspectorController?.abort();
      state.inspectorController = null;
      for (const poll of state.polls) {
        clearTimeout(poll.timer);
        poll.controller?.abort();
      }
      window.KilnFleetRuntime?.clear();
      window.KilnFleetCapacity?.clear();
      window.KilnInventory?.clearFleetCampaign?.();
      state.root.replaceChildren();
    }
    document.removeEventListener("visibilitychange", onVisible);
    window.removeEventListener("kiln-session-changed", onSessionChanged);
  }

  // r[impl kiln.sdlc.guest_move]
  function guestMoveSection(state) {
    const section = element("section", undefined, "panel fleet-section fleet-guest-move");
    section.id = "fleet-guest-move";
    const status = element("p", "Checking running guest move prerequisites…", "small");
    status.setAttribute("role", "status");
    const controls = element("div", undefined, "fleet-guest-move-controls");
    const detail = element("div", undefined, "fleet-guest-move-detail");
    const refreshBtn = element("button", "Refresh guest move status", "quiet");
    refreshBtn.type = "button";
    section.append(
      element("h2", "Move running guest workload"),
      element("p", "Stopped-guest migration under exclusive Aspen placement and Lattice epoch reconnect: Pause -> Checkpoint -> Transfer -> Restore -> Reconnect -> RetireSource.", "small"),
      status,
      refreshBtn,
      controls,
      detail,
    );
    const base = "/api/fleet/placement/guest-move";
    const current = () => active === state && state.context.isCurrent() && state.context.canOperate?.() === true && section.isConnected;
    const renderMove = (move) => {
      detail.replaceChildren();
      if (!record(move) || !record(move.plan)) return;
      const planId = text(move.plan.identity) || "unknown";
      const outcome = text(move.outcome) || "unknown";
      const currentStep = text(move.current_step) || "none";
      const summary = element(
        "p",
        `Plan ${planId} · ${text(move.plan.source_host)} -> ${text(move.plan.request?.destination)} · current step: ${currentStep} · final outcome: ${outcome} · source retired: ${move.source_retired === true ? "yes" : "no"}`,
      );
      const list = element("ol", undefined, "fleet-guest-move-steps");
      for (const step of Array.isArray(move.steps) ? move.steps : []) {
        const line = `${text(step.kind)} [${text(step.status)}]${step.evidence ? ` · ${text(step.evidence).slice(0, 12)}` : ""}${step.diagnostic ? ` · ${text(step.diagnostic)}` : ""}`;
        list.append(element("li", line));
      }
      detail.append(summary, list);
      if (IDENTITY.test(planId)) {
        const readBtn = element("button", "Read guest move status", "quiet");
        readBtn.type = "button";
        readBtn.addEventListener("click", async () => {
          readBtn.disabled = true;
          try {
            const reply = await state.context.api(`${base}/${planId}`, { quiet: true });
            if (current() && reply?.guest_move) renderMove(reply.guest_move);
          } catch (error) {
            if (current()) status.textContent = `Guest move status unavailable: ${error?.message || "read failed"}.`;
          } finally {
            readBtn.disabled = false;
          }
        });
        detail.append(readBtn);
      }
    };
    const renderOverview = (overview) => {
      if (!current()) return;
      const available = overview?.available === true;
      const ordered = Array.isArray(overview?.ordered_steps) ? overview.ordered_steps.join(" -> ") : "pause -> checkpoint -> transfer -> restore -> reconnect -> retire_source";
      if (!available) {
        status.textContent = text(overview?.reason) || "Fail-closed: running guest move prerequisites (Aspen #15 placement fencing and Lattice epoch reconnect) are unavailable.";
      } else {
        status.textContent = `Running guest move ready · ordered steps: ${ordered}`;
      }
      controls.replaceChildren();
      if (Array.isArray(overview?.prerequisites) && overview.prerequisites.length) {
        const prereqList = element("ul", undefined, "small");
        for (const req of overview.prerequisites) {
          prereqList.append(element("li", `${text(req.owner)} · ${text(req.id)}: ${req.satisfied === true ? "satisfied" : "unavailable"} (${text(req.detail)})`));
        }
        controls.append(prereqList);
      }
      if (!available) {
        if (Array.isArray(overview?.moves) && overview.moves.length) renderMove(overview.moves[0]);
        return;
      }
      const attemptInput = element("input");
      attemptInput.placeholder = "Attempt ID (64 hex)";
      attemptInput.setAttribute("aria-label", "Guest attempt ID");
      const sessionInput = element("input");
      sessionInput.placeholder = "Session ID (64 hex)";
      sessionInput.setAttribute("aria-label", "Guest session ID");
      const fenceInput = element("input");
      fenceInput.type = "number";
      fenceInput.min = "1";
      fenceInput.value = "1";
      fenceInput.setAttribute("aria-label", "Guest attempt fence");
      const versionInput = element("input");
      versionInput.type = "number";
      versionInput.min = "1";
      versionInput.value = "1";
      versionInput.setAttribute("aria-label", "Placement version");
      const destInput = element("input");
      destInput.placeholder = "Destination host";
      destInput.setAttribute("aria-label", "Guest destination host");
      const previewBtn = element("button", "Review guest move", "primary");
      previewBtn.type = "button";
      previewBtn.addEventListener("click", async () => {
        if (!current() || previewBtn.disabled) return;
        const request = {
          attempt: text(attemptInput.value).trim(),
          session: text(sessionInput.value).trim(),
          fence: Number(fenceInput.value),
          placement_version: Number(versionInput.value),
          destination: text(destInput.value).trim(),
        };
        if (!IDENTITY.test(request.attempt) || !IDENTITY.test(request.session) || !request.destination || !(request.fence > 0) || !(request.placement_version > 0)) {
          detail.replaceChildren(element("p", "Provide valid attempt/session 64-hex IDs, positive fence/placement version, and destination host."));
          return;
        }
        previewBtn.disabled = true;
        try {
          const preview = await state.context.api(`${base}/plan`, { method: "POST", body: request, quiet: true });
          const plan = preview?.plan;
          if (!current() || !IDENTITY.test(plan?.identity)) throw new Error("Guest move plan response invalid.");
          const stepNames = (plan.steps || []).map((s) => text(s.kind)).join(" -> ");
          const approval = await openConfirmDialog(state, {
            title: "Move running guest workload?",
            verb: "Execute guest move",
            opener: previewBtn,
            facts: [
              ["Plan", plan.identity],
              ["Attempt", request.attempt],
              ["Session", request.session],
              ["From", text(plan.source_host)],
              ["To", request.destination],
              ["Steps", stepNames],
            ],
            warning: "Pauses, checkpoints, transfers, and restores the running guest under Aspen placement ownership; retires source only after fence and reconnect.",
          });
          if (!approval.confirmed || !current()) return;
          const reply = await state.context.api(base, {
            method: "POST",
            body: { plan: plan.identity, ...request },
            quiet: true,
          });
          if (current() && reply?.guest_move) renderMove(reply.guest_move);
        } catch (error) {
          if (current()) detail.replaceChildren(element("p", `Guest move refused: ${error?.message || "request failed"}.`));
        } finally {
          previewBtn.disabled = false;
        }
      });
      controls.append(attemptInput, sessionInput, fenceInput, versionInput, destInput, previewBtn);
      if (Array.isArray(overview?.moves) && overview.moves.length) renderMove(overview.moves[0]);
    };
    const refresh = async () => {
      try {
        const overview = await state.context.api(base, { quiet: true });
        renderOverview(overview);
      } catch (error) {
        if (current()) status.textContent = `Running guest move unavailable: ${error?.message || "failed closed"}.`;
      }
    };
    refreshBtn.addEventListener("click", () => { void refresh(); });
    void refresh();
    return section;
  }

  window.KilnFleet = Object.freeze({ load, clear });
})();
