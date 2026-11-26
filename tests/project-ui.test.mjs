import assert from "node:assert/strict";
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
    this.scrollLeft = 0;
    this.scrollWidth = 100;
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
  get firstElementChild() {
    return this.children.find((c) => c.nodeType === 1) || null;
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
    this.attributes.set(key, String(value));
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
      const node = typeof item === "object" && item instanceof FakeNode ? item : new FakeNode(3, String(item));
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
      const node = typeof item === "object" && item instanceof FakeNode ? item : new FakeNode(3, String(item));
      node.parentNode = this;
      setConnected(node, this.isConnected);
      converted.push(node);
    }
    this.children = [...converted, ...this.children];
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
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  dispatchEvent(event) {
    for (const fn of this.listeners.get(event.type) || []) fn(event);
  }
  click() {
    this.dispatchEvent({
      type: "click",
      button: 0,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      altKey: false,
      preventDefault() {},
    });
  }
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
  closest(selector) {
    let cur = this;
    while (cur && cur.nodeType === 1) {
      if (matchesSimple(cur, selector)) return cur;
      cur = cur.parentNode;
    }
    return null;
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
  if (selector === "[data-project-focus]") return el.dataset.projectFocus !== undefined;
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

function setup() {
  const elementsById = new Map();
  const doc = {
    title: "",
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
  };
  for (const id of ["page-primary-action", "workspace-title", "app-breadcrumb"]) {
    const el = doc.createElement("div");
    el.id = id;
    el.isConnected = true;
    elementsById.set(id, el);
  }
  const container = doc.createElement("div");
  container.isConnected = true;

  const context = {
    window: {},
    document: doc,
    location: { search: "?page=project&project=alpha" },
    URLSearchParams,
    DOMException,
    Node: FakeNode,
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
    setTimeout,
    clearTimeout,
  };
  context.window = context;
  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("project.js"), context);
  return {
    projectUi: context.window.KilnProject,
    ui: context.window.KilnUi,
    doc,
    container,
    elementsById,
  };
}

const enrolledProject = {
  id: "alpha",
  label: "Alpha",
  repository: "/repos/alpha",
  reference: "refs/heads/main",
  scope: { root: "crates/alpha", graphs: ["cargo"] },
};

test("empty project id renders selection prompt and clears owned header action", async () => {
  const { projectUi, container, elementsById, doc } = setup();
  const staleAction = doc.createElement("button");
  staleAction.dataset.projectAction = "alpha";
  elementsById.get("page-primary-action").replaceChildren(staleAction);

  await projectUi.load(container, {
    project: "",
    api: async () => { throw new Error("should not call api"); },
    navigate() {},
  });

  const text = allText(container);
  assert.match(text, /No project selected/);
  assert.match(text, /Browse projects/);
  assert.equal(elementsById.get("page-primary-action").children.length, 0);
});

test("project detail calculates streak skipping active runs, draws timed duration bars, and excludes untimed runs", async () => {
  const { projectUi, container, elementsById, doc } = setup();
  const started = [];
  const navigated = [];
  const runs = [
    { id: "run-active", project_id: "alpha", ordinal: 5, outcome: "Pending", phase: "active", started_at_ms: 5000, duration_ms: null, source_revision: "rev555555555555" },
    { id: "run-4", project_id: "alpha", ordinal: 4, outcome: "Success", phase: "completed", started_at_ms: 4000, completed_at_ms: 4500, duration_ms: 500, source_revision: "rev444444444444" },
    { id: "run-3-untimed", project_id: "alpha", ordinal: 3, outcome: "Success", phase: "completed", duration_ms: null, source_revision: "rev333333333333" },
    { id: "run-2-zero", project_id: "alpha", ordinal: 2, outcome: "Success", phase: "completed", started_at_ms: 2000, completed_at_ms: 2000, duration_ms: 0, source_revision: "rev222222222222" },
    { id: "run-1-fail", project_id: "alpha", ordinal: 1, outcome: "DomainFailure", phase: "completed", started_at_ms: 1000, completed_at_ms: 2200, duration_ms: 1200, source_revision: "rev111111111111" },
    { id: "run-other-proj", project_id: "beta", ordinal: 99, outcome: "DomainFailure", duration_ms: 999 },
  ];

  await projectUi.load(container, {
    project: "alpha",
    canOperate: true,
    startRun: (id) => started.push(id),
    navigate: (patch) => navigated.push(patch),
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      if (path.startsWith("/api/runs?")) return { runs, total: 5, next_cursor: null };
      throw new Error(`unexpected path ${path}`);
    },
  });

  const health = descendants(container).find((n) => n.classList.contains("pp-health"));
  assert.ok(health);
  // Streak skips run-active (Pending) and counts run-4, run-3-untimed, run-2-zero before run-1-fail = 3 passing runs.
  assert.equal(health.dataset.health, "success");
  assert.match(allText(health), /Passing for 3 runs/);
  assert.match(allText(health), /1 in progress/);
  assert.match(allText(health), /crates\/alpha/);
  assert.match(allText(health), /Cargo graph/);

  // Trend card excludes run-3-untimed and never treats missing duration as 0, while keeping real 0ms run-2-zero.
  const trendNote = descendants(container).find((n) => n.classList.contains("pp-trend-note"));
  assert.equal(trendNote.dataset.excluded, "1");
  assert.match(trendNote.textContent, /3 timed runs plotted · 1 run without recorded timing excluded/);
  const bars = descendants(container).filter((n) => n.localName === "rect" && n.classList.contains("pp-bar"));
  assert.equal(bars.length, 3);

  // Header action button
  const primaryBtn = elementsById.get("page-primary-action").firstElementChild;
  assert.ok(primaryBtn);
  assert.equal(primaryBtn.textContent, "Run latest…");
  primaryBtn.click();
  assert.deepEqual(started, ["alpha"]);

  assert.equal(doc.title, "Kiln · Alpha");
  assert.equal(elementsById.get("workspace-title").textContent, "Alpha");

  // Clicking a run link calls ctx.navigate with exact run id
  const logsLink = descendants(container).find((n) => n.localName === "a" && n.textContent === "Logs");
  assert.ok(logsLink);
  logsLink.click();
  assert.equal(navigated.length, 1);
  assert.equal(navigated[0].page, "runs");
  assert.equal(navigated[0].view, "logs");
});

