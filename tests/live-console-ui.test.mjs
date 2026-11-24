import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
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
vm.runInNewContext(asset("source.js"), context);
vm.runInNewContext(asset("receipts.js"), context);
vm.runInNewContext(asset("workers.js"), context);
vm.runInNewContext(asset("runbooks.js"), context);
vm.runInNewContext(asset("cairn.js"), context);
const ui = context.window.KilnUi;
const icons = context.window.KilnIcons;
// Review-query route normalization must mirror the service's trimmed, bounded,
// control-free agent text admission.
test("agent route text matches review-query admission", () => {
  assert.equal(ui.agentText("  omp  "), "omp");
  assert.equal(ui.agentText(""), "");
  assert.equal(ui.agentText(null), "");
  assert.equal(ui.agentText("o\u0007mp"), "");
  assert.equal(ui.agentText("o\u0085mp"), "");
  assert.equal(ui.agentText("x".repeat(128)), "x".repeat(128));
  assert.equal(ui.agentText("x".repeat(129)), "");
  // 64 two-byte characters are exactly 128 bytes; one more exceeds the byte bound.
  assert.equal(ui.agentText("é".repeat(64)), "é".repeat(64));
  assert.equal(ui.agentText("é".repeat(65)), "");
  assert.equal(ui.agentText("😀".repeat(32)), "😀".repeat(32));
  assert.equal(ui.agentText("😀".repeat(33)), "");
  assert.equal(ui.utf8Bytes("é"), 2);
  assert.equal(ui.utf8Bytes("😀"), 4);
});
const source = context.window.KilnSource;
// Plain copies: values built inside the vm realm carry that realm's prototypes.
const plain = (value) => JSON.parse(JSON.stringify(value));
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

// r[verify kiln.web.live_activity]
// r[verify kiln.web.live_activity.scenario.count]
test("live activity counts only runs the snapshot reports as running", () => {
  const running = { id: "a", outcome: "Pending", phase: "active" };
  const snapshot = [
    running, { id: "b", outcome: "Pending", phase: "active" },
    { id: "c", outcome: "Pending", phase: "queued" }, { id: "d", outcome: "Pending", phase: "awaiting_approval" },
    { id: "e", outcome: "Pending", phase: "unknown" }, { id: "f", outcome: "Pending" }, { id: "g", outcome: null, phase: "queued" },
    { id: "h", outcome: "Success", phase: "completed" }, { id: "i", outcome: "DomainFailure", phase: "completed" },
    { id: "j", outcome: "Cancelled", phase: "completed" }, { id: "k", outcome: "Unknown", phase: "completed" }, { id: "l", outcome: "FutureOutcome", phase: "active" },
  ];
  assert.deepEqual({ ...ui.liveActivity(snapshot) }, { running: 2 });
  // The count is the status rule's: every counted run is one the console labels Running, and no other is.
  assert.deepEqual(snapshot.filter((run) => ui.outcomeStatus(run.outcome, run.phase).status === "running").map((run) => run.id), ["a", "b"]);
  assert.deepEqual({ ...ui.liveActivity(snapshot.filter((run) => run.phase !== "active")) }, { running: 0 });
  // A finished run whose phase still reads active is not running: the outcome decides.
  assert.deepEqual({ ...ui.liveActivity([{ outcome: "Success", phase: "active" }, { outcome: "TimedOut", phase: "active" }]) }, { running: 0 });
  assert.deepEqual({ ...ui.liveActivity([]) }, { running: 0 });
  // Not a snapshot list, or entries that are not runs: nothing is counted and nothing throws.
  for (const value of [null, undefined, "runs", 3, {}, { length: 2, 0: running, 1: running }]) assert.deepEqual({ ...ui.liveActivity(value) }, { running: 0 });
  assert.deepEqual({ ...ui.liveActivity([null, undefined, "Pending", 7, [], {}, running]) }, { running: 1 });
});

// r[verify kiln.web.live_activity.scenario.stale]
test("a running count is presented only while live observation is current", () => {
  const admitted = ui.liveActivity([{ outcome: "Pending", phase: "active" }, { outcome: "Pending", phase: "active" }, { outcome: "Pending", phase: "queued" }]).running;
  assert.equal(ui.presentedRunning(admitted, true), 2);
  // Stale or disconnected observation presents none; the admitted count itself is kept by the caller, so a
  // recovered connection presents it again without a new snapshot.
  assert.equal(ui.presentedRunning(admitted, false), 0);
  assert.equal(ui.presentedRunning(admitted, true), 2);
  // No snapshot admitted yet (null) and nothing running present none even while current.
  assert.equal(ui.presentedRunning(null, true), 0);
  assert.equal(ui.presentedRunning(0, true), 0);
  // Only the literal `true` states current observation: a status message or any other truthy value does not.
  for (const current of [undefined, null, 0, 1, "Live · authenticated", {}, []]) assert.equal(ui.presentedRunning(admitted, current), 0);
  // A value that is not a count is never presented as one.
  for (const value of [undefined, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 2 ** 53, "2", true, {}, [2]]) assert.equal(ui.presentedRunning(value, true), 0);
});

// r[verify kiln.web.readable_observations.scenario.pending]
test("pending timing is named from the recorded phase; finished or unreadable runs never borrow it", () => {
  const timing = (run) => ({ ...ui.runTiming(run) });
  const started = 1_700_000_000_000;
  assert.deepEqual(timing({ outcome: "Pending", phase: "active", started_at_ms: started }), { duration: "in_progress", started: "recorded", completed: "not_finished" });
  for (const phase of ["queued", "awaiting_approval"]) assert.deepEqual(timing({ outcome: "Pending", phase }), { duration: "not_started", started: "not_started", completed: "not_finished" }, phase);
  assert.deepEqual(timing({ outcome: null, phase: "queued" }), { duration: "not_started", started: "not_started", completed: "not_finished" });
  // A recorded value always wins, even on a run that is still pending.
  assert.deepEqual(timing({ outcome: "Pending", phase: "active", started_at_ms: started, duration_ms: 0 }), { duration: "recorded", started: "recorded", completed: "not_finished" });
  assert.deepEqual(timing({ outcome: "Success", started_at_ms: started, completed_at_ms: started + 5, duration_ms: 5 }), { duration: "recorded", started: "recorded", completed: "recorded" });
  // Finished, unknown and unreadable runs with absent or invalid timing read as not recorded.
  for (const outcome of ["DomainFailure", "Cancelled", "Unknown", "SomethingNew"]) assert.deepEqual(timing({ outcome }), { duration: "not_recorded", started: "not_recorded", completed: "not_recorded" }, outcome);
  for (const duration_ms of [-1, 1.5, "273357", Number.NaN, 2 ** 53]) {
    assert.equal(timing({ outcome: "Success", duration_ms }).duration, "not_recorded", String(duration_ms));
    assert.equal(timing({ outcome: "Pending", phase: "active", duration_ms }).duration, "in_progress", String(duration_ms));
  }
  for (const run of [null, undefined, "run", 3, []]) assert.deepEqual(timing(run), { duration: "not_recorded", started: "not_recorded", completed: "not_recorded" }, String(run));
});

// r[verify kiln.web.wayfinding]
// r[verify kiln.web.wayfinding.scenario.register]
test("every destination has one section number in an existing group, and a nested page names an existing parent", () => {
  const places = [...ui.destinations];
  assert.ok(places.length > 10);
  assert.equal(new Set(places.map((place) => place.page)).size, places.length, "one entry per page");
  const numbered = places.filter((place) => !place.parent);
  assert.equal(new Set(numbered.map((place) => place.number)).size, numbered.length, "no two destinations share a number");
  for (const place of places) {
    assert.match(place.number, /^\d+\.\d+$/, place.page);
    assert.equal(ui.destinationGroup(place.group)?.number, place.number.split(".")[0], place.page);
    assert.ok(place.title && place.purpose, place.page);
    if (place.parent) {
      const parent = ui.destination(place.parent);
      assert.ok(parent && !parent.parent, `${place.page} nests under a top-level destination`);
      assert.equal(parent.number, place.number, `${place.page} shares its parent's number`);
    }
  }
  assert.equal(ui.destination("project").parent, "projects");
  for (const value of ["", "nope", "__proto__", "toString", null, undefined, 3]) assert.equal(ui.destination(value), null, String(value));
  assert.equal(ui.destinationGroup("9"), null);
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
  const sources = ["web.html", "web.js", "ui.js", "work.js", "work-board.js", "work-interaction.js", "code-review.js", "project.js", "source.js", "receipts.js", "workers.js", "runbooks.js"];
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
  // The Source page's icons are reviewed, generated and named literally where they are drawn.
  for (const name of ["copy", "corner-left-up", "download", "file", "file-diff", "file-minus", "file-pen", "file-plus", "folder", "git-commit-horizontal", "link", "pin", "text-wrap"]) {
    assert.ok(used.has(name), name);
    assert.ok(Object.hasOwn(icons, name), name);
  }
});

// r[verify kiln.web.readable_observations]
test("byte sizes read in binary units; unknown or inexact sizes stay blank", () => {
  const cases = [[0, "0 B"], [1, "1 B"], [1023, "1023 B"], [1024, "1.0 KiB"], [1536, "1.5 KiB"], [10 * 1024 - 1, "10 KiB"], [10 * 1024, "10 KiB"], [74364, "73 KiB"], [1048575, "1.0 MiB"], [5.25 * 1048576, "5.3 MiB"], [3 * 1024 ** 3, "3.0 GiB"]];
  for (const [bytes, label] of cases) assert.equal(ui.humanBytes(bytes), label, String(bytes));
  for (const value of [null, undefined, -1, 1.5, Number.NaN, Infinity, "12", 2 ** 53, {}]) assert.equal(ui.humanBytes(value), "", String(value));
});

// r[verify kiln.dashboard.source.navigation]
test("source entries list directories first, then case-insensitively by name, without reordering the input", () => {
  const entries = [
    { name: "web.js", kind: "file" }, { name: "src", kind: "directory" }, { name: "README.md", kind: "file" },
    { name: "Assets", kind: "directory" }, { name: "link", kind: "symlink" }, { name: "b.rs", kind: "file" }, { name: "B.rs", kind: "file" }, { name: "assets", kind: "directory" },
  ];
  const before = plain(entries);
  assert.deepEqual(plain(source.sortEntries(entries)).map((entry) => entry.name), ["Assets", "assets", "src", "B.rs", "b.rs", "link", "README.md", "web.js"]);
  assert.deepEqual(plain(entries), before);
  for (const value of [null, undefined, "src", {}]) assert.deepEqual(plain(source.sortEntries(value)), []);
});

// r[verify kiln.dashboard.source.navigation.scenario.anchor]
test("line selections accept a line or a range and report garbage or lines past the end", () => {
  assert.deepEqual(plain(source.parseLineSelection("12", 100)), { state: "ok", start: 12, end: 12 });
  assert.deepEqual(plain(source.parseLineSelection("12-20", 100)), { state: "ok", start: 12, end: 20 });
  assert.deepEqual(plain(source.parseLineSelection("20-12", 100)), { state: "ok", start: 12, end: 20 }, "a reversed range is the same range");
  assert.deepEqual(plain(source.parseLineSelection("100", 100)), { state: "ok", start: 100, end: 100 });
  assert.deepEqual(plain(source.parseLineSelection("5000")), { state: "ok", start: 5000, end: 5000 }, "without a line count only the form is checked");
  assert.deepEqual(plain(source.parseLineSelection("101", 100)), { state: "outside", start: 101, end: 101, lines: 100 });
  assert.deepEqual(plain(source.parseLineSelection("90-120", 100)), { state: "outside", start: 90, end: 120, lines: 100 }, "a range is never clamped onto existing lines");
  assert.deepEqual(plain(source.parseLineSelection("1", 0)), { state: "outside", start: 1, end: 1, lines: 0 });
  for (const value of ["", null, undefined, 12]) assert.deepEqual(plain(source.parseLineSelection(value, 100)), { state: "none" });
  for (const value of ["abc", "0", "012", "12-", "-3", "1-2-3", "1.5", " 12", "12 ", "L12", "1e3", "12-0", "1234567890"]) {
    assert.deepEqual(plain(source.parseLineSelection(value, 100)), { state: "invalid", value }, value);
  }
});

// r[verify kiln.dashboard.source.navigation]
test("retained text splits into lines that rejoin to the exact text", () => {
  for (const text of ["", "a", "a\n", "a\nb", "a\nb\n", "\n", "\n\n", "x\r\ny\r\n", "tab\there\n\u{1F600}\n"]) {
    const { lines, finalNewline } = source.splitLines(text);
    assert.equal(lines.join("\n") + (finalNewline ? "\n" : ""), text, JSON.stringify(text));
  }
  assert.equal(source.splitLines("a\nb\n").lines.length, 2);
  assert.equal(source.splitLines("a\nb").finalNewline, false);
  assert.deepEqual(plain(source.splitLines(null)), { lines: [], finalNewline: false });
});

// Server highlighting (kiln.highlight.v1) as the service returns it.
const served = (lines, extra = {}) => ({ schema: "kiln.highlight.v1", language: "rust", status: "highlighted", reason: null, lines, ...extra });
const rejoined = (fragments) => fragments.map((fragment) => fragment.text).join("");

// r[verify kiln.highlight.render]
test("server tokens render as fragments that rejoin to each line, addressed in UTF-16 code units", () => {
  // "\u{1F600}" is one astral character: two UTF-16 code units before the string's closing quote.
  const line = "const x = \"\u{1F600}a\"; // \u{1F600}";
  const result = ui.tokenFragments(line, [[0, 5, "keyword"], [10, 15, "string"], [17, 22, "comment"]]);
  assert.deepEqual(plain(result.fragments), [
    { text: "const", kind: "keyword" }, { text: " x = ", kind: null }, { text: "\"\u{1F600}a\"", kind: "string" }, { text: "; ", kind: null }, { text: "// \u{1F600}", kind: "comment" },
  ]);
  assert.equal(rejoined(result.fragments), line);
  const lines = ["fn main() {", "", "    let é = 1; // naïve", "}"];
  const plan = ui.highlightLines(served([[[0, 2, "keyword"], [3, 7, "function"], [7, 8, "punctuation"], [8, 9, "punctuation"]], [], [[4, 7, "keyword"], [12, 13, "number"], [15, 23, "comment"]], [[0, 1, "punctuation"]]]), lines);
  assert.equal(plan.state, "highlighted");
  assert.equal(plan.note, null);
  assert.equal(plan.language, "rust");
  assert.deepEqual(plain(plan.fragments.map(rejoined)), lines);
  assert.deepEqual(plain(plan.fragments[2].filter((fragment) => fragment.kind)), [{ text: "let", kind: "keyword" }, { text: "1", kind: "number" }, { text: "// naïve", kind: "comment" }]);
  assert.deepEqual(plain(plan.fragments[1]), [], "an empty line has no fragments");
  // Diff lines: tokens include the one-unit marker; the code cell starts after it.
  const added = "+let \u{1F600} = 1;";
  const diffPlan = ui.highlightLines(served([[], [[1, 4, "keyword"], [10, 11, "number"]], [[0, 4, "keyword"]]], { language: null }), ["@@ -1 +1 @@", added, "-let"]);
  assert.equal(diffPlan.state, "highlighted");
  assert.equal(rejoined(ui.fragmentsFrom(diffPlan.fragments[1], 1)), added.slice(1));
  assert.deepEqual(plain(ui.fragmentsFrom(diffPlan.fragments[1], 1)[0]), { text: "let", kind: "keyword" });
  assert.deepEqual(plain(ui.fragmentsFrom(diffPlan.fragments[2], 1)), [{ text: "let", kind: "keyword" }], "a token over the marker is cut at it");
  assert.deepEqual(plain(ui.highlightLines(served([]), []).fragments), []);
  assert.equal(ui.languageLabel("rust"), "Rust");
  assert.equal(ui.languageLabel(null), "Plain text");
});

// r[verify kiln.highlight.render]
test("malformed server tokens fall back to plain text with one explicit note", () => {
  const line = "let a = \"\u{1F600}\";";
  const malformed = {
    overlap: [[0, 3, "keyword"], [2, 5, "variable"]],
    unsorted: [[4, 5, "variable"], [0, 3, "keyword"]],
    "past the end": [[0, line.length + 1, "string"]],
    negative: [[-1, 3, "keyword"]],
    empty: [[2, 2, "keyword"]],
    reversed: [[3, 1, "keyword"]],
    fractional: [[0, 1.5, "keyword"]],
    "string offset": [["0", 3, "keyword"]],
    "unknown class": [[0, 3, "bogus"]],
    "inherited name": [[0, 3, "constructor"]],
    "missing class": [[0, 3]],
    "object token": [{ start: 0, end: 3, class: "keyword" }],
    "split astral character": [[8, 10, "string"]],
    "not a list": "keyword",
  };
  for (const [name, tokens] of Object.entries(malformed)) {
    assert.ok(typeof ui.tokenFragments(line, tokens).problem === "string", name);
    const plan = ui.highlightLines(served([[], tokens]), ["", line]);
    assert.equal(plan.state, "invalid", name);
    assert.equal(plan.fragments, null, name);
    assert.match(plan.note, /^Highlighting unavailable: on line 2, .+\.$/, name);
  }
  assert.equal(ui.tokenFragments(line, [[8, 11, "string"]]).problem, undefined, "a whole astral character is a valid span");
  const responses = {
    "missing highlight": undefined,
    "wrong schema": served([[]], { schema: "kiln.highlight.v2" }),
    "unknown status": served([[]], { status: "partial" }),
    "too few token lines": served([]),
    "too many token lines": served([[], []]),
    "lines not a list": served(null),
    "malformed language": served([[]], { language: 7 }),
  };
  for (const [name, highlight] of Object.entries(responses)) {
    const plan = ui.highlightLines(highlight, [line]);
    assert.equal(plan.state, "invalid", name);
    assert.equal(plan.fragments, null, name);
    assert.match(plan.note, /^Highlighting unavailable: .+\.$/, name);
  }
});

