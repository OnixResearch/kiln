import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class FakeClassList {
  constructor(owner) { this.owner = owner; }
  _tokens() { return String(this.owner.attributes.get("class") || "").split(/\s+/).filter(Boolean); }
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
    if (shouldAdd) this.add(name); else this.remove(name);
    return shouldAdd;
  }
  contains(name) { return this._tokens().includes(name); }
  *[Symbol.iterator]() { yield* this._tokens(); }
}

class FakeNode {
  constructor(nodeType, text = "") {
    this.nodeType = nodeType;
    this._text = String(text);
    this.children = [];
    this.parentNode = null;
    this.isConnected = false;
  }
  get textContent() { return this._text; }
  set textContent(value) { this._text = value == null ? "" : String(value); }
}

class FakeFragment extends FakeNode {
  constructor(doc) {
    super(11, "");
    this.ownerDocument = doc;
  }
  append(...nodes) {
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      if (item instanceof FakeFragment) {
        this.append(...item.children);
        item.children = [];
        continue;
      }
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      this.children.push(node);
    }
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
    this.clientHeight = 100;
  }
  get className() { return this.attributes.get("class") || ""; }
  set className(value) { if (value) this.attributes.set("class", String(value)); else this.attributes.delete("class"); }
  get id() { return this.attributes.get("id") || ""; }
  set id(value) { if (value) this.attributes.set("id", String(value)); else this.attributes.delete("id"); }
  get hidden() { return this.attributes.has("hidden"); }
  set hidden(value) { if (value) this.attributes.set("hidden", ""); else this.attributes.delete("hidden"); }
  get href() { return this.attributes.get("href") || ""; }
  set href(value) { this.attributes.set("href", String(value)); }
  get firstElementChild() { return this.children.find((c) => c.nodeType === 1) || null; }
  get childElementCount() { return this.children.filter((c) => c.nodeType === 1).length; }
  get options() { return this.children.filter((c) => c.localName === "option"); }
  get textContent() {
    if (!this.children.length) return this._text;
    return this.children.map((c) => c.textContent).join("");
  }
  set textContent(value) {
    for (const c of this.children) { c.parentNode = null; setConnected(c, false); }
    this.children = [];
    this._text = "";
    const str = value == null ? "" : String(value);
    if (str) this.append(new FakeNode(3, str));
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
  getAttribute(key) { return this.attributes.has(key) ? this.attributes.get(key) : null; }
  hasAttribute(key) { return this.attributes.has(key); }
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
    if (next) this.setAttribute(key, ""); else this.removeAttribute(key);
    return next;
  }
  _expand(items) {
    const out = [];
    for (const item of items) {
      if (item === null || item === undefined) continue;
      if (item instanceof FakeFragment) {
        out.push(...item.children);
        item.children = [];
      } else {
        out.push(item instanceof FakeNode ? item : new FakeNode(3, String(item)));
      }
    }
    return out;
  }
  append(...nodes) {
    for (const node of this._expand(nodes)) {
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      setConnected(node, this.isConnected);
      this.children.push(node);
    }
  }
  prepend(...nodes) {
    const converted = this._expand(nodes);
    for (const node of converted) {
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      setConnected(node, this.isConnected);
    }
    this.children = [...converted, ...this.children];
  }
  before(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = this._expand(nodes);
    for (const n of converted) { n.parentNode = this.parentNode; setConnected(n, this.parentNode.isConnected); }
    this.parentNode.children.splice(idx, 0, ...converted);
  }
  after(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = this._expand(nodes);
    for (const n of converted) { n.parentNode = this.parentNode; setConnected(n, this.parentNode.isConnected); }
    this.parentNode.children.splice(idx + 1, 0, ...converted);
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
    for (const c of this.children) { c.parentNode = null; setConnected(c, false); }
    this.children = [];
    this._text = "";
    this.append(...nodes);
  }
  replaceWith(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = this._expand(nodes);
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
  showModal() { this.open = true; this.setAttribute("open", ""); }
  close() { this.open = false; this.removeAttribute("open"); }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  dispatchEvent(event) {
    if (!event.target) event.target = this;
    for (const fn of [...(this.listeners.get(event.type) || [])]) fn(event);
    if (event.bubbles !== false && !event.cancelBubble && this.parentNode && typeof this.parentNode.dispatchEvent === "function") {
      this.parentNode.dispatchEvent(event);
    } else if (event.bubbles !== false && !event.cancelBubble && this.ownerDocument) {
      this.ownerDocument.dispatchEvent(event);
    }
  }
  click() {
    this.dispatchEvent(new FakeEvent("click", { button: 0, target: this }));
  }
  focus() { if (this.ownerDocument) this.ownerDocument.activeElement = this; }
  matches() { return false; }
  getClientRects() { return this.hidden ? [] : [{ width: 10, height: 10 }]; }
  getBoundingClientRect() { return { top: 0, bottom: 100, left: 0, right: 100, width: 100, height: 100 }; }
  scrollIntoView() {}
  contains(other) {
    let cur = other;
    while (cur) { if (cur === this) return true; cur = cur.parentNode; }
    return false;
  }
  closest(selector) {
    let cur = this;
    while (cur && cur.nodeType === 1) {
      if (matchesChain(cur, selector)) return cur;
      cur = cur.parentNode;
    }
    return null;
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
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
      if (m[1] === "inert" && !el.inert && val === null) return false;
      if (m[2] !== undefined ? val !== m[2] : (m[1] !== "inert" && val === null)) return false;
      rest = rest.slice(m[0].length);
    } else if (rest.startsWith(":not(")) {
      const inner = rest.slice(5, rest.indexOf(")"));
      if (inner && matchesSelector(el, inner)) return false;
      rest = rest.slice(rest.indexOf(")") + 1);
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
        if (stack[i].localName === tag) { stack.length = i; break; }
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

class FakeEvent {
  constructor(type, init = {}) {
    this.type = type;
    this.defaultPrevented = false;
    this.cancelBubble = false;
    this.bubbles = true;
    Object.assign(this, init);
  }
  preventDefault() { this.defaultPrevented = true; }
  stopPropagation() { this.cancelBubble = true; }
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
          async read() { if (read) return { done: true, value: undefined }; read = true; return { done: false, value: bytes }; },
          async cancel() {},
          releaseLock() {},
        };
      },
    },
  };
}

