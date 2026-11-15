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
