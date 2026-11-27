import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");
const settle = () => new Promise((resolve) => setImmediate(resolve));

const HASH_A = "a".repeat(64);
const HASH_B = "b".repeat(64);
const HASH_C = "c".repeat(64);

class FakeNode {
  constructor(nodeType, text = "") {
    this.nodeType = nodeType;
    this._text = String(text);
    this.children = [];
    this.parentNode = null;
    this.isConnected = false;
  }
  get textContent() { return this._text; }
  set textContent(v) { this._text = v == null ? "" : String(v); }
}

function setConnected(node, value) {
  node.isConnected = value;
  for (const child of node.children || []) setConnected(child, value);
}

class FakeElement extends FakeNode {
  constructor(tagName, doc = null) {
    super(1, "");
    this.tagName = String(tagName).toUpperCase();
    this.localName = String(tagName).toLowerCase();
    this.ownerDocument = doc;
    this.attributes = new Map();
    this.dataset = {};
    this.style = {};
    this.listeners = new Map();
    this.disabled = false;
    this.open = false;
    this.value = "";
    this.checked = false;
    this.required = false;
    this.tabIndex = 0;
    this.scrollCount = 0;
  }
  get className() { return this.attributes.get("class") || ""; }
  set className(v) { if (v) this.attributes.set("class", String(v)); else this.attributes.delete("class"); }
  get classList() {
    const owner = this;
    const tokens = () => String(owner.attributes.get("class") || "").split(/\s+/).filter(Boolean);
    return {
      add: (...ns) => { owner.className = [...new Set([...tokens(), ...ns.filter(Boolean)])].join(" "); },
      remove: (...ns) => { const d = new Set(ns); owner.className = tokens().filter((t) => !d.has(t)).join(" "); },
      contains: (n) => tokens().includes(n),
    };
  }
  get id() { return this.attributes.get("id") || ""; }
  set id(v) { if (v) this.attributes.set("id", String(v)); else this.attributes.delete("id"); }
  get hidden() { return this.attributes.has("hidden"); }
  set hidden(v) { if (v) this.attributes.set("hidden", ""); else this.attributes.delete("hidden"); }
  get textContent() { return this.children.length ? this.children.map((c) => c.textContent).join("") : this._text; }
  set textContent(v) { for (const c of this.children) setConnected(c, false); this.children = []; this._text = v == null ? "" : String(v); }
  get childElementCount() { return this.children.filter((c) => c.nodeType === 1).length; }
  get firstChild() { return this.children[0] || null; }
  get lastChild() { return this.children[this.children.length - 1] || null; }
  get parentElement() { return this.parentNode && this.parentNode.nodeType === 1 ? this.parentNode : null; }
  setAttribute(k, v) {
    const key = String(k), val = String(v);
    this.attributes.set(key, val);
    if (key.startsWith("data-")) {
      const camel = key.slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
      this.dataset[camel] = val;
    }
  }
  getAttribute(k) { return this.attributes.has(k) ? this.attributes.get(k) : null; }
  removeAttribute(k) { this.attributes.delete(k); }
  hasAttribute(k) { return this.attributes.has(k); }
  append(...nodes) {
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      setConnected(node, this.isConnected);
      this.children.push(node);
    }
  }
  prepend(...nodes) {
    const converted = [];
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      setConnected(node, this.isConnected);
      converted.push(node);
    }
    this.children.unshift(...converted);
  }
  replaceChildren(...nodes) {
    for (const c of this.children) { c.parentNode = null; setConnected(c, false); }
    this.children = [];
    this._text = "";
    this.append(...nodes);
  }
  replaceWith(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx < 0) return;
    const converted = nodes.map((item) => item instanceof FakeNode ? item : new FakeNode(3, String(item)));
    for (const n of converted) { n.parentNode = this.parentNode; setConnected(n, this.parentNode.isConnected); }
    this.parentNode.children.splice(idx, 1, ...converted);
    this.parentNode = null;
    setConnected(this, false);
  }
  remove() {
    if (!this.parentNode) return;
    this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
    this.parentNode = null;
    setConnected(this, false);
  }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  removeEventListener(type, fn) {
    if (!this.listeners.has(type)) return;
    this.listeners.set(type, this.listeners.get(type).filter((f) => f !== fn));
  }
  dispatchEvent(event) {
    const ev = typeof event === "string" ? { type: event, preventDefault() {} } : event;
    ev.target = ev.target || this;
    ev.currentTarget = this;
    for (const fn of [...(this.listeners.get(ev.type) || [])]) fn.call(this, ev);
  }
  click() {
    if (this.disabled) return;
    this.dispatchEvent({ type: "click", button: 0, preventDefault() {} });
  }
  focus() { if (this.ownerDocument) this.ownerDocument.activeElement = this; }
  scrollIntoView() { this.scrollCount += 1; }
  getBoundingClientRect() { return { top: 0, bottom: 0, left: 0, right: 0 }; }
  contains(other) {
    let cur = other;
    while (cur) { if (cur === this) return true; cur = cur.parentNode; }
    return false;
  }
  closest(sel) {
    let cur = this;
    while (cur) {
      if (cur.nodeType === 1 && matchesSimple(cur, sel)) return cur;
      cur = cur.parentNode;
    }
    return null;
  }
  querySelectorAll(sel) {
    const parts = sel.split(",").map((s) => s.trim()).filter(Boolean);
    const out = [];
    const walk = (n) => {
      for (const c of n.children || []) {
        if (c.nodeType === 1) {
          if (parts.some((p) => matchesSelector(c, p))) out.push(c);
          walk(c);
        }
      }
    };
    walk(this);
    return out;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}

