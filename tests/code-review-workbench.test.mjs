import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");
const SVG_NS = "http://www.w3.org/2000/svg";

// Lightweight DOM harness supporting queries, events, classList, dataset, hidden, and matchMedia.
function createDom({ wideViewport = true } = {}) {
  let narrow = !wideViewport;
  const mediaListeners = new Set();

  class FakeClassList {
    constructor(el) { this.el = el; }
    _list() { return (this.el.attributes.get("class") || "").split(/\s+/).filter(Boolean); }
    add(...names) { const set = new Set([...this._list(), ...names.filter(Boolean)]); this.el.attributes.set("class", [...set].join(" ")); }
    remove(...names) { const drop = new Set(names); this.el.attributes.set("class", this._list().filter(n => !drop.has(n)).join(" ")); }
    toggle(name, force) {
      const has = this.contains(name);
      const next = force === undefined ? !has : !!force;
      if (next) this.add(name); else this.remove(name);
      return next;
    }
    contains(name) { return this._list().includes(name); }
    *[Symbol.iterator]() { yield* this._list(); }
  }

  class FakeNode {
    constructor(nodeType) {
      this.nodeType = nodeType;
      this.parentNode = null;
      this.childNodes = [];
    }
    get isConnected() {
      let cur = this;
      while (cur) {
        if (cur === doc.body || cur === doc.documentElement) return true;
        cur = cur.parentNode;
      }
      return false;
    }
    get parentElement() { return this.parentNode instanceof FakeElement ? this.parentNode : null; }
    get children() { return this.childNodes.filter(c => c instanceof FakeElement); }
    get firstChild() { return this.childNodes[0] || null; }
    get lastChild() { return this.childNodes[this.childNodes.length - 1] || null; }
    get firstElementChild() { return this.children[0] || null; }
    remove() {
      if (!this.parentNode) return;
      const idx = this.parentNode.childNodes.indexOf(this);
      if (idx >= 0) this.parentNode.childNodes.splice(idx, 1);
      this.parentNode = null;
    }
  }

  class FakeText extends FakeNode {
    constructor(text) { super(3); this.nodeValue = String(text ?? ""); }
    get textContent() { return this.nodeValue; }
    set textContent(v) { this.nodeValue = String(v ?? ""); }
  }

  class FakeFragment extends FakeNode {
    constructor() { super(11); }
    append(...nodes) {
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        if (n instanceof FakeFragment) {
          this.append(...[...n.childNodes]);
          continue;
        }
        const child = n instanceof FakeNode ? n : new FakeText(String(n));
        child.remove();
        child.parentNode = this;
        this.childNodes.push(child);
      }
    }
    prepend(...nodes) {
      let ref = this.childNodes[0] || null;
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        if (n instanceof FakeFragment) {
          for (const f of [...n.childNodes]) this.insertBefore(f, ref);
          continue;
        }
        this.insertBefore(n, ref);
      }
    }
    after(...nodes) {
      if (!this.parentNode) return;
      const idx = this.parentNode.childNodes.indexOf(this);
      let ref = idx >= 0 ? this.parentNode.childNodes[idx + 1] || null : null;
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        this.parentNode.insertBefore(n, ref);
      }
    }
    get textContent() { return this.childNodes.map(c => c.textContent).join(""); }
  }

  class FakeElement extends FakeNode {
    constructor(tagName, namespaceURI = null) {
      super(1);
      this.tagName = String(tagName).toLowerCase();
      this.namespaceURI = namespaceURI;
      this.attributes = new Map();
      this.listeners = new Map();
      this.classList = new FakeClassList(this);
      this._dataset = {};
      this.value = "";
      this.checked = false;
      this.disabled = false;
      this.open = false;
      this.hidden = false;
      this.required = false;
      this.type = "";
      this.tabIndex = -1;
      this.rows = 0;
      this.colSpan = 1;
    }
    get id() { return this.attributes.get("id") || ""; }
    set id(v) { this.attributes.set("id", String(v)); }
    get className() { return this.attributes.get("class") || ""; }
    set className(v) { this.attributes.set("class", String(v)); }
    get href() { return this.attributes.get("href") || ""; }
    set href(v) { this.attributes.set("href", String(v)); }
    get title() { return this.attributes.get("title") || ""; }
    set title(v) { this.attributes.set("title", String(v)); }
    get dataset() {
      const self = this;
      return new Proxy(this._dataset, {
        get(target, prop) {
          if (typeof prop !== "string") return undefined;
          const attr = `data-${prop.replace(/[A-Z]/g, m => `-${m.toLowerCase()}`)}`;
          return self.attributes.has(attr) ? self.attributes.get(attr) : target[prop];
        },
        set(target, prop, val) {
          if (typeof prop !== "string") return false;
          const attr = `data-${prop.replace(/[A-Z]/g, m => `-${m.toLowerCase()}`)}`;
          self.attributes.set(attr, String(val));
          target[prop] = String(val);
          return true;
        },
      });
    }
    setAttribute(k, v) {
      this.attributes.set(String(k), String(v));
      if (k === "open") this.open = true;
      if (k === "hidden") this.hidden = true;
    }
    getAttribute(k) { return this.attributes.has(String(k)) ? this.attributes.get(String(k)) : null; }
    removeAttribute(k) { this.attributes.delete(String(k)); if (k === "open") this.open = false; if (k === "hidden") this.hidden = false; }
    hasAttribute(k) { return this.attributes.has(String(k)); }
    append(...nodes) {
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        if (n instanceof FakeFragment) {
          this.append(...[...n.childNodes]);
          continue;
        }
        const child = n instanceof FakeNode ? n : new FakeText(String(n));
        child.remove();
        child.parentNode = this;
        this.childNodes.push(child);
      }
    }
    prepend(...nodes) {
      let ref = this.childNodes[0] || null;
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        if (n instanceof FakeFragment) {
          for (const f of [...n.childNodes]) this.insertBefore(f, ref);
          continue;
        }
        this.insertBefore(n, ref);
      }
    }
    after(...nodes) {
      if (!this.parentNode) return;
      const idx = this.parentNode.childNodes.indexOf(this);
      let ref = idx >= 0 ? this.parentNode.childNodes[idx + 1] || null : null;
      for (const n of nodes) {
        if (n === null || n === undefined) continue;
        this.parentNode.insertBefore(n, ref);
      }
    }
    insertBefore(newNode, refNode) {
      if (!refNode) { this.append(newNode); return newNode; }
      const child = newNode instanceof FakeNode ? newNode : new FakeText(String(newNode));
      child.remove();
      child.parentNode = this;
      const idx = this.childNodes.indexOf(refNode);
      if (idx < 0) this.childNodes.push(child);
      else this.childNodes.splice(idx, 0, child);
      return child;
    }
    replaceChildren(...nodes) {
      for (const c of [...this.childNodes]) c.parentNode = null;
      this.childNodes = [];
      this.append(...nodes);
    }
    get textContent() { return this.childNodes.map(c => c.textContent).join(""); }
    set textContent(v) { this.replaceChildren(v === "" || v == null ? null : new FakeText(String(v))); }
    addEventListener(type, fn) {
      if (!this.listeners.has(type)) this.listeners.set(type, []);
      this.listeners.get(type).push(fn);
    }
    dispatchEvent(event) {
      if (!event.target) event.target = this;
      event.preventDefault = event.preventDefault || (() => { event.defaultPrevented = true; });
      for (const fn of this.listeners.get(event.type) || []) fn(event);
      return !event.defaultPrevented;
    }
    click() {
      const ev = { type: "click", button: 0, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, target: this, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
      this.dispatchEvent(ev);
    }
    focus() { doc.activeElement = this; }
    scrollIntoView() {}
    showModal() { this.open = true; this.setAttribute("open", ""); }
    close() { this.open = false; this.removeAttribute("open"); }
    closest(selector) {
      let cur = this;
      while (cur instanceof FakeElement) {
        if (matchesSimpleSelector(cur, selector)) return cur;
        cur = cur.parentElement;
      }
      return null;
    }
    querySelectorAll(selector) {
      const parts = selector.split(",").map(s => s.trim()).filter(Boolean);
      const out = [];
      const seen = new Set();
      const visit = (el) => {
        for (const c of el.children) {
          for (const part of parts) {
            if (matchesChain(c, part, this) && !seen.has(c)) {
              seen.add(c);
              out.push(c);
            }
          }
          visit(c);
        }
      };
      visit(this);
      return out;
    }
    querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  }

  function matchesSimpleSelector(el, sel) {
    if (!sel) return false;
    // Handle :not(...) minimally and split compound selectors.
    let rest = sel.trim();
    if (rest.includes(">")) return false;
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
      const attrName = m[1];
      const expected = m[2] ?? m[3] ?? m[4];
      const actual = attrName === "hidden" ? (el.hidden ? "" : null) : el.getAttribute(attrName);
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
    const tokens = chain.trim().split(/\s+/);
    let idx = tokens.length - 1;
    let cur = el;
    let directChild = false;
    while (idx >= 0) {
      const tok = tokens[idx];
      if (tok === ">") { directChild = true; idx--; continue; }
      if (idx === tokens.length - 1) {
        if (!matchesSimpleSelector(cur, tok)) return false;
        idx--;
        continue;
      }
      if (directChild) {
        cur = cur.parentElement;
        if (!cur || !matchesSimpleSelector(cur, tok)) return false;
        directChild = false;
        idx--;
      } else {
        let p = cur.parentElement;
        let matched = false;
        while (p && p !== root?.parentElement) {
          if (matchesSimpleSelector(p, tok)) { cur = p; matched = true; break; }
          p = p.parentElement;
        }
        if (!matched) return false;
        idx--;
      }
    }
    return true;
  }

  const doc = {
    body: new FakeElement("body"),
    documentElement: new FakeElement("html"),
    activeElement: null,
    listeners: new Map(),
    createElement: (tag) => new FakeElement(tag),
    createElementNS: (ns, tag) => new FakeElement(tag, ns),
    createTextNode: (t) => new FakeText(t),
    createDocumentFragment: () => new FakeFragment(),
    getElementById(id) { return this.body.querySelector(`#${id}`); },
    querySelector(sel) { return this.body.querySelector(sel); },
    querySelectorAll(sel) { return this.body.querySelectorAll(sel); },
    addEventListener(type, fn) {
      if (!this.listeners.has(type)) this.listeners.set(type, []);
      this.listeners.get(type).push(fn);
    },
    dispatchKey(key, extra = {}) {
      const ev = { key, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, target: doc.activeElement || doc.body, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, ...extra };
      for (const fn of this.listeners.get("keydown") || []) fn(ev);
      return ev;
    },
  };
  doc.documentElement.append(doc.body);
  const root = new FakeElement("div");
  root.id = "code-review-content";
  const actionSlot = new FakeElement("div");
  actionSlot.id = "page-primary-action";
  doc.body.append(actionSlot, root);

  const windowObj = {
    location: { search: "?page=code-review" },
    matchMedia(query) {
      return {
        media: query,
        get matches() { return query === "(min-width: 90rem)" ? !narrow : false; },
        addEventListener(ev, fn) { if (ev === "change") mediaListeners.add(fn); },
      };
    },
    setViewportWide(isWide) {
      narrow = !isWide;
      for (const fn of mediaListeners) fn({ matches: !narrow });
    },
  };

  const context = {
    window: windowObj,
    document: doc,
    Element: FakeElement,
    Node: FakeNode,
    URLSearchParams,
    crypto: { randomUUID: () => "00000000-0000-4000-8000-000000000001" },
    Event: class { constructor(type) { this.type = type; } },
  };
  vm.runInNewContext(asset("icons.js"), context);
  vm.runInNewContext(asset("ui.js"), context);
  vm.runInNewContext(asset("code-review.js"), context);
  return { dom: doc, root, window: windowObj, cr: context.window.KilnCodeReview };
}