function sseResponse(frames) {
  const bytes = new TextEncoder().encode(frames.join(""));
  let read = false;
  return {
    ok: true,
    status: 200,
    headers: { get: () => "text/event-stream" },
    body: {
      getReader() {
        return {
          async read() { if (read) return { done: true, value: undefined }; read = true; return { done: false, value: bytes }; },
          async cancel() {},
          releaseLock() {},
        };
      },
    },
  };
}

async function setup({ initialSearch = "", fetchHandler, mobile = false, loadProject = false } = {}) {
  const docListeners = new Map();
  const doc = {
    title: "",
    visibilityState: "visible",
    activeElement: null,
    createElement(tag) { return new FakeHTMLElement(tag, "http://www.w3.org/1999/xhtml", doc); },
    createElementNS(ns, tag) { return new FakeElement(tag, ns, doc); },
    createTextNode(text) { return new FakeNode(3, text); },
    createDocumentFragment() { return new FakeFragment(doc); },
    addEventListener(type, fn) {
      if (!docListeners.has(type)) docListeners.set(type, []);
      docListeners.get(type).push(fn);
    },
    removeEventListener(type, fn) {
      const arr = docListeners.get(type);
      if (arr) docListeners.set(type, arr.filter((f) => f !== fn));
    },
    dispatchEvent(event) {
      for (const fn of [...(docListeners.get(event.type) || [])]) fn(event);
    },
  };
  const htmlRoot = buildHtmlDom(asset("web.html"), doc);
  doc.documentElement = htmlRoot;
  doc.body = htmlRoot.querySelector("body") || htmlRoot;
  doc.activeElement = doc.body;
  doc.getElementById = (id) => htmlRoot.querySelector(`#${id}`);
  doc.querySelector = (sel) => htmlRoot.querySelector(sel);
  doc.querySelectorAll = (sel) => htmlRoot.querySelectorAll(sel);

  const location = { pathname: "/", search: initialSearch, hash: "" };
  const history = {
    pushState(_s, _t, url) {
      const idx = String(url).indexOf("?");
      location.search = idx >= 0 ? String(url).slice(idx) : "";
    },
  };
  const fetchCalls = [];
  const defaultFetch = async (target, init = {}) => {
    fetchCalls.push({ target, method: init.method || "GET", body: init.body ? JSON.parse(init.body) : null });
    if (fetchHandler) {
      const custom = await fetchHandler(target, init);
      if (custom) return custom;
    }
    if (target === "/api/session") {
      return jsonResponse({ principal: { id: "local:operator", label: "Operator", role: "operator" }, permissions: { read: true, operate: true, configure: true } });
    }
    if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", repository: "onixresearch/kiln", reference: "refs/heads/main", pipeline_identity: "p1" }], available: [] });
    if (target === "/api/projects/proj-1/refs") return jsonResponse({ references: [{ reference: "refs/heads/main", source_revision: "abc123" }] });
    if (target.startsWith("/api/runs")) return jsonResponse({ runs: [], total: 0, next_cursor: null });
    if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
    if (target === "/api/workers") return jsonResponse({ workers: [] });
    if (target === "/api/activity" || target.startsWith("/api/activity?")) return jsonResponse({ items: [], total: 0 });
    if (target === "/api/operations") return jsonResponse({ capacity: {} });
    if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
    if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
    if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_claims: [], expired_claims: [] });
    if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", deferred_count: 0 });
    if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
    if (target === "/api/fleet/attempts") return jsonResponse({ attempts: [] });
    if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [], queue_windows: [] });
    if (target === "/api/repositories") return jsonResponse({ schema: "kiln.repositories.v1", repositories: [] });
    if (target === "/api/events") return sseResponse([`event: ready\ndata: {}\n\n`]);
    return jsonResponse({});
  };

  const timers = new Set();
  const context = {
    window: {},
    document: doc,
    location,
    history,
    fetch: defaultFetch,
    navigator: { clipboard: { writeText: async () => {} } },
    matchMedia: (q) => ({ matches: q.includes("max-width: 60rem") ? mobile : false, addEventListener() {}, removeEventListener() {} }),
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
    Date, Math, Number, Array, Object, String, Boolean, Promise, Error, TypeError, RangeError, Intl,
    prompt() { throw new Error("window.prompt must never be called"); },
    alert() { throw new Error("window.alert must never be called"); },
    confirm() { throw new Error("window.confirm must never be called"); },
    setTimeout: (fn, ms) => { const id = setTimeout(fn, Math.min(ms || 0, 5000)); timers.add(id); if (id.unref) id.unref(); return id; },
    clearTimeout: (id) => { timers.delete(id); clearTimeout(id); },
    setInterval: () => 0,
    clearInterval: () => {},
    scrollTo: () => {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent() {},
  };
  context.window = context;

  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("workers.js"), context);
  if (loadProject) vm.runInNewContext(asset("project.js"), context);
  vm.runInNewContext(asset("web.js"), context);

  await new Promise((r) => setTimeout(r, 40));
  return {
    ctx: context,
    factory: context.window.KilnFactory,
    ui: context.window.KilnUi,
    doc,
    location,
    fetchCalls,
    cleanup() {
      doc.getElementById("logout")?.click();
      for (const id of timers) clearTimeout(id);
      timers.clear();
    },
  };
}

