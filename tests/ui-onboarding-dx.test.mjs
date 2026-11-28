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
  get title() {
    return this.attributes.get("title") || "";
  }
  set title(value) {
    if (value) this.attributes.set("title", String(value));
    else this.attributes.delete("title");
  }
  get dateTime() {
    return this.attributes.get("datetime") || "";
  }
  set dateTime(value) {
    if (value) this.attributes.set("datetime", String(value));
    else this.attributes.delete("datetime");
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
  get nextElementSibling() {
    if (!this.parentNode) return null;
    const idx = this.parentNode.children.indexOf(this);
    for (let i = idx + 1; i < this.parentNode.children.length; i++) {
      if (this.parentNode.children[i].nodeType === 1) return this.parentNode.children[i];
    }
    return null;
  }
  get childElementCount() {
    return this.children.filter((c) => c.nodeType === 1).length;
  }
  get options() {
    return this.children.filter((c) => c.localName === "option");
  }
  get textContent() {
    return this.children.map((c) => c.textContent).join("");
  }
  set textContent(value) {
    for (const c of this.children) setConnected(c, false);
    this.children = [];
    this._text = "";
    const text = value == null ? "" : String(value);
    if (text) {
      const node = new FakeNode(3, text);
      node.parentNode = this;
      setConnected(node, this.isConnected);
      this.children.push(node);
    }
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
  after(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = nodes.filter((n) => n !== null && n !== undefined).map((n) => (n instanceof FakeNode ? n : new FakeNode(3, String(n))));
    for (const n of converted) {
      n.parentNode = this.parentNode;
      setConnected(n, this.parentNode.isConnected);
    }
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
  getClientRects() {
    return this.hidden ? [] : [{ width: 10, height: 10 }];
  }
  getBoundingClientRect() {
    return { top: 0, bottom: 100, left: 0, right: 100, width: 100, height: 100 };
  }
  scrollIntoView() {}
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

async function setup({ initialSearch = "", fetchHandler, nowMs, extraAssets = [] } = {}) {
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

  const location = { pathname: "/", search: initialSearch, hash: "" };
  const history = {
    pushState(_s, _t, url) {
      const idx = String(url).indexOf("?");
      location.search = idx >= 0 ? String(url).slice(idx) : "";
    },
  };

  const FakeDate = typeof nowMs === "number"
    ? class extends Date {
        constructor(...args) { super(...(args.length ? args : [nowMs])); }
        static now() { return nowMs; }
      }
    : Date;

  const timers = new Set();
  const context = {
    window: {},
    document: doc,
    location,
    history,
    fetch: async (target, init = {}) => {
      if (fetchHandler) return fetchHandler(target, init);
      if (target === "/api/session") {
        return jsonResponse({ principal: { id: "local:operator", label: "Operator", role: "operator" }, permissions: { read: true, operate: true, configure: true } });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", reference: "refs/heads/main" }], available: [] });
      if (target.startsWith("/api/runs")) return jsonResponse({ runs: [], total: 0, next_cursor: null });
      if (target === "/api/overview") return jsonResponse({ projects: [], totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, duration_trend: [], timing: { recorded: 0, total: 0 } });
      if (target === "/api/workers") return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { free_slots: 0, active: 0, configured_slots: 0, queued: 0 } });
      if (target === "/api/activity") return jsonResponse({ items: [], total: 0 });
      if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_claims: [], expired_claims: [] });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", deferred_count: 0 });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [], snapshot: {} });
      if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [], queue_windows: [] });
      return jsonResponse({});
    },
    navigator: { clipboard: { writeText: async () => {} } },
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
  for (const name of extraAssets) vm.runInNewContext(asset(name), context);
  vm.runInNewContext(asset("workers.js"), context);
  vm.runInNewContext(asset("web.js"), context);

  await new Promise((r) => setTimeout(r, 30));

  return {
    ui: context.window.KilnUi,
    doc,
    cleanup() {
      doc.getElementById("logout")?.click();
      for (const id of timers) clearTimeout(id);
      timers.clear();
    },
  };
}

