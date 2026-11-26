import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class DomNode {
  constructor(tag, id = "") {
    this.tagName = String(tag).toUpperCase();
    this.id = id;
    this.children = [];
    this.parentNode = null;
    this.attributes = new Map();
    const self = this;
    this.dataset = new Proxy({}, {
      get(_, prop) {
        if (typeof prop !== "string") return undefined;
        const attr = "data-" + prop.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
        return self.attributes.get(attr);
      },
      set(_, prop, val) {
        if (typeof prop !== "string") return true;
        const attr = "data-" + prop.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
        self.attributes.set(attr, String(val));
        return true;
      },
      deleteProperty(_, prop) {
        if (typeof prop !== "string") return true;
        const attr = "data-" + prop.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
        self.attributes.delete(attr);
        return true;
      },
    });
    this.listeners = new Map();
    this.hidden = false;
    this.disabled = false;
    this.checked = false;
    this.required = false;
    this.open = false;
    this.tabIndex = 0;
    this.value = "";
    this.className = "";
    this.title = "";
    this.href = "";
    this.type = "";
    this.dateTime = "";
    this.scrollTop = 0;
    this.scrollHeight = 1000;
    this.clientHeight = 200;
    this.offsetTop = 0;
    this._text = "";
    this.style = { zoom: 1 };
    this.classList = {
      add(...names) {
        const set = new Set((self.className || "").split(/\s+/).filter(Boolean));
        for (const n of names) set.add(n);
        self.className = [...set].join(" ");
      },
      remove(...names) {
        const rm = new Set(names);
        self.className = (self.className || "").split(/\s+/).filter((x) => x && !rm.has(x)).join(" ");
      },
      toggle(name, force) {
        const has = self.classList.contains(name);
        const want = force === undefined ? !has : Boolean(force);
        if (want) self.classList.add(name);
        else self.classList.remove(name);
        return want;
      },
      contains(name) {
        return (self.className || "").split(/\s+/).includes(name);
      },
    };
  }
  get parentElement() { return this.parentNode; }
  get isConnected() {
    let cur = this;
    while (cur) {
      if (cur.tagName === "BODY") return true;
      cur = cur.parentNode;
    }
    return false;
  }
  get firstChild() { return this.children[0] || null; }
  get firstElementChild() { return this.children.find((c) => c.tagName !== "#TEXT") || null; }
  get options() { return this.children; }
  get childElementCount() { return this.children.filter((c) => c.tagName !== "#TEXT").length; }
  set textContent(v) { this.children = []; this._text = v == null ? "" : String(v); }
  get textContent() {
    if (!this.children.length) return this._text;
    return this.children.map((c) => c.textContent).join("");
  }
  setAttribute(k, v) {
    const str = String(v);
    this.attributes.set(k, str);
    if (k.startsWith("data-")) {
      const prop = k.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      this.dataset[prop] = str;
    }
  }
  getAttribute(k) { return this.attributes.get(k) ?? null; }
  hasAttribute(k) { return this.attributes.has(k); }
  removeAttribute(k) { this.attributes.delete(k); }
  toggleAttribute(name, force) {
    const has = this.attributes.has(name);
    const want = force === undefined ? !has : Boolean(force);
    if (want) this.setAttribute(name, "");
    else this.removeAttribute(name);
    return want;
  }
  append(...items) {
    for (const item of items) {
      if (item == null) continue;
      if (typeof item === "string" || typeof item === "number") {
        const t = new DomNode("#text");
        t._text = String(item);
        t.parentNode = this;
        this.children.push(t);
      } else if (item.tagName === "#FRAGMENT") {
        for (const ch of [...item.children]) {
          ch.parentNode = this;
          this.children.push(ch);
        }
        item.children = [];
      } else {
        item.parentNode = this;
        this.children.push(item);
      }
    }
  }
  prepend(...items) {
    const normalized = [];
    for (const item of items) {
      if (item == null) continue;
      if (typeof item === "string") {
        const t = new DomNode("#text");
        t._text = item;
        t.parentNode = this;
        normalized.push(t);
      } else {
        item.parentNode = this;
        normalized.push(item);
      }
    }
    this.children.unshift(...normalized);
  }
  insertBefore(newNode, refNode) {
    if (!refNode) { this.append(newNode); return newNode; }
    const idx = this.children.indexOf(refNode);
    newNode.parentNode = this;
    if (idx < 0) this.children.push(newNode);
    else this.children.splice(idx, 0, newNode);
    return newNode;
  }
  after(...items) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    const normalized = items.map((item) => {
      if (typeof item === "string") {
        const t = new DomNode("#text");
        t._text = item;
        t.parentNode = this.parentNode;
        return t;
      }
      item.parentNode = this.parentNode;
      return item;
    });
    this.parentNode.children.splice(idx + 1, 0, ...normalized);
  }
  replaceChildren(...items) { this.children = []; this._text = ""; this.append(...items); }
  remove() {
    if (this.parentNode) {
      this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
      this.parentNode = null;
    }
  }
  addEventListener(type, fn) { this.listeners.set(type, [...(this.listeners.get(type) || []), fn]); }
  removeEventListener(type, fn) { this.listeners.set(type, (this.listeners.get(type) || []).filter((x) => x !== fn)); }
  dispatchEvent(ev) { for (const fn of this.listeners.get(ev.type) || []) fn(ev); }
  click() { this.dispatchEvent({ type: "click", button: 0, preventDefault() {} }); }
  focus() { if (this.ownerDocument) this.ownerDocument.activeElement = this; }
  scrollIntoView() { this.scrolledIntoView = true; }
  showModal() { this.open = true; }
  close() { this.open = false; }
  getClientRects() { return []; }
  getBoundingClientRect() { return { top: 0, bottom: 100, left: 0, right: 100, width: 100, height: 100 }; }
  closest(sel) {
    let cur = this;
    while (cur) {
      if (matchesSimpleSelector(cur, sel)) return cur;
      cur = cur.parentNode;
    }
    return null;
  }
  querySelectorAll(sel) {
    const out = [];
    const visit = (n) => {
      for (const c of n.children || []) {
        if (matchesSimpleSelector(c, sel)) out.push(c);
        visit(c);
      }
    };
    visit(this);
    return out;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}

