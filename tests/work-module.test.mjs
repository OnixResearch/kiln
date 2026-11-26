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
    this.multiple = false;
    this.scrollTop = 0;
    this.scrollLeft = 0;
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
  get firstElementChild() {
    return this.children.find((c) => c.nodeType === 1) || null;
  }
  get childElementCount() {
    return this.children.filter((c) => c.nodeType === 1).length;
  }
  get options() {
    return this.children.filter((c) => c.localName === "option");
  }
  get selectedOptions() {
    return this.options.filter((o) => o.selected);
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
    const idx = this.children.indexOf(refNode);
    newNode.parentNode = this;
    setConnected(newNode, this.isConnected);
    if (idx === -1) this.children.push(newNode);
    else this.children.splice(idx, 0, newNode);
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
  }
  close() {
    this.open = false;
    queueMicrotask(() => this.dispatchEvent({ type: "close" }));
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
      target: this,
      preventDefault() {},
    });
  }
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
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
      if (matchesSimple(cur, selector)) return cur;
      cur = cur.parentNode;
    }
    return null;
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

class FakeHTMLElement extends FakeElement {}

function setConnected(node, connected) {
  node.isConnected = connected;
  for (const child of node.children || []) setConnected(child, connected);
}

function matchesSimple(el, selector) {
  if (selector === "span[data-icon]") return el.localName === "span" && el.getAttribute("data-icon") !== null;
  if (selector === "[data-work-id]") return el.dataset.workId !== undefined;
  if (selector === "[hidden]") return el.hasAttribute("hidden");
  if (selector === "[inert]") return el.hasAttribute("inert");
  if (selector === ".work-board-column[data-drop]") return el.classList.contains("work-board-column") && el.dataset.drop !== undefined;
  if (selector === "a") return el.localName === "a";
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

class FakeCustomEvent {
  constructor(type, init = {}) {
    this.type = type;
    this.detail = init.detail;
  }
}

function setup({
  principal = "local:operator",
  canWrite = true,
  apiHandler,
  commandHandler,
  changeEvidenceHandler,
} = {}) {
  const elementsById = new Map();
  const windowListeners = new Map();
  const docListeners = new Map();
  const notices = [];
  const navigations = [];
  let currentRoute = { page: "work" };

  const body = new FakeElement("body", "http://www.w3.org/1999/xhtml", null);
  body.isConnected = true;

  const doc = {
    body,
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
    getElementById(id) {
      return elementsById.get(id) || null;
    },
    querySelectorAll(selector) {
      return body.querySelectorAll(selector);
    },
    addEventListener(type, fn) {
      if (!docListeners.has(type)) docListeners.set(type, []);
      docListeners.get(type).push(fn);
    },
  };
  body.ownerDocument = doc;

  const primaryAction = doc.createElement("div");
  primaryAction.id = "page-primary-action";
  body.append(primaryAction);
  elementsById.set("page-primary-action", primaryAction);

  const container = doc.createElement("div");
  container.id = "work-content";
  body.append(container);
  elementsById.set("work-content", container);

  const bridge = {
    session: () => (principal ? { principal, can_write: canWrite } : null),
    route: () => currentRoute,
    navigate: (patch) => {
      navigations.push(patch);
      currentRoute = { ...currentRoute, ...patch };
    },
    notice: (msg) => notices.push(msg),
    element: (tag, text, cls) => {
      const el = doc.createElement(tag);
      if (text !== undefined) el.textContent = String(text);
      if (cls) el.className = cls;
      return el;
    },
    runLink: (id, label) => doc.createElement("a", label),
    api: apiHandler || (async () => { throw new Error("unexpected api call"); }),
    command: commandHandler || (async () => { throw new Error("unexpected command call"); }),
    changeEvidence: changeEvidenceHandler || (async () => ({ source: [], reviews: [], runs: [], lifecycle: [], artifacts: [] })),
  };

  const context = {
    window: {},
    document: doc,
    Node: FakeNode,
    Element: FakeElement,
    HTMLElement: FakeHTMLElement,
    CustomEvent: FakeCustomEvent,
    URLSearchParams,
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
    setTimeout,
    clearTimeout,
    addEventListener(type, fn) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(fn);
    },
    dispatchEvent(event) {
      for (const fn of windowListeners.get(event.type) || []) fn(event);
    },
  };
  context.window = context;
  context.window.KilnWork = bridge;

  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("work-board.js"), context);
  vm.runInNewContext(asset("work-evidence.js"), context);
  vm.runInNewContext(asset("work.js"), context);

  return {
    work: context.window.KilnWorkUI,
    doc,
    body,
    container,
    primaryAction,
    notices,
    navigations,
    setRoute(route) { currentRoute = route; },
  };
}