test("0.0 Guide renders accurate 1.1 Factory description, 3-step New to Kiln? orientation, and full Plain-English Glossary", async () => {
  const env = await setup({ initialSearch: "?page=guide" });
  try {
    const guide = env.doc.getElementById("guide-content");
    const guideText = guide.textContent;

    // 1.1 Factory description must be updated (stale copy gone, clarifies Runs & Evidence).
    assert.doesNotMatch(guideText, /Failing projects come first\. Each card opens the exact failed run at its logs\./);
    assert.match(env.ui.destination("overview").purpose, /escalations, delivery spine, speculative merge train, machine slots and expiring claims/i);
    assert.match(env.ui.destination("overview").purpose, /2\.2 Runs & Evidence/);
    assert.match(guideText, /Live Factory cockpit: escalations, delivery spine, speculative merge train, machine slots and expiring claims/);

    // 3-step "New to Kiln? 3 things to try first" strip at top of Guide.
    const onboarding = guide.querySelector(".guide-onboarding");
    assert.ok(onboarding, "Guide must render .guide-onboarding orientation strip");
    assert.equal(guide.children[0], onboarding, "Orientation strip sits at top of 0.0 Guide");
    assert.match(onboarding.textContent, /New to Kiln\? 3 things to try first/);
    const cards = onboarding.querySelectorAll(".guide-try-card");
    assert.equal(cards.length, 3);
    assert.match(cards[0].textContent, /Run or inspect a CI build/);
    assert.match(cards[0].textContent, /2\.2 Runs & Evidence/);
    assert.match(cards[0].textContent, /CI projects/);
    assert.match(cards[1].textContent, /Review a change & its required checks/);
    assert.match(cards[1].textContent, /2\.3 Code Review & Queue/);
    assert.match(cards[2].textContent, /Monitor machines, claims & escalations/);
    assert.match(cards[2].textContent, /1\.1 Factory/);
    assert.match(cards[2].textContent, /3\.1 Fleet & Workers/);

    // Full Plain-English Glossary.
    const glossary = guide.querySelector(".guide-glossary");
    assert.ok(glossary, "Guide must render .guide-glossary");
    const terms = glossary.querySelectorAll("dt").map((node) => node.textContent);
    const definitions = glossary.querySelectorAll("dd").map((node) => node.textContent);
    const expectedTerms = [
      "Cairn",
      "Lattice",
      "Mantle",
      "Aspen / Molten",
      "Herdr",
      "Basalt / UCAN",
      "COB",
      "Fence",
      "Admitted",
      "Canonical",
      "Candidate-set identity",
      "Projection",
      "Receipt",
    ];
    assert.deepEqual(terms, expectedTerms);
    for (let i = 0; i < expectedTerms.length; i++) {
      assert.ok(definitions[i].trim().length > 20, `Glossary definition for ${expectedTerms[i]} must be non-empty`);
    }
  } finally {
    env.cleanup();
  }
});

test("Shell navigation links and destination tabs carry plain-English title tooltips", async () => {
  const env = await setup({ initialSearch: "?page=runs&tab=reviews" });
  try {
    const railLinks = env.doc.querySelectorAll("#app-sidebar a[data-page]");
    assert.ok(railLinks.length >= 9);
    for (const link of railLinks) {
      assert.ok(link.title.trim().length > 15, `Rail link ${link.dataset.page} must carry a plain-English title tooltip`);
    }
    const byPage = new Map(railLinks.map((link) => [link.dataset.page, link.title]));
    assert.match(byPage.get("cairn"), /Cairn/);
    assert.match(byPage.get("fleet"), /Fleet coordinator/);

    // Check Runs & Evidence sub-navigation tabs (including Patchgate receipts and Runbooks).
    const runsTabs = env.doc.querySelectorAll("#destination-tabs a");
    const tabTooltips = new Map(runsTabs.map((tab) => [tab.textContent.trim(), tab.title]));
    assert.match(tabTooltips.get("Patchgate receipts") || "", /Patchgate/);
    assert.match(tabTooltips.get("Runbooks") || "", /runbook/i);
    assert.match(tabTooltips.get("CI projects") || "", /CI repositories|source-bound run/i);
  } finally {
    env.cleanup();
  }
});

