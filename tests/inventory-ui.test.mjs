import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

class Element {
  constructor(tag) { this.tagName = tag; this.children = []; this.attributes = new Map(); this.dataset = {}; this.isConnected = true; this.textContent = ""; this.listeners = new Map(); }
  append(...nodes) { for (const node of nodes) { node.parent = this; this.children.push(node); } }
  replaceChildren(...nodes) { for (const node of this.children) node.isConnected = false; this.children = []; this.append(...nodes); }
  setAttribute(key, value) { this.attributes.set(key, value); }
  removeAttribute(key) { this.attributes.delete(key); }
  addEventListener(type, handler) { this.listeners.set(type, handler); }
  remove() { this.parent.children = this.parent.children.filter((x) => x !== this); }
  get lastChild() { return this.children.at(-1); }
  get firstChild() { return this.children[0]; }
  querySelector(selector) { return selector === "#inventory-work h3" ? this.children.find((x) => x.id === "inventory-work")?.firstChild : null; }
  focus() { this.focused = true; }
  scrollIntoView() {}
}
const source = readFileSync(new URL("../crates/kiln-web-service/src/inventory.js", import.meta.url), "utf8");
const input = JSON.parse(readFileSync(new URL("../fixtures/inventory/synthetic-v2.json", import.meta.url), "utf8"));
function setup() {
  const context = { window: {}, document: { createElement: (tag) => new Element(tag) } };
  vm.runInNewContext(source, context);
  return context.window.KilnInventory;
}
function fixture() {
  const snapshot = structuredClone(input.snapshot);
  return {
    schema: input.profile.schema, identity: "a".repeat(64), profile_identity: "b".repeat(64), evaluated_at_ms: 2000,
    sources: snapshot.sources.map((s) => ({ ...s, state: "current" })),
    facts: ["agents", "workers", "compute", "pools", "models", "admissions", "policies"].flatMap((key) => snapshot[key].map((x) => ({ reference: x.meta.reference, state: "current" }))),
    snapshot, explanation: null, attempt_explanations: [], work_requirements_unknown: true, non_claims: ["read_only_no_dispatch_authority"],
  };
}
function allText(node) { return [node.textContent, ...node.children.map(allText)].join("\n"); }
function descendants(node) { return [node, ...node.children.flatMap(descendants)]; }

// r[verify kiln.inventory.consumers]
test("four separate views preserve bindings and do not turn a definition into idle workers", () => {
  const inventory = setup();
  const projection = fixture();
  projection.snapshot.workers = [];
  const page = inventory.render(projection);
  const text = allText(page);
  for (const title of ["Agents —", "Workers —", "Compute —", "Models / Serving —"]) assert.ok(text.includes(title));
  assert.ok(text.includes("0 supplied; not a fleet total"));
  assert.ok(text.includes("No rows supplied in this bounded snapshot"));
  assert.ok(text.includes("Synthetic, disconnected example"));
  assert.ok(text.includes("Exact current admitted work requirements are unknown"));
  assert.equal(descendants(page).filter((node) => node.tagName === "nav")[0].attributes.get("aria-label"), "Resource inventory sections");
});

test("shared service/pool identities are shown once without summing device references", () => {
  const inventory = setup();
  const projection = fixture();
  const page = inventory.render(projection);
  const text = allText(page);
  assert.ok(text.includes("workers / worker-a"));
  assert.ok(text.includes("workers / worker-b"));
  assert.ok(text.includes("serving / shared-service"));
  assert.ok(text.includes("Actually pinned revision"));
  assert.ok(text.includes("Count this exact pool once"));
  assert.ok(text.includes("Claimed model (not exact weight verification)"));
  assert.equal(descendants(page).filter((node) => node.tagName === "h4" && node.textContent === "remote-model · warm").length, 1);
});

