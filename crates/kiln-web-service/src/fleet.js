// The Fleet page projects the fleet coordinator's loopback API into the console.
// It is a read view plus the same explicit coordinator commands an operator uses
// from the fleet CLI: claim, renew, release, answer a decision and cancel an
// attempt. The fleet service remains the authority for every command.
(() => {
  "use strict";
  const STREAM_RECONNECT_MS = 1_000;
  const CAIRN_REFRESH_MS = 15_000;
  const CAIRN_INDEX = "/api/implementation/cairn";
  const STREAM_BACKOFF_MAX_MS = 30_000;
  const LEASE_NOTICE_MS = 15 * 60_000;
  const ATTENTION_LIMIT = 8;
  const IDENTITY = /^[0-9a-f]{64}$/;
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

  function renderClaims(state, claims) {
    state.listedClaims = claims;
    renderAttention(state);
    const now = Date.now();
    const open = claims.filter((claim) => stateText(claim.state) === "open" && (claim.expires_ms || 0) > now).length;
    state.metrics.claims.textContent = String(open);
    const rows = claims.map((claim) => {
      const actions = element("div", undefined, "fleet-actions");
      actions.id = `fleet-claim-${claim.identity}`;
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
    const rows = attempts.map((attempt) => {
      const session = record(attempt.session) ? attempt.session : {};
      const actions = element("div", undefined, "fleet-actions");
      const cancel = element("button", "Cancel", "quiet");
      cancel.type = "button";
      cancel.addEventListener("click", () => {
        const reason = prompt("Cancel reason");
        if (reason === null) return;
        const bounded = reason.trim().slice(0, REASON_MAX);
        if (!bounded) { notice(state.command, "A cancel reason is required."); return; }
        mutate(state, cancel, `/api/fleet/attempts/${attempt.attempt}/cancel`, { reason: bounded });
      });
      actions.append(cancel);
      return [
        identityCell(attempt.attempt),
        `${label(attempt.state)}\nPhase: ${label(attempt.phase)}`,
        `${text(attempt.repository) || "Repository not recorded"}\n${text(attempt.title) || "Title not recorded"}`,
        `${text(session.machine) || "Machine not recorded"} ${text(session.agent_name) || ""}\n${label(session.state)}`,
        when(attempt.lease_expires_ms),
        actions,
      ];
    });
    state.attempts.replaceChildren(table(["ID", "State", "Work", "Session", "Lease", "Actions"], rows,
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
    parts.forEach((part, index) => {
      const signature = JSON.stringify(part);
      if (signature === state.signatures[index]) return;
      render[index](state, values[index]);
      state.signatures[index] = signature;
    });
    state.machine.textContent = `${snapshot.health.machine} · fleet ${snapshot.health.version}`;
    const received = new Date();
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
    state.epoch += 1;
    state.controller?.abort();
    state.inFlight = false;
    clearTimeout(state.timer);
    if (state.version && state.connection.dataset.state === "live") connection(state, "reconnecting", "Reconnecting · last snapshot shown");
    void stream(state);
  }

  function cairnLink(state, row) {
    const patch = { page: "cairn", cairn_repository: row.repository, cairn_change: row.change, cairn_unit: "" };
    const name = row.projects.join(", ") || row.repository.slice(0, 12);
    const cell = element("div");
    const link = element("a", row.change, "row-link");
    link.href = `?${new URLSearchParams({ ...patch, theme: state.context.route?.theme || "system" })}`;
    link.title = row.repository;
    link.addEventListener("click", (event) => {
      if (!state.context.navigate || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      state.context.navigate(patch);
    });
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
      if (!confirm(`Reconcile the original request for attempt ${attempt.identity}, fence ${attempt.fence}? This does not start a replacement.`)) return;
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
    entry.append(element("p", "The CI links are for independent verification. The implementation execution reference does not establish a retained log route.", "small"));
    if (attempt.unknown_outcome === "unreconciled" || ["reserved", "transport_started"].includes(attempt.dispatch) || attempt.workflow?.provider_namespace === "lattice") entry.append(recoveryReadout(state, attempt));
    if (state.context.canOperate?.() !== false) entry.append(nativeRuntimeReadout(state, attempt));
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

  function onVisible() {
    if (active && document.visibilityState === "visible" && active.context.isCurrent()) {
      restart(active);
      void pollCairn(active);
    }
  }

  let active = null;
  function load(root, context) {
    clear();
    const state = {
      context, controller: null, timer: 0, inFlight: false, mutating: false, signatures: [],
      version: "", epoch: 0, failures: 0, answerInputs: new Map(),
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
    const metrics = element("dl", undefined, "fleet-metrics");
    const claimMetric = metric("Open claims", null), readyMetric = metric("Ready & unclaimed", null), attemptMetric = metric("Listed attempts", null), decisionMetric = metric("Listed open decisions", null);
    metrics.append(claimMetric, readyMetric, attemptMetric, decisionMetric);
    const transport = element("p", undefined, "fleet-banner");
    const command = element("p", undefined, "fleet-banner");
    for (const banner of [transport, command]) {
      banner.setAttribute("role", "alert");
      banner.hidden = true;
    }
    head.append(headRow, transport, command, metrics);
    const claimsPanel = element("section", undefined, "panel fleet-section");
    const claimsHead = element("div", undefined, "fleet-section-head");
    claimsHead.append(element("h2", "Claims"), element("p", "Live coordination claims with their current lease. Renew or release uses the claim's recorded fence.", "small"));
    const acquire = element("div");
    claimsHead.append(acquire);
    const claims = element("div");
    claimsPanel.append(claimsHead, claims);
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
    const attempts = element("div");
    attempts.id = "fleet-attempt-register";
    attemptsPanel.append(charts, filterBar, attempts);
    const decisionsPanel = element("section", undefined, "panel fleet-section");
    state.attention = element("div");
    decisionsPanel.append(element("h2", "Operator inbox"), element("p", "Listed Cairn blockers, claims due within 15 minutes, and Fleet decisions. Check the source status before acting on a retained observation.", "small"), state.attention, element("h3", "Fleet decisions"));
    const decisions = element("div");
    decisionsPanel.append(decisions);
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
    cairnPanel.append(element("h2", "Cairn progress"), cairnStatusBar,
      element("p", "Dispatch bindings describe the plan, not the running host or model that served a request. Session freshness follows Fleet status above.", "small"),
      state.cairnCoverage, state.cairnRows);
    state.runtime = element("div");
    state.campaign = element("div", undefined, "fleet-work");
    state.capacity = element("div");
    page.append(head, decisionsPanel, cairnPanel, state.inspector, attemptsPanel, state.runtime, claimsPanel, window.KilnFleetCapacity ? state.capacity : work, state.campaign);
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
    renderAcquire(state);
    active = state;
    renderAttention(state);
    document.addEventListener("visibilitychange", onVisible);
    void stream(state);
    void pollCairn(state);
    void window.KilnFleetCapacity?.mount(state.capacity, context);
    if (window.KilnInventory?.loadFleetCampaign) {
      void window.KilnInventory.loadFleetCampaign(state.campaign, context, context.route?.worker_campaign || "", (campaign) => {
        if (context.isCurrent() && campaign !== (context.route?.worker_campaign || "")) context.navigate?.({ worker_campaign: campaign });
      });
    }
  }

  function clear() {
    if (active) {
      active.epoch += 1;
      clearTimeout(active.timer);
      active.controller?.abort();
      clearTimeout(active.cairnTimer);
      active.cairnController?.abort();
      active.inspectorController?.abort();
      active.inspectorController = null;
      window.KilnFleetRuntime?.clear();
      window.KilnFleetCapacity?.clear();
      window.KilnInventory?.clearFleetCampaign?.();
      active = null;
    }
    document.removeEventListener("visibilitychange", onVisible);
  }

  window.KilnFleet = Object.freeze({ load, clear });
})();
