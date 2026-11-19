// The Fleet page projects the fleet coordinator's loopback API into the console.
// It is a read view plus the same explicit coordinator commands an operator uses
// from the fleet CLI: claim, renew, release, answer a decision and cancel an
// attempt. The fleet service remains the authority for every command.
(() => {
  "use strict";
  const STREAM_RECONNECT_MS = 1_000;
  const STREAM_BACKOFF_MAX_MS = 30_000;
  const IDENTITY = /^[0-9a-f]{64}$/;
  const VERSION = /^[0-9a-f]{64}$/;
  const TOOL = "kiln-web";
  const SESSION_KEY = "kiln-fleet-session";
  const QUERY_MAX = 256;
  const PURPOSE_MAX = 1024;
  const ANSWER_MAX = 4096;
  const REASON_MAX = 1024;

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
      if (existing) return existing;
      const created = crypto.randomUUID();
      sessionStorage.setItem(SESSION_KEY, created);
      return created;
    } catch (_) {
      return "kiln-web-session";
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

  function renderClaims(state, claims) {
    const now = Date.now();
    const open = claims.filter((claim) => stateText(claim.state) === "open" && (claim.expires_ms || 0) > now).length;
    state.metrics.claims.textContent = String(open);
    const rows = claims.map((claim) => {
      const actions = element("div", undefined, "fleet-actions");
      const renew = element("button", "Renew", "quiet");
      renew.type = "button";
      renew.addEventListener("click", () => mutate(state, renew, `/api/fleet/claims/${claim.identity}/renew`, { fence: claim.fence }));
      const release = element("button", "Release", "quiet");
      release.type = "button";
      release.addEventListener("click", () => mutate(state, release, `/api/fleet/claims/${claim.identity}/release`, { fence: claim.fence }));
      actions.append(renew, release);
      return [identityCell(claim.identity), scopeText(claim.scope), holderText(claim.holder), text(claim.purpose), when(claim.expires_ms), actions];
    });
    state.claims.replaceChildren(table(["ID", "Scope", "Holder", "Purpose", "Expires", "Actions"], rows, "No live claims", "The coordinator reports no live claims."));
  }

  function renderWork(state, work) {
    state.metrics.ready.textContent = String(work.ready_unclaimed);
    const ready = work.ranked.map((item) => [
      identityCell(item.identity), text(item.repository), text(item.title),
      item.interactive ? element("span", "interactive", "state-badge fleet-tag") : "",
    ]);
    state.ready.replaceChildren(table(["ID", "Repository", "Title", "Mode"], ready, "No ranked work", "The coordinator reports no ranked work."));
    const blocked = work.blocked.map((item) => [
      identityCell(item.identity), text(item.repository), text(item.title),
      Array.isArray(item.reasons) ? item.reasons.map((reason) => typeof reason === "string" ? reason : stateText(reason)).join(" · ") : "Not recorded",
    ]);
    state.blocked.replaceChildren(table(["ID", "Repository", "Title", "Reasons"], blocked, "Nothing blocked", "The coordinator reports no blocked work."));
  }

  function renderAttempts(state, attempts) {
    state.metrics.attempts.textContent = String(attempts.length);
    const rows = attempts.map((attempt) => {
      const session = record(attempt.session) ? attempt.session : {};
      const actions = element("div", undefined, "fleet-actions");
      const cancel = element("button", "Cancel", "quiet");
      cancel.type = "button";
      cancel.addEventListener("click", () => {
        const reason = prompt("Cancel reason");
        if (reason === null) return;
        const bounded = reason.trim().slice(0, REASON_MAX);
        if (!bounded) { state.banner.textContent = "A cancel reason is required."; state.banner.classList.add("show"); return; }
        mutate(state, cancel, `/api/fleet/attempts/${attempt.attempt}/cancel`, { reason: bounded });
      });
      actions.append(cancel);
      return [
        identityCell(attempt.attempt),
        `${stateText(attempt.state)} · ${stateText(attempt.phase)}`,
        `${text(attempt.repository) || "Repository not recorded"}\n${text(attempt.title) || "Title not recorded"}`,
        `${text(session.machine) || "Machine not recorded"} ${text(session.agent_name) || ""}\n${stateText(session.state)}`,
        when(attempt.lease_expires_ms),
        actions,
      ];
    });
    state.attempts.replaceChildren(table(["ID", "State", "Work", "Session", "Lease", "Actions"], rows, "No attempts", "The coordinator reports no attempts."));
  }

  function renderDecisions(state, decisions) {
    const open = decisions.filter((decision) => !decision.answer && decision.closed_ms == null).length;
    state.metrics.decisions.textContent = String(open);
    const rows = decisions.map((decision) => {
      let action;
      if (decision.answer != null || decision.closed_ms != null) {
        action = element("span", "Answered", "state-badge");
      } else {
        const form = element("form", undefined, "fleet-answer");
        const input = element("input");
        Object.assign(input, { type: "text", maxLength: ANSWER_MAX, placeholder: "Answer", required: true, autocomplete: "off" });
        const button = element("button", "Answer");
        button.type = "submit";
        form.append(input, button);
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          const answer = input.value.trim().slice(0, ANSWER_MAX);
          if (!answer) return;
          void mutate(state, button, `/api/fleet/decisions/${decision.identity}/answer`, { text: answer });
        });
        action = form;
      }
      return [identityCell(decision.identity), text(decision.question), when(decision.raised_ms), action];
    });
    state.decisions.replaceChildren(table(["ID", "Question", "Raised", "Action"], rows, "No decisions", "The coordinator reports no decisions."));
  }

  function renderAcquire(state) {
    const form = element("form", undefined, "fleet-acquire");
    const scope = element("input");
    Object.assign(scope, { type: "text", maxLength: QUERY_MAX, placeholder: "scope, for example owner/repo", required: true, autocomplete: "off" });
    const purpose = element("input");
    Object.assign(purpose, { type: "text", maxLength: PURPOSE_MAX, placeholder: "purpose", required: true, autocomplete: "off" });
    const minutes = element("input");
    Object.assign(minutes, { type: "number", min: "1", step: "1", placeholder: "lease minutes" });
    const button = element("button", "Acquire");
    button.type = "submit";
    form.append(scope, purpose, minutes, button);
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

  async function mutate(state, button, path, body, form) {
    if (state.mutating) return;
    state.mutating = true;
    button.disabled = true;
    try {
      await state.context.api(path, { method: "POST", body });
      if (form) form.reset();
      restart(state);
    } catch (error) {
      if (error?.name !== "AbortError") {
        state.banner.textContent = error?.message || "The fleet coordinator rejected the command.";
        state.banner.classList.add("show");
      }
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
      let comment = false;
      const data = [];
      for (const line of block.split("\n")) {
        if (line.startsWith(":")) comment = true;
        else if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
      }
      if (data.length) frames.push({ event, data: data.join("\n") });
      else if (comment) frames.push({ event: "heartbeat", data: "" });
    }
    return frames;
  }

  function applySnapshot(state, snapshot) {
    if (!record(snapshot) || typeof snapshot.version !== "string" || !VERSION.test(snapshot.version)) throw new Error("Fleet event response is invalid.");
    validateHealth(snapshot.health);
    validateClaims(snapshot.claims);
    validateWork(snapshot.work);
    validateAttempts(snapshot.attempts);
    validateDecisions(snapshot.decisions);
    const sig = JSON.stringify([snapshot.claims, snapshot.work, snapshot.attempts, snapshot.decisions]);
    if (sig !== state.signature) {
      state.signature = sig;
      renderClaims(state, snapshot.claims.claims);
      renderWork(state, snapshot.work);
      renderAttempts(state, snapshot.attempts.attempts);
      renderDecisions(state, snapshot.decisions.decisions);
    }
    state.machine.textContent = `${snapshot.health.machine} · fleet ${snapshot.health.version}`;
    state.updated.textContent = `Updated ${new Date().toLocaleTimeString()}`;
    state.banner.classList.remove("show");
    state.banner.textContent = "";
    state.failures = 0;
  }

  async function stream(state) {
    if (state.inFlight || !state.context.isCurrent()) return;
    const epoch = state.epoch;
    state.inFlight = true;
    const controller = new AbortController();
    state.controller = controller;
    try {
      const query = state.version ? `?after=${encodeURIComponent(state.version)}` : "";
      const source = await state.context.api(`/api/fleet/events${query}`, { kind: "sse", quiet: true, controller });
      if (epoch !== state.epoch || !state.context.isCurrent()) return;
      let recognized = false;
      for (const frame of eventFrames(source)) {
        if (frame.event === "snapshot") {
          const snapshot = JSON.parse(frame.data);
          applySnapshot(state, snapshot);
          state.version = String(snapshot.version);
          recognized = true;
        } else if (frame.event === "heartbeat") {
          state.updated.textContent = "Live · waiting for changes";
          recognized = true;
        }
      }
      if (!recognized) throw new Error("The fleet event endpoint returned no snapshot or heartbeat.");
      state.failures = 0;
      state.banner.classList.remove("show");
      state.banner.textContent = "";
      if (epoch === state.epoch && state.context.isCurrent()) {
        clearTimeout(state.timer);
        state.timer = setTimeout(() => { void stream(state); }, STREAM_RECONNECT_MS);
      }
    } catch (error) {
      if (error?.name === "AbortError" || epoch !== state.epoch) return;
      state.failures += 1;
      state.banner.textContent = `Fleet unavailable: ${error?.message || "the coordinator did not answer."}`;
      state.banner.classList.add("show");
      state.updated.textContent = "Stale";
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
    state.epoch += 1;
    state.controller?.abort();
    state.inFlight = false;
    clearTimeout(state.timer);
    void stream(state);
  }

  function onVisible() {
    if (active && document.visibilityState === "visible" && active.context.isCurrent()) restart(active);
  }

  let active = null;
  function load(root, context) {
    clear();
    const state = {
      root, context, controller: null, timer: 0, inFlight: false, mutating: false, signature: "",
      version: "", epoch: 0, failures: 0,
    };
    const page = element("div", undefined, "fleet-page");
    const head = element("section", undefined, "panel fleet-summary");
    const headRow = element("div", undefined, "fleet-summary-head");
    const headline = element("div");
    headline.append(element("h2", "Fleet coordination"), element("p", "Claims, attempts and decisions from this host's fleet coordinator. The console forwards commands; the fleet service decides them.", "small"));
    const status = element("div", undefined, "fleet-status");
    const machine = element("span", "Connecting to fleet", "mono");
    const updated = element("span", "Loading", "small");
    status.append(machine, updated);
    headRow.append(headline, status);
    const metrics = element("dl", undefined, "fleet-metrics");
    const claimMetric = metric("Open claims", null), readyMetric = metric("Ready work", null), attemptMetric = metric("Attempts", null), decisionMetric = metric("Open decisions", null);
    metrics.append(claimMetric, readyMetric, attemptMetric, decisionMetric);
    const banner = element("p", undefined, "fleet-banner");
    banner.setAttribute("role", "alert");
    head.append(headRow, metrics, banner);
    const claimsPanel = element("section", undefined, "panel fleet-section");
    const claimsHead = element("div", undefined, "fleet-section-head");
    claimsHead.append(element("h2", "Claims"), element("p", "Live coordination claims with their current lease. Renew or release uses the claim's recorded fence.", "small"));
    const acquire = element("div");
    claimsHead.append(acquire);
    const claims = element("div");
    claimsPanel.append(claimsHead, claims);
    const work = element("div", undefined, "fleet-columns");
    const readyPanel = element("section", undefined, "panel fleet-section");
    readyPanel.append(element("h2", "Ready work"), element("p", "Canonical work eligible for a new attempt, ranked by the coordinator.", "small"));
    const ready = element("div");
    readyPanel.append(ready);
    const blockedPanel = element("section", undefined, "panel fleet-section");
    blockedPanel.append(element("h2", "Blocked work"), element("p", "Canonical work with a recorded eligibility reason. The console does not infer a next action.", "small"));
    const blocked = element("div");
    blockedPanel.append(blocked);
    work.append(readyPanel, blockedPanel);
    const attemptsPanel = element("section", undefined, "panel fleet-section");
    attemptsPanel.append(element("h2", "Attempts"), element("p", "Current attempts, their phase, holder session and lease expiry.", "small"));
    const attempts = element("div");
    attemptsPanel.append(attempts);
    const decisionsPanel = element("section", undefined, "panel fleet-section");
    decisionsPanel.append(element("h2", "Decisions"), element("p", "Questions raised by fleet work. An answer is an operator command and is recorded by the fleet service.", "small"));
    const decisions = element("div");
    decisionsPanel.append(decisions);
    page.append(head, claimsPanel, work, attemptsPanel, decisionsPanel);
    root.replaceChildren(page);
    state.machine = machine;
    state.updated = updated;
    state.banner = banner;
    state.metrics = { claims: claimMetric.querySelector("dd"), ready: readyMetric.querySelector("dd"), attempts: attemptMetric.querySelector("dd"), decisions: decisionMetric.querySelector("dd") };
    state.acquire = acquire;
    state.claims = claims;
    state.ready = ready;
    state.blocked = blocked;
    state.attempts = attempts;
    state.decisions = decisions;
    renderAcquire(state);
    active = state;
    document.addEventListener("visibilitychange", onVisible);
    void stream(state);
  }

  function clear() {
    if (active) {
      clearTimeout(active.timer);
      active.controller?.abort();
      active = null;
    }
    document.removeEventListener("visibilitychange", onVisible);
  }

  window.KilnFleet = Object.freeze({ load, clear });
})();