test("Project summary cards and recent activity render clean relative <time> with full UTC in title/datetime", async () => {
  const nowMs = Date.parse("2026-11-27T22:09:44.702Z");
  const requestedMs = Date.parse("2026-11-27T03:09:44.702Z"); // 19h ago
  const env = await setup({
    initialSearch: "?page=projects",
    nowMs,
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({ principal: { id: "local:operator", label: "Operator", role: "operator" }, permissions: { read: true, operate: true } });
      }
      if (target === "/api/projects") {
        return jsonResponse({
          enrolled: [
            { id: "proj-queued", label: "Kiln Queued", reference: "refs/heads/main", repository: "https://example/kiln.git", pipeline_identity: "pipe-1" },
            { id: "proj-untimed", label: "Kiln Untimed", reference: "refs/heads/next", repository: "https://example/kiln.git", pipeline_identity: "pipe-2" },
          ],
          available: [],
        });
      }
      if (target === "/api/overview") {
        return jsonResponse({
          totals: { runs: 1, failed: 0, active: 0, queued: 1, succeeded: 0, cancelled: 0, unknown: 0 },
          projects: [
            {
              id: "proj-queued",
              label: "Kiln Queued",
              health: "queued",
              latest_run: { id: "run-q1", outcome: "Pending", phase: "queued", created_at_ms: requestedMs },
            },
            {
              id: "proj-untimed",
              label: "Kiln Untimed",
              health: "healthy",
              latest_run: { id: "run-u1", outcome: "Success", phase: "completed", completed_at_ms: null },
            },
          ],
          duration_trend: [],
          timing: { recorded: 0, total: 1 },
        });
      }
      return jsonResponse({});
    },
  });
  try {
    const cards = env.doc.querySelectorAll("#project-cards .project-card");
    assert.equal(cards.length, 2);

    // Queued project card: relative time only in DOM text, exact local + UTC in title & datetime.
    const queuedHealth = cards[0].querySelector(".project-card-health");
    assert.match(queuedHealth.textContent, /Requested 19h ago/);
    assert.doesNotMatch(queuedHealth.textContent, /2026-11-27T03:09:44\.702Z/, "Card textContent must not cram inline UTC timestamp");
    const timeEl = queuedHealth.querySelector("time.relative-time");
    assert.ok(timeEl, "Project card must render a <time> element");
    assert.equal(timeEl.textContent, "19h ago");
    assert.equal(timeEl.dateTime, "2026-11-27T03:09:44.702Z");
    assert.match(timeEl.title, /2026-11-27T03:09:44\.702Z/);

    // Negative check: null timestamp stays explicitly "Not recorded".
    const untimedHealth = cards[1].querySelector(".project-card-health");
    assert.match(untimedHealth.textContent, /Finished Not recorded/);
  } finally {
    env.cleanup();
  }
});

test("Overview recent activity keeps timestamps clean while Run Detail exact facts keep full UTC + local text", async () => {
  const nowMs = Date.parse("2026-11-27T22:09:44.702Z");
  const eventMs = Date.parse("2026-11-27T20:09:44.702Z"); // 2h ago
  const env = await setup({
    initialSearch: "?page=overview",
    nowMs,
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({ principal: { id: "local:operator", label: "Operator", role: "operator" }, permissions: { read: true, operate: true } });
      }
      if (target === "/api/projects") {
        return jsonResponse({ enrolled: [{ id: "proj-1", label: "Kiln", reference: "refs/heads/main" }], available: [] });
      }
      if (target === "/api/overview") {
        return jsonResponse({
          totals: { runs: 1, failed: 0, active: 0, queued: 0, succeeded: 1, cancelled: 0, unknown: 0 },
          projects: [],
          duration_trend: [],
          timing: { recorded: 0, total: 0 },
        });
      }
      if (target === "/api/workers") {
        return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { free_slots: 0, active: 0, configured_slots: 0, queued: 0 } });
      }
      if (target.startsWith("/api/activity")) {
        return jsonResponse({
          total: 1,
          events: [],
          items: [{ kind: "event", project_id: "proj-1", recorded_at_ms: eventMs, event: { RunStarted: {} } }],
        });
      }
      if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_claims: [], expired_claims: [] });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", deferred_count: 0 });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [], snapshot: {} });
      if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [], queue_windows: [] });
      return jsonResponse({});
    },
  });
  try {
    const feed = env.doc.getElementById("overview-activity");
    const timeEl = feed.querySelector("time.relative-time");
    assert.ok(timeEl);
    assert.equal(timeEl.textContent, "2h ago");
    assert.equal(timeEl.dateTime, "2026-11-27T20:09:44.702Z");
    assert.match(timeEl.title, /2026-11-27T20:09:44\.702Z/);
    assert.doesNotMatch(feed.textContent, /2026-11-27T20:09:44\.702Z/);
  } finally {
    env.cleanup();
  }
});

test("web.css styles Guide orientation strip and glossary cleanly for narrow viewports", () => {
  const css = asset("web.css");
  assert.match(css, /\.guide-try-list\s*\{[^}]*grid-template-columns:/);
  assert.match(css, /\.guide-try-card\s*\{[^}]*min-width:\s*0/);
  assert.match(css, /\.guide-try-title\s*\{[^}]*overflow-wrap:\s*anywhere/);
  assert.match(css, /\.guide-terms\s*>\s*dt\s*\{[^}]*overflow-wrap:\s*anywhere/);
});