const sampleCi = {
  schema: "kiln.work.ci.v1",
  projects: [{ id: "proj-a", label: "Alpha", repository: "/repo", repository_key: "a".repeat(64), scope_root: "crates/a" }],
  records: { "issue-1": ["proj-a"] },
};

const sampleObjects = [
  {
    id: "issue-1",
    kind: "issue",
    head: "head-1",
    created_by: "local:operator",
    created_at_ms: 1_700_000_000_000,
    updated_at_ms: 1_700_000_010_000,
    fields: {
      title: "Fix flaky parser",
      description: "Reproduce and fix edge case",
      status: "started",
      priority: 2,
      owner: "local:operator",
      assignees: ["local:operator"],
      labels: ["parser"],
      due_at_ms: 1_000,
      links: [],
    },
    comments: [],
  },
  {
    id: "req-1",
    kind: "request",
    head: "head-r1",
    created_by: "local:triage",
    created_at_ms: 1_700_000_005_000,
    updated_at_ms: 1_700_000_020_000,
    fields: {
      title: "Customer intake request",
      description: "Needs triage",
      status: "triage",
      priority: 1,
      owner: "",
      assignees: [],
      labels: ["intake"],
      customer: "Acme Corp",
      source_url: "https://example.test/tickets/1",
      links: [],
    },
    comments: [{ id: "c1", author: "local:triage", body: "Ping @operator", mentions: ["local:operator"], recorded_at_ms: 1_700_000_006_000, reactions: {} }],
  },
  {
    id: "view-1",
    kind: "view",
    head: "head-v1",
    created_by: "local:operator",
    created_at_ms: 1_700_000_001_000,
    updated_at_ms: 1_700_000_001_000,
    fields: {
      title: "Active issues",
      status: "started",
      owner: "local:operator",
      view: { filters: { status: "started" }, sort: "updated", group_by: "none", layout: "list", shared: false },
    },
  },
];

function snapshotPayload(overrides = {}) {
  return {
    schema: "kiln.work.v1",
    bounded: true,
    configured: true,
    can_write: true,
    principal: "local:operator",
    objects: sampleObjects,
    ci: sampleCi,
    ...overrides,
  };
}

test("issue list renders filtered items, overdue state, CI chips, and read-only banner when not writable", async () => {
  const { work, container, primaryAction } = setup({
    canWrite: false,
    apiHandler: async (path) => {
      if (path === "/api/work") return snapshotPayload({ can_write: false });
      throw new Error(`unexpected ${path}`);
    },
  });

  await work.render(container, { page: "work" });

  const text = allText(container);
  const listScroll = descendants(container).find((n) => n.classList.contains("work-list-scroll"));
  assert.match(text, /Read-only work workspace/);
  assert.match(allText(listScroll), /Fix flaky parser/);
  assert.match(allText(listScroll), /Customer intake request/);
  assert.doesNotMatch(allText(listScroll), /Active issues/); // saved view excluded from default issue/request list
  assert.match(text, /Alpha/); // CI project chip
  assert.match(text, /Overdue/);

  const newBtn = descendants(primaryAction).find((n) => n.localName === "button" && n.textContent === "New issue…");
  assert.ok(newBtn);
  assert.equal(newBtn.disabled, true);
  assert.equal(work.objects().length, 3);
  work.clear();
  assert.equal(work.objects().length, 0);
});

