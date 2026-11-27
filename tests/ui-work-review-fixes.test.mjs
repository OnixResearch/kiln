import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) =>
  readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

function createHarness({ wideViewport = true, search = "" } = {}) {
  const mediaListeners = new Set();
  const windowListeners = new Map();
  let narrow = !wideViewport;

  class FakeNode {
    constructor() {
      this.parentNode = null;
      this.childNodes = [];
    }
    get parentElement() {
      return this.parentNode instanceof FakeElement ? this.parentNode : null;
    }
    get firstChild() { return this.childNodes[0] || null; }
    get lastChild() { return this.childNodes.at(-1) || null; }
    get firstElementChild() { return this.childNodes.find((n) => n instanceof FakeElement) || null; }
    get children() { return this.childNodes.filter((n) => n instanceof FakeElement); }
    get childElementCount() { return this.children.length; }
    get isConnected() {
      let cur = this;
      while (cur.parentNode) cur = cur.parentNode;
      return cur === doc.documentElement || cur === doc.body;
    }
    remove() {
      if (!this.parentNode) return;
      const idx = this.parentNode.childNodes.indexOf(this);
      if (idx >= 0) this.parentNode.childNodes.splice(idx, 1);
      this.parentNode = null;
    }
    append(...items) {
      for (const item of items) {
        if (item == null) continue;
        if (item instanceof FakeFragment) {
          this.append(...item.childNodes.slice());
          continue;
        }
        const node = typeof item === "string" ? new FakeText(item) : item;
        if (node.parentNode) node.remove();
        node.parentNode = this;
        this.childNodes.push(node);
      }
    }
    prepend(...items) {
      const built = [];
      for (const item of items) {
        if (item == null) continue;
        if (item instanceof FakeFragment) {
          built.push(...item.childNodes.slice());
          continue;
        }
        const node = typeof item === "string" ? new FakeText(item) : item;
        if (node.parentNode) node.remove();
        node.parentNode = this;
        built.push(node);
      }
      this.childNodes.unshift(...built);
    }
    insertBefore(newNode, refNode) {
      if (!refNode) { this.append(newNode); return newNode; }
      if (newNode.parentNode) newNode.remove();
      const idx = this.childNodes.indexOf(refNode);
      if (idx < 0) this.append(newNode);
      else { newNode.parentNode = this; this.childNodes.splice(idx, 0, newNode); }
      return newNode;
    }
    after(...items) {
      if (!this.parentNode) return;
      const idx = this.parentNode.childNodes.indexOf(this);
      const ref = this.parentNode.childNodes[idx + 1] || null;
      for (const item of items) {
        if (item == null) continue;
        const node = typeof item === "string" ? new FakeText(item) : item;
        this.parentNode.insertBefore(node, ref);
      }
    }
    replaceChildren(...items) {
      for (const child of this.childNodes.slice()) child.parentNode = null;
      this.childNodes = [];
      this.append(...items);
    }
    replaceWith(...items) {
      if (!this.parentNode) return;
      const parent = this.parentNode;
      const idx = parent.childNodes.indexOf(this);
      const ref = parent.childNodes[idx + 1] || null;
      this.remove();
      for (const item of items) {
        if (item == null) continue;
        const node = typeof item === "string" ? new FakeText(item) : item;
        parent.insertBefore(node, ref);
      }
    }
    contains(other) {
      let cur = other;
      while (cur) {
        if (cur === this) return true;
        cur = cur.parentNode;
      }
      return false;
    }
  }

  class FakeText extends FakeNode {
    constructor(text) {
      super();
      this.nodeType = 3;
      this.nodeValue = String(text ?? "");
    }
    get textContent() { return this.nodeValue; }
    set textContent(v) { this.nodeValue = String(v ?? ""); }
  }

  class FakeFragment extends FakeNode {
    constructor() {
      super();
      this.nodeType = 11;
    }
  }

  class FakeElement extends FakeNode {
    constructor(tag, ns = null) {
      super();
      this.nodeType = 1;
      this.tagName = String(tag).toLowerCase();
      this.localName = this.tagName;
      this.namespaceURI = ns;
      this.attrs = new Map();
      this.style = {};
      this.listeners = new Map();
      this._value = "";
      this.checked = false;
      this.disabled = false;
      this.hidden = false;
      this.open = false;
      this.required = false;
      this.readOnly = false;
      this.multiple = false;
      this.scrollTop = 0;
      this.scrollLeft = 0;
      this.scrollWidth = 600;
      this.scrollHeight = 400;
      this.offsetLeft = 0;
      this.offsetTop = 0;
      this.offsetWidth = 100;
      this.offsetHeight = 40;
      this._dataset = {};
      const self = this;
      this.dataset = new Proxy(this._dataset, {
        set(target, prop, val) {
          target[prop] = String(val);
          const attr = "data-" + String(prop).replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
          self.attrs.set(attr, String(val));
          return true;
        },
        get(target, prop) { return target[prop]; },
      });
      this.classList = {
        _set() { return new Set((self.attrs.get("class") || "").split(/\s+/).filter(Boolean)); },
        add(...names) {
          const s = this._set();
          for (const n of names) if (n) s.add(n);
          self.attrs.set("class", [...s].join(" "));
        },
        remove(...names) {
          const s = this._set();
          for (const n of names) s.delete(n);
          self.attrs.set("class", [...s].join(" "));
        },
        toggle(name, force) {
          const s = this._set();
          const want = force === undefined ? !s.has(name) : Boolean(force);
          if (want) s.add(name); else s.delete(name);
          self.attrs.set("class", [...s].join(" "));
          return want;
        },
        contains(name) { return this._set().has(name); },
      };
    }
    get id() { return this.attrs.get("id") || ""; }
    set id(v) { this.setAttribute("id", v); }
    get className() { return this.attrs.get("class") || ""; }
    set className(v) { this.setAttribute("class", v); }
    get href() { return this.attrs.get("href") || ""; }
    set href(v) { this.setAttribute("href", v); }
    get name() { return this.attrs.get("name") || ""; }
    set name(v) { this.setAttribute("name", v); }
    get type() { return this.attrs.get("type") || (this.tagName === "button" ? "submit" : ""); }
    set type(v) { this.setAttribute("type", v); }
    get title() { return this.attrs.get("title") || ""; }
    set title(v) { this.setAttribute("title", v); }
    get tabIndex() { return Number(this.attrs.get("tabindex") ?? -1); }
    set tabIndex(v) { this.setAttribute("tabindex", String(v)); }
    get value() {
      if (this.tagName === "select" && !this._value) {
        const opt = this.options.find((o) => o.selected) || this.options[0];
        return opt ? opt.value : "";
      }
      return this._value;
    }
    set value(v) {
      this._value = String(v ?? "");
      if (this.tagName === "select") {
        for (const opt of this.options) opt.selected = opt.value === this._value;
      }
    }
    get options() { return this.querySelectorAll("option"); }
    get selectedOptions() { return this.options.filter((o) => o.selected); }
    get textContent() { return this.childNodes.map((n) => n.textContent).join(""); }
    set textContent(v) {
      this.childNodes = [];
      if (v !== "" && v != null) this.append(String(v));
    }
    setAttribute(name, value) {
      this.attrs.set(name, String(value));
      if (name === "hidden") this.hidden = true;
      if (name === "open") this.open = true;
      if (name.startsWith("data-")) {
        const prop = name.slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
        this._dataset[prop] = String(value);
      }
    }
    getAttribute(name) {
      if (name === "hidden") return this.hidden ? "" : null;
      if (name === "open") return this.open ? "" : null;
      return this.attrs.has(name) ? this.attrs.get(name) : null;
    }
    hasAttribute(name) { return this.getAttribute(name) !== null; }
    removeAttribute(name) {
      this.attrs.delete(name);
      if (name === "hidden") this.hidden = false;
      if (name === "open") this.open = false;
    }
    addEventListener(type, fn) {
      if (!this.listeners.has(type)) this.listeners.set(type, []);
      this.listeners.get(type).push(fn);
    }
    removeEventListener(type, fn) {
      const list = this.listeners.get(type);
      if (!list) return;
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    }
    dispatchEvent(event) {
      const ev = { target: this, currentTarget: this, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, ...event };
      for (const fn of this.listeners.get(ev.type) || []) fn(ev);
      return !ev.defaultPrevented;
    }
    focus() { doc.activeElement = this; }
    scrollIntoView() {}
    getClientRects() { return this.hidden ? [] : [{ left: 0, top: 0, width: 100, height: 24 }]; }
    getBoundingClientRect() { return { left: 0, right: 100, top: 0, bottom: 24, width: 100, height: 24 }; }
    showModal() { this.open = true; this.setAttribute("open", ""); }
    close() {
      this.open = false;
      this.removeAttribute("open");
      this.dispatchEvent({ type: "close" });
    }
    showPopover() { this._popoverOpen = true; }
    hidePopover() { this._popoverOpen = false; }
    click() {
      if (this.disabled) return;
      if (this.tagName === "input" && this.type === "checkbox") {
        this.checked = !this.checked;
        this.dispatchEvent({ type: "change" });
      }
      const ev = { type: "click", button: 0, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, target: this, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {} };
      for (const fn of this.listeners.get("click") || []) fn(ev);
    }
    matches(sel) {
      return matchesAny(this, sel, null);
    }
    closest(sel) {
      let cur = this;
      while (cur) {
        if (cur instanceof FakeElement && matchesAny(cur, sel, null)) return cur;
        cur = cur.parentNode;
      }
      return null;
    }
    querySelectorAll(sel) {
      const out = [];
      const walk = (n) => {
        for (const child of n.childNodes) {
          if (child instanceof FakeElement) {
            if (matchesAny(child, sel, this)) out.push(child);
            walk(child);
          }
        }
      };
      walk(this);
      return out;
    }
    querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
  }

  function matchesSimple(el, sel) {
    let rest = sel.trim();
    if (!rest) return false;
    if (rest.includes(":popover-open")) {
      if (!el._popoverOpen) return false;
      rest = rest.replace(/:popover-open/g, "");
    }
    if (rest.includes(":checked")) {
      if (!el.checked) return false;
      rest = rest.replace(/:checked/g, "");
    }
    let notMatch = null;
    const notRe = /:not\(([^)]+)\)/g;
    let nm;
    while ((nm = notRe.exec(rest))) {
      notMatch = nm[1];
      if (matchesSimple(el, notMatch)) return false;
    }
    rest = rest.replace(/:not\([^)]+\)/g, "");
    const tagMatch = /^([a-z0-9*-]+)/i.exec(rest);
    if (tagMatch) {
      if (tagMatch[1] !== "*" && el.tagName !== tagMatch[1].toLowerCase()) return false;
      rest = rest.slice(tagMatch[1].length);
    }
    const idRe = /#([A-Za-z0-9_-]+)/g;
    let m;
    while ((m = idRe.exec(rest))) {
      if (el.id !== m[1]) return false;
    }
    rest = rest.replace(/#[A-Za-z0-9_-]+/g, "");
    const attrRe = /\[([A-Za-z0-9_-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?\]/g;
    while ((m = attrRe.exec(rest))) {
      const name = m[1];
      const expected = m[2] ?? m[3] ?? m[4];
      const actual = el.getAttribute(name);
      if (expected === undefined) {
        if (actual === null) return false;
      } else if (actual !== expected) return false;
    }
    rest = rest.replace(/\[[^\]]+\]/g, "");
    const clsRe = /\.([A-Za-z0-9_-]+)/g;
    while ((m = clsRe.exec(rest))) {
      if (!el.classList.contains(m[1])) return false;
    }
    return true;
  }

  function matchesChain(el, chain, root) {
    const tokens = [];
    let buf = "", inQuote = null, bracketDepth = 0;
    for (const ch of chain.trim()) {
      if (inQuote) {
        buf += ch;
        if (ch === inQuote) inQuote = null;
      } else if (ch === '"' || ch === "'") {
        buf += ch;
        inQuote = ch;
      } else if (ch === "[") {
        buf += ch;
        bracketDepth += 1;
      } else if (ch === "]") {
        buf += ch;
        bracketDepth = Math.max(0, bracketDepth - 1);
      } else if (/\s/.test(ch) && bracketDepth === 0) {
        if (buf) { tokens.push(buf); buf = ""; }
      } else {
        buf += ch;
      }
    }
    if (buf) tokens.push(buf);
    let idx = tokens.length - 1;
    let cur = el;
    let direct = false;
    while (idx >= 0) {
      const tok = tokens[idx];
      if (tok === ">") { direct = true; idx -= 1; continue; }
      if (idx === tokens.length - 1) {
        if (!matchesSimple(cur, tok)) return false;
        idx -= 1;
        continue;
      }
      if (direct) {
        cur = cur.parentElement;
        if (!cur || !matchesSimple(cur, tok)) return false;
        direct = false;
        idx -= 1;
      } else {
        let p = cur.parentElement;
        let ok = false;
        while (p && p !== root?.parentElement) {
          if (matchesSimple(p, tok)) { cur = p; ok = true; break; }
          p = p.parentElement;
        }
        if (!ok) return false;
        idx -= 1;
      }
    }
    return true;
  }

  function matchesAny(el, selector, root) {
    return selector.split(",").some((part) => matchesChain(el, part, root));
  }

  const docListeners = new Map();
  const doc = {
    body: new FakeElement("body"),
    documentElement: new FakeElement("html"),
    activeElement: null,
    title: "",
    createElement: (tag) => new FakeElement(tag),
    createElementNS: (ns, tag) => new FakeElement(tag, ns),
    createTextNode: (t) => new FakeText(t),
    createDocumentFragment: () => new FakeFragment(),
    getElementById(id) { return this.documentElement.querySelector(`#${id}`); },
    querySelector(sel) { return this.documentElement.querySelector(sel); },
    querySelectorAll(sel) { return this.documentElement.querySelectorAll(sel); },
    addEventListener(type, fn) {
      if (!docListeners.has(type)) docListeners.set(type, []);
      docListeners.get(type).push(fn);
    },
    dispatchEvent(ev) {
      for (const fn of docListeners.get(ev.type) || []) fn(ev);
    },
    dispatchKey(key, extra = {}) {
      const ev = {
        key, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false,
        target: doc.activeElement || doc.body, defaultPrevented: false,
        preventDefault() { this.defaultPrevented = true; },
        stopPropagation() {},
        ...extra,
      };
      for (const fn of docListeners.get("keydown") || []) fn(ev);
      return ev;
    },
  };
  doc.documentElement.append(doc.body);

  const historyCalls = [];
  const locationObj = { pathname: "/", search };
  const historyObj = {
    replaceState(_state, _title, url) {
      historyCalls.push(url);
      const q = String(url).indexOf("?");
      locationObj.search = q >= 0 ? String(url).slice(q) : "";
    },
    pushState(_state, _title, url) {
      const q = String(url).indexOf("?");
      locationObj.search = q >= 0 ? String(url).slice(q) : "";
    },
  };

  class FakeMutationObserver {
    constructor(cb) { this.cb = cb; }
    observe() {}
    disconnect() {}
  }

  let uuidCounter = 0;
  const cryptoObj = {
    randomUUID: () => `00000000-0000-4000-8000-${String(++uuidCounter).padStart(12, "0")}`,
    subtle: webcrypto.subtle,
  };

  const windowObj = {
    location: locationObj,
    history: historyObj,
    innerWidth: wideViewport ? 1600 : 1024,
    innerHeight: 900,
    matchMedia(query) {
      return {
        media: query,
        get matches() { return query === "(min-width: 90rem)" ? !narrow : false; },
        addEventListener(ev, fn) { if (ev === "change") mediaListeners.add(fn); },
      };
    },
    addEventListener(type, fn) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(fn);
    },
    dispatchEvent(ev) {
      for (const fn of windowListeners.get(ev.type) || []) fn(ev);
    },
  };

  const context = {
    window: windowObj,
    document: doc,
    Element: FakeElement,
    HTMLElement: FakeElement,
    Node: FakeNode,
    URL,
    URLSearchParams,
    location: locationObj,
    history: historyObj,
    crypto: cryptoObj,
    AbortController,
    TextEncoder,
    TextDecoder,
    MutationObserver: FakeMutationObserver,
    CustomEvent: class { constructor(type, init = {}) { this.type = type; this.detail = init.detail; } },
    Event: class { constructor(type) { this.type = type; } },
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
  };
  windowObj.window = windowObj;
  context.globalThis = context;
  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  return { context, doc, window: windowObj, historyCalls, FakeElement };
}

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
const HEX_A = "a".repeat(64);
const HEX_B = "b".repeat(64);
const COMMIT_0 = "0".repeat(40);
const COMMIT_1 = "1".repeat(40);
const REV_1 = "1".repeat(64);

