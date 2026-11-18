import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const source = await readFile(new URL("../crates/kiln-web-service/src/preview-review.js", import.meta.url));
const review = await import(`data:text/javascript;base64,${source.toString("base64")}`);

const A = "a".repeat(64), B = "b".repeat(64), C = "c".repeat(64);
function row(identity, current, overrides = {}) {
  return {
    identity, current, review_run_id: current ? "source-run" : null,
    observation: {
      descriptor_identity: "d".repeat(64), engine_identity: "e".repeat(64), platform: "x86_64-linux",
      binding: { project: "p", repository: "r", revision: current ? "1".repeat(40) : "0".repeat(40), run_id: "build-run", attempt_id: "attempt", artifact_identity: "f".repeat(64) },
      invocation: { root_path: "/nix/store/x-cli", executable: "/nix/store/x-cli/bin/cli", arguments: ["noop"] },
      observed_at: 1_800_000_000, phases: [{ phase: "artifact discovery", outcome: "ready", elapsed_ms: 12 }],
      result: { status: 0, output: "emubench: done", error: null }, ...overrides,
    },
  };
}

// r[verify kiln.preview.comparison]
test("deep links accept only an exact baseline~candidate identity pair", () => {
  assert.deepEqual(review.comparisonRoute(`${A}~${B}`), { baseline: A, candidate: B });
  for (const value of ["", `${A}~`, `${A}..${B}`, `${A.toUpperCase()}~${B}`, `${A}~${B}~${C}`, `${A.slice(1)}~${B}`]) assert.equal(review.comparisonRoute(value), null);
});

// r[verify kiln.preview.comparison]
test("only current observations can be candidates and malformed rows are dropped", () => {
  const { baselines, candidates } = review.comparisonChoices([row(A, false), row(B, true), { identity: "short", current: true, observation: {} }, null]);
  assert.deepEqual(baselines.map(r => r.identity), [A, B]);
  assert.deepEqual(candidates.map(r => r.identity), [B]);
  assert.deepEqual(review.comparisonChoices(undefined), { baselines: [], candidates: [] });
});

// r[verify kiln.preview.comparison]
test("incompatibilities are explained, never hidden", () => {
  assert.match(review.explainComparison({ compatible: true, reasons: [] })[0], /^Comparable/);
  assert.deepEqual(review.explainComparison({ compatible: false, reasons: ["engine_differs", "arguments_differ", "novel_reason"] }), [
    review.COMPARISON_REASONS.engine_differs, review.COMPARISON_REASONS.arguments_differ, "Unrecognised incompatibility: novel_reason",
  ]);
});

// r[verify kiln.preview.comparison]
test("late, moved or mismatched comparison responses are ignored", () => {
  const pair = `${A}~${B}`;
  const response = { comparison: { baseline: A, candidate: B }, candidate: { current: true } };
  assert.equal(review.acceptComparison(pair, pair, response, () => true), true);
  assert.equal(review.acceptComparison(pair, `${A}~${C}`, response, () => true), false);
  assert.equal(review.acceptComparison(pair, pair, response, () => false), false);
  assert.equal(review.acceptComparison(pair, pair, { comparison: { baseline: C, candidate: B }, candidate: { current: true } }, () => true), false);
  assert.equal(review.acceptComparison(pair, pair, { comparison: { baseline: A, candidate: B }, candidate: { current: false } }, () => true), false);
});

function fakeDocument() {
  const all = [];
  const element = tag => {
    const node = {
      tagName: tag.toUpperCase(), children: [], attributes: {}, textContent: "", className: "", listeners: {},
      append(...children) { for (const child of children) { child.parent = this; this.children.push(child); } },
      replaceChildren(...children) { this.children = []; this.append(...children); },
      setAttribute(key, value) { this.attributes[key] = value; },
      addEventListener(type, handler) { this.listeners[type] = handler; },
      focus() { globalThis.focused = this; },
    };
    all.push(node); return node;
  };
  return { all, document: { createElement: element } };
}
const text = node => [node.textContent, ...node.children.map(text)].join(" ");