// r[verify kiln.highlight.render]
test("skipped highlighting renders plain and names the reason", () => {
  const reasons = { unsupported_language: "language not supported", binary: "binary content", too_large: "larger than the highlighting bound", parse_error: "the grammar could not parse it", truncated_input: "the input was truncated" };
  for (const [reason, label] of Object.entries(reasons)) {
    const plan = ui.highlightLines(served([], { status: "skipped", reason, language: null }), ["plain text"]);
    assert.deepEqual(plain(plan), { state: "skipped", language: null, note: `Not highlighted: ${label}.`, fragments: null });
  }
  assert.equal(ui.highlightLines(served([], { status: "skipped", reason: "too_large" }), ["x"]).language, "rust", "a skipped file still names its language");
  assert.equal(ui.highlightLines(served([[]], { status: "skipped", reason: "too_large" }), ["x"]).state, "invalid", "a skip must carry no tokens");
  assert.equal(ui.highlightLines(served([], { status: "skipped", reason: "tired" }), ["x"]).state, "invalid", "an unknown reason is not invented");
  assert.equal(ui.highlightLines(served([], { status: "skipped", reason: null }), ["x"]).state, "invalid");
});

// r[verify kiln.dashboard.source.navigation.scenario.changes]
test("unified diffs parse per file with status, counts and old/new numbers, keeping every line", () => {
  const diff = [
    "diff --git a/src/lib.rs b/src/lib.rs",
    "index 1111111..2222222 100644",
    "--- a/src/lib.rs",
    "+++ b/src/lib.rs",
    "@@ -1,3 +1,4 @@ fn top()",
    " keep",
    "-old",
    "+new",
    "+added",
    " tail",
    "@@ -10 +11 @@",
    "-x",
    "+y",
    "diff --git a/docs/new.md b/docs/new.md",
    "new file mode 100644",
    "index 0000000..3333333",
    "--- /dev/null",
    "+++ b/docs/new.md",
    "@@ -0,0 +1,2 @@",
    "+# Title",
    "+body",
    "\\ No newline at end of file",
    "diff --git a/gone.txt b/gone.txt",
    "deleted file mode 100644",
    "index 4444444..0000000",
    "--- a/gone.txt",
    "+++ /dev/null",
    "@@ -1,2 +0,0 @@",
    "-one",
    "-two",
    "diff --git a/old name.txt b/new name.txt",
    "similarity index 90%",
    "rename from old name.txt",
    "rename to new name.txt",
    "index 5555555..6666666 100644",
    "--- a/old name.txt",
    "+++ b/new name.txt",
    "@@ -2,2 +2,2 @@",
    " same",
    "?? not a diff line",
    "-before",
    "+after",
    "trailing garbage after the hunk",
    "diff --git a/img.png b/img.png",
    "index 7777777..8888888 100644",
    "Binary files a/img.png and b/img.png differ",
    "diff --git a/logo.png b/logo.png",
    "new file mode 100644",
    "index 0000000..9999999",
    "Binary files /dev/null and b/logo.png differ",
    "diff --git a/cut.rs b/cut.rs",
    "--- a/cut.rs",
    "+++ b/cut.rs",
    "@@ -1,5 +1,5 @@",
    " a",
    "-b",
  ].join("\n") + "\n";
  const parsed = source.parseDiff(diff, { truncated: true });
  const files = plain(parsed.files);
  assert.deepEqual(files.map((file) => [file.path, file.status, file.binary, file.additions, file.deletions, file.truncated]), [
    ["src/lib.rs", "modified", false, 3, 2, false],
    ["docs/new.md", "added", false, 2, 0, false],
    ["gone.txt", "deleted", false, 0, 2, false],
    ["new name.txt", "renamed", false, 1, 1, false],
    ["img.png", "binary", true, 0, 0, false],
    ["logo.png", "added", true, 0, 0, false],
    ["cut.rs", "modified", false, 0, 1, true],
  ]);
  assert.deepEqual([files[2].oldPath, files[2].newPath], ["gone.txt", null]);
  assert.deepEqual([files[3].oldPath, files[3].newPath], ["old name.txt", "new name.txt"]);
  assert.deepEqual([files[1].oldPath, files[5].oldPath], [null, null]);
  const rows = (file) => file.hunks.flatMap((hunk) => hunk.lines.map((line) => [line.kind, line.oldLine, line.newLine, line.text]));
  assert.deepEqual(rows(files[0]), [
    ["context", 1, 1, " keep"], ["delete", 2, null, "-old"], ["add", null, 2, "+new"], ["add", null, 3, "+added"], ["context", 3, 4, " tail"],
    ["delete", 10, null, "-x"], ["add", null, 11, "+y"],
  ]);
  assert.deepEqual(files[0].hunks.map((hunk) => [hunk.oldStart, hunk.oldCount, hunk.newStart, hunk.newCount, hunk.section]), [[1, 3, 1, 4, "fn top()"], [10, 1, 11, 1, ""]]);
  assert.deepEqual(rows(files[1]).at(-1), ["no-newline", null, null, "\\ No newline at end of file"]);
  assert.deepEqual(rows(files[2]), [["delete", 1, null, "-one"], ["delete", 2, null, "-two"]]);
  assert.deepEqual(rows(files[3]), [
    ["context", 2, 2, " same"], ["context", null, null, "?? not a diff line"], ["delete", 3, null, "-before"], ["add", null, 3, "+after"], ["context", null, null, "trailing garbage after the hunk"],
  ]);
  assert.ok(files[3].hunks[0].lines[1].unparsed && files[3].hunks[0].lines[4].unparsed);
  assert.deepEqual(files[4].hunks, []);
  assert.ok(files[4].headers.includes("Binary files a/img.png and b/img.png differ"));
  // Every input line survives, in order, as a header, a hunk header or a hunk line.
  const rebuilt = [...parsed.preamble, ...parsed.files.flatMap((file) => [...file.headers, ...file.hunks.flatMap((hunk) => [hunk.header, ...hunk.lines.map((line) => line.text)])])];
  assert.equal(rebuilt.join("\n") + (parsed.finalNewline ? "\n" : ""), diff);
  // Each hunk line names its line in the diff text, which server highlighting addresses.
  const diffLines = source.splitLines(diff).lines;
  for (const file of parsed.files) for (const hunk of file.hunks) for (const line of hunk.lines) assert.equal(diffLines[line.index], line.text);
  // A diff that is not truncated marks no file incomplete.
  assert.ok(plain(source.parseDiff(diff).files).every((file) => !file.truncated));
});

// r[verify kiln.dashboard.source.navigation.scenario.changes]
test("empty, foreign or headerless diff text is kept rather than invented", () => {
  for (const value of ["", null, undefined, 42]) assert.deepEqual(plain(source.parseDiff(value)), { files: [], preamble: [], finalNewline: false });
  const plainDiff = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b";
  const parsed = plain(source.parseDiff(plainDiff, { truncated: true }));
  assert.deepEqual(parsed.files, [], "without a diff --git header no file is guessed");
  assert.equal(parsed.preamble.join("\n"), plainDiff);
  assert.equal(parsed.finalNewline, false);
  const malformed = plain(source.parseDiff("diff --git a/x b/x\n@@ nonsense @@\n+a\n").files[0]);
  assert.deepEqual(malformed.hunks, [], "an unreadable hunk header starts no hunk");
  assert.deepEqual(malformed.headers, ["diff --git a/x b/x", "@@ nonsense @@", "+a"]);
  assert.equal(malformed.additions, 0);
  const spaced = plain(source.parseDiff("diff --git a/a b/c b/a b/c\nindex 1..2 100644\n").files[0]);
  assert.deepEqual([spaced.oldPath, spaced.newPath, spaced.path], ["a b/c", "a b/c", "a b/c"], "equal halves resolve a path that contains ' b/'");
});

// Kanban board helpers: pure, loaded in their own context exactly as served.
const boardContext = { window: {}, URLSearchParams };
vm.runInNewContext(asset("work-board.js"), boardContext);
const board = boardContext.window.KilnWorkBoard;
const boardFixture = JSON.parse(readFileSync(new URL("../crates/kiln-web-service/tests/fixtures/work-board/eligibility.json", import.meta.url), "utf8"));

// r[verify kiln.work.kanban.projection]
test("list and board eligibility follow the fixture shared with the Rust projection", () => {
  for (const item of boardFixture.cases) {
    const shown = boardFixture.records.filter((record) => board.listed(record, item.filters, item.view_active));
    assert.deepEqual(shown.filter((record) => record.kind === "issue").map((record) => record.id), item.issues, item.name);
    assert.equal(shown.filter((record) => record.kind !== "issue").length, item.list_only, item.name);
  }
});

test("board reads carry only the Work page's overrides and an admitted sort", () => {
  assert.equal(board.boardPath("", {}), "/api/work/board");
  const route = { work_status: "review", work_q: " needle case ", work_kind: "", work_sort: "due", work_layout: "board", work_group: "team" };
  assert.equal(board.boardPath("ab".repeat(20), route), `/api/work/board?view=${"ab".repeat(20)}&status=review&q=needle+case&sort=due`);
  assert.equal(board.boardPath("", { work_sort: "random" }), "/api/work/board", "an unknown sort is not forwarded");
});

const columns = [
  { status: "backlog", builtin: true, teams: [], retired: false },
  { status: "review", builtin: false, teams: ["team-a"], retired: false },
  { status: "gone", builtin: false, teams: [], retired: true },
  { status: "completed", builtin: true, teams: [], retired: false },
  { status: "snoozed", builtin: true, teams: [], retired: false },
  { status: "duplicate", builtin: true, teams: [], retired: false },
];
const card = { id: "issue-1", head: "head-1", title: "Fix", status: "backlog", team: "team-a", due_at_ms: null };

// r[verify kiln.work.kanban.move]
test("move targets follow the issue's team workflow and name every refusal", () => {
  const now = 1_000;
  const reasons = (subject) => Object.fromEntries(plain(board.moveTargets(subject, columns, now)).map((target) => [target.status, target.allowed ? "allowed" : target.reason]));
  assert.deepEqual(reasons(card), {
    backlog: "Current column", review: "allowed", gone: "Retired: no team workflow defines this status", completed: "allowed",
    snoozed: "Needs a future snooze date: use Edit…", duplicate: "Needs a duplicate target: use Edit…",
  });
  assert.equal(reasons({ ...card, team: "team-b" }).review, "Not in this issue's team workflow");
  assert.equal(reasons({ ...card, team: null }).review, "Custom status: the issue has no team");
  assert.equal(reasons({ ...card, due_at_ms: now + 1 }).snoozed, "allowed");
  assert.equal(reasons({ ...card, due_at_ms: now }).snoozed, "Needs a future snooze date: use Edit…");
  // The default target is the next allowed column to the right, else the first allowed one.
  assert.equal(board.defaultTarget(card, columns, now), "review");
  assert.equal(board.defaultTarget({ ...card, status: "completed" }, columns, now), "backlog");
  assert.equal(board.defaultTarget({ ...card, status: "review", team: "team-b" }, [columns[1]], now), "");
});

// r[verify kiln.work.kanban.move.scenario.retry] r[verify kiln.work.kanban.interaction]
test("a move names its exact head and only unknown outcomes are retryable", () => {
  assert.deepEqual(plain(board.moveCommand(card, "review", "request-1")), { request_id: "request-1", object_id: "issue-1", expected_head: "head-1", action: { type: "set_status", status: "review" } });
  const failure = (status, message) => Object.assign(new Error(message), { status });
  assert.equal(board.classify(failure(409, "work_head_conflict: Request conflicts")), "conflict");
  assert.equal(board.classify(failure(409, "work_request_conflict: Request conflicts")), "replay");
  assert.equal(board.classify(failure(400, "work_command_invalid: x")), "invalid");
  assert.equal(board.classify(failure(403, "work_write_denied: x")), "denied");
  assert.equal(board.classify(failure(404, "work_object_missing: x")), "missing");
  for (const unknown of [failure(503, "work_store_unavailable: x"), new TypeError("Failed to fetch"), Object.assign(new Error("Request timed out"), { name: "TimeoutError" }), new Error("No matching durable work record was returned.")]) assert.equal(board.classify(unknown), "unknown");
  assert.equal(board.classify(Object.assign(new Error("Signed out"), { name: "AbortError" })), "aborted");
});

// r[verify kiln.work.kanban.move.scenario.stale]
test("a reloaded conflict is resolved by the user, never by overwriting", () => {
  const attempt = { card, target: "review" };
  assert.equal(board.resolve(attempt, null).kind, "missing");
  assert.equal(board.resolve(attempt, { ...card, head: "head-2", status: "review" }).kind, "applied");
  assert.equal(board.resolve(attempt, { ...card }).kind, "unchanged");
  const changed = plain(board.resolve(attempt, { ...card, head: "head-2", status: "completed" }));
  assert.deepEqual([changed.kind, changed.card.head, changed.card.status], ["changed", "head-2", "completed"]);
});

const projection = (overrides = {}) => ({
  schema: "kiln.work.board.v1", principal: "local:alice", bounded: true, complete: true, filtered: false,
  matched_issues: 2, shown_cards: 2, list_only: 0, snapshot_objects: 9, object_limit: 1024, card_limit: 100, columns_truncated: 0,
  columns: [{ ...columns[0], total: 2, cards: [card, { ...card, id: "issue-2", head: "head-2" }] }],
  other: { statuses: [], total: 0, cards: [] }, ...overrides,
});

// r[verify kiln.work.kanban.projection]
test("board projections are admitted only for their principal and state their completeness", () => {
  const value = projection();
  assert.equal(board.admit(value, "local:alice"), value);
  assert.equal(board.findCard(value, "issue-2").head, "head-2");
  assert.equal(board.findCard(value, "issue-9"), null);
  for (const invalid of [projection({ principal: "local:bob" }), projection({ bounded: false }), projection({ matched_issues: -1 }), projection({ columns: [{ ...columns[0], total: 1, cards: [{ ...card, head: "" }] }] }), projection({ other: null })]) {
    assert.throws(() => board.admit(invalid, "local:alice"), /invalid or belongs to a different principal/);
  }
  const whole = plain(board.completeness(value));
  assert.equal(whole.summary, "2 matching issues, all shown");
  assert.deepEqual(whole.notes, ["Bounded snapshot: 9 of at most 1024 work objects."]);
  const partial = plain(board.completeness(projection({ complete: false, matched_issues: 107, shown_cards: 105, filtered: true, list_only: 1, columns_truncated: 3 })));
  assert.equal(partial.summary, "105 of 107 matching issues shown");
  assert.deepEqual(partial.notes, [
    "Columns show at most 100 cards each; 2 matching issue(s) are not shown. Narrow the filters or use the list layout.",
    "Filtered: issues outside this view's filters are not on this board.",
    "1 matching record is not an issue and appears only in the list layout.",
    "3 team statuses exceed the column bound; their issues are under Other statuses.",
    "Bounded snapshot: 9 of at most 1024 work objects.",
  ]);
});

// Declared authorship: the console sends only validated self-declarations and labels them as unverified.
const reviewContext = { window: {} };
vm.runInNewContext(asset("ui.js"), reviewContext);
vm.runInNewContext(asset("code-review.js"), reviewContext);
const reviewAuthorship = reviewContext.window.KilnCodeReview.authorship;
assert.equal(reviewAuthorship.limit, ui.agentTextLimit);

// r[verify kiln.review.authorship.console]
test("authorship payloads mirror the service bounds and send nothing when undeclared", () => {
  const payload = (...args) => plain(reviewAuthorship.payload(...args));
  assert.equal(reviewAuthorship.payload("undeclared", "ignored", "ignored"), null);
  assert.equal(reviewAuthorship.payload("", "", ""), null);
  assert.deepEqual(payload("human", "ignored"), { kind: "human" });
  assert.deepEqual(payload("mixed"), { kind: "mixed" });
  assert.deepEqual(payload("agent", "  omp ", " claude "), { kind: "agent", harness: "omp", model: "claude" });
  assert.deepEqual(payload("agent", "omp", "   "), { kind: "agent", harness: "omp" });
  const bounded = "h".repeat(reviewAuthorship.limit);
  assert.deepEqual(payload("agent", bounded, bounded), { kind: "agent", harness: bounded, model: bounded });
  // 64 two-byte characters are exactly the byte bound; one more exceeds it although it is only 65 characters.
  assert.equal(payload("agent", "é".repeat(64)).harness.length, 64);
  for (const [args, pattern] of [
    [["agent", "", "claude"], /Harness is required/],
    [["agent", "   "], /Harness is required/],
    [["agent", "om\u0007p"], /Harness must not contain control characters/],
    [["agent", "h".repeat(reviewAuthorship.limit + 1)], /Harness must be at most 128 bytes/],
    [["agent", "é".repeat(65)], /Harness must be at most 128 bytes/],
    [["agent", "omp", "m".repeat(reviewAuthorship.limit + 1)], /Model must be at most 128 bytes/],
    [["agent", "omp", "cl\nude"], /Model must not contain control characters/],
    [["robot"], /Select a listed authorship/],
  ]) {
    assert.throws(() => reviewAuthorship.payload(...args), pattern);
  }
});

// r[verify kiln.review.authorship.console]
test("change detail labels declared authorship as self-declared and absent as undeclared", () => {
  assert.equal(reviewAuthorship.classLabel("agent"), "Agent (self-declared)");
  assert.equal(reviewAuthorship.classLabel("mixed"), "Mixed (self-declared)");
  assert.equal(reviewAuthorship.classLabel(undefined), "Undeclared");
  assert.equal(reviewAuthorship.label(undefined), "Undeclared");
  assert.equal(reviewAuthorship.label({ kind: "human" }), "Human");
  assert.equal(reviewAuthorship.label({ kind: "agent", harness: "omp", model: "claude" }), "Agent · harness omp · model claude");
  assert.equal(reviewAuthorship.label({ kind: "agent", harness: "omp" }), "Agent · harness omp");
});

// r[verify kiln.review.agent.search]
test("saved review queries preserve every agent filter and omit absent fields", () => {
  const query = reviewContext.window.KilnCodeReview.savedQuery;
  assert.deepEqual(plain(query({})), { kind: "all", text: "", status: null });
  assert.deepEqual(plain(query({
    review_view: "awaiting_review", review_q: "fix", review_status: "open",
    review_advisory: "without", review_agent: "with",
    review_agent_harness: "omp", review_agent_model: "deepseek",
  })), {
    kind: "awaiting_review", text: "fix", status: "open", advisory: false,
    agent: true, agent_harness: "omp", agent_model: "deepseek",
  });
  assert.deepEqual(plain(query({ review_agent: "without", review_agent_harness: "omp" })), {
    kind: "all", text: "", status: null, agent: false, agent_harness: "omp",
  });
  assert.deepEqual(plain(query({ review_agent_harness: " omp ", review_agent_model: "deepseek\u0007" })), {
    kind: "all", text: "", status: null, agent_harness: "omp",
  });
});