// --- Findings 1, 2, 4, 5, 12: Code Review ---

test("Code Review: renders agent submission without ReferenceError, skips double-load/diff-wipe on line click, retains focus, skips collapsed noise, matches renamed base path", async () => {
  const { context, doc, window } = createHarness({ wideViewport: true });
  vm.runInNewContext(asset("code-review.js"), context);
  const cr = window.KilnCodeReview;

  const root = doc.createElement("div");
  root.id = "code-review-content";
  const slot = doc.createElement("div");
  slot.id = "page-primary-action";
  const inspector = doc.createElement("aside");
  inspector.id = "slide-over-inspector";
  inspector.hidden = true;
  doc.body.append(slot, root, inspector);

  const changeDetail = {
    change: {
      id: HEX_A, version: 1, project: "kiln", repository: "repo-1", target_ref: "refs/heads/main",
      owner: "local:alice", title: "Agent submitted fix", description: "desc", state: "submitted", wip: false,
      authorship: { kind: "agent", harness: "omp", model: "claude" },
      revisions: [{ id: REV_1, number: 1, commit: COMMIT_1, base: COMMIT_0, author: "local:agent", created_ms: 1700000000000 }],
      reviewers: {}, attention: [], threads: [],
      submission: {
        submitted_by: "local:agent",
        agent: { harness: "omp", model: "sonnet-4" },
        submitted_ms: 1700000001000,
        result_commit: COMMIT_1,
        candidate: { id: "cand-1", revision: REV_1, commit: COMMIT_1, target_ref: "refs/heads/main", target: COMMIT_0, ci_run: "run-1", dependencies: [] },
      },
    },
    checklist: { ready: true, items: [] },
    drafts: [], operations: [], ci_runs: [],
  };
  const snapshot = {
    schema: "kiln.code-review.v1", configured: true, principal: "local:alice", can_write: true,
    project: "kiln", repository: "repo-1", target_refs: ["refs/heads/main"], capabilities: [],
    policy: { id: "pol-1", eligible_reviewers: ["local:bob"], minimum_approvals: 1, require_ci: true, ownership: { rules: [], teams: {} } },
    changes: [changeDetail.change], total: 1, next_offset: null, saved_views: [], notifications: [], notifications_total: 0, operations: [], operations_total: 0,
  };
  const diff = {
    old_commit: COMMIT_0, new_commit: COMMIT_1, truncated: false,
    noise: { identity: HEX_B, rules: [{ id: "lock", reason: "lockfile" }], files: { "Cargo.lock": "lock" } },
    review_hunks: {
      exact_base: true,
      files: [
        { path: "src/new_name.rs", total: 1, viewed: 0, hunks: [{ digest: "d1", viewed: false }] },
        { path: "Cargo.lock", total: 1, viewed: 0, hunks: [{ digest: "d2", viewed: false }] },
      ],
    },
    files: [
      {
        old_path: "src/old_name.rs", new_path: "src/new_name.rs", renamed: true, binary: false, truncated: false,
        hunks: [{ header: "@@ -1,1 +1,1 @@", old_start: 1, old_lines: 1, new_start: 1, new_lines: 1, lines: [
          { kind: "deletion", old_line: 1, new_line: null, text: "-old" },
          { kind: "addition", old_line: null, new_line: 1, text: "+new" },
        ] }],
      },
      {
        old_path: "Cargo.lock", new_path: "Cargo.lock", renamed: false, binary: false, truncated: false,
        hunks: [{ header: "@@ -1,1 +1,1 @@", old_start: 1, old_lines: 1, new_start: 1, new_lines: 1, lines: [
          { kind: "addition", old_line: null, new_line: 1, text: "+lock" },
        ] }],
      },
    ],
  };

  let apiCalls = 0;
  let routeState = { page: "code-review", review_change: HEX_A };
  const api = async (url) => {
    apiCalls += 1;
    if (url.startsWith("/api/code-review?")) return snapshot;
    if (url === `/api/code-review/changes/${HEX_A}`) return changeDetail;
    if (url.startsWith(`/api/code-review/changes/${HEX_A}/diff`)) return diff;
    throw new Error(`Unexpected url ${url}`);
  };
  const navigate = (patch) => {
    const merged = { ...routeState, ...patch };
    for (const [k, v] of Object.entries(merged)) if (!v) delete merged[k];
    routeState = merged;
    // Simulate web.js async restoreRoute() echo
    setTimeout(() => { void cr.load(root, { principal: "local:alice", route: new URLSearchParams(Object.entries(routeState)), navigate, api }); }, 0);
  };

  await cr.load(root, { principal: "local:alice", route: new URLSearchParams(Object.entries(routeState)), navigate, api });
  // Finding 1: agent submission rendered cleanly
  assert.match(root.textContent, /omp · sonnet-4 \(declared\)/);
  const callsAfterInitial = apiCalls;

  // Finding 5: data-cr-key present on file links, file mark, and hunk mark
  const fileLink = root.querySelector('.cr-file-row > a');
  const hunkMark = root.querySelector('.cr-hunk-mark');
  const fileMark = root.querySelector('.cr-mark');
  assert.equal(fileLink.dataset.crKey, "file-link:src/new_name.rs");
  assert.equal(fileMark.dataset.crKey, "file-mark:src/new_name.rs");
  assert.equal(hunkMark.dataset.crKey, "hunk-mark:src/new_name.rs:d1");

  // Finding 5: focus retained on hunkMark across draw/refresh, and j/k skips collapsed noise file
  hunkMark.focus();
  assert.equal(doc.activeElement, hunkMark);
  await cr.refresh();
  assert.equal(doc.activeElement.dataset.crKey, "hunk-mark:src/new_name.rs:d1");
  const callsBeforeLineClick = apiCalls;

  // Clicking base line on renamed file sets review_file=src/old_name.rs:
  // Finding 2: zero extra API fetches (no double load, no diff wipe), and Finding 5: renamed file stays highlighted via old_path
  const baseLineLink = root.querySelector('.cr-line-number a[aria-label="base line 1"]');
  assert.ok(baseLineLink);
  const diffScroll = root.querySelector(".cr-diff-scroll");
  diffScroll.scrollTop = 42;
  baseLineLink.click();
  await tick(20);
  assert.equal(apiCalls, callsBeforeLineClick, "line click and restoreRoute echo must not re-fetch APIs");
  assert.equal(root.querySelector(".cr-diff-scroll").scrollTop, 42, "diff scroll preserved on line click");
  assert.equal(root.querySelector('.cr-file-row > a[aria-current="true"]').dataset.crKey, "file-link:src/new_name.rs");

  // Finding 5: j shortcut must NOT navigate into collapsed .cr-noise:not([open])
  doc.activeElement = doc.body;
  doc.dispatchKey("j");
  assert.equal(routeState.review_file, "src/new_name.rs", "collapsed noise file skipped");

  // Finding 4: g-chord and Slide-Over Inspector suppress shortcuts
  doc.documentElement.dataset.kilnChord = "g";
  doc.dispatchKey("m");
  assert.equal(root.querySelector("table.cr-diff.cr-split"), null, "g-chord suppresses 'm' layout toggle");
  delete doc.documentElement.dataset.kilnChord;

  inspector.hidden = false;
  doc.dispatchKey("m");
  assert.equal(root.querySelector("table.cr-diff.cr-split"), null, "open inspector suppresses 'm'");
  inspector.hidden = true;

  // Finding 12: collapsed drawer close button & Escape dismiss
  doc.dispatchKey("m"); // switch to split -> rail collapses
  await tick();
  const workbench = root.querySelector(".cr-workbench");
  assert.equal(workbench.classList.contains("is-rail-collapsed"), true);
  root.querySelector("#cr-file-rail-toggle").click();
  await tick();
  assert.equal(workbench.classList.contains("is-rail-drawer-open"), true);
  assert.ok(root.querySelector(".cr-file-rail-close"));
  doc.dispatchKey("Escape");
  assert.equal(workbench.classList.contains("is-rail-drawer-open"), false);
});