test("System & Source subnav hides Feeds tab when feeds_configured is false or absent, while direct ?page=feeds still renders", async () => {
  const hiddenEnv = await setup({
    initialSearch: "?page=source",
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: false,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      return jsonResponse({});
    },
  });
  try {
    const tabIds = hiddenEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.ok(tabIds.includes("source"));
    assert.ok(tabIds.includes("automation"));
    assert.equal(tabIds.includes("feeds"), false);
  } finally {
    hiddenEnv.cleanup();
  }

  const directEnv = await setup({
    initialSearch: "?page=feeds",
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: false,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/feeds") return jsonResponse({ configured: false, subscriptions: [] });
      return jsonResponse({});
    },
  });
  try {
    const tabIds = directEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.equal(tabIds.includes("feeds"), false);
    assert.match(directEnv.doc.getElementById("feeds-content").textContent, /Feeds are not configured/);
  } finally {
    directEnv.cleanup();
  }
});

test("System & Source subnav shows Feeds tab when feeds_configured is true", async () => {
  const env = await setup({
    initialSearch: "?page=source",
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: true,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      return jsonResponse({});
    },
  });
  try {
    const tabIds = env.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.ok(tabIds.includes("feeds"));
  } finally {
    env.cleanup();
  }
});

test("1.1 Factory renders recent activity from items[] instead of showing Activity unavailable", async () => {
  const env = await setup({
    initialSearch: "?page=overview",
    fetchHandler: async (target) => {
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: false,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/overview") {
        return jsonResponse({
          totals: { runs: 1, failed: 0, active: 0, queued: 0, succeeded: 1, cancelled: 0, unknown: 0 },
          projects: [],
          duration_trend: [],
          timing: { recorded: 0, total: 0 },
        });
      }
      if (target === "/api/workers") {
        return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { free_slots: 1, active: 0, configured_slots: 1, queued: 0 } });
      }
      if (target.startsWith("/api/activity")) {
        return jsonResponse({
          total: 1,
          bounded: true,
          items: [
            {
              kind: "audit",
              ordinal: 42,
              recorded_at_ms: 1795800000000,
              audit: { actor_label: "Operator", method: "POST", target: "/api/runs", decision: "accepted", status: 200 },
            },
          ],
        });
      }
      if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0, counts: {} });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [] });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ enabled: true, last_tick: { deferral_counts: { lattice: 5 } } });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
      if (target === "/api/fleet/attempts") return jsonResponse({ attempts: [] });
      if (target === "/api/code-review") return jsonResponse({ changes: [] });
      if (target === "/api/repositories") return jsonResponse({ repositories: [] });
      return jsonResponse({});
    },
  });
  try {
    const text = env.doc.getElementById("overview-activity").textContent;
    assert.doesNotMatch(text, /Activity unavailable/);
    assert.match(text, /POST \/api\/runs/);
  } finally {
    env.cleanup();
  }
});

test("Capability-aware subnav hides unconfigured and secondary tabs while keeping direct URLs reachable", async () => {
  const unconfiguredSession = {
    principal: { id: "local:operator", label: "Operator", role: "operator" },
    permissions: { read: true, operate: true, configure: true },
    feeds_configured: false,
    capabilities: { feeds: false, runbooks: false, inventory: false, campaign_controls: false, automation: false },
  };
  const makeFetch = (session = unconfiguredSession) => async (target) => {
    if (target === "/api/session") return jsonResponse(session);
    if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
    if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
    return jsonResponse({});
  };

  // Runs & Evidence: hides runbooks, reviews, cobs.
  const runsEnv = await setup({ initialSearch: "?page=runs", fetchHandler: makeFetch() });
  try {
    const tabIds = runsEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.deepEqual(tabIds, ["runs", "projects"]);
  } finally {
    runsEnv.cleanup();
  }

  // Direct deep-link to hidden tab still works and displays itself in subnav.
  const directReviewsEnv = await setup({ initialSearch: "?page=runs&tab=reviews", fetchHandler: makeFetch() });
  try {
    const tabIds = directReviewsEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.ok(tabIds.includes("reviews"));
  } finally {
    directReviewsEnv.cleanup();
  }

  // Delivery & Cairn: hides planning, workviews, evidence.
  const cairnEnv = await setup({ initialSearch: "?page=cairn", fetchHandler: makeFetch() });
  try {
    const tabIds = cairnEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.deepEqual(tabIds, ["cairn", "work"]);
  } finally {
    cairnEnv.cleanup();
  }

  // Attention: hides single-tab bar completely.
  const inboxEnv = await setup({ initialSearch: "?page=inbox", fetchHandler: makeFetch() });
  try {
    assert.equal(inboxEnv.doc.getElementById("destination-tabs").hidden, true);
    assert.equal(inboxEnv.doc.querySelectorAll("#destination-tabs a").length, 0);
  } finally {
    inboxEnv.cleanup();
  }

  // System & Source: puts repositories first, hides automation and feeds when unconfigured.
  const sourceEnv = await setup({ initialSearch: "?page=source", fetchHandler: makeFetch() });
  try {
    const tabIds = sourceEnv.doc.querySelectorAll("#destination-tabs a").map((tab) => tab.dataset.tab);
    assert.deepEqual(tabIds, ["repositories", "source", "activity", "operations"]);
  } finally {
    sourceEnv.cleanup();
  }

  // Projects viewer note hidden for operators, shown for viewers.
  const opProjEnv = await setup({ initialSearch: "?page=projects", fetchHandler: makeFetch(unconfiguredSession) });
  try {
    assert.equal(opProjEnv.doc.getElementById("projects-viewer-note").hidden, true);
  } finally {
    opProjEnv.cleanup();
  }
  const viewerProjEnv = await setup({
    initialSearch: "?page=projects",
    fetchHandler: makeFetch({ ...unconfiguredSession, principal: { id: "local:viewer", label: "Viewer", role: "viewer" }, permissions: { read: true, operate: false, configure: false } }),
  });
  try {
    assert.equal(viewerProjEnv.doc.getElementById("projects-viewer-note").hidden, false);
  } finally {
    viewerProjEnv.cleanup();
  }
});