function matchesSimpleSelector(node, selector) {
  if (!node || !selector) return false;
  return selector.split(",").some((part) => {
    const s = part.trim();
    if (!s) return false;
    // Handle descendant combinator
    const parts = s.split(/\s+/);
    const leaf = parts[parts.length - 1];
    if (!matchesLeaf(node, leaf)) return false;
    if (parts.length === 1) return true;
    let cur = node.parentNode;
    for (let i = parts.length - 2; i >= 0; i--) {
      while (cur && !matchesLeaf(cur, parts[i])) cur = cur.parentNode;
      if (!cur) return false;
      cur = cur.parentNode;
    }
    return true;
  });
}

function matchesLeaf(node, s) {
  let rem = s;
  const tagMatch = /^[a-zA-Z0-9_-]+/.exec(rem);
  if (tagMatch) {
    if (node.tagName !== tagMatch[0].toUpperCase()) return false;
    rem = rem.slice(tagMatch[0].length);
  }
  while (rem.length) {
    if (rem.startsWith("#")) {
      const m = /^#([a-zA-Z0-9_-]+)/.exec(rem);
      if (!m || node.id !== m[1]) return false;
      rem = rem.slice(m[0].length);
    } else if (rem.startsWith(".")) {
      const m = /^\.([a-zA-Z0-9_-]+)/.exec(rem);
      if (!m || !node.classList?.contains(m[1])) return false;
      rem = rem.slice(m[0].length);
    } else if (rem.startsWith("[")) {
      const m = /^\[([a-zA-Z0-9_-]+)(?:="([^"]*)")?\]/.exec(rem);
      if (!m) return false;
      const val = node.getAttribute(m[1]);
      if (m[2] !== undefined ? val !== m[2] : val == null) return false;
      rem = rem.slice(m[0].length);
    } else {
      return false;
    }
  }
  return true;
}