// --- Finding 3 & 11: Cairn artifact preservation, authoring UX, plan deletion, lifecycle & Atlas ---

test("Cairn: preserves artifact edits on error, keeps authoring form on unknown/error with hints & datalist, allows deletion without text, and completes Atlas", async () => {
  const { context, doc, window } = createHarness();
  vm.runInNewContext(asset("cairn.js"), context);
  const cairn = window.KilnCairn;

  const repoHash = HEX_A;
  const revHash = HEX_B;
  const root = doc.createElement("div");
  doc.body.append(root);

  let failArtifactPost = true;
  let failAuthoringUnknown = true;
  const index = { schema: "kiln.implementation-workspace.v1", principal: "local:op", changes: [] };
  const authoringSnapshot = {
    schema: "kiln.cairn-authoring.v1",
    repository: repoHash,
    snapshot: {
      revision: revHash,
      change: null,
      graph: {
        command: "change graph", layout: "cairn", valid: true, active_change_count: 2,
        nodes: [
          { name: "alpha", dependencies: ["beta"] },
          { name: "beta", dependencies: [] },
        ],
        archived_changes: ["old-one"],
      },
    },
  };
  const artifactRead = {
    repository: repoHash,
    revision: revHash,
    artifact: { change: "alpha", artifact: "proposal", content_blake3: "c".repeat(64), content: "Original content" },
  };

  await cairn.load(root, {
    principal: "local:op",
    canOperate: () => true,
    route: { page: "cairn" },
    navigate: () => {},
    confirmControl: async () => ({ confirmed: true, requestId: "req-1", reason: "ok" }),
    api: async (url, opts) => {
      if (url === "/api/implementation/cairn") return index;
      if (url === "/api/implementation/cairn/authoring") return {
        schema: "kiln.cairn-authoring.v1",
        capabilities: { create: true, edit_dependencies: true, edit_artifacts: true },
        roots: [{ repository: repoHash, label: "kiln" }],
      };
      if (url.startsWith("/api/implementation/cairn/authoring?repository=")) return authoringSnapshot;
      if (url.startsWith("/api/implementation/cairn/authoring/artifact?")) return artifactRead;
      if (url === "/api/implementation/cairn/authoring/artifact" && opts?.method === "POST") {
        if (failArtifactPost) throw new Error("409 conflict");
        return { repository: repoHash, change: "alpha", artifact: "proposal", outcome: "applied", content_blake3: "d".repeat(64) };
      }
      if (url === "/api/implementation/cairn/authoring" && opts?.method === "POST") {
        if (failAuthoringUnknown) return {
          schema: "kiln.cairn-authoring.v1", repository: repoHash, outcome: "unknown",
          action: opts.body.action,
        };
      }
      throw new Error(`Unexpected ${url}`);
    },
  });

  const authorDetails = root.querySelector("details.cairn-authoring");
  assert.ok(authorDetails);
  authorDetails.open = true;
  authorDetails.dispatchEvent({ type: "toggle" });
  await tick(10);

  // Finding 11: inline explanation for invalid names and cycles + datalist for prerequisite
  const inputs = authorDetails.querySelectorAll("input");
  const newNameInput = inputs[0];
  const prereqInput = inputs[1];
  const changeSelect = authorDetails.querySelectorAll("select")[1];
  assert.ok(prereqInput.getAttribute("list"));
  assert.ok(authorDetails.querySelector(`#${prereqInput.getAttribute("list")}`));

  newNameInput.value = "Bad_Name";
  newNameInput.dispatchEvent({ type: "input" });
  assert.match(authorDetails.querySelector(".cairn-authoring-hint").textContent, /Invalid new change name/);
  newNameInput.value = "";
  newNameInput.dispatchEvent({ type: "input" });

  changeSelect.value = "beta";
  changeSelect.dispatchEvent({ type: "change" });
  prereqInput.value = "alpha";
  prereqInput.dispatchEvent({ type: "input" });
  assert.match(authorDetails.querySelector(".cairn-authoring-hint").textContent, /cycle/);

  // Keep authoring form on unknown outcome
  newNameInput.value = "gamma-new";
  newNameInput.dispatchEvent({ type: "input" });
  const createBtn = authorDetails.querySelectorAll("button").find((b) => b.textContent === "Create change");
  assert.equal(createBtn.disabled, false);
  createBtn.click();
  await tick(10);
  assert.equal(newNameInput.value, "gamma-new", "authoring form preserved on unknown outcome");
  assert.equal(createBtn.disabled, true, "writes blocked until graph refreshed");

  // Finding 3: artifact edits preserved on save failure, cleared on success
  changeSelect.value = "alpha";
  changeSelect.dispatchEvent({ type: "change" });
  const openArtBtn = authorDetails.querySelectorAll("button").find((b) => b.textContent === "Open artifact");
  openArtBtn.click();
  await tick(10);
  const textarea = authorDetails.querySelector("textarea");
  const saveArtBtn = authorDetails.querySelectorAll("button").find((b) => b.textContent === "Save artifact");
  assert.ok(textarea && saveArtBtn);
  textarea.value = "Unsaved edits must survive error";
  textarea.dispatchEvent({ type: "input" });
  assert.equal(saveArtBtn.disabled, false);
  saveArtBtn.click();
  await tick(10);
  assert.equal(textarea.value, "Unsaved edits must survive error", "textarea edits preserved on failure");
  assert.equal(textarea.disabled, false);
  assert.equal(saveArtBtn.disabled, false);

  failArtifactPost = false;
  saveArtBtn.click();
  await tick(10);
  assert.equal(authorDetails.querySelector("textarea"), null, "fields cleared after confirmed save");

  // Finding 11: renderPlanReview allows "Request deletion" without dummy text & preserves unsaved step drafts
  const planHost = doc.createElement("div");
  const fbPosts = [];
  cairn.renderPlanReview(planHost, {
    workId: HEX_A, planId: HEX_B, revision: 1, canOperate: true,
    steps: [{ id: "s1", text: "Step 1" }, { id: "s2", text: "Step 2" }],
    annotations: [],
    api: async (_url, opts) => { fbPosts.push(opts.body); return { annotation_id: "ann-1" }; },
  });
  const s1Select = planHost.querySelector('[data-step-id="s1"] select');
  const s1Input = planHost.querySelector('[data-step-id="s1"] input[type="text"]');
  const s2Input = planHost.querySelector('[data-step-id="s2"] input[type="text"]');
  s2Input.value = "unsaved note on s2";
  s2Input.dispatchEvent({ type: "input" });
  s1Select.value = "delete";
  s1Select.dispatchEvent({ type: "change" });
  assert.equal(s1Input.required, false, "Request deletion does not require dummy text");
  planHost.querySelector('[data-step-id="s1"] form').dispatchEvent({ type: "submit" });
  await tick(10);
  assert.equal(fbPosts.length, 1);
  assert.equal(fbPosts[0].note_kind, "delete");
  assert.equal(planHost.querySelector('[data-step-id="s2"] input[type="text"]').value, "unsaved note on s2", "unsaved step input preserved across renderSteps");

  // Finding 11: Atlas inspectorHost populated, aria-pressed set, >26 pin labels (AA, AB)
  const nodes28 = Array.from({ length: 28 }, (_, i) => ({ id: `n-${i}`, label: `Node ${i}`, role: "worker" }));
  const atlasEl = cairn.renderAtlasTopology({
    schema: "kiln.cairn-atlas.v1", identity: HEX_A, title: "Large Atlas",
    nodes: nodes28,
    routes: [{ from: "n-0", to: "n-27", label: "edge", provenance: "declared" }],
    chapters: [{ id: "ch1", title: "Phase 1", nodes: ["n-0"] }],
  });
  const pins = atlasEl.querySelectorAll(".cairn-atlas-pin");
  assert.equal(pins.length, 28);
  assert.match(pins[26].textContent, /^AA ·/);
  assert.match(pins[27].textContent, /^AB ·/);
  assert.equal(pins[0].getAttribute("aria-pressed"), "false");
  pins[0].click();
  assert.equal(pins[0].getAttribute("aria-pressed"), "true");
  assert.match(atlasEl.querySelector(".cairn-atlas-inspector").textContent, /Node 0.*n-0 → n-27/);
});