// r[verify kiln.review.agent.search]
test("saved review views normalize every agent filter when applied", () => {
  const savedView = reviewContext.window.KilnCodeReview.savedView;
  assert.deepEqual(plain(savedView({ query: {} })), {
    review_q: "", review_view: "all", review_status: "",
    review_advisory: "", review_agent: "", review_agent_harness: "", review_agent_model: "",
  });
  assert.deepEqual(plain(savedView({ query: {
    kind: "awaiting_review", text: "fix", status: "open",
    advisory: false, agent: false, agent_harness: " omp ", agent_model: "deepseek\u0085",
  } })), {
    review_q: "fix", review_view: "awaiting_review", review_status: "open",
    review_advisory: "without", review_agent: "without", review_agent_harness: "omp", review_agent_model: "",
  });
});

// r[verify kiln.review.keyboard] r[verify kiln.review.keyboard.navigation] r[verify kiln.review.keyboard.actions]
const reviewKeys = reviewContext.window.KilnCodeReview.keys;
const shortcut = (key, extra = {}) => ({ key, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, ...extra });
const shortcutBindings = [
  ["j", false, "next-file"], ["ArrowDown", false, "next-file"],
  ["k", false, "previous-file"], ["ArrowUp", false, "previous-file"],
  ["n", false, "next-hunk"], ["p", false, "previous-hunk"],
  ["t", false, "next-thread"], ["T", true, "previous-thread"],
  ["u", false, "next-unviewed"], ["U", true, "previous-unviewed"],
  ["m", false, "toggle-layout"],
  ["v", false, "toggle-hunk-viewed"], ["V", true, "toggle-file-viewed"],
  ["d", false, "start-draft"], ["P", true, "publish"],
  ["s", false, "requirements"], ["?", true, "help"],
];

// r[verify kiln.review.keyboard.scenario.inert]
test("review shortcuts bind their documented keys and yield to typing, dialogs, modifiers and composition", () => {
  for (const [key, shift, id] of shortcutBindings) assert.equal(reviewKeys.resolve(shortcut(key, { shiftKey: shift })), id, `${key}`);
  // Shift is the discriminator: a shifted lowercase-only binding stays inert, while an uppercase key
  // without shift (Caps Lock) resolves the unshifted command rather than inventing a second one.
  for (const key of ["J", "K", "N", "M", "D", "S"]) assert.equal(reviewKeys.resolve(shortcut(key, { shiftKey: true })), null, `${key} shift=true`);
  for (const [key, id] of [["T", "next-thread"], ["U", "next-unviewed"], ["V", "toggle-hunk-viewed"], ["P", "previous-hunk"]]) assert.equal(reviewKeys.resolve(shortcut(key)), id, key);
  // Typing, overlays, IME composition and command modifiers always suppress the layer.
  assert.equal(reviewKeys.resolve(shortcut("j"), { editable: true }), null);
  assert.equal(reviewKeys.resolve(shortcut("j"), { dialogOpen: true }), null);
  assert.equal(reviewKeys.resolve(shortcut("j", { isComposing: true })), null);
  for (const modifier of ["ctrlKey", "metaKey", "altKey"]) assert.equal(reviewKeys.resolve(shortcut("j", { [modifier]: true })), null, modifier);
  for (const value of [null, undefined, {}, { key: "" }]) assert.equal(reviewKeys.resolve(value), null);
  // Help accepts the produced "?" key or layout-independent Shift+/ and needs no shift flag on "?" itself.
  assert.equal(reviewKeys.resolve(shortcut("?")), "help");
  assert.equal(reviewKeys.resolve(shortcut("/", { shiftKey: true })), "help");
  assert.equal(reviewKeys.resolve(shortcut("/")), null);
  // Unknown keys stay unbound rather than falling through to a neighbour.
  for (const key of ["q", "x", "Enter", " ", "Escape"]) assert.equal(reviewKeys.resolve(shortcut(key)), null, key);
});

test("the shortcut help lists every bound command once, in table order", () => {
  const commands = plain(reviewKeys.commands);
  const ids = shortcutBindings.map(([, , id]) => id).filter((id, index, all) => all.indexOf(id) === index);
  assert.deepEqual(commands.map((command) => command.id), ids);
  for (const command of commands) {
    assert.ok(command.keys, `${command.id} keys`);
    assert.ok(command.description, `${command.id} description`);
  }
});

// r[verify kiln.monorepo.console]
test("projects sharing one repository key group together; single projects and keyless records stay alone", () => {
  const shape = (groups) => [...groups].map((group) => ({ key: group.key, label: group.label, shared: group.shared, ids: [...group.projects].map((project) => project.id) }));
  assert.deepEqual(shape(ui.groupProjects([
    { id: "api", repository: "/srv/mono", repository_key: "aa" },
    { id: "solo", repository: "/srv/solo", repository_key: "bb" },
    { id: "web", repository: "/srv/mono", repository_key: "aa" },
  ])), [
    { key: "aa", label: "/srv/mono", shared: true, ids: ["api", "web"] },
    { key: "bb", label: "/srv/solo", shared: false, ids: ["solo"] },
  ]);
  // Older records without a key never merge, even on one repository path; malformed rows are skipped.
  assert.deepEqual(shape(ui.groupProjects([{ id: "a", repository: "/srv/r" }, null, "x", { id: "b", repository: "/srv/r", repository_key: "" }])), [
    { key: null, label: "/srv/r", shared: false, ids: ["a"] },
    { key: null, label: "/srv/r", shared: false, ids: ["b"] },
  ]);
  for (const value of [undefined, null, {}, "projects"]) assert.equal(ui.groupProjects(value).length, 0);
});

// r[verify kiln.monorepo.console]
test("path scopes show the root or the whole repository, graph kinds and only deliberate narrowing", () => {
  const scoped = plain(ui.scopeSummary({ scope: { root: "crates/api", include: ["**"], exclude: ["docs/**"], depends_on: ["proto/**"], graphs: ["cargo", "nix"], identity: "ff" } }));
  assert.deepEqual(scoped, { scoped: true, whole: false, root: "crates/api", graphs: ["Cargo", "Nix"], include: [], exclude: ["docs/**"], dependsOn: ["proto/**"], identity: "ff" });
  assert.equal(ui.scopeSummary({ scope: { root: "", include: ["src/**"] } }).root, "Whole repository");
  assert.deepEqual([...ui.scopeSummary({ scope: { root: "", include: ["src/**"] } }).include], ["src/**"]);
  const unscoped = ui.scopeSummary({ scope: null });
  assert.equal(unscoped.scoped, false);
  assert.equal(unscoped.root, "Whole repository");
  // Absent (older service) or malformed scopes render nothing rather than a guessed scope.
  for (const holder of [{}, { scope: "crates/api" }, { scope: { include: ["**"] } }, { scope: [] }, null, undefined]) assert.equal(ui.scopeSummary(holder), null);
});

// r[verify kiln.monorepo.console]
test("affected reasons read as plain words and unknown or malformed reasons give a safe generic line", () => {
  const read = (reason) => plain(ui.reasonText(reason));
  assert.deepEqual(read({ kind: "path", sample: ["crates/api/src/lib.rs", 7], count: 3 }), { kind: "path", text: "3 changed files are in this project's scope.", sample: ["crates/api/src/lib.rs"] });
  assert.equal(read({ kind: "path", sample: ["a", "b"], count: 1 }).text, "Changed files are in this project's scope.");
  assert.deepEqual(read({ kind: "declared", pattern: "proto/**", sample: ["proto/api.proto"] }), { kind: "declared", text: "A changed file matches the declared dependency proto/**.", sample: ["proto/api.proto"] });
  assert.equal(read({ kind: "graph", graph: "cargo", from_package: "kiln-highlight", via_package: "kiln-web-service" }).text, "Cargo build graph: kiln-web-service depends on kiln-highlight, which changed.");
  assert.equal(read({ kind: "graph", graph: "nix", from_package: "flake.lock", via_package: "*" }).text, "flake.lock changed; every project using the Nix build graph depends on it.");
  assert.equal(read({ kind: "graph", graph: "cargo", from_package: "api", via_package: "api" }).text, "Package api under this project's root changed (Cargo build graph).");
  assert.equal(read({ kind: "unscoped" }).text, "This project has no path scope, so every change runs it.");
  assert.equal(read({ kind: "fallback", reason: "truncated" }).text, "Ran to be safe: too many files changed to list them exactly.");
  assert.equal(read({ kind: "fallback", reason: "graph_unavailable: cargo: missing manifest crates/x/Cargo.toml" }).text, "Ran to be safe: the Cargo build graph could not be read (missing manifest crates/x/Cargo.toml).");
  assert.deepEqual(read({ kind: "teleport", sample: ["x"] }), { kind: "unknown", text: "Recorded reason “teleport” is not recognised by this console.", sample: [] });
  for (const value of [null, "path", 3, [], { sample: ["x"] }]) assert.deepEqual(read(value), { kind: "unknown", text: "A recorded reason could not be read.", sample: [] });
});

// r[verify kiln.monorepo.console]
test("a run's affected record yields reasons and the changed-file fact; absent or malformed records yield nothing", () => {
  const summary = plain(ui.affectedSummary({ set_identity: "abc", reasons: [{ kind: "path", sample: ["a"], count: 1 }, { kind: "nope" }], changed: { exact_count: 12 } }));
  assert.equal(summary.identity, "abc");
  assert.equal(summary.changed, "12 changed files compared with the previous revision.");
  assert.deepEqual(summary.reasons.map((reason) => reason.kind), ["path", "unknown"]);
  assert.equal(ui.affectedSummary({ reasons: [], changed: { fallback: "no_base" } }).changed, "Changed files not known: there was no earlier revision to compare with (a new reference or a manual run).");
  assert.equal(ui.affectedSummary({ reasons: [], changed: { exact_count: -1 } }).changed, null);
  for (const value of [undefined, null, "affected", [], { set_identity: "abc" }, { reasons: "path" }]) assert.equal(ui.affectedSummary(value), null);
});

// r[verify kiln.monorepo.console]
test("review CI rows list affected projects with state and run; skipped projects are not required with their reason", () => {
  const rows = plain(ui.ciProjects([
    { project_id: "api", label: "API", outcome: "affected", reasons: [{ kind: "path", sample: ["crates/api/a.rs"], count: 1 }], run_id: "run-1", state: "passed" },
    { project_id: "web", label: "Web", outcome: "affected", reasons: [], run_id: null, state: "stale_scope" },
    { project_id: "docs", outcome: "skipped", reason: "no_changes_in_scope" },
    { project_id: "odd", outcome: "affected", state: "exploded" },
    { project_id: "bad", outcome: "maybe" }, { outcome: "affected" }, null,
  ]));
  assert.deepEqual(rows.map((row) => [row.projectId, row.label, row.required, row.stateLabel, row.tone, row.runId]), [
    ["api", "API", true, "Passed", "success", "run-1"],
    ["web", "Web", true, "Scope changed since its run", "warning", null],
    ["docs", "docs", false, "Not required", "cancelled", null],
    ["odd", "odd", true, "exploded", "unknown", null],
  ]);
  assert.equal(rows[2].detail, "No changed file is in its scope, its declared dependencies or its build graph.");
  assert.equal(ui.skipText("future_reason"), "Recorded reason “future_reason” is not recognised by this console.");
  assert.equal(ui.ciProjects(undefined).length, 0);
  const decisions = plain(ui.affectedDecisions({ identity: "id", decisions: [{ project_id: "api", outcome: "affected", reasons: [{ kind: "unscoped" }] }, { project_id: "web", outcome: "skipped", reason: "no_changes_in_scope" }, { project_id: "x" }] }));
  assert.deepEqual(decisions.rows.map((row) => [row.projectId, row.affected]), [["api", true], ["web", false]]);
  for (const value of [undefined, null, { identity: "id" }, { decisions: {} }]) assert.equal(ui.affectedDecisions(value), null);
});

// r[verify kiln.untrusted.console]
test("untrusted gates show state, reason, approver, approved commit and counting; trusted or malformed runs show none", () => {
  const commit = "a".repeat(40);
  const blocked = plain(ui.untrustedSummary({ state: "awaiting_approval", reason: "fork", pull_request: 7, head_repository: "outside/fork", source_commit: commit, approved_by: null, approved_by_id: null, approved_commit: null, approved_at_ms: null, counts_for_required_ci: false, superseded_by: null }));
  assert.deepEqual([blocked.stateLabel, blocked.tone, blocked.awaitingApproval, blocked.reasonText, blocked.pullRequest, blocked.sourceCommit, blocked.approvedBy, blocked.countsForRequiredCi], ["Awaiting approval", "queued", true, "The pull request comes from a fork.", 7, commit, null, false]);
  const approved = plain(ui.untrustedSummary({ state: "approved", reason: "author_without_push_access", pull_request: 7, head_repository: "o/r", source_commit: commit, approved_by: "Operator", approved_by_id: "local:operator", approved_commit: commit, approved_at_ms: 5, counts_for_required_ci: true }));
  assert.deepEqual([approved.stateLabel, approved.tone, approved.awaitingApproval, approved.approvedBy, approved.approvedById, approved.approvedCommit, approved.approvedAtMs, approved.countsForRequiredCi], ["Approved to run untrusted", "success", false, "Operator", "local:operator", commit, 5, true]);
  // Unknown reasons and non-boolean counting never read as recognised or counting.
  const superseded = plain(ui.untrustedSummary({ state: "superseded", reason: "future", pull_request: 0, superseded_by: "web-run-2", counts_for_required_ci: "yes" }));
  assert.deepEqual([superseded.stateLabel, superseded.tone, superseded.reasonText, superseded.pullRequest, superseded.supersededBy, superseded.countsForRequiredCi], ["Superseded by a newer commit", "cancelled", "No recognised reason was recorded.", null, "web-run-2", false]);
  assert.equal(ui.untrustedSummary({ state: "future_state" }).tone, "unknown");
  for (const value of [undefined, null, "approved", [], {}, { state: "" }]) assert.equal(ui.untrustedSummary(value), null);
  assert.deepEqual({ ...ui.outcomeStatus("Pending", "awaiting_approval") }, { status: "queued", label: "Awaiting approval" });
});

// r[verify kiln.review.structural.scenario.anchors]
test("structural and line modes create equal anchors; an interdiff's rebased side has none", () => {
  const comparison = reviewContext.window.KilnCodeReview.comparison;
  const revision = { id: "r2", commit: "c2", base: "b2" };
  const line = { old_commit: "b2", new_commit: "c2", files: [] };
  const structural = { ...line, structural: { files: [{ path: "lib.rs", status: "structural" }] } };
  assert.deepEqual(plain(comparison.anchorFor(structural, revision, "lib.rs", "revision", 4, 4)), plain(comparison.anchorFor(line, revision, "lib.rs", "revision", 4, 4)));
  assert.deepEqual(plain(comparison.anchorFor(structural, revision, "lib.rs", "base", 2, 3)), { revision: "r2", base_commit: "b2", file: "lib.rs", side: "base", start_line: 2, end_line: 3 });
  const exact = { old_commit: "c1", new_commit: "c2", interdiff: { status: "exact" } };
  assert.equal(comparison.anchorFor(exact, revision, "f", "base", 1, 1), null);
  assert.equal(comparison.anchorBaseFor(exact, revision, "revision"), "b2");
  assert.equal(comparison.anchorBaseFor({ ...exact, interdiff: { status: "unavailable" } }, revision, "base"), "c1");
  const range = { old_commit: "m", new_commit: "c2", commit_range: { from: "m", to: "c2" } };
  assert.equal(comparison.anchorBaseFor(range, revision, "base"), null);
  assert.equal(comparison.anchorBaseFor(range, revision, "revision"), "b2");
  assert.equal(comparison.anchorBaseFor({ ...range, old_commit: "b2" }, revision, "base"), "b2");
  assert.equal(comparison.anchorBaseFor({ old_commit: "b2", new_commit: "m", commit_range: { from: "b2", to: "m" } }, revision, "revision"), null);
});

// r[verify kiln.review.interdiff.route]
test("diff identity admits only the requested base, interdiff or commit range", () => {
  const { diffMatches } = reviewContext.window.KilnCodeReview.comparison;
  const r1 = { id: "r1", commit: "c1", base: "b1" }, r2 = { id: "r2", commit: "c2", base: "b2" };
  assert.equal(diffMatches({ old_commit: "b2", new_commit: "c2" }, r2, null, null, null), true);
  assert.equal(diffMatches({ old_commit: "c1", new_commit: "c2" }, r2, null, null, r1), true);
  assert.equal(diffMatches({ old_commit: "b1", new_commit: "c2" }, r2, null, null, null), false);
  assert.equal(diffMatches({ old_commit: "c1", new_commit: "c2", interdiff: { from: "r1", to: "r2" } }, r2, r1, null, null), true);
  assert.equal(diffMatches({ old_commit: "c1", new_commit: "c2", interdiff: { from: "r0", to: "r2" } }, r2, r1, null, null), false);
  assert.equal(diffMatches({ old_commit: "x", new_commit: "y", interdiff: { from: "r1", to: "r2", change: "k" } }, r2, r1, null, null), true);
  assert.equal(diffMatches({ old_commit: "c1", new_commit: "c2" }, r2, r1, null, null), false);
  assert.equal(diffMatches({ old_commit: "m", new_commit: "c2" }, r2, null, { from: "m", to: "c2" }, null), true);
  assert.equal(diffMatches({ old_commit: "b2", new_commit: "c2" }, r2, null, { from: "m", to: "c2" }, null), false);
  assert.equal(diffMatches({ old_commit: "b2", new_commit: "c2" }, undefined, null, null, null), false);
});

const receipts = context.window.KilnReceipts;
const NINE = ["accepted", "rejected", "escalated", "blocked", "waiting", "pending", "removed", "reverted", "unknown"];