const REV1_ID = "1".repeat(64);
const REV2_ID = "2".repeat(64);
const CHANGE_1 = "a".repeat(64);
const CHANGE_2 = "b".repeat(64);
const CHANGE_3 = "c".repeat(64);
const BASE_COMMIT = "0".repeat(40);
const COMMIT_1 = "1".repeat(40);
const COMMIT_2 = "2".repeat(40);
const TARGET_COMMIT = "9".repeat(40);
const CANDIDATE_1 = "d".repeat(40);
const CANDIDATE_2 = "e".repeat(40);
const CANDIDATE_3 = "f".repeat(40);

function baseSnapshot(overrides = {}) {
  return {
    schema: "kiln.code-review.v1",
    configured: true,
    principal: "local:alice",
    can_write: true,
    project: "kiln",
    repository: "repo-1",
    target_refs: ["refs/heads/main"],
    capabilities: ["inspect", "compare", "register", "speculative_integration"],
    policy: { id: "policy-1", eligible_reviewers: ["local:bob"], submitters: ["local:alice"], minimum_approvals: 1, require_ci: true, ownership: { teams: {}, rules: [] } },
    agent_principals: [],
    agent_metadata: {},
    changes: [
      {
        id: CHANGE_1, version: 1, project: "kiln", repository: "repo-1", target_ref: "refs/heads/main",
        owner: "local:bob", title: "Predecessor change A", state: "open", wip: false,
        authorship: "human", revisions: [{ id: REV1_ID, number: 1, commit: COMMIT_1, base: BASE_COMMIT, author: "local:bob", created_ms: 1700000000000 }],
        reviewers: { "local:alice": "reviewer" }, attention: [], unresolved: 0, advisory_current: 0,
        ci_summary: "fail",
        approval_summary: { approvals: 1, required: 1, vetoed: false },
        queue_position: 1, queue_status: "blocked",
        jj_stack: { jj_change_id: "knopqrstuvwxyzabcdefghijklmnoxyz", stack: "stk-1", position: 1, size: 3, removed: false },
      },
      {
        id: CHANGE_2, version: 2, project: "kiln", repository: "repo-1", target_ref: "refs/heads/main",
        owner: "local:alice", title: "Stacked change B", state: "open", wip: false,
        authorship: "human", revisions: [{ id: REV1_ID, number: 1, commit: COMMIT_1, base: BASE_COMMIT, author: "local:alice", created_ms: 1700000001000 }, { id: REV2_ID, number: 2, commit: COMMIT_2, base: BASE_COMMIT, author: "local:alice", created_ms: 1700000002000 }],
        reviewers: { "local:bob": "reviewer" }, attention: ["local:alice"], unresolved: 1, advisory_current: 0,
        ci_summary: "pass",
        approval_summary: { approvals: 0, required: 1, vetoed: false },
        queue_position: 1, queue_status: "waiting_ci",
        jj_stack: { jj_change_id: "wnloytsvxwxwxtpqyuwxutmotlmslszv", stack: "stk-1", position: 2, size: 3, removed: false },
      },
      {
        id: CHANGE_3, version: 1, project: "kiln", repository: "repo-1", target_ref: "refs/heads/main",
        owner: "local:alice", title: "Dropped stack change C", state: "open", wip: false,
        authorship: "human", revisions: [{ id: REV1_ID, number: 1, commit: COMMIT_1, base: BASE_COMMIT, author: "local:alice", created_ms: 1700000003000 }],
        reviewers: {}, attention: [], unresolved: 0, advisory_current: 0,
        ci_summary: "awaiting_approval",
        approval_summary: { approvals: 0, required: 1, vetoed: true },
        jj_stack: { jj_change_id: "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt", stack: "stk-1", position: null, size: 2, removed: true },
      },
    ],
    total: 3,
    next_offset: null,
    saved_views: [],
    notifications: [],
    notifications_total: 0,
    next_notification_offset: null,
    operations: [],
    operations_total: 0,
    next_operation_offset: null,
    contribution_sources: [],
    intake_enabled: true,
    jj_intake: { jj: true, change_id_header: true, max_stack: 8, target_ref: "refs/heads/main" },
    queue: [
      {
        id: "q-1", change: CHANGE_1, revision: REV1_ID, actor: "local:bob", target_ref: "refs/heads/main",
        source_commit: COMMIT_1, target_commit: TARGET_COMMIT, candidate_commit: CANDIDATE_1,
        ci_run: "run-q1", status: "blocked", detail: "review_queue_ci_failed",
        created_ms: 1700000010000, generation: 1, policy_id: "policy-1",
        chain: { id: "c1".repeat(32), root_target: TARGET_COMMIT, predecessors: [] },
        superseded_chains: [],
      },
      {
        id: "q-2", change: CHANGE_2, revision: REV2_ID, actor: "local:alice", target_ref: "refs/heads/main",
        source_commit: COMMIT_2, target_commit: TARGET_COMMIT, candidate_commit: CANDIDATE_2,
        ci_run: "run-q2", status: "waiting_ci", detail: "Rebuilt after predecessor ejection",
        created_ms: 1700000020000, generation: 2, policy_id: "policy-1",
        chain: { id: "c2".repeat(32), root_target: TARGET_COMMIT, predecessors: [] },
        superseded_chains: ["99".repeat(32)],
      },
      {
        id: "q-3", change: CHANGE_3, revision: REV1_ID, actor: "local:alice", target_ref: "refs/heads/main",
        source_commit: COMMIT_1, target_commit: CANDIDATE_2, candidate_commit: CANDIDATE_3,
        ci_run: "run-q3", status: "queued", detail: "Stacked on #1",
        created_ms: 1700000030000, generation: 2, policy_id: "policy-1",
        chain: { id: "c3".repeat(32), root_target: TARGET_COMMIT, predecessors: [{ queue_id: "q-2", generation: 2, candidate_commit: CANDIDATE_2 }] },
        superseded_chains: ["88".repeat(32)],
      },
    ],
    queue_enabled: true,
    queue_windows: [
      { target_ref: "refs/heads/main", mode: "speculative", depth: 2, bounds: { floor: 1, ceiling: 4, increase: 1, decrease: 1 } },
    ],
    ...overrides,
  };
}

