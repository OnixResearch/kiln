import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class FakeClassList {
  constructor(owner) {
    this.owner = owner;
  }
  _tokens() {
    return String(this.owner.attributes.get("class") || "").split(/\s+/).filter(Boolean);
  }
  add(...names) {
    const next = new Set(this._tokens());
    for (const name of names) if (name) next.add(name);
    this.owner.attributes.set("class", [...next].join(" "));
  }
  remove(...names) {
    const drop = new Set(names);
    const next = this._tokens().filter((t) => !drop.has(t));
    if (next.length) this.owner.attributes.set("class", next.join(" "));
    else this.owner.attributes.delete("class");
  }
  contains(name) {
    return this._tokens().includes(name);
  }
  *[Symbol.iterator]() {
    yield* this._tokens();
  }
}

class FakeNode {
  constructor(nodeType, text = "") {
    this.nodeType = nodeType;
    this._text = String(text);
    this.children = [];
    this.parentNode = null;
    this.isConnected = false;
  }
  get textContent() {
    return this._text;
  }
  set textContent(value) {
    this._text = value == null ? "" : String(value);
  }
}

class FakeElement extends FakeNode {
  constructor(tagName, namespaceURI = "http://www.w3.org/1999/xhtml", doc = null) {
    super(1, "");
    this.tagName = String(tagName).toUpperCase();
    this.localName = String(tagName).toLowerCase();
    this.namespaceURI = namespaceURI;
    this.ownerDocument = doc;
    this.attributes = new Map();
    this.dataset = {};
    this.style = {};
    this.listeners = new Map();
    this.classList = new FakeClassList(this);
    this.disabled = false;
    this.open = false;
    this.value = "";
    this.checked = false;
    this.required = false;
    this.tabIndex = 0;
  }
  get className() {
    return this.attributes.get("class") || "";
  }
  set className(value) {
    if (value) this.attributes.set("class", String(value));
    else this.attributes.delete("class");
  }
  get id() {
    return this.attributes.get("id") || "";
  }
  set id(value) {
    if (value) this.attributes.set("id", String(value));
    else this.attributes.delete("id");
  }
  get hidden() {
    return this.attributes.has("hidden");
  }
  set hidden(value) {
    if (value) this.attributes.set("hidden", "");
    else this.attributes.delete("hidden");
  }
  get textContent() {
    if (!this.children.length) return this._text;
    return this.children.map((c) => c.textContent).join("");
  }
  set textContent(value) {
    for (const c of this.children) setConnected(c, false);
    this.children = [];
    this._text = value == null ? "" : String(value);
  }
  setAttribute(key, value) {
    const k = String(key);
    const v = String(value);
    this.attributes.set(k, v);
    if (k.startsWith("data-")) {
      const camel = k.slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
      this.dataset[camel] = v;
    }
  }
  getAttribute(key) {
    return this.attributes.has(key) ? this.attributes.get(key) : null;
  }
  hasAttribute(key) {
    return this.attributes.has(key);
  }
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
  replaceChildren(...nodes) {
    for (const c of this.children) {
      c.parentNode = null;
      setConnected(c, false);
    }
    this.children = [];
    this._text = "";
    this.append(...nodes);
  }
  replaceWith(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = nodes.map((n) => (n instanceof FakeNode ? n : new FakeNode(3, String(n))));
    for (const n of converted) {
      n.parentNode = this.parentNode;
      setConnected(n, this.parentNode.isConnected);
    }
    this.parentNode.children.splice(idx, 1, ...converted);
    this.parentNode = null;
    setConnected(this, false);
  }
  showModal() {
    this.open = true;
  }
  close() {
    this.open = false;
  }
  reset() {
    for (const input of this.querySelectorAll("input")) input.value = "";
  }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  removeEventListener(type, fn) {
    const arr = this.listeners.get(type);
    if (arr) this.listeners.set(type, arr.filter((f) => f !== fn));
  }
  dispatchEvent(event) {
    for (const fn of [...(this.listeners.get(event.type) || [])]) fn(event);
  }
  click() {
    this.dispatchEvent({
      type: "click",
      button: 0,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      altKey: false,
      target: this,
      preventDefault() {},
    });
  }
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }
  querySelectorAll(selector) {
    const out = [];
    const walk = (node) => {
      for (const child of node.children || []) {
        if (child.nodeType === 1) {
          if (matchesSimple(child, selector)) out.push(child);
          walk(child);
        }
      }
    };
    walk(this);
    return out;
  }
}

function setConnected(node, connected) {
  node.isConnected = connected;
  for (const child of node.children || []) setConnected(child, connected);
}