// r[verify kiln.preview.comparison]
test("both lanes render exact identities and the baseline lane never launches", async () => {
  const { all, document } = fakeDocument();
  globalThis.document = document;
  try {
    const parent = document.createElement("div");
    const pair = `${A}~${B}`;
    const response = {
      comparison: { identity: C, rules: "kiln.preview.comparison-rules.v1", baseline: A, candidate: B, compatible: false, reasons: ["arguments_differ"], exit_status_equal: true, output_equal: false },
      baseline: row(A, false), candidate: row(B, true, { result: { status: 0, output: "changed", error: null } }),
    };
    review.renderComparison(parent, [row(A, false), row(B, true)], {
      requested: pair, selected: () => pair, select: () => {}, isCurrent: () => true,
      compare: async (baseline, candidate) => { assert.deepEqual([baseline, candidate], [A, B]); return response; },
    });
    await new Promise(resolve => setTimeout(resolve, 0));
    const rendered = text(parent);
    assert.match(rendered, new RegExp(`Comparison ${C}`));
    assert.match(rendered, /The program arguments differ\./);
    assert.match(rendered, /Historical or baseline lane: launch unavailable\./);
    assert.match(rendered, new RegExp(A));
    assert.match(rendered, new RegExp(B));
    assert.equal(all.filter(node => node.tagName === "A").length, 0);
    assert.equal(all.filter(node => node.className === "cr-preview-lanes").length, 1);
  } finally { delete globalThis.document; }
});

// r[verify kiln.preview.comparison]
test("a missing linked subject is reported without substitution or a request", () => {
  const { document } = fakeDocument();
  globalThis.document = document;
  try {
    const parent = document.createElement("div");
    let requests = 0;
    review.renderComparison(parent, [row(A, false), row(B, true)], {
      requested: `${C}~${B}`, selected: () => `${C}~${B}`, select: () => {}, isCurrent: () => true,
      compare: async () => { requests += 1; },
    });
    assert.match(text(parent), /missing, expired or no longer current\. No substitute was selected\./);
    assert.equal(requests, 0);
  } finally { delete globalThis.document; }
});

// The frozen fixture's real engine output and settings, served as a structural record.
const fixture = name => readFile(new URL(`../crates/kiln-web-service/tests/fixtures/structural-preview/${name}`, import.meta.url)).then(bytes => JSON.parse(bytes));
const [FILES, SETTINGS] = await Promise.all([fixture("expected.json"), fixture("settings.json")]);
function structuralResponse(overrides = {}, evidence = {}) {
  return {
    comparison: { identity: C, rules: "kiln.preview.comparison-rules.v1", baseline: A, candidate: B, compatible: true, reasons: [], exit_status_equal: true, output_equal: true },
    baseline: row(A, false), candidate: row(B, true),
    structural: {
      identity: "9".repeat(64),
      evidence: { schema: "kiln.preview.structural.v1", comparison: C, baseline_revision: "0".repeat(40), candidate_revision: "1".repeat(40), settings: SETTINGS, status: "structural", reasons: [], files: FILES, ...evidence },
    },
    ...overrides,
  };
}

// r[verify kiln.preview.structural]
test("structural evidence applies only to its exact comparison and revisions", () => {
  assert.equal(review.acceptStructural(structuralResponse()), true);
  for (const response of [
    structuralResponse({ structural: undefined }),
    structuralResponse({}, { comparison: A }),
    structuralResponse({}, { baseline_revision: "1".repeat(40) }),
    structuralResponse({}, { candidate_revision: "0".repeat(40) }),
    { ...structuralResponse(), structural: { identity: "short", evidence: structuralResponse().structural.evidence } },
  ]) assert.equal(review.acceptStructural(response), false);
});

// r[verify kiln.preview.structural.scenario.fallback]
test("structural summaries name every fallback and rows stay bounded", () => {
  const byPath = Object.fromEntries(FILES.map(file => [file.path, file]));
  assert.equal(review.structuralSummary(byPath["reformat.rs"]), "No syntactic change: identical or formatting only.");
  assert.match(review.structuralSummary(byPath["moved.rs"]), /^0 changed regions and 1 moved unchanged block;/);
  assert.match(review.structuralSummary(byPath["edited.rs"]), /^1 changed region and 0 moved unchanged blocks; 1 removed and 1 added tokens\.$/);
  for (const [path, reason] of [["broken.rs", "parse_error"], ["notes.txt", "unsupported_language"], ["pixel.bin", "binary"]]) {
    assert.equal(review.structuralSummary(byPath[path]), `Fallback: ${review.STRUCTURAL_REASONS[reason]} No partial tree is shown.`);
  }
  assert.equal(review.structuralSummary({ status: "fallback", reason: "novel_reason" }), "Fallback: Unrecognised reason: novel_reason No partial tree is shown.");
  const { lines, omitted } = review.structuralLines(byPath["edited.rs"]);
  assert.deepEqual(lines, ["-     1        pub const LIMIT: u32 = 2;", "+           1  pub const LIMIT: u32 = 3;", "      3     3  pub fn limit() -> u32 {", "      4     4      LIMIT"]);
  assert.equal(omitted, 0);
  assert.deepEqual(review.structuralLines(byPath["edited.rs"], 1), { lines: [lines[0]], omitted: 3 });
  assert.deepEqual(review.structuralLines(byPath["broken.rs"]), { lines: [], omitted: 0 });
});

