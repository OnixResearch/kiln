import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");

class FakeNode {
  constructor(nodeType, text = "") {
    this.nodeType = nodeType;
    this._text = String(text);
    this.children = [];
    this.parentNode = null;
    this.isConnected = true;
  }
  get textContent() {
    return this._text;
  }
  set textContent(value) {
    this._text = value == null ? "" : String(value);
  }
}

class FakeClassList {
  constructor(owner) {
    this.owner = owner;
  }
  _tokens() {
    return String(this.owner.attributes.get("class") || "").split(/\s+/).filter(Boolean);
  }
  add(...names) {
    const next = new Set(this._tokens());
    for (const n of names) if (n) next.add(n);
    this.owner.attributes.set("class", [...next].join(" "));
  }
  contains(name) {
    return this._tokens().includes(name);
  }
}

class FakeElement extends FakeNode {
  constructor(tagName, namespaceURI = "http://www.w3.org/1999/xhtml", doc = null) {
    super(1, "");
    this.tagName = String(tagName).toLowerCase();
    this.localName = this.tagName;
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
  get firstChild() {
    return this.children[0] || null;
  }
  get lastChild() {
    return this.children.at(-1) || null;
  }
  get textContent() {
    if (!this.children.length) return this._text;
    return this.children.map((c) => c.textContent).join("");
  }
  set textContent(value) {
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
  removeAttribute(key) {
    this.attributes.delete(String(key));
  }
  append(...nodes) {
    for (const item of nodes) {
      if (item === null || item === undefined) continue;
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      if (node.parentNode && node.parentNode !== this) {
        node.parentNode.children = node.parentNode.children.filter((c) => c !== node);
      }
      node.parentNode = this;
      this.children.push(node);
    }
  }
  prepend(...nodes) {
    for (let i = nodes.length - 1; i >= 0; i--) {
      const item = nodes[i];
      if (item === null || item === undefined) continue;
      const node = item instanceof FakeNode ? item : new FakeNode(3, String(item));
      node.parentNode = this;
      this.children.unshift(node);
    }
  }
  replaceChildren(...nodes) {
    this.children = [];
    this._text = "";
    this.append(...nodes);
  }
  replaceWith(...nodes) {
    if (!this.parentNode) return;
    const idx = this.parentNode.children.indexOf(this);
    if (idx === -1) return;
    const converted = nodes.map((n) => (n instanceof FakeNode ? n : new FakeNode(3, String(n))));
    for (const n of converted) n.parentNode = this.parentNode;
    this.parentNode.children.splice(idx, 1, ...converted);
  }
  remove() {
    if (!this.parentNode) return;
    this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
    this.parentNode = null;
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
    this.dispatchEvent({ type: "click", button: 0, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, preventDefault() {} });
  }
  focus() {
    if (this.ownerDocument) this.ownerDocument.activeElement = this;
  }
  scrollIntoView() {}
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }
  querySelectorAll(selector) {
    const out = [];
    const match = (el) => {
      if (selector.startsWith(".")) return el.classList.contains(selector.slice(1));
      if (selector.startsWith("#")) return el.id === selector.slice(1);
      return el.tagName === selector.toLowerCase();
    };
    const walk = (node) => {
      for (const child of node.children || []) {
        if (child.nodeType === 1) {
          if (match(child)) out.push(child);
          walk(child);
        }
      }
    };
    walk(this);
    return out;
  }
}

function createDomEnv(scripts) {
  const doc = {
    activeElement: null,
    title: "",
    createElement(tag) {
      return new FakeElement(tag, "http://www.w3.org/1999/xhtml", doc);
    },
    createElementNS(ns, tag) {
      return new FakeElement(tag, ns, doc);
    },
    createTextNode(text) {
      return new FakeNode(3, text);
    },
    getElementById() {
      return null;
    },
  };
  const ctx = {
    window: { addEventListener() {}, removeEventListener() {} },
    document: doc,
    Node: FakeNode,
    location: { search: "" },
    URLSearchParams,
    setTimeout,
    clearTimeout,
    crypto: webcrypto,
    Intl,
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
    prompt() { throw new Error("prompt() must never be called"); },
    alert() { throw new Error("alert() must never be called"); },
  };
  for (const name of scripts) {
    vm.runInNewContext(asset(name), ctx);
  }
  return { doc, window: ctx.window };
}

const allText = (node) => !node ? "" : !node.children?.length ? (node.textContent || "") : node.children.map(allText).join(" ");
const descendants = (node) => (node.children || []).flatMap((c) => (c.nodeType === 1 ? [c, ...descendants(c)] : []));
const tick = () => new Promise((r) => setTimeout(r, 15));

const HEX_A = "a".repeat(64);
const HEX_B = "b".repeat(64);
const HEX_C = "c".repeat(64);
const HEX_D = "d".repeat(64);
const HEX_P = "f".repeat(64);

// 1. Fair-Admission & AttemptMetrics (r[kiln.admission.live_console_governance])
test("Fair-Admission displays transfers separately from consumption and renders missing AttemptMetrics as 'not recorded', never 0", async () => {
  const env = createDomEnv(["fleet-capacity.js"]);
  const fc = env.window.KilnFleetCapacity;
  const root = env.doc.createElement("div");

  const admissionPayload = {
    schema: "kiln.implementation-admission.v1",
    configured: true,
    policy: {
      identity: HEX_P,
      version: 2,
      accounting_basis: "kiln.tokens.v1",
      strategy: "dominant_share",
      strategy_weights: { tokens: 1 },
      bounds: { max_items: 64, max_bytes: 65536, max_wait_ms: 60000, max_observation_age_ms: 30000, max_retry_after_ms: 10000, max_children: 8, max_active: 4 },
      shares: { "tenant-alpha": 60, "tenant-beta": 40 },
      group_limits: {},
      serving: null,
    },
    accounts: [
      {
        identity: HEX_A, parent: null, parent_attempt: null, root: HEX_A, depth: 0,
        tenant: "tenant-alpha", campaign: "camp-a", deadline_ms: 1_900_000_000_000, revoked: false,
        allocated: { tokens: 1000 }, transferred_out: { tokens: 350 }, available: { tokens: 500 },
        held: { tokens: 100 }, tentative: { tokens: 0 }, unknown: { tokens: 0 },
        consumed: { tokens: 50 }, settled_measured: { tokens: 50 }, settled_unknown: {},
        children: [HEX_B],
      },
      {
        identity: HEX_B, parent: HEX_A, parent_attempt: HEX_C, root: HEX_A, depth: 1,
        tenant: "tenant-beta", campaign: "camp-b", deadline_ms: 1_850_000_000_000, revoked: false,
        allocated: { tokens: 350 }, transferred_out: {}, available: { tokens: 250 },
        held: { tokens: 25 }, tentative: {}, unknown: {},
        consumed: { tokens: 75 }, settled_measured: { tokens: 75 }, settled_unknown: {},
        children: [],
      },
    ],
    serving: [
      {
        provider: "animus", scope: "qwen3-coder", bounds: { scope: "qwen3-coder", max_ongoing: 8, max_queued: 16 },
        readiness: "ready", origin: "host-snapshot", remaining_slots: null, queued: null,
        observed_at_ms: 1_700_000_000_000, valid_until_ms: 1_700_000_030_000, source_generation: 3, sequence: 9, deployment_generation: 1,
      },
    ],
    queue_items: [],
    attempt_metrics: [
      {
        attempt_id: HEX_C, fence: 2, work_id: HEX_D, work_revision: 1, phase: "executing", open: true, recorded: true,
        telemetry_source: "metrics_receipt", claimed_at_ms: 1_700_000_000_000, observed_at_ms: 1_700_000_005_000, ended_at_ms: null,
        wall_time_ms: 5000, tokens: 4200, tokens_status: "observed", tokens_absence_reason: null,
        model_turns: 4, model_turns_status: "observed", model_turns_absence_reason: null,
        auxiliary_calls: 2, auxiliary_calls_status: "observed", auxiliary_calls_absence_reason: null,
        harness: "omp", model: "gemini-4", worker_terminal: null, verification_outcome: null,
      },
      {
        attempt_id: HEX_D, fence: 1, work_id: HEX_A, work_revision: 1, phase: "claimed", open: true, recorded: false,
        telemetry_source: "not_recorded", claimed_at_ms: 1_700_000_000_000, observed_at_ms: null, ended_at_ms: null,
        wall_time_ms: null, tokens: null, tokens_status: "absent", tokens_absence_reason: "worker_telemetry_absent",
        model_turns: null, model_turns_status: "absent", model_turns_absence_reason: "worker_telemetry_absent",
        auxiliary_calls: null, auxiliary_calls_status: "absent", auxiliary_calls_absence_reason: "worker_telemetry_absent",
        harness: "omp", model: null, worker_terminal: null, verification_outcome: null,
      },
    ],
    bounded: true,
    truncated: false,
    non_claims: ["Display is not an admission grant."],
  };

  const posts = [];
  let confirmCalls = 0;
  await fc.mount(root, {
    isCurrent: () => true,
    canOperate: () => true,
    isFresh: () => true,
    confirmControl: async () => {
      confirmCalls += 1;
      return { confirmed: true, reason: "", requestId: "req-share-1" };
    },
    async api(path, opts) {
      if (opts?.method === "POST") {
        posts.push({ path, body: opts.body });
        if (opts.body.shares["tenant-alpha"] === 999) {
          throw Object.assign(new Error("admission_shares_invalid: share exceeds bound"), { status: 400 });
        }
        return { schema: "kiln.implementation-admission-result.v1", result: "updated", policy_identity: HEX_P, request_id: opts.body.request_id };
      }
      if (path === "/api/implementation/admission") return admissionPayload;
      if (path === "/api/fleet/work") return { ready_unclaimed: 0, ranked: [], blocked: [] };
      if (path === "/api/workers") return { schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, summary: {}, workers: [] };
      throw new Error(`unexpected ${path}`);
    },
  });

  const text = allText(root);
  assert.match(text, /tenant-alpha/);
  assert.match(text, /tenant-beta/);
  assert.match(text, /Transfers \(transferred_out\)/);
  assert.match(text, /tokens: 350/);
  assert.match(text, /tokens: 50/);
  assert.match(text, /4,200/);
  assert.match(text, /5,000 ms/);
  // Missing telemetry row must show 'not recorded', NEVER '0'
  const rows = root.querySelectorAll("tr");
  const missingRow = rows.find((r) => allText(r).includes(HEX_D.slice(0, 12)));
  assert.ok(missingRow, "missing telemetry attempt row must render");
  const missingText = allText(missingRow);
  assert.match(missingText, /not recorded \(worker_telemetry_absent\)/);
  assert.doesNotMatch(missingText, /\b0 ms\b/);

  // Negative: invalid share (0) fails closed client-side without POST
  const shareForm = root.querySelector(".fleet-admission-share-form");
  const inputs = shareForm.querySelectorAll("input");
  inputs[0].value = "tenant-alpha";
  inputs[1].value = "0";
  await shareForm.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(posts.length, 0);
  assert.match(allText(root), /positive integer share/i);

  // Positive: valid share update routes through Tier-2 confirmation & posts exact schema
  inputs[1].value = "75";
  await shareForm.listeners.get("submit")[0]({ preventDefault() {} });
  await tick();
  assert.equal(confirmCalls, 1);
  assert.equal(posts.length, 1);
  assert.equal(posts[0].path, "/api/implementation/admission/policy");
  assert.deepEqual(JSON.parse(JSON.stringify(posts[0].body)), {
    request_id: "req-share-1",
    expected_policy_id: HEX_P,
    command: "configure",
    shares: { "tenant-alpha": 75, "tenant-beta": 40 },
  });

  // Negative: backend 400 error surfaces explicitly
  const refreshedForm = root.querySelector(".fleet-admission-share-form");
  const refreshedInputs = refreshedForm.querySelectorAll("input");
  refreshedInputs[0].value = "tenant-alpha";
  refreshedInputs[1].value = "999";
  await refreshedForm.listeners.get("submit")[0]({ preventDefault() {} });
  await tick();
  assert.match(allText(root), /refused \(400\).*admission_shares_invalid/);

  // Negative: malformed admission schema throws
  assert.throws(() => fc.validateAdmission({ schema: "wrong", configured: true, bounded: true, accounts: [], serving: [], attempt_metrics: [] }));
  fc.clear();

  // Negative: stale snapshot (isFresh() === false) blocks Tier-2 share update without calling confirmControl or POST
  const staleRoot = env.doc.createElement("div");
  const stalePosts = [];
  await fc.mount(staleRoot, {
    isCurrent: () => true,
    canOperate: () => true,
    isFresh: () => false,
    confirmControl: async () => { throw new Error("must not open confirmation when stale"); },
    async api(path, opts) {
      if (opts?.method === "POST") { stalePosts.push(path); return {}; }
      if (path === "/api/implementation/admission") return admissionPayload;
      if (path === "/api/fleet/work") return { ready_unclaimed: 0, ranked: [], blocked: [] };
      if (path === "/api/workers") return { schema: "kiln.web-workers.v1", bounded: true, storage_failed: false, summary: {}, workers: [] };
      throw new Error(`unexpected ${path}`);
    },
  });
  const staleForm = staleRoot.querySelector(".fleet-admission-share-form");
  const staleInputs = staleForm.querySelectorAll("input");
  staleInputs[0].value = "tenant-alpha";
  staleInputs[1].value = "80";
  await staleForm.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(stalePosts.length, 0);
  assert.match(allText(staleRoot), /stale or disconnected/i);
  fc.clear();
});

// 2. Plannotator-Style Plan Review Step Annotations & 409 Stale-Revision Guard (r[kiln.implementation.plan_review_console_surface])
test("Plan Review renders step annotations, sends expected_revision, and fails closed on 409 stale revision", async () => {
  const env = createDomEnv(["inventory.js", "cairn.js"]);
  const cairn = env.window.KilnCairn;
  const host = env.doc.createElement("div");
  const posts = [];
  let currentRev = 2;

  cairn.renderPlanReview(host, {
    workId: HEX_A,
    planId: HEX_B,
    revision: 2,
    gate: "unreviewed",
    author: "agent-omp",
    receiptDigest: HEX_C,
    steps: [
      { id: "step-1", text: "Add bounded parser" },
      { id: "step-2", text: "Add negative tests" },
    ],
    annotations: [
      { annotation: "ann-1", revision: 2, reviewer: "human-op", region: { kind: "step", step: "step-1" }, note: { kind: "comment", text: "Check UTF-8 limit" }, blocking: true },
    ],
    comparison: { previous: HEX_D, current: HEX_B, kept: ["step-2"], changed: ["step-1"], added: [], removed: [], reordered: false },
    canOperate: true,
    async api(path, opts) {
      if (opts?.method === "POST") {
        posts.push({ path, body: opts.body });
        if (path === "/api/implementation/plan-review" && opts.body.expected_revision !== currentRev) {
          throw Object.assign(new Error("implementation_plan_revision: expected revision 2, actual revision 3"), { status: 409 });
        }
        return {
          schema: "kiln.plan-feedback.v1",
          work_id: HEX_A,
          plan_id: HEX_B,
          revision: 2,
          author: "agent-omp",
          gate: "unresolved",
          steps: [{ id: "step-1", text: "Add bounded parser" }, { id: "step-2", text: "Add negative tests" }],
          items: [
            { annotation: "ann-1", revision: 2, reviewer: "human-op", region: { kind: "step", step: "step-1" }, note: { kind: "comment", text: "Check UTF-8 limit" }, blocking: true },
            { annotation: "ann-2", revision: 2, reviewer: "human-op", region: { kind: "step", step: "step-2" }, note: { kind: "comment", text: "Include 409 test" }, blocking: false },
          ],
          receipt_digest: HEX_P,
        };
      }
      if (path.startsWith("/api/implementation/plan-feedback?work=")) {
        return {
          schema: "kiln.plan-feedback.v1",
          work_id: HEX_A,
          plan_id: HEX_D,
          revision: 3,
          author: "agent-omp",
          gate: "unreviewed",
          steps: [{ id: "step-1", text: "Add bounded parser v3" }],
          items: [],
          receipt_digest: HEX_D,
        };
      }
      throw new Error(`unexpected ${path}`);
    },
  });

  const initialText = allText(host);
  assert.match(initialText, /Add bounded parser/);
  assert.match(initialText, /Check UTF-8 limit/);
  assert.match(initialText, new RegExp(HEX_C.slice(0, 12)));
  assert.match(initialText, /changed: 1/);

  // Submit step-level feedback on step-2
  const forms = host.querySelectorAll(".cairn-plan-feedback-form");
  assert.equal(forms.length, 2);
  const step2Input = forms[1].querySelector("input");
  step2Input.value = "Include 409 test";
  await forms[1].listeners.get("submit")[0]({ preventDefault() {} });
  await tick();
  assert.equal(posts.length, 1);
  assert.equal(posts[0].path, "/api/implementation/plan-feedback");
  assert.equal(posts[0].body.step_id, "step-2");
  assert.equal(posts[0].body.comment, "Include 409 test");
  assert.equal(posts[0].body.expected_revision, 2);
  assert.match(allText(host), /Include 409 test/);

  // Simulate concurrent revision bump -> 409 stale revision mismatch blocks acceptance
  currentRev = 3;
  const acceptBtn = descendants(host).find((b) => b.tagName === "button" && b.textContent === "Accept plan");
  acceptBtn.click();
  await tick();
  assert.equal(posts.length, 2);
  assert.equal(posts[1].path, "/api/implementation/plan-review");
  assert.equal(posts[1].body.expected_revision, 2);
  assert.match(allText(host), /Stale plan revision mismatch \(409\)/);
  assert.equal(acceptBtn.disabled, true, "stale decision button must remain disabled until reloaded");

  // Reloading fetches revision 3 and clears stale lock
  const reloadBtn = descendants(host).find((b) => b.tagName === "button" && b.textContent === "Reload latest plan");
  reloadBtn.click();
  await tick();
  assert.match(allText(host), /Add bounded parser v3/);
  assert.match(allText(host), /Revision 3/);
  assert.equal(acceptBtn.disabled, false);
});

// 3. Guarded Publication & Integration Recovery failing closed (r[kiln.cairn.gate_passport_surface])
test("Guarded publication and integration recovery gate through Tier 2/3 confirmation and fail closed on stale/unauthorized", async () => {
  const env = createDomEnv(["inventory.js", "cairn.js"]);
  const cairn = env.window.KilnCairn;
  const index = { schema: "kiln.cairn-workspace-index.v2", truncated: false, snapshot: {}, non_claims: [], changes: [] };
  const makeDetail = (applicability, candidate) => ({
    schema: "kiln.cairn-workspace-detail.v4",
    identity: HEX_A,
    repository_identity: HEX_B,
    change: "demo-change",
    projects: ["kiln"],
    overview: { lane: "verified", lanes: { verified: 1 }, units: 1, current_work: 1, attempts: 1, active_attempts: 0, campaigns: 0, blockers: 0, open_decisions: 0 },
    snapshot: { work_recorded: true, attempts_recorded: true },
    non_claims: [],
    blockers: [],
    decisions: [],
    memberships: [],
    agents: { source: "not_configured", definitions: [], workers: [], workflow_revisions: { source: "not_configured", definitions: [] } },
    units: [{
      native_id: "demo-change/u1",
      unit: "u1",
      sources: ["canonical_work"],
      lane: "verified",
      facets: { readiness: "ready", execution: "verified", evidence: "current_verified", integration: "not_published", lifecycle: "not_observed" },
      current_work: [{ identity: HEX_C, canonical_key: HEX_D, revision: 1, project: "kiln", title: "Demo unit", requested_outcome: "", readiness_snapshot: HEX_A, acceptance: [], dependencies: [], dependents: [], terminal_policy: "verified_candidate", priority: 1, readiness: "ready", block_reasons: [] }],
      superseded_work: [],
      attempts: [{
        identity: HEX_D, ordinal: 1, fence: 5, phase: "verified", active: false,
        work_identity: HEX_C, work_revision: 1, applicability,
        claimed_at_ms: 1_700_000_000_000, lease_until_ms: 1_700_001_000_000, ended_at_ms: 1_700_000_010_000,
        dispatch: "transport_started",
        candidate: candidate ? { provider_namespace: "git", native_kind: "commit", native_id: candidate } : null,
        verification: { run_id: "run-1", success: true, candidate: { provider_namespace: "git", native_kind: "commit", native_id: candidate || "none" }, tree: { provider_namespace: "git", native_kind: "tree", native_id: "t1" }, verification_profile: "default", checks: [] },
        publication: null,
        lifecycle: null,
      }],
    }],
  });

  // 3a. Negative: viewer (canOperate: false) -> disabled + diagnostic, zero posts
  const rootViewer = env.doc.createElement("div");
  const posts = [];
  await cairn.load(rootViewer, {
    route: { cairn_repository: HEX_B, cairn_change: "demo-change", cairn_unit: "demo-change/u1" },
    navigate() {},
    canOperate: () => false,
    async api(path, opts) {
      if (opts?.method === "POST") { posts.push({ path, body: opts.body }); return {}; }
      return path === "/api/implementation/cairn" ? index : makeDetail("current", "deadbeef");
    },
  });
  const viewerBtns = descendants(rootViewer).filter((b) => b.tagName === "button" && b.dataset?.lifecycleAction);
  assert.equal(viewerBtns.length, 4);
  assert.ok(viewerBtns.every((b) => b.disabled === true));
  assert.match(allText(rootViewer), /Operator authority required/);
  cairn.clear();

  // 3b. Negative: historical/stale candidate -> fail closed, disabled
  const rootStale = env.doc.createElement("div");
  await cairn.load(rootStale, {
    route: { cairn_repository: HEX_B, cairn_change: "demo-change", cairn_unit: "demo-change/u1" },
    navigate() {},
    canOperate: () => true,
    async api(path) {
      return path === "/api/implementation/cairn" ? index : makeDetail("historical", "deadbeef");
    },
  });
  const staleBtns = descendants(rootStale).filter((b) => b.tagName === "button" && b.dataset?.lifecycleAction);
  assert.ok(staleBtns.every((b) => b.disabled === true));
  assert.match(allText(rootStale), /Stale\/historical attempt/);
  cairn.clear();

  // 3c. Positive & backend 403 refusal with CLI parity
  const rootOp = env.doc.createElement("div");
  let confirmFactCheck = null;
  await cairn.load(rootOp, {
    route: { cairn_repository: HEX_B, cairn_change: "demo-change", cairn_unit: "demo-change/u1" },
    navigate() {},
    canOperate: () => true,
    confirmControl: async (opts) => {
      confirmFactCheck = opts;
      return { confirmed: true, reason: "Operator approved publish", requestId: "req-life-1" };
    },
    async api(path, opts) {
      if (opts?.method === "POST") {
        posts.push({ path, body: opts.body });
        if (path.endsWith("/publish")) {
          const err = new Error("lifecycle_provider_mutation_unsupported: Radicle push requires explicit operator CLI");
          err.status = 403;
          err.cli_parity = "kiln-implementer publish --work ...";
          throw err;
        }
        return { schema: "kiln.implementation-lifecycle-result.v1", action: "prepare-publication", work_id: HEX_C, attempt_id: HEX_D, fence: 5, candidate_identity: opts.body.candidate_identity, publication_digest: HEX_P, already_recorded: false, request_id: opts.body.request_id };
      }
      return path === "/api/implementation/cairn" ? index : makeDetail("current", "cand-sha-1");
    },
  });
  const prepBtn = descendants(rootOp).find((b) => b.dataset?.lifecycleAction === "prepare-publication");
  prepBtn.click();
  await tick();
  assert.equal(posts.length, 1);
  assert.equal(posts[0].path, "/api/implementation/lifecycle/prepare-publication");
  assert.equal(posts[0].body.work_id, HEX_C);
  assert.equal(posts[0].body.attempt_id, HEX_D);
  assert.equal(posts[0].body.fence, 5);
  assert.equal(posts[0].body.reason, "Operator approved publish");
  assert.ok(confirmFactCheck.cliCommand.includes("kiln-implementer prepare-publication"));

  const pubBtn = descendants(rootOp).find((b) => b.dataset?.lifecycleAction === "publish");
  pubBtn.click();
  await tick();
  assert.match(allText(rootOp), /Publish refused \(403\).*CLI parity: kiln-implementer publish/);
  cairn.clear();
});

// 4. Structured Campaign Obligations/Budget tables in inventory.js
test("Inventory renderCampaign renders structured obligations and budget tables with collapsible raw JSON", () => {
  const env = createDomEnv(["inventory.js"]);
  const inv = env.window.KilnInventory;
  const fixture = JSON.parse(readFileSync(new URL("../fixtures/selected-campaign/projection-v1.json", import.meta.url), "utf8"));
  const page = inv.renderCampaign(fixture);
  const text = allText(page);
  assert.ok(page.querySelectorAll("table").length >= 1, "campaign obligations/freshness use structured tables");
  assert.match(text, /Selection basis and next-action proposal/);
  assert.match(text, /Aggregate budget/);
  assert.match(text, /Raw snapshot-level obligations and non-claims JSON/);
  assert.equal(descendants(page).some((n) => n.tagName === "button" || n.tagName === "a"), false, "renderCampaign stays read-only");
});

// 5. Opt-in Bounded Atlas Topology (r[kiln.web.causal_timeline_and_failure_digest])
test("Atlas Topology is off by default, renders <=64-node Observed solid vs Declared dashed graph, and refuses >64 nodes with bounded summary", async () => {
  const env = createDomEnv(["inventory.js", "cairn.js"]);
  const cairn = env.window.KilnCairn;

  // Off by default on cairn detail
  const root = env.doc.createElement("div");
  const index = { schema: "kiln.cairn-workspace-index.v2", truncated: false, snapshot: {}, non_claims: [], changes: [] };
  const detail = {
    schema: "kiln.cairn-workspace-detail.v4",
    identity: HEX_A,
    repository_identity: HEX_B,
    change: "atlas-demo",
    projects: ["kiln"],
    overview: { lane: "verified", lanes: { verified: 1 }, units: 1, current_work: 1, attempts: 1, active_attempts: 0, campaigns: 0, blockers: 0, open_decisions: 0 },
    snapshot: { work_recorded: true, attempts_recorded: true },
    non_claims: [],
    blockers: [],
    decisions: [],
    memberships: [],
    agents: { source: "not_configured", definitions: [], workers: [], workflow_revisions: { source: "not_configured", definitions: [] } },
    units: [{
      native_id: "atlas-demo/u1", unit: "u1", sources: ["canonical_work"], lane: "verified",
      facets: { readiness: "ready", execution: "verified", evidence: "current_verified", integration: "not_published", lifecycle: "not_observed" },
      current_work: [{ identity: HEX_C, canonical_key: HEX_D, revision: 1, project: "kiln", title: "U1", requested_outcome: "", readiness_snapshot: HEX_A, acceptance: [], dependencies: [], dependents: [], terminal_policy: "verified_candidate", priority: 1, readiness: "ready", block_reasons: [] }],
      superseded_work: [],
      attempts: [{ identity: HEX_D, ordinal: 1, fence: 1, phase: "verified", active: false, work_identity: HEX_C, work_revision: 1, applicability: "current", claimed_at_ms: 1000, lease_until_ms: 2000, dispatch: "transport_started", verification: { run_id: "run-9", success: true, candidate: { provider_namespace: "git", native_kind: "commit", native_id: "c1" }, tree: { provider_namespace: "git", native_kind: "tree", native_id: "t1" }, verification_profile: "default", checks: [] } }],
    }],
  };
  await cairn.load(root, {
    route: { cairn_repository: HEX_B, cairn_change: "atlas-demo" },
    navigate() {},
    async api(path) { return path === "/api/implementation/cairn" ? index : detail; },
  });
  const atlasHost = root.querySelector(".cairn-atlas-host");
  assert.equal(atlasHost.hidden, true, "Atlas must be off by default");
  root.querySelector(".cairn-atlas-toggle").click();
  assert.equal(atlasHost.hidden, false);
  assert.ok(atlasHost.querySelector(".cairn-atlas"), "Opt-in click renders bounded Atlas");
  cairn.clear();

  // Direct Atlas rendering with Observed (solid) vs Declared (dashed) routes & packets
  const smallDoc = {
    identity: HEX_A,
    title: "Small Topology",
    nodes: [
      { id: "n1", label: "Plan", role: "planner", lane: "ready" },
      { id: "n2", label: "Worker", role: "executor", lane: "in_progress" },
      { id: "n3", label: "Receipt", role: "verifier", lane: "verified" },
    ],
    routes: [
      { from: "n1", to: "n2", label: "handoff", provenance: "declared" },
      { from: "n2", to: "n3", label: "verified_run", provenance: "observed", digest: HEX_C },
    ],
    packets: [
      {
        id: "p1", label: "Flow",
        hops: [
          { from: "n1", to: "n2", provenance: "declared", payload: { intent: "implement" } },
          { from: "n2", to: "n3", provenance: "observed", observation_id: "run-9", digest: HEX_C, payload: { passed: true } },
        ],
      },
    ],
  };
  const rendered = cairn.renderAtlasTopology(smallDoc);
  assert.ok(rendered.querySelector(".dag-canvas"), "Canvas rendered for <=64 nodes");
  const edges = rendered.querySelectorAll(".cairn-atlas-edge");
  assert.equal(edges.length, 2);
  assert.equal(edges[0].dataset.provenance, "declared");
  assert.equal(edges[0].getAttribute("stroke-dasharray"), "6 4");
  assert.equal(edges[1].dataset.provenance, "observed");
  assert.equal(edges[1].getAttribute("stroke-dasharray"), null);
  assert.match(allText(rendered), new RegExp(cairn.ATLAS_DECLARED_QUALIFIER));

  // Negative: >64 nodes (65 nodes) strictly refuses unbounded graph and renders bounded summary table
  const hugeDoc = {
    identity: HEX_B,
    title: "Oversized Topology",
    nodes: Array.from({ length: 65 }, (_, i) => ({ id: `node-${i}`, label: `Node ${i}`, role: "worker" })),
    routes: [{ from: "node-0", to: "node-1", label: "step", provenance: "declared" }],
  };
  const fallback = cairn.renderAtlasTopology(hugeDoc);
  assert.equal(fallback.querySelector(".dag-canvas"), null, "Must refuse .dag-canvas when nodes > 64");
  assert.equal(fallback.querySelector("svg"), null, "Must refuse SVG graph when nodes > 64");
  assert.ok(fallback.querySelector(".cairn-atlas-summary-table"), "Must render bounded summary table");
  assert.match(allText(fallback), /64-node/);
  assert.match(allText(fallback), /65 nodes/);
  assert.equal(fallback.querySelector("tbody").querySelectorAll("tr").length, 64, "Bounded summary caps rows at 64");

  // Negative: malformed document (off-route packet hop) refused
  const badHopDoc = {
    ...smallDoc,
    packets: [{ id: "bad", hops: [{ from: "n1", to: "n3", provenance: "declared", payload: "bad" }] }],
  };
  assert.equal(cairn.admitAtlasDocument(badHopDoc).ok, false);
});