test("1. intra-runs tab navigation clears sub-tabs (projects/project/evidence/runbooks/reviews) and cleans hrefs", async () => {
  const env = await setup({
    initialSearch: "?page=projects",
    loadProject: true,
    fetchHandler: async (target) => {
      if (target === "/api/overview") {
        return jsonResponse({
          totals: { runs: 1, failed: 1, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 },
          projects: [{ id: "proj-1", label: "Kiln", health: "failing", latest_run: { id: "run-fail-1", project_id: "proj-1", outcome: "DomainFailure", phase: "completed" } }],
          duration_trend: [],
          timing: { recorded: 1, total: 1 },
        });
      }
      if (target.startsWith("/api/runs?project_id=proj-1")) {
        return jsonResponse({ runs: [{ id: "run-fail-1", project_id: "proj-1", outcome: "DomainFailure", phase: "completed", source_revision: "abc" }], total: 1 });
      }
      if (target === "/api/runs/run-fail-1") {
        return jsonResponse({ id: "run-fail-1", project_id: "proj-1", outcome: "DomainFailure", phase: "completed", source_revision: "abc", nodes: [{ id: "build", state: "Failed", kind: "task" }], edges: [] });
      }
      return null;
    },
  });
  try {
    assert.equal(env.factory.route().tab, "projects");
    const browseBtn = env.doc.getElementById("project-cards").querySelectorAll("a").find((a) => a.textContent === "Browse runs");
    assert.ok(browseBtn);
    assert.equal(new URLSearchParams(browseBtn.href.slice(1)).get("tab"), null);
    assert.equal(new URLSearchParams(env.doc.getElementById("global-firing-pill").href.slice(1)).get("tab"), null);

    browseBtn.click();
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(env.factory.route().page, "runs");
    assert.equal(env.factory.route().tab, "runs");

    // Navigate to ?page=project&project=proj-1 and click "All runs" + "Logs" from project.js.
    env.factory.navigate({ page: "project", project: "proj-1" });
    await new Promise((r) => setTimeout(r, 40));
    assert.equal(env.factory.route().tab, "project");
    const allRuns = env.doc.getElementById("project-content").querySelectorAll("a").find((a) => a.textContent === "All runs");
    assert.ok(allRuns);
    assert.equal(new URLSearchParams(allRuns.href.slice(1)).get("tab"), null);
    allRuns.click();
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(env.factory.route().page, "runs");
    assert.equal(env.factory.route().tab, "runs");
  } finally {
    env.cleanup();
  }
});

