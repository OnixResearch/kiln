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
