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
  toggle(name, force) {
    const has = this.contains(name);
    const shouldAdd = force === undefined ? !has : Boolean(force);
    if (shouldAdd) this.add(name);
    else this.remove(name);
    return shouldAdd;
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
    this.inert = false;
    this.tabIndex = 0;
    this.scrollTop = 0;
    this.scrollLeft = 0;
    this.scrollHeight = 0;
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
  get href() {
    return this.attributes.get("href") || "";
  }
  set href(value) {
    this.attributes.set("href", String(value));
  }
  get firstElementChild() {
    return this.children.find((c) => c.nodeType === 1) || null;
  }
  get childElementCount() {
    return this.children.filter((c) => c.nodeType === 1).length;
  }
  get options() {
    return this.children.filter((c) => c.localName === "option");
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
    if (k === "value") this.value = v;
  }
  getAttribute(key) {
    return this.attributes.has(key) ? this.attributes.get(key) : null;
  }
  hasAttribute(key) {
    return this.attributes.has(key);
  }
  removeAttribute(key) {
    this.attributes.delete(key);
    if (key.startsWith("data-")) {
      const camel = key.slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
      delete this.dataset[camel];
    }
  }
  toggleAttribute(key, force) {
    const has = this.hasAttribute(key);
    const next = force === undefined ? !has : Boolean(force);
    if (next) this.setAttribute(key, "");
    else this.removeAttribute(key);
    return next;
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
  prepend(...nodes) {
    const converted = [];
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      node.parentNode = this;
      setConnected(node, this.isConnected);
      converted.push(node);
    }
    this.children = [...converted, ...this.children];
  }
  before(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = nodes.filter((n) => n !== null && n !== undefined).map((n) => (n instanceof FakeNode ? n : new FakeNode(3, String(n))));
    for (const n of converted) {
      n.parentNode = this.parentNode;
      setConnected(n, this.parentNode.isConnected);
    }
    this.parentNode.children.splice(idx, 0, ...converted);
  }
  insertBefore(newNode, refNode) {
    if (newNode.parentNode && newNode.parentNode !== this) {
      newNode.parentNode.children = newNode.parentNode.children.filter((c) => c !== newNode);
    }
    const idx = refNode ? this.children.indexOf(refNode) : -1;
    newNode.parentNode = this;
    setConnected(newNode, this.isConnected);
    if (idx === -1) this.children.push(newNode);
    else this.children.splice(idx, 0, newNode);
    return newNode;
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
  remove() {
    if (!this.parentNode) return;
    this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
    this.parentNode = null;
    setConnected(this, false);
  }
  showModal() {
    this.open = true;
    this.setAttribute("open", "");
  }
  close() {
    this.open = false;
    this.removeAttribute("open");
    queueMicrotask(() => this.dispatchEvent({ type: "close" }));
  }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  removeEventListener(type, fn) {
    const arr = this.listeners.get(type);
    if (!arr) return;
    this.listeners.set(type, arr.filter((f) => f !== fn));
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
  matches() {
    return false;
  }
  getClientRects() {
    return this.hidden ? [] : [{ width: 10, height: 10 }];
  }
  getBoundingClientRect() {
    return { top: 0, bottom: 100, left: 0, right: 100, width: 100, height: 100 };
  }
  scrollIntoView() {}
  contains(other) {
    let cur = other;
    while (cur) {
      if (cur === this) return true;
      cur = cur.parentNode;
    }
    return false;
  }
  closest(selector) {
    let cur = this;
    while (cur && cur.nodeType === 1) {
      if (matchesSelector(cur, selector)) return cur;
      cur = cur.parentNode;
    }
    return null;
  }
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }
  querySelectorAll(selector) {
    const parts = selector.split(",").map((s) => s.trim()).filter(Boolean);
    const out = [];
    const walk = (node) => {
      for (const child of node.children || []) {
        if (child.nodeType === 1) {
          if (parts.some((p) => matchesChain(child, p))) out.push(child);
          walk(child);
        }
      }
    };
    walk(this);
    return out;
  }
}

class FakeHTMLElement extends FakeElement {}

function setConnected(node, connected) {
  node.isConnected = connected;
  for (const child of node.children || []) setConnected(child, connected);
}

function matchesChain(el, chain) {
  const parts = chain.trim().split(/\s+/);
  if (!matchesSelector(el, parts.at(-1))) return false;
  if (parts.length === 1) return true;
  let cur = el.parentNode;
  for (let i = parts.length - 2; i >= 0; i -= 1) {
    while (cur && cur.nodeType === 1 && !matchesSelector(cur, parts[i])) cur = cur.parentNode;
    if (!cur || cur.nodeType !== 1) return false;
    cur = cur.parentNode;
  }
  return true;
}

function matchesSelector(el, sel) {
  let rest = sel;
  const tagMatch = rest.match(/^[a-zA-Z0-9_-]+/);
  if (tagMatch) {
    if (el.localName !== tagMatch[0].toLowerCase()) return false;
    rest = rest.slice(tagMatch[0].length);
  }
  while (rest.length) {
    if (rest.startsWith("#")) {
      const m = rest.match(/^#([a-zA-Z0-9_-]+)/);
      if (!m || el.id !== m[1]) return false;
      rest = rest.slice(m[0].length);
    } else if (rest.startsWith(".")) {
      const m = rest.match(/^\.([a-zA-Z0-9_-]+)/);
      if (!m || !el.classList.contains(m[1])) return false;
      rest = rest.slice(m[0].length);
    } else if (rest.startsWith("[")) {
      const m = rest.match(/^\[([a-zA-Z0-9_-]+)(?:=["']?([^"'\]]*)["']?)?\]/);
      if (!m) return false;
      const val = el.getAttribute(m[1]);
      if (m[2] !== undefined ? val !== m[2] : val === null) return false;
      rest = rest.slice(m[0].length);
    } else if (rest.startsWith(":not(")) {
      const end = rest.indexOf(")");
      if (end === -1) return false;
      rest = rest.slice(end + 1);
    } else {
      return false;
    }
  }
  return true;
}

const VOID_TAGS = new Set(["meta", "link", "input", "br", "hr", "img"]);
function buildHtmlDom(html, doc) {
  const root = new FakeHTMLElement("html", "http://www.w3.org/1999/xhtml", doc);
  root.isConnected = true;
  const cleaned = html.replace(/<!doctype[^>]*>/i, "").replace(/<!--[\s\S]*?-->/g, "");
  const tokenRe = /<(\/)?([a-zA-Z0-9-]+)([^>]*)>|([^<]+)/g;
  const stack = [root];
  let match;
  while ((match = tokenRe.exec(cleaned)) !== null) {
    if (match[4] !== undefined) {
      const text = match[4].replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&rsquo;/g, "'");
      if (text.trim()) stack.at(-1).append(new FakeNode(3, text));
      continue;
    }
    const closing = Boolean(match[1]);
    const tag = match[2].toLowerCase();
    const attrsRaw = match[3] || "";
    if (tag === "html") continue;
    if (closing) {
      for (let i = stack.length - 1; i > 0; i -= 1) {
        if (stack[i].localName === tag) {
          stack.length = i;
          break;
        }
      }
      continue;
    }
    const el = new FakeHTMLElement(tag, "http://www.w3.org/1999/xhtml", doc);
    const attrRe = /([a-zA-Z0-9_-]+)(?:\s*=\s*"([^"]*)")?/g;
    let am;
    while ((am = attrRe.exec(attrsRaw)) !== null) {
      const key = am[1];
      const val = am[2] ?? "";
      el.setAttribute(key, val);
      if (key === "disabled") el.disabled = true;
    }
    stack.at(-1).append(el);
    if (!VOID_TAGS.has(tag) && !attrsRaw.trim().endsWith("/")) stack.push(el);
  }
  return root;
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

function jsonResponse(payload, status = 200) {
  const bytes = new TextEncoder().encode(JSON.stringify(payload));
  let read = false;
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (k) => (k.toLowerCase() === "content-type" ? "application/json" : null) },
    body: {
      getReader() {
        return {
          async read() {
            if (read) return { done: true, value: undefined };
            read = true;
            return { done: false, value: bytes };
          },
          async cancel() {},
          releaseLock() {},
        };
      },
    },
  };
}