function baseDetail(overrides = {}) {
  return {
    change: {
      id: CHANGE_2, version: 2, project: "kiln", repository: "repo-1", target_ref: "refs/heads/main",
      owner: "local:alice", title: "Stacked change B", description: "Update `lib.rs` safely.", state: "open", wip: false,
      authorship: "human",
      revisions: [
        { id: REV1_ID, number: 1, commit: COMMIT_1, base: BASE_COMMIT, author: "local:alice", created_ms: 1700000001000, files: ["src/lib.rs", "Cargo.lock"] },
        { id: REV2_ID, number: 2, commit: COMMIT_2, base: BASE_COMMIT, author: "local:alice", created_ms: 1700000002000, files: ["src/lib.rs", "Cargo.lock"] },
      ],
      reviewers: { "local:bob": "reviewer" },
      agent_reviewers: [],
      votes: [{ actor: "local:bob", revision: REV1_ID, value: "approve", created_ms: 1700000001500, agent: false }],
      threads: [
        {
          id: "thread-inline-open",
          resolved: false,
          blocking: true,
          agent: false,
          anchor: { revision: REV2_ID, base_commit: BASE_COMMIT, file: "src/lib.rs", side: "revision", start_line: 12, end_line: 12 },
          comments: [{ id: "c-1", author: "local:bob", created_ms: 1700000002500, body: "Inline review note on line 12" }],
        },
        {
          id: "thread-inline-resolved",
          resolved: true,
          blocking: false,
          agent: false,
          anchor: { revision: REV2_ID, base_commit: BASE_COMMIT, file: "src/lib.rs", side: "base", start_line: 10, end_line: 10 },
          comments: [{ id: "c-2", author: "local:bob", created_ms: 1700000002400, body: "Resolved note on base 10" }],
        },
        {
          id: "thread-other-file",
          resolved: false,
          blocking: false,
          agent: false,
          anchor: { revision: REV2_ID, base_commit: BASE_COMMIT, file: "README.md", side: "revision", start_line: 3, end_line: 3 },
          comments: [{ id: "c-3", author: "local:bob", created_ms: 1700000002600, body: "Other file thread" }],
        },
        {
          id: "thread-general",
          resolved: false,
          blocking: false,
          agent: false,
          anchor: { revision: REV2_ID, base_commit: BASE_COMMIT, file: "", side: "revision", start_line: 1, end_line: 1 },
          comments: [{ id: "c-4", author: "local:alice", created_ms: 1700000002700, body: "General change comment" }],
        },
      ],
      advisory: [],
      dependencies: [{ change: CHANGE_1, revision: REV1_ID }],
      candidate: { id: "cand-1", revision: REV2_ID, commit: CANDIDATE_2, target_ref: "refs/heads/main", target: "7".repeat(40), ci_run: "run-1", dependencies: [] },
      submission: null,
    },
    checklist: {
      ready: false,
      items: [
        { key: "open", satisfied: true, detail: "Change is open" },
        { key: "ci", satisfied: true, detail: "Required CI passed" },
        { key: "approvals", satisfied: false, detail: "Needs 1 approval on current revision" },
      ],
    },
    source: { current_target: TARGET_COMMIT },
    stack: {
      dependencies: [{ id: CHANGE_1, title: "Predecessor change A", state: "open" }],
      dependents: [{ id: CHANGE_3, title: "Dropped stack change C", state: "open" }],
    },
    operations: [
      { operation_id: "op-uncertain", kind: "submit", state: "uncertain", commit: COMMIT_2, target_ref: "refs/heads/main", target_commit: TARGET_COMMIT },
    ],
    ci_runs: [
      { project: "kiln", repository: "repo-1", revision: COMMIT_2, run_id: "run-ci-2", success: true, full_run: true, clean: true, ordinal: 5, checks: [{ name: "test", kind: "unit", outcome: "pass", artifact_digest: "b".repeat(64) }] },
    ],
    drafts: [],
    subscribed: true,
    delivery_outcomes: [],
    queue: null,
    queue_enabled: true,
    jj_stack: {
      jj_change_id: "wnloytsvxwxwxtpqyuwxutmotlmslszv",
      stack: "stk-1",
      position: 2,
      size: 3,
      removed: false,
      members: [
        { jj_change_id: "knopqrstuvwxyzabcdefghijklmnoxyz", change: CHANGE_1 },
        { jj_change_id: "wnloytsvxwxwxtpqyuwxutmotlmslszv", change: CHANGE_2 },
        { jj_change_id: "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt", change: CHANGE_3 },
      ],
    },
    jj_evolog: [
      { revision: REV1_ID, number: 1, commit: COMMIT_1, created_ms: 1700000001000, submitted_by: "local:alice", authorship: { kind: "human" }, predecessor_matches_previous: null, interdiff: null },
      { revision: REV2_ID, number: 2, commit: COMMIT_2, created_ms: 1700000002000, submitted_by: "local:alice", authorship: { kind: "human" }, predecessor_matches_previous: true, interdiff: `${REV1_ID}..${REV2_ID}` },
    ],
    ...overrides,
  };
}

