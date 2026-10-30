import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

// Load the served icon data and helper in one isolated context, as web.html orders them.
const asset = (name) => readFileSync(new URL(`../crates/kiln-web-service/src/${name}`, import.meta.url), "utf8");
const SVG_NS = "http://www.w3.org/2000/svg";

// Minimal DOM: enough for KilnUi.icon and hydrateIcons; formatters need none.
class FakeElement {
  constructor(namespace, tag) {
    this.namespaceURI = namespace;
    this.tagName = tag;
    this.attributes = new Map();
    this.children = [];
    const element = this;
    this.classList = {
      add(name) { element.setAttribute("class", [...new Set([...element.classList, name])].join(" ")); },
      *[Symbol.iterator]() { yield* (element.getAttribute("class") || "").split(" ").filter(Boolean); },
    };
  }
  setAttribute(key, value) { this.attributes.set(key, String(value)); }
  getAttribute(key) { return this.attributes.has(key) ? this.attributes.get(key) : null; }
  append(...nodes) { this.children.push(...nodes); }
}
const placeholders = [];
const document = {
  createElementNS: (namespace, tag) => new FakeElement(namespace, tag),
  querySelectorAll: (selector) => (selector === "span[data-icon]" ? placeholders.splice(0) : []),
};
const context = { window: {}, document };
vm.runInNewContext(asset("icons.js"), context);
vm.runInNewContext(asset("ui.js"), context);
const ui = context.window.KilnUi;
const icons = context.window.KilnIcons;
const reviewedIcons = asset("icons.list").split("\n").filter(Boolean);

// r[verify kiln.web.readable_observations.scenario.failed]
test("canonical outcomes map to human labels without losing unknown values", () => {
  assert.deepEqual({ ...ui.outcomeStatus("DomainFailure") }, { status: "failure", label: "Failed" });
  assert.deepEqual({ ...ui.outcomeStatus("Success") }, { status: "success", label: "Passed" });
  assert.deepEqual({ ...ui.outcomeStatus("Pending", "active") }, { status: "running", label: "Running" });
  assert.deepEqual({ ...ui.outcomeStatus("Pending", "queued") }, { status: "queued", label: "Queued" });
  assert.deepEqual({ ...ui.outcomeStatus("FutureOutcome") }, { status: "unknown", label: "FutureOutcome" });
  assert.equal(ui.formatDuration(273357), "4m 33s");
  assert.equal(ui.formatDuration(2132), "2.1s");
  assert.equal(ui.formatDuration(42900), "42s");
  assert.equal(ui.formatDuration(3_900_000), "1h 05m");
});

// r[verify kiln.web.readable_observations.scenario.unknown]
test("absent or invalid observations read as not recorded, never zero or now", () => {
  for (const value of [null, undefined, Number.NaN, -1, "273357"]) assert.equal(ui.formatDuration(value), "Not recorded");
  assert.equal(ui.outcomeStatus(null).label, "Not recorded");
  const now = Date.parse("2026-10-29T12:00:00Z");
  for (const value of [null, undefined, "", "not a time"]) assert.equal(ui.formatRelative(value, now), "Not recorded");
  assert.equal(ui.formatRelative(now - 10_000, now), "just now");
  assert.equal(ui.formatRelative(now - 2 * 86_400_000, now), "2d ago");
  assert.equal(ui.formatRelative(now + 5 * 60_000, now), "in 5m");
  assert.equal(ui.formatRelative(now - 59 * 60_000 - 50_000, now), "1h ago");
});

// r[verify kiln.web.failure_first_run.scenario.failed]
test("error lines are recognized conservatively by index without altering text", () => {
  const log = [
    "   Compiling kiln v0.1.0",
    "error[E0425]: cannot find value `x` in this scope",
    "thread 'main' panicked at src/main.rs:2:5:",
    "test tests::it_works ... FAILED",
    "Traceback (most recent call last):",
    "ValueError: invalid literal for int()",
    "fatal: not a git repository",
    "Process exited with exit code 101",
    "Build failed.",
    "ERROR: provider rejected the request",
  ].join("\n");
  assert.deepEqual([...ui.errorLines(log)], [1, 2, 3, 4, 5, 6, 7, 8, 9]);
  const benign = [
    "test result: ok. 12 passed; 0 failed; 0 ignored",
    "   Compiling error-chain v0.12.4",
    "warning: unused variable `error_code`",
    "src/error.rs:12: note: defined here",
    "No errors found",
    "no error reported",
    "exit code: 0",
    "Finished `test` profile [unoptimized] in 2.1s",
    "assert_failed_count = 3",
  ].join("\n");
  assert.deepEqual([...ui.errorLines(benign)], []);
  for (const value of [null, undefined, 42, "", {}]) assert.deepEqual([...ui.errorLines(value)], []);
  assert.deepEqual([...ui.errorLines("ok\r\nerror: windows line\r\n")], [1]);
  // Bounded: matches stop at the documented cap and very long lines are sampled, never scanned whole.
  assert.equal(ui.errorLines(Array(1500).fill("error: x").join("\n")).length, 1000);
  assert.deepEqual([...ui.errorLines(`${"a".repeat(5000)} error: late`)], []);
});

