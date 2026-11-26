import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class TestNode {
  constructor(tag, id = "") {
    this.tagName = String(tag).toUpperCase();
    this.localName = String(tag).toLowerCase();
    this.id = id;
    this.children = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.dataset = {};
    this.style = {};
    this.listeners = new Map();
    this.hidden = false;
    this.disabled = false;
    this.open = false;
    this.value = "";
    this.checked = false;
    this.required = false;
    this.tabIndex = 0;
    this._text = "";
    this.isConnected = true;
    this.classList = {
      _set: new Set(),
      add: (...xs) => xs.forEach((x) => x && this.classList._set.add(x)),
      remove: (...xs) => xs.forEach((x) => this.classList._set.delete(x)),
      toggle: (x, f) => {
        const next = f === undefined ? !this.classList._set.has(x) : Boolean(f);
        if (next) this.classList._set.add(x);
        else this.classList._set.delete(x);
        return next;
      },
      contains: (x) => this.classList._set.has(x),
    };
  }
  get className() {
    return [...this.classList._set].join(" ");
  }
  set className(v) {
    this.classList._set = new Set(String(v || "").split(/\s+/).filter(Boolean));
  }
  get textContent() {
    if (this._text && !this.children.length) return this._text;
    return [this._text, ...this.children.map((c) => c.textContent ?? c.text ?? "")].join("");
  }
  set textContent(v) {
    this.children = [];
    this._text = v == null ? "" : String(v);
  }
  get childElementCount() {
    return this.children.filter((c) => c.tagName && c.tagName !== "#TEXT").length;
  }
  get firstChild() {
    return this.children[0] || null;
  }
  get lastChild() {
    return this.children.at(-1) || null;
  }
  get options() {
    return this.children.filter((c) => c.localName === "option");
  }
  setAttribute(k, v) {
    this.attributes.set(String(k), String(v));
    if (String(k).startsWith("data-")) {
      const camel = String(k).slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
      this.dataset[camel] = String(v);
    }
    if (k === "hidden") this.hidden = true;
  }
  getAttribute(k) {
    return this.attributes.get(String(k)) ?? null;
  }
  hasAttribute(k) {
    return this.attributes.has(String(k));
  }
  removeAttribute(k) {
    this.attributes.delete(String(k));
    if (k === "hidden") this.hidden = false;
  }
  toggleAttribute(k, f) {
    const next = f === undefined ? !this.hasAttribute(k) : Boolean(f);
    if (next) this.setAttribute(k, "");
    else this.removeAttribute(k);
    return next;
  }
  append(...xs) {
    for (const x of xs) {
      if (x == null) continue;
      const n = typeof x === "object" ? x : Object.assign(new TestNode("#text"), { text: String(x), _text: String(x) });
      n.parentNode = this;
      this.children.push(n);
    }
  }
  prepend(...xs) {
    for (const x of xs.reverse()) {
      if (x == null) continue;
      const n = typeof x === "object" ? x : Object.assign(new TestNode("#text"), { text: String(x), _text: String(x) });
      n.parentNode = this;
      this.children.unshift(n);
    }
  }
  before() {}
  after(...xs) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const nodesToAdd = xs.map((x) => (typeof x === "object" ? x : Object.assign(new TestNode("#text"), { text: String(x), _text: String(x) })));
    for (const n of nodesToAdd) n.parentNode = this.parentNode;
    this.parentNode.children.splice(idx + 1, 0, ...nodesToAdd);
  }
  get nextElementSibling() {
    if (!this.parentNode) return null;
    const elems = this.parentNode.children.filter((c) => c.tagName && c.tagName !== "#TEXT");
    const idx = elems.indexOf(this);
    return idx >= 0 && idx + 1 < elems.length ? elems[idx + 1] : null;
  }
  insertBefore(newNode, refNode) {
    if (!newNode) return newNode;
    const idx = refNode ? this.children.indexOf(refNode) : -1;
    newNode.parentNode = this;
    if (idx === -1) this.children.push(newNode);
    else this.children.splice(idx, 0, newNode);
    return newNode;
  }
  replaceChildren(...xs) {
    this.children = [];
    this._text = "";
    this.append(...xs);
  }
  replaceWith(n) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx >= 0) this.parentNode.children.splice(idx, 1, n);
  }
  remove() {}
  addEventListener(type, fn) {
    this.listeners.set(type, [...(this.listeners.get(type) || []), fn]);
  }
  removeEventListener(type, fn) {
    this.listeners.set(type, (this.listeners.get(type) || []).filter((x) => x !== fn));
  }
  dispatchEvent(ev) {
    for (const fn of this.listeners.get(ev.type) || []) fn(ev);
  }
  click() {
    this.dispatchEvent({ type: "click", button: 0, preventDefault() {}, stopPropagation() {} });
  }
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
  showModal() {
    this.open = true;
  }
  close() {
    this.open = false;
    this.dispatchEvent({ type: "close" });
  }
  closest(sel) {
    const parts = String(sel || "").split(",").map((s) => s.trim()).filter(Boolean);
    let cur = this;
    while (cur) {
      for (const part of parts) {
        if (part === "[hidden]" && cur.hidden) return cur;
        if (part === "[inert]" && cur.inert) return cur;
        if (part.startsWith(".") && cur.classList?.contains(part.slice(1))) return cur;
        if (part.startsWith("#") && cur.id === part.slice(1)) return cur;
        if (/^[a-z]+$/i.test(part) && cur.localName === part.toLowerCase()) return cur;
      }
      cur = cur.parentNode;
    }
    return null;
  }
  contains(other) {
    if (other === this) return true;
    return this.children.some((c) => c.contains?.(other));
  }
  getBoundingClientRect() {
    return { top: 0, bottom: 100, left: 0, right: 100 };
  }
  getClientRects() {
    return this.hidden ? [] : [{ width: 10, height: 10 }];
  }
  scrollIntoView() {}
  querySelectorAll(sel) {
    const out = [];
    const match = (n) => {
      if (!n || !n.tagName) return false;
      if (sel === "a[data-page]") return n.localName === "a" && n.dataset?.page != null;
      if (sel === "a[data-view]") return n.localName === "a" && n.dataset?.view != null;
      if (sel === "[data-panel]") return n.dataset?.panel != null;
      if (sel === "[data-page]") return n.dataset?.page != null;
      if (sel === "[data-view]") return n.dataset?.view != null;
      if (sel === "[data-run-panel]") return n.dataset?.runPanel != null;
      if (sel === "[role='option']") return n.getAttribute?.("role") === "option";
      if (sel.startsWith(".")) return n.classList?.contains(sel.slice(1));
      if (sel.startsWith("#")) return n.id === sel.slice(1);
      return n.localName === sel.toLowerCase();
    };
    const walk = (n) => {
      for (const c of n.children || []) {
        if (match(c)) out.push(c);
        walk(c);
      }
    };
    walk(this);
    return out;
  }
  querySelector(sel) {
    return this.querySelectorAll(sel)[0] || null;
  }
}

