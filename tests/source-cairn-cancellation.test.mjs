import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) =>
  readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class DomNode {
  constructor(tag) {
    this.tagName = String(tag).toUpperCase();
    this.children = [];
    this.parent = null;
    this.attributes = new Map();
    this.dataset = {};
    this.listeners = new Map();
    this.text = "";
    this.value = "";
    this.hidden = false;
    this.disabled = false;
    this.open = false;
    this.ownerDocument = null;
    this.scrollTop = 0;
    this.style = {};
    const node = this;
    this.classList = {
      add(...names) {
        const set = new Set([...node.classList, ...names.filter(Boolean)]);
        node.setAttribute("class", [...set].join(" "));
      },
      toggle(name, force) {
        const has = [...node.classList].includes(name);
        const want = force === undefined ? !has : Boolean(force);
        const next = new Set([...node.classList]);
        if (want) next.add(name);
        else next.delete(name);
        node.setAttribute("class", [...next].join(" "));
      },
      contains(name) {
        return [...node.classList].includes(name);
      },
      *[Symbol.iterator]() {
        yield* (node.getAttribute("class") || "").split(" ").filter(Boolean);
      },
    };
  }
  get className() {
    return this.getAttribute("class") || "";
  }
  set className(value) {
    this.setAttribute("class", value);
  }
  get isConnected() {
    let cur = this;
    while (cur.parent) cur = cur.parent;
    return cur === this.ownerDocument?.root || cur.tagName === "DIV";
  }
  get firstChild() {
    return this.children[0] || null;
  }
  get lastChild() {
    return this.children[this.children.length - 1] || null;
  }
  get textContent() {
    if (this.tagName === "#TEXT") return this.text;
    if (!this.children.length && this.text) return this.text;
    return this.children.map((c) => c.textContent).join(" ");
  }
  set textContent(value) {
    this.children = [];
    this.text = String(value ?? "");
  }
  setAttribute(key, value) {
    this.attributes.set(key, String(value));
  }
  getAttribute(key) {
    return this.attributes.has(key) ? this.attributes.get(key) : null;
  }
  removeAttribute(key) {
    this.attributes.delete(key);
  }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  append(...nodes) {
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const child =
        typeof item === "string"
          ? Object.assign(new DomNode("#text"), { text: item, ownerDocument: this.ownerDocument })
          : item;
      if (child.parent) child.remove();
      child.parent = this;
      this.children.push(child);
    }
  }
  prepend(...nodes) {
    const built = [];
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const child =
        typeof item === "string"
          ? Object.assign(new DomNode("#text"), { text: item, ownerDocument: this.ownerDocument })
          : item;
      if (child.parent) child.remove();
      child.parent = this;
      built.push(child);
    }
    this.children.unshift(...built);
  }
  replaceChildren(...nodes) {
    for (const child of this.children) child.parent = null;
    this.children = [];
    this.text = "";
    this.append(...nodes);
  }
  replaceWith(node) {
    if (!this.parent) return;
    const idx = this.parent.children.indexOf(this);
    if (idx < 0) return;
    node.parent = this.parent;
    this.parent.children.splice(idx, 1, node);
    this.parent = null;
  }
  remove() {
    if (!this.parent) return;
    const idx = this.parent.children.indexOf(this);
    if (idx >= 0) this.parent.children.splice(idx, 1);
    this.parent = null;
  }
  contains(node) {
    let cur = node;
    while (cur) {
      if (cur === this) return true;
      cur = cur.parent;
    }
    return false;
  }
  querySelectorAll(selector) {
    const out = [];
    const match = (node) => {
      if (selector.startsWith(".")) return node.classList.contains(selector.slice(1));
      if (selector.startsWith("[") && selector.endsWith("]")) {
        const inner = selector.slice(1, -1);
        const [k, v] = inner.split("=");
        if (v !== undefined) return node.getAttribute(k) === v.replace(/^"|"$/g, "");
        return node.getAttribute(k) !== null;
      }
      return node.tagName.toLowerCase() === selector.toLowerCase();
    };
    const walk = (node) => {
      for (const child of node.children) {
        if (match(child)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }
  closest() {
    return null;
  }
  getBoundingClientRect() {
    return { top: 0 };
  }
  scrollIntoView() {}
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
}

function makeEnv() {
  const document = {
    activeElement: null,
    title: "",
    root: null,
    createElement(tag) {
      return Object.assign(new DomNode(tag), { ownerDocument: document });
    },
    createElementNS(_ns, tag) {
      return Object.assign(new DomNode(tag), { ownerDocument: document });
    },
    createTextNode(text) {
      return Object.assign(new DomNode("#text"), { text, ownerDocument: document });
    },
    querySelectorAll() {
      return [];
    },
    getElementById() {
      return null;
    },
  };
  const page = {
    window: { addEventListener() {}, removeEventListener() {}, matchMedia: () => ({ matches: false }) },
    document,
    location: { search: "" },
    URLSearchParams,
    AbortController,
    DOMException,
    TextEncoder,
    setTimeout,
    clearTimeout,
    crypto: webcrypto,
  };
  vm.runInNewContext(asset("icons.js"), page);
  vm.runInNewContext(asset("ui.js"), page);
  vm.runInNewContext(asset("inventory.js"), page);
  vm.runInNewContext(asset("source.js"), page);
  vm.runInNewContext(asset("cairn.js"), page);
  return { page, document };
}

function deferredApi() {
  const calls = [];
  const api = (target, opts = {}) =>
    new Promise((resolve, reject) => {
      calls.push({ target, controller: opts?.controller || null, resolve, reject });
    });
  const resolveNext = (predicate, value) => {
    const idx = calls.findIndex(predicate);
    assert.ok(idx >= 0, `expected pending call; have: ${JSON.stringify(calls.map((c) => c.target))}`);
    const [call] = calls.splice(idx, 1);
    call.resolve(value);
    return call;
  };
  return { api, calls, resolveNext };
}

test("KilnSource.load aborts prior in-flight request, ignores delayed response, and scopes cache by restricted view handle", async () => {
  const { page, document } = makeEnv();
  const container = document.createElement("div");
  document.root = container;
  const { api, calls, resolveNext } = deferredApi();

  // Start loading source "alpha", then immediately switch to "beta".
  const loadA = page.window.KilnSource.load(container, {
    api,
    navigate() {},
    route: { src: "alpha", sview: "files" },
  });
  assert.equal(calls.length, 1);
  const firstInventoryCall = calls[0];
  assert.ok(firstInventoryCall.controller, "expected AbortController passed to ctx.api");
  assert.equal(firstInventoryCall.controller.signal.aborted, false);

  const loadB = page.window.KilnSource.load(container, {
    api,
    navigate() {},
    route: { src: "beta", sview: "files" },
  });
  assert.equal(
    firstInventoryCall.controller.signal.aborted,
    true,
    "newer selection must abort previous AbortController",
  );

  const revA = "a".repeat(40);
  const revB = "b".repeat(40);
  const viewBroad = "11".repeat(32);
  const viewNarrow = "22".repeat(32);

  // Settle beta first.
  resolveNext((c) => c.target === "/api/sources" && c.controller !== firstInventoryCall.controller, {
    configured: true,
    sources: [
      { id: "beta", label: "Beta", kind: "git", status: "available", head: revB, revision_scope: "revision", view: { view: viewBroad } },
    ],
  });
  await new Promise((r) => setTimeout(r, 0));
  resolveNext((c) => c.target.startsWith("/api/sources/beta/log"), { entries: [{ id: revB, summary: "beta commit" }], next_cursor: null });
  resolveNext((c) => c.target.startsWith("/api/sources/beta/tree"), {
    revision: revB,
    entries: [{ name: "beta.rs", path: "beta.rs", kind: "file", bytes: 10 }],
  });
  await loadB;

  assert.match(container.textContent, /beta\.rs/);
  assert.match(document.title, /Beta/);

  // Delayed alpha response settles AFTER beta: must NOT overwrite beta!
  const alphaIdx = calls.indexOf(firstInventoryCall);
  if (alphaIdx >= 0) calls.splice(alphaIdx, 1);
  firstInventoryCall.resolve({
    configured: true,
    sources: [
      { id: "alpha", label: "Alpha", kind: "git", status: "available", head: revA, revision_scope: "revision" },
    ],
  });
  await loadA;
  assert.match(container.textContent, /beta\.rs/);
  assert.doesNotMatch(container.textContent, /alpha\.rs/);
  assert.match(document.title, /Beta/);

  // Narrowing restricted view opaque handle for same repo & revision MUST NOT reuse broader tree cache.
  const loadNarrow = page.window.KilnSource.load(container, {
    api,
    navigate() {},
    route: { src: "beta", sview: "files" },
  });
  resolveNext((c) => c.target === "/api/sources", {
    configured: true,
    sources: [
      { id: "beta", label: "Beta", kind: "git", status: "available", head: revB, revision_scope: "revision", view: { view: viewNarrow } },
    ],
  });
  await new Promise((r) => setTimeout(r, 0));
  resolveNext((c) => c.target.startsWith("/api/sources/beta/log"), { entries: [{ id: revB, summary: "beta commit" }], next_cursor: null });
  resolveNext((c) => c.target.startsWith("/api/sources/beta/tree"), {
    revision: revB,
    entries: [{ name: "beta_narrow.rs", path: "beta_narrow.rs", kind: "file", bytes: 4 }],
  });
  await loadNarrow;
  assert.match(container.textContent, /beta_narrow\.rs/);

  // clear() must abort any in-flight request.
  const pendingClear = page.window.KilnSource.load(container, {
    api,
    navigate() {},
    route: { src: "alpha", sview: "files" },
  });
  const pendingCall = calls.at(-1);
  assert.equal(pendingCall.controller.signal.aborted, false);
  page.window.KilnSource.clear();
  assert.equal(pendingCall.controller.signal.aborted, true);
  pendingCall.reject(new DOMException("Aborted", "AbortError"));
  await pendingClear;
  assert.equal(container.children.length, 0);
});

test("KilnCairn.load aborts prior in-flight index/detail/fleet requests on selection change and clear", async () => {
  const { page, document } = makeEnv();
  const container = document.createElement("div");
  document.root = container;
  const { api, calls, resolveNext } = deferredApi();
  const REPO = "ab".repeat(32);

  const indexFor = (...changes) => ({
    schema: "kiln.cairn-workspace-index.v2",
    truncated: false,
    snapshot: {},
    non_claims: [],
    changes: changes.map((change) => ({
      repository_identity: REPO,
      change,
      lane: "awaiting",
      lanes: { attention: 0, awaiting: 1, ready: 0, in_progress: 0, verified: 0, integrated: 0, recorded: 0 },
      open_decisions: 0,
      first_blocker: null,
      projects: ["kiln"],
      units: 1,
      current_work: 0,
      attempts: 0,
      active_attempts: 0,
      campaigns: [],
      blockers: 0,
    })),
  });
  const detailFor = (change) => ({
    schema: "kiln.cairn-workspace-detail.v4",
    identity: "11".repeat(32),
    repository_identity: REPO,
    change,
    projects: ["kiln"],
    snapshot: { work_recorded: true, attempts_recorded: true, campaigns_read: 0, campaigns_truncated: false, lifecycle_truncated: false },
    overview: {
      units: 1,
      current_work: 0,
      attempts: 0,
      active_attempts: 0,
      campaigns: 0,
      blockers: 0,
      first_blocker: null,
      lane: "awaiting",
      lanes: { attention: 0, awaiting: 1, ready: 0, in_progress: 0, verified: 0, integrated: 0, recorded: 0 },
      open_decisions: 0,
    },
    units: [
      {
        native_id: `${change}/unit-of-${change}`,
        unit: `unit-of-${change}`,
        sources: ["canonical_work"],
        current_work: [],
        superseded_work: [],
        facets: { readiness: "not_admitted", execution: "not_started", evidence: "none", integration: "not_published", lifecycle: "not_observed" },
        lane: "awaiting",
        attempts: [],
      },
    ],
    memberships: [],
    decisions: [],
    blockers: [],
    agents: {
      source: "not_configured",
      definitions: [],
      workers: [],
      workflow_revisions: { source: "not_configured", definitions: [] },
    },
    non_claims: [],
  });

  const ctxA = { api, navigate() {}, route: { cairn_repository: REPO, cairn_change: "alpha" } };
  const ctxB = { api, navigate() {}, route: { cairn_repository: REPO, cairn_change: "beta" } };

  const loadA = page.window.KilnCairn.load(container, ctxA);
  const firstBatch = calls.slice();
  assert.equal(firstBatch.length, 2);
  assert.ok(firstBatch.every((c) => c.controller && !c.controller.signal.aborted));

  const loadB = page.window.KilnCairn.load(container, ctxB);
  assert.ok(
    firstBatch.every((c) => c.controller.signal.aborted),
    "switching selection must abort previous index & detail AbortControllers",
  );

  // Resolve beta index + detail
  resolveNext((c) => c.target === "/api/implementation/cairn" && !c.controller.signal.aborted, indexFor("alpha", "beta"));
  resolveNext((c) => c.target.includes("change=beta") && !c.controller.signal.aborted, detailFor("beta"));
  await loadB;
  assert.match(container.textContent, /unit-of-beta/);

  // Delayed alpha responses settle AFTER beta -> ignored!
  firstBatch[0].resolve(indexFor("alpha"));
  firstBatch[1].resolve(detailFor("alpha"));
  await loadA;
  assert.match(container.textContent, /unit-of-beta/);
  assert.doesNotMatch(container.textContent, /unit-of-alpha/);

  // Pending fleet requests from beta must also be aborted by clear().
  const fleetCalls = calls.filter((c) => c.target.startsWith("/api/fleet/"));
  assert.equal(fleetCalls.length, 2);
  assert.ok(fleetCalls.every((c) => c.controller && !c.controller.signal.aborted));
  page.window.KilnCairn.clear();
  assert.ok(fleetCalls.every((c) => c.controller.signal.aborted));
  assert.equal(container.children.length, 0);
});