function matchesSimple(el, selector) {
  if (selector === "span[data-icon]") return el.localName === "span" && el.getAttribute("data-icon") !== null;
  if (selector.startsWith(".")) return el.classList.contains(selector.slice(1));
  if (selector.startsWith("#")) return el.id === selector.slice(1);
  return el.localName === selector.toLowerCase();
}

function allText(node) {
  if (!node) return "";
  if (!node.children?.length) return node.textContent || "";
  return node.children.map(allText).join(" ");
}

function descendants(node) {
  const out = [];
  for (const child of node.children || []) {
    if (child.nodeType === 1) {
      out.push(child);
      out.push(...descendants(child));
    }
  }
  return out;
}

function setup({ apiHandler, canOperate = true, withSubscribeEvents = true, initialNow = 1_700_000_000_000 } = {}) {
  let currentNow = initialNow;
  const FakeDate = class extends Date {
    constructor(...args) {
      if (args.length === 0) super(currentNow);
      else super(...args);
    }
    static now() {
      return currentNow;
    }
  };

  const elementsById = new Map();
  const docListeners = new Map();
  const windowListeners = new Map();
  const doc = {
    visibilityState: "visible",
    activeElement: null,
    createElement(tag) {
      return new FakeElement(tag, "http://www.w3.org/1999/xhtml", doc);
    },
    createElementNS(ns, tag) {
      return new FakeElement(tag, ns, doc);
    },
    createTextNode(text) {
      return new FakeNode(3, text);
    },
    getElementById(id) {
      return elementsById.get(id) || null;
    },
    querySelectorAll() {
      return [];
    },
    addEventListener(type, fn) {
      if (!docListeners.has(type)) docListeners.set(type, []);
      docListeners.get(type).push(fn);
    },
    removeEventListener(type, fn) {
      const arr = docListeners.get(type);
      if (arr) docListeners.set(type, arr.filter((f) => f !== fn));
    },
  };

  // Build #control-dialog elements matching web.html
  const controlDialog = doc.createElement("dialog");
  controlDialog.id = "control-dialog";
  const controlForm = doc.createElement("form");
  controlForm.id = "control-form";
  for (const [id, tag] of [
    ["confirm-title", "h2"],
    ["confirm-warning", "p"],
    ["confirm-facts", "dl"],
    ["confirm-reason-field", "div"],
    ["confirm-reason-label", "label"],
    ["confirm-reason", "input"],
    ["confirm-reason-error", "p"],
    ["confirm-cli-parity", "div"],
    ["confirm-cli-cmd", "code"],
    ["confirm-cli-status", "span"],
    ["dismiss-control", "button"],
    ["confirm-control", "button"],
  ]) {
    const el = doc.createElement(tag);
    el.id = id;
    el.isConnected = true;
    controlForm.append(el);
    elementsById.set(id, el);
  }
  controlDialog.append(controlForm);
  controlDialog.isConnected = true;
  elementsById.set("control-dialog", controlDialog);
  elementsById.set("control-form", controlForm);

  const root = doc.createElement("div");
  root.id = "fleet-content";
  root.isConnected = true;
  elementsById.set("fleet-content", root);

  const timers = new Set();
  let eventSubscriber = null;
  const storage = new Map();

  const context = {
    window: {},
    document: doc,
    Node: FakeNode,
    Date: FakeDate,
    Math,
    Number,
    Array,
    Object,
    String,
    Boolean,
    Promise,
    Error,
    TypeError,
    RangeError,
    DOMException,
    AbortController,
    AbortSignal,
    URLSearchParams,
    TextDecoder,
    TextEncoder,
    Intl,
    crypto: webcrypto,
    sessionStorage: {
      getItem: (k) => (storage.has(k) ? storage.get(k) : null),
      setItem: (k, v) => { storage.set(k, String(v)); },
    },
    prompt() { throw new Error("prompt() must never be called"); },
    alert() { throw new Error("alert() must never be called"); },
    confirm() { throw new Error("confirm() must never be called"); },
    setTimeout(fn, ms) {
      const id = setTimeout(fn, Math.min(ms || 0, 5000));
      timers.add(id);
      if (id.unref) id.unref();
      return id;
    },
    clearTimeout(id) {
      timers.delete(id);
      clearTimeout(id);
    },
    addEventListener(type, fn) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(fn);
    },
    removeEventListener(type, fn) {
      const arr = windowListeners.get(type);
      if (arr) windowListeners.set(type, arr.filter((f) => f !== fn));
    },
  };
  context.window = context;
  context.window.KilnWork = { session: () => ({ principal: "local:operator", can_write: true }) };
  context.window.KilnCairn = {
    indexRows: () => [],
    lane: (k) => ({ label: k, tone: "neutral" }),
    condition: (c) => ({ label: String(c || "") }),
    detailPath: ({ repository, change }) => `/api/implementation/cairn/detail?repository=${repository}&change=${change}`,
    admitDetail: (v) => v,
  };

  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("fleet-capacity.js"), context);
  vm.runInNewContext(asset("fleet.js"), context);

  const apiCalls = [];
  const fleet = context.window.KilnFleet;
  let current = true;
  const loadCtx = {
    isCurrent: () => current,
    canOperate: () => canOperate,
    route: { theme: "system", worker_campaign: "" },
    navigate() {},
    api: async (path, opts = {}) => {
      apiCalls.push({ path, method: opts.method || "GET", body: opts.body || null });
      if (apiHandler) return apiHandler(path, opts);
      if (path === "/api/fleet/machines") return { machines: [] };
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/fleet/dispatch/status") return { state: "running", deferred_count: 0 };
      if (path === "/api/fleet/claims?include=foreign,expired") return { claims: [], foreign_claims: [], expired_claims: [] };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      if (path === "/api/fleet/swarms") return { swarms: [] };
      return {};
    },
    ...(withSubscribeEvents ? {
      subscribeEvents: (fn) => {
        eventSubscriber = fn;
        return () => { eventSubscriber = null; };
      },
    } : {}),
  };

  return {
    fleet,
    root,
    doc,
    context,
    elementsById,
    loadCtx,
    apiCalls,
    emitEvent: (name, payload) => eventSubscriber?.(name, payload),
    dispatchDocument: (type) => { for (const fn of [...(docListeners.get(type) || [])]) fn({ type }); },
    advanceNow: (ms) => { currentNow += ms; },
    cleanup: () => {
      current = false;
      fleet.clear();
      for (const id of timers) clearTimeout(id);
      timers.clear();
    },
  };
}