function createHarness({ locationSearch = "?page=source&tab=repositories", fetchImpl, loadWorkInteraction = false } = {}) {
  const nodes = new Map();
  const bodyEl = new TestNode("body");
  const runPrimary = new TestNode("div");
  runPrimary.className = "run-primary-panels";
  bodyEl.append(runPrimary);
  for (const id of [
    "workspace", "login-panel", "notice", "brand-home", "app-sidebar", "app-menu-toggle", "app-menu-backdrop",
    "app-breadcrumb", "workspace-title", "workspace-eyebrow", "observation-note", "page-primary-action",
    "destination-tabs", "refresh-all", "account-button", "account-menu", "account-avatar", "account-name",
    "account-role", "account-settings", "auth-status", "connection-status", "logout", "login-form",
    "login-button", "operator-token", "theme-switch", "app-theme", "app-density", "principal-facts",
    "live-activity", "live-activity-count", "live-activity-unit", "global-live-strip", "sse-health-badge",
    "global-firing-pill", "global-firing-count", "global-escalations-pill", "global-escalations-count",
    "global-expiring-claims-pill", "global-expiring-claims-count", "global-command-trigger", "work-command-open",
    "rail-pin", "main", "overview-content", "source-content", "repositories-content", "run-split",
    "run-list-toggle", "run-list-toggle-label", "run-detail", "empty-run", "empty-run-title", "empty-run-message",
    "run-search", "run-filter", "run-outcome", "run-source", "run-order", "run-more-filters", "runs-first",
    "runs-next", "compare-other", "compare-form", "evidence-run", "evidence-form", "cob-search-form",
    "cob-repository", "cob-type", "cob-search", "cob-reset", "cob-first", "cob-next", "source-mode",
    "source-revision", "source-reference", "start-button", "available-project", "enrolled-project",
    "run-search-form", "enroll-form", "start-form", "download-log", "log-jump-error", "follow-logs",
    "clear-log", "log-form", "restart-log", "log-node", "log-stream", "log-search", "log-cursor",
    "log-position", "log-status", "log-output", "log-gap", "log-error-bar", "log-error-count",
    "rerun-exact", "rerun-latest", "rerun-failed", "control-cancel", "control-pause", "control-resume",
    "header-cancel", "header-rerun", "header-approve", "control-dialog", "control-form", "confirm-title",
    "confirm-warning", "confirm-facts", "confirm-reason-field", "confirm-reason-label", "confirm-reason",
    "confirm-reason-error", "confirm-cli-parity", "confirm-cli-cmd", "confirm-cli-copy", "confirm-cli-status",
    "confirm-control", "dismiss-control", "slide-over-inspector", "slide-over-backdrop", "slide-over-eyebrow",
    "slide-over-title", "slide-over-body", "slide-over-close", "slide-over-prev", "slide-over-next",
    "slide-over-open-full",
  ]) {
    const tag = id === "control-dialog" ? "dialog" : id.includes("form") ? "form" : id.includes("select") || ["app-theme", "app-density", "run-filter", "run-outcome", "run-order", "source-mode", "source-reference", "available-project", "enrolled-project", "log-node", "log-stream", "cob-repository", "cob-type"].includes(id) ? "select" : id.includes("input") || ["operator-token", "confirm-reason", "run-search", "run-source", "compare-other", "evidence-run", "cob-search", "source-revision", "log-search", "log-cursor"].includes(id) ? "input" : "div";
    const n = new TestNode(tag, id);
    if (["source-content", "repositories-content", "overview-content"].includes(id)) {
      const panel = new TestNode("section");
      panel.dataset.panel = id.replace("-content", "");
      panel.append(n);
      bodyEl.append(panel);
    } else {
      bodyEl.append(n);
    }
    nodes.set(id, n);
  }

  const findById = (id) => {
    if (nodes.has(id)) return nodes.get(id);
    const stack = [bodyEl, ...nodes.values()];
    const seen = new Set();
    while (stack.length) {
      const cur = stack.pop();
      if (!cur || seen.has(cur)) continue;
      seen.add(cur);
      if (cur.id === id) return cur;
      if (Array.isArray(cur.children)) stack.push(...cur.children);
    }
    const created = new TestNode("div", id);
    nodes.set(id, created);
    return created;
  };

  const docEl = new TestNode("html");
  const docListeners = new Map();
  const webDoc = {
    documentElement: docEl,
    body: bodyEl,
    activeElement: null,
    visibilityState: "visible",
    title: "",
    createElement: (tag) => Object.assign(new TestNode(tag), { ownerDocument: webDoc }),
    createElementNS: (_ns, tag) => Object.assign(new TestNode(tag), { ownerDocument: webDoc }),
    createTextNode: (t) => Object.assign(new TestNode("#text"), { text: t, _text: t, ownerDocument: webDoc }),
    getElementById: (id) => findById(id),
    querySelectorAll: (sel) => bodyEl.querySelectorAll(sel),
    querySelector: (sel) => {
      if (sel === "dialog[open]") return [...nodes.values(), ...bodyEl.querySelectorAll("dialog")].find((n) => n.open) || null;
      return bodyEl.querySelector(sel);
    },
    addEventListener(type, fn) { docListeners.set(type, [...(docListeners.get(type) || []), fn]); },
    removeEventListener(type, fn) { docListeners.set(type, (docListeners.get(type) || []).filter((x) => x !== fn)); },
    dispatchEvent(ev) { for (const fn of docListeners.get(ev.type) || []) fn(ev); },
  };
  const winListeners = new Map();
  const webWindow = {
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    addEventListener(type, fn) { winListeners.set(type, [...(winListeners.get(type) || []), fn]); },
    removeEventListener(type, fn) { winListeners.set(type, (winListeners.get(type) || []).filter((x) => x !== fn)); },
    dispatchEvent(ev) { for (const fn of winListeners.get(ev.type) || []) fn(ev); },
    scrollTo() {},
  };
  const mockResponse = (bodyText, contentType = "application/json", status = 200) => {
    const bytes = new TextEncoder().encode(bodyText);
    let read = false;
    return {
      ok: status >= 200 && status < 300,
      status,
      headers: { get: (name) => (String(name).toLowerCase() === "content-type" ? contentType : null) },
      body: {
        getReader: () => ({
          read: async () => { if (read) return { done: true, value: undefined }; read = true; return { done: false, value: bytes }; },
          cancel: async () => {},
          releaseLock() {},
        }),
      },
    };
  };
  const loc = { search: locationSearch, hash: "", pathname: "/" };
  const updateLocation = (url) => { if (typeof url === "string") loc.search = new URL(url, "http://localhost").search; };
  const clipboardWrites = [];
  const ctx = {
    window: webWindow, document: webDoc, Node: TestNode, Element: TestNode, HTMLElement: TestNode, Event, URL, DOMException,
    AbortController, URLSearchParams, TextEncoder, TextDecoder, Intl, crypto: webcrypto,
    navigator: { clipboard: { writeText: async (txt) => { clipboardWrites.push(txt); } } },
    MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
    location: loc,
    history: { pushState: (_s, _t, u) => updateLocation(u), replaceState: (_s, _t, u) => updateLocation(u) },
    fetch: (url, opts) => fetchImpl(url, opts, mockResponse),
    setTimeout, clearTimeout, setInterval, clearInterval,
  };
  webWindow.window = webWindow;
  vm.runInNewContext(asset("icons.js"), ctx);
  vm.runInNewContext(asset("ui.js"), ctx);
  vm.runInNewContext(asset("workers.js"), ctx);
  vm.runInNewContext(asset("source.js"), ctx);
  vm.runInNewContext(asset("web.js"), ctx);
  if (loadWorkInteraction) vm.runInNewContext(asset("work-interaction.js"), ctx);
  return { nodeFor: findById, ctx, webDoc, loc, clipboardWrites };
}