class FakeEvent {
  constructor(type, init = {}) {
    this.type = type;
    Object.assign(this, init);
  }
}

async function setup({ initialSearch = "", initialHash = "", fetchHandler } = {}) {
  const windowListeners = new Map();
  const docListeners = new Map();
  const doc = {
    title: "",
    visibilityState: "visible",
    activeElement: null,
    createElement(tag) {
      return new FakeHTMLElement(tag, "http://www.w3.org/1999/xhtml", doc);
    },
    createElementNS(ns, tag) {
      return new FakeElement(tag, ns, doc);
    },
    createTextNode(text) {
      return new FakeNode(3, text);
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
  const htmlRoot = buildHtmlDom(asset("web.html"), doc);
  doc.documentElement = htmlRoot;
  doc.body = htmlRoot.querySelector("body") || htmlRoot;
  doc.getElementById = (id) => htmlRoot.querySelector(`#${id}`);
  doc.querySelector = (sel) => htmlRoot.querySelector(sel);
  doc.querySelectorAll = (sel) => htmlRoot.querySelectorAll(sel);

  const location = { pathname: "/", search: initialSearch, hash: initialHash };
  const history = {
    pushState(_s, _t, url) {
      const idx = String(url).indexOf("?");
      location.search = idx >= 0 ? String(url).slice(idx) : "";
    },
  };

  const fetchCalls = [];
  const defaultFetch = async (target, init = {}) => {
    fetchCalls.push({ target, method: init.method || "GET", body: init.body ? JSON.parse(init.body) : null });
    if (fetchHandler) return fetchHandler(target, init);
    if (target === "/api/session") {
      return jsonResponse({ principal: { id: "local:operator", label: "Operator", role: "operator" }, permissions: { read: true, operate: true, configure: true } });
    }
    if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", reference: "refs/heads/main" }], available: [] });
    if (target.startsWith("/api/runs")) return jsonResponse({ runs: [], total: 0, next_cursor: null });
    if (target === "/api/overview") return jsonResponse({ projects: [], recent_runs: [], active_runs: [] });
    if (target === "/api/workers") return jsonResponse({ workers: [] });
    if (target === "/api/activity") return jsonResponse({ items: [], total: 0, audit_window: "24h" });
    if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
    if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
    if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_claims: [], expired_claims: [] });
    if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", deferred_count: 0 });
    if (target === "/api/implementation/cairn") return jsonResponse({ changes: [], snapshot: {} });
    if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [], queue_windows: [] });
    return jsonResponse({});
  };

  const clipboardWrites = [];
  const timers = new Set();
  const context = {
    window: {},
    document: doc,
    location,
    history,
    fetch: defaultFetch,
    navigator: { clipboard: { writeText: async (v) => { clipboardWrites.push(v); } } },
    matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }),
    Node: FakeNode,
    Element: FakeElement,
    HTMLElement: FakeHTMLElement,
    Event: FakeEvent,
    CustomEvent: FakeEvent,
    URL,
    URLSearchParams,
    TextDecoder,
    TextEncoder,
    DOMException,
    AbortController,
    AbortSignal,
    Blob,
    crypto: webcrypto,
    Date,
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
    Intl,
    setTimeout: (fn, ms) => {
      const id = setTimeout(fn, Math.min(ms || 0, 5000));
      timers.add(id);
      if (id.unref) id.unref();
      return id;
    },
    clearTimeout: (id) => { timers.delete(id); clearTimeout(id); },
    setInterval: () => 0,
    clearInterval: () => {},
    scrollTo: () => {},
    addEventListener(type, fn) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(fn);
    },
    removeEventListener(type, fn) {
      const arr = windowListeners.get(type);
      if (arr) windowListeners.set(type, arr.filter((f) => f !== fn));
    },
    dispatchEvent(event) {
      for (const fn of [...(windowListeners.get(event.type) || [])]) fn(event);
    },
  };
  context.window = context;

  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("workers.js"), context);
  vm.runInNewContext(asset("web.js"), context);

  // Let initial discoverNetworkAccess() and initial loadPage() settle.
  await new Promise((r) => setTimeout(r, 30));

  return {
    factory: context.window.KilnFactory,
    work: context.window.KilnWork,
    ui: context.window.KilnUi,
    doc,
    location,
    fetchCalls,
    clipboardWrites,
    dispatchWindow(type, props = {}) {
      context.dispatchEvent(new FakeEvent(type, props));
    },
    cleanup() {
      doc.getElementById("logout")?.click();
      for (const id of timers) clearTimeout(id);
      timers.clear();
    },
  };
}