test("all-untimed finished runs omit duration SVG chart and state unknown timing is never zero", async () => {
  const { projectUi, container } = setup();
  const runs = [
    { id: "r1", project_id: "alpha", ordinal: 2, outcome: "DomainFailure", duration_ms: null },
    { id: "r2", project_id: "alpha", ordinal: 1, outcome: "DomainFailure", duration_ms: undefined },
  ];

  await projectUi.load(container, {
    project: "alpha",
    canOperate: false,
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      if (path.startsWith("/api/runs?")) return { runs, total: 2, next_cursor: null };
      throw new Error(`unexpected path ${path}`);
    },
  });

  const health = descendants(container).find((n) => n.classList.contains("pp-health"));
  assert.equal(health.dataset.health, "failure");
  assert.match(allText(health), /Failing for 2 runs/);

  const trend = descendants(container).find((n) => n.classList.contains("pp-trend"));
  assert.ok(trend);
  assert.equal(descendants(trend).filter((n) => n.localName === "svg").length, 0);
  assert.match(allText(trend), /Unknown timing is never treated as zero/);
  assert.match(allText(trend), /2 finished runs have no recorded start and finish/);
});

test("bounded pagination reads at most 2 pages and marks truncated streak with 'at least'", async () => {
  const { projectUi, container } = setup();
  let calls = 0;

  await projectUi.load(container, {
    project: "alpha",
    canOperate: false,
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      if (path.startsWith("/api/runs?")) {
        calls += 1;
        return {
          runs: [{ id: `run-p${calls}`, project_id: "alpha", ordinal: 10 - calls, outcome: "Cancelled", duration_ms: 100 }],
          total: 25,
          next_cursor: `cursor-${calls + 1}`,
        };
      }
      throw new Error(`unexpected ${path}`);
    },
  });

  assert.equal(calls, 2);
  const text = allText(container);
  assert.match(text, /Cancelled for at least 2 runs/);
  assert.match(text, /at most 2 pages of 50 are read/);
});

test("in-progress-only, unknown project, allowlisted-only, and partial/full error states render explicitly", async () => {
  const { projectUi, container, elementsById } = setup();

  // 1. All runs in progress
  await projectUi.load(container, {
    project: "alpha",
    canOperate: false,
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      return { runs: [{ id: "r-wait", project_id: "alpha", outcome: "Pending", phase: "queued" }], total: 1, next_cursor: null };
    },
  });
  assert.match(allText(container), /No finished runs yet/);
  assert.match(allText(container), /Every loaded run is still in progress/);

  // 2. Unknown project (not in inventory and 0 runs)
  await projectUi.load(container, {
    project: "missing-id",
    canOperate: true,
    startRun() {},
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      return { runs: [], total: 0, next_cursor: null };
    },
  });
  assert.match(allText(container), /Unknown project/);
  assert.equal(elementsById.get("page-primary-action").children.length, 0);

  // 3. Allowlisted but not enrolled -> warning + no Run latest button
  await projectUi.load(container, {
    project: "allowlisted",
    canOperate: true,
    startRun() {},
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [], available: [{ id: "allowlisted", label: "Allowlisted" }] };
      return { runs: [], total: 0, next_cursor: null };
    },
  });
  assert.match(allText(container), /allowlisted but not enrolled/);
  assert.match(allText(container), /No retained runs for this project yet/);
  assert.equal(elementsById.get("page-primary-action").children.length, 0);

  // 4. Both inventory and run history fail -> error state with data-state="unavailable"
  await projectUi.load(container, {
    project: "alpha",
    canOperate: true,
    navigate() {},
    api: async () => { throw new TypeError("network down"); },
  });
  const errNode = descendants(container).find((n) => n.classList.contains("error-state"));
  assert.ok(errNode);
  assert.equal(errNode.dataset.state, "unavailable");
  assert.match(errNode.textContent, /Project page unavailable/);
  assert.match(errNode.textContent, /Nothing is inferred/);
});

test("clear() cancels in-flight load without writing stale project data into container", async () => {
  const { projectUi, container } = setup();
  let resolveRuns;
  const pendingRuns = new Promise((r) => { resolveRuns = r; });

  const loadPromise = projectUi.load(container, {
    project: "alpha",
    navigate() {},
    api: async (path) => {
      if (path === "/api/projects") return { enrolled: [enrolledProject], available: [] };
      return pendingRuns;
    },
  });

  projectUi.clear();
  resolveRuns({ runs: [{ id: "late", project_id: "alpha", outcome: "Success", duration_ms: 10 }], total: 1, next_cursor: null });
  await loadPromise;
  assert.equal(container.children.length, 0);
});