// r[verify kiln.dashboard.reviews.view.scenario.decisions]
test("each of the nine admitted decisions reads in its own words; other values never read as accepted", () => {
  assert.deepEqual([...receipts.decisions].map((entry) => entry.key), NINE);
  const tones = Object.fromEntries(NINE.map((key) => [key, receipts.decision(key).tone]));
  assert.deepEqual(tones, { accepted: "success", rejected: "failure", escalated: "warning", blocked: "warning", waiting: "queued", pending: "queued", removed: "cancelled", reverted: "cancelled", unknown: "unknown" });
  const labels = NINE.map((key) => receipts.decision(key).label);
  assert.equal(new Set(labels).size, 9);
  for (const key of NINE) {
    const entry = receipts.decision(key);
    assert.equal(entry.key, key);
    assert.doesNotMatch(entry.label, /unrecognized|not recorded/i, key);
    assert.ok(entry.meaning.length > 20, key);
    // Every decision but acceptance carries Patchgate's next step.
    assert.equal(Boolean(entry.next), key !== "accepted", key);
  }
  const merged = receipts.decision("merged");
  assert.deepEqual([merged.key, merged.label, merged.tone, merged.recorded], ["unrecognized", "Unrecognized: merged", "unknown", "merged"]);
  const long = receipts.decision("x".repeat(80));
  assert.equal(long.recorded, "x".repeat(80));
  assert.ok(long.label.length <= "Unrecognized: ".length + 48 && long.label.endsWith("…"));
  // Names that exist on every object's prototype are not decisions.
  for (const value of ["toString", "__proto__", "constructor", "Accepted", " accepted"]) assert.equal(receipts.decision(value).key, "unrecognized", value);
  for (const value of [null, undefined, "", 3, {}, ["accepted"]]) assert.deepEqual([receipts.decision(value).key, receipts.decision(value).label], ["missing", "Not recorded"], String(value));
  for (const value of ["merged", "toString", null]) assert.notEqual(receipts.decision(value).tone, "success");
  assert.deepEqual([...receipts.filters].map((filter) => filter.key), ["", "attention", ...NINE]);
});

// r[verify kiln.dashboard.reviews.view.scenario.decisions]
test("a tally puts what needs a person first and keeps distinct unrecognized values apart", () => {
  const rows = ["accepted", "rejected", "accepted", "merged", null, "escalated", "merged", "other", "waiting"].map((decision) => ({ decision }));
  assert.deepEqual(plain(receipts.tally(rows).map((row) => [row.key, row.label, row.count])), [
    ["rejected", "Rejected", 1], ["escalated", "Escalated", 1], ["unrecognized", "Unrecognized: merged", 2], ["unrecognized", "Unrecognized: other", 1],
    ["missing", "Not recorded", 1], ["waiting", "Waiting", 1], ["accepted", "Accepted", 2],
  ]);
  for (const value of [undefined, null, "rows", {}]) assert.deepEqual(plain(receipts.tally(value)), [], String(value));
});

const nativeReceipt = (report) => ({ schema: "kiln.patchgate.review.v1", observation: { reviews: [] }, native_report: typeof report === "string" ? report : JSON.stringify(report) });

// r[verify kiln.dashboard.reviews.view.scenario.context]
test("Patchgate's report is read for each change, its records and the review context", () => {
  const configuration = { source: "main", path: "patchgate.json", sha256: "f".repeat(64), values: { policy_path: "POLICY.md", protected: ["src/**"], documentation_paths: ["docs/*", "*.md"] } };
  const context = receipts.nativeContext(nativeReceipt({
    dry_run: true, authorizes_acceptance: false,
    main: { channel: "main", state: "M".repeat(53) }, inbox: { channel: "inbox", state: "I".repeat(53) }, configuration,
    items: [
      { hash: "AAA", message: "Clarify the guide", paths: ["docs/guide.md", 7], state: "accepted", reason: "", needs: [], decided: true, merge: null, note: "" },
      { hash: "BBB", message: "Serve on port 9090", paths: ["server.conf"], state: "blocked", reason: "A prerequisite has a blocked or terminal state.", needs: [["AAA", "rejected"], ["CCC"], "bad", [7, "x"]], decided: false, merge: { hash: "MMM" }, note: "check" },
      { hash: "AAA", message: "A duplicate is ignored" },
    ],
    records: [
      { hash: "AAA", decision: "accept", by: "gate", reason: "Only documentation paths.", at: "2026-10-28T12:21:25+00:00", provenance: { requests: [], verifiers: { schema_version: 1, patchgate: "0.1.0", pijul: "pijul 1.0.0-beta.24", nix: null } }, verification: { results: [{ check: "build" }, { check: "" }] } },
      { hash: "BBB", decision: "escalate", by: "model", model: "gemma", at: "5", coverage: { planned_pages: 2, complete: false, pages: [{ index: 0, decision: "accept" }] }, provenance: { requests: [{}, {}] } },
    ],
    review_usage: { requests: 2, max_requests: 128, elapsed_seconds: 0.25, timeout_seconds: 900, stop: null },
  }));
  assert.equal(context.status, "read");
  assert.equal(context.changes.size, 2);
  const first = context.changes.get("AAA");
  assert.deepEqual([first.message, [...first.paths], first.decided, first.record.by, first.record.reason, first.record.atMs, [...first.record.checks]], ["Clarify the guide", ["docs/guide.md"], true, "gate", "Only documentation paths.", Date.parse("2026-10-28T12:21:25Z"), ["build"]]);
  const second = context.changes.get("BBB");
  assert.deepEqual(plain(second.needs), [{ hash: "AAA", state: "rejected" }, { hash: "CCC" }]);
  assert.deepEqual([second.merge, second.note, second.record.model, second.record.modelRequests], ["MMM", "check", "gemma", 2]);
  // A time without an RFC 3339 offset is not an instant, even where Date.parse would guess one.
  assert.equal(second.record.atMs, null);
  assert.deepEqual(plain(second.record.coverage), { planned: 2, reviewed: 1, complete: false, accepted: 1, rejected: 0, escalated: 0 });
  assert.deepEqual([context.report.dryRun, context.report.main.channel, context.report.inbox.state.length, context.report.configuration.policy, [...context.report.configuration.protected]], [true, "main", 53, "POLICY.md", ["src/**"]]);
  assert.deepEqual(plain(context.report.usage), { requests: 2, maxRequests: 128, elapsedSeconds: 0.25, timeoutSeconds: 900, stop: null });
  assert.deepEqual(plain(context.report.verifiers), { patchgate: "0.1.0", pijul: "pijul 1.0.0-beta.24", nix: null });
});

// r[verify kiln.dashboard.reviews.view.scenario.context]
test("an unreadable, absent or hostile report never removes a decision or reaches the prototype", () => {
  const text = receipts.nativeContext(nativeReceipt("patchgate 0.4\nreview: accepted\n"));
  assert.deepEqual([text.status, text.changes.size, text.bytes], ["unreadable", 0, 31]);
  for (const value of ["[1,2]", "\"report\"", "null", "{"]) assert.equal(receipts.nativeContext(nativeReceipt(value)).status, "unreadable", value);
  for (const document of [{ schema: "kiln.patchgate.review.v1" }, { native_report: 5 }, null, "receipt"]) {
    const absent = receipts.nativeContext(document);
    assert.deepEqual([absent.status, absent.bytes, absent.changes.size], ["absent", null, 0]);
  }
  const hostile = receipts.nativeContext(nativeReceipt('{"__proto__":{"polluted":true},"items":[{"hash":"__proto__","message":"proto"},{"hash":"constructor","message":5,"paths":"src"}],"records":{"hash":"x"},"main":{"channel":7}}'));
  assert.equal(hostile.status, "read");
  // The reader runs in the module's realm, so that realm's prototype is the one that must stay clean.
  assert.equal(vm.runInNewContext("({}).polluted", context), undefined);
  assert.equal(hostile.changes.get("__proto__").message, "proto");
  assert.deepEqual([hostile.changes.get("constructor").message, [...hostile.changes.get("constructor").paths], hostile.changes.get("constructor").record], [null, [], null]);
  assert.equal(hostile.report.main, null);
});

test("a decider reads as Patchgate's rules, a model or a person", () => {
  const cases = [
    [{ by: "gate" }, "Patchgate rules, no model"], [{ by: "model", model: "gemma" }, "Model gemma"], [{ by: "model" }, "A model"],
    [{ by: "human", user: "britton" }, "A person (britton)"], [{ by: "human" }, "A person"], [{ by: "gate", reused: true }, "Patchgate rules, no model · reused an earlier verdict"],
    [{ by: "lane" }, "lane"], [{ by: null }, null], [null, null],
  ];
  for (const [record, label] of cases) assert.equal(receipts.decider(record), label, JSON.stringify(record));
});

// r[verify kiln.dashboard.reviews.view.scenario.find]
test("receipts list newest decision first and filter by text and decision", () => {
  const receipt = (name, at, decisions, extra = {}) => ({ name, latest_decision_at_ms: at, decisions: decisions.map((decision, index) => ({ change: { native_id: `${name.toUpperCase()}${index}HASH` }, decision })), isolated: true, authorizes_acceptance: false, ...extra });
  const rows = receipts.listRows({ receipts: [
    receipt("older.json", 100, ["accepted"], { first_change_message: "Clarify the guide", candidate_id: "candidate-guide" }),
    receipt("newer.json", 300, ["escalated", "accepted"]),
    receipt("untimed-b.json", null, ["waiting"]),
    receipt("untimed-a.json", "300", []),
    receipt("older.json", 900, ["rejected"]),
    { latest_decision_at_ms: 500 },
    receipt("claims.json", 50, ["accepted"], { authorizes_acceptance: true, isolated: false }),
  ] });
  assert.deepEqual(plain(rows.map((row) => row.name)), ["newer.json", "older.json", "claims.json", "untimed-a.json", "untimed-b.json"]);
  const named = Object.fromEntries(rows.map((row) => [row.name, row]));
  assert.deepEqual([named["older.json"].message, named["newer.json"].changes, named["untimed-a.json"].atMs], ["Clarify the guide", 2, null]);
  assert.deepEqual([named["claims.json"].claimsAuthority, named["claims.json"].isolated, named["older.json"].claimsAuthority, named["older.json"].isolated], [true, false, false, true]);
  const visible = (query, filter) => plain(rows.filter((row) => receipts.matches(row, query, filter)).map((row) => row.name));
  assert.deepEqual(visible("", ""), plain(rows.map((row) => row.name)));
  assert.deepEqual(visible("GUIDE", ""), ["older.json"]);
  assert.deepEqual(visible("newer.json1h", ""), ["newer.json"]);
  assert.deepEqual(visible(" candidate-guide ", ""), ["older.json"]);
  assert.deepEqual(visible("", "accepted"), ["newer.json", "older.json", "claims.json"]);
  // Escalated and an empty receipt need attention; waiting and accepted alone do not.
  assert.deepEqual(visible("", "attention"), ["newer.json", "untimed-a.json"]);
  assert.deepEqual(visible("guide", "escalated"), []);
  for (const value of [undefined, null, {}, { receipts: "x" }]) assert.deepEqual(plain(receipts.listRows(value)), [], String(value));
  assert.deepEqual(plain(receipts.normalizeRoute({ receipt: "a.json", receipt_q: "q".repeat(300), receipt_decision: "attention" })), { receipt: "a.json", q: "q".repeat(256), decision: "attention" });
  assert.equal(receipts.normalizeRoute({ receipt_decision: "everything" }).decision, "");
  assert.deepEqual(plain(receipts.normalizeRoute(undefined)), { receipt: "", q: "", decision: "" });
});

const workers = context.window.KilnWorkers;
const target = (id, label, profile, extra = {}) => ({
  id, provider_profile_id: profile, platform: "linux-x86_64", status: "slots_available", liveness: "unknown",
  capacity: { configured_slots: 1, enrolled_slots: 1, active: 0, free_slots: 1, queued: 0 },
  projects: [{ id: `${id}-project`, label, enrolled: true, active_run_ids: [], queued: 0 }], latest_run: null,
  ...extra,
});

// r[verify kiln.dashboard.workers.view] r[verify kiln.dashboard.workers.view.scenario.register]
test("worker targets sort by readable name without coalescing distinct bindings or reordering their source", () => {
  const inventory = { workers: Object.freeze([
    target("z", "Shared project", "beta"), target("b", "Beta project", "provider"),
    target("a", "alpha project", "provider"), target("x", "Shared project", "alpha"),
  ]) };
  const rows = workers.rows(inventory);
  assert.deepEqual(plain(rows.map((row) => row.id)), ["a", "b", "x", "z"]);
  assert.deepEqual(inventory.workers.map((row) => row.id), ["z", "b", "a", "x"]);
  assert.equal(rows[2].worker, inventory.workers[3]);
  assert.equal(workers.matches(rows[0], " ALPHA PROJECT ", "slots_available"), true);
  assert.equal(workers.matches(rows[0], "a-project", ""), true);
  assert.equal(workers.matches(rows[0], "linux-x86_64", "busy"), false);
  assert.equal(workers.matches(rows[0], "not a project", ""), false);
});

// r[verify kiln.dashboard.workers.view.scenario.restore]
test("worker filters never substitute an exact selected target, including a missing identity", () => {
  const rows = workers.rows({ workers: [target("a", "Alpha", "profile-a"), target("b", "Beta", "profile-b", { status: "busy" })] });
  const route = workers.normalizeRoute({ worker: "a", worker_q: "beta", worker_status: "busy" });
  const selected = workers.selection(rows, route);
  assert.equal(selected.selected.id, "a");
  assert.equal(selected.hidden, true);
  assert.equal(selected.missing, false);
  const absent = workers.selection(rows, { ...route, worker: "missing" });
  assert.equal(absent.selected, null);
  assert.equal(absent.missing, true);
  assert.equal(absent.hidden, false);
  assert.equal(workers.selection(rows, workers.normalizeRoute({})).selected, null);
  assert.equal(workers.normalizeRoute({ worker_status: "offline" }).status, "offline");
  assert.equal(workers.normalizeRoute({ worker_status: "constructor" }).status, "");
  assert.equal(workers.normalizeRoute({ worker_campaign: " exact-campaign " }).campaign, " exact-campaign ");
  assert.equal(workers.normalizeRoute({ worker_campaign: "c".repeat(256) }).campaign, "c".repeat(256));
  assert.equal(workers.normalizeRoute({ worker_campaign: "c".repeat(257) }).campaign, "");
  assert.equal(workers.normalizeRoute({ worker_campaign: "bad\nvalue" }).campaign, "");
});

// r[verify kiln.dashboard.workers.view.scenario.detail]
test("worker counts and execution timing preserve zero without inventing missing or invalid observations", () => {
  assert.deepEqual(plain(workers.capacity({ configured_slots: 0, enrolled_slots: null, active: -1, free_slots: "0", queued: 1.5 })), { configured_slots: 0, enrolled_slots: null, active: null, free_slots: null, queued: null });
  const timed = workers.execution({ started_at_ms: 0, completed_at_ms: 1250, created_at_ms: 0 });
  assert.deepEqual([timed.atMs, timed.durationMs], [1250, 1250]);
  const zero = workers.execution({ started_at_ms: 0, completed_at_ms: 0 });
  assert.deepEqual([zero.atMs, zero.durationMs], [0, 0]);
  for (const run of [{}, { started_at_ms: null }, { started_at_ms: "0", completed_at_ms: null }, { started_at_ms: -1 }]) {
    const value = workers.execution(run);
    assert.deepEqual([value.atMs, value.durationMs], [null, null]);
  }
  assert.equal(workers.execution({ started_at_ms: 20, completed_at_ms: 10 }).durationMs, null);
  assert.equal(workers.execution(null), null);
});

// r[verify kiln.dashboard.workers.view.scenario.heartbeat]
test("expired or unknown heartbeats never turn historical free slots or a passed run into current capacity", () => {
  const observation = { observed_slots: 4, active_slots: 1, free_slots: 3, last_seen_at_ms: 1000 };
  const online = workers.heartbeat({ liveness: "online", observed_capacity: observation });
  assert.deepEqual([online.currentFree, online.reportedFree, online.atMs], [3, 3, 1000]);
  const expired = workers.heartbeat({ liveness: "offline", observed_capacity: observation, latest_run: { outcome: "Success" } });
  assert.deepEqual([expired.state, expired.currentFree, expired.reportedFree], ["offline", null, 3]);
  const unknown = workers.heartbeat({ liveness: "unknown", latest_run: { outcome: "Success" } });
  assert.deepEqual([unknown.observed, unknown.currentFree, unknown.atMs], [false, null, null]);
  const foreign = workers.heartbeat({ liveness: "future-state", observed_capacity: observation });
  assert.deepEqual([foreign.state, foreign.currentFree], ["future-state", null]);
  const occupied = workers.heartbeat({ liveness: "online", observed_capacity: { ...observation, free_slots: 0, last_seen_at_ms: 0 } });
  assert.deepEqual([occupied.currentFree, occupied.atMs], [0, 0]);
  // Monitoring may be configured before any admitted heartbeat has arrived: the server supplies a zero aggregate
  // with no last-seen time. That is no report, not an expired report of zero physical capacity.
  const neverSeen = workers.heartbeat({ liveness: "offline", observed_capacity: { observed_slots: 0, active_slots: 0, free_slots: 0, last_seen_at_ms: null } });
  assert.deepEqual([neverSeen.observed, neverSeen.atMs, neverSeen.slots, neverSeen.reportedFree, neverSeen.currentFree], [false, null, null, null, null]);
  assert.equal(observation.free_slots, 3);
});

const cairn = context.window.KilnCairn;
const CAIRN_REPOSITORY = "ab".repeat(32);
const LANE_KEYS = ["attention", "awaiting", "ready", "in_progress", "verified", "integrated", "recorded"];
const laneCounts = (lane = "awaiting") => Object.fromEntries(LANE_KEYS.map((key) => [key, Number(key === lane)]));
const cairnWorkflows = (extra = {}) => ({ source: "not_configured", reason: null, sources: [], definitions: [], ...extra });
const cairnAgents = (extra = {}) => ({
  source: "not_configured", reason: null, inventory_identity: null, evaluated_at_ms: null, synthetic: null, sources: [],
  workflow_revisions: cairnWorkflows(), definitions: [], workers: [], ...extra,
});
const cairnDetail = (change, extra = {}) => ({
  schema: "kiln.cairn-workspace-detail.v4", identity: "c".repeat(64), repository_identity: CAIRN_REPOSITORY, change, projects: ["kiln"],
  snapshot: { work_recorded: true, attempts_recorded: true, campaigns_read: 0, campaigns_truncated: false, lifecycle_truncated: false },
  overview: { units: 1, current_work: 0, attempts: 0, active_attempts: 0, campaigns: 0, blockers: 0, open_decisions: 0, lane: "awaiting", lanes: laneCounts() },
  units: [{ native_id: `${change}/unit-of-${change}`, unit: `unit-of-${change}`, lane: "awaiting", sources: ["campaign_selection"], current_work: [], superseded_work: [], attempts: [],
    facets: { readiness: "not_admitted", execution: "not_started", evidence: "none", integration: "not_published", lifecycle: "not_observed" } }],
  memberships: [], decisions: [], blockers: [], agents: cairnAgents(), non_claims: ["display is not campaign, work, verification or lifecycle authority"], ...extra,
});