test("route parsing resolves consolidated destinations, legacy ?page= aliases, and rejects malformed ?inspect=", async () => {
  // 1. Legacy alias ?page=triage -> page="inbox", tab="triage"
  const alias = await setup({ initialSearch: "?page=triage&inspect=run:run-123" });
  try {
    const r = alias.factory.route();
    assert.equal(r.page, "inbox");
    assert.equal(r.tab, "triage");
    assert.equal(r.inspect, "run:run-123");
  } finally {
    alias.cleanup();
  }

  // 2. Unknown ?page= falls back to overview; malformed ?inspect= (unknown kind, whitespace, "..") is rejected to ""
  for (const badInspect of ["unknownkind:id1", "run:", ":id1", "run:has space", "run:../escape", `run:${"x".repeat(513)}`]) {
    const bad = await setup({ initialSearch: `?page=nonexistent&inspect=${encodeURIComponent(badInspect)}` });
    try {
      const r = bad.factory.route();
      assert.equal(r.page, "overview");
      assert.equal(r.inspect, "", `expected malformed inspect ${badInspect} to be rejected`);
    } finally {
      bad.cleanup();
    }
  }
});

test("Unknown run reconciliation probes POST /api/runs/{id}/reconcile and updates state without creating a replacement", async () => {
  const unknownRun = {
    id: "run-unk-1",
    project_id: "proj-1",
    repository: "/repo",
    source_revision: "abc123def456",
    outcome: "Unknown",
    phase: "unknown",
    created_at_ms: 1_700_000_000_000,
    nodes: [],
  };
  const reconciledRun = { ...unknownRun, outcome: "Success", phase: "completed", completed_at_ms: 1_700_000_010_000, duration_ms: 10_000 };
  let reconcileCalls = 0;

  const env = await setup({
    initialSearch: "?page=runs&run=run-unk-1",
    fetchHandler: async (target, init = {}) => {
      if (target === "/api/session") return jsonResponse({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } });
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", reference: "refs/heads/main" }], available: [] });
      if (target.startsWith("/api/runs?")) return jsonResponse({ runs: [unknownRun], total: 1, next_cursor: null });
      if (target === "/api/runs/run-unk-1") return jsonResponse(unknownRun);
      if (target === "/api/runs/run-unk-1/reconcile" && init.method === "POST") {
        reconcileCalls += 1;
        return jsonResponse({ run: reconciledRun, reconciled: true, still_unknown: false, outcome: "Success", phase: "completed", note: "Provider confirmed Success." });
      }
      if (target.startsWith("/api/runs/run-unk-1/")) return jsonResponse({});
      return jsonResponse({});
    },
  });

  try {
    const reconcilePanel = env.doc.getElementById("run-reconcile");
    assert.equal(reconcilePanel.hidden, false);
    assert.match(allText(reconcilePanel), /Unknown execution · Reconciliation required/);
    const probeBtn = env.doc.getElementById("reconcile-run");
    assert.ok(probeBtn);
    assert.equal(probeBtn.disabled, false);

    probeBtn.click();
    await new Promise((r) => setTimeout(r, 30));

    assert.equal(reconcileCalls, 1);
    // After reconciliation to Success, the Unknown reconciliation card hides and status reflects resolved run.
    assert.equal(env.doc.getElementById("run-reconcile").hidden, true);
    assert.match(allText(env.doc.getElementById("run-badges")), /Passed/);
  } finally {
    env.cleanup();
  }
});