function baseDiff(overrides = {}) {
  return {
    old_commit: BASE_COMMIT,
    new_commit: COMMIT_2,
    truncated: false,
    highlight: { schema: "kiln.highlight.v1", status: "skipped", reason: "unsupported_language", lines: [] },
    noise: {
      identity: "noise-id",
      files: { "Cargo.lock": "lockfile" },
      rules: [{ id: "lockfile", reason: "Generated lockfile" }],
    },
    review_hunks: {
      exact_base: true,
      last_viewed: { number: 1 },
      files: [
        { path: "src/lib.rs", total: 1, viewed: 0, new_since_viewed: 1, hunks: [{ digest: "hunk-1", viewed: false, new_since_viewed: true }] },
        { path: "Cargo.lock", total: 1, viewed: 0, new_since_viewed: 0, hunks: [{ digest: "hunk-2", viewed: false, new_since_viewed: false }] },
      ],
    },
    files: [
      {
        old_path: "src/lib.rs", new_path: "src/lib.rs", binary: false, truncated: false,
        hunks: [{
          header: "@@ -10,2 +10,3 @@",
          old_start: 10, old_lines: 2, new_start: 10, new_lines: 3,
          lines: [
            { kind: "context", old_line: 10, new_line: 10, text: "fn keep() {}" },
            { kind: "deletion", old_line: 11, new_line: null, text: "fn old() {}" },
            { kind: "addition", old_line: null, new_line: 11, text: "fn next_a() {}" },
            { kind: "addition", old_line: null, new_line: 12, text: "fn next_b() {}" },
          ],
        }],
      },
      {
        old_path: "Cargo.lock", new_path: "Cargo.lock", binary: false, truncated: false,
        hunks: [{ header: "@@ -1,1 +1,1 @@", old_start: 1, old_lines: 1, new_start: 1, new_lines: 1, lines: [{ kind: "addition", old_line: null, new_line: 1, text: "lock = 2" }] }],
      },
    ],
    ...overrides,
  };
}