// r[verify kiln.cairn.workspace.scenario.reload]
test("the URL preserves exact Cairn selection, filters and unit, and detail must echo it", () => {
  const route = cairn.normalizeRoute({ cairn_repository: CAIRN_REPOSITORY, cairn_change: "add cairn&docs", cairn_unit: "add cairn&docs/unit-one", cairn_q: "q".repeat(300), cairn_lane: "attention", cairn_blocked: "1", cairn_decisions: "1", cairn_view: "board" });
  assert.deepEqual(plain(route), { repository: CAIRN_REPOSITORY, change: "add cairn&docs", unit: "add cairn&docs/unit-one", q: "q".repeat(256), lane: "attention", blocked: true, decisions: true, view: "board", invalid: false, invalidUnit: false });
  assert.equal(cairn.detailPath(route), `/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=add%20cairn%26docs`);
  assert.deepEqual(plain(cairn.filterPatch(route)), { cairn_q: "q".repeat(256), cairn_lane: "attention", cairn_blocked: "1", cairn_decisions: "1", cairn_view: "board" });
  for (const [repository, change] of [["AB".repeat(32), "a"], ["ab".repeat(31), "a"], [CAIRN_REPOSITORY, "a/b"], [CAIRN_REPOSITORY, " "], [CAIRN_REPOSITORY, "a\u0085b"], [CAIRN_REPOSITORY, "é".repeat(129)], ["", "a"], [CAIRN_REPOSITORY, ""]]) {
    const rejected = cairn.normalizeRoute({ cairn_repository: repository, cairn_change: change });
    assert.deepEqual([rejected.repository, rejected.change, rejected.invalid], ["", "", true], `${repository} ${change}`);
  }
  for (const unit of ["other/unit-one", "add cairn&docs/", "add cairn&docs/a\u0000b", "add cairn&docs/" + "é".repeat(512)]) {
    const rejected = cairn.normalizeRoute({ cairn_repository: CAIRN_REPOSITORY, cairn_change: "add cairn&docs", cairn_unit: unit });
    assert.deepEqual([rejected.change, rejected.unit, rejected.invalidUnit], ["add cairn&docs", "", true]);
  }
  assert.equal(cairn.normalizeRoute({ cairn_repository: CAIRN_REPOSITORY, cairn_change: "é".repeat(128) }).change, "é".repeat(128));
  assert.deepEqual(plain(cairn.normalizeRoute({ cairn_unit: "orphan", cairn_lane: "unknown", cairn_blocked: "true", cairn_decisions: "0", cairn_view: "unknown" })),
    { repository: "", change: "", unit: "", q: "", lane: "", blocked: false, decisions: false, view: "list", invalid: false, invalidUnit: true });
  const selected = cairn.normalizeRoute({ cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" });
  assert.equal(cairn.admitDetail(cairnDetail("wanted"), selected).change, "wanted");
  for (const crossed of [cairnDetail("other"), cairnDetail("wanted", { repository_identity: "cd".repeat(32) }), cairnDetail("wanted", { schema: "kiln.cairn-workspace-detail.v1" }),
    cairnDetail("wanted", { schema: "kiln.cairn-workspace-detail.v2" }), cairnDetail("wanted", { schema: "kiln.cairn-workspace-detail.v3" }),
    cairnDetail("wanted", { agents: undefined }), cairnDetail("wanted", { agents: cairnAgents({ source: "empty" }) }),
    cairnDetail("wanted", { overview: { units: 1 } }), cairnDetail("wanted", { units: null }), null]) {
    assert.throws(() => cairn.admitDetail(crossed, selected), /No other change was substituted/);
  }
});

// r[verify kiln.cairn.workspace.scenario.register]
test("the register orders by lane, blockers, decisions and identity; filters and neighbors keep exact keys", () => {
  const entry = (change, lane, blockers, openDecisions, extra = {}) => ({ repository_identity: CAIRN_REPOSITORY, change, lane, lanes: laneCounts(lane), projects: ["kiln"], units: 1, current_work: 1, attempts: 0, active_attempts: 0, campaigns: ["campaign-1"], blockers, open_decisions: openDecisions, first_blocker: blockers ? { condition: "dispatch_recovery", subject: `${change}/unit`, owner: "operator" } : null, ...extra });
  const rows = cairn.indexRows({ schema: "kiln.cairn-workspace-index.v2", changes: [
    entry("ready-z", "ready", 1, 0), entry("attention-z", "attention", 2, 0), entry("attention-a", "attention", 2, 2),
    entry("attention-b", "attention", 2, 2), entry("attention-b", "attention", 99, 99), entry("unknown", "surprise", null, null),
    entry("ready-a", "ready", 0, 1), entry("ready-a", "ready", 0, 1, { repository_identity: "cd".repeat(32), projects: ["infra"] }), entry("bad/name", "attention", 1, 1),
  ] });
  assert.deepEqual(plain(rows.map((row) => row.change)), ["unknown", "attention-a", "attention-b", "attention-z", "ready-z", "ready-a", "ready-a"]);
  assert.equal(rows[1].firstBlocker.condition, "dispatch_recovery");
  assert.equal(cairn.lane("surprise").tone === "good", false);
  const names = (filters) => plain(rows.filter((row) => cairn.matches(row, filters)).map((row) => row.change));
  assert.deepEqual(names({ q: "INFRA" }), ["ready-a"]);
  assert.equal(names({ q: "campaign-1" }).length, rows.length);
  assert.equal(names({ q: "  " }).length, rows.length);
  assert.deepEqual(names({ lane: "attention", blocked: true, decisions: true }), ["unknown", "attention-a", "attention-b"]);
  assert.deepEqual(names({ lane: "ready", blocked: true }), ["ready-z"]);
  const counts = cairn.filterCounts(rows, { lane: "attention", blocked: true });
  assert.deepEqual([counts.all, counts.blocked, counts.decisions, counts.lanes.get("attention"), counts.lanes.get("ready")], [5, 4, 3, 4, 1]);
  const around = cairn.neighbors(rows, { lane: "attention", decisions: true }, `${CAIRN_REPOSITORY}/attention-a`);
  assert.deepEqual([around.previous.change, around.next.change, around.position, around.total, around.filtered], ["unknown", "attention-b", 2, 3, true]);
  const hidden = cairn.neighbors(rows, { lane: "attention" }, `${CAIRN_REPOSITORY}/ready-z`);
  assert.deepEqual([hidden.previous.change, hidden.next.change, hidden.filtered, hidden.listed], ["attention-z", "ready-a", false, true]);
  assert.equal(cairn.neighbors(rows, {}, `${CAIRN_REPOSITORY}/absent`).listed, false);
  assert.throws(() => cairn.indexRows({ schema: "kiln.cairn-workspace-index.v1", changes: [] }));
  assert.throws(() => cairn.indexRows(null));
});

// r[verify kiln.cairn.workspace.scenario.stages]
test("every facet value reads in its own words and anything else never reads as success", () => {
  const values = {
    readiness: ["ready", "blocked", "unknown", "not_admitted"],
    execution: ["not_started", "claimed", "executing", "candidate_produced", "verifying", "verified", "blocked", "failed", "cancelled", "superseded"],
    evidence: ["current_verified", "current_failed", "historical_only", "none"],
    integration: ["not_published", "prepared", "branch_published", "pull_request_published", "integrated", "source_reconciled"],
    lifecycle: ["receipt_observed", "not_observed"],
  };
  assert.deepEqual(Object.keys(cairn.facets), Object.keys(values));
  for (const [name, keys] of Object.entries(values)) {
    const labels = keys.map((key) => cairn.facet(name, key).label);
    assert.equal(new Set(labels).size, keys.length, name);
    assert.ok(labels.every((label) => !label.startsWith("Unrecognized")), name);
    for (const foreign of ["constructor", "__proto__", "accepted", "x".repeat(80)]) {
      const read = cairn.facet(name, foreign);
      assert.ok(read.label.startsWith("Unrecognized: "), `${name} ${foreign}`);
      assert.notEqual(read.tone, "good");
      assert.ok(read.label.length <= "Unrecognized: ".length + 48);
    }
    assert.equal(cairn.facet(name, null).label, "Not recorded");
  }
  assert.equal(cairn.facet("evidence", "historical_only").tone, "warning");
  assert.equal(cairn.facet("evidence", "current_verified").tone, "good");
  assert.ok(cairn.binding("stalest").label.startsWith("Unrecognized"));
  for (const key of ["missing_dependency", "no_acceptance_obligations", "dependency_cycle", "campaign_hold", "pending_effect", "dispatch_recovery", "stale_work_binding"]) {
    assert.ok(!cairn.condition(key).label.startsWith("Unrecognized"), key);
  }
});

// r[verify kiln.cairn.workspace.scenario.lanes]
test("each board lane stays distinct while an unknown lane never reads as progress", () => {
  const words = LANE_KEYS.map((key) => cairn.lane(key).label);
  assert.equal(new Set(words).size, LANE_KEYS.length);
  assert.ok(!words.some((word) => word.startsWith("Unrecognized")));
  for (const unknown of ["constructor", "__proto__", "completed"]) {
    assert.match(cairn.lane(unknown).label, /^Unrecognized/);
    assert.notEqual(cairn.lane(unknown).tone, "good");
  }
});

// r[verify kiln.cairn.workspace.scenario.dependencies]
test("dependency depth preserves cross-repository identity, absent work and every edge in cycles", () => {
  const otherRepo = "cd".repeat(32);
  const view = (identity, repository_identity, native_ids, state = "current") => ({ identity, repository_identity, native_ids, state });
  const unit = (native_id, dependencies = [], dependents = []) => ({ native_id, lane: "ready", current_work: [{ dependencies, dependents }] });
  const a = "chain/A", b = "chain/B", c = "chain/C";
  const detail = cairnDetail("chain", { units: [
    unit(a, [view("1", CAIRN_REPOSITORY, [b]), view("2", otherRepo, ["chain/A"]), view("3", CAIRN_REPOSITORY, [], "not_current")]),
    unit(b, [view("4", CAIRN_REPOSITORY, [c])]), unit(c),
  ] });
  const graph = cairn.dependencyGraph(detail);
  const node = (id) => graph.nodes.find((entry) => entry.id === id);
  const own = (id) => `unit:${CAIRN_REPOSITORY}/${id}`;
  assert.equal(node(own(c)).depth, 0);
  assert.equal(node(own(b)).depth, 1);
  assert.equal(node(own(a)).depth, 2);
  assert.equal(node(`unit:${otherRepo}/${a}`).kind, "external");
  assert.notEqual(node(`unit:${otherRepo}/${a}`).id, node(own(a)).id);
  assert.deepEqual([node("work:3").kind, node("work:3").state], ["work", "not_current"]);
  assert.equal(graph.edges.filter((edge) => edge.to === own(a)).length, 3);
  assert.equal(graph.cyclic, false);
  const multi = cairn.dependencyGraph(cairnDetail("chain", { units: [unit(a, [view("5", CAIRN_REPOSITORY, [b, c])]), unit(b), unit(c)] }));
  assert.equal(multi.edges.length, 2);
  const cycle = cairn.dependencyGraph(cairnDetail("chain", { units: [unit(a, [view("6", CAIRN_REPOSITORY, [b])]), unit(b, [view("7", CAIRN_REPOSITORY, [a])]), unit(c)] }));
  assert.equal(cycle.cyclic, true);
  assert.deepEqual(plain(cycle.nodes.filter((entry) => entry.cycle).map((entry) => entry.nativeId).sort()), [a, b]);
  assert.equal(cycle.nodes.find((entry) => entry.nativeId === c).cycle, false);
  const self = cairn.dependencyGraph(cairnDetail("chain", { units: [unit(a, [view("8", CAIRN_REPOSITORY, [a])])] }));
  assert.equal(self.cyclic, true);
  assert.deepEqual(plain(self.edges), [{ from: own(a), to: own(a) }]);
});

// A small DOM, enough for the page module to draw into a container.
class CairnNode {
  constructor(tag) {
    this.tagName = tag; this.children = []; this.attributes = new Map(); this.dataset = {}; this.listeners = new Map();
    this.isConnected = true; this.hidden = false; this.text = ""; this.style = {};
    this.classList = { add: (name) => { this.className = `${this.className || ""} ${name}`.trim(); }, contains: (name) => (this.className || "").split(/\s+/).includes(name) };
  }
  get textContent() { return [this.text, ...this.children.map((child) => child.textContent)].join(" "); }
  set textContent(value) { this.text = String(value); this.children = []; }
  get firstChild() { return this.children[0] || null; }
  get lastChild() { return this.children.at(-1) || null; }
  get parentElement() { return this.parent || null; }
  append(...nodes) { for (const node of nodes) { if (node === null || node === undefined) continue; const child = typeof node === "string" ? Object.assign(new CairnNode("#text"), { text: node }) : node; child.parent = this; this.children.push(child); } }
  prepend(...nodes) { for (const node of nodes.reverse()) { node.parent = this; this.children.unshift(node); } }
  replaceChildren(...nodes) { for (const child of this.children) child.isConnected = false; this.children = []; this.append(...nodes); }
  replaceWith(node) { if (!this.parent) return; const at = this.parent.children.indexOf(this); this.parent.children.splice(at, 1, node); node.parent = this.parent; this.isConnected = false; }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter((node) => node !== this); this.isConnected = false; }
  setAttribute(key, value) { this.attributes.set(key, String(value)); }
  getAttribute(key) { return this.attributes.has(key) ? this.attributes.get(key) : null; }
  removeAttribute(key) { this.attributes.delete(key); }
  addEventListener(type, handler) { this.listeners.set(type, [...(this.listeners.get(type) || []), handler]); }
  matches(selector) {
    const notHidden = selector.endsWith(":not([hidden])");
    const simple = notHidden ? selector.slice(0, -":not([hidden])".length) : selector;
    if (notHidden && this.hidden) return false;
    if (simple.startsWith(".")) return this.classList.contains(simple.slice(1));
    if (simple === "[data-focus-key]") return Boolean(this.dataset.focusKey);
    if (simple === "details[data-cairn-disclosure]") return this.tagName === "details" && Boolean(this.dataset.cairnDisclosure);
    return this.tagName === simple;
  }
  querySelectorAll(selector) {
    const parts = selector.split(" ");
    const walk = (root, part) => root.children.flatMap((child) => [...(child.matches(part) ? [child] : []), ...walk(child, part)]);
    return parts.reduce((parents, part) => parents.flatMap((parent) => walk(parent, part)), [this]);
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  contains(node) { return node === this || this.children.some((child) => child.contains(node)); }
  closest(selector) { return selector === "[hidden]" && this.hidden ? this : this.parent?.closest(selector) || null; }
  focus() { if (this.ownerDocument) this.ownerDocument.activeElement = this; else if (this.parent) this.parent.focusedChild = this; }
  scrollIntoView() {}
}
function cairnPage() {
  const document = { activeElement: null, title: "" };
  document.createElement = (tag) => Object.assign(new CairnNode(tag), { ownerDocument: document });
  document.createTextNode = (text) => Object.assign(new CairnNode("#text"), { text, ownerDocument: document });
  const page = { window: {}, document, location: { search: "" }, URLSearchParams, setTimeout, clearTimeout, crypto: webcrypto };
  vm.runInNewContext(asset("inventory.js"), page);
  vm.runInNewContext(asset("cairn.js"), page);
  return page;
}
function deferredApi() {
  const pending = [];
  const api = (target) => new Promise((resolve, reject) => pending.push({ target, resolve, reject }));
  const answer = (target, value) => { for (const entry of pending.filter((item) => item.target === target)) { pending.splice(pending.indexOf(entry), 1); entry.resolve(value); } };
  return { api, answer };
}
const cairnIndex = (...changes) => ({ schema: "kiln.cairn-workspace-index.v2", truncated: false, snapshot: {}, non_claims: [],
  changes: changes.map((change) => ({ repository_identity: CAIRN_REPOSITORY, change, lane: "awaiting", lanes: laneCounts(), open_decisions: 0, first_blocker: null, projects: ["kiln"], units: 1, current_work: 0, attempts: 0, active_attempts: 0, campaigns: [], blockers: 0 })) });

// r[verify kiln.cairn.workspace.scenario.reload]
test("a late response for an earlier selection never draws over the URL's selection", async () => {
  const page = cairnPage();
  const { api, answer } = deferredApi();
  const container = new CairnNode("div");
  const route = (change) => ({ cairn_repository: CAIRN_REPOSITORY, cairn_change: change });
  const first = page.window.KilnCairn.load(container, { api, navigate() {}, route: route("first") });
  const second = page.window.KilnCairn.load(container, { api, navigate() {}, route: route("second") });
  answer("/api/implementation/cairn", cairnIndex("first", "second"));
  answer(`/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=second`, cairnDetail("second"));
  await second;
  answer(`/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=first`, cairnDetail("first"));
  await first;
  const text = container.textContent;
  assert.match(text, /unit-of-second/);
  assert.doesNotMatch(text, /unit-of-first/);
  assert.equal(page.document.title, "Kiln · Cairn changes · second");

  // A detail for another change is refused, never drawn in place of the selection.
  const crossed = deferredApi();
  const load = page.window.KilnCairn.load(container, { api: crossed.api, navigate() {}, route: route("wanted") });
  // A new selection must remove the old change and its actions before its read settles.
  assert.doesNotMatch(container.textContent, /unit-of-second/);
  crossed.answer("/api/implementation/cairn", cairnIndex("wanted", "other"));
  crossed.answer(`/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=wanted`, cairnDetail("other"));
  await load;
  assert.match(container.textContent, /No other change was substituted/);
  assert.doesNotMatch(container.textContent, /other\/unit-of-other/);

  // Clearing invalidates a read still in flight.
  const cleared = deferredApi();
  const pending = page.window.KilnCairn.load(container, { api: cleared.api, navigate() {}, route: route("second") });
  page.window.KilnCairn.clear();
  cleared.answer("/api/implementation/cairn", cairnIndex("second"));
  cleared.answer(`/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=second`, cairnDetail("second"));
  await pending;
  assert.equal(container.children.length, 0);
});

// r[verify kiln.cairn.workspace]
test("an unconfigured store and an empty store say so instead of drawing nothing", async () => {
  const page = cairnPage();
  const container = new CairnNode("div");
  const unconfigured = Object.assign(new Error("cairn_workspace_input_not_configured: Request could not be admitted"), { status: 503 });
  await page.window.KilnCairn.load(container, { api: async () => { throw unconfigured; }, navigate() {}, route: {} });
  assert.match(container.textContent, /No implementation work database is configured/);
  assert.match(container.textContent, /not proof that no Cairn change exists/);
  await page.window.KilnCairn.load(container, { api: async () => cairnIndex(), navigate() {}, route: {} });
  assert.match(container.textContent, /No Cairn change is named yet/);
});

// r[verify kiln.cairn.workspace.scenario.blockers]
test("ranked blockers keep their structured subject, owner and campaign targets", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const blockers = [
    { condition: "dispatch_recovery", subject_kind: "unit", subject: "wanted/unit-of-wanted", operation: "reconcile", owner: "dispatcher", observation: "a".repeat(64), detail: "not a command" },
    { condition: "campaign_hold", subject_kind: "member", subject: "wanted/*", campaign_id: "selected", operation: "wait", owner: "reviewer", observation: "b".repeat(64), detail: "resolve_decision is only displayed text" },
  ];
  const detail = cairnDetail("wanted", { blockers, memberships: [{ campaign_id: "selected", member: "wanted/*", pending_effects: [] }], overview: { ...cairnDetail("wanted").overview, blockers: 2, first_blocker: blockers[0] } });
  await page.window.KilnCairn.load(root, { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" }, navigate() {}, async api(path) { return path === "/api/implementation/cairn" ? { ...cairnIndex("wanted"), changes: [{ ...cairnIndex("wanted").changes[0], blockers: 2, first_blocker: blockers[0] }] } : detail; } });
  const items = root.querySelectorAll(".cairn-blocker");
  assert.equal(items.length, 2);
  assert.match(items[0].textContent, /Unknown dispatch outcome/);
  assert.match(items[1].textContent, /Campaign hold/);
  assert.equal(items[0].querySelector(".cairn-subject").tagName, "a");
  assert.equal(items[1].querySelector(".cairn-subject").tagName, "button");
  assert.equal(items[1].querySelector(".cairn-campaign-link").href.includes("worker_campaign=selected"), true);
  assert.equal(root.querySelectorAll(".cairn-decision form").length, 0);
  page.window.KilnCairn.clear();
});

// r[verify kiln.cairn.workspace.scenario.reload]
test("missing changes and units remain named, including an empty detail, without opening a neighbor", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const route = (unit) => ({ cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted", cairn_unit: unit });
  const api = async (path) => path === "/api/implementation/cairn" ? cairnIndex("other") : cairnDetail("wanted", { units: [] });
  await page.window.KilnCairn.load(root, { api, navigate() {}, route: route("wanted/missing") });
  assert.match(root.textContent, /wanted\/missing.*does not list/);
  assert.equal(root.querySelectorAll(".cairn-unit-toggle").length, 0);
  await page.window.KilnCairn.load(root, { api, navigate() {}, route: route("other/unit") });
  assert.match(root.textContent, /invalid unit/);
  assert.equal(root.querySelectorAll(".cairn-unit-toggle").length, 0);
  await page.window.KilnCairn.load(root, { api: async (path) => { if (path === "/api/implementation/cairn") return cairnIndex("other"); throw Object.assign(new Error("not found"), { status: 404 }); }, navigate() {}, route: route("") });
  assert.match(root.textContent, /wanted is not named/);
  assert.doesNotMatch(root.textContent, /other\/unit-of-other/);
});

// r[verify kiln.cairn.workspace.scenario.reload]
test("an exact unit URL opens only its named row and preserves filters on the detail", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const route = { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted", cairn_unit: "wanted/two", cairn_q: "wanted", cairn_lane: "ready", cairn_view: "board" };
  const units = ["wanted/one", "wanted/two"].map((native_id) => ({ ...cairnDetail("wanted").units[0], native_id, unit: native_id.split("/")[1] }));
  const api = async (path) => path === "/api/implementation/cairn" ? cairnIndex("wanted") : cairnDetail("wanted", { units, overview: { ...cairnDetail("wanted").overview, units: 2, lanes: { ...laneCounts(), awaiting: 2 } } });
  await page.window.KilnCairn.load(root, { route, api, navigate() {} });
  const toggles = root.querySelectorAll(".cairn-unit-toggle");
  assert.deepEqual(toggles.map((node) => [node.dataset.unit, node.getAttribute("aria-expanded")]), [["wanted/one", "false"], ["wanted/two", "true"]]);
  assert.equal(root.querySelector(".cairn-unit-detail-row").hidden, true);
  assert.equal(root.querySelectorAll(".cairn-unit-detail-row")[1].hidden, false);
  assert.equal(root.querySelector(".cairn-register").querySelector("input").value, "wanted");
  page.window.KilnCairn.clear();
});

// r[verify kiln.cairn.workspace.scenario.register]
test("list arrows skip the hidden board and filters narrow locally without another read", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  let reads = 0;
  const navigations = [];
  const ctx = { route: {}, api: async () => { reads += 1; return cairnIndex("alpha", "beta"); }, navigate: (patch) => navigations.push(patch) };
  await page.window.KilnCairn.load(root, ctx);
  const body = root.querySelector(".cairn-register-body");
  const links = body.querySelectorAll("tr:not([hidden]) .cairn-change-link");
  assert.equal(links.length, 2);
  links[0].focus();
  let prevented = false;
  body.listeners.get("keydown")[0]({ key: "ArrowDown", preventDefault() { prevented = true; } });
  assert.equal(prevented, true);
  assert.equal(page.document.activeElement, links[1]);
  const search = root.querySelector("input");
  search.value = "beta";
  search.listeners.get("input")[0]();
  assert.match(root.querySelector(".cairn-count").textContent, /1 of 2 changes/);
  assert.equal(reads, 1);
  root.querySelector("form").listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(navigations.at(-1).cairn_q, "beta");
  assert.equal(reads, 1);
  page.window.KilnCairn.clear();
});

// r[verify kiln.cairn.workspace.scenario.decisions]
test("inline actions require exact decision, exact member and the requested campaign's inbox", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const hex = (digit) => digit.repeat(64);
  const member = (name) => ({ campaign_id: "selected", member: name, work_binding: "not_bound", stage: "planning", admission: "open", pending_effects: [], remaining_obligations: [], blockers: [] });
  const decision = (identity, memberName, state = "outstanding") => ({ campaign_id: "selected", identity: hex(identity), member: memberName, category: "required_review", state, question: `${identity} question`, options: ["yes", "no"], scope: "current", priority: 1 });
  const own = decision("a", "wanted/one");
  const issue = decision("b", "wanted/two", "unissued");
  const wrongMember = decision("c", "wanted/one");
  const foreign = decision("d", "other/one");
  const projected = cairnDetail("wanted", { memberships: [member("wanted/one"), member("wanted/two")], decisions: [own, issue, wrongMember, foreign], overview: { ...cairnDetail("wanted").overview, open_decisions: 4 } });
  const inboxDecision = (entry, actions, memberName = entry.member) => ({ ...entry, member: memberName, work_identity: hex("e"), policy_identity: hex("f"), candidate_identity: hex("1"), review_roles: [], actions });
  const inbox = { schema: "kiln.campaign-inbox.v1", campaign_id: "selected", store_version: 17, campaign_identity: hex("2"), principal: "local:operator", admission: "open", operations: ["pause", "cancel"], non_claims: [], decisions: [
    inboxDecision(foreign, ["resolve"]), inboxDecision(wrongMember, ["resolve"], "wanted/two"), inboxDecision(own, ["resolve"]), inboxDecision(issue, ["issue"]),
  ] };
  const calls = [];
  const ctx = { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" }, navigate() {}, async api(path, request) {
    calls.push({ path, request });
    if (path === "/api/implementation/cairn") return cairnIndex("wanted");
    if (path.includes("/cairn/detail?")) return projected;
    if (path.includes("/campaigns/inbox?")) return inbox;
    return { replayed: false, command: request.body.command, store_version: 18, admission: "open", request_id: request.body.request_id };
  } };
  await page.window.KilnCairn.load(root, ctx);
  await new Promise((resolve) => setTimeout(resolve, 0));
  const card = (id) => root.querySelectorAll(".cairn-decision").find((node) => node.dataset.decision === hex(id));
  assert.equal(card("a").querySelectorAll("form").length, 1);
  assert.equal(card("b").querySelectorAll("button").filter((node) => node.textContent === "Issue to reviewers").length, 1);
  assert.equal(card("c").querySelectorAll("form").length, 0);
  assert.equal(card("d").querySelectorAll("form").length, 0);
  assert.equal(root.querySelectorAll("button").some((node) => /Pause|Cancel|Start|Continue/.test(node.textContent)), false);
  const form = card("a").querySelector("form");
  form.querySelectorAll("input").find((node) => node.value === "no").checked = true;
  form.listeners.get("submit")[0]({ preventDefault() {} });
  await new Promise((resolve) => setTimeout(resolve, 0));
  const command = calls.find((call) => call.request?.method === "POST");
  assert.equal(command.path, "/api/implementation/campaigns/commands");
  assert.deepEqual(plain(command.request.body), { command: "resolve_decision", campaign_id: "selected", expected_version: 17, decision_identity: hex("a"), work_identity: hex("e"), policy_identity: hex("f"), candidate_identity: hex("1"), answer: "no", request_id: command.request.body.request_id });
  assert.ok(command.request.body.request_id);
  page.window.KilnCairn.clear();
});

// The inline fleet panel reads independently of the Cairn workspace and never blocks its detail.
test("a selected Cairn change shows fleet claims without blocking on the fleet read", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const claim = {
    identity: "a".repeat(64), fence: 1, purpose: "inline fleet claim",
    holder: { tool: "omp", host: "britton-desktop", session: "s" },
    scope: { target: { repository: "brittonr/kiln" }, path: "." },
  };
  const ctx = {
    route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" }, navigate() {},
    async api(path) {
      if (path === "/api/implementation/cairn") return cairnIndex("wanted");
      if (path.includes("/cairn/detail?")) return cairnDetail("wanted");
      if (path === "/api/fleet/claims") return { claims: [claim] };
      if (path === "/api/fleet/attempts") return { attempts: [] };
      throw new Error(`unexpected ${path}`);
    },
  };
  await page.window.KilnCairn.load(root, ctx);
  await new Promise((resolve) => setTimeout(resolve, 0));
  const panel = root.querySelector(".cairn-fleet");
  assert.ok(panel);
  assert.match(panel.textContent, /Fleet coordination/);
  assert.match(panel.textContent, /inline fleet claim/);
  assert.match(panel.textContent, /brittonr\/kiln/);
  page.window.KilnCairn.clear();
});