// --- Findings 4, 6, 8, 9, 10, 15: Work, Kanban, Interaction, Evidence ---

test("Work: Kanban unknown recovery disables commit & refreshes columns, modal inline errors, 409 reload, discussion draft preservation, and WorkInteraction AbortSignal", async () => {
  const { context, doc, window } = createHarness();
  const bridgeCalls = [];
  const commands = [];
  let recordHead = "head-1";
  let failUnknown = true;
  let failConflict = true;
  const issue1 = {
    id: "issue-1", kind: "issue", head: "head-1", created_by: "local:op", created_at_ms: 1000, updated_at_ms: 2000,
    fields: { title: "Fix board", description: "", status: "backlog", priority: 2, owner: "local:op", assignees: [], labels: [], links: [] },
    comments: [], updates: [],
  };
  let boardProjection = {
    schema: "kiln.work.board.v1", principal: "local:op", bounded: true, lane_by: "ci_project",
    matched_issues: 1, shown_cards: 1, list_only: 0, snapshot_objects: 1, object_limit: 1024,
    card_limit: 100, columns_truncated: 0, complete: true,
    columns: [{ status: "", teams: [], ci: { id: "p1", label: "Core", repository: "repo-1", repository_key: HEX_A, scope_root: "" }, total: 1, cards: [{ id: "issue-1", head: "head-1", title: "Fix board", status: "backlog", priority: 2, assignees: [], labels: [] }] }],
    move_columns: [
      { status: "backlog", builtin: true, retired: false, teams: [] },
      { status: "started", builtin: true, retired: false, teams: [] },
    ],
    other: { statuses: [], total: 0, cards: [] },
  };
  let currentRoute = { page: "work", work_layout: "board" };
  const notices = [];
  const bridge = {
    session: () => ({ principal: "local:op", can_write: true }),
    route: () => currentRoute,
    navigate: (patch) => { currentRoute = { ...currentRoute, ...patch }; },
    notice: (msg) => notices.push(msg),
    runLink: (id, label) => { const a = doc.createElement("a"); a.textContent = label; return a; },
    api: async (path, opts) => {
      bridgeCalls.push({ path, opts });
      if (path === "/api/work") return { schema: "kiln.work.v1", bounded: true, configured: true, can_write: true, principal: "local:op", objects: [{ ...issue1, head: recordHead }] };
      if (path.startsWith("/api/work/board")) return boardProjection;
      if (path === "/api/work/objects/issue-1") return { ...issue1, head: recordHead };
      return { objects: [], available: [], enrolled: [], runs: [], receipts: [] };
    },
    command: async (cmd) => {
      commands.push(cmd);
      if (cmd.action.type === "set_status" && failUnknown) {
        const err = new Error("network timeout");
        err.name = "TimeoutError";
        throw err;
      }
      if (cmd.action.type === "edit" && failConflict) {
        const err = new Error("409 conflict: head mismatch");
        err.status = 409;
        throw err;
      }
      const nextStatus = cmd.action.type === "set_status" ? cmd.action.status : cmd.action.fields?.status || issue1.fields.status;
      return { object: { ...issue1, head: "head-2", fields: { ...issue1.fields, ...(cmd.action.fields || {}), status: nextStatus } } };
    },
    changeEvidence: async (rec, opts) => window.KilnWorkEvidence.read(rec, opts),
  };
  window.KilnWork = bridge;
  vm.runInNewContext(asset("work-board.js"), context);
  vm.runInNewContext(asset("work-evidence.js"), context);
  vm.runInNewContext(asset("work.js"), context);
  vm.runInNewContext(asset("work-interaction.js"), context);

  const container = doc.createElement("div");
  container.id = "work-content";
  doc.body.append(container);

  await window.KilnWorkUI.render(container, currentRoute);
  await tick(20);

  // Finding 8: ci_project lane label in listHead & scrollLeft restoration
  assert.match(container.querySelector(".work-list-order").textContent, /CI project lanes/);
  const lanes = container.querySelector(".work-board-lanes");
  lanes.scrollLeft = 180;
  await window.KilnWorkUI.refresh();
  await tick(10);
  assert.equal(container.querySelector(".work-board-lanes").scrollLeft, 180, "lane scrollLeft preserved");

  // Finding 6: moveDialog unknown recovery disables Move card and resends identical request_id
  const card = container.querySelector(".work-board-card");
  card.querySelector(".work-board-move").click();
  await tick();
  const moveDialog = doc.querySelector("dialog.work-move-dialog");
  assert.ok(moveDialog);
  const startedRadio = moveDialog.querySelectorAll('input[type="radio"]').find((r) => r.value === "started");
  startedRadio.checked = true;
  moveDialog.querySelector("form").dispatchEvent({ type: "submit" });
  await tick(10);
  const commitBtn = moveDialog.querySelector('button[type="submit"]');
  assert.equal(commitBtn.disabled, true, "commit button disabled during unknown recovery");
  const firstReqId = commands[0].request_id;
  // Submit via form directly must still refuse minting a new UUID
  moveDialog.querySelector("form").dispatchEvent({ type: "submit" });
  await tick(10);
  assert.equal(commands.length, 1, "must not mint duplicate request while unknown");

  // Retry same request resends exact request_id
  failUnknown = false;
  failConflict = false;
  const retryBtn = moveDialog.querySelectorAll(".work-move-recovery button").find((b) => b.textContent === "Retry same request");
  retryBtn.click();
  await tick(20);
  assert.equal(commands[1].request_id, firstReqId, "retry reuses exact request_id");

  // Finding 9: discussion whitespace rejection, draft preservation across refresh, and editor 409 reload
  currentRoute = { page: "work", work_id: "issue-1" };
  await window.KilnWorkUI.render(container, currentRoute);
  await tick(20);
  const discForm = container.querySelector(".work-discussion-form");
  const discTa = discForm.querySelector("textarea");
  discTa.value = "   ";
  discForm.dispatchEvent({ type: "submit" });
  assert.match(discForm.querySelector(".work-dialog-status").textContent, /cannot be empty/);
  discTa.value = "Unsaved comment draft";
  discTa.dispatchEvent({ type: "input" });
  await window.KilnWorkUI.refresh();
  await tick(20);
  assert.equal(container.querySelector(".work-discussion-form textarea").value, "Unsaved comment draft");

  // Finding 10: work-interaction search passes AbortSignal and aborts previous search on new input
  doc.dispatchKey("k", { ctrlKey: true });
  const paletteInput = doc.querySelector("#work-command-query");
  assert.ok(paletteInput);
  await tick(20);
  const firstSignal = bridgeCalls.find((c) => c.opts?.signal)?.opts.signal;
  assert.ok(firstSignal, "search passes AbortSignal");
  paletteInput.value = "next";
  paletteInput.dispatchEvent({ type: "input" });
  assert.equal(firstSignal.aborted, true, "prior search aborted on new input");

  // Finding 15: work-evidence propagates AbortSignal
  const abortCtrl = new AbortController();
  abortCtrl.abort();
  await assert.rejects(() => window.KilnWorkEvidence.read({ fields: { links: [{ kind: "run", id: "r1" }] } }, { signal: abortCtrl.signal }), { name: "AbortError" });
});