test("superseded_by and dirty_snapshot banners render drift facts and direct jump link", async () => {
  const supersededRun = {
    id: "run-old",
    project_id: "proj-1",
    repository: "/repo",
    source_revision: "rev-old",
    outcome: "Success",
    phase: "completed",
    superseded_by: "run-new-999",
    superseded_revision: "rev-new-999",
    dirty_snapshot: { manifest_id: "manifest-abc", base_revision: "rev-base", synthetic_revision: "rev-synth" },
    created_at_ms: 1_700_000_000_000,
    nodes: [],
  };

  const env = await setup({
    initialSearch: "?page=runs&run=run-old",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } });
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln" }], available: [] });
      if (target.startsWith("/api/runs?")) return jsonResponse({ runs: [supersededRun], total: 1, next_cursor: null });
      if (target === "/api/runs/run-old") return jsonResponse(supersededRun);
      return jsonResponse({});
    },
  });

  try {
    const banner = env.doc.getElementById("run-supersession");
    assert.equal(banner.hidden, false);
    const bannerText = allText(banner);
    assert.match(bannerText, /Target drift · superseded run/);
    assert.match(bannerText, /Dirty working-tree snapshot/);
    assert.match(bannerText, /manifest-abc/);
    assert.match(bannerText, /rev-new-999/);
    const jump = descendants(banner).find((n) => n.localName === "a" && n.textContent === "Open superseding run");
    assert.ok(jump);
    assert.match(jump.href, /run=run-new-999/);
  } finally {
    env.cleanup();
  }
});