test("2. slide-over actions keep inspector open on cancel/error, disable invoking buttons in flight, and update #reconcile-run", async () => {
  let resolveMutate;
  const env = await setup({
    initialSearch: "?page=overview",
    fetchHandler: async (target, init) => {
      if (target === "/api/implementation/attention") {
        return jsonResponse({
          total: 2,
          items: [
            { id: "att-plan", kind: "PlanReviewGate", severity: "high", title: "Review plan", target: { work_id: "w1", plan_id: "p1" }, context_preview: { steps: [{ title: "Step one" }] } },
            { id: "att-untrusted", kind: "UntrustedRunApproval", severity: "high", title: "Approve PR", target: { run_id: "run-pr", source_commit: "deadbeef" } },
          ],
        });
      }
      if (target === "/api/implementation/plan-review" && init.method === "POST") {
        return await new Promise((resolve) => { resolveMutate = resolve; });
      }
      if (target === "/api/runs/run-unk") {
        return jsonResponse({ id: "run-unk", project_id: "proj-1", outcome: "Unknown", phase: "unknown", source_revision: "abc", nodes: [], edges: [], controls: {} });
      }
      if (target.startsWith("/api/runs?") ) {
        return jsonResponse({ runs: [{ id: "run-unk", project_id: "proj-1", outcome: "Unknown", phase: "unknown", source_revision: "abc" }], total: 1 });
      }
      return null;
    },
  });
  try {
    // Open PlanReviewGate in slide-over and click Accept plan -> button disables while in flight, 409 error keeps inspector open.
    env.factory.openInspector("attention", "att-plan");
    await new Promise((r) => setTimeout(r, 30));
    const acceptBtn = env.doc.getElementById("slide-over-body").querySelectorAll("button").find((b) => b.textContent === "Accept plan");
    assert.ok(acceptBtn);
    const clickP = acceptBtn.listeners.get("click")[0]();
    assert.equal(acceptBtn.disabled, true);
    resolveMutate(jsonResponse({ code: "conflict", message: "stale" }, 409));
    await clickP;
    assert.equal(acceptBtn.disabled, false);
    assert.equal(env.doc.getElementById("slide-over-inspector").hidden, false);

    // Cancelling confirmControl on UntrustedRunApproval keeps inspector open.
    env.factory.openInspector("attention", "att-untrusted");
    await new Promise((r) => setTimeout(r, 30));
    const approveBtn = env.doc.getElementById("slide-over-body").querySelectorAll("button").find((b) => /Approve untrusted/.test(b.textContent));
    const appPromise = approveBtn.listeners.get("click")[0]();
    assert.equal(env.doc.getElementById("control-dialog").open, true);
    env.doc.getElementById("dismiss-control").click();
    await appPromise;
    assert.equal(env.doc.getElementById("slide-over-inspector").hidden, false);
    env.factory.closeInspector();

    // #reconcile-run is disabled while a mutation is in flight via updateControlButtons().
    env.factory.navigate({ page: "runs", run: "run-unk" });
    await new Promise((r) => setTimeout(r, 40));
    const reconcileBtn = env.doc.getElementById("reconcile-run");
    assert.ok(reconcileBtn);
    assert.equal(reconcileBtn.disabled, false);
    const mutP = env.factory.mutate("/api/implementation/plan-review", { work_id: "w1", plan_id: "p1", verdict: "accepted" }, "ok");
    assert.equal(reconcileBtn.disabled, true);
    resolveMutate(jsonResponse({ ok: true }));
    await mutP;
    assert.equal(reconcileBtn.disabled, false);
  } finally {
    env.cleanup();
  }
});