test("unknown/stale and partial provider facts remain explicit beside capacities", () => {
  const inventory = setup();
  const projection = fixture();
  const model = projection.snapshot.models[0];
  projection.sources.find((x) => x.source === "serving").state = "incomplete";
  projection.facts.find((x) => x.reference.source === "serving").state = "stale";
  model.available_requests = null;
  model.meta.unknown_fields = ["inference_capacity"];
  const basis = inventory.sourceBasis(projection, model.meta);
  assert.ok(basis.includes("source incomplete"));
  assert.ok(basis.includes("fact stale"));
  assert.ok(basis.includes("inference_capacity"));
  assert.ok(allText(inventory.render(projection)).includes("Unknown"));
  projection.sources = [];
  assert.equal(inventory.sourceBasis(projection, model.meta), "Source missing; freshness unknown");
});

test("producer labels stay inert and no endpoint becomes a link or remote resource", () => {
  const inventory = setup();
  const projection = fixture();
  const attack = '<img src="https://private.example" onerror="bad()">';
  projection.snapshot.models[0].claimed_model = attack;
  projection.snapshot.agents[0].qualifications[0].limitations = ["</script><script>bad()</script>"];
  const page = inventory.render(projection);
  assert.ok(allText(page).includes(attack));
  assert.equal(descendants(page).filter((node) => ["script", "img", "iframe"].includes(node.tagName)).length, 0);
  assert.ok(descendants(page).filter((node) => node.tagName === "a").every((node) => node.href.startsWith("#inventory-")));
});

test("malformed/unsupported/unbounded projections fail before rendering capacity", () => {
  const inventory = setup();
  for (const change of [
    (p) => { p.schema = "kiln.observe.inventory.v1"; },
    (p) => { p.identity = "not-an-identity"; },
    (p) => { p.snapshot = null; },
    (p) => { p.snapshot.models = null; },
    (p) => { p.snapshot.workers[0].meta.reference.source = "https://private"; },
    (p) => { p.sources[0].state = "probably-ready"; },
    (p) => { p.snapshot.agents = Array(4097).fill(p.snapshot.agents[0]); },
  ]) {
    const projection = fixture(); change(projection); assert.throws(() => inventory.validate(projection));
  }
});

test("reservation rejection stays visible in the exact attempt drilldown", () => {
  const inventory = setup();
  const projection = fixture();
  projection.attempt_explanations = [{ binding: { work_version: "exact-work", attempt: "attempt-a", fence: 9, worker: { source: "workers", id: "worker-a" }, execution_grant: { source: "policy", id: "grant-a" }, model_services: [{ source: "serving", id: "shared-service" }], request_admissions: [{ source: "policy", id: "request-a" }] }, blockers: [{ code: "reservation_not_admitted", constraint: "serving_queue_full", subject: { source: "policy", id: "request-a" } }] }];
  const text = allText(inventory.render(projection));
  assert.ok(text.includes("Attempt attempt-a · fence 9"));
  assert.ok(text.includes("reservation_not_admitted"));
  assert.ok(text.includes("serving_queue_full"));
});

test("delayed reads cannot restore private inventory after clear", async () => {
  const inventory = setup();
  const root = new Element("div");
  let resolve;
  const pending = new Promise((ready) => { resolve = ready; });
  const load = inventory.load(root, { api: () => pending, isCurrent: () => true });
  inventory.clear();
  resolve(fixture());
  await load;
  assert.equal(root.children.length, 0);
  assert.equal(root.attributes.has("aria-busy"), false);
});

test("unavailable endpoint produces unknown rather than zero resources", async () => {
  const inventory = setup();
  const root = new Element("div");
  await inventory.load(root, { api: async () => { throw new Error("inventory_input_not_configured"); }, isCurrent: () => true });
  const text = allText(root);
  assert.ok(text.includes("Observed inventories unavailable"));
  assert.ok(text.includes("No absence, liveness, free compute or model capacity inferred"));
  assert.ok(!text.includes("0 workers"));
});

test("scoped identities containing colons cannot collapse into another producer identity", () => {
  const inventory = setup();
  assert.notEqual(inventory.refText({ source: "a:b", id: "c" }), inventory.refText({ source: "a", id: "b:c" }));
});