// r[verify kiln.review.workbench_three_pane]
// r[verify kiln.review.workbench_three_pane.scenario.adaptive_panes]
test("3-pane workbench opens all 3 panes at >=90rem unified and auto-collapses left rail in split mode or <90rem", async () => {
  const { root, window, cr, dom } = createDom({ wideViewport: true });
  const snapshot = baseSnapshot();
  const detail = baseDetail();
  const diff = baseDiff();

  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review", review_change: CHANGE_2 },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return snapshot;
      if (url === `/api/code-review/changes/${CHANGE_2}`) return detail;
      if (url.startsWith(`/api/code-review/changes/${CHANGE_2}/diff`)) return diff;
      throw new Error(`Unexpected API call: ${url}`);
    },
  });

  const workbench = root.querySelector(".cr-workbench");
  const rail = root.querySelector("#cr-file-rail");
  const toggle = root.querySelector("#cr-file-rail-toggle");
  assert.ok(workbench, "3-pane workbench exists");
  assert.ok(rail, "left file rail exists");
  assert.ok(toggle, "file rail toggle exists");
  assert.equal(workbench.classList.contains("is-rail-collapsed"), false);
  assert.equal(rail.hidden, false);
  assert.equal(toggle.getAttribute("aria-expanded"), "true");

  // File rail shows per-file unresolved thread count, hunk progress, and noise fold.
  assert.match(rail.textContent, /1 open/);
  assert.match(rail.textContent, /0\/1/);
  assert.ok(rail.querySelector(".cr-noise"), "noise group rendered in file rail");

  const tick = () => new Promise(r => setTimeout(r, 0));

  // Switch to split mode: left file rail MUST auto-collapse into toggleable drawer.
  const splitBtn = [...root.querySelectorAll(".cr-diff-toolbar .segmented button")].find(b => b.textContent === "Side by side");
  assert.ok(splitBtn);
  splitBtn.click();
  await tick();
  assert.equal(workbench.classList.contains("is-rail-collapsed"), true);
  assert.equal(rail.hidden, true);
  assert.equal(toggle.getAttribute("aria-expanded"), "false");
  assert.ok(root.querySelector("table.cr-diff.cr-split"), "side-by-side table rendered");

  // Toggle drawer open and closed manually in split mode.
  toggle.click();
  await tick();
  assert.equal(workbench.classList.contains("is-rail-drawer-open"), true);
  assert.equal(rail.hidden, false);
  assert.equal(toggle.getAttribute("aria-expanded"), "true");
  toggle.click();
  await tick();
  assert.equal(workbench.classList.contains("is-rail-drawer-open"), false);
  assert.equal(rail.hidden, true);

  // Switch back to unified via keyboard shortcut 'm' -> re-opens all 3 panes at >=90rem.
  dom.dispatchKey("m");
  await tick();
  assert.equal(workbench.classList.contains("is-rail-collapsed"), false);
  assert.equal(rail.hidden, false);

  // Viewport shrinking below 90rem auto-collapses left rail even in unified mode.
  window.setViewportWide(false);
  assert.equal(workbench.classList.contains("is-rail-collapsed"), true);
  assert.equal(rail.hidden, true);
  window.setViewportWide(true);
  assert.equal(workbench.classList.contains("is-rail-collapsed"), false);
  assert.equal(rail.hidden, false);
});