test("Fleet and Workers skip unconfigured 503 inventory/campaign endpoints and collapse secondary Fleet panels", async () => {
  const calls = [];
  const capabilities = { feeds: false, runbooks: false, inventory: false, campaign_controls: false, automation: false };

  // Exercise Workers directly with real KilnInventory loaded.
  const workersEnv = await setup({
    initialSearch: "?page=workers",
    extraAssets: ["inventory.js"],
    fetchHandler: async (target) => {
      calls.push(target);
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: false,
          capabilities,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/workers") {
        return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { free_slots: 0, active: 0, configured_slots: 0, queued: 0 } });
      }
      if (target === "/api/inventory" || target === "/api/implementation/campaign-definitions") {
        throw new Error(`Unexpected unconfigured call to ${target}`);
      }
      return jsonResponse({});
    },
  });
  try {
    assert.equal(calls.includes("/api/inventory"), false, "Workers must not call /api/inventory when capabilities.inventory is false");
    assert.equal(calls.includes("/api/implementation/campaign-definitions"), false, "Workers must not call campaign-definitions when capabilities.campaign_controls is false");
    const workersContent = workersEnv.doc.getElementById("workers-content");
    assert.equal(workersContent.querySelector("[data-inventory-root]"), null);
    assert.equal(workersContent.querySelector("details.workers-campaign-definitions"), null);
  } finally {
    workersEnv.cleanup();
  }

  // Exercise Fleet directly with real KilnInventory, KilnFleetRuntime, KilnFleetCapacity, and KilnFleet loaded.
  const fleetCalls = [];
  const fleetEnv = await setup({
    initialSearch: "?page=fleet",
    extraAssets: ["inventory.js", "fleet-runtime.js", "fleet-capacity.js", "fleet.js"],
    fetchHandler: async (target) => {
      fleetCalls.push(target);
      if (target === "/api/session") {
        return jsonResponse({
          principal: { id: "local:operator", label: "Operator", role: "operator" },
          permissions: { read: true, operate: true, configure: true },
          feeds_configured: false,
          capabilities,
        });
      }
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/inventory" || target === "/api/implementation/campaign-definitions") {
        throw new Error(`Unexpected unconfigured call to ${target}`);
      }
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target === "/api/implementation/work") return jsonResponse({ items: [] });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ enabled: true, last_tick: { deferral_counts: {} } });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [] });
      if (target === "/api/fleet/work") return jsonResponse({ ready_unclaimed: 0, ranked: [], blocked: [] });
      if (target === "/api/workers") return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { targets: 0, enrolled_slots: 0, configured_slots: 0, active: 0, free_slots: 0, queued: 0 } });
      if (target === "/api/implementation/admission") return jsonResponse({ schema: "kiln.implementation-admission.v1", configured: false, bounded: true, policy: null, accounts: [], serving: [], attempt_metrics: [] });
      return jsonResponse({});
    },
  });
  try {
    assert.equal(fleetCalls.includes("/api/inventory"), false, "Fleet must not call /api/inventory when capabilities.inventory is false");
    assert.equal(fleetCalls.includes("/api/implementation/campaign-definitions"), false, "Fleet must not call campaign-definitions when capabilities.campaign_controls is false");
    const fleetContent = fleetEnv.doc.getElementById("fleet-content");
    const disclosures = fleetContent.querySelectorAll("details.fleet-secondary-disclosure");
    const byKey = new Map(disclosures.map((d) => [d.dataset.fleetDisclosure, d]));
    for (const key of ["decisions", "swarms", "cairn", "issues", "guest-move"]) {
      assert.ok(byKey.has(key), `Fleet should wrap ${key} in a secondary disclosure`);
      assert.equal(byKey.get(key).open, false, `Fleet secondary disclosure ${key} should start collapsed`);
    }
    assert.equal(byKey.get("campaigns").hidden, true, "Fleet campaigns disclosure should be hidden when campaign_controls is false");
    assert.ok(byKey.has("admission"), "Unconfigured fair-admission should collapse into secondary disclosure");
    assert.equal(byKey.get("admission").open, false);
    assert.ok(byKey.has("targets"), "Duplicate configured execution targets should collapse into secondary disclosure");
    assert.equal(byKey.get("targets").open, false);
    assert.ok(byKey.has("placement"), "Placement controls should collapse into secondary disclosure");
    assert.equal(byKey.get("placement").open, false);
    const acquireDisclosure = fleetContent.querySelector("details.fleet-acquire-disclosure");
    assert.ok(acquireDisclosure, "Acquire claim form should sit in a collapsed disclosure");
    assert.equal(acquireDisclosure.open, false);
  } finally {
    fleetEnv.cleanup();
  }
});