test("selected-campaign projection preserves partial and missing members without active controls", () => {
  const campaign = setup();
  const fixture = JSON.parse(readFileSync(new URL("../fixtures/selected-campaign/projection-v1.json", import.meta.url), "utf8"));
  assert.equal(campaign.validateCampaign(fixture), fixture);
  const page = campaign.renderCampaign(fixture);
  const text = allText(page);
  assert.ok(text.includes(fixture.identity));
  assert.ok(text.includes("incomplete_limit"));
  assert.ok(text.includes("stale"));
  assert.ok(text.includes("change-b/task-two"));
  assert.ok(text.includes("missing_source_or_owner_observation"));
  assert.ok(text.includes("<script>not markup</script>"));
  assert.ok(text.includes("Selection hold: wait · selected member awaits its first source/readiness observation · owner native-source-owner"));
  const withoutHolds = { ...fixture, members: fixture.members.map(({ selection_hold, ...member }) => member) };
  assert.ok(allText(campaign.renderCampaign(withoutHolds)).includes("Selection hold: none recorded"));
  assert.throws(() => campaign.validateCampaign({ ...fixture, members: [{ ...fixture.members[0], selection_hold: { operation: "wait" } }] }));
  assert.equal(descendants(page).some((node) => node.tagName === "button" || node.tagName === "a"), false);
  assert.throws(() => campaign.validateCampaign({ ...fixture, schema: "kiln.observe.inventory.v2" }));
  assert.throws(() => campaign.validateCampaign({ ...fixture, members: Array(4097).fill(fixture.members[0]) }));
});

// r[verify kiln.cairn.workspace]
test("each selected change links to its exact Cairn change beside the inert projection", () => {
  const campaign = setup();
  const fixture = JSON.parse(readFileSync(new URL("../fixtures/selected-campaign/projection-v1.json", import.meta.url), "utf8"));
  const links = descendants(campaign.cairnLinks(fixture)).filter((node) => node.tagName === "a");
  const changes = [...new Set(fixture.members.map((member) => member.change))];
  assert.ok(changes.length > 0);
  assert.deepEqual(links.map((link) => link.textContent), changes);
  assert.deepEqual(links.map((link) => link.href), changes.map((change) => `?page=cairn&cairn_repository=${fixture.repository_identity}&cairn_change=${encodeURIComponent(change)}`));
});

test("resource and campaign loads finish independently when both are deferred", async () => {
  const inventory = setup();
  const resourceRoot = new Element("div");
  const campaignRoot = new Element("div");
  const campaignFixture = JSON.parse(readFileSync(new URL("../fixtures/selected-campaign/projection-v1.json", import.meta.url), "utf8"));
  const selectedId = "a/ b+雪";
  let resolveResource; let resolveCampaign;
  const calls = [];
  const ctx = {
    isCurrent: () => true,
    api(path) {
      calls.push(path);
      return new Promise((resolve) => {
        if (path === "/api/inventory") resolveResource = resolve;
        else resolveCampaign = resolve;
      });
    },
  };
  const resourceRequest = inventory.load(resourceRoot, ctx);
  const campaignRequest = inventory.loadCampaign(campaignRoot, ctx, selectedId);
  assert.equal(calls.length, 2);
  assert.ok(calls.includes("/api/inventory"));
  assert.ok(calls.includes(`/api/implementation/campaigns?campaign=${encodeURIComponent(selectedId)}`));
  resolveCampaign(campaignFixture);
  resolveResource(fixture());
  await Promise.all([resourceRequest, campaignRequest]);
  assert.ok(allText(resourceRoot).includes("Observed resource inventories"));
  assert.ok(allText(campaignRoot).includes(campaignFixture.identity));
  inventory.clear();
  assert.equal(resourceRoot.children.length, 0);
  assert.equal(campaignRoot.children.length, 0);
});