const plain = (v) => JSON.parse(JSON.stringify(v));
const HASH_A = "a".repeat(64);
const HASH_B = "b".repeat(64);
const HASH_C = "c".repeat(64);
const HASH_V1 = "1".repeat(64);
const HASH_V2 = "2".repeat(64);

function makeSnapshot(overrides = {}) {
  return {
    version: HASH_V1,
    health: { api: "v1", machine: "aspen3", version: "0.9.0" },
    claims: {
      claims: [{
        identity: HASH_A,
        fence: 7,
        state: "open",
        scope: { target: "onixresearch/kiln", path: "crates/kiln-web-service" },
        holder: { tool: "omp", host: "aspen3", session: "s1" },
        purpose: "Testing fleet UI",
        expires_ms: 1_700_000_300_000,
      }],
    },
    work: { ready_unclaimed: 1, ranked: [], blocked: [] },
    attempts: {
      attempts: [{
        attempt: HASH_B,
        fence: 3,
        state: "executing",
        phase: "active",
        repository: "onixresearch/kiln",
        title: "Build task",
        session: { machine: "aspen3", agent_name: "omp-1", state: "working" },
        lease_expires_ms: 1_700_000_400_000,
      }],
    },
    decisions: { decisions: [] },
    ...overrides,
  };
}