// r[verify kiln.cairn.workspace.scenario.reload]
test("an in-flight index renders the latest local filters but never a changed selection", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const pending = deferredApi();
  const ctx = { route: {}, api: pending.api, navigate() {} };
  const loading = page.window.KilnCairn.load(root, ctx);
  ctx.route = { cairn_q: "beta", cairn_view: "board" };
  pending.answer("/api/implementation/cairn", cairnIndex("alpha", "beta"));
  await loading;
  assert.match(root.querySelector(".cairn-count").textContent, /1 of 2 changes/);
  assert.equal(root.querySelector(".cairn-board").hidden, false);
  const next = deferredApi();
  const old = page.window.KilnCairn.load(root, { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "alpha" }, api: next.api, navigate() {} });
  const current = { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "beta" }, api: async (path) => path === "/api/implementation/cairn" ? cairnIndex("beta") : cairnDetail("beta"), navigate() {} };
  await page.window.KilnCairn.load(root, current);
  next.answer("/api/implementation/cairn", cairnIndex("alpha"));
  next.answer(`/api/implementation/cairn/detail?repository=${CAIRN_REPOSITORY}&change=alpha`, cairnDetail("alpha"));
  await old;
  assert.match(root.textContent, /unit-of-beta/);
  assert.doesNotMatch(root.textContent, /unit-of-alpha/);
  page.window.KilnCairn.clear();
});

