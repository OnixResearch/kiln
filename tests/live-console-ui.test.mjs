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
vm.runInNewContext(asset("source.js"), context);
const ui = context.window.KilnUi;
const icons = context.window.KilnIcons;
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
  const sources = ["web.html", "web.js", "ui.js", "work.js", "work-interaction.js", "code-review.js", "project.js", "source.js"];
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

// r[verify kiln.dashboard.source.navigation]
test("source sizes read in binary units; unknown or inexact sizes stay blank", () => {
  const cases = [[0, "0 B"], [1, "1 B"], [1023, "1023 B"], [1024, "1.0 KiB"], [1536, "1.5 KiB"], [10 * 1024 - 1, "10 KiB"], [10 * 1024, "10 KiB"], [74364, "73 KiB"], [1048575, "1.0 MiB"], [5.25 * 1048576, "5.3 MiB"], [3 * 1024 ** 3, "3.0 GiB"]];
  for (const [bytes, label] of cases) assert.equal(source.humanBytes(bytes), label, String(bytes));
  for (const value of [null, undefined, -1, 1.5, Number.NaN, Infinity, "12", 2 ** 53, {}]) assert.equal(source.humanBytes(value), "", String(value));
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