function createHarness({ locationSearch = "?page=runs", fetchImpl, clipboardLog = [] } = {}) {
  const nodes = new Map();
  const body = new DomNode("body");
  const docListeners = new Map();
  const doc = {
    documentElement: new DomNode("html"),
    body,
    activeElement: null,
    visibilityState: "visible",
    title: "",
    createElement: (tag) => Object.assign(new DomNode(tag), { ownerDocument: doc }),
    createElementNS: (_ns, tag) => Object.assign(new DomNode(tag), { ownerDocument: doc }),
    createTextNode: (t) => Object.assign(new DomNode("#text"), { _text: String(t), ownerDocument: doc }),
    createDocumentFragment: () => Object.assign(new DomNode("#fragment"), { ownerDocument: doc }),
    getElementById: (id) => findById(id),
    querySelectorAll: (sel) => body.querySelectorAll(sel),
    querySelector: (sel) => body.querySelector(sel),
    addEventListener: (t, fn) => docListeners.set(t, [...(docListeners.get(t) || []), fn]),
    removeEventListener: (t, fn) => docListeners.set(t, (docListeners.get(t) || []).filter((x) => x !== fn)),
    dispatchEvent: (ev) => { for (const fn of docListeners.get(ev.type) || []) fn(ev); },
  };
  const attach = (el) => { if (el.id) nodes.set(el.id, el); return el; };
  const el = (tag, id = "") => attach(Object.assign(new DomNode(tag), { id, ownerDocument: doc }));
  const findById = (id) => {
    if (nodes.has(id)) return nodes.get(id);
    const stack = [body];
    while (stack.length) {
      const cur = stack.pop();
      if (cur.id === id) { nodes.set(id, cur); return cur; }
      if (Array.isArray(cur.children)) stack.push(...cur.children);
    }
    const created = el("div", id);
    body.append(created);
    return created;
  };

  // Build structural run panels matching web.html so insertBefore / applyRunPanels / #log-form work authentically
  const runPanels = el("div");
  runPanels.className = "run-primary-panels";
  const pipelinePanel = el("section");
  pipelinePanel.setAttribute("data-run-panel", "pipeline");
  const logsPanel = el("section");
  logsPanel.setAttribute("data-run-panel", "logs");
  const logForm = el("form", "log-form");
  const logNode = el("select", "log-node");
  const logStream = el("select", "log-stream");
  for (const val of ["stdout", "stderr", "diagnostic"]) {
    const opt = el("option");
    opt.value = val;
    opt.textContent = val;
    logStream.append(opt);
  }
  logForm.append(logNode, logStream);
  const logOutput = el("pre", "log-output");
  const logErrorBar = el("div", "log-error-bar");
  const logErrorCount = el("span", "log-error-count");
  const logJumpError = el("button", "log-jump-error");
  logsPanel.append(logForm, logErrorBar, logErrorCount, logJumpError, logOutput);
  runPanels.append(pipelinePanel, logsPanel);
  for (const name of ["artifacts", "tests", "reviews", "compare", "activity", "evidence"]) {
    const p = el("section");
    p.setAttribute("data-run-panel", name);
    body.append(p);
  }
  body.append(runPanels);

  const winListeners = new Map();
  const win = {
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    addEventListener: (t, fn) => winListeners.set(t, [...(winListeners.get(t) || []), fn]),
    removeEventListener: (t, fn) => winListeners.set(t, (winListeners.get(t) || []).filter((x) => x !== fn)),
    dispatchEvent: (ev) => { for (const fn of winListeners.get(ev.type) || []) fn(ev); },
    scrollTo() {},
  };
  const loc = { search: locationSearch, hash: "", pathname: "/" };
  const mockResponse = (bodyText, contentType = "application/json", status = 200) => {
    const bytes = new TextEncoder().encode(bodyText);
    let read = false;
    return {
      ok: status >= 200 && status < 300,
      status,
      headers: { get: (n) => (String(n).toLowerCase() === "content-type" ? contentType : null) },
      body: {
        getReader: () => ({
          read: async () => {
            if (read) return { done: true, value: undefined };
            read = true;
            return { done: false, value: bytes };
          },
          cancel: async () => {},
          releaseLock() {},
        }),
      },
    };
  };
  const ctx = {
    window: win,
    document: doc,
    Node: DomNode,
    Element: DomNode,
    HTMLElement: DomNode,
    Event: class { constructor(type) { this.type = type; } },
    URL,
    DOMException,
    AbortController,
    URLSearchParams,
    TextEncoder,
    TextDecoder,
    Intl,
    crypto: webcrypto,
    navigator: { clipboard: { writeText: async (txt) => { clipboardLog.push(txt); } } },
    MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
    location: loc,
    history: {
      pushState: (_s, _t, u) => { if (typeof u === "string") loc.search = new URL(u, "http://localhost").search; },
      replaceState: (_s, _t, u) => { if (typeof u === "string") loc.search = new URL(u, "http://localhost").search; },
    },
    fetch: (url, opts) => (fetchImpl ? fetchImpl(url, opts, mockResponse) : Promise.resolve(mockResponse("{}", "application/json", 401))),
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
  };
  win.window = win;
  win.navigator = ctx.navigator;
  vm.runInNewContext(asset("icons.js"), ctx);
  vm.runInNewContext(asset("ui.js"), ctx);
  vm.runInNewContext(asset("workers.js"), ctx);
  vm.runInNewContext(asset("receipts.js"), ctx);
  vm.runInNewContext(asset("code-review.js"), ctx);
  vm.runInNewContext(asset("web.js"), ctx);
  return { ctx, doc, win, loc, findById, logsPanel, settle: () => new Promise((r) => setTimeout(r, 20)) };
}