// --- Findings 7, 14, 16: Source filter non-clobber, unavailable clear button, hub reset, commit stepping ---

test("Source: #src-filter avoids clobbering typing, unavailable source offers Clear link, commit stepping keeps spath/sfile, and hub resets filter state", async () => {
  const { context, doc, window, historyCalls } = createHarness({
    search: `?page=source&src=repo-1&rev=${COMMIT_1}&spath=crates&sfile=crates%2Flib.rs&sview=files`,
  });
  vm.runInNewContext(asset("source.js"), context);
  const src = window.KilnSource;

  const root = doc.createElement("div");
  doc.body.append(root);
  let inventoryReads = 0;
  let route = { page: "source", src: "repo-1", rev: COMMIT_1, spath: "crates", sfile: "crates/lib.rs", sview: "files" };
  const api = async (url) => {
    if (url === "/api/sources") {
      inventoryReads += 1;
      return {
        configured: true,
        sources: [
          { id: "repo-1", label: "kiln", kind: "git", status: "available", revision_scope: "revision", head: COMMIT_1 },
          { id: "repo-down", label: "down", kind: "git", status: "unavailable", reason: "disk offline" },
        ],
      };
    }
    if (url.includes("/log")) return { entries: [{ id: COMMIT_1, summary: "head", author: "a", recorded_at_ms: 1 }, { id: COMMIT_0, summary: "prev", author: "a", recorded_at_ms: 0 }] };
    if (url.includes("/tree")) return { kind: "git", revision_scope: "revision", revision: COMMIT_1, entries: [{ kind: "file", name: "lib.rs", path: "crates/lib.rs", bytes: 10 }, { kind: "file", name: "mod.rs", path: "crates/mod.rs", bytes: 20 }] };
    if (url.includes("/file")) return { path: "crates/lib.rs", text: "fn main() {}\n", binary: false };
    throw new Error(`Unexpected ${url}`);
  };

  await src.load(root, { route, api, navigate: (p) => { route = { ...route, ...p }; } });
  const beforeReads = inventoryReads;

  // Finding 16: Older step keeps spath/sfile
  const olderLink = root.querySelector('[data-src-focus="step:Older"]');
  assert.ok(olderLink);
  assert.match(olderLink.href, /spath=crates/);
  assert.match(olderLink.href, /sfile=crates%2Flib\.rs/);

  // Finding 7: typing in #src-filter filters locally & uses history.replaceState without re-reading /api/sources
  const filter = root.querySelector("#src-filter");
  filter.focus();
  filter.value = "mod";
  filter.dispatchEvent({ type: "input" });
  await tick(400);
  assert.equal(inventoryReads, beforeReads, "#src-filter debounce must not re-fetch /api/sources");
  assert.ok(historyCalls.some((u) => u.includes("sq=mod")), "history.replaceState synced sq");
  assert.equal(root.querySelector("#src-filter").value, "mod");

  // Finding 14: unavailable source renders Clear source selection link
  await src.load(root, { route: { page: "source", src: "repo-down" }, api, navigate: () => {} });
  assert.ok(root.querySelector('[data-src-focus="clear"]'), "unavailable source offers Clear source selection");

  // Finding 14: clearRepositories resets filter state & Add Mirror clears #repo-import-error
  const repoRoot = doc.createElement("div");
  doc.body.append(repoRoot);
  await src.loadRepositories(repoRoot, {
    canOperate: () => true,
    confirmControl: async () => ({ confirmed: false }),
    mutate: async () => null,
    navigate: () => {},
    api: async (url) => {
      if (url === "/api/repositories") return {
        repositories: [{ identity: HEX_A, label: "kiln", sync_status: "synced", total_cairns: 0, remotes: [], local_copies: [], admission: {} }],
        sources: {}, hosts: [],
      };
      return { sources: [] };
    },
  });
  const search = repoRoot.querySelector("#repo-filter-search");
  search.value = "kiln";
  search.dispatchEvent({ type: "input" });
  repoRoot.querySelector("#repo-import-form").dispatchEvent({ type: "submit" }); // invalid empty url -> sets #repo-import-error
  assert.equal(repoRoot.querySelector("#repo-import-error").hidden, false);
  repoRoot.querySelectorAll("button").find((b) => b.textContent.includes("Add Mirror")).click();
  assert.equal(repoRoot.querySelector("#repo-import-error").hidden, true, "Add Mirror clears #repo-import-error");

  src.clearRepositories();
  await src.loadRepositories(repoRoot, {
    canOperate: () => false, navigate: () => {},
    api: async (url) => url === "/api/repositories" ? { repositories: [], sources: {}, hosts: [] } : { sources: [] },
  });
  assert.equal(repoRoot.querySelector("#repo-filter-search").value, "", "repoFilterState reset after clearRepositories()");
});