test("fleet.js contains zero prompt(), alert(), or confirm() browser dialogs", () => {
  const src = asset("fleet.js");
  assert.equal(/\b(?:prompt|alert|confirm)\s*\(/.test(src), false);
});

test("#control-dialog freshness guard enforces 30s bound and required reason on Renew, Release, and Cancel", async () => {
  const env = setup();
  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 15));

    // Before live snapshot -> Renew/Release/Cancel cannot mutate; emit live snapshot first.
    env.emitEvent("fleet", makeSnapshot());
    await new Promise((r) => setTimeout(r, 15));

    const findButton = (label) => descendants(env.root).find((n) => n.localName === "button" && n.textContent === label);
    const renewBtn = findButton("Renew");
    const releaseBtn = findButton("Release");
    const cancelBtn = findButton("Cancel");
    assert.ok(renewBtn && releaseBtn && cancelBtn);

    // 1. Stale snapshot (> 30s old) refuses to open #control-dialog or POST
    env.advanceNow(31_000);
    renewBtn.click();
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, false);
    assert.match(allText(env.root), /stale or disconnected \(older than 30s\)/);
    assert.equal(env.apiCalls.filter((c) => c.method === "POST").length, 0);

    // Refresh via heartbeat -> observedAt updated, fresh again
    env.emitEvent("heartbeat", {});
    await new Promise((r) => setTimeout(r, 10));

    // 2. Fresh Release requires reason; blank reason fails closed inside #control-dialog
    releaseBtn.click();
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, true);
    assert.match(env.elementsById.get("confirm-title").textContent, /Release claim/);
    env.elementsById.get("control-form").dispatchEvent({ type: "submit", preventDefault() {} });
    assert.match(env.elementsById.get("confirm-reason-error").textContent, /A reason is required/);
    assert.equal(env.elementsById.get("control-dialog").open, true);

    // Provide reason, but advance clock past 30s before submitting -> freshness guard blocks POST!
    env.elementsById.get("confirm-reason").value = "Operator release";
    env.advanceNow(31_000);
    env.elementsById.get("control-form").dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, false);
    assert.match(allText(env.root), /became stale or changed before confirmation completed/);
    assert.equal(env.apiCalls.filter((c) => c.method === "POST").length, 0);

    // 3. Fresh Renew + confirm -> sends exact POST with fence
    env.emitEvent("heartbeat", {});
    renewBtn.click();
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, true);
    env.elementsById.get("control-form").dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 15));
    const renewCall = env.apiCalls.find((c) => c.path === `/api/fleet/claims/${HASH_A}/renew`);
    assert.ok(renewCall);
    assert.equal(renewCall.method, "POST");
    assert.deepEqual(plain(renewCall.body), { fence: 7 });

    // 4. Fresh Cancel with reason -> sends exact POST with reason
    cancelBtn.click();
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, true);
    env.elementsById.get("confirm-reason").value = "Duplicate attempt";
    env.elementsById.get("control-form").dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 15));
    const cancelCall = env.apiCalls.find((c) => c.path === `/api/fleet/attempts/${HASH_B}/cancel`);
    assert.ok(cancelCall);
    assert.equal(cancelCall.method, "POST");
    assert.deepEqual(plain(cancelCall.body), { reason: "Duplicate attempt" });
  } finally {
    env.cleanup();
  }
});

test("Machine & Slot Census distinguishes Unavailable vs Available slots and refuses malformed machine roster", async () => {
  const env = setup({
    apiHandler: async (path) => {
      if (path === "/api/fleet/machines") {
        return {
          machines: [
            {
              machine: "aspen3",
              placement_configured: true,
              state: "reporting",
              health: "available",
              observed_at_ms: 1_700_000_000_000,
              last_seen_at_ms: 1_700_000_000_000,
              sessions: 2,
              slots: { worker: { used: 2, capacity: 4 }, model: { used: 1, capacity: 2 } },
            },
            {
              machine: "spark-offline",
              placement_configured: true,
              state: "unreachable",
              health: "unavailable",
              observed_at_ms: null,
              last_seen_at_ms: 1_699_999_000_000,
              sessions: null,
              error: { code: "connect_timeout", detail: "Herdr socket timed out" },
            },
          ],
        };
      }
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/fleet/dispatch/status") return { state: "running", deferred_count: 0 };
      if (path === "/api/fleet/claims?include=foreign,expired") return { claims: [], foreign_claims: [], expired_claims: [] };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      return {};
    },
  });

  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 25));

    const machinesPanel = descendants(env.root).find((n) => n.id === "fleet-machines");
    const text = allText(machinesPanel);
    assert.match(text, /aspen3/);
    assert.match(text, /worker: 2\/4 · model: 1\/2/);
    assert.match(text, /spark-offline/);
    assert.match(text, /OFFLINE · Unavailable/);
    assert.match(text, /Herdr socket timed out/);
  } finally {
    env.cleanup();
  }
});