// r[verify kiln.review.workbench_three_pane]
test("anchored threads render inline directly beneath target diff lines while keeping hunks, new_since_viewed, and other-file discussion", async () => {
  const { root, cr } = createDom({ wideViewport: true });
  const snapshot = baseSnapshot();
  const detail = baseDetail();
  const diff = baseDiff();

  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review", review_change: CHANGE_2 },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return snapshot;
      if (url === `/api/code-review/changes/${CHANGE_2}`) return detail;
      if (url.startsWith(`/api/code-review/changes/${CHANGE_2}/diff`)) return diff;
      throw new Error(`Unexpected API call: ${url}`);
    },
  });

  const tbody = root.querySelector("table.cr-diff tbody");
  assert.ok(tbody);
  const inlineRows = tbody.querySelectorAll("tr.cr-inline-thread-row");
  assert.equal(inlineRows.length, 2, "two anchored threads on src/lib.rs render inline in tbody");

  // Unresolved thread on line 12 is placed directly after line 12 row and expanded by default.
  const openRow = [...inlineRows].find(r => r.dataset.threadId === "thread-inline-open");
  assert.ok(openRow);
  const prevOpen = tbody.children[tbody.children.indexOf(openRow) - 1];
  assert.match(prevOpen.textContent, /fn next_b\(\) \{\}/);
  assert.equal(openRow.querySelector("details.cr-inline-thread").open, true);
  assert.match(openRow.textContent, /Inline review note on line 12/);

  // Resolved thread on base 10 is collapsed by default.
  const resolvedRow = [...inlineRows].find(r => r.dataset.threadId === "thread-inline-resolved");
  assert.ok(resolvedRow);
  assert.equal(resolvedRow.querySelector("details.cr-inline-thread").open, false);

  // Hunk Viewed marker and new_since_viewed chip are preserved.
  const hunkMark = root.querySelector(".cr-hunk-mark");
  assert.ok(hunkMark);
  assert.ok(root.querySelector(".cr-new-hunk"), "new_since_viewed chip preserved");

  // Discussion below keeps other-file and general/unanchored threads without duplicating src/lib.rs inline threads.
  const discussion = root.querySelector(".cr-discussion");
  assert.match(discussion.textContent, /2 anchored threads displayed inline in the diff table above/);
  assert.match(discussion.textContent, /Other file thread/);
  assert.match(discussion.textContent, /General change discussion/);
  assert.match(discussion.textContent, /General change comment/);
  assert.doesNotMatch(discussion.textContent, /Inline review note on line 12/);
});