test("Pass 2 pruning: Factory, Guide, Projects, Cairn authoring, and Source/Repositories default cleanly", async () => {
  const capabilities = { feeds: false, runbooks: false, inventory: false, campaign_controls: false, automation: false, cairn_authoring: false };
  const baseSession = {
    principal: { id: "local:operator", label: "Operator", role: "operator" },
    permissions: { read: true, operate: true, configure: true },
    feeds_configured: false,
    capabilities,
  };

  // Factory hides unconfigured Swarm mirror, empty merge train strip, and duplicate #connection-status.
  const factoryEnv = await setup({
    initialSearch: "?page=overview",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
      if (target.startsWith("/api/activity")) return jsonResponse({ total: 0, bounded: true, items: [] });
      if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_mirror: { status: "unconfigured" } });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", last_tick: { deferred_count: 0 } });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
      if (target === "/api/fleet/attempts") return jsonResponse({ attempts: [] });
      if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [] });
      if (target === "/api/repositories") return jsonResponse({ repositories: [] });
      return jsonResponse({});
    },
  });
  try {
    assert.doesNotMatch(factoryEnv.doc.getElementById("factory-ribbon").textContent, /Swarm mirror/);
    assert.equal(factoryEnv.doc.getElementById("factory-merge-train").hidden, true);
    assert.equal(factoryEnv.doc.getElementById("connection-status").hidden, true);
  } finally {
    factoryEnv.cleanup();
  }

  // Guide omits redundant Start here and Words Kiln uses sections.
  const guideEnv = await setup({ initialSearch: "?page=guide" });
  try {
    const guideText = guideEnv.doc.getElementById("guide-content").textContent;
    assert.doesNotMatch(guideText, /Words Kiln uses/);
    assert.equal(guideEnv.doc.getElementById("guide-start-here"), null);
  } finally {
    guideEnv.cleanup();
  }

  // CI Projects hides Enroll panel when no unenrolled allowlisted repos exist.
  const projEnv = await setup({
    initialSearch: "?page=projects",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [{ id: "p1", label: "Kiln", reference: "refs/heads/main" }], available: [] });
      if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
      return jsonResponse({});
    },
  });
  try {
    assert.equal(projEnv.doc.getElementById("enroll-panel").hidden, true);
  } finally {
    projEnv.cleanup();
  }

  // System & Source defaults to Repositories tab and collapses import form when repos already exist.
  const repoEnv = await setup({
    initialSearch: "?page=source",
    extraAssets: ["source.js"],
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/repositories") {
        return jsonResponse({
          repositories: [{ identity: "a".repeat(64), slug: "OnixResearch/kiln", label: "Kiln", status: "synced", remotes: [], local_copies: [], active_cairns: 0 }],
          summary: { total: 1, total_local_copies: 1, total_cairns: 0, synced: 1, diverged: 0, dirty: 0, unknown: 0, local_only: 0 },
        });
      }
      if (target === "/api/sources") return jsonResponse({ sources: [] });
      return jsonResponse({});
    },
  });
  try {
    const activeTab = repoEnv.doc.querySelector('#destination-tabs a[aria-current="page"]');
    assert.equal(activeTab?.dataset?.tab, "repositories");
    const importDisclosure = repoEnv.doc.querySelectorAll("details.fleet-secondary-disclosure").find((d) => d.dataset.repoDisclosure === "import");
    assert.ok(importDisclosure, "Import form should collapse when repositories already exist");
    assert.equal(importDisclosure.open, false);
  } finally {
    repoEnv.cleanup();
  }

  // Cairn omits nativeAuthoringPanel when capabilities.cairn_authoring is false.
  const cairnEnv = await setup({
    initialSearch: "?page=cairn",
    extraAssets: ["cairn.js"],
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/implementation/cairn") {
        return jsonResponse({ schema: "kiln.cairn-workspace-index.v2", snapshot: { observed_at_ms: 1795800000000 }, repositories: [], changes: [], bounded: true, truncated: false });
      }
      return jsonResponse({});
    },
  });
  try {
    assert.equal(cairnEnv.doc.querySelector("details.cairn-authoring"), null);
  } finally {
    cairnEnv.cleanup();
  }
});