// r[verify kiln.cairn.workspace.scenario.decisions]
test("unavailable or crossed campaign inbox leaves the decision read-only", async () => {
  const page = cairnPage();
  const root = new CairnNode("div");
  const id = "e".repeat(64);
  const decision = { campaign_id: "selected", identity: id, member: "wanted/one", state: "outstanding", category: "required_review", question: "Ready?", options: ["yes"], scope: "current" };
  const detail = cairnDetail("wanted", { memberships: [{ campaign_id: "selected", member: "wanted/one", pending_effects: [] }], decisions: [decision] });
  const base = { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" }, navigate() {} };
  const api = async (path) => {
    if (path === "/api/implementation/cairn") return cairnIndex("wanted");
    if (path.includes("/cairn/detail?")) return detail;
    throw Object.assign(new Error("campaign_controls_not_configured"), { status: 503 });
  };
  await page.window.KilnCairn.load(root, { ...base, api });
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(root.querySelectorAll(".cairn-decision form").length, 0);
  assert.match(root.textContent, /read-only|not configured/i);
  assert.ok(root.querySelector(".cairn-answer-note a"));
  const crossed = { schema: "kiln.campaign-inbox.v1", campaign_id: "other", store_version: 2, campaign_identity: "a".repeat(64), principal: "local:operator", admission: "open", operations: [], non_claims: [], decisions: [{ ...decision, policy_identity: "a".repeat(64), work_identity: null, candidate_identity: null, review_roles: [], actions: ["resolve"] }] };
  page.window.KilnCairn.clear();
  await page.window.KilnCairn.load(root, { ...base, api: async (path) => path.includes("/campaigns/inbox?") ? crossed : api(path) });
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(root.querySelectorAll(".cairn-decision form").length, 0);
  assert.match(root.textContent, /different campaign/);
  page.window.KilnCairn.clear();
});

// r[verify kiln.review.session_traces.scenario.surface]
// Exercise the actual component used for the selected revision, not its source.
test("session pointers render as inert, untrusted historical context only", () => {
  const page = {
    window: {}, Node: CairnNode,
    document: { createElement: tag => new CairnNode(tag), querySelectorAll: () => [] },
    fetch() { assert.fail("trace context must never fetch"); },
    location: { assign() { assert.fail("trace context must never navigate"); } },
  };
  vm.runInNewContext(asset("ui.js"), page);
  vm.runInNewContext(asset("code-review.js"), page);
  const render = page.window.KilnCodeReview.sessionTraces.render;
  assert.equal(render(undefined), null);
  assert.equal(render({ id: "disabled-revision" }), null, "disabled revisions add no DOM");
  const observation = { commit: "old-commit", status: "attached", note_digest: "exact-pointer-note-digest", traces: [
    { identity: "kiln-pointer-id", source_kind: "git_notes_traces", external_id: "<script>alert('untrusted')</script>", shared_url_host: "traces.com", shared_url: "https://traces.com/private?secret=hidden", body: "private session body" },
    { identity: "second-kiln-id", source_kind: "git_notes_traces", external_id: "https://host.invalid/not-a-link" },
  ] };
  const selected = { id: "old-selected-revision", session_traces: observation, authorship: { kind: "agent", harness: "not-a-trace" }, submitted_by: { principal: "not-a-trace-principal" } };
  const card = render(selected);
  assert.match(card.textContent, /Untrusted historical context/);
  assert.match(card.textContent, /not verified authorship, attested submission, CI evidence or merge authority/);
  for (const value of ["old-selected-revision", "old-commit", "exact-pointer-note-digest", "kiln-pointer-id", "second-kiln-id", "traces.com", "<script>alert('untrusted')</script>", "https://host.invalid/not-a-link"]) assert.ok(card.textContent.includes(value), value);
  assert.match(card.textContent, /not a session-body digest/);
  assert.doesNotMatch(card.textContent, /private\?secret|private session body|not-a-trace/);
  const descendants = root => [root, ...root.children.flatMap(descendants)];
  for (const element of descendants(card)) {
    assert.ok(!["a", "iframe", "script", "img", "button"].includes(element.tagName));
    assert.equal(element.getAttribute("href"), null);
    assert.equal(element.getAttribute("src"), null);
    assert.equal(element.listeners.size, 0);
  }
  const absent = render({ id: "no-note", session_traces: { commit: "no-note-commit", status: "absent" } });
  assert.match(absent.textContent, /No trace note was present/);
  assert.doesNotMatch(absent.textContent, /observation unavailable/);
  for (const [reason, message] of [
    ["unsupported", "does not support"], ["malformed", "malformed or incompatible"],
    ["oversized", "exceeded the capture bound"], ["too_many", "too many distinct pointers"],
    ["provider_unavailable", "could not observe"],
  ]) {
    const unavailable = render({ id: reason, session_traces: { commit: "exact", status: "unavailable", reason } });
    assert.ok(unavailable.textContent.includes(message));
    assert.match(unavailable.textContent, /Trace observation unavailable/);
    assert.doesNotMatch(unavailable.textContent, /No trace note was present/);
  }
  const next = render({ id: "new-selected-revision", session_traces: { commit: "new-commit", status: "absent" } });
  assert.match(next.textContent, /new-selected-revision/);
  assert.doesNotMatch(next.textContent, /old-selected-revision|old-commit|kiln-pointer-id/);
});

// Agent revisions in the Cairn workspace (Lattice #24 and #25): the projection's words, drawn on the change page.
const AGENT_REVISION = "a1".repeat(32);
const OTHER_REVISION = "b2".repeat(32);
const AGENT_QUALIFICATION = `blake3:${"f8".repeat(32)}`;
const agentSlot = (state, value = null, owner = null) => ({ state, value, owner });
const agentDefinition = (id, extra = {}) => ({
  reference: { source: "lattice-agents", id }, observation: "current", profile_revision: AGENT_REVISION, management: "managed",
  desired: agentSlot("recorded", AGENT_REVISION), prepared: agentSlot("unknown", null, "kiln#49"), qualified: agentSlot("not_recorded"),
  selected_for_new_sessions: agentSlot("recorded", AGENT_REVISION), rollout_generation: 2, task_families_state: "declared",
  task_families: [{ task_family: "rust-implementation", declared: true, state: "fixture", qualification: { reference: AGENT_QUALIFICATION, evidence_class: "task_family_evaluation", provenance: "fixture", counts_as_live: false } }],
  tools: { state: "unknown", names: [] }, harnesses: { state: "declared", names: ["omp"] }, output_contracts: { state: "none", names: [] }, models: { state: "unknown", names: [] },
  authority: "unknown", wait_reasons: [{ code: "revision_fact_unknown", subject: "fact:revisions.prepared", owner: "kiln#49" }], workers: 2, ...extra,
});
const agentWorker = (id, extra = {}) => ({
  reference: { source: "lattice-workers", id }, observation: "current", definition: { source: "lattice-agents", id: "definition-a" }, lifecycle: "executing",
  pinned_revision: AGENT_REVISION, selection: "matches_selected", work: "other_work", attempt: null, fence: null, wait_reasons: [],
  pin: { state: "recorded", owner: null, qualification: AGENT_QUALIFICATION, rollout_generation: 2, identity: `blake3:${"9e".repeat(32)}`, status: "live", blocked: false }, ...extra,
});
// Workflow revisions as the projection words them: identities, the paired worker revision, counted pins.
const WORKFLOW_REVISION = "c3".repeat(32);
const OLD_WORKFLOW_REVISION = "d4".repeat(32);
const workflowRevision = (revision, worker, observed, extra = {}) => ({
  revision,
  workflow: { compiler_revision: "lattice.workflow-intent-compiler.v1", compiler_artifact: `b3:${"e5".repeat(32)}`, plan: `b3:${"f6".repeat(32)}`,
    graph: `blake3:${"a7".repeat(32)}`, selection: `b3:${"b8".repeat(32)}`, obligations: `b3:${"c9".repeat(32)}`, policy: `b3:${"da".repeat(32)}` },
  worker: { definition: { source: "lattice-agents", id: "definition-a" }, revision: worker, bundle: `blake3:${"eb".repeat(32)}`, observed }, ...extra,
});
const workflowSlot = (state, revision = null) => ({ state, revision });
const workflowDefinition = (extra = {}) => ({
  reference: { source: "lattice-workflows", id: `blake3:${"fc".repeat(32)}` }, observation: "current",
  revisions: [
    workflowRevision(WORKFLOW_REVISION, OTHER_REVISION, "other_revision"),
    // Fields the projection never defines are never drawn, even when a bad response carries them.
    workflowRevision(OLD_WORKFLOW_REVISION, AGENT_REVISION, "profile_revision", { guidance_identity: "restricted-guidance", context_identity: "restricted-context" }),
  ],
  desired: workflowSlot("recorded", WORKFLOW_REVISION),
  last_usable: workflowSlot("not_recorded"),
  selected_for_new_sessions: workflowSlot("recorded", WORKFLOW_REVISION),
  rollout_generation: 2, selection_kind: "promotion", qualification: `blake3:${"0d".repeat(32)}`, pinned_state: "recorded",
  pinned: [
    { revision: WORKFLOW_REVISION, status: "live", blocked: false, sessions: 2, selection: "matches_selected" },
    { revision: OLD_WORKFLOW_REVISION, status: "closed", blocked: true, sessions: 1, selection: "differs_from_selected", session: "restricted-session", name: "restricted-name" },
  ],
  ...extra,
});
const admittedWorkflows = (extra = {}) => cairnWorkflows({
  source: "admitted", sources: [{ source: "lattice-workflows", producer: "lattice", kind: "workflow_revisions", state: "current", reason: null }],
  definitions: [workflowDefinition()], ...extra,
});
const admittedAgents = (extra = {}) => cairnAgents({
  source: "admitted", inventory_identity: "d".repeat(64), evaluated_at_ms: 1_795_237_895_000, synthetic: true,
  sources: [{ source: "lattice-agents", producer: "lattice", kind: "agents", state: "current", reason: null }],
  definitions: [
    agentDefinition("definition-a"),
    agentDefinition("definition-unmanaged", { management: "unmanaged", desired: agentSlot("unmanaged"), prepared: agentSlot("unmanaged"), qualified: agentSlot("unmanaged"),
      selected_for_new_sessions: agentSlot("unmanaged"), rollout_generation: null, task_families: [], task_families_state: "unknown", wait_reasons: [], workers: 0 }),
    agentDefinition("definition-unpublished", { management: "unpublished", desired: agentSlot("unpublished"), prepared: agentSlot("unpublished"), qualified: agentSlot("unpublished"),
      selected_for_new_sessions: agentSlot("unpublished"), rollout_generation: null, task_families: [], task_families_state: "unknown", wait_reasons: [], workers: 0 }),
  ],
  workers: [
    agentWorker("worker-on-change", { work: "this_change", attempt: "e".repeat(64), fence: 7 }),
    agentWorker("worker-pinned-old", { pinned_revision: OTHER_REVISION, selection: "differs_from_selected", pin: { ...agentWorker("x").pin, blocked: true },
      wait_reasons: [{ code: "revision_not_admitted", subject: "fact:revisions", owner: "lattice#25" }],
      // Fields the projection never defines, and another work's attempt a bad response might carry, are never drawn.
      attempt: "restricted-other-attempt", fence: 3, session: "restricted-session", workspace: "/home/restricted/workspace", command: "restricted command", prompt: "restricted prompt" }),
    agentWorker("worker-unknown-pin", { pin: { state: "unknown", owner: "lattice#25", qualification: null, rollout_generation: null, identity: null, status: null, blocked: null }, selection: "no_selection" }),
  ],
  workflow_revisions: admittedWorkflows(),
  ...extra,
});
async function drawAgents(agents) {
  const page = cairnPage();
  const root = new CairnNode("div");
  const detail = cairnDetail("wanted", { agents });
  await page.window.KilnCairn.load(root, { route: { cairn_repository: CAIRN_REPOSITORY, cairn_change: "wanted" }, navigate() {},
    async api(path) { return path === "/api/implementation/cairn" ? cairnIndex("wanted") : detail; } });
  const section = root.querySelectorAll(".cairn-section").find((node) => node.getAttribute("aria-labelledby") === "cairn-agents-title");
  page.window.KilnCairn.clear();
  return section;
}

// r[verify kiln.cairn.workspace.agents.scenario.slots]
// r[verify kiln.cairn.workspace.agents.scenario.provenance]
test("revision slots keep five distinct meanings and fixture evidence never reads as live", () => {
  const states = ["recorded", "not_recorded", "unknown", "unmanaged", "unpublished"];
  const words = states.map((state) => cairn.slotStates.get(state));
  assert.equal(new Set(words.map((entry) => entry.label)).size, 5);
  assert.ok(words.every((entry) => entry.tone !== "good"), "no revision fact reads as success");
  assert.equal(cairn.slotText(agentSlot("unknown", null, "kiln#49")), "Unknown · owner kiln#49");
  assert.equal(cairn.slotText(agentSlot("unknown")), "Unknown · no owner named");
  assert.equal(cairn.slotText(agentSlot("unmanaged")), "Unmanaged");
  assert.equal(cairn.slotText(agentSlot("recorded", AGENT_REVISION)), `Recorded: ${AGENT_REVISION}`);
  assert.equal(cairn.slotText(agentSlot("unpublished", null, "lattice#25")), "Not published · owner lattice#25");
  assert.match(cairn.slotText(agentSlot("surprise")), /^Unrecognized/);
  const family = (provenance, counts_as_live) => ({ qualification: { reference: AGENT_QUALIFICATION, evidence_class: "task_family_evaluation", provenance, counts_as_live } });
  assert.equal(cairn.familyProvenance(family("live", true)).tone, "good");
  for (const unproven of [family("fixture", false), family("fixture", true), family("live", false), family("simulated", true), {}]) {
    const entry = cairn.familyProvenance(unproven);
    assert.notEqual(entry.tone, "good");
    assert.doesNotMatch(entry.label, /^Live/);
  }
  assert.equal(cairn.familyProvenance(family("fixture", true)).label, "Fixture evidence, not live");
  assert.equal(cairn.familyProvenance({}).label, "Unqualified");
  assert.equal(cairn.admitAgents(cairnAgents()), true);
  for (const malformed of [null, cairnAgents({ source: "empty" }), cairnAgents({ definitions: null }), cairnAgents({ workflow_revisions: null }),
    cairnAgents({ workflow_revisions: cairnWorkflows({ source: "unpublished" }) }), cairnAgents({ workflow_revisions: cairnWorkflows({ definitions: null }) })]) {
    assert.equal(cairn.admitAgents(malformed), false);
  }
  assert.equal(new Set([...cairn.workerPairings.values()].map((entry) => entry.label)).size, 3);
  assert.ok([...cairn.workerPairings.values()].every((entry) => entry.tone !== "good"));
});

// r[verify kiln.cairn.workspace.agents]
// r[verify kiln.cairn.workspace.agents.scenario.selection]
// r[verify kiln.cairn.workspace.agents.scenario.redaction]
test("the change page shows workflow revisions apart from selected and pinned worker revisions without restricted context", async () => {
  const section = await drawAgents(admittedAgents());
  assert.ok(section, "the agents section is drawn");
  const text = section.textContent;
  assert.match(text, /Workflow revisions/);
  const workflow = section.querySelectorAll(".cairn-workflow");
  assert.equal(workflow.length, 1, "one workflow definition card");
  const card = workflow[0].textContent;
  assert.match(card, /Desired.*Recorded.*Pairs with another worker revision/s);
  assert.match(card, /Last usable.*None recorded/s);
  assert.match(card, /Selected by.*Promotion/s);
  assert.match(card, /2 sessions.*Live session.*Pinned to the selected revision/s);
  assert.match(card, /1 session.*Session closed.*Pinned to another revision.*Pin blocked.*Pairs with the projected worker revision/s);
  for (const label of ["Plan", "Graph", "Compiler artifact", "Cairn selection", "Cairn obligations", "Policy", "Worker revision", "Worker bundle"]) assert.match(card, new RegExp(label));
  assert.equal(section.querySelectorAll(".cairn-agent-worker").filter((node) => /Workflow/.test(node.textContent)).length, 0, "workflow revisions stay out of worker revisions");
  assert.match(text, /lattice-workflows \(workflow_revisions\): Current/);
  assert.match(text, /Agent definitions/);
  assert.match(text, /Worker revisions/);
  const slots = section.querySelectorAll(".cairn-slot").map((node) => node.dataset.slotState);
  for (const state of ["recorded", "unknown", "not_recorded", "unmanaged", "unpublished"]) assert.ok(slots.includes(state), state);
  assert.match(text, /Owner kiln#49/);
  assert.match(text, /Fixture evidence, not live/);
  assert.doesNotMatch(text, /Live evidence/);
  assert.match(text, /revision_fact_unknown · fact:revisions\.prepared · owner kiln#49/);
  assert.match(text, /revision_not_admitted · fact:revisions · owner lattice#25/);
  const workers = section.querySelectorAll(".cairn-agent-worker");
  assert.deepEqual(workers.map((node) => node.dataset.work), ["this_change", "other_work", "other_work"]);
  assert.match(workers[0].textContent, /On this change.*Pinned to the selected revision/s);
  assert.match(workers[0].textContent, /Attempt on this change/);
  assert.match(workers[1].textContent, /Pinned to another revision.*Pin blocked/s);
  assert.match(workers[2].textContent, /Pin: Unknown.*Owner lattice#25/s);
  assert.doesNotMatch(workers[1].textContent, /Attempt on this change/);
  assert.doesNotMatch(text, /restricted/);
  assert.match(text, /Synthetic inventory: fixture observations, not a live fleet/);
});

// r[verify kiln.cairn.workspace.agents.scenario.source]
test("an unconfigured or unavailable inventory is named and never drawn as an empty one", async () => {
  const unconfigured = await drawAgents(cairnAgents());
  assert.match(unconfigured.textContent, /No resource inventory is configured.*not proof that none exist/s);
  assert.match(unconfigured.textContent, /Agent, worker and workflow revisions are not known here/);
  assert.doesNotMatch(unconfigured.textContent, /Agent definitions|lists no agent definition/);
  const refused = await drawAgents(cairnAgents({ source: "unavailable", reason: "inventory_principal_denied" }));
  assert.match(refused.textContent, /unavailable \(inventory_principal_denied\).*nothing was substituted/s);
  assert.equal(refused.querySelectorAll(".cairn-agent").length, 0);
  const empty = await drawAgents(admittedAgents({ definitions: [], workers: [], synthetic: false }));
  assert.match(empty.textContent, /lists no agent definition/);
  assert.match(empty.textContent, /A definition is not a running worker/);
  assert.doesNotMatch(empty.textContent, /Synthetic inventory/);
});

// r[verify kiln.cairn.workspace.agents.scenario.workflow_source]
test("missing or refused workflow revisions are named and leave agent and worker revisions drawn", async () => {
  const unconfigured = await drawAgents(admittedAgents({ workflow_revisions: cairnWorkflows() }));
  assert.match(unconfigured.textContent, /No workflow revision source is configured.*not proof that none exist/s);
  assert.equal(unconfigured.querySelectorAll(".cairn-workflow").length, 0);
  assert.equal(unconfigured.querySelectorAll(".cairn-agent").length, 3);
  assert.equal(unconfigured.querySelectorAll(".cairn-agent-worker").length, 3);
  const refused = await drawAgents(admittedAgents({ workflow_revisions: cairnWorkflows({ source: "unavailable", reason: "lattice_workflow_slots_unsupported_schema",
    sources: [{ source: "lattice-workflows", producer: "lattice", kind: "workflow_revisions", state: "unavailable", reason: "lattice_workflow_slots_unsupported_schema" }] }) }));
  assert.match(refused.textContent, /Workflow revisions are unavailable \(lattice_workflow_slots_unsupported_schema\).*agent and worker revisions are unaffected/s);
  assert.equal(refused.querySelectorAll(".cairn-workflow").length, 0);
  assert.equal(refused.querySelectorAll(".cairn-agent").length, 3);
  assert.match(refused.textContent, /lattice-workflows \(workflow_revisions\): Unavailable · lattice_workflow_slots_unsupported_schema/);
  const empty = await drawAgents(admittedAgents({ workflow_revisions: admittedWorkflows({ definitions: [] }) }));
  assert.match(empty.textContent, /lists no workflow definition/);
  const unknown = await drawAgents(admittedAgents({ workflow_revisions: admittedWorkflows({ definitions: [workflowDefinition({ desired: workflowSlot("unknown"), pinned_state: "unknown", pinned: [] })] }) }));
  assert.match(unknown.querySelectorAll(".cairn-workflow")[0].textContent, /Desired.*Unknown.*Pinned sessions.*Unknown/s);
});

// r[verify kiln.cairn.workspace.agents.scenario.workflow_source]
test("a refused workflow source beside an admitted one is named, and an unlisted revision never reads as a match", async () => {
  const partial = await drawAgents(admittedAgents({ workflow_revisions: admittedWorkflows({ sources: [
    { source: "lattice-workflows", producer: "lattice", kind: "workflow_revisions", state: "current", reason: null },
    { source: "lattice-workflows-b", producer: "lattice", kind: "workflow_revisions", state: "unavailable", reason: "lattice_workflow_slots_malformed" },
  ] }) }));
  assert.match(partial.textContent, /Some workflow revision sources are not current.*lattice-workflows-b \(workflow_revisions\): Unavailable · lattice_workflow_slots_malformed/s);
  assert.equal(partial.querySelectorAll(".cairn-workflow").length, 1);
  const unlisted = await drawAgents(admittedAgents({ workflow_revisions: admittedWorkflows({ definitions: [workflowDefinition({ revisions: [] })] }) }));
  const card = unlisted.querySelectorAll(".cairn-workflow")[0].textContent;
  assert.match(card, /Revision not listed/);
  assert.doesNotMatch(card, /Pairs with/);
});

test("KilnRunbooks helpers validate targets, typed params, routes, and errors", () => {
  const rb = context.window.KilnRunbooks;
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "  flake-update  " })), { runbook: "flake-update" });
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "../etc/passwd" })), { runbook: "" });
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "a".repeat(64) })), { runbook: "a".repeat(64) });
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "a".repeat(65) })), { runbook: "" });

  assert.equal(rb.parseTarget("onixresearch/kiln@refs/heads/main").repository, "onixresearch/kiln");
  assert.equal(rb.parseTarget("kiln@main").reference, "main");
  assert.ok(!rb.parseTarget(`${"a".repeat(256)}@${"b".repeat(1023)}`).error);
  assert.ok(rb.parseTarget(`${"a".repeat(256)}@${"b".repeat(1024)}`).error);
  assert.ok(rb.parseTarget("no-at-sign").error);
  assert.ok(rb.parseTarget("@refs/heads/main").error);
  assert.ok(rb.parseTarget("repo@").error);
  assert.ok(rb.parseTarget("repo@refs/heads/main ").error);
  assert.ok(rb.parseTarget("repo@-bad").error);
  assert.ok(rb.parseTarget("repo@a..b").error);

  const decls = [
    { name: "dry", kind: "bool", required: true },
    { name: "limit", kind: "integer", required: false, default: 5, integer_min: 1, integer_max: 10 },
    { name: "profile", kind: "enum", required: true, enum_values: ["fast", "full"] },
    { name: "path", kind: "relative_path", required: true, relative_path_max_bytes: 32 },
  ];
  const good = rb.coerceParams(decls, { dry: "true", limit: "7", profile: "full", path: "crates/kiln" });
  assert.deepEqual(plain(good), {
    params: { dry: true, limit: 7, profile: "full", path: "crates/kiln" },
    errors: [],
  });
  const bad = rb.coerceParams(decls, { dry: "maybe", limit: "11", profile: "other", path: "../escape" });
  assert.equal(bad.errors.length, 4);

  const missingReq = rb.validateInputs({ params: decls }, { target: "onixresearch/kiln@refs/heads/main", params: { dry: "false" }, requestId: "req-1" });
  assert.equal(missingReq.ok, false);
  assert.ok(missingReq.errors.some((e) => e.includes("profile")));
  assert.ok(missingReq.errors.some((e) => e.includes("path")));

  assert.equal(rb.classifyError({ status: 400, message: "bad param" }).kind, "validation");
  assert.equal(rb.classifyError({ status: 403, code: "runbook_grant_required", message: "denied" }).title, "Missing runbook grant");
  assert.equal(rb.classifyError({ status: 404, message: "missing" }).kind, "not_found");
  assert.equal(rb.classifyError({ status: 409, message: "conflict" }).kind, "conflict");
  const unknownErr = rb.classifyError({ status: 503 });
  assert.equal(unknownErr.kind, "unknown");
  assert.match(unknownErr.message, /Do not blindly retry/);
});