// r[verify kiln.web.project_page.scenario.untimed]
test("duration trend points use timed terminal runs only and count untimed exclusions", () => {
  const runs = [
    { id: "c", ordinal: 3, outcome: "Success", duration_ms: 3000, completed_at_ms: 30 },
    { id: "a", ordinal: 1, outcome: "DomainFailure", duration_ms: 1000, started_at_ms: 10 },
    { id: "untimed", ordinal: 2, outcome: "Success", duration_ms: null },
    { id: "active", ordinal: 4, outcome: "Pending", duration_ms: null },
    { id: "b", ordinal: 2, outcome: "Success", duration_ms: 0, created_at_ms: 20 },
  ];
  const result = ui.durationPoints(runs);
  assert.deepEqual([...result.points].map((point) => ({ ...point })), [
    { id: "a", duration_ms: 1000, outcome: "DomainFailure", at_ms: 10 },
    { id: "b", duration_ms: 0, outcome: "Success", at_ms: 20 },
    { id: "c", duration_ms: 3000, outcome: "Success", at_ms: 30 },
  ]);
  assert.equal(result.excluded, 1);
  const invalid = [Number.NaN, -1, 1.5, "300", 2 ** 53, undefined].map((duration_ms, ordinal) => ({ id: String(ordinal), ordinal, outcome: "Success", duration_ms }));
  const rejected = ui.durationPoints(invalid);
  assert.equal(rejected.points.length, 0);
  assert.equal(rejected.excluded, invalid.length);
  for (const value of [null, undefined, {}, "runs"]) {
    const empty = ui.durationPoints(value);
    assert.equal(empty.points.length, 0);
    assert.equal(empty.excluded, 0);
  }
  assert.equal(ui.durationPoints([{ id: "q", outcome: null, duration_ms: 5 }, { id: "p", outcome: "Pending", duration_ms: 5 }]).excluded, 0);
});

// r[verify kiln.web.live_progress]
test("live progress reports recorded elapsed time and finished node counts only", () => {
  const now = Date.parse("2026-10-29T12:00:00Z");
  assert.equal(ui.elapsedSince(now - 83_000, now), 83_000);
  assert.equal(ui.formatDuration(ui.elapsedSince(now - 83_000, now)), "1m 23s");
  for (const value of [null, undefined, "", "not a time"]) assert.equal(ui.elapsedSince(value, now), null);
  assert.equal(ui.formatDuration(ui.elapsedSince(null, now)), "Not recorded");
  assert.equal(ui.elapsedSince(now + 5_000, now), 0);
  const nodes = ["Completed", "Failed", "Running", "Pending", "Skipped", "Cancelled", "Unknown", "Paused"].map((state, index) => ({ id: String(index), state }));
  assert.deepEqual({ ...ui.nodeProgress(nodes) }, { complete: 4, total: 8 });
  for (const value of [null, undefined, "nodes", {}]) assert.deepEqual({ ...ui.nodeProgress(value) }, { complete: 0, total: 0 });
  assert.deepEqual({ ...ui.nodeProgress([null, { state: "Completed" }, { state: "completed" }]) }, { complete: 1, total: 2 });
});

// r[verify kiln.web.runs_table]
test("runs group by local calendar day; unrecorded instants have no day", () => {
  const now = new Date(2026, 9, 29, 12, 0).getTime();
  assert.deepEqual({ ...ui.localDay(new Date(2026, 9, 29, 0, 5).getTime(), now) }, { key: "2026-10-29", label: "Today" });
  assert.deepEqual({ ...ui.localDay(new Date(2026, 9, 28, 23, 59).getTime(), now) }, { key: "2026-10-28", label: "Yesterday" });
  const older = ui.localDay(new Date(2026, 9, 20, 8, 0).getTime(), now);
  assert.equal(older.key, "2026-10-20");
  assert.notEqual(older.label, "Today");
  assert.notEqual(older.label, "Yesterday");
  for (const value of [null, undefined, "", "not a time"]) assert.equal(ui.localDay(value, now), null);
});