async function renderWith(context, response) {
  const { all, document } = fakeDocument();
  globalThis.document = document;
  try {
    const parent = document.createElement("div"), requests = [], selections = [];
    review.renderComparison(parent, [row(A, false), row(B, true)], {
      requested: `${A}~${B}`, selected: () => `${A}~${B}`, select: (...args) => selections.push(args), isCurrent: () => true,
      compare: async (...args) => { requests.push(args); return response; }, ...context,
    });
    await new Promise(resolve => setTimeout(resolve, 0));
    return { all, parent, requests, selections };
  } finally { delete globalThis.document; }
}

// r[verify kiln.preview.structural.scenario.off]
test("structural evidence is requested and shown only when selected", async () => {
  const off = await renderWith({}, structuralResponse({ structural: undefined }));
  assert.deepEqual(off.requests, [[A, B, false]]);
  assert.equal(off.all.filter(node => node.className === "cr-operation cr-preview-structural").length, 0);
  assert.doesNotMatch(text(off.parent), /Structural source evidence Opt-in/);
  const box = off.all.find(node => node.tagName === "INPUT" && node.type === "checkbox");
  assert.equal(box.checked, false);
  // Selecting it routes the same pair with the opt-in; nothing is fetched until the route changes.
  for (const select of off.all.filter(node => node.tagName === "SELECT")) select.value = select.children.find(option => option.selected)?.value;
  box.checked = true;
  off.all.find(node => node.tagName === "FORM").listeners.submit({ preventDefault() {} });
  assert.deepEqual(off.selections, [[`${A}~${B}`, true]]);
  assert.equal(off.requests.length, 1);

  const on = await renderWith({ structural: true }, structuralResponse());
  assert.deepEqual(on.requests, [[A, B, true]]);
  assert.equal(on.all.find(node => node.tagName === "INPUT" && node.type === "checkbox").checked, true);
  const rendered = text(on.parent);
  assert.match(rendered, /Structural source evidence/);
  assert.match(rendered, new RegExp(`9{64}`));
  assert.match(rendered, new RegExp(SETTINGS.grammar_identity));
  assert.match(rendered, /reformat\.rs No syntactic change/);
  assert.match(rendered, /Moved unchanged: lines 9–11 → 1–3\./);
  assert.match(rendered, /broken\.rs Fallback: A side does not parse\. No partial tree is shown\./);
  assert.match(rendered, /does not measure review time/);
  assert.equal(on.all.filter(node => node.className === "cr-operation cr-preview-structural").length, 1);
  assert.equal(on.all.filter(node => node.tagName === "A").length, 0);
});

// r[verify kiln.preview.structural.scenario.fallback]
test("withheld, unavailable or mismatched structural evidence is explained without substitution", async () => {
  const withheld = await renderWith({ structural: true }, structuralResponse({}, { status: "incompatible", reasons: ["arguments_differ"], files: [] }));
  assert.match(text(withheld.parent), /Structural evidence withheld: the subjects are not comparable/);
  assert.match(text(withheld.parent), /The program arguments differ\./);
  const unavailable = await renderWith({ structural: true }, structuralResponse({}, { status: "unavailable", reasons: ["evidence_exhausted"], files: [] }));
  assert.match(text(unavailable.parent), /Structural evidence unavailable\. No other revision was substituted\./);
  assert.match(text(unavailable.parent), /exceeded its size bound, so it keeps no files/);
  const mismatched = await renderWith({ structural: true }, structuralResponse({}, { comparison: A }));
  assert.match(text(mismatched.parent), /missing or bound to another comparison\. Nothing was substituted\./);
  assert.doesNotMatch(text(mismatched.parent), /reformat\.rs/);
});