// --- Findings 12, 13, 14, 16: Preview + CSS checks ---

test("Preview & CSS: no async focus steal, incompatible structural admitted without revisions, runtime abort guard, and CSS fixes", async () => {
  const review = await import(`data:text/javascript;base64,${Buffer.from(asset("preview-review.js")).toString("base64")}`);
  const { doc } = createHarness();
  globalThis.document = doc;
  try {
    const parent = doc.createElement("div");
    const otherButton = doc.createElement("button");
    doc.body.append(otherButton, parent);
    otherButton.focus();

    const obsA = HEX_A, obsB = HEX_B, compId = "c".repeat(64);
    const incompatResponse = {
      comparison: { identity: compId, rules: "r1", baseline: obsA, candidate: obsB, compatible: false, reasons: ["arguments_differ"], exit_status_equal: true, output_equal: true },
      baseline: { identity: obsA, current: false, observation: { binding: { revision: COMMIT_0 } } },
      candidate: { identity: obsB, current: true, observation: { binding: { revision: COMMIT_1 } } },
      structural: { identity: "", evidence: { comparison: compId, status: "incompatible", reasons: ["arguments_differ"] } },
    };
    assert.equal(review.acceptStructural(incompatResponse), true);

    review.renderComparison(parent, [incompatResponse.baseline, incompatResponse.candidate], {
      requested: `${obsA}~${obsB}`, structural: true,
      selected: () => `${obsA}~${obsB}`, select: () => {}, isCurrent: () => true,
      compare: async () => incompatResponse,
    });
    await tick(10);
    assert.equal(doc.activeElement, otherButton, "async comparison completion must not steal focus");
    assert.match(parent.textContent, /Structural evidence withheld/);
  } finally {
    delete globalThis.document;
  }

  const runtimeSrc = asset("preview-runtime.js");
  assert.match(runtimeSrc, /withAbort\(vm\.run/);

  const previewCss = asset("preview.css");
  assert.doesNotMatch(previewCss, /#terminal\s*\{[^}]*height:\s*90vh/);

  const sourceCss = asset("source.css");
  assert.match(sourceCss, /\.src-code,\s*\.src-diff\s*\{[^}]*border-collapse:\s*separate/);
  assert.match(sourceCss, /justify-content:\s*center/);
  assert.doesNotMatch(sourceCss, /\.src-ln a\s*\{\s*display:\s*none;\s*\}/);

  const workCss = asset("work.css");
  assert.match(workCss, /\.work-group\s*\{[^}]*overflow:\s*visible/);
  assert.match(workCss, /\.work-list-scroll\[data-layout="kanban"\]\s*\{[^}]*overflow:\s*visible/);

  const crCss = asset("code-review.css");
  assert.match(crCss, /\.cr-workbench\.is-rail-collapsed\.is-rail-drawer-open\s*>\s*\.cr-file-rail\s*\{[^}]*position:\s*fixed/);
  assert.match(crCss, /\.cr-line-number:nth-child\(1\)\s*\{\s*position:\s*sticky/);

  const cairnCss = asset("cairn.css");
  assert.match(cairnCss, /\.cairn-authoring\s*\{/);
  assert.match(cairnCss, /\.cairn-table\s*\{\s*table-layout:\s*auto;/);
});