test("Herdr browser uses configured hosts and keeps terminal capture operator-only", async () => {
  const pane = { pane_id: "w1:pA", workspace_id: "w1", tab_id: "w1:t1" };
  const rows = ["aspen3", "offline", "enrolled-only"].map((machine) => ({
    machine, herdr_windows_configured: machine !== "enrolled-only", placement_configured: false,
    state: "not_reporting", observed_at_ms: null, last_seen_at_ms: null, sessions: null,
  }));
  const list = "/api/fleet/machines/aspen3/windows";
  const capture = `${list}/w1:pA/capture?lines=200`;
  const result = {
    machine: "aspen3", window: pane, requested_lines: 200, line_count: 1, truncated: false, text: "<script>plain text</script>",
  };
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  for (const canOperate of [false, true]) {
    let finishLateCapture;
    let captures = 0;
    const env = setup({
      canOperate,
      apiHandler: async (path) => {
        if (path === "/api/fleet/machines") return { machines: rows };
        if (path === list) return { machine: "aspen3", windows: [pane] };
        if (path === capture) {
          if (captures++ === 0) return result;
          return new Promise((resolve) => { finishLateCapture = resolve; });
        }
        if (path === "/api/fleet/machines/offline/windows") throw new Error("unavailable");
        return {};
      },
    });
    const control = (label) => descendants(env.root).find((node) => node.getAttribute?.("aria-label") === label);
    try {
      env.fleet.load(env.root, env.loadCtx);
      await settle();
      assert.equal(control("Browse Herdr windows on enrolled-only"), undefined);
      control("Browse Herdr windows on aspen3").click();
      await settle();
      assert.match(allText(env.root), /Herdr windows · aspen3/);
      assert.match(allText(env.root), /w1:pA/);
      const button = control("Capture output for aspen3 / w1:pA");
      if (!canOperate) {
        assert.equal(button, undefined);
        assert.equal(env.apiCalls.some((call) => call.path === capture), false);
        continue;
      }
      button.click();
      await settle();
      assert.deepEqual(plain(env.apiCalls.find((call) => call.path === capture)), { path: capture, method: "POST", body: {} });
      const pre = descendants(env.root).find((node) => node.localName === "pre" && node.textContent === result.text);
      assert.ok(pre, "terminal content is displayed as text");
      button.click();
      control("Browse Herdr windows on offline").click();
      finishLateCapture(result);
      await settle();
      assert.match(allText(env.root), /Herdr windows · offline/);
      assert.match(allText(env.root), /Herdr windows unavailable on this host/);
      assert.doesNotMatch(allText(env.root), /plain text/);
    } finally {
      env.cleanup();
    }
  }
});

function herdrViewerFixture() {
  const pane = { pane_id: "w1:pA", workspace_id: "w1", tab_id: "w1:t1" };
  const machines = [{
    machine: "aspen3", herdr_windows_configured: true, placement_configured: false,
    state: "not_reporting", observed_at_ms: null, last_seen_at_ms: null, sessions: null,
  }];
  const list = "/api/fleet/machines/aspen3/windows";
  return {
    pane,
    machines,
    list,
    text: `${list}/w1:pA/capture?lines=200`,
    ansi: `${list}/w1:pA/capture?lines=1000&format=ansi&source=visible`,
    snapshot: {
      machine: "aspen3", window: pane, requested_lines: 1000, format: "ansi", source: "visible",
      columns: 120, rows: 40, line_count: 2, truncated: false,
      ansi: "\u001b[31mred\u001b[0m\r\n\u001b]52;c;aGk=\u0007<script>alert(1)</script>",
    },
  };
}