test("Pass 3 pruning: healthy Factory ribbon deduplicates live strip, Operations collapses policy, Runs hides sort note, Code Review hides off Merge Queue tab", async () => {
  const baseSession = {
    principal: { id: "local:operator", label: "Operator", role: "operator" },
    permissions: { read: true, operate: true, configure: true },
    feeds_configured: false,
    capabilities: { feeds: false, runbooks: false, inventory: false, campaign_controls: false, automation: false, cairn_authoring: false },
  };

  // Factory ribbon hides Escalations & Claims <15m when both are zero.
  const healthyFactory = await setup({
    initialSearch: "?page=overview",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
      if (target.startsWith("/api/activity")) return jsonResponse({ total: 0, bounded: true, items: [] });
      if (target === "/api/implementation/attention") return jsonResponse({ items: [], total: 0 });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [], foreign_claims: [], foreign_mirror: { status: "unconfigured" } });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ state: "running", last_tick: { deferred_count: 0 } });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
      if (target === "/api/fleet/attempts") return jsonResponse({ attempts: [] });
      if (target.startsWith("/api/code-review")) return jsonResponse({ changes: [] });
      if (target === "/api/repositories") return jsonResponse({ repositories: [] });
      return jsonResponse({});
    },
  });
  try {
    const ribbonText = healthyFactory.doc.getElementById("factory-ribbon").textContent;
    assert.match(ribbonText, /Dispatch/);
    assert.match(ribbonText, /Host slot occupancy/);
    assert.match(ribbonText, /Repositories & Fleet Sync/);
    assert.doesNotMatch(ribbonText, /Escalations/);
    assert.doesNotMatch(ribbonText, /Claims <15m/);
  } finally {
    healthyFactory.cleanup();
  }

  // Operations collapses Retention & access policy into a closed disclosure.
  const opsEnv = await setup({
    initialSearch: "?page=source&tab=operations",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/operations") {
        return jsonResponse({
          storage_failed: false,
          warnings: [],
          capacity: { active_runs: 0, queued_runs: 0 },
          queue: [],
          providers: [],
          usage: [],
          retention_policy: { max_runs: 500 },
          access_policy_identity: "b".repeat(64),
        });
      }
      return jsonResponse({});
    },
  });
  try {
    const policyDisclosure = opsEnv.doc.querySelectorAll("details.fleet-secondary-disclosure").find((d) => d.dataset.opsDisclosure === "policy");
    assert.ok(policyDisclosure, "Operations should collapse Retention & access policy");
    assert.equal(policyDisclosure.open, false);
  } finally {
    opsEnv.cleanup();
  }

  // Runs hides #run-sort-note when unsorted and unpaginated.
  const runsEnv = await setup({
    initialSearch: "?page=runs",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target.startsWith("/api/runs")) return jsonResponse({ runs: [], total: 0, next_cursor: null });
      return jsonResponse({});
    },
  });
  try {
    assert.equal(runsEnv.doc.getElementById("run-sort-note").hidden, true);
  } finally {
    runsEnv.cleanup();
  }

  // Code Review hides Merge Queue tab when queue_enabled is false.
  const crEnv = await setup({
    initialSearch: "?page=code-review",
    extraAssets: ["code-review.js"],
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target.startsWith("/api/code-review")) {
        return jsonResponse({
          schema: "kiln.code-review.v1",
          configured: true,
          principal: "local:operator",
          can_write: true,
          queue_enabled: false,
          total: 0,
          changes: [],
          queue: [],
          queue_windows: [],
          non_claims: [],
        });
      }
      return jsonResponse({});
    },
  });
  try {
    const crText = crEnv.doc.getElementById("code-review-content").textContent;
    assert.doesNotMatch(crText, /Merge Queue/);
  } finally {
    crEnv.cleanup();
  }
});