// ---------- 1. Causal Event-Sourced Decision Timeline ----------

test("r[kiln.web.causal_timeline_and_failure_digest] Causal timeline orders stages chronologically, renders copyable 12-char BLAKE3 prefix, and preserves raw JSON in <details>", async () => {
  const clipboardLog = [];
  const { ctx, doc, settle } = createHarness({ clipboardLog });
  await settle();
  const timeline = ctx.window.KilnFactory.timeline;
  const parent = doc.createElement("div");

  const hashA = "a1b2c3d4e5f60123456789abcdef0123456789abcdef0123456789abcdef0123";
  const hashB = "f0e1d2c3b4a50123456789abcdef0123456789abcdef0123456789abcdef9999";
  const hashC = "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff";

  // Supply reverse-chronological API items (as /api/activity returns) to verify chronological sorting
  timeline.renderActivity(parent, {
    total: 4,
    audit_window: 256,
    next_cursor: null,
    items: [
      {
        kind: "canonical",
        run_id: "run-42",
        ordinal: 1,
        event_index: 2,
        recorded_at_ms: 1700000004000,
        event_identity: hashC,
        event: { AttemptCompleted: { command_id: "cmd-term", attempt_id: "att-1", outcome: "DomainFailure", products: [], metrics: [] } },
      },
      {
        kind: "canonical",
        run_id: "run-42",
        ordinal: 1,
        event_index: 1,
        recorded_at_ms: 1700000003000,
        event_identity: hashB,
        event: { RetryStarted: { command_id: "cmd-disp", run_id: "run-42", prior_attempt_id: "att-0", next_attempt_id: "att-1", effect_id: "eff-99", provider_profile: "lattice-v1" } },
      },
      {
        kind: "audit",
        run_id: "run-42",
        ordinal: 1,
        recorded_at_ms: 1700000002000,
        audit: { method: "POST", target: "/api/runs/run-42/approve", decision: "allowed", actor_label: "alice@example", agent: { harness: "omp", model: "gemini-4" }, identity: "99".repeat(32) },
      },
      {
        kind: "canonical",
        run_id: "run-42",
        ordinal: 1,
        event_index: 0,
        recorded_at_ms: 1700000001000,
        event_identity: hashA,
        event: { RunStarted: { command_id: "cmd-admit", run_id: "run-42", attempt_id: "att-0", provider_profile: "lattice-v1" } },
      },
    ],
  });

  // Rail shows reached stages
  const railSteps = parent.querySelectorAll(".causal-stage-step");
  assert.equal(railSteps.length, 6);
  const reachedStages = railSteps.filter((s) => s.dataset.reached === "true").map((s) => s.dataset.stage);
  assert.deepEqual(reachedStages, ["admitted", "untrusted_gate", "dispatched", "terminal"]);

  // Events are rendered in chronological stage progression order
  const eventNodes = parent.querySelectorAll(".causal-event");
  assert.equal(eventNodes.length, 4);
  assert.deepEqual(eventNodes.map((n) => n.dataset.stage), ["admitted", "untrusted_gate", "dispatched", "terminal"]);

  // Attribution & copyable 12-char BLAKE3 prefix
  assert.match(eventNodes[1].textContent, /alice@example/);
  assert.match(eventNodes[1].textContent, /Agent omp/);
  const prefixes = parent.querySelectorAll(".causal-identity-prefix");
  assert.equal(prefixes[0].textContent, hashA.slice(0, 12));
  assert.equal(prefixes[0].title, hashA);

  // Clicking copy button copies full 64-char BLAKE3 identity
  const copyBtn = eventNodes[0].querySelector(".causal-copy-btn");
  assert.ok(copyBtn);
  await copyBtn.click();
  assert.deepEqual(clipboardLog, [hashA]);

  // Raw JSON stays inside collapsible <details class="causal-raw">
  const rawDetails = eventNodes[0].querySelector("details.causal-raw");
  assert.ok(rawDetails);
  assert.equal(rawDetails.open, false);
  assert.match(rawDetails.querySelector("pre").textContent, /"RunStarted"/);
});

test("Causal timeline negative path: malformed activity response throws explicitly without inventing events", async () => {
  const { ctx, doc, settle } = createHarness();
  await settle();
  const parent = doc.createElement("div");
  assert.throws(() => ctx.window.KilnFactory.timeline.renderActivity(parent, { events: [] }), /Activity response is invalid/);
});

// ---------- 2. Deduplicated Cross-Node Failure Digest ----------