const HEX_A = "a".repeat(64);
function inboxFixture(overrides = {}) {
  const decision = (identity, category, state, scope, actions) => ({
    identity: identity.repeat(64), member: "change-a/*", category, state, question: "Approve <b>the candidate</b>?", options: ["approve", "reject"],
    recommendation: "approve", work_identity: HEX_A, policy_identity: HEX_A, candidate_identity: "c".repeat(64), decision_revision: null,
    priority: 1, created_ms: 1, pending_reason: null, resolved_answer: null, active_effort_ms: null, scope,
    review_roles: category === "required_review" ? ["maintainer"] : [], actions,
  });
  return {
    schema: "kiln.campaign-inbox.v1", campaign_id: "selected", store_version: 7, campaign_identity: "d".repeat(64), principal: "local:operator",
    admission: "open", operations: ["pause", "cancel", "resolve_decision"], grant_expires_at_ms: null,
    decisions: [decision("e", "required_review", "outstanding", "current", ["resolve"]), decision("f", "preference", "unissued", "changed", [])],
    non_claims: ["pause stops successor admission and decision issuance; running attempts and workers are not stopped"],
    ...overrides,
  };
}
const buttons = (node) => descendants(node).filter((child) => child.tagName === "button");
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
// Intents are created in the console's realm; compare their JSON values.
const plain = (value) => JSON.parse(JSON.stringify(value));

// r[verify kiln.implementation.campaign.controls.scenario.console]
test("campaign controls offer only listed operations and answers bound to the displayed version", () => {
  const inventory = setup();
  const sent = [];
  const panel = inventory.renderControls(inboxFixture(), (intent) => sent.push(intent));
  assert.deepEqual(buttons(panel).map((button) => button.textContent), ["Pause successor admission", "Cancel campaign", "Submit answer"]);
  const text = allText(panel);
  assert.ok(text.includes("Approve <b>the candidate</b>?"));
  assert.ok(text.includes("Changed: the member moved on"));
  const cancel = buttons(panel).find((button) => button.textContent === "Cancel campaign");
  assert.equal(cancel.disabled, true, "cancel needs explicit confirmation");
  const confirm = descendants(panel).find((node) => node.type === "checkbox");
  confirm.checked = true; confirm.listeners.get("change")();
  assert.equal(cancel.disabled, false);
  buttons(panel).find((button) => button.textContent === "Pause successor admission").listeners.get("click")();
  const form = descendants(panel).find((node) => node.tagName === "form");
  form.listeners.get("submit")({ preventDefault() {} });
  assert.equal(sent.length, 1, "no answer is sent before an option is chosen");
  descendants(form).find((node) => node.type === "radio" && node.value === "reject").checked = true;
  form.listeners.get("submit")({ preventDefault() {} });
  assert.deepEqual(plain(sent), [
    { command: "pause", campaign_id: "selected", expected_version: 7 },
    { command: "resolve_decision", campaign_id: "selected", expected_version: 7, decision_identity: "e".repeat(64), work_identity: HEX_A, policy_identity: HEX_A, candidate_identity: "c".repeat(64), answer: "reject" },
  ]);
  const viewer = inventory.renderControls(inboxFixture({ operations: [], decisions: inboxFixture().decisions.map((d) => ({ ...d, actions: [] })) }), () => assert.fail("viewer sent a command"));
  assert.equal(buttons(viewer).length, 0);
  assert.ok(allText(viewer).includes("No admission control is open to this principal"));
  assert.throws(() => inventory.validateInbox(inboxFixture({ admission: "running" })));
  assert.throws(() => inventory.validateInbox(inboxFixture({ decisions: [{ ...inboxFixture().decisions[0], actions: ["approve_all"] }] })));
});