// r[verify kiln.sdlc.herdr_viewer.scenario.view]
test("Herdr viewer loads lazily, refreshes only while open and visible, and stops on a stale window", async () => {
  const fixture = herdrViewerFixture();
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  let stale = false;
  const env = setup({
    apiHandler: async (path, opts) => {
      if (path === "/api/fleet/machines") return { machines: fixture.machines };
      if (path === fixture.list) return { machine: "aspen3", windows: [fixture.pane] };
      if (path === fixture.ansi) {
        assert.equal(opts.method, "POST");
        if (stale) throw Object.assign(new Error("gone"), { code: "fleet_herdr_window_unknown", status: 404 });
        return fixture.snapshot;
      }
      return {};
    },
  });
  const renders = [];
  let loads = 0;
  let disposed = 0;
  env.context.KilnHerdrViewer = {
    load: async () => {
      loads += 1;
      return {
        mount: (surface) => {
          assert.equal(surface.className, "fleet-herdr-viewer");
          return {
            render: (bytes, columns, rows) => renders.push([new TextDecoder().decode(bytes), columns, rows]),
            dispose: () => { disposed += 1; },
          };
        },
      };
    },
  };
  const realSetTimeout = env.context.setTimeout;
  const refreshes = [];
  env.context.setTimeout = (fn, ms) => {
    if (ms === 2_000) {
      refreshes.push(fn);
      return -refreshes.length;
    }
    return realSetTimeout(fn, ms);
  };
  const ansiCalls = () => env.apiCalls.filter((call) => call.path === fixture.ansi).length;
  const control = (label) => descendants(env.root).find((node) => node.getAttribute?.("aria-label") === label);
  const runRefresh = async () => {
    refreshes.shift()();
    await settle();
  };
  try {
    env.fleet.load(env.root, env.loadCtx);
    await settle();
    control("Browse Herdr windows on aspen3").click();
    await settle();
    assert.equal(loads, 0, "the viewer module loads only when a window is opened");
    control("Open viewer for aspen3 / w1:pA").click();
    await settle();
    assert.equal(loads, 1);
    assert.deepEqual(plain(env.apiCalls.find((call) => call.path === fixture.ansi)),
      { path: fixture.ansi, method: "POST", body: {} });
    assert.deepEqual(renders, [[fixture.snapshot.ansi, 120, 40]]);
    assert.match(allText(env.root), /aspen3 \/ w1:pA · Visible screen · Read .* · Refreshing every 2s while open and visible/);
    assert.doesNotMatch(allText(env.root), /script|alert|aGk=/, "pane bytes reach only the viewer, never DOM text");
    assert.equal(refreshes.length, 1, "one bounded refresh is scheduled");

    await runRefresh();
    assert.equal(ansiCalls(), 2);
    assert.equal(renders.length, 2);
    assert.equal(refreshes.length, 1);

    env.doc.visibilityState = "hidden";
    await runRefresh();
    assert.equal(ansiCalls(), 2, "a hidden page does not refresh");
    assert.equal(refreshes.length, 0);
    assert.match(allText(env.root), /Paused while the page is hidden/);
    env.doc.visibilityState = "visible";
    env.dispatchDocument("visibilitychange");
    await settle();
    assert.equal(ansiCalls(), 3, "returning to the page resumes the open viewer");
    assert.equal(refreshes.length, 1);

    stale = true;
    await runRefresh();
    assert.equal(ansiCalls(), 4);
    assert.equal(refreshes.length, 0, "a stale window stops refreshing");
    assert.match(allText(env.root), /Refresh stopped · This window is no longer in the host inventory/);
    env.dispatchDocument("visibilitychange");
    await settle();
    assert.equal(ansiCalls(), 4, "a stopped viewer stays stopped");

    control("Close viewer for aspen3 / w1:pA").click();
    assert.equal(disposed, 1);
    assert.match(allText(env.root), /Viewer closed/);
    assert.equal(renders.length, 3);

    stale = false;
    control("Open viewer for aspen3 / w1:pA").click();
    await settle();
    assert.equal(refreshes.length, 1);
    const closeWindows = descendants(env.root).find((node) => node.textContent === "Close windows");
    closeWindows.click();
    assert.equal(disposed, 2, "closing the window browser disposes the viewer");
    const pending = refreshes.shift();
    pending();
    await settle();
    assert.equal(ansiCalls(), 5, "a closed viewer never refreshes");
  } finally {
    env.cleanup();
  }
});

test("Herdr viewer falls back to plain-text capture when the module or canvas is unavailable", async () => {
  const fixture = herdrViewerFixture();
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  const plainText = {
    machine: "aspen3", window: fixture.pane, requested_lines: 200, line_count: 1, truncated: false, text: "<b>plain</b>",
  };
  const unavailable = [
    ["missing module", undefined],
    ["no canvas", { load: async () => ({ mount: () => { throw new Error("2D canvas context unavailable"); } }) }],
    ["draw failure", { load: async () => ({ mount: () => ({ render: () => { throw new Error("bad"); }, dispose() {} }) }) }],
  ];
  for (const [name, module] of unavailable) {
    const env = setup({
      apiHandler: async (path) => {
        if (path === "/api/fleet/machines") return { machines: fixture.machines };
        if (path === fixture.list) return { machine: "aspen3", windows: [fixture.pane] };
        if (path === fixture.ansi) return fixture.snapshot;
        if (path === fixture.text) return plainText;
        return {};
      },
    });
    if (module) env.context.KilnHerdrViewer = module;
    const control = (label) => descendants(env.root).find((node) => node.getAttribute?.("aria-label") === label);
    try {
      env.fleet.load(env.root, env.loadCtx);
      await settle();
      control("Browse Herdr windows on aspen3").click();
      await settle();
      control("Open viewer for aspen3 / w1:pA").click();
      for (let turn = 0; turn < 4; turn += 1) await settle();
      assert.ok(env.apiCalls.some((call) => call.path === fixture.text), name);
      const pre = descendants(env.root).find((node) => node.localName === "pre" && node.textContent === plainText.text);
      assert.ok(pre, `${name}: the plain-text capture is displayed as text`);
      assert.match(allText(env.root), /showing plain text/, name);
    } finally {
      env.cleanup();
    }
  }
});