test("r[kiln.web.causal_timeline_and_failure_digest.scenario.multi_node] Cross-node failure digest deduplicates repeated signatures across failed nodes and 1-click jump switches node/stream and scrolls to first error", async () => {
  const failedRun = {
    id: "run-dag-fail",
    project_id: "kiln",
    source_revision: "a".repeat(40),
    canonical_tree: "b".repeat(64),
    repository: { provider_namespace: "git", native_kind: "repo", native_id: "onixresearch/kiln" },
    revision: 4,
    attempt_id: "att-1",
    outcome: "DomainFailure",
    phase: "terminal",
    execution: { provider_namespace: "lattice", native_kind: "workflow", native_id: "wf-9" },
    sequence: 5,
    nodes: [
      { id: "build-linux", kind: "test", state: "Failed" },
      { id: "build-macos", kind: "test", state: "Failed" },
      { id: "lint-rust", kind: "lint", state: "Failed" },
      { id: "docs-check", kind: "docs", state: "Completed" },
    ],
    edges: [
      { source: "build-linux", target: "docs-check" },
      { source: "build-macos", target: "docs-check" },
    ],
    controls: { cancel: false, pause: false, resume: false },
    error_code: null,
    log_bounds: { maximum_chunk_bytes: 16384, maximum_total_bytes: 65536, maximum_chunks: 16 },
    ordinal: 1,
    parent_run_id: null,
    selected_nodes: [],
    created_at_ms: 1700000000000,
    started_at_ms: 1700000001000,
    completed_at_ms: 1700000005000,
    duration_ms: 4000,
    report_summary: {},
    step_telemetry: [],
  };

  // Pure deduplication check: identical normalized error across build-linux and build-macos collapses to 1 group
  const { ctx, findById, doc, loc } = createHarness({
    locationSearch: "?page=runs&run=run-dag-fail&view=logs",
    fetchImpl: async (url, _opts, mockResponse) => {
      const u = String(url);
      if (u === "/api/session") return mockResponse(JSON.stringify({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } }));
      if (u === "/api/projects") return mockResponse(JSON.stringify({ available: [], enrolled: [{ id: "kiln", label: "Kiln", repository: "onixresearch/kiln", reference: "refs/heads/main", pipeline_identity: "p" }] }));
      if (u.startsWith("/api/live") || u.startsWith("/api/events")) return mockResponse(": heartbeat\n\n", "text/event-stream");
      if (u.startsWith("/api/runs?")) return mockResponse(JSON.stringify({ runs: [failedRun], next_cursor: null, total: 1 }));
      if (u === "/api/runs/run-dag-fail") return mockResponse(JSON.stringify(failedRun));
      if (u.includes("/logs?") && u.includes("node_id=build-linux") && u.includes("stream=stderr")) {
        return mockResponse(JSON.stringify({ node_id: "build-linux", stream: "stderr", source: "worker-capture", cursor: "0", next_cursor: "120", text: "compiling kiln\nerror[E0308]: mismatched types at src/lib.rs:42:5\n", retained: "complete", truncated: false }));
      }
      if (u.includes("/logs?") && u.includes("node_id=build-macos") && u.includes("stream=stderr")) {
        return mockResponse(JSON.stringify({ node_id: "build-macos", stream: "stderr", source: "worker-capture", cursor: "0", next_cursor: "140", text: "line 1\nline 2\nerror[E0308]: mismatched types at src/lib.rs:99:12\n", retained: "complete", truncated: false }));
      }
      if (u.includes("/logs?") && u.includes("node_id=lint-rust") && u.includes("stream=stderr")) {
        return mockResponse(JSON.stringify({ node_id: "lint-rust", stream: "stderr", source: "worker-capture", cursor: "0", next_cursor: "80", text: "error: clippy::unwrap_used denied\n", retained: "complete", truncated: false }));
      }
      if (u.endsWith("/artifacts")) return mockResponse(JSON.stringify({ artifacts: [] }));
      if (u.endsWith("/activity")) return mockResponse(JSON.stringify({ items: [], total: 0, audit_window: 64, next_cursor: null }));
      if (u.endsWith("/evidence")) return mockResponse(JSON.stringify({ run_id: "run-dag-fail" }));
      return mockResponse("{}");
    },
  });

  try {
    const pureDigest = ctx.window.KilnFactory.failureDigest.build(failedRun, new Map([
      ["build-linux", { stream: "stderr", firstErrorLine: "error[E0308]: mismatched types at src/lib.rs:42:5", lineIndex: 1 }],
      ["build-macos", { stream: "stderr", firstErrorLine: "error[E0308]: mismatched types at src/lib.rs:99:12", lineIndex: 2 }],
      ["lint-rust", { stream: "stderr", firstErrorLine: "error: clippy::unwrap_used denied", lineIndex: 0 }],
    ]));
    assert.equal(pureDigest.failedNodeCount, 3);
    assert.equal(pureDigest.groups.length, 2);
    assert.equal(pureDigest.groups[0].count, 2);
    assert.deepEqual([...pureDigest.groups[0].nodes.map((n) => n.nodeId)], ["build-linux", "build-macos"]);

    await new Promise((r) => setTimeout(r, 80));
    const digestBox = doc.getElementById("log-failure-digest");
    assert.equal(digestBox.hidden, false);
    const jumpButtons = digestBox.querySelectorAll(".log-digest-jump");
    assert.equal(jumpButtons.length, 3);
    assert.equal(digestBox.querySelectorAll(".log-digest-group").length, 2);

    // 1-click jump to build-macos (stderr) without touching #log-node dropdown manually
    const macBtn = jumpButtons.find((b) => b.dataset.nodeId === "build-macos");
    assert.ok(macBtn);
    macBtn.click();
    await new Promise((r) => setTimeout(r, 60));

    assert.equal(findById("log-node").value, "build-macos");
    assert.equal(findById("log-stream").value, "stderr");
    assert.match(loc.search, /node=build-macos/);
    const logOut = findById("log-output");
    assert.match(logOut.textContent, /mismatched types at src\/lib\.rs:99:12/);
    const firstMark = logOut.querySelector("mark.log-error-line");
    assert.ok(firstMark);
    assert.equal(logOut.scrolledIntoView, true);
    assert.equal(doc.activeElement, firstMark);

    // Negative check: passing run with 0 failed nodes hides digest
    ctx.window.KilnFactory.failureDigest.render({ id: "run-ok", nodes: [{ id: "ok", kind: "test", state: "Completed" }] });
    assert.equal(digestBox.hidden, true);
  } finally {
    findById("logout").listeners.get("click")[0]();
  }
});