// r[verify kiln.implementation.campaign.controls.scenario.console]
test("a lost response retries the same request ID and a conflict reloads without applying", async () => {
  const inventory = setup();
  const ui = () => ({ status: new Element("p"), retry: new Element("button"), reloads: 0, applied: [], buttons: () => [] });
  const bodies = [];
  const lost = ui();
  let attempts = 0;
  const runner = inventory.commandRunner({
    async api(path, request) {
      assert.equal(path, "/api/implementation/campaigns/commands");
      bodies.push(request.body);
      attempts += 1;
      if (attempts === 1) throw new TypeError("network connection lost");
      return { replayed: true, command: "pause", store_version: 8, admission: "paused", request_id: request.body.request_id };
    },
  }, { ...lost, reload: () => { lost.reloads += 1; }, applied: (result) => lost.applied.push(result) }, (() => { let n = 0; return () => `console-request-${++n}`; })());
  await runner.start({ command: "pause", campaign_id: "selected", expected_version: 7 });
  assert.equal(lost.retry.hidden, false);
  assert.ok(lost.status.textContent.includes("may have applied"));
  lost.retry.listeners.get("click")();
  await tick();
  assert.deepEqual(bodies.map((body) => body.request_id), ["console-request-1", "console-request-1"]);
  assert.equal(lost.applied.length, 1);
  assert.ok(lost.status.textContent.startsWith("Already applied"));

  for (const [status, reloads] of [[409, 1], [403, 0]]) {
    const refused = ui();
    const conflict = inventory.commandRunner({
      async api() { throw Object.assign(new Error("campaign_version_conflict: Campaign state changed"), { status }); },
    }, { ...refused, reload: () => { refused.reloads += 1; }, applied: () => assert.fail("refused command applied") }, () => "console-request-x");
    await conflict.start({ command: "pause", campaign_id: "selected", expected_version: 6 });
    assert.ok(refused.status.textContent.includes("Nothing was applied"), String(status));
    assert.equal(refused.reloads, reloads);
    assert.notEqual(refused.retry.hidden, false);
  }
});

// r[verify kiln.implementation.campaign.controls.scenario.start]
test("start previews show refusals and start only startable frozen definitions", () => {
  const inventory = setup();
  const member = (goal, supported) => ({ member: "change-a/*", change: "change-a", tasks: [], prerequisite_inclusion: false, profile: "spec_driven", goal, goal_supported: supported, required_checks: ["acceptance"], required_reviews: [] });
  const definition = (campaign, identity, goal, supported, refusals) => ({
    definition_identity: identity.repeat(64), campaign_id: campaign, revision: 1, repository: "repo", delivery_route: "native_git",
    offered_goals: ["verified_candidate", "integrated_change", "native_lifecycle_complete"], members: [member(goal, supported)],
    max_concurrency: 1, max_outstanding_decisions: 1, parent_budget_id: HEX_A, agent_definition: HEX_A, require_one_writer_per_repository: true,
    existing: null, can_start: refusals.length === 0, refusals,
  });
  const listing = { schema: "kiln.campaign-definitions.v1", principal: "local:operator", non_claims: [], definitions: [
    definition("ready", "1", "native_lifecycle_complete", true, []),
    definition("refused", "2", "verified_pull_request", false, ["endpoint_unsupported"]),
  ] };
  const sent = [];
  const panel = inventory.renderDefinitions(listing, (intent) => sent.push(intent));
  assert.deepEqual(buttons(panel).map((button) => button.textContent), ["Start this exact selection"]);
  buttons(panel)[0].listeners.get("click")();
  assert.deepEqual(plain(sent), [{ command: "start", definition_identity: "1".repeat(64) }]);
  const text = allText(panel);
  assert.ok(text.includes("goal verified_pull_request (not offered by this route)"));
  assert.ok(text.includes("Cannot start: a member's completion goal is not offered by the delivery route."));
  assert.throws(() => inventory.validateDefinitions({ ...listing, definitions: [{ ...listing.definitions[0], definition_identity: "not-hex" }] }));
});

test("campaign controls load beside the projection and stay read-only when not configured", async () => {
  const inventory = setup();
  const root = new Element("div");
  const projection = JSON.parse(readFileSync(new URL("../fixtures/selected-campaign/projection-v1.json", import.meta.url), "utf8"));
  const calls = [];
  await inventory.loadCampaign(root, {
    isCurrent: () => true,
    async api(path) {
      calls.push(path);
      if (path.startsWith("/api/implementation/campaigns?")) return projection;
      throw Object.assign(new Error("campaign_controls_not_configured: Request could not be admitted"), { status: 503 });
    },
  }, "selected");
  await tick();
  assert.deepEqual(calls, ["/api/implementation/campaigns?campaign=selected", "/api/implementation/campaigns/inbox?campaign=selected"]);
  const text = allText(root);
  assert.ok(text.includes(projection.identity));
  assert.ok(text.includes("Campaign controls are not configured for this service"));
  assert.equal(buttons(root).length, 0);
});