test("board view renders server Kanban projection and surfaces alert on invalid/failed board API", async () => {
  const projection = {
    schema: "kiln.work.board.v1",
    principal: "local:operator",
    bounded: true,
    lane_by: "status",
    matched_issues: 1,
    shown_cards: 1,
    list_only: 0,
    snapshot_objects: 3,
    object_limit: 1024,
    card_limit: 100,
    columns_truncated: 0,
    complete: true,
    columns: [
      { status: "started", builtin: true, retired: false, teams: [], total: 1, cards: [{ id: "issue-1", head: "head-1", status: "started", title: "Fix flaky parser", priority: 2, assignees: [], labels: [] }] },
      { status: "completed", builtin: true, retired: false, teams: [], total: 0, cards: [] },
    ],
    other: { statuses: [], total: 0, cards: [] },
  };

  let failBoard = false;
  const { work, container } = setup({
    apiHandler: async (path) => {
      if (path === "/api/work") return snapshotPayload();
      if (path.startsWith("/api/work/board")) {
        if (failBoard) throw new Error("board_service_down");
        return projection;
      }
      throw new Error(`unexpected ${path}`);
    },
  });

  await work.render(container, { page: "work", work_layout: "board" });
  // Wait a microtask for async loadBoard to settle and re-draw.
  await new Promise((r) => setTimeout(r, 10));

  const text = allText(container);
  assert.match(text, /1 matching issue, all shown/);
  assert.match(text, /Lanes and columns are planning facts/);
  assert.match(text, /Move…/);

  // Negative: failed board read shows explicit alert + Retry, no substituted columns.
  failBoard = true;
  await work.refresh();
  await new Promise((r) => setTimeout(r, 10));
  const alertNode = descendants(container).find((n) => n.getAttribute("role") === "alert");
  assert.ok(alertNode);
  assert.match(allText(alertNode), /Board unavailable: board_service_down/);
  assert.match(allText(alertNode), /Retry/);
});