test("session continuity preserves active session across pagehide and refreshes on persisted pageshow", async () => {
  const env = await setup({ initialSearch: "?page=overview" });
  try {
    assert.equal(env.work.session()?.principal, "local:operator");
    assert.equal(env.doc.getElementById("workspace").hidden, false);
    assert.equal(env.doc.getElementById("login-panel").hidden, true);

    const callsBefore = env.fetchCalls.length;
    env.dispatchWindow("pagehide", { persisted: true });
    await new Promise((r) => setTimeout(r, 10));

    // Session MUST NOT be cleared on pagehide
    assert.equal(env.work.session()?.principal, "local:operator");
    assert.equal(env.doc.getElementById("workspace").hidden, false);

    // pageshow (persisted) keeps session and refreshes workspace data
    env.dispatchWindow("pageshow", { persisted: true });
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(env.work.session()?.principal, "local:operator");
    assert.ok(env.fetchCalls.length > callsBefore);
  } finally {
    env.cleanup();
  }
});

test("Causal Event Timeline renders ordered state-machine progression, attribution, copyable BLAKE3 badges, and safe empty/malformed fallbacks", async () => {
  const evtHash1 = "a1b2c3d4e5f6".repeat(5) + "abcd";
  const evtHash2 = "f6e5d4c3b2a1".repeat(5) + "1234";
  const runId = "run-causal-1";
  const env = await setup({
    initialSearch: `?page=runs&run=${runId}&view=activity`,
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } });
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln" }], available: [] });
      if (target.startsWith("/api/runs?")) return jsonResponse({ runs: [{ id: runId, project_id: "proj-1", outcome: "Success", phase: "completed", created_at_ms: 1_700_000_000_000, nodes: [] }], total: 1 });
      if (target === `/api/runs/${runId}`) return jsonResponse({ id: runId, project_id: "proj-1", outcome: "Success", phase: "completed", created_at_ms: 1_700_000_000_000, nodes: [] });
      if (target.startsWith(`/api/runs/${runId}/activity`)) {
        return jsonResponse({
          total: 3,
          audit_window: "24h",
          items: [
            { kind: "event", ordinal: 3, event_index: 2, recorded_at_ms: 1_700_000_003_000, event_identity: evtHash2, principal: "kiln-runner", event: { AttemptCompleted: { outcome: "Success", attempt_id: "att-1" } } },
            { kind: "event", ordinal: 1, event_index: 0, recorded_at_ms: 1_700_000_001_000, event_identity: evtHash1, principal: "local:op", event: { RunStarted: { command_id: "cmd-1" } } },
            { kind: "audit", recorded_at_ms: 1_700_000_002_000, audit: { target: "/api/runs/run-causal-1/approve", method: "POST", decision: "admitted", actor_label: "Operator", agent: { harness: "omp", model: "hydrogen" }, identity: "audit-1234567890" } },
          ],
        });
      }
      return jsonResponse({});
    },
  });

  try {
    const activityRoot = env.doc.getElementById("run-activity-content");
    const railSteps = descendants(activityRoot).filter((n) => n.classList.contains("causal-stage-step"));
    assert.deepEqual(
      railSteps.map((s) => [s.dataset.stage, s.dataset.reached]),
      [
        ["admitted", "true"],
        ["untrusted_gate", "true"],
        ["dispatched", "false"],
        ["observations", "false"],
        ["controls_unknown", "false"],
        ["terminal", "true"],
      ],
    );

    // Events sorted chronologically: RunStarted (admitted) -> approve audit (untrusted_gate) -> AttemptCompleted (terminal)
    const events = descendants(activityRoot).filter((n) => n.classList.contains("causal-event"));
    assert.deepEqual(events.map((e) => e.dataset.stage), ["admitted", "untrusted_gate", "terminal"]);

    // 12-char BLAKE3 event_identity prefix + copy button copies full identity
    const idPrefix = descendants(events[0]).find((n) => n.classList.contains("causal-identity-prefix"));
    assert.equal(idPrefix.textContent, evtHash1.slice(0, 12));
    assert.equal(idPrefix.dataset.fullIdentity, evtHash1);
    const copyBtn = descendants(events[0]).find((n) => n.classList.contains("causal-copy-btn"));
    copyBtn.click();
    await new Promise((r) => setTimeout(r, 10));
    assert.deepEqual(env.clipboardWrites, [evtHash1]);

    // Attribution + raw JSON disclosure present
    assert.match(allText(events[1]), /Attribution: Operator · Agent omp \/ hydrogen \(declared\)/);
    assert.ok(descendants(events[0]).find((n) => n.localName === "details" && n.classList.contains("causal-raw")));

    // Negative: empty and malformed items do not throw
    const negativeBox = env.doc.createElement("div");
    env.factory.timeline.renderCausalTimeline(negativeBox, [], "Nothing recorded.");
    assert.match(allText(negativeBox), /Nothing recorded/);
    const malformedBox = env.doc.createElement("div");
    env.factory.timeline.renderCausalTimeline(malformedBox, [null, { kind: "unknown_kind" }, { event: null }]);
    assert.match(allText(malformedBox), /Recorded item/);
    assert.match(allText(malformedBox), /Not recorded/);
  } finally {
    env.cleanup();
  }
});

