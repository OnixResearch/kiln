import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const asset = readFileSync(new URL("../crates/kiln-web-service/src/runbooks.js", import.meta.url), "utf8");
const route = "/api/lattice/authoring";

class Element {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.listeners = {};
    this.textContent = "";
  }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.children = children; }
  setAttribute() {}
  addEventListener(event, handler) { this.listeners[event] = handler; }
  find(tag) { return this.tag === tag ? this : this.children.map((node) => node.find(tag)).find(Boolean); }
  findAll(tag) { return [...(this.tag === tag ? [this] : []), ...this.children.flatMap((node) => node.findAll(tag))]; }
  copy() { return [this.textContent, ...this.children.map((node) => node.copy())].join(" "); }
}

function harness() {
  const context = { window: {}, TextDecoder, document: { createElement: (tag) => new Element(tag) } };
  vm.runInNewContext(asset, context);
  return { runbooks: context.window.KilnRunbooks, root: new Element("div") };
}

function revision(id) {
  return {
    workflow: "review-me", archive_blake3: id.repeat(64), source_revision: "c".repeat(64),
    expanded_artifact: "d".repeat(64), state: "CapturedModules", entry: "workflow.ncl",
    members: ["assets/raw.bin", "workflow.ncl"],
  };
}

const settled = () => new Promise((resolve) => setImmediate(resolve));

test("operator can inspect exact pinned sources; a superseded member response cannot replace the current selection", async () => {
  const { runbooks, root } = harness();
  const calls = [];
  let completeOld;
  const api = async (path, options) => {
    calls.push([path, options?.kind]);
    if (path === "/api/runbooks") return {}; // Source view is independent of reviewed dispatch availability.
    if (path === route) return { revisions: [revision("a"), revision("b")] };
    if (path === `${route}?archive=${"a".repeat(64)}&member=1`) return new Promise((resolve) => { completeOld = resolve; });
    if (path === `${route}?archive=${"b".repeat(64)}&member=1`) return new Blob(["\uFEFF<script>source only</script>\r\n"]);
    throw new Error(`Unexpected request: ${path}`);
  };
  await runbooks.load(root, { api, canOperate: true });
  assert.match(root.copy(), /not live catalog heads/);
  assert.equal(calls.length, 1);
  await root.find("button").listeners.click();
  const [revisionSelect] = root.findAll("select");
  revisionSelect.value = "1";
  revisionSelect.listeners.change();
  await settled();
  assert.equal(root.find("pre").textContent, "\uFEFF<script>source only</script>\r\n");
  completeOld(new Blob(["superseded source"]));
  await settled();
  assert.equal(root.find("pre").textContent, "\uFEFF<script>source only</script>\r\n");
  assert.equal(root.findAll("button").length, 1);
  assert.deepEqual(calls, [
    ["/api/runbooks", undefined], [route, undefined],
    [`${route}?archive=${"a".repeat(64)}&member=1`, "blob"], [`${route}?archive=${"b".repeat(64)}&member=1`, "blob"],
  ]);
});

test("unconfigured sources are explicit and viewers have no source control", async () => {
  const { runbooks, root } = harness();
  const api = async (path) => {
    if (path === "/api/runbooks") return {};
    assert.equal(path, route);
    throw Object.assign(new Error("operator path is private"), { code: "lattice_authoring_unconfigured" });
  };
  await runbooks.load(root, { api, canOperate: true });
  await root.find("button").listeners.click();
  assert.match(root.copy(), /No Lattice source archive index is configured/);
  assert.doesNotMatch(root.copy(), /operator path is private/);
  await runbooks.load(root, { api, canOperate: false });
  assert.equal(root.find("button"), undefined);
});