test("inbox filters attention items excluding read head, and triage renders request triage controls", async () => {
  const objectsWithPersonal = [
    ...sampleObjects,
    {
      id: "personal-1",
      kind: "personal",
      head: "head-p1",
      created_by: "local:operator",
      created_at_ms: 1,
      updated_at_ms: 1,
      fields: {
        title: "My attention",
        owner: "local:operator",
        status: "backlog",
        personal: { watched: [], read: { "issue-1": "head-1" }, favorites: [], reminders: {} },
      },
    },
  ];
  const { work, container } = setup({
    apiHandler: async (path) => {
      if (path === "/api/work") return snapshotPayload({ objects: objectsWithPersonal });
      if (path === "/api/work/objects/req-1") return sampleObjects[1];
      throw new Error(`unexpected ${path}`);
    },
  });

  // Inbox should hide issue-1 (read at head-1) and show req-1 (Mentioned)
  await work.render(container, { page: "inbox" });
  let text = allText(container);
  assert.match(text, /Customer intake request/);
  assert.match(text, /Mentioned/);
  assert.doesNotMatch(text, /Fix flaky parser/);

  // Mark req-1 and view-1 read too -> empty inbox state
  objectsWithPersonal[3].fields.personal.read["req-1"] = "head-r1";
  objectsWithPersonal[3].fields.personal.read["view-1"] = "head-v1";
  await work.refresh();
  text = allText(container);
  assert.match(text, /You're all caught up/);

  // Triage page with selected request detail
  await work.render(container, { page: "triage", work_id: "req-1" });
  await new Promise((r) => setTimeout(r, 10));
  text = allText(container);
  assert.match(text, /Request triage/);
  assert.match(text, /Accept to backlog/);
  assert.match(text, /Acme Corp/);
  assert.match(text, /https:\/\/example\.test\/tickets\/1/);
});

test("unconfigured, storage unavailable, invalid snapshot, and missing saved view states render explicitly", async () => {
  // 1. Unconfigured
  let mode = "unconfigured";
  const { work, container, primaryAction } = setup({
    apiHandler: async () => {
      if (mode === "unconfigured") return snapshotPayload({ configured: false, objects: [] });
      if (mode === "bad-principal") return snapshotPayload({ principal: "other:user" });
      if (mode === "error") throw new Error("work_store_offline");
      return snapshotPayload();
    },
  });

  await work.render(container, { page: "work" });
  assert.match(allText(container), /Work storage is not configured/);
  assert.equal(primaryAction.children.length, 0);

  // 2. Principal mismatch -> error state
  mode = "bad-principal";
  await work.refresh();
  assert.match(allText(container), /Work storage unavailable/);
  assert.match(allText(container), /invalid or belongs to a different principal/);

  // 3. Storage error -> Reconnect prompt
  mode = "error";
  await work.refresh();
  assert.match(allText(container), /work_store_offline/);
  assert.match(allText(container), /No empty or demo data has been substituted/);
  assert.ok(descendants(container).find((n) => n.localName === "button" && n.textContent === "Reconnect"));

  // 4. Missing saved view
  mode = "ok";
  await work.render(container, { page: "work", work_view: "nonexistent-view" });
  assert.match(allText(container), /Selected saved view is unavailable/);
});

test("work action validation errors, confirmation cancellation, and command failure reporting", async () => {
  const sent = [];
  let commandShouldFail = false;
  const { work, body, container, notices } = setup({
    apiHandler: async (path) => {
      if (path === "/api/work") return snapshotPayload();
      if (path === "/api/work/objects/issue-1") return sampleObjects[0];
      if (path === "/api/work/objects/view-1") return sampleObjects[2];
      throw new Error(`unexpected ${path}`);
    },
    commandHandler: async (cmd) => {
      sent.push(cmd);
      if (commandShouldFail) {
        const err = new Error("work_head_conflict: stale head");
        err.status = 409;
        throw err;
      }
      return { object: { ...sampleObjects[0], head: "head-2", fields: { ...sampleObjects[0].fields, status: "completed" } } };
    },
  });

  await work.render(container, { page: "work" });

  // Negative validation errors
  assert.throws(() => work.create("not-a-kind"), /Unsupported work kind/);
  await assert.rejects(() => work.bulk("completed", []), /Select at least one work record/);
  await assert.rejects(() => work.bulk({ type: "bogus" }, ["issue-1"]), /Unsupported bulk work action/);
  await assert.rejects(() => work.bulk({ type: "edit", fields: { customer: "x" } }, ["issue-1"]), /Bulk editing supports typed metadata only/);
  await assert.rejects(() => work.bulk("completed", ["view-1"]), /Edit private records and saved views individually/);

  // Cancel confirmation -> sends nothing
  const cancelPromise = work.bulk("completed", ["issue-1"]);
  await new Promise((r) => setTimeout(r, 10));
  const cancelDialog = descendants(body).find((n) => n.localName === "dialog" && n.open);
  assert.ok(cancelDialog);
  assert.equal(cancelDialog.getAttribute("role"), "alertdialog");
  const cancelBtn = descendants(cancelDialog).find((n) => n.localName === "button" && n.textContent === "Cancel");
  cancelBtn.click();
  assert.equal(await cancelPromise, null);
  assert.equal(sent.length, 0);

  // Confirm -> sends command and persists
  const confirmPromise = work.bulk("completed", ["issue-1"]);
  await new Promise((r) => setTimeout(r, 10));
  const confirmDialog = descendants(body).find((n) => n.localName === "dialog" && n.open);
  const confirmForm = descendants(confirmDialog).find((n) => n.localName === "form");
  confirmForm.dispatchEvent({ type: "submit", preventDefault() {} });
  const results = await confirmPromise;
  assert.equal(results.length, 1);
  assert.equal(sent.length, 1);
  assert.equal(sent[0].expected_head, "head-1");
  assert.match(notices.at(-1), /1 signed work command\(s\) persisted/);

  // Command failure -> reports observed completed count and does not retry
  commandShouldFail = true;
  const failPromise = work.bulk("completed", ["issue-1"]);
  await new Promise((r) => setTimeout(r, 10));
  const failDialog = descendants(body).find((n) => n.localName === "dialog" && n.open);
  const failForm = descendants(failDialog).find((n) => n.localName === "form");
  failForm.dispatchEvent({ type: "submit", preventDefault() {} });
  await assert.rejects(() => failPromise, /work_head_conflict.*0 command\(s\) were observed complete/);
  assert.equal(sent.length, 2);
});