test("Cross-Node Failure Digest deduplicates multi-node DAG errors and jumps to exact failing node stream", async () => {
  const runId = "run-dag-fail";
  const failedRun = {
    id: runId,
    project_id: "proj-1",
    outcome: "DomainFailure",
    phase: "completed",
    created_at_ms: 1_700_000_000_000,
    log_bounds: { maximum_chunk_bytes: 16384 },
    nodes: [
      { id: "build-a", kind: "task", state: "Failed", error: "error[E0425]: missing symbol at src/a.rs:12:5" },
      { id: "build-b", kind: "task", state: "Failed", error: "error[E0425]: missing symbol at src/a.rs:48:9" },
      { id: "test-c", kind: "task", state: "Failed", error: "exit code 101" },
      { id: "lint-ok", kind: "task", state: "Completed" },
    ],
  };

  const env = await setup({
    initialSearch: `?page=runs&run=${runId}&view=logs`,
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } });
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln" }], available: [] });
      if (target.startsWith("/api/runs?")) return jsonResponse({ runs: [failedRun], total: 1 });
      if (target === `/api/runs/${runId}`) return jsonResponse(failedRun);
      if (target.startsWith(`/api/runs/${runId}/logs?`)) {
        const u = new URL("http://localhost" + target);
        const nodeId = u.searchParams.get("node_id");
        const stream = u.searchParams.get("stream");
        const line = nodeId === "build-a"
          ? "error[E0425]: missing symbol at src/a.rs:12:5\n"
          : nodeId === "build-b"
            ? "error[E0425]: missing symbol at src/a.rs:48:9\n"
            : "exit code 101\n";
        return jsonResponse({
          node_id: nodeId,
          stream,
          cursor: "0",
          next_cursor: "0",
          text: stream === "stderr" ? line : "",
          eof: true,
        });
      }
      return jsonResponse({});
    },
  });

  try {
    await new Promise((r) => setTimeout(r, 30));
    const digestBox = env.doc.getElementById("log-failure-digest");
    assert.ok(digestBox);
    assert.equal(digestBox.hidden, false);
    const digestText = allText(digestBox);
    assert.match(digestText, /Cross-Node Failure Digest/);
    assert.match(digestText, /3 failed nodes · 2 unique signatures/);
    assert.match(digestText, /2 nodes/);

    // Clicking jump button for build-b selects build-b and reads its stderr
    const jumpB = descendants(digestBox).find((n) => n.classList.contains("log-digest-jump") && n.dataset.nodeId === "build-b");
    assert.ok(jumpB);
    jumpB.click();
    await new Promise((r) => setTimeout(r, 30));
    assert.equal(env.doc.getElementById("log-node").value, "build-b");
    assert.equal(env.doc.getElementById("log-stream").value, "stderr");
    assert.ok(env.fetchCalls.some((c) => c.target.includes("node_id=build-b") && c.target.includes("stream=stderr")));

    // Negative: null or all-passing run yields 0 failed nodes and hides digest
    const clean = env.factory.failureDigest.build({ id: "ok", nodes: [{ id: "a", state: "Completed" }] });
    assert.equal(clean.failedNodeCount, 0);
    assert.equal(clean.groups.length, 0);
    assert.equal(env.factory.failureDigest.build(null).failedNodeCount, 0);
  } finally {
    env.cleanup();
  }
});