test("foreign and expired claims counts and mirror errors render explicitly", async () => {
  const env = setup({
    apiHandler: async (path) => {
      if (path === "/api/fleet/claims?include=foreign,expired") {
        return {
          claims: [],
          foreign_claims: [{ identity: HASH_C }],
          expired_claims: [{ identity: HASH_A }, { identity: HASH_B }],
          foreign_mirror: { error: "NNTP mirror unreachable" },
        };
      }
      if (path === "/api/fleet/machines") return { machines: [] };
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/fleet/dispatch/status") return { state: "running", deferred_count: 0 };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      return {};
    },
  });

  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 25));
    const text = allText(env.root);
    assert.match(text, /1 foreign claim conflict · 2 expired leases · Mirror: NNTP mirror unreachable/);
  } finally {
    env.cleanup();
  }
});

test("SSE disconnect and invalid snapshot transitions state to Unavailable/Stale without wiping retained rows", async () => {
  const env = setup();
  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 15));

    // 1. Unavailability before any snapshot -> Unavailable
    env.emitEvent("fleet", { unavailable: true, error: "Coordinator starting" });
    await new Promise((r) => setTimeout(r, 10));
    const conn = descendants(env.root).find((n) => n.classList.contains("fleet-connection"));
    assert.equal(conn.dataset.state, "unavailable");
    assert.match(allText(env.root), /Coordinator starting/);

    // 2. Valid snapshot -> Live with claim rendered
    env.emitEvent("fleet", makeSnapshot({ version: HASH_V1 }));
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(conn.dataset.state, "live");
    assert.match(allText(env.root), /Testing fleet UI/);

    // 3. Subsequent disconnect -> Stale · last snapshot shown, retained claim remains visible
    env.emitEvent("fleet", { unavailable: true, error: "SSE dropped" });
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(conn.dataset.state, "stale");
    assert.equal(conn.textContent, "Stale · last snapshot shown");
    assert.match(allText(env.root), /SSE dropped/);
    assert.match(allText(env.root), /Testing fleet UI/);
    assert.match(allText(env.root), /Fleet assignments disconnected or reconnecting/);

    // 4. Malformed snapshot payload (invalid version hash) -> Stale + transport error
    env.emitEvent("fleet", makeSnapshot({ version: HASH_V2 }));
    assert.equal(conn.dataset.state, "live");
    env.emitEvent("fleet", { version: "not-hex", health: {}, claims: {}, work: {}, attempts: {}, decisions: {} });
    assert.equal(conn.dataset.state, "stale");
    assert.match(allText(env.root), /Fleet event response is invalid/);
  } finally {
    env.cleanup();
  }
});