// ---------- 3. Unified Gate & Receipt Passport & Provenance Honesty ----------

test("r[kiln.cairn.gate_passport_surface.scenario.provenance_separation] Agent handoff carries dotted Agent Claim and NEVER marks cairn-authoritative-completion as passed without independent canonical receipt", async () => {
  const { ctx, settle } = createHarness();
  await settle();
  const receipts = ctx.window.KilnReceipts;

  const passportInput = {
    current_candidate_id: "cand-cur-111",
    current_revision: "rev-cur-111",
    handoff: {
      agent: "omp-worker-1",
      outcome: "passed",
      attempt_id: "att-agent-1",
      candidate_id: "cand-cur-111",
      subject_hash: "a".repeat(64),
    },
    // Explicitly claim completion without an independent canonical receipt
    verification: {
      independent_receipt: null,
      outcome: "passed",
      candidate_id: "cand-cur-111",
    },
    checks: [
      { name: "cairn-authoritative-completion", kind: "cairn-agent-completion-v2", outcome: "pass", self_reported: true, candidate_id: "cand-cur-111" },
      { name: "cargo-test", kind: "unit_test", outcome: "pass", artifact_digest: "b".repeat(64), candidate_id: "cand-cur-111" },
    ],
  };

  const model = receipts.normalizeGatePassport(passportInput);
  assert.equal(model.canonicalCompletionPassed, false);
  assert.equal(model.hasAgentHandoffOnly, true);

  const el = receipts.renderGatePassport(passportInput);
  const completionCards = el.querySelectorAll('[data-category="cairn_completion"]');
  assert.ok(completionCards.length >= 2);

  const agentClaims = completionCards.filter((c) => c.dataset.provenance === "agent-claim");
  assert.equal(agentClaims.length, 2);
  for (const card of agentClaims) {
    assert.equal(card.querySelector(".passport-badge").textContent, "Agent Claim");
  }

  const canonicalCard = completionCards.find((c) => c.querySelector(".passport-entry-title")?.textContent === "cairn-authoritative-completion");
  assert.ok(canonicalCard);
  assert.equal(canonicalCard.dataset.provenance, "declared");
  assert.match(canonicalCard.textContent, /Unverified \/ Pending/);
  assert.doesNotMatch(canonicalCard.textContent, /\bPassed\b/);
  const statusPill = canonicalCard.querySelector(".status-pill");
  assert.notEqual(statusPill.dataset.status, "success");
});