function matchesSimple(el, sel) {
  if (!sel) return false;
  let rest = sel;
  const attrMatches = [];
  rest = rest.replace(/\[([^\]=]+)(?:="([^"]*)")?\]/g, (_, k, v) => { attrMatches.push([k, v]); return ""; });
  const classes = [];
  rest = rest.replace(/\.([a-zA-Z0-9_-]+)/g, (_, cls) => { classes.push(cls); return ""; });
  let idMatch = null;
  rest = rest.replace(/#([a-zA-Z0-9_-]+)/g, (_, id) => { idMatch = id; return ""; });
  const tag = rest.trim().toLowerCase();
  if (tag && el.localName !== tag) return false;
  if (idMatch !== null && el.id !== idMatch) return false;
  for (const cls of classes) if (!el.classList.contains(cls)) return false;
  for (const [k, v] of attrMatches) {
    if (v === undefined) { if (!el.hasAttribute(k)) return false; }
    else if (el.getAttribute(k) !== v) return false;
  }
  return true;
}

function matchesSelector(el, sel) {
  const chain = sel.split(/\s+/).filter(Boolean);
  if (chain.length === 1) return matchesSimple(el, chain[0]);
  if (!matchesSimple(el, chain[chain.length - 1])) return false;
  let cur = el.parentElement;
  for (let i = chain.length - 2; i >= 0; i -= 1) {
    while (cur && !matchesSimple(cur, chain[i])) cur = cur.parentElement;
    if (!cur) return false;
    cur = cur.parentElement;
  }
  return true;
}

const all = (root, pred) => {
  const out = [];
  const walk = (n) => {
    for (const c of n.children || []) {
      if (c.nodeType === 1) { if (pred(c)) out.push(c); walk(c); }
    }
  };
  walk(root);
  return out;
};

const textOf = (root) => {
  const parts = [];
  const walk = (n) => {
    if (n.nodeType === 3) parts.push(n._text);
    else if (n.nodeType === 1) {
      if (!n.children.length && n._text) parts.push(n._text);
      for (const c of n.children) walk(c);
    }
  };
  walk(root);
  return parts.join(" ");
};

function createDomEnv(modules, { url = "http://localhost:8080/?page=fleet" } = {}) {
  const doc = {
    visibilityState: "visible",
    activeElement: null,
    listeners: new Map(),
    createElement: (tag) => new FakeElement(tag, doc),
    getElementById: (id) => all(rootContainer, (n) => n.id === id)[0] || null,
    addEventListener: (t, fn) => {
      if (!doc.listeners.has(t)) doc.listeners.set(t, []);
      doc.listeners.get(t).push(fn);
    },
    removeEventListener: (t, fn) => {
      if (doc.listeners.has(t)) doc.listeners.set(t, doc.listeners.get(t).filter((f) => f !== fn));
    },
  };
  const rootContainer = new FakeElement("body", doc);
  rootContainer.isConnected = true;
  const root = new FakeElement("main", doc);
  rootContainer.append(root);

  const historyCalls = [];
  const parsed = new URL(url);
  const locationObj = {
    href: parsed.href,
    pathname: parsed.pathname,
    search: parsed.search,
    hash: parsed.hash,
  };
  const historyObj = {
    replaceState: (_state, _title, nextUrl) => {
      historyCalls.push(String(nextUrl));
      const next = new URL(String(nextUrl), parsed.origin);
      locationObj.href = next.href;
      locationObj.pathname = next.pathname;
      locationObj.search = next.search;
      locationObj.hash = next.hash;
    },
  };
  const winListeners = new Map();
  const win = {
    KilnUi: {
      timeElement: (ms) => { const t = doc.createElement("time"); t.textContent = String(ms); return t; },
      durationElement: (ms) => { const s = doc.createElement("span"); s.textContent = `${ms}ms`; return s; },
      statusPill: (o, p) => { const s = doc.createElement("span"); s.textContent = `${o}/${p}`; return s; },
      statusIcon: () => doc.createElement("span"),
      disclosureSummary: (label) => doc.createElement("summary", label),
      copyText: () => {},
      utf8Bytes: (v) => new TextEncoder().encode(String(v)).length,
      humanBytes: (b) => `${b} B`,
      outcomeStatus: (o, p) => ({ status: o === "Success" ? "success" : "unknown", label: `${o} ${p}`.trim() }),
    },
    KilnWork: { session: () => ({ principal: "local:operator" }) },
    KilnCairn: {
      lane: (k) => ({ label: String(k || "active"), tone: "info" }),
      condition: (c) => ({ label: String(c || "") }),
      indexRows: () => [],
      detailPath: () => "/api/cairn/detail",
      admitDetail: () => ({ units: [] }),
    },
    addEventListener: (t, fn) => {
      if (!winListeners.has(t)) winListeners.set(t, []);
      winListeners.get(t).push(fn);
    },
    removeEventListener: (t, fn) => {
      if (winListeners.has(t)) winListeners.set(t, winListeners.get(t).filter((f) => f !== fn));
    },
  };

  let fakeNow = 1_700_000_000_000;
  class FakeDate extends Date {
    constructor(...args) { super(...(args.length ? args : [fakeNow])); }
    static now() { return fakeNow; }
  }
  const timers = new Set();
  const context = {
    window: win,
    document: doc,
    location: locationObj,
    history: historyObj,
    URL,
    URLSearchParams,
    TextEncoder,
    TextDecoder,
    Node: FakeNode,
    DOMException,
    AbortController,
    crypto: webcrypto,
    Date: FakeDate,
    setTimeout: (fn, ms) => {
      const id = globalThis.setTimeout(fn, ms);
      timers.add(id);
      return id;
    },
    clearTimeout: (id) => { timers.delete(id); globalThis.clearTimeout(id); },
    console,
  };
  for (const name of modules) vm.runInNewContext(asset(name), context);
  const cleanup = () => { for (const id of timers) globalThis.clearTimeout(id); timers.clear(); };
  return {
    win, doc, root, rootContainer, locationObj, historyCalls, context,
    advanceNow: (ms) => { fakeNow += ms; },
    cleanup,
  };
}

function makeSnapshot(overrides = {}) {
  return {
    version: HASH_A,
    health: { api: "v1", machine: "britton-desktop", version: "0.1.0" },
    claims: {
      claims: [{
        identity: HASH_A,
        fence: 5,
        state: "open",
        scope: { target: "onixresearch/kiln", path: "crates/web" },
        holder: { tool: "omp", host: "aspen1", session: "s1" },
        purpose: "test",
        expires_ms: 1_700_000_500_000,
      }],
    },
    work: { ready_unclaimed: 0, ranked: [], blocked: [] },
    attempts: {
      attempts: [{
        attempt: HASH_B,
        work: HASH_C,
        fence: 3,
        state: "executing",
        phase: "running",
        repository: "onixresearch/kiln",
        title: "Fix UI",
        session: { machine: "aspen1", agent_name: "w1", state: "working" },
        lease_expires_ms: 1_700_000_600_000,
      }],
    },
    decisions: { decisions: [] },
    ...overrides,
  };
}

test("#1, #11, #12: Runbooks draft preserves unsaved edits on save failure, shows >48KB feedback, styles receipt <pre>, clears preview on mode switch, disables busy fanout controls, adds blank optional bool/enum, and shows loading state", async () => {
  const env = createDomEnv(["runbooks.js"]);
  const rb = env.win.KilnRunbooks;
  try {
    let failSave = true;
    let resolveFanout;
    let resolveDispatch;
    const rbPayload = {
      schema: "kiln.web-runbooks-view.v1",
      configured: true,
      principal: "local:operator",
      runbooks: [{
        id: "audit-rb",
        display_name: "Audit RB",
        mode: "read_only",
        can_dispatch: true,
        params: [
          { name: "opt_bool", kind: "bool", required: false },
          { name: "opt_enum", kind: "enum", required: false, enum_values: ["fast", "deep"] },
        ],
        allowed_targets: [{ repository: "onixresearch/kiln", project_id: "kiln", references: ["refs/heads/main"] }],
        fanout: { enabled: true, max_targets: 4, max_concurrency_limit: 4, max_budget_cap: 10 },
      }],
    };
    const ctx = {
      route: { page: "runbooks", runbook: "audit-rb" },
      canOperate: true,
      isCurrent: () => true,
      navigate: () => {},
      notice: () => {},
      confirmControl: async () => ({ confirmed: true }),
      api: async (path, opts = {}) => {
        if (path === "/api/runbooks") return rbPayload;
        if (path === "/api/runbooks/fanouts") return { fanouts: [{ id: "fo-1", runbook_id: "audit-rb", phase: "active", admission: "open", counts: {} }] };
        if (path === "/api/runbooks/dispatches?limit=20") return { dispatches: [] };
        if (path === "/api/runbooks/audit-rb/preview") return { mode: "read_only", approval_gate: "none", eligible: true };
        if (path === "/api/lattice/drafts" && !opts.method) return { workflows: ["wf-1"] };
        if (path === "/api/lattice/drafts" && opts.body?.action === "inspect") {
          return { status: "inspected", workflow: "wf-1", head: HASH_A, source: { entry: "wf.ncl", members: ["wf.ncl"] } };
        }
        if (path === "/api/lattice/drafts" && opts.body?.action === "view") {
          return { status: "viewed", workflow: "wf-1", head: HASH_A, draft_id: "d1", member: "wf.ncl", draft_revision: null, content: "original" };
        }
        if (path === "/api/lattice/drafts" && opts.body?.action === "save") {
          if (failSave) throw new Error("409 conflict");
          return { status: "saved", workflow: "wf-1", head: HASH_A, draft_id: "d1", member: "wf.ncl", draft_revision: HASH_B, receipt: { ok: true } };
        }
        if (path === "/api/runbooks/fanouts/fo-1") return new Promise((r) => { resolveFanout = r; });
        if (path === "/api/runbooks/dispatches/disp-1/rollup") return new Promise((r) => { resolveDispatch = r; });
        throw new Error(`unexpected ${path}`);
      },
    };
    await rb.load(env.root, ctx);
    await settle();

    // #12: Optional bool and enum without defaults have blank Select… option as default.
    const boolSelect = env.root.querySelector("#runbook-param-opt_bool");
    const enumSelect = env.root.querySelector("#runbook-param-opt_enum");
    assert.equal(boolSelect.value, "");
    assert.equal(boolSelect.children[0].value, "");
    assert.equal(enumSelect.value, "");
    assert.equal(enumSelect.children[0].value, "");

    // #12: Preview result box clears/hides when switching from Single target to Fan-out mode.
    const previewBtn = env.root.querySelector("#runbook-preview-btn");
    assert.ok(previewBtn);
    previewBtn.click();
    await settle();
    const previewBox = env.root.querySelector("#runbook-preview-result");
    assert.equal(previewBox.hidden, false);
    const fanoutModeBtn = env.root.querySelector("#runbook-mode-fanout");
    fanoutModeBtn.click();
    assert.equal(previewBox.hidden, true);

    // #1 & #12 & #11: Draft editor preserves edits on save failure, shows >48KB warning, and styles receipt <pre>.
    const draftDetails = all(env.root, (n) => n.localName === "details" && textOf(n).includes("Edit Lattice workflow drafts"))[0];
    draftDetails.open = true;
    draftDetails.dispatchEvent({ type: "toggle" });
    await settle();
    const wfSelect = all(draftDetails, (n) => n.localName === "select")[0];
    wfSelect.value = "wf-1";
    all(draftDetails, (n) => n.localName === "button" && n.textContent === "Read current workflow head")[0].click();
    await settle();
    const draftIdInput = all(draftDetails, (n) => n.localName === "input")[0];
    draftIdInput.value = "d1";
    const memberSelect = all(draftDetails, (n) => n.localName === "select")[1];
    memberSelect.value = "wf.ncl";
    all(draftDetails, (n) => n.localName === "button" && n.textContent === "Open draft member")[0].click();
    await settle();
    const textarea = all(draftDetails, (n) => n.localName === "textarea")[0];
    const saveBtn = all(draftDetails, (n) => n.localName === "button" && n.textContent === "Validate and save draft")[0];

    // >48KB explicit size feedback.
    textarea.value = "x".repeat(48 * 1024 + 5);
    textarea.dispatchEvent({ type: "input" });
    assert.equal(saveBtn.disabled, true);
    assert.match(textOf(draftDetails), /exceeds the 48 KB limit/);

    // Valid edit, save fails -> textarea MUST stay connected with edits intact and save re-enabled!
    textarea.value = "edited unsaved content";
    textarea.dispatchEvent({ type: "input" });
    assert.equal(saveBtn.disabled, false);
    saveBtn.click();
    await settle();
    assert.equal(textarea.isConnected, true);
    assert.equal(textarea.disabled, false);
    assert.equal(textarea.value, "edited unsaved content");
    assert.equal(saveBtn.disabled, false);

    // Retry save succeeds -> receipt <pre> has .identity class.
    failSave = false;
    saveBtn.click();
    await settle();
    const receiptPre = all(draftDetails, (n) => n.localName === "pre")[0];
    assert.ok(receiptPre);
    assert.equal(receiptPre.classList.contains("identity"), true);

    // #12: Selecting a fanout shows loading state immediately while API is pending.
    const loadPromise = rb.load(env.root, { ...ctx, route: { page: "runbooks", runbook: "audit-rb", fanout: "fo-1" } });
    await settle();
    assert.match(textOf(env.root), /Loading fan-out details/);
    resolveFanout({
      id: "fo-1", runbook_id: "audit-rb", phase: "active", admission: "open",
      controls: { pause: true, cancel: true },
      counts: {},
      children: [{ target: "onixresearch/kiln@main", source_revision: HASH_A, status: "active", run_id: "r1" }],
    });
    await loadPromise;
    // #2: fanout children cells carry data-label attributes.
    const foCells = all(env.root.querySelector("#fanout-children-table"), (n) => n.localName === "td");
    assert.deepEqual(foCells.map((c) => c.dataset.label), ["Target", "Revision", "Status", "Reason / Scope", "Child Run"]);
    // #12: Selecting a dispatch shows loading state immediately while API is pending.
    const dispatchPromise = rb.load(env.root, { ...ctx, route: { page: "runbooks", runbook: "audit-rb", dispatch: "disp-1" } });
    await settle();
    assert.match(textOf(env.root), /Loading dispatch roll-up/);
    resolveDispatch({ subject_kind: "dispatch", subject_id: "disp-1", runbook_id: "audit-rb", mode: "read_only", overall_outcome: "Success", phase: "completed", counts: {}, targets: [] });
    await dispatchPromise;
  } finally {
    rb.clear();
    env.cleanup();
  }
});

test("#8 & #12: Single-target dispatch preserves generated request_id on retryable 503 and fanout sendControl disables buttons while busy", async () => {
  const env = createDomEnv(["runbooks.js"]);
  const rb = env.win.KilnRunbooks;
  try {
    let resolveControl;
    const rbPayload = {
      schema: "kiln.web-runbooks-view.v1",
      configured: true,
      principal: "local:operator",
      runbooks: [{
        id: "audit-rb",
        display_name: "Audit RB",
        mode: "read_only",
        can_dispatch: true,
        params: [],
        allowed_targets: [{ repository: "onixresearch/kiln", project_id: "kiln", references: ["refs/heads/main"] }],
      }],
    };
    const ctx = {
      route: { page: "runbooks", runbook: "audit-rb" },
      canOperate: true,
      isCurrent: () => true,
      navigate: () => {},
      notice: () => {},
      api: async (path, opts = {}) => {
        if (path === "/api/runbooks") return rbPayload;
        if (path === "/api/runbooks/fanouts") return { fanouts: [] };
        if (path === "/api/runbooks/dispatches?limit=20") return { dispatches: [] };
        if (path === "/api/runbooks/audit-rb/dispatch") {
          throw Object.assign(new Error("Service Unavailable"), { status: 503 });
        }
        if (path === "/api/runbooks/fanouts/fo-1") {
          return {
            id: "fo-1", runbook_id: "audit-rb", phase: "active", admission: "open",
            controls: { pause: true, cancel: true }, counts: {}, children: [],
          };
        }
        if (path === "/api/runbooks/fanouts/fo-1/rollup") {
          return {
            targets: [{ target: "onixresearch/kiln@main", project_id: "kiln", source_revision: HASH_A, status: "success", patchgate_receipt: { state: "unrun" }, verification_receipt: { state: "unrun" }, pull_requests: [], run_id: "r1" }],
          };
        }
        if (path === "/api/runbooks/fanouts/fo-1/pause" && opts.method === "POST") {
          return new Promise((r) => { resolveControl = r; });
        }
        throw new Error(`unexpected ${path}`);
      },
    };
    await rb.load(env.root, ctx);
    const reqInput = env.root.querySelector("#runbook-request-id");
    assert.equal(reqInput.value, "");
    env.root.querySelector("#runbook-dispatch-form").dispatchEvent({ type: "submit", preventDefault() {} });
    await settle();
    assert.match(reqInput.value, /^ui-rb-/);
    assert.match(env.root.querySelector("#runbook-dispatch-error").textContent, new RegExp(reqInput.value));

    // Fan-out control buttons disabled while sendControl is busy, and rollup table cells carry data-label.
    await rb.load(env.root, { ...ctx, route: { page: "runbooks", runbook: "audit-rb", fanout: "fo-1" } });
    const rollupCells = all(env.root.querySelector("#rollup-targets-table"), (n) => n.localName === "td");
    assert.deepEqual(rollupCells.map((c) => c.dataset.label), ["Target", "Exact Revision", "Outcome", "Patchgate Receipt", "Verification Receipt", "PR / Stack Links", "Run"]);
    const pauseBtn = env.root.querySelector("#fanout-pause-btn");
    const cancelBtn = env.root.querySelector("#fanout-cancel-btn");
    const confirmBox = env.root.querySelector("#fanout-cancel-confirm");
    pauseBtn.click();
    assert.equal(pauseBtn.disabled, true);
    confirmBox.checked = true;
    confirmBox.dispatchEvent({ type: "change" });
    assert.equal(cancelBtn.disabled, true, "cancel remains disabled while control request is busy");
    resolveControl({ replayed: false });
    await settle();
    assert.equal(env.root.querySelector("#fanout-pause-btn").disabled, false);
  } finally {
    rb.clear();
    env.cleanup();
  }
});

test("#3, #4, #5, #7, #9, #10, #13, #14: Fleet fixes — campaign replaceState, protected bulk/dispatch mutations, swarm race & slider, runtime poll stability, guest move labels/refresh, confirmation target guard, queued/split placement, Herdr viewer retry & scroll", async () => {
  const env = createDomEnv(["inventory.js", "fleet-runtime.js", "fleet.js"]);
  const fleet = env.win.KilnFleet;
  const navigated = [];
  let failBulk = false;
  let ansiCalls = 0;
  let ansiFail = false;
  let inventoryCalls = 0;
  let guestOverviewCount = 0;
  let resolveGuestOverview1;
  let resolveSwarmA;
  let swarmDetailCalls = 0;
  const confirmCalls = [];
  let confirmHook = null;

  const ctx = {
    route: { page: "fleet", worker_campaign: "" },
    isCurrent: () => true,
    canOperate: () => true,
    navigate: (p) => navigated.push(p),
    subscribeEvents: null,
    stream: () => new Promise(() => {}),
    confirmControl: async (opts) => {
      confirmCalls.push(opts);
      if (confirmHook) confirmHook();
      return { confirmed: true, reason: "operator verified" };
    },
    api: async (path, opts = {}) => {
      if (path === "/api/fleet/machines") {
        return {
          machines: [
            { machine: "aspen1", placement_configured: true, state: "reporting", observed_at_ms: null, last_seen_at_ms: null, sessions: 1, herdr_windows_configured: true },
          ],
        };
      }
      if (path === "/api/fleet/machines/aspen1/windows") {
        return { machine: "aspen1", windows: [{ pane_id: "w1:p1", workspace_id: "ws", tab_id: "t1" }] };
      }
      if (path.includes("/capture") && path.includes("format=ansi")) {
        ansiCalls += 1;
        if (ansiFail) throw new Error("transient capture error");
        return { machine: "aspen1", window: { pane_id: "w1:p1", workspace_id: "ws", tab_id: "t1" }, format: "ansi", source: "visible", requested_lines: 1000, line_count: 1, truncated: false, ansi: "ok", columns: 80, rows: 24 };
      }
      if (path.includes("/capture")) {
        return { machine: "aspen1", window: { pane_id: "w1:p1", workspace_id: "ws", tab_id: "t1" }, requested_lines: 200, line_count: 1, truncated: false, text: "plain line" };
      }
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      if (path === "/api/inventory") {
        inventoryCalls += 1;
        return {
          schema: "kiln.observe.inventory.v2", identity: HASH_A, evaluated_at_ms: 100,
          sources: [], facts: [], attempt_explanations: [],
          snapshot: { schema: "kiln.observe.inventory.v2", synthetic: false, workers: [], compute: [], models: [], admissions: [], pools: [], agents: [], policies: [] },
        };
      }
      if (path === "/api/implementation/campaigns" && !path.includes("?")) {
        return { schema: "kiln.implementation-campaigns.v1", campaigns: [{ campaign_id: "camp-1", status: "active", pending_decisions: 0 }], bounded: true, truncated: false };
      }
      if (path === "/api/implementation/campaigns?campaign=camp-1") {
        return {
          schema: "kiln.campaign.v2", identity: HASH_A, campaign_id: "camp-1", campaign_identity: HASH_B,
          selection_identity: HASH_C, selection_revision: 1, store_version: 2, repository: "onixresearch/kiln",
          repository_identity: HASH_A, status: "active", admission: "open", blocked: false,
          members: [{ member: "m1", change: "c1", status: "active", dependencies: [], blockers: [] }],
          non_claims: [],
        };
      }
      if (path === "/api/implementation/campaign-definitions") return { schema: "kiln.campaign-definitions.v1", definitions: [] };
      if (path === "/api/implementation/campaigns/inbox?campaign=camp-1") {
        return {
          schema: "kiln.campaign-inbox.v1", campaign_id: "camp-1", campaign_identity: HASH_B, store_version: 2,
          admission: "open", operations: ["pause"], decisions: [], non_claims: [],
        };
      }
      if (path === "/api/fleet/swarms") {
        return {
          swarms: [
            { identity: HASH_A, config: { title: "Swarm A" }, phase: "running", members: {}, board: { posts: [{ seq: 1, sender: "c", sender_role: "coord", kind: "post", mentions: [], body: "hi" }, { seq: 2, sender: "w", sender_role: "worker", kind: "post", mentions: [], body: "done" }] }, budget: { drawn_units: 1, ceiling_units: 10 } },
            { identity: HASH_B, config: { title: "Swarm B" }, phase: "running", members: {}, board: { posts: [{ seq: 1, sender: "c", sender_role: "coord", kind: "post", mentions: [], body: "b" }] }, budget: { drawn_units: 1, ceiling_units: 10 } },
          ],
        };
      }
      if (path === `/api/fleet/swarms/${HASH_A}`) {
        swarmDetailCalls += 1;
        if (swarmDetailCalls === 1) return new Promise((r) => { resolveSwarmA = r; });
        return { swarm: { identity: HASH_A, config: { title: "Swarm A" }, phase: "running", members: {}, board: { posts: [{ seq: 1, sender: "c", sender_role: "coord", kind: "post", mentions: [], body: "hi" }, { seq: 2, sender: "w", sender_role: "worker", kind: "post", mentions: [], body: "done" }] } } };
      }
      if (path === `/api/fleet/swarms/${HASH_A}/graph`) return { graph: { head_seq: 2, up_to_seq: 2, edges: [] } };
      if (path === `/api/fleet/swarms/${HASH_A}/graph?up_to_seq=1`) return { graph: { head_seq: 2, up_to_seq: 1, edges: [] } };
      if (path === `/api/fleet/swarms/${HASH_B}`) {
        return { swarm: { identity: HASH_B, config: { title: "Swarm B" }, phase: "running", members: {}, board: { posts: [{ seq: 1, sender: "c", sender_role: "coord", kind: "post", mentions: [], body: "b" }] } } };
      }
      if (path === `/api/fleet/swarms/${HASH_B}/graph`) return { graph: { head_seq: 1, up_to_seq: 1, edges: [] } };
      if (path === "/api/fleet/placement/capabilities") return { move: true, split: true };
      if (path === "/api/fleet/placement/guest-move") {
        guestOverviewCount += 1;
        const payload = {
          available: true,
          prerequisites: [],
          ordered_steps: ["pause", "checkpoint", "transfer", "restore", "reconnect", "retire_source"],
          moves: [],
        };
        if (guestOverviewCount === 2) return new Promise((r) => { resolveGuestOverview1 = r; });
        return payload;
      }
      if (path.endsWith("/renew") && opts.method === "POST") {
        if (failBulk) throw new Error("Coordinator 409 conflict");
        return { ok: true };
      }
      return {};
    },
  };
  env.win.KilnHerdrViewer = {
    load: async () => ({
      mount: () => ({ render: () => false, dispose: () => {} }),
    }),
  };

  try {
    // Load Fleet with subscribeEvents so we can push snapshots directly.
    let listener = null;
    fleet.load(env.root, { ...ctx, subscribeEvents: (fn) => { listener = fn; return () => {}; } });
    await settle();
    listener("fleet", makeSnapshot());
    await settle();

    // #3: Selecting a campaign uses history.replaceState, NOT context.navigate.
    const campSelect = env.root.querySelector("#fleet-campaign-select");
    assert.ok(campSelect);
    campSelect.value = "camp-1";
    campSelect.dispatchEvent({ type: "change" });
    await settle();
    assert.equal(navigated.length, 0);
    assert.match(env.locationObj.search, /worker_campaign=camp-1/);

    // #7: Trigger Cairn SSE poll -> runtime should NOT re-fetch when attempts haven't changed, and <details> stays open.
    const runtimeDetails = env.root.querySelector(".fleet-runtime details.inventory-disclosure");
    assert.ok(runtimeDetails);
    runtimeDetails.open = true;
    const invCountBefore = inventoryCalls;
    listener("cairn", {});
    await settle();
    assert.equal(inventoryCalls, invCountBefore);
    assert.equal(runtimeDetails.open, true);

    // #10: Unrelated snapshot version bump while modal is open does NOT cancel Renew.
    const renewBtn = all(env.root, (n) => n.localName === "button" && n.textContent === "Renew")[0];
    confirmHook = () => {
      listener("fleet", makeSnapshot({ version: HASH_B })); // unrelated version bump
    };
    renewBtn.click();
    await settle();
    assert.doesNotMatch(textOf(env.root), /became stale or changed/);
    confirmHook = null;

    // #4: Bulk renew error surfaces in state.command and re-enables button.
    const claimCheckbox = all(env.root, (n) => n.localName === "input" && n.getAttribute("aria-label")?.startsWith("Select claim"))[0];
    claimCheckbox.checked = true;
    claimCheckbox.dispatchEvent({ type: "change" });
    const bulkRenew = all(env.root, (n) => n.localName === "button" && n.textContent === "Renew selected (+30m)")[0];
    failBulk = true;
    bulkRenew.click();
    await settle();
    assert.match(textOf(env.root), /Coordinator 409 conflict/);
    assert.equal(bulkRenew.disabled, false);
    failBulk = false;

    // #5: Swarm inspection race guard, htmlFor, and replay slider preserved across SSE ticks.
    const inspectBtns = all(env.root, (n) => n.localName === "button" && n.textContent === "Inspect");
    const swarmInspectA = inspectBtns[inspectBtns.length - 2];
    const swarmInspectB = inspectBtns[inspectBtns.length - 1];
    swarmInspectA.click(); // starts slow Swarm A fetch
    swarmInspectB.click(); // switches to fast Swarm B
    await settle();
    assert.match(textOf(env.root), /Swarm B/);
    resolveSwarmA({ swarm: { identity: HASH_A, config: { title: "Swarm A" }, phase: "running", members: {}, board: { posts: [] } } });
    await settle();
    assert.doesNotMatch(textOf(env.root), /Swarm A · running/, "stale Swarm A response must not overwrite Swarm B");

    // Inspect Swarm A cleanly, move slider to seq 1, then send SSE frame -> slider stays at 1!
    swarmInspectA.click();
    await settle();
    const slider = env.root.querySelector(`#fleet-swarm-replay-${HASH_A}`);
    const sliderLabel = all(env.root, (n) => n.localName === "label" && n.htmlFor === slider.id)[0];
    assert.ok(sliderLabel, "slider label has htmlFor matching slider id");
    slider.value = "1";
    slider.dispatchEvent({ type: "change" });
    await settle();
    listener("fleet", makeSnapshot({ version: HASH_C }));
    await settle();
    assert.equal(env.root.querySelector(`#fleet-swarm-replay-${HASH_A}`).value, "1");

    // #9: Guest move inputs wrapped in <label>, refresh disables button & preserves in-progress inputs.
    const guestSection = env.root.querySelector("#fleet-guest-move");
    const guestLabels = all(guestSection, (n) => n.localName === "label");
    assert.equal(guestLabels.length, 5);
    const attemptInput = all(guestSection, (n) => n.localName === "input")[0];
    attemptInput.value = HASH_A;
    const guestRefreshBtn = all(guestSection, (n) => n.localName === "button" && n.textContent === "Refresh guest move status")[0];
    guestRefreshBtn.click();
    assert.equal(guestRefreshBtn.disabled, true);
    resolveGuestOverview1({ available: true, prerequisites: [], ordered_steps: [], moves: [] });
    await settle();
    assert.equal(guestRefreshBtn.disabled, false);
    assert.equal(all(guestSection, (n) => n.localName === "input")[0].value, HASH_A);

    // #13: Placement split shows empty/unavailable message when <2 hosts instead of empty selects.
    const splitBtn = all(env.root, (n) => n.localName === "button" && n.textContent === "Place independent work units")[0];
    splitBtn.click();
    await settle();
    assert.match(textOf(env.root), /At least two reporting placement-configured hosts/);
    assert.equal(all(env.root, (n) => n.localName === "button" && n.textContent === "Review split").length, 0);

    // #14: Herdr windows scrolls into view on open & viewer shows Retry/Plain text on transient error.
    const browseBtn = all(env.root, (n) => n.localName === "button" && n.textContent === "Browse Herdr windows")[0];
    browseBtn.click();
    await settle();
    const herdrHeading = all(env.root.querySelector("#fleet-machines"), (n) => n.localName === "h3" && n.textContent.includes("Herdr windows"))[0];
    assert.equal(herdrHeading.parentNode.scrollCount > 0, true);
    ansiFail = true;
    const openViewerBtn = all(env.root, (n) => n.localName === "button" && n.textContent === "Open viewer")[0];
    openViewerBtn.click();
    await settle();
    const retryViewer = all(env.root, (n) => n.localName === "button" && n.textContent === "Retry viewer")[0];
    const plainBtn = all(env.root, (n) => n.localName === "button" && n.textContent === "Plain text")[0];
    assert.equal(retryViewer.hidden, false);
    assert.equal(plainBtn.hidden, false);
    plainBtn.click();
    await settle();
    const capturePre = all(env.root, (n) => n.localName === "pre")[0];
    assert.equal(capturePre.classList.contains("identity"), true);
  } finally {
    fleet.clear();
    env.cleanup();
  }
});

test("#6 & #16: Workers preserves campaign definitions & active inputs on CI SSE redraw, and puts Workers unavailable above Observed inventories", async () => {
  const env = createDomEnv(["inventory.js", "workers.js"]);
  const workers = env.win.KilnWorkers;
  const inventory = env.win.KilnInventory;
  let workersPayload = {
    schema: "kiln.web-workers.v1", bounded: true, storage_failed: false,
    summary: { targets: 1, configured_slots: 1, enrolled_slots: 1, active: 0, free_slots: 1, queued: 0 },
    workers: [{ id: "w1", provider_profile_id: "p1", platform: "linux", status: "slots_available", liveness: "online", capacity: { configured_slots: 1, enrolled_slots: 1, active: 0, free_slots: 1, queued: 0 }, projects: [] }],
  };
  let failWorkers = false;
  const ctx = {
    route: { page: "workers", worker_campaign: "" },
    isCurrent: () => true,
    navigate: () => {},
    runLink: () => env.doc.createElement("a"),
    projectLink: () => env.doc.createElement("a"),
    api: async (path) => {
      if (path === "/api/workers") {
        if (failWorkers) throw new Error("workers down");
        return workersPayload;
      }
      if (path === "/api/inventory") throw new Error("inv");
      if (path === "/api/implementation/campaigns") return { schema: "kiln.implementation-campaigns.v1", campaigns: [], bounded: true, truncated: false };
      if (path === "/api/implementation/campaign-definitions") return { schema: "kiln.campaign-definitions.v1", definitions: [] };
      throw new Error(`unexpected ${path}`);
    },
  };
  try {
    await workers.load(env.root, ctx);
    await settle();
    const defs = env.root.querySelector("details.workers-campaign-definitions");
    defs.open = true;
    defs.dispatchEvent({ type: "toggle" });
    await settle();
    // Simulate CI SSE update changing workers count -> defs MUST remain open!
    workersPayload = { ...workersPayload, summary: { ...workersPayload.summary, queued: 2 } };
    await workers.load(env.root, ctx);
    await settle();
    assert.equal(env.root.querySelector("details.workers-campaign-definitions").open, true);

    // #16: Workers failure puts Workers unavailable first, collapsed disclosure second.
    failWorkers = true;
    await workers.load(env.root, ctx);
    await settle();
    assert.equal(env.root.children[0].classList.contains("workers-unavailable"), true);
    assert.equal(env.root.children[1].localName, "details");
    assert.equal(env.root.children[1].open, false);

    // #16: loadCampaignIndex generation guard ignores stale response.
    const holder = env.doc.createElement("div");
    env.root.append(holder);
    let resolveFirst;
    const raceCtx = {
      isCurrent: () => true,
      api: async () => {
        if (!resolveFirst) return new Promise((r) => { resolveFirst = r; });
        return { schema: "kiln.implementation-campaigns.v1", campaigns: [{ campaign_id: "fresh", status: "active", pending_decisions: 0 }], bounded: true, truncated: false };
      },
    };
    const p1 = inventory.loadCampaignIndex(holder, raceCtx);
    const p2 = inventory.loadCampaignIndex(holder, raceCtx);
    await p2;
    resolveFirst({ schema: "kiln.implementation-campaigns.v1", campaigns: [{ campaign_id: "stale", status: "active", pending_decisions: 0 }], bounded: true, truncated: false });
    await p1;
    assert.match(textOf(holder), /fresh/);
    assert.doesNotMatch(textOf(holder), /stale/);
  } finally {
    workers.clear();
    env.cleanup();
  }
});

test("#15 & #16: Fair-admission preserves success notice after refresh and Inventory cancel stays disabled while command inFlight", async () => {
  const env = createDomEnv(["fleet-capacity.js", "inventory.js"]);
  const fc = env.win.KilnFleetCapacity;
  const inv = env.win.KilnInventory;
  try {
    const admissionPayload = {
      schema: "kiln.implementation-admission.v1", configured: true, bounded: true,
      policy: { identity: HASH_A, version: 1, accounting_basis: "tokens", strategy: "drf", bounds: { max_items: 1, max_bytes: 1, max_active: 1, max_children: 1 }, shares: { "tenant-a": 10 } },
      accounts: [], serving: [], attempt_metrics: [],
    };
    await fc.mount(env.root, {
      isCurrent: () => true,
      canOperate: () => true,
      isFresh: () => true,
      confirmControl: async () => ({ confirmed: true, requestId: "req-1" }),
      api: async (path, opts = {}) => {
        if (path === "/api/implementation/cairn") return { changes: [] };
        if (path === "/api/implementation/admission" && !opts.method) return admissionPayload;
        if (path === "/api/implementation/admission/policy") return { result: "updated", policy_identity: HASH_B };
        return {};
      },
    });
    await settle();
    const form = all(env.root, (n) => n.localName === "form")[0];
    form.dispatchEvent({ type: "submit", preventDefault() {} });
    await settle();
    assert.match(textOf(env.root), /Fair-admission policy updated/);

    // #16: Inventory cancel checkbox stays disabled while another command is inFlight.
    let resolveCmd;
    const runnerPanel = {};
    const uiObj = inv.runnerUi(runnerPanel, () => {}, () => {});
    const runner = inv.commandRunner({ api: async () => new Promise((r) => { resolveCmd = r; }) }, uiObj);
    runnerPanel.current = inv.renderControls({
      schema: "kiln.campaign-inbox.v1", campaign_id: "c1", campaign_identity: HASH_A, store_version: 1,
      admission: "open", operations: ["pause", "cancel"], decisions: [], non_claims: [],
    }, runner.start);
    const pauseBtn = all(runnerPanel.current, (n) => n.localName === "button" && n.textContent === "Pause successor admission")[0];
    const cancelBtn = all(runnerPanel.current, (n) => n.localName === "button" && n.textContent === "Cancel campaign")[0];
    const cancelBox = all(runnerPanel.current, (n) => n.localName === "input" && n.type === "checkbox")[0];
    pauseBtn.click();
    cancelBox.checked = true;
    cancelBox.dispatchEvent({ type: "change" });
    assert.equal(cancelBtn.disabled, true);
    resolveCmd({ replayed: false, command: "pause", store_version: 2, admission: "paused", request_id: "r1" });
    await settle();
    assert.equal(cancelBtn.disabled, false);
  } finally {
    fc.clear();
    inv.clear();
    env.cleanup();
  }
});

test("#17: Receipts mobile back-link clears debounce timer and preserves pending q and decision filters", async () => {
  const env = createDomEnv(["receipts.js"]);
  const receipts = env.win.KilnReceipts;
  const navigated = [];
  const ctx = {
    route: { page: "reviews", receipt: "rec-1", receipt_q: "", receipt_decision: "" },
    navigate: (p) => navigated.push(p),
    api: async (path) => {
      if (path === "/api/reviews") {
        return { configured: true, label: "reviews", non_claims: [], receipts: [{ name: "rec-1", message: "m", changes: 1, tally: {} }] };
      }
      if (path === "/api/reviews/rec-1") {
        return { name: "rec-1", blake3: HASH_A, bytes: 10, document: { schema: "patchgate.v1", changes: [] } };
      }
      throw new Error(`unexpected ${path}`);
    },
  };
  try {
    await receipts.load(env.root, ctx);
    const search = env.root.querySelector("#receipts-q");
    const decision = env.root.querySelector("#receipts-decision");
    search.value = "m";
    search.dispatchEvent({ type: "input" });
    decision.value = "accepted";
    decision.dispatchEvent({ type: "change" });
    const back = env.root.querySelector(".receipts-back");
    back.click();
    assert.deepEqual(JSON.parse(JSON.stringify(navigated)), [{ receipt: "", receipt_q: "m", receipt_decision: "accepted" }]);
  } finally {
    receipts.clear();
    env.cleanup();
  }
});

test("#2, #11, #18: CSS stacking labels, bounded <pre>, and polish rules are present without dead rules or undefined vars", () => {
  const fleetCss = asset("fleet.css");
  const fcCss = asset("fleet-capacity.css");
  const invCss = asset("inventory.css");
  const recCss = asset("receipts.css");

  assert.doesNotMatch(fleetCss, /--surface-subtle|--surface\b|\.fleet-columns/);
  assert.match(fleetCss, /\.fleet-dispatch-bar[\s\S]*background:\s*var\(--secondary\)/);
  assert.match(fleetCss, /\.fleet-attempt-inspection\s*\{[^}]*min-width:\s*0/);
  assert.match(fleetCss, /@container data-table \(max-width:\s*48rem\)/);
  assert.match(fleetCss, /#rollup-targets-table td\[data-label\]/);
  assert.match(fleetCss, /#fanout-children-table td\[data-label\]/);

  assert.match(fcCss, /@container data-table \(max-width:\s*48rem\)[\s\S]*\.fleet-capacity-table td\[data-label\]/);
  assert.match(invCss, /@container data-table \(max-width:\s*48rem\)[\s\S]*\.inventory-table td\[data-label\]/);
  assert.match(recCss, /\.receipt-tally-cell\s*\{[^}]*border-bottom:\s*1px solid var\(--border\)/);
});