test("Pass 4 nonsense removal: zero about-view/boundary clutter in shell, Fleet hides dead disclosures, live strip auto-hides when idle, Cairn & Repos hide zero chips", async () => {
  const html = asset("web.html");
  assert.doesNotMatch(html, /class="about-view"/);
  assert.doesNotMatch(html, /menu-boundary/);
  assert.doesNotMatch(html, /Recorded evidence is not acceptance authority/);

  const baseSession = {
    principal: { id: "local:operator", label: "Operator", role: "operator" },
    permissions: { read: true, operate: true, configure: true },
    feeds_configured: false,
    capabilities: { feeds: false, runbooks: false, inventory: false, campaign_controls: false, automation: false, cairn_authoring: false },
  };

  // Fleet hides empty/duplicate/unconfigured disclosures completely (hidden=true).
  const fleetEnv = await setup({
    initialSearch: "?page=fleet",
    extraAssets: ["inventory.js", "fleet-runtime.js", "fleet-capacity.js", "fleet.js"],
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/fleet/machines") return jsonResponse({ machines: [] });
      if (target === "/api/implementation/work") return jsonResponse({ items: [] });
      if (target === "/api/implementation/cairn") return jsonResponse({ changes: [] });
      if (target === "/api/fleet/dispatch/status") return jsonResponse({ enabled: true, last_tick: { deferral_counts: {} } });
      if (target.startsWith("/api/fleet/claims")) return jsonResponse({ claims: [] });
      if (target === "/api/fleet/work") return jsonResponse({ ready_unclaimed: 0, ranked: [], blocked: [] });
      if (target === "/api/workers") return jsonResponse({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, workers: [], summary: { targets: 0, enrolled_slots: 0, configured_slots: 0, active: 0, free_slots: 0, queued: 0 } });
      if (target === "/api/implementation/admission") return jsonResponse({ schema: "kiln.implementation-admission.v1", configured: false, bounded: true, policy: null, accounts: [], serving: [], attempt_metrics: [] });
      return jsonResponse({});
    },
  });
  try {
    const fleetContent = fleetEnv.doc.getElementById("fleet-content");
    const byKey = new Map(fleetContent.querySelectorAll("details.fleet-secondary-disclosure").map((d) => [d.dataset.fleetDisclosure, d]));
    for (const key of ["decisions", "swarms", "cairn", "issues", "campaigns", "guest-move", "admission", "targets", "placement"]) {
      assert.equal(byKey.get(key)?.hidden, true, `Fleet ${key} disclosure should be hidden when empty/duplicate/unconfigured`);
    }
  } finally {
    fleetEnv.cleanup();
  }

  // CI projects hides reference/revision inputs when source-mode is latest.
  const projEnv = await setup({
    initialSearch: "?page=projects",
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/overview") return jsonResponse({ totals: { runs: 0, failed: 0, active: 0, queued: 0, succeeded: 0, cancelled: 0, unknown: 0 }, projects: [], duration_trend: [], timing: { recorded: 0, total: 0 } });
      return jsonResponse({});
    },
  });
  try {
    assert.equal(projEnv.doc.getElementById("source-reference-field").hidden, true);
    assert.equal(projEnv.doc.getElementById("source-revision-field").hidden, true);
  } finally {
    projEnv.cleanup();
  }

  // Repositories hides zero-count filter chips and clean KPI tiles.
  const repoEnv = await setup({
    initialSearch: "?page=source",
    extraAssets: ["source.js"],
    fetchHandler: async (target) => {
      if (target === "/api/session") return jsonResponse(baseSession);
      if (target === "/api/projects") return jsonResponse({ enrolled: [], available: [] });
      if (target === "/api/repositories") {
        return jsonResponse({
          repositories: [{ identity: "a".repeat(64), slug: "OnixResearch/kiln", label: "Kiln", status: "synced", remotes: [], local_copies: [], active_cairns: 0 }],
          summary: { total: 1, total_local_copies: 1, total_cairns: 0, synced: 1, diverged: 0, dirty: 0, unknown: 0, local_only: 0 },
        });
      }
      if (target === "/api/sources") return jsonResponse({ sources: [] });
      return jsonResponse({});
    },
  });
  try {
    const chips = repoEnv.doc.querySelectorAll(".repo-filter-chip");
    const visibleChips = chips.filter((c) => !c.hidden).map((c) => c.dataset.filterStatus);
    assert.deepEqual(visibleChips, ["all"]);
    assert.equal(repoEnv.doc.querySelector(".repo-kpis"), null);
  } finally {
    repoEnv.cleanup();
  }
});