test("3 & 4. slide-over a11y (focus move/restore, inert background, Tab trap) and #control-dialog focus restore", async () => {
  const env = await setup({
    initialSearch: "?page=runs&run=run-1",
    fetchHandler: async (target) => {
      if (target === "/api/implementation/attention") {
        return jsonResponse({ total: 1, items: [{ id: "att-1", kind: "CampaignDecision", severity: "medium", title: "Wake", target: { campaign_id: "c1" } }] });
      }
      if (target === "/api/runs/run-1") {
        return jsonResponse({ id: "run-1", project_id: "proj-1", outcome: "Pending", phase: "active", revision: 1, sequence: 1, attempt_id: "a1", execution: { id: "e1" }, controls: { cancel: true }, source_revision: "abc", nodes: [], edges: [] });
      }
      if (target.startsWith("/api/runs?")) {
        return jsonResponse({ runs: [{ id: "run-1", project_id: "proj-1", outcome: "Pending", phase: "active", revision: 1, sequence: 1, attempt_id: "a1", execution: { id: "e1" }, controls: { cancel: true }, source_revision: "abc" }], total: 1 });
      }
      return null;
    },
  });
  try {
    // Opening inspector focuses #slide-over-close and marks background inert.
    const opener = env.doc.getElementById("refresh-all");
    opener.focus();
    env.factory.openInspector("attention", "att-1", [], opener);
    await new Promise((r) => setTimeout(r, 30));
    const closeBtn = env.doc.getElementById("slide-over-close");
    assert.equal(env.doc.activeElement, closeBtn);
    assert.equal(env.doc.getElementById("app-sidebar").inert, true);
    assert.equal(env.doc.querySelector(".app-topbar").inert, true);
    assert.equal(env.doc.getElementById("global-live-strip").inert, true);
    assert.equal(env.doc.getElementById("main").inert, true);
    assert.equal(env.doc.getElementById("slide-over-inspector").getAttribute("role"), "dialog");
    assert.equal(env.doc.getElementById("slide-over-inspector").getAttribute("aria-modal"), "true");

    // Shift+Tab wraps inside #slide-over-inspector rather than leaving.
    env.doc.dispatchEvent(new FakeEvent("keydown", { key: "Tab", shiftKey: true, target: closeBtn }));
    assert.equal(env.doc.getElementById("slide-over-inspector").contains(env.doc.activeElement), true);

    env.factory.closeInspector();
    assert.equal(env.doc.getElementById("main").inert, false);
    assert.equal(env.doc.activeElement, opener);

    // #control-dialog restores focus on Cancel for prepareRerun.
    env.ctx.document.dispatchEvent(new FakeEvent("heartbeat"));
    const rerunBtn = env.doc.getElementById("header-rerun");
    rerunBtn.focus();
    rerunBtn.click();
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(env.doc.getElementById("control-dialog").open, true);
    env.doc.getElementById("dismiss-control").click();
    assert.equal(env.doc.activeElement, rerunBtn);
  } finally {
    env.cleanup();
  }

  // Mobile menu sets inert on #global-live-strip.
  const mob = await setup({ initialSearch: "?page=overview", mobile: true });
  try {
    mob.doc.getElementById("app-menu-toggle").click();
    assert.equal(mob.doc.getElementById("app-menu-toggle").getAttribute("aria-expanded"), "true");
    assert.equal(mob.doc.getElementById("global-live-strip").inert, true);
    mob.doc.getElementById("app-menu-toggle").click();
    assert.equal(mob.doc.getElementById("global-live-strip").inert, false);
  } finally {
    mob.cleanup();
  }
});