// r[verify kiln.review.workbench_three_pane]
test("right sticky aside consolidates into 4 focused tabs with target-drift warning and 's' shortcut switching", async () => {
  const { root, cr, dom } = createDom({ wideViewport: true });
  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review", review_change: CHANGE_2 },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return baseSnapshot();
      if (url === `/api/code-review/changes/${CHANGE_2}`) return baseDetail();
      if (url.startsWith(`/api/code-review/changes/${CHANGE_2}/diff`)) return baseDiff();
      throw new Error(`Unexpected API call: ${url}`);
    },
  });

  const tabs = root.querySelectorAll('.cr-aside-tabs > [role="tab"]');
  const panels = root.querySelectorAll('.cr-aside-panel[role="tabpanel"]');
  assert.deepEqual(tabs.map(t => t.dataset.crTab), ["checklist", "ci", "queue", "history"]);
  assert.equal(panels.length, 4);
  assert.equal(panels.find(p => p.dataset.crTab === "checklist").hidden, false);
  assert.equal(panels.find(p => p.dataset.crTab === "ci").hidden, true);

  const tick = () => new Promise(r => setTimeout(r, 0));

  // Switch to CI & Previews -> verifies binding & target drift strip.
  tabs.find(t => t.dataset.crTab === "ci").click();
  await tick();
  const ciPanel = panels.find(p => p.dataset.crTab === "ci");
  assert.equal(ciPanel.hidden, false);
  assert.equal(panels.find(p => p.dataset.crTab === "checklist").hidden, true);
  assert.match(ciPanel.textContent, /Exact revision binding & target drift/);
  assert.match(ciPanel.textContent, /Target drifted since candidate/);

  // Switch to History & Ops -> verifies uncertain operation reconciliation state.
  tabs.find(t => t.dataset.crTab === "history").click();
  await tick();
  const histPanel = panels.find(p => p.dataset.crTab === "history");
  assert.equal(histPanel.hidden, false);
  assert.match(histPanel.textContent, /This operation reserves the affected change until the service reconciles/);

  // Switch to Queue & Stack -> verifies Merge Train slot, evolog, and stack links.
  tabs.find(t => t.dataset.crTab === "queue").click();
  await tick();
  const queuePanel = panels.find(p => p.dataset.crTab === "queue");
  assert.equal(queuePanel.hidden, false);
  assert.match(queuePanel.textContent, /Merge train slot/);
  assert.match(queuePanel.textContent, /Speculative window 2\/4 \(1\.\.4\)/);
  assert.match(queuePanel.textContent, /Open Merge Queue view/);

  // Shortcut 's' jumps back to Checklist & Votes tab and focuses #cr-requirements.
  const ev = dom.dispatchKey("s");
  await tick();
  assert.equal(ev.defaultPrevented, true);
  assert.equal(panels.find(p => p.dataset.crTab === "checklist").hidden, false);
  assert.equal(panels.find(p => p.dataset.crTab === "queue").hidden, true);
});

// r[verify kiln.review.workbench_three_pane]
test("drawList renders ci_summary, approval_summary, queue position, and jj_stack badges with zero N+1 detail fetches", async () => {
  const { root, cr } = createDom({ wideViewport: true });
  const calls = [];
  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review" },
    navigate: () => {},
    api: async (url) => {
      calls.push(url);
      if (url.startsWith("/api/code-review?")) return baseSnapshot();
      throw new Error(`Unexpected N+1 or detail fetch: ${url}`);
    },
  });

  assert.equal(calls.length, 1, "only the list snapshot endpoint is fetched");
  const rows = root.querySelectorAll(".cr-change-table tbody tr");
  assert.equal(rows.length, 3);
  assert.match(rows[0].textContent, /CI failed/);
  assert.match(rows[0].textContent, /1\/1 approvals/);
  assert.match(rows[0].textContent, /Queue #1 · Blocked/);
  assert.match(rows[1].textContent, /CI passed/);
  assert.match(rows[1].textContent, /0\/1 approvals/);
  assert.match(rows[1].textContent, /Queue #1 · Waiting ci/);
  assert.match(rows[1].textContent, /jj stack 2\/3/);
  assert.match(rows[2].textContent, /Awaiting approval/);
  assert.match(rows[2].textContent, /vetoed/);
  assert.match(rows[2].textContent, /Removed from jj stack/);

  // Negative/fallback check: client-side join on snapshot.queue & unknown/none ci_summary values.
  const fallbackSnap = baseSnapshot({
    changes: [
      { ...baseSnapshot().changes[0], ci_summary: "unknown", queue_position: null, queue_status: null },
      { ...baseSnapshot().changes[1], ci_summary: "none", queue_position: null, queue_status: null },
      { ...baseSnapshot().changes[2], ci_summary: "future_ci_state", queue_position: null, queue_status: null },
    ],
  });
  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review" },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return fallbackSnap;
      throw new Error(`Unexpected detail fetch: ${url}`);
    },
  });
  const fallbackRows = root.querySelectorAll(".cr-change-table tbody tr");
  assert.match(fallbackRows[0].textContent, /CI unknown/);
  assert.match(fallbackRows[0].textContent, /Queue · Blocked/);
  assert.match(fallbackRows[1].textContent, /No CI/);
  assert.match(fallbackRows[1].textContent, /Queue #1 · Waiting ci/);
  assert.match(fallbackRows[2].textContent, /Queue #2 · Queued/);
  assert.equal(fallbackRows[2].querySelector('[data-status="unknown"]') !== null, true);
});