const SAMPLE_REPO_ID = "a".repeat(64);
const SAMPLE_REPO_2_ID = "b".repeat(64);
const SAMPLE_CATALOG = {
  schema: "kiln.repositories.v1",
  generated_at_ms: 1700000000000,
  repositories: [
    {
      identity: SAMPLE_REPO_ID,
      slug: "onixresearch/kiln",
      label: "Kiln Control Plane",
      canonical: {
        provider: "radicle",
        role: "canonical",
        url: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
        native_id: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
        normalized_key: "radicle:rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
        default_ref: "main",
        observed_revision: "deadbeef12345678",
        observed_at_ms: 1700000000000,
        status: "observed",
      },
      remotes: [
        {
          provider: "radicle",
          role: "canonical",
          url: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
          native_id: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
          normalized_key: "radicle:rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
          default_ref: "main",
          observed_revision: "deadbeef12345678",
          observed_at_ms: 1700000000000,
          status: "observed",
        },
        {
          provider: "github",
          role: "mirror",
          url: "https://github.com/OnixResearch/kiln.git",
          native_id: "OnixResearch/kiln",
          normalized_key: "github:onixresearch/kiln",
          default_ref: "main",
          observed_revision: "deadbeef12345678",
          observed_at_ms: 1700000000000,
          status: "observed",
        },
      ],
      local_copies: [
        {
          machine: "britton-desktop",
          path: "/home/brittonr/git/OnixResearch/kiln",
          vcs: "jj",
          head_commit: "deadbeef12345678",
          branch_or_bookmark: "main",
          dirty: false,
          ahead: 0,
          behind: 0,
          remotes: ["https://github.com/OnixResearch/kiln.git"],
          cairn_count: 3,
          observed_at_ms: 1700000000000,
          health: "available",
          error: null,
        },
        {
          machine: "aspen3",
          path: "/srv/src/kiln",
          vcs: "git",
          head_commit: "cafebabe87654321",
          branch_or_bookmark: "main",
          dirty: true,
          ahead: 1,
          behind: 0,
          remotes: ["https://github.com/OnixResearch/kiln.git"],
          cairn_count: 1,
          observed_at_ms: 1700000000000,
          health: "available",
          error: null,
        },
        {
          machine: "aspen1",
          path: "/srv/src/kiln",
          vcs: null,
          head_commit: null,
          branch_or_bookmark: null,
          dirty: null,
          ahead: null,
          behind: null,
          remotes: [],
          cairn_count: null,
          observed_at_ms: null,
          health: "unavailable",
          error: "ssh connection timed out",
        },
      ],
      sync_status: "diverged",
      admission: { catalog_imported: true, ci_configured: true, source_configured: true, fleet_enrolled: true },
      total_cairns: 4,
      non_claims: [
        "Radicle RIDs are recorded as read-only declarations; Kiln performs no Radicle initialization, signing, push, or mirror publication.",
      ],
    },
    {
      identity: SAMPLE_REPO_2_ID,
      slug: "local-scratch",
      label: "Local Scratch",
      canonical: null,
      remotes: [],
      local_copies: [
        {
          machine: "britton-desktop",
          path: "/home/brittonr/git/local-scratch",
          vcs: "git",
          head_commit: "112233445566",
          branch_or_bookmark: "main",
          dirty: true,
          ahead: 0,
          behind: 0,
          remotes: [],
          cairn_count: 0,
          observed_at_ms: 1700000000000,
          health: "available",
          error: null,
        },
      ],
      sync_status: "dirty",
      admission: { catalog_imported: false, ci_configured: false, source_configured: false, fleet_enrolled: false },
      total_cairns: 0,
      non_claims: ["Catalog registration does not grant CI workflow execution or Fleet agent dispatch."],
    },
  ],
  hosts: [
    { machine: "britton-desktop", health: "available", error: null, copy_count: 2 },
    { machine: "aspen1", health: "unavailable", error: "ssh connection timed out", copy_count: 1 },
  ],
  summary: { total: 2, synced: 0, diverged: 1, dirty: 1, unknown: 0, local_only: 0, total_local_copies: 4, total_cairns: 4 },
  sources: {
    catalog: { configured: true, available: true, error: null },
    local_scan: { configured: true, available: true, error: null },
    fleet: { configured: true, available: false, error: "fleet coordinator degraded on aspen1" },
  },
  bounded: true,
  truncated: false,
  non_claims: [],
};