test("Fair-Admission governance renders tenant shares, separates transfers from consumption, shows not recorded for missing AttemptMetrics, and guards share updates", async () => {
  const env = setup({
    apiHandler: async (path, opts = {}) => {
      if (path === "/api/implementation/admission") {
        return {
          schema: "kiln.implementation-admission.v1",
          configured: true,
          bounded: true,
          truncated: false,
          policy: {
            identity: HASH_A,
            version: 3,
            accounting_basis: "ledger_v1",
            strategy: "drf",
            bounds: { max_items: 64, max_bytes: 1048576, max_active: 8, max_children: 4 },
            shares: { "tenant-alpha": 2, "tenant-beta": 1 },
          },
          accounts: [
            {
              identity: HASH_B,
              parent: null,
              depth: 0,
              tenant: "tenant-alpha",
              allocated: { tokens: 10000 },
              transferred_out: { tokens: 3000 },
              consumed: { tokens: 1200 },
              held: { tokens: 500 },
              tentative: { tokens: 200 },
              unknown: {},
              available: { tokens: 5100 },
              settled_measured: { tokens: 1000 },
              settled_unknown: {},
            },
          ],
          serving: [
            {
              provider: "aspen",
              scope: "pool-a",
              bounds: { max_ongoing: 4, max_queued: 8 },
              readiness: "ready",
              origin: "observed",
              remaining_slots: 2,
              queued: 1,
              source_generation: 5,
              deployment_generation: 2,
            },
          ],
          attempt_metrics: [
            {
              attempt_id: HASH_C,
              fence: 4,
              phase: "executing",
              open: true,
              telemetry_source: "not_recorded",
              tokens: null,
              tokens_status: "absent",
              tokens_absence_reason: "provider_omitted",
              model_turns: null,
              model_turns_status: "absent",
              auxiliary_calls: null,
              auxiliary_calls_status: "absent",
              wall_time_ms: null,
              harness: "omp",
              model: "hydrogen",
              verification_outcome: "Pending",
            },
          ],
          non_claims: ["Admission ledger is not physical host liveness."],
        };
      }
      if (path === "/api/implementation/admission/policy" && opts.method === "POST") {
        return { schema: "kiln.implementation-admission-result.v1", result: "updated", policy_identity: HASH_A, request_id: opts.body?.request_id };
      }
      if (path === "/api/fleet/machines") return { machines: [] };
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/fleet/work") return { ready_unclaimed: 0, ranked: [], blocked: [] };
      if (path === "/api/workers") return { schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, summary: { targets: 0, enrolled_slots: 0, configured_slots: 0, active: 0, free_slots: 0, queued: 0 }, workers: [] };
      if (path === "/api/fleet/dispatch/status") return { state: "running", deferred_count: 0 };
      if (path === "/api/fleet/claims?include=foreign,expired") return { claims: [], foreign_claims: [], expired_claims: [] };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      if (path === "/api/fleet/swarms") return { swarms: [] };
      return {};
    },
  });

  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 25));

    const text = allText(env.root);
    assert.match(text, /Fair-admission governance & AttemptMetrics/);
    assert.match(text, /tenant-alpha/);
    assert.match(text, /Transfers \(transferred_out\)/);
    assert.match(text, /tokens: 3,000/);
    assert.match(text, /tokens: 1,200/);
    // Unobserved AttemptMetrics MUST read as "not recorded", never "0"
    assert.match(text, /not recorded \(provider_omitted\)/);

    // Opt-in Atlas toggle is off by default
    const atlasBtn = descendants(env.root).find((n) => n.classList.contains("fleet-atlas-toggle"));
    assert.ok(atlasBtn);
    assert.equal(atlasBtn.getAttribute("aria-expanded"), "false");

    // Negative: share update blocked when snapshot is stale (not yet live)
    const shareForm = descendants(env.root).find((n) => n.classList.contains("fleet-admission-share-form"));
    assert.ok(shareForm);
    shareForm.dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 10));
    assert.match(allText(env.root), /snapshot is stale or disconnected/);
    assert.equal(env.apiCalls.filter((c) => c.path === "/api/implementation/admission/policy").length, 0);

    // Negative: invalid share (<= 0) rejected before confirmation
    env.emitEvent("fleet", makeSnapshot());
    await new Promise((r) => setTimeout(r, 10));
    const inputs = descendants(shareForm).filter((n) => n.localName === "input");
    inputs[0].value = "tenant-alpha";
    inputs[1].value = "0";
    shareForm.dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 10));
    assert.match(allText(env.root), /positive integer share/);

    // Positive: valid share while live opens #control-dialog and submits exact policy update
    inputs[1].value = "5";
    shareForm.dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.elementsById.get("control-dialog").open, true);
    env.elementsById.get("control-form").dispatchEvent({ type: "submit", preventDefault() {} });
    await new Promise((r) => setTimeout(r, 20));
    const policyPost = env.apiCalls.find((c) => c.path === "/api/implementation/admission/policy" && c.method === "POST");
    assert.ok(policyPost);
    assert.equal(policyPost.body.expected_policy_id, HASH_A);
    assert.equal(policyPost.body.command, "configure");
    assert.deepEqual(plain(policyPost.body.shares), { "tenant-alpha": 5, "tenant-beta": 1 });
  } finally {
    env.cleanup();
  }
});

test("Fair-Admission unconfigured and unavailable states render explicitly without fabricating quotas", async () => {
  const env = setup({
    apiHandler: async (path) => {
      if (path === "/api/implementation/admission") {
        return { schema: "kiln.implementation-admission.v1", configured: false, bounded: true, policy: null, accounts: [], serving: [], attempt_metrics: [] };
      }
      if (path === "/api/fleet/machines") return { machines: [] };
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, principal: "local:operator", objects: [] };
      if (path === "/api/fleet/work") return { ready_unclaimed: 0, ranked: [], blocked: [] };
      if (path === "/api/workers") return { schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, summary: { targets: 0, enrolled_slots: 0, configured_slots: 0, active: 0, free_slots: 0, queued: 0 }, workers: [] };
      if (path === "/api/fleet/dispatch/status") return { state: "running", deferred_count: 0 };
      if (path === "/api/fleet/claims?include=foreign,expired") return { claims: [], foreign_claims: [], expired_claims: [] };
      if (path === "/api/implementation/cairn") return { changes: [], snapshot: { work_recorded: true, attempts_recorded: true } };
      if (path === "/api/fleet/swarms") return { swarms: [] };
      return {};
    },
  });

  try {
    env.fleet.load(env.root, env.loadCtx);
    await new Promise((r) => setTimeout(r, 25));
    assert.match(allText(env.root), /No fair-admission policy configured/);
    assert.match(allText(env.root), /no quota or burn is fabricated/);
  } finally {
    env.cleanup();
  }
});