test("r[kiln.cairn.gate_passport_surface] Gate Passport unites all 6 evidence domains, enforces solid/dotted/dashed borders in CSS, and flags superseded candidates as historical/stale", async () => {
  const { ctx, settle } = createHarness();
  await settle();
  const receipts = ctx.window.KilnReceipts;
  const receiptsCss = asset("receipts.css");

  // Verify strict border grammar tokens & border-style declarations in receipts.css
  assert.match(receiptsCss, /--passport-observed:\s*var\(--success\)/);
  assert.match(receiptsCss, /--passport-agent-claim:\s*var\(--warning\)/);
  assert.match(receiptsCss, /--passport-declared:\s*var\(--border-emphasis\)/);
  assert.match(receiptsCss, /\[data-provenance="observed"\][^{]*\{\s*border:\s*1px solid var\(--passport-observed\)/);
  assert.match(receiptsCss, /\[data-provenance="agent-claim"\][^{]*\{\s*border:\s*1px dotted var\(--passport-agent-claim\)/);
  assert.match(receiptsCss, /\[data-provenance="declared"\][^{]*\{\s*border:\s*1px dashed var\(--passport-declared\)/);

  const currentCand = "cand-current-999";
  const staleCand = "cand-superseded-000";

  const passport = receipts.renderGatePassport({
    current_candidate_id: currentCand,
    current_revision: "rev-2",
    checks: [
      { name: "clippy", kind: "lint", outcome: "pass", artifact_digest: "c".repeat(64), candidate_id: currentCand },
      { name: "old-check", kind: "test", outcome: "pass", artifact_digest: "d".repeat(64), candidate_id: staleCand },
    ],
    cairn_completion: {
      outcome: "verified",
      independent_receipt: "e".repeat(64),
      verifier: "kiln-independent-verifier-v1",
      candidate_id: currentCand,
    },
    handoff: {
      agent: "omp-worker",
      outcome: "complete",
      candidate_id: currentCand,
    },
    patchgate: {
      schema: "kiln.patchgate.review.v1",
      observation: {
        isolated: true,
        report_identity: "f".repeat(64),
        candidate_id: currentCand,
        reviews: [{ change: { native_id: "1".repeat(64) }, decision: "accepted" }],
      },
    },
    provenance: {
      _type: "https://in-toto.io/Statement/v1",
      predicateType: "https://slsa.dev/provenance/v1",
      subject: [{ name: "kiln-bin", digest: { blake3: "2".repeat(64) } }],
      predicate: {
        buildDefinition: {
          buildType: "https://kiln.onox/nix-build/v1",
          externalParameters: { candidateId: currentCand },
          resolvedDependencies: [{ uri: "pkg:nix/rustc", digest: { sha256: "3".repeat(64) } }],
        },
        runDetails: { builder: { id: "mantle-v1" } },
      },
    },
    plan_review_receipt: {
      work_id: "work-123",
      plan_id: "plan-456",
      plan_revision: 2,
      outcome: "changed",
      reviewer: "alice",
      candidate_id: currentCand,
      comparison: { kept: ["step-1"], changed: ["step-2"], added: ["step-3"], removed: [] },
    },
    semantic_merge: {
      outcome: "Verified",
      mode: "Required",
      lineage_policy: "PijulChangeContainment",
      checker: "noble-semantic-v1",
      request_identity: "4".repeat(64),
      base: { exact_revision: "O-rev-111", canonical_tree: "5".repeat(64) },
      variant_a: { exact_revision: "A-rev-222", canonical_tree: "6".repeat(64) },
      variant_b: { exact_revision: "B-rev-333", canonical_tree: "7".repeat(64) },
      candidate: { exact_revision: currentCand, canonical_tree: "8".repeat(64) },
    },
  });

  // All 6 categories rendered
  for (const cat of ["ci_checks", "cairn_completion", "patchgate", "provenance_digests", "plan_review", "semantic_merge"]) {
    assert.ok(passport.querySelector(`[data-category="${cat}"]`), `missing category ${cat}`);
  }

  // O/A/B/M quartet cells rendered
  const quartetCells = passport.querySelectorAll(".passport-quartet-cell");
  assert.equal(quartetCells.length, 4);
  assert.match(quartetCells[0].textContent, /O \(Base\)/);
  assert.match(quartetCells[1].textContent, /A \(Variant A\)/);
  assert.match(quartetCells[2].textContent, /B \(Variant B\)/);
  assert.match(quartetCells[3].textContent, /M \(Candidate\)/);

  // SLSA subject & dependency digests rendered
  assert.match(passport.textContent, /kiln-bin/);
  assert.match(passport.textContent, /pkg:nix\/rustc/);

  // Plan review comparison rendered
  assert.match(passport.textContent, /kept 1, changed 1, added 1, removed 0/);

  // Stale candidate flagged explicitly
  const staleEntries = passport.querySelectorAll('.passport-entry[data-stale="true"]');
  assert.equal(staleEntries.length, 1);
  assert.match(staleEntries[0].textContent, /historical\/stale/);
  assert.match(staleEntries[0].textContent, /old-check/);
});

test("Gate Passport mounts in Run Evidence (web.js), Run Reviews (receipts.js), and Code Review CI & Previews (code-review.js)", async () => {
  const { ctx, doc, settle } = createHarness();
  await settle();

  // 1. Run Evidence
  const evParent = doc.createElement("div");
  ctx.window.KilnFactory.timeline.renderEvidence(evParent, {
    run_id: "run-ev-1",
    source: { provider_profile: "git-v1", repository: "onixresearch/kiln", symbolic_reference: "refs/heads/main", exact_revision: "a".repeat(40), canonical_tree: "b".repeat(64) },
    pipeline: { identity: "c".repeat(64), revision_id: "r1", nodes: [], edges: [] },
    execution: { attempt_id: "att-1", candidate_id: "cand-1", effect_id: "eff-1", observation: { sequence: 2, state: "Completed" } },
    handoff: { agent: "omp", outcome: "passed", candidate_id: "cand-1" },
    reports: [],
    artifacts: [],
  });
  assert.ok(evParent.querySelector(".causal-timeline"));
  assert.ok(evParent.querySelector(".gate-passport"));
  assert.match(evParent.textContent, /Unverified \/ Pending/);

  // 2. Run Reviews
  const revParent = doc.createElement("div");
  ctx.window.KilnReceipts.renderRun(revParent, {
    run_id: "run-ev-1",
    reports: [
      {
        id: "rep-1",
        kind: "review",
        name: "patchgate.json",
        node_id: "review",
        source_revision: "a".repeat(40),
        report: {
          kind: "review",
          evidence: {
            document: {
              schema: "kiln.patchgate.review.v1",
              observation: { isolated: true, report_identity: "d".repeat(64), reviews: [{ change: { native_id: "e".repeat(64) }, decision: "accepted" }] },
            },
          },
        },
      },
    ],
  }, {});
  assert.ok(revParent.querySelector(".gate-passport"));
  assert.match(revParent.textContent, /Observed Receipt/);

  // 3. Code Review CI & Previews
  const { body } = ctx.document;
  const crHost = doc.createElement("div");
  body.append(crHost);
  const commitHash = "f".repeat(40);
  const revId = "9".repeat(64);
  await ctx.window.KilnCodeReview.load(crHost, {
    principal: "alice",
    navigate: () => {},
    route: { review_change: "chg-1" },
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) {
        return { schema: "kiln.code-review.v1", principal: "alice", configured: true, can_write: false, changes: [], total: 0, policy: {} };
      }
      if (url === "/api/code-review/changes/chg-1") {
        return {
          change: {
            id: "chg-1",
            title: "Test change",
            state: "open",
            owner: "alice",
            project: "kiln",
            repository: "onixresearch/kiln",
            target_ref: "refs/heads/main",
            revisions: [{ id: revId, number: 1, commit: commitHash, base: "0".repeat(40), author: "alice", created_ms: 1700000000000 }],
            candidate: { id: "cand-cr-1", revision: revId, commit: commitHash, target: "0".repeat(40), dependencies: [] },
            threads: [],
            votes: [],
            advisory: [],
          },
          checklist: { ready: false, items: [] },
          ci_runs: [{ project: "kiln", repository: "onixresearch/kiln", revision: commitHash, run_id: "run-cr-1", success: true, full_run: true, clean: true, ordinal: 1, checks: [{ name: "unit-tests", kind: "test", outcome: "pass", artifact_digest: "5".repeat(64) }] }],
          handoff: { agent: "omp", outcome: "passed", candidate_id: "cand-cr-1" },
          operations: [],
        };
      }
      if (url.includes("/diff?")) {
        return { old_commit: "0".repeat(40), new_commit: commitHash, files: [], truncated: false };
      }
      return {};
    },
  });
  const ciPanel = crHost.querySelector("#cr-aside-panel-ci");
  assert.ok(ciPanel);
  const crPassport = ciPanel.querySelector(".gate-passport");
  assert.ok(crPassport);
  assert.match(crPassport.textContent, /unit-tests/);
  assert.match(crPassport.textContent, /Agent Claim/);
  assert.match(crPassport.textContent, /Unverified \/ Pending/);
  ctx.window.KilnCodeReview.clear();
});