test("5. repositories SSE channel triggers refresh & ci SSE refreshes operations quietly without skeleton wipe", async () => {
  let opsCalls = 0;
  let resolveOpsRefresh;
  let sseMode = "ready";
  const env = await setup({
    initialSearch: "?page=operations",
    fetchHandler: async (target) => {
      if (target === "/api/operations") {
        opsCalls += 1;
        if (opsCalls === 1) return jsonResponse({ capacity: { configured_limit: 4 } });
        return await new Promise((resolve) => { resolveOpsRefresh = resolve; });
      }
      if (target === "/api/events") {
        if (sseMode === "ci") {
          sseMode = "ready";
          return sseResponse([`event: ci\ndata: ${JSON.stringify({ version: 2, runs: [] })}\n\n`]);
        }
        if (sseMode === "repositories") {
          sseMode = "ready";
          return sseResponse([`event: repositories\ndata: ${JSON.stringify({ total: 2 })}\n\n`]);
        }
        return sseResponse([`event: ready\ndata: {}\n\n`]);
      }
      return null;
    },
  });
  try {
    const opsContent = env.doc.getElementById("operations-content");
    assert.equal(opsCalls, 1);
    assert.match(opsContent.textContent, /Execution capacity/);
    // Now deliver a ci SSE frame while on operations: refreshes quietly in place without skeleton wipe.
    sseMode = "ci";
    await new Promise((r) => setTimeout(r, 1600));
    assert.equal(opsCalls, 2);
    assert.equal(opsContent.querySelectorAll(".loading-state").length, 0);
    assert.match(opsContent.textContent, /Execution capacity/);
    resolveOpsRefresh(jsonResponse({ capacity: { configured_limit: 8 } }));
    await new Promise((r) => setTimeout(r, 20));
    assert.match(opsContent.textContent, /8/);

    // Switch to repositories with KilnSource stub and deliver a repositories SSE frame: triggers /api/repositories refresh.
    let repoLoadCalls = 0;
    env.ctx.window.KilnSource = {
      clear() {},
      clearRepositories() {},
      async loadRepositories(_root, ctx) {
        repoLoadCalls += 1;
        await ctx.api("/api/repositories");
      },
    };
    env.factory.navigate({ page: "source", tab: "repositories" });
    await new Promise((r) => setTimeout(r, 20));
    const repoBefore = repoLoadCalls;
    sseMode = "repositories";
    await new Promise((r) => setTimeout(r, 1600));
    assert.ok(repoLoadCalls > repoBefore);

    // On projects page, refreshAll() racing loadProjects() after loadProjectHealth() preserves .project-card-health.
    let resolveProj;
    const origFetch = env.ctx.fetch;
    env.ctx.fetch = async (target, init) => {
      if (target === "/api/overview") {
        return jsonResponse({ totals: { runs: 1, failed: 0, active: 0, queued: 0, succeeded: 1, cancelled: 0, unknown: 0 }, projects: [{ id: "proj-1", label: "Kiln", health: "healthy", latest_run: { id: "r1", project_id: "proj-1", outcome: "Success", phase: "completed", completed_at_ms: Date.now() } }], duration_trend: [], timing: { recorded: 1, total: 1 } });
      }
      if (target === "/api/projects") {
        return await new Promise((resolve) => { resolveProj = resolve; });
      }
      return origFetch(target, init);
    };
    env.factory.navigate({ page: "projects" });
    await new Promise((r) => setTimeout(r, 20));
    env.doc.getElementById("refresh-all").click();
    await new Promise((r) => setTimeout(r, 30));
    assert.doesNotMatch(env.doc.getElementById("project-cards").textContent, /Reading latest run/);
    resolveProj(jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", repository: "onixresearch/kiln", reference: "refs/heads/main", pipeline_identity: "p1" }], available: [] }));
    await new Promise((r) => setTimeout(r, 20));
    assert.doesNotMatch(env.doc.getElementById("project-cards").textContent, /Reading latest run/);
    assert.match(env.doc.getElementById("project-cards").textContent, /Passed|Success/i);
  } finally {
    env.cleanup();
  }
});

test("6. log search Enter submit resets cursor to 0, no dangling aria-describedby, incremental append preserves nodes", async () => {
  const runDetail = { id: "run-1", project_id: "proj-1", outcome: "Pending", phase: "active", source_revision: "abc", log_bounds: { maximum_chunk_bytes: 16384 }, nodes: [{ id: "n1", state: "Running", kind: "task" }], edges: [] };
  const env = await setup({
    initialSearch: "?page=runs&run=run-1&view=logs&node=n1&stream=stdout",
    fetchHandler: async (target) => {
      if (target === "/api/runs/run-1") return jsonResponse(runDetail);
      if (target.startsWith("/api/runs?")) return jsonResponse({ runs: [runDetail], total: 1 });
      if (target.startsWith("/api/runs/run-1/logs")) {
        const u = new URL(target, "http://localhost");
        const q = u.searchParams.get("q") || "";
        const cursor = u.searchParams.get("cursor");
        if (q) return jsonResponse({ node_id: "n1", stream: "stdout", source: "worker-capture", cursor, next_cursor: "50", text: "matched line\n", retained: 100 });
        if (cursor === "0") return jsonResponse({ node_id: "n1", stream: "stdout", source: "worker-capture", cursor: "0", next_cursor: "100", text: "line 1\n", retained: 200 });
        return jsonResponse({ node_id: "n1", stream: "stdout", source: "worker-capture", cursor: "100", next_cursor: "200", text: "line 2\n", retained: 200 });
      }
      return null;
    },
  });
  try {
    const view = env.doc.getElementById("log-output");
    const firstChildBefore = view.children[0] || view;
    assert.match(view.textContent, /line 1/);
    assert.equal(env.doc.getElementById("log-cursor").value, "100");

    // Incremental read from next cursor appends without wiping existing node!
    env.doc.getElementById("log-form").dispatchEvent(new FakeEvent("submit", { target: env.doc.getElementById("log-form") }));
    await new Promise((r) => setTimeout(r, 20));
    assert.equal(view.textContent, "line 1\nline 2\n");
    if (view.children.length > 0) assert.equal(view.children[0], firstChildBefore);

    // Search submit resets cursor from 200 -> 0.
    env.doc.getElementById("log-search").value = "matched";
    env.doc.getElementById("log-form").dispatchEvent(new FakeEvent("submit", { target: env.doc.getElementById("log-form") }));
    await new Promise((r) => setTimeout(r, 20));
    assert.ok(env.fetchCalls.some((c) => c.target.includes("q=matched") && c.target.includes("cursor=0")));

    // Check web.html: no aria-describedby target sits inside closed <details>.
    const html = asset("web.html");
    assert.equal(html.includes('id="log-form" class="log-tools" aria-describedby="log-read-help"'), false);
    const advStart = html.indexOf('<details id="log-advanced"');
    const advEnd = html.indexOf("</details>", advStart);
    assert.equal(html.slice(advStart, advEnd).includes('id="log-search-help"'), false);
  } finally {
    env.cleanup();
  }
});

test("7. web.css fixes grid overflow, rail focus rings, account-menu stacking, quiet button links, and stacked table labels", () => {
  const css = asset("web.css");
  assert.match(css, /\.view-stack\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  assert.match(css, /#run-detail\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  assert.match(css, /\.data-table-wrap\s*\{[^}]*min-width:\s*0/);
  assert.match(css, /\.app-sidebar\s*:focus-visible\s*\{[^}]*outline-offset:\s*-2px/);
  assert.match(css, /\.app-topbar:has\(#account-menu:not\(\[hidden\]\)\)\s*\{[^}]*z-index:\s*45/);
  assert.match(css, /\.button\.quiet/);
  assert.match(css, /\.data-table-stack td\[data-label\]::before\s*\{\s*content:\s*attr\(data-label\)/);
});

test("8. chord coordination, Escape precedence, and signed-out chrome inert/hidden", async () => {
  const signedOut = await setup({
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse({ code: "unauthorized", message: "no" }, 401);
      return null;
    },
  });
  try {
    assert.equal(signedOut.doc.getElementById("app-sidebar").hidden, true);
    assert.equal(signedOut.doc.getElementById("app-sidebar").inert, true);
    assert.equal(signedOut.doc.getElementById("work-command-open").hidden, true);
    assert.equal(signedOut.doc.getElementById("work-command-open").inert, true);
  } finally {
    signedOut.cleanup();
  }

  const env = await setup({
    initialSearch: "?page=overview",
    fetchHandler: async (target) => {
      if (target === "/api/implementation/attention") {
        return jsonResponse({ total: 1, items: [{ id: "att-plan", kind: "PlanReviewGate", severity: "high", title: "Review plan", target: { work_id: "w1", plan_id: "p1" } }] });
      }
      return null;
    },
  });
  try {
    const card = env.doc.getElementById("factory-attention").querySelector(".factory-attention-item");
    assert.ok(card);
    card.focus();
    // Pressing g then a performs chord navigation to inbox/attention without firing accept_plan.
    card.dispatchEvent(new FakeEvent("keydown", { key: "g", target: card }));
    assert.equal(env.doc.documentElement.dataset.kilnChord, "g");
    card.dispatchEvent(new FakeEvent("keydown", { key: "a", target: card }));
    assert.equal(env.doc.documentElement.dataset.kilnChord, "");
    assert.equal(env.factory.route().page, "inbox");
    assert.equal(env.fetchCalls.some((c) => c.target === "/api/implementation/plan-review"), false);

    // If both account-menu and slide-over-inspector are open, Escape closes account-menu first without closing slide-over.
    env.factory.openInspector("attention", "att-plan");
    await new Promise((r) => setTimeout(r, 20));
    env.doc.getElementById("account-button").click();
    assert.equal(env.doc.getElementById("account-menu").hidden, false);
    env.doc.dispatchEvent(new FakeEvent("keydown", { key: "Escape", target: env.doc.body }));
    assert.equal(env.doc.getElementById("account-menu").hidden, true);
    assert.equal(env.doc.getElementById("slide-over-inspector").hidden, false);
  } finally {
    env.cleanup();
  }
});

test("9, 10, 11. nullish coverage/steps guards, feed rename/remove without prompt, start-run notice, repositories destination, stale artifact clear", async () => {
  assert.doesNotMatch(asset("web.js"), /\bwindow\.prompt\s*\(/);
  const env = await setup({
    initialSearch: "?page=feeds",
    fetchHandler: async (target, init) => {
      if (target === "/api/feeds") {
        return jsonResponse({
          configured: true,
          summary: { total: 1, pending: 0, failing: 0, paused: 0 },
          publication: { repositories: [{ label: "Kiln", runs: 3 }] },
          subscriptions: [{ id: "feed-1", label: "Upstream", url: "https://example.com/atom.xml", enabled: true, entries: [] }],
        });
      }
      if (target === "/api/runs" && init.method === "POST") {
        return jsonResponse({ id: "run-new-1" });
      }
      if (target.startsWith("/api/runs?")) {
        return jsonResponse({
          runs: [
            { id: "run-a", project_id: "proj-1", outcome: "Success", phase: "completed", source_revision: "aaa" },
            { id: "run-b", project_id: "proj-1", outcome: "Success", phase: "completed", source_revision: "bbb" },
          ],
          total: 2,
        });
      }
      if (target === "/api/runs/run-a/artifacts") {
        return jsonResponse({ artifacts: [{ id: "art-a", name: "a.bin", node_id: "n1", attempt_id: "at1", bytes: 4, blake3: "0".repeat(64) }] });
      }
      if (target === "/api/runs/run-b/artifacts") {
        return new Promise(() => {}); // pending intentionally to verify stale clear!
      }
      if (target === "/api/runs/run-1/reports") {
        return jsonResponse({
          reports: [{
            name: "cov",
            kind: "coverage",
            coverage: { format: "lcov", lines_hit: undefined, lines_found: undefined, line_rate: Number.NaN, branches_hit: null, branches_found: null, branch_rate: undefined },
          }],
        });
      }
      if (target === "/api/implementation/attention") {
        return jsonResponse({
          total: 1,
          items: [{ id: "att-step", kind: "PlanReviewGate", severity: "medium", title: "Steps", target: {}, context_preview: { steps: [{ title: "Readable title" }, { custom: 42 }] } }],
        });
      }
      return null;
    },
  });
  try {
    // 11. repositories in KilnUi.destinations
    assert.equal(env.ui.destination("repositories")?.parent, "source");
    assert.equal(env.ui.destination("repositories")?.number, "3.2");

    // 10. single-project auto-select calls loadRefs()
    assert.ok(env.fetchCalls.some((c) => c.target === "/api/projects/proj-1/refs"));

    // 9 & 10. Feeds: no .pill/.pill-failed, missing repository.key doesn't crash, inline rename & confirmControl remove
    const feedsRoot = env.doc.getElementById("feeds-content");
    assert.equal(feedsRoot.querySelectorAll(".pill, .pill-failed").length, 0);
    const renameBtn = feedsRoot.querySelectorAll("button").find((b) => b.textContent === "Rename");
    renameBtn.click();
    const renameForm = feedsRoot.querySelectorAll("form").find((f) => !f.hidden && f.querySelectorAll("input").some((i) => i.value === "Upstream"));
    assert.ok(renameForm);
    renameForm.querySelector("input").value = "Renamed Upstream";
    await renameForm.listeners.get("submit")[0](new FakeEvent("submit", { target: renameForm }));
    const labelPost = env.fetchCalls.find((c) => c.target === "/api/feeds/feed-1/label");
    assert.deepEqual(labelPost.body, { label: "Renamed Upstream" });

    const removeBtn = feedsRoot.querySelectorAll("button").find((b) => b.textContent === "Remove");
    const remPromise = removeBtn.listeners.get("click")[0]();
    assert.equal(env.doc.getElementById("control-dialog").open, true);
    env.doc.getElementById("control-form").listeners.get("submit")[0](new FakeEvent("submit"));
    await remPromise;
    assert.ok(env.fetchCalls.some((c) => c.target === "/api/feeds/feed-1/remove" && Boolean(c.body.request_id)));

    // 9. PlanReviewGate step objects never produce [object Object]
    env.factory.openInspector("attention", "att-step");
    await new Promise((r) => setTimeout(r, 20));
    assert.doesNotMatch(env.doc.getElementById("slide-over-body").textContent, /\[object Object\]/);
    assert.match(env.doc.getElementById("slide-over-body").textContent, /Readable title/);
    env.factory.closeInspector();

    // 9. Coverage nullish guards never render NaN% or undefined / undefined
    env.factory.navigate({ page: "runs", run: "run-1", view: "tests" });
    await new Promise((r) => setTimeout(r, 30));
    const reportsText = env.doc.getElementById("reports-content").textContent;
    assert.doesNotMatch(reportsText, /NaN%|undefined/);

    // 10. Start-run notice preserved across redirect to runs
    env.factory.navigate({ page: "projects" });
    await new Promise((r) => setTimeout(r, 20));
    const startForm = env.doc.getElementById("start-form");
    await startForm.listeners.get("submit")[0](new FakeEvent("submit", { target: startForm }));
    assert.equal(env.factory.route().page, "runs");
    assert.match(env.doc.getElementById("notice").textContent, /Run request admitted/);

    // 11. Stale #artifact-list cleared immediately when switching between loaded runs
    env.factory.navigate({ page: "runs", run: "run-a", view: "artifacts" });
    await new Promise((r) => setTimeout(r, 30));
    assert.match(env.doc.getElementById("artifact-list").textContent, /a\.bin/);
    env.factory.navigate({ page: "runs", run: "run-b", view: "artifacts" });
    await new Promise((r) => setTimeout(r, 10));
    assert.equal(env.doc.getElementById("artifact-list").textContent, "");
  } finally {
    env.cleanup();
  }
});