// r[verify kiln.review.workbench_three_pane]
test("jj Stack Rail renders stack progression, 1-click interdiff, and explicit jj_stack.removed warning", async () => {
  const { root, cr } = createDom({ wideViewport: true });
  const removedDetail = baseDetail({
    jj_stack: {
      jj_change_id: "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt",
      stack: "stk-1",
      position: null,
      size: 2,
      removed: true,
      members: [
        { jj_change_id: "knopqrstuvwxyzabcdefghijklmnoxyz", change: CHANGE_1 },
        { jj_change_id: "wnloytsvxwxwxtpqyuwxutmotlmslszv", change: CHANGE_2 },
      ],
    },
  });

  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review", review_change: CHANGE_2 },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return baseSnapshot();
      if (url === `/api/code-review/changes/${CHANGE_2}`) return removedDetail;
      if (url.startsWith(`/api/code-review/changes/${CHANGE_2}/diff`)) return baseDiff();
      throw new Error(`Unexpected API call: ${url}`);
    },
  });

  const rail = root.querySelector(".cr-stack-rail");
  assert.ok(rail, "jj Stack Rail rendered above diff");
  assert.match(rail.textContent, /Removed from stack/);
  assert.ok(rail.querySelector(".cr-stack-removed"), "explicit jj_stack.removed warning rendered");
  const interdiffLink = [...rail.querySelectorAll("a")].find(a => a.textContent.includes("Interdiff vs previous"));
  assert.ok(interdiffLink);
  assert.match(interdiffLink.href, new RegExp(`review_interdiff=${REV1_ID}\\.\\.${REV2_ID}`));
});

// r[verify kiln.review.queue.speculative.visual_train]
// r[verify kiln.review.queue.speculative.visual_train.scenario.retired_chain]
test("Merge Queue mode renders speculative predecessor chain, window bounds, retired superseded_chains causes, and omits bounds in serial mode", async () => {
  const { root, cr } = createDom({ wideViewport: true });
  const snapshot = baseSnapshot({
    queue_windows: [
      { target_ref: "refs/heads/main", mode: "speculative", depth: 2, bounds: { floor: 1, ceiling: 4, increase: 1, decrease: 1 } },
      { target_ref: "refs/heads/release", mode: "serial" },
    ],
    queue: [
      ...baseSnapshot().queue,
      {
        id: "q-serial", change: CHANGE_1, revision: REV1_ID, actor: "local:bob", target_ref: "refs/heads/release",
        source_commit: COMMIT_1, target_commit: TARGET_COMMIT, candidate_commit: CANDIDATE_1,
        ci_run: "run-serial", status: "waiting_ci", detail: "Serial build",
        created_ms: 1700000040000, generation: 1, policy_id: "policy-1",
      },
    ],
  });

  await cr.load(root, {
    principal: "local:alice",
    route: { page: "code-review", review_tab: "queue" },
    navigate: () => {},
    api: async (url) => {
      if (url.startsWith("/api/code-review?")) return snapshot;
      throw new Error(`Unexpected API call: ${url}`);
    },
  });

  const modeTabs = root.querySelectorAll('.cr-mode-tabs > [role="tab"]');
  assert.equal(modeTabs.find(t => t.dataset.crMode === "queue").getAttribute("aria-selected"), "true");

  const mainCard = root.querySelector('[data-target-ref="refs/heads/main"]');
  assert.ok(mainCard);
  assert.match(mainCard.textContent, /Speculative/);
  assert.match(mainCard.textContent, /Window 2\/4 \(1\.\.4\)/);
  assert.match(mainCard.textContent, /Stacked on #1 Stacked change B \(gen 2\)/);
  assert.match(mainCard.textContent, /Retired superseded_chains \(1\)/);
  assert.match(mainCard.textContent, /Predecessor ejection: Predecessor change A: review_queue_ci_failed/);

  // Negative check: serial target MUST NOT fabricate or show speculative window bounds.
  const releaseCard = root.querySelector('[data-target-ref="refs/heads/release"]');
  assert.ok(releaseCard);
  assert.match(releaseCard.textContent, /Serial/);
  assert.match(releaseCard.textContent, /Head-only \(serial\)/);
  assert.doesNotMatch(releaseCard.textContent, /Window \d/);

  // Helper export for 1.1 Factory Cockpit.
  const plain = (v) => JSON.parse(JSON.stringify(v));
  const summaries = plain(cr.summarizeMergeTrains(snapshot));
  assert.equal(summaries.length, 2);
  assert.equal(summaries[0].target_ref, "refs/heads/main");
  assert.equal(summaries[0].mode, "speculative");
  assert.equal(summaries[0].depth, 2);
  assert.deepEqual(summaries[0].bounds, { floor: 1, ceiling: 4, increase: 1, decrease: 1 });
  assert.equal(summaries[0].href, "?page=code-review&review_tab=queue");
  assert.equal(summaries[1].target_ref, "refs/heads/release");
  assert.equal(summaries[1].mode, "serial");
  assert.equal(summaries[1].depth, null);
  assert.equal(summaries[1].bounds, null);
  assert.deepEqual(plain(cr.summarizeMergeTrains(null)), []);
});

test("code-review.css enforces 110rem frame, 90rem 3-pane grid, and bounded diff scroll without overflow", () => {
  const css = asset("code-review.css");
  assert.match(css, /max-width:\s*110rem/);
  assert.match(css, /@media \(min-width:\s*90rem\)/);
  assert.match(css, /grid-template-columns:\s*16rem minmax\(0,\s*1fr\) 23rem/);
  assert.match(css, /\.cr-diff-scroll\s*\{[^}]*max-width:\s*100%[^}]*overflow:\s*auto/);
});