// r[verify kiln.web.design_system]
test("every reviewed icon is generated, and only reviewed icons are", () => {
  assert.ok(reviewedIcons.length > 0);
  assert.deepEqual(reviewedIcons, [...reviewedIcons].sort(), "icons.list stays sorted");
  assert.deepEqual(Object.keys(icons), reviewedIcons);
  assert.ok(Object.isFrozen(icons));
  for (const name of reviewedIcons) {
    assert.ok(icons[name].length > 0, name);
    for (const [tag, attributes] of icons[name]) {
      assert.ok(["path", "circle", "rect", "line", "polyline", "polygon", "ellipse"].includes(tag), `${name}: ${tag}`);
      assert.ok(Object.keys(attributes).length > 0, `${name}: ${tag} has geometry`);
    }
  }
});

// r[verify kiln.web.design_system]
test("icon() builds a decorative Lucide SVG from the generated data", () => {
  for (const name of reviewedIcons) {
    const svg = ui.icon(name);
    assert.equal(svg.namespaceURI, SVG_NS);
    assert.equal(svg.tagName, "svg");
    assert.deepEqual(Object.fromEntries(svg.attributes), {
      viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", "stroke-width": "2",
      "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true", focusable: "false",
      class: "icon", "data-icon": name,
    });
    // Plain copies: the generated data lives in the vm realm.
    assert.deepEqual(svg.children.map((child) => [child.tagName, Object.fromEntries(child.attributes)]),
      JSON.parse(JSON.stringify(icons[name])));
    assert.ok(svg.children.every((child) => child.namespaceURI === SVG_NS));
  }
  assert.equal(ui.statusIcon("success").getAttribute("data-icon"), "circle-check");
  assert.equal(ui.statusIcon("failure").getAttribute("data-icon"), "circle-x");
  assert.equal(ui.statusIcon("FutureStatus").getAttribute("data-icon"), "circle-question-mark");
  for (const status of ["success", "failure", "running", "queued", "cancelled", "unknown", "warning"]) {
    assert.ok(reviewedIcons.includes(ui.statusIcon(status).getAttribute("data-icon")), status);
  }
});

// r[verify kiln.web.design_system]
test("unknown icon names fail loudly instead of drawing a stand-in", () => {
  for (const name of ["chevron", "close", "success", "", "__proto__", "toString", null, undefined, 42]) {
    assert.throws(() => ui.icon(name), { name: "RangeError", message: `Unknown icon: ${String(name)}` });
  }
});

// r[verify kiln.web.design_system]
test("static placeholders hydrate into icons that keep their classes", () => {
  const replaced = [];
  const placeholder = (name, className) => ({
    dataset: { icon: name },
    classList: className.split(" "),
    replaceWith(node) { replaced.push(node); },
  });
  placeholders.push(placeholder("menu", "icon"), placeholder("chevron-down", "icon account-chevron"));
  ui.hydrateIcons(document);
  assert.deepEqual(replaced.map((svg) => [svg.getAttribute("data-icon"), svg.getAttribute("class")]),
    [["menu", "icon"], ["chevron-down", "icon account-chevron"]]);
  placeholders.push(placeholder("not-an-icon", "icon"));
  assert.throws(() => ui.hydrateIcons(document), { name: "RangeError", message: "Unknown icon: not-an-icon" });
});

// r[verify kiln.web.design_system]
test("every icon the console names is in the reviewed list", () => {
  const sources = ["web.html", "web.js", "ui.js", "work.js", "work-interaction.js", "code-review.js", "project.js"];
  const used = new Set();
  for (const name of sources) {
    const text = asset(name);
    if (name === "web.html") for (const match of text.matchAll(/data-icon="([^"]+)"/g)) used.add(match[1]);
    for (const match of text.matchAll(/\bicon\("([^"]+)"\)/g)) used.add(match[1]);
    for (const match of text.matchAll(/iconButton\("[^"]*", "([^"]+)"/g)) used.add(match[1]);
    for (const match of text.matchAll(/disclosureSummary\([^)]*?, "([^"]+)"\)/g)) used.add(match[1]);
  }
  assert.ok(used.size > 20);
  assert.deepEqual([...used].filter((name) => !reviewedIcons.includes(name)), []);
  assert.doesNotMatch(asset("web.html"), /<svg/);
});