test("KilnRunbooks UI renders register, dispatches valid runbook, handles errors, and clears", async () => {
  const page = cairnPage();
  const doc = {
    createElement: (tag) => new CairnNode(tag),
  };
  const win = { KilnUi: ui };
  const ctxObj = {
    window: win,
    document: doc,
    TextEncoder,
    crypto: webcrypto,
  };
  vm.runInNewContext(asset("runbooks.js"), ctxObj);
  const rb = ctxObj.window.KilnRunbooks;

  const sampleRunbooks = {
    schema: "kiln.web-runbooks-view.v1",
    configured: true,
    principal: "local:operator",
    runbooks: [
      {
        id: "security-audit",
        display_name: "Security Audit",
        description: "Read-only audit.",
        mode: "read_only",
        lattice_bundle_id: "security-audit",
        lattice_bundle_identity: "a".repeat(64),
        lattice_workflow_name: "security-audit",
        lattice_workflow_revision: "b".repeat(64),
        params: [
          { name: "strict", kind: "bool", required: false, default: true, bindings: [{ node: "audit", shell_env: "LATTICE_PARAM_STRICT" }] },
        ],
        allowed_targets: [
          { repository: "onixresearch/kiln", project_id: "kiln-audit", references: ["refs/heads/main"], allow_commit_sha: false },
        ],
        can_dispatch: true,
        dispatch_refusal: null,
      },
      {
        id: "flake-update",
        display_name: "Flake Update",
        description: "Mutating update.",
        mode: "mutating",
        lattice_bundle_id: "flake-update",
        lattice_bundle_identity: "1".repeat(64),
        lattice_workflow_name: "flake-update",
        lattice_workflow_revision: "2".repeat(64),
        params: [],
        allowed_targets: [
          { repository: "onixresearch/kiln", project_id: "kiln-flake", references: ["refs/heads/main"], allow_commit_sha: true },
        ],
        can_dispatch: false,
        dispatch_refusal: "runbook_grant_required",
      },
    ],
  };

  const root = new CairnNode("div");
  const dispatched = [];
  const notices = [];
  const ctx = {
    route: { page: "runbooks", runbook: "security-audit" },
    canOperate: true,
    principal: { id: "local:operator", role: "operator" },
    navigate() {},
    notice(msg, tone) { notices.push({ msg, tone }); },
    runLink(runId, label) {
      const a = new CairnNode("a");
      a.href = `?page=runs&run=${runId}&view=pipeline`;
      a.textContent = label;
      return a;
    },
    isCurrent: () => true,
    async api(path, opts) {
      if (path === "/api/runbooks") return sampleRunbooks;
      if (path === "/api/runbooks/security-audit/dispatch") {
        dispatched.push(opts.body);
        return {
          schema: "kiln.runbook-dispatch-result.v1",
          request_id: opts.body.request_id,
          replayed: false,
          run_id: "web-run-abc123",
          target: opts.body.target,
          source_revision: "c".repeat(40),
        };
      }
      throw new Error(`unexpected ${path}`);
    },
  };

  const findById = (node, id) => {
    if (!node) return null;
    if (node.id === id) return node;
    for (const child of node.children || []) {
      const hit = findById(child, id);
      if (hit) return hit;
    }
    return null;
  };

  await rb.load(root, ctx);
  assert.match(root.textContent, /Security Audit/);
  assert.match(root.textContent, /Flake Update/);
  const form = findById(root, "runbook-dispatch-form");
  assert.ok(form);
  const reqInput = findById(root, "runbook-request-id");
  reqInput.value = "ui-req-1";
  await form.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(dispatched.length, 1);
  assert.deepEqual(plain(dispatched[0]), {
    request_id: "ui-req-1",
    target: "onixresearch/kiln@refs/heads/main",
    params: { strict: true },
  });
  const resultBox = findById(root, "runbook-dispatch-result");
  assert.equal(resultBox.hidden, false);
  assert.match(resultBox.textContent, /web-run-abc123/);

  // Ungranted mutating runbook keeps submit disabled and explains grant refusal.
  await rb.load(root, { ...ctx, route: { page: "runbooks", runbook: "flake-update" } });
  assert.equal(findById(root, "runbook-dispatch-submit").disabled, true);
  assert.match(root.textContent, /runbook_grant_required/);

  // Missing runbook does not substitute another runbook.
  await rb.load(root, { ...ctx, route: { page: "runbooks", runbook: "unknown-id" } });
  assert.match(root.textContent, /Runbook not found: unknown-id/);
  assert.equal(findById(root, "runbook-dispatch-form"), null);

  rb.clear();
});

test("KilnRunbooks stale submit finally does not clear newer generation busy state or disabled button", async () => {
  const doc = { createElement: (tag) => new CairnNode(tag) };
  const ctxObj = {
    window: { KilnUi: ui },
    document: doc,
    TextEncoder,
    crypto: webcrypto,
  };
  vm.runInNewContext(asset("runbooks.js"), ctxObj);
  const rb = ctxObj.window.KilnRunbooks;

  const makeRunbook = (id, canDispatch) => ({
    id,
    display_name: id,
    description: `${id} desc`,
    mode: "read_only",
    lattice_bundle_id: id,
    lattice_bundle_identity: "a".repeat(64),
    lattice_workflow_name: id,
    lattice_workflow_revision: "b".repeat(64),
    params: [],
    allowed_targets: [
      { repository: "onixresearch/kiln", project_id: "kiln-audit", references: ["refs/heads/main"], allow_commit_sha: false },
    ],
    can_dispatch: canDispatch,
    dispatch_refusal: canDispatch ? null : "runbook_grant_required",
  });

  const listPayload = {
    schema: "kiln.web-runbooks-view.v1",
    configured: true,
    principal: "local:operator",
    runbooks: [makeRunbook("rb-a", true), makeRunbook("rb-b", true), makeRunbook("rb-locked", false)],
  };

  const deferred = () => {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  };

  let dispatchA;
  let dispatchB;
  let dispatchCountB = 0;

  const findById = (node, id) => {
    if (!node) return null;
    if (node.id === id) return node;
    for (const child of node.children || []) {
      const hit = findById(child, id);
      if (hit) return hit;
    }
    return null;
  };

  const makeCtx = (runbookId) => ({
    route: { page: "runbooks", runbook: runbookId },
    canOperate: true,
    principal: { id: "local:operator", role: "operator" },
    navigate() {},
    notice() {},
    isCurrent: () => true,
    async api(path, opts) {
      if (path === "/api/runbooks") return listPayload;
      if (path === "/api/runbooks/rb-a/dispatch") {
        return dispatchA.promise;
      }
      if (path === "/api/runbooks/rb-b/dispatch") {
        dispatchCountB += 1;
        return dispatchB.promise;
      }
      throw new Error(`unexpected ${path} ${JSON.stringify(opts)}`);
    },
  });

  // Case 1: stale submit resolves while newer generation submit is in flight.
  dispatchA = deferred();
  dispatchB = deferred();
  const root = new CairnNode("div");
  await rb.load(root, makeCtx("rb-a"));
  const formA = findById(root, "runbook-dispatch-form");
  const submitBtnA = findById(root, "runbook-dispatch-submit");
  findById(root, "runbook-request-id").value = "req-a-1";
  const pendingA = formA.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(submitBtnA.disabled, true);

  await rb.load(root, makeCtx("rb-b"));
  const formB = findById(root, "runbook-dispatch-form");
  const submitBtnB = findById(root, "runbook-dispatch-submit");
  findById(root, "runbook-request-id").value = "req-b-1";
  const pendingB = formB.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(dispatchCountB, 1);
  assert.equal(submitBtnB.disabled, true);

  dispatchA.resolve({
    schema: "kiln.runbook-dispatch-result.v1",
    request_id: "req-a-1",
    replayed: false,
    run_id: "web-run-a",
    target: "onixresearch/kiln@refs/heads/main",
    source_revision: "c".repeat(40),
  });
  await pendingA;
  await new Promise((r) => setTimeout(r, 0));

  // Second submit on formB must be ignored because busy is still true for generation B.
  await formB.listeners.get("submit")[0]({ preventDefault() {} });
  assert.equal(dispatchCountB, 1);
  assert.equal(submitBtnB.disabled, true);

  dispatchB.resolve({
    schema: "kiln.runbook-dispatch-result.v1",
    request_id: "req-b-1",
    replayed: false,
    run_id: "web-run-b",
    target: "onixresearch/kiln@refs/heads/main",
    source_revision: "d".repeat(40),
  });
  await pendingB;
  assert.equal(submitBtnB.disabled, false);
  const resultBoxB = findById(root, "runbook-dispatch-result");
  assert.equal(resultBoxB.hidden, false);

  // Case 2: stale submit rejects after navigating to unpermitted rb-locked.
  dispatchA = deferred();
  await rb.load(root, makeCtx("rb-a"));
  const formA2 = findById(root, "runbook-dispatch-form");
  findById(root, "runbook-request-id").value = "req-a-2";
  const pendingA2 = formA2.listeners.get("submit")[0]({ preventDefault() {} });

  await rb.load(root, makeCtx("rb-locked"));
  const submitBtnLocked = findById(root, "runbook-dispatch-submit");
  assert.equal(submitBtnLocked.disabled, true);

  dispatchA.reject({ status: 503, message: "upstream unavailable" });
  await pendingA2;
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(submitBtnLocked.disabled, true);

  rb.clear();
});

test("KilnRunbooks fan-out helpers expand enrolled globs, validate inputs, and map child outcomes", () => {
  const rb = context.window.KilnRunbooks;
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "security-audit", fanout: "rb-fanout-abc_1" })), {
    runbook: "security-audit",
    fanout: "rb-fanout-abc_1",
  });
  assert.deepEqual(plain(rb.normalizeRoute({ runbook: "security-audit", fanout: "../bad" })), {
    runbook: "security-audit",
    fanout: "",
  });

  const explicit = rb.parseExplicitTargets("onixresearch/kiln@refs/heads/main\nonixresearch/lattice@refs/heads/main");
  assert.equal(explicit.errors.length, 0);
  assert.equal(explicit.targets.length, 2);

  const badGlobInExplicit = rb.parseExplicitTargets("onixresearch/*@refs/heads/main");
  assert.equal(badGlobInExplicit.errors.length, 1);

  const sampleRb = {
    params: [{ name: "strict", kind: "bool", required: false, default: true }],
    allowed_targets: [
      { repository: "onixresearch/kiln", project_id: "kiln-audit", references: ["refs/heads/main"] },
      { repository: "onixresearch/lattice", project_id: "lattice-audit", references: ["refs/heads/main"] },
      { repository: "other/repo", project_id: "other-audit", references: ["refs/heads/main"] },
    ],
    fanout: {
      enabled: true,
      default_max_concurrency: 2,
      max_concurrency_limit: 4,
      default_budget_cap: 8,
      max_budget_cap: 16,
      max_targets: 10,
      write_scopes: [],
    },
  };

  const preview = rb.matchEnrolledTargets(
    sampleRb,
    [{ id: "kiln-audit" }],
    "onixresearch/*@refs/heads/main",
  );
  assert.equal(preview.error, "");
  assert.equal(preview.matched.length, 1);
  assert.equal(preview.matched[0].project_id, "kiln-audit");
  assert.equal(preview.excludedUnenrolled.length, 1);
  assert.equal(preview.excludedUnenrolled[0].project_id, "lattice-audit");

  const validFo = rb.validateFanoutInputs(sampleRb, {
    targetMode: "glob",
    targetGlob: "onixresearch/*@refs/heads/main",
    maxConcurrency: "2",
    budgetCap: "4",
    claimPolicy: "serialize",
    params: { strict: "true" },
    requestId: "ui-fo-req-1",
  });
  assert.equal(validFo.ok, true);
  assert.deepEqual(plain(validFo.payload), {
    request_id: "ui-fo-req-1",
    targets: [],
    target_globs: ["onixresearch/*@refs/heads/main"],
    params: { strict: true },
    max_concurrency: 2,
    budget_cap: 4,
    claim_policy: "serialize",
  });

  const overCap = rb.validateFanoutInputs(sampleRb, {
    targetMode: "explicit",
    targetsText: "onixresearch/kiln@refs/heads/main",
    maxConcurrency: "99",
    budgetCap: "0",
    params: {},
    requestId: "ui-fo-req-2",
  });
  assert.equal(overCap.ok, false);
  assert.equal(overCap.errors.length, 2);

  assert.deepEqual(plain(rb.fanoutOutcomeStatus("unknown")), { status: "unknown", label: "Unknown" });
  assert.deepEqual(plain(rb.fanoutOutcomeStatus("skipped")), { status: "cancelled", label: "Skipped" });
  assert.deepEqual(plain(rb.fanoutOutcomeStatus("failure")), { status: "failure", label: "Failure" });
  assert.deepEqual(plain(rb.fanoutOutcomeStatus("success")), { status: "success", label: "Success" });
});

test("KilnRunbooks fan-out inspector renders ledger, cancel confirmation gate, and 503 retry", async () => {
  const doc = { createElement: (tag) => new CairnNode(tag) };
  const ctxObj = {
    window: { KilnUi: ui },
    document: doc,
    TextEncoder,
    crypto: webcrypto,
  };
  vm.runInNewContext(asset("runbooks.js"), ctxObj);
  const rb = ctxObj.window.KilnRunbooks;

  const fanoutDoc = {
    id: "rb-fanout-123",
    ordinal: 5,
    runbook_id: "security-audit",
    mode: "read_only",
    principal: "local:operator",
    request_id: "req-fo-1",
    target_selectors: ["onixresearch/*@refs/heads/main"],
    claim_policy: "refuse",
    admission: "open",
    phase: "active",
    stop_reason: null,
    max_concurrency: 2,
    active_children: 1,
    budget_cap: 3,
    budget_drawn: 2,
    budget_remaining: 1,
    unit_cost: 1,
    admitted_runs: 2,
    controls: { pause: true, continue: false, cancel: true },
    counts: { total: 4, pending: 0, active: 1, success: 1, failure: 0, unknown: 1, skipped: 1 },
    children: [
      { index: 0, project_id: "p0", repository: "onixresearch/a", target: "onixresearch/a@refs/heads/main", resolved_reference: "refs/heads/main", source_revision: "a".repeat(40), write_scope: null, status: "success", run_id: "web-run-0", outcome_detail: "success", reason: null },
      { index: 1, project_id: "p1", repository: "onixresearch/b", target: "onixresearch/b@refs/heads/main", resolved_reference: "refs/heads/main", source_revision: "b".repeat(40), write_scope: null, status: "unknown", run_id: "web-run-1", outcome_detail: "unknown", reason: "provider_unknown" },
      { index: 2, project_id: "p2", repository: "onixresearch/c", target: "onixresearch/c@refs/heads/main", resolved_reference: "refs/heads/main", source_revision: "c".repeat(40), write_scope: null, status: "active", run_id: "web-run-2", outcome_detail: null, reason: null },
      { index: 3, project_id: "p3", repository: "onixresearch/d", target: "onixresearch/d@refs/heads/main", resolved_reference: "refs/heads/main", source_revision: "d".repeat(40), write_scope: null, status: "skipped", run_id: null, outcome_detail: null, reason: "fanout_budget_exhausted" },
    ],
  };

  const controlCalls = [];
  let pauseAttempts = 0;

  const findById = (node, id) => {
    if (!node) return null;
    if (node.id === id) return node;
    for (const child of node.children || []) {
      const hit = findById(child, id);
      if (hit) return hit;
    }
    return null;
  };

  const ctx = {
    route: { page: "runbooks", runbook: "security-audit", fanout: "rb-fanout-123" },
    canOperate: true,
    principal: { id: "local:operator", role: "operator" },
    enrolledProjects: [{ id: "p0" }],
    navigate() {},
    notice() {},
    runLink(runId, label) {
      const a = new CairnNode("a");
      a.href = `?page=runs&run=${runId}&view=pipeline`;
      a.textContent = label;
      return a;
    },
    isCurrent: () => true,
    async api(path, opts) {
      if (path === "/api/runbooks") {
        return { schema: "kiln.web-runbooks-view.v1", configured: true, principal: "local:operator", runbooks: [{ id: "security-audit", display_name: "Security Audit", mode: "read_only", allowed_targets: [], params: [], can_dispatch: true }] };
      }
      if (path === "/api/runbooks/fanouts") {
        return { schema: "kiln.runbook-fanouts-view.v1", fanouts: [fanoutDoc], next_cursor: null, total: 1, bounded: true };
      }
      if (path === "/api/runbooks/fanouts/rb-fanout-123") {
        return fanoutDoc;
      }
      if (path === "/api/runbooks/fanouts/rb-fanout-123/pause") {
        controlCalls.push(opts.body.request_id);
        pauseAttempts += 1;
        if (pauseAttempts === 1) throw { status: 503, message: "transient" };
        return { schema: "kiln.runbook-fanout-control-result.v1", request_id: opts.body.request_id, replayed: true, action: "pause", fanout_id: "rb-fanout-123", fanout: { ...fanoutDoc, admission: "paused", phase: "paused" } };
      }
      throw new Error(`unexpected ${path}`);
    },
  };

  const root = new CairnNode("div");
  await rb.load(root, ctx);
  assert.match(root.textContent, /1 \/ 2/);
  assert.match(root.textContent, /2 \/ 3/);
  assert.match(root.textContent, /1 passed · 0 failed · 1 unknown · 1 skipped/);
  assert.match(root.textContent, /fanout_budget_exhausted/);
  assert.match(root.textContent, /provider_unknown/);

  const cancelBtn = findById(root, "fanout-cancel-btn");
  const cancelConfirm = findById(root, "fanout-cancel-confirm");
  assert.equal(cancelBtn.disabled, true);
  cancelConfirm.checked = true;
  cancelConfirm.listeners.get("change")[0]();
  assert.equal(cancelBtn.disabled, false);

  // First pause fails with 503 -> retains exact request_id and reveals Retry button.
  const pauseBtn = findById(root, "fanout-pause-btn");
  pauseBtn.listeners.get("click")[0]();
  await new Promise((r) => setTimeout(r, 10));
  const retryBtn = findById(root, "fanout-control-retry");
  const reqInput = findById(root, "fanout-control-request-id");
  assert.equal(retryBtn.hidden, false);
  assert.ok(reqInput.value);
  const retainedId = reqInput.value;

  retryBtn.listeners.get("click")[0]();
  await new Promise((r) => setTimeout(r, 10));
  assert.deepEqual(controlCalls, [retainedId, retainedId]);

  rb.clear();
});