// r[verify kiln.web.repository_import_ui]
test("validateRemoteInput accepts valid GitHub/Gitea/Radicle endpoints and rejects credentials, SSRF, malformed RIDs, and duplicate canonicals", () => {
  const ctx = { window: {}, document: { createElement: () => ({}) }, TextEncoder };
  vm.runInNewContext(asset("source.js"), ctx);
  const src = ctx.window.KilnSource;

  // Positive cases
  assert.equal(src.validateRemoteInput({ provider: "github", role: "mirror", url: "onixresearch/kiln" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "github", role: "mirror", url: "https://github.com/OnixResearch/kiln.git" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "github", role: "mirror", url: "git@github.com:OnixResearch/kiln.git" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "https://gitea.example.com/org/repo" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "ssh://git@gitea.example.com:2222/org/repo.git" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "radicle", role: "canonical", url: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE" }).ok, true);
  assert.equal(src.validateRemoteInput({ provider: "radicle", role: "canonical", url: "rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE" }).normalized, "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE");

  // Negative cases
  assert.equal(src.validateRemoteInput({ provider: "github", role: "mirror", url: "" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "https://user:secret@gitea.example.com/org/repo" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "github", role: "mirror", url: "https://token@github.com/org/repo" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "https://127.0.0.1/org/repo" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "https://localhost/org/repo" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "gitea", role: "mirror", url: "https://[::1]/org/repo" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "radicle", role: "canonical", url: "rad:z0InvalidZeroOIl" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "radicle", role: "canonical", url: "https://seed.radicle.garden/z2wsv.git" }).ok, false);
  assert.equal(src.validateRemoteInput({ provider: "radicle", role: "canonical", url: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE", attachRepo: SAMPLE_CATALOG.repositories[0] }).ok, false);

  // Filter and summary helpers
  assert.equal(src.filterRepositories(SAMPLE_CATALOG.repositories, { status: "diverged" }).length, 1);
  assert.equal(src.filterRepositories(SAMPLE_CATALOG.repositories, { status: "dirty" }).length, 1);
  assert.equal(src.filterRepositories(SAMPLE_CATALOG.repositories, { query: "aspen3" }).length, 1);
  assert.equal(src.filterRepositories(SAMPLE_CATALOG.repositories, { query: "scratch" }).length, 1);
  assert.match(src.formatLocalCopySummary(SAMPLE_CATALOG.repositories[0].local_copies[0]), /britton-desktop · jj · main · clean/);
  assert.match(src.formatLocalCopySummary(SAMPLE_CATALOG.repositories[0].local_copies[2]), /aspen1 · OFFLINE/);
});

// r[verify kiln.web.repository_import_ui]
// r[verify kiln.web.repository_import_ui.scenario.workbench]
test("Repositories hub renders matrix, degraded health banner, Radicle notice, Tier-2/3 #control-dialog flows, and honest Viewer read-only state", async () => {
  const posts = [];
  const h = createHarness({
    locationSearch: "?page=source&tab=repositories",
    fetchImpl: async (url, opts = {}, mockResponse) => {
      const u = String(url);
      if (opts.method === "POST") posts.push({ url: u, body: opts.body ? JSON.parse(opts.body) : null });
      if (u === "/api/session") return mockResponse(JSON.stringify({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } }));
      if (u === "/api/projects") return mockResponse(JSON.stringify({ available: [], enrolled: [] }));
      if (u.startsWith("/api/events")) return mockResponse(": heartbeat\n\n", "text/event-stream");
      if (u === "/api/repositories") return mockResponse(JSON.stringify(SAMPLE_CATALOG));
      if (u === "/api/sources") return mockResponse(JSON.stringify({ configured: true, sources: [{ id: "kiln-src", label: "Kiln", kind: "jj", status: "available", path: "/home/brittonr/git/OnixResearch/kiln" }] }));
      if (u === "/api/repositories/import" || u.endsWith("/mirrors") || u.endsWith("/remove") || u === "/api/repositories/refresh") {
        return mockResponse(JSON.stringify({ schema: "kiln.repository-mutation.v1", action: "ok", request_id: "req-1" }));
      }
      return mockResponse("{}");
    },
  });

  try {
    await new Promise((r) => setTimeout(r, 60));
    const root = h.nodeFor("repositories-content");
    const text = root.textContent;
    assert.match(text, /Kiln Control Plane/);
    assert.match(text, /aaaaaaaaaaaa/);
    assert.match(text, /Radicle · Canonical/);
    assert.match(text, /GitHub · Mirror/);
    assert.match(text, /britton-desktop · jj · main · clean/);
    assert.match(text, /aspen3 · git · main · dirty/);
    assert.match(text, /aspen1 · OFFLINE/);
    assert.match(text, /4 Cairns/);
    assert.match(text, /CI Configured/);
    assert.match(text, /Fleet Enrolled/);
    assert.match(text, /Observed/);

    // Degraded Fleet/Host health banner keeps local/catalog repos visible.
    const banner = h.nodeFor("repo-health-banner");
    assert.ok(banner);
    assert.match(banner.textContent, /Fleet census unavailable: fleet coordinator degraded on aspen1/);
    assert.match(banner.textContent, /aspen1 \(ssh connection timed out\)/);

    // Selecting Radicle shows non-mutation notice.
    const provSel = h.nodeFor("repo-import-provider");
    const radNotice = h.nodeFor("repo-radicle-notice");
    assert.equal(radNotice.hidden, true);
    provSel.value = "radicle";
    provSel.dispatchEvent({ type: "change" });
    assert.equal(radNotice.hidden, false);
    assert.match(radNotice.textContent, /never runs rad init, signing, push, sync, or delegate mutation/);

    // Negative: invalid RID stops inline without opening #control-dialog or posting.
    h.nodeFor("repo-import-url").value = "bad-rid";
    await h.nodeFor("repo-import-form").listeners.get("submit")[0]({ preventDefault() {} });
    assert.equal(h.nodeFor("repo-import-error").hidden, false);
    assert.equal(h.nodeFor("control-dialog").open, false);
    assert.equal(posts.length, 0);

    // Positive: attach Gitea mirror onto existing repo routes through Tier-2 #control-dialog with request_id.
    provSel.value = "gitea";
    provSel.dispatchEvent({ type: "change" });
    h.nodeFor("repo-import-attach").value = SAMPLE_REPO_ID;
    h.nodeFor("repo-import-attach").dispatchEvent({ type: "change" });
    assert.equal(h.nodeFor("repo-import-role").value, "mirror");
    h.nodeFor("repo-import-url").value = "https://gitea.example.com/onixresearch/kiln";
    const submitPromise = h.nodeFor("repo-import-form").listeners.get("submit")[0]({ preventDefault() {} });
    assert.equal(h.nodeFor("control-dialog").open, true);
    assert.match(h.nodeFor("confirm-cli-cmd").textContent, /kiln repositories add-mirror/);
    await h.nodeFor("control-form").listeners.get("submit")[0]({ preventDefault() {} });
    await submitPromise;
    assert.equal(posts.some((p) => p.url === `/api/repositories/${SAMPLE_REPO_ID}/mirrors` && p.body.provider === "gitea" && p.body.role === "mirror" && Boolean(p.body.request_id)), true);

    // Refresh census button calls POST /api/repositories/refresh.
    await h.nodeFor("repo-refresh-btn").listeners.get("click")[0]();
    assert.equal(posts.some((p) => p.url === "/api/repositories/refresh"), true);

    // Tier-3 Remove requires reason + notes disk files are never deleted.
    const removeBtn = root.querySelectorAll("button").find((b) => b.textContent === "Remove…");
    assert.ok(removeBtn);
    const remPromise = removeBtn.listeners.get("click")[0]();
    assert.equal(h.nodeFor("control-dialog").open, true);
    assert.match(h.nodeFor("confirm-warning").textContent, /NEVER deleted/);
    h.nodeFor("confirm-reason").value = "retiring test entry";
    await h.nodeFor("control-form").listeners.get("submit")[0]({ preventDefault() {} });
    await remPromise;
    assert.equal(posts.some((p) => p.url === `/api/repositories/${SAMPLE_REPO_ID}/remove` && p.body.reason === "retiring test entry" && Boolean(p.body.request_id)), true);

    // Filter chips narrow matrix cleanly.
    const dirtyChip = root.querySelectorAll("button").find((b) => b.dataset?.filterStatus === "dirty");
    dirtyChip.click();
    assert.equal(h.nodeFor("repo-matrix").children.length, 1);
    assert.match(h.nodeFor("repo-matrix").textContent, /Local Scratch/);
  } finally {
    h.nodeFor("logout").click();
  }

  // Viewer mode: read-only notice shown, mutation controls disabled/absent.
  const viewer = createHarness({
    locationSearch: "?page=source&tab=repositories",
    fetchImpl: async (url, _opts, mockResponse) => {
      const u = String(url);
      if (u === "/api/session") return mockResponse(JSON.stringify({ principal: { id: "local:view", label: "Viewer", role: "viewer" }, permissions: { read: true, operate: false } }));
      if (u === "/api/projects") return mockResponse(JSON.stringify({ available: [], enrolled: [] }));
      if (u.startsWith("/api/events")) return mockResponse(": heartbeat\n\n", "text/event-stream");
      if (u === "/api/repositories") return mockResponse(JSON.stringify(SAMPLE_CATALOG));
      if (u === "/api/sources") return mockResponse(JSON.stringify({ configured: false, sources: [] }));
      return mockResponse("{}");
    },
  });
  try {
    await new Promise((r) => setTimeout(r, 60));
    assert.match(viewer.nodeFor("repositories-content").textContent, /Read-only principal/);
    assert.equal(viewer.nodeFor("repo-import-submit").disabled, true);
    assert.equal(viewer.nodeFor("repo-refresh-btn").disabled, true);
    assert.equal(viewer.nodeFor("repositories-content").querySelectorAll("button").find((b) => b.textContent === "Remove…"), undefined);
  } finally {
    viewer.nodeFor("logout").click();
  }
});

// r[verify kiln.web.repository_import_ui]
test("1.1 Factory ribbon summary, ?inspect=repo:<id> Slide-Over, and Ctrl+K > import repo work end-to-end", async () => {
  const posts = [];
  const h = createHarness({
    locationSearch: "?page=overview",
    loadWorkInteraction: true,
    fetchImpl: async (url, opts = {}, mockResponse) => {
      const u = String(url);
      if (opts.method === "POST") posts.push({ url: u, body: opts.body ? JSON.parse(opts.body) : null });
      if (u === "/api/session") return mockResponse(JSON.stringify({ principal: { id: "local:op", label: "Op", role: "operator" }, permissions: { read: true, operate: true } }));
      if (u === "/api/projects") return mockResponse(JSON.stringify({ available: [], enrolled: [] }));
      if (u.startsWith("/api/events")) return mockResponse(": heartbeat\n\n", "text/event-stream");
      if (u === "/api/overview") return mockResponse(JSON.stringify({ projects: [], totals: { runs: 0, failed: 0, active: 0, queued: 0, unknown: 0, succeeded: 0, cancelled: 0 } }));
      if (u === "/api/workers") return mockResponse(JSON.stringify({ schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, summary: { targets: 0, configured_slots: 0, enrolled_slots: 0, active: 0, free_slots: 0, queued: 0 }, workers: [] }));
      if (u.startsWith("/api/activity")) return mockResponse(JSON.stringify({ items: [], total: 0 }));
      if (u === "/api/implementation/attention") return mockResponse(JSON.stringify({ total: 0, items: [] }));
      if (u === "/api/fleet/machines") return mockResponse(JSON.stringify({ machines: [] }));
      if (u.startsWith("/api/fleet/claims")) return mockResponse(JSON.stringify({ claims: [], foreign_claims: [], expired_claims: [] }));
      if (u === "/api/fleet/dispatch/status") return mockResponse(JSON.stringify({ state: "running" }));
      if (u === "/api/implementation/cairn") return mockResponse(JSON.stringify({ changes: [] }));
      if (u === "/api/fleet/attempts") return mockResponse(JSON.stringify({ attempts: [] }));
      if (u.startsWith("/api/code-review")) return mockResponse(JSON.stringify({ changes: [] }));
      if (u === "/api/repositories") return mockResponse(JSON.stringify(SAMPLE_CATALOG));
      if (u === "/api/work") return mockResponse(JSON.stringify({ objects: [] }));
      if (u === "/api/reviews") return mockResponse(JSON.stringify({ receipts: [] }));
      if (u.startsWith("/api/runs")) return mockResponse(JSON.stringify({ runs: [], total: 0 }));
      if (u.endsWith("/mirrors")) return mockResponse(JSON.stringify({ schema: "kiln.repository-mutation.v1", action: "mirror_attached" }));
      return mockResponse("{}");
    },
  });

  try {
    await new Promise((r) => setTimeout(r, 80));
    // 1.1 Factory telemetry ribbon displays compact repository drift summary.
    assert.match(h.nodeFor("factory-ribbon").textContent, /Repositories & Fleet Sync/);
    assert.match(h.nodeFor("factory-ribbon").textContent, /2 repos · 1 diverged · 1 dirty/);

    // Slide-Over ?inspect=repo:<id> opens and displays full remotes & fleet copy tables + inline Add Mirror.
    h.ctx.window.KilnFactory.openInspector("repo", SAMPLE_REPO_ID);
    await new Promise((r) => setTimeout(r, 60));
    assert.equal(h.nodeFor("slide-over-inspector").hidden, false);
    const bodyText = h.nodeFor("slide-over-body").textContent;
    assert.match(bodyText, /Remotes & mirrors/);
    assert.match(bodyText, /Fleet local working copies/);
    assert.match(bodyText, /britton-desktop/);
    assert.match(bodyText, /aspen1/);
    assert.match(bodyText, /ssh connection timed out/);

    h.nodeFor("slide-over-mirror-provider").value = "github";
    h.nodeFor("slide-over-mirror-role").value = "mirror";
    h.nodeFor("slide-over-mirror-url").value = "onixresearch/kiln-mirror";
    const mirrorForm = h.nodeFor("slide-over-body").querySelectorAll("form").find((f) => f.classList.contains("slide-over-mirror-form"));
    const mirrorPromise = mirrorForm.listeners.get("submit")[0]({ preventDefault() {} });
    assert.equal(h.nodeFor("control-dialog").open, true);
    await h.nodeFor("control-form").listeners.get("submit")[0]({ preventDefault() {} });
    await mirrorPromise;
    assert.equal(posts.some((p) => p.url === `/api/repositories/${SAMPLE_REPO_ID}/mirrors` && p.body.url === "onixresearch/kiln-mirror"), true);

    // Esc closes Slide-Over.
    h.webDoc.dispatchEvent({ type: "keydown", key: "Escape", target: h.webDoc.body, preventDefault() {} });
    assert.equal(h.nodeFor("slide-over-inspector").hidden, true);

    // Negative: malformed ?inspect=repo:../escape is rejected by readRoute().
    h.ctx.window.KilnFactory.navigate({ inspect: "repo:../escape" });
    assert.equal(h.ctx.window.KilnFactory.route().inspect, "");

    // Ctrl+K indexes repositories and offers > import repo verb.
    h.webDoc.dispatchEvent({ type: "keydown", key: "k", ctrlKey: true, target: h.webDoc.body, preventDefault() {} });
    await new Promise((r) => setTimeout(r, 60));
    const paletteInput = h.nodeFor("work-command-query");
    paletteInput.value = "Kiln Control";
    paletteInput.listeners.get("input")[0]();
    await new Promise((r) => setTimeout(r, 240));
    assert.match(h.nodeFor("work-command-results").textContent, /Kiln Control Plane/);

    paletteInput.value = "> import repo";
    paletteInput.listeners.get("input")[0]();
    await new Promise((r) => setTimeout(r, 240));
    assert.match(h.nodeFor("work-command-results").textContent, /> import repo/);
    const importOption = h.nodeFor("work-command-results").querySelectorAll("[role='option']")[0];
    assert.ok(importOption);
    importOption.click();
    await new Promise((r) => setTimeout(r, 80));
    assert.equal(h.ctx.window.KilnFactory.route().page, "source");
    assert.equal(h.ctx.window.KilnFactory.route().tab, "repositories");
  } finally {
    h.nodeFor("logout").click();
  }
});
