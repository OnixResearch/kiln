import assert from "node:assert/strict";
import { test } from "node:test";

const compare = await import(new URL("../scripts/explore-compare.mjs", import.meta.url).href);
// r[verify kiln.web.exploration.cases.capture] Incompatible captures are reported and subject changes are never normalized.

function base(overrides = {}) {
  return {
    case_id: "case.run.failed",
    recipe_revision: "a".repeat(64),
    viewport: { width: 1280, height: 800, device_pixel_ratio: 2 },
    appearance: {
      requested_theme: "dark",
      observed_theme: "dark",
      requested_density: "compact",
      observed_density: "compact",
    },
    browser_version: "Chrome/1",
    readiness: { ready_state: "complete", network_failures: 0 },
    fonts: [],
    resources: ["http://127.0.0.1:1/x.css", "http://127.0.0.1:1/y.js"],
    resolved_subjects: { run: "run-1" },
    ...overrides,
  };
}

test("classifier comparable with different subjects", () => {
  const left = base();
  const right = base({ resolved_subjects: { run: "run-2" } });
  const result = compare.compareExplorationManifests(left, right);
  assert.equal(result.comparable, true);
  assert.deepEqual(result.incompatibilities, []);
  assert.deepEqual(result.subject_changes, { run: { left: "run-1", right: "run-2" } });
  assert.equal(result.normalization, "none");
});

test("classifier reports each incompatibility", () => {
  const left = base();
  const right = base({
    case_id: "case.run.partial-retry",
    recipe_revision: "b".repeat(64),
    viewport: { width: 800, height: 600, device_pixel_ratio: 1 },
    appearance: {
      requested_theme: "light",
      observed_theme: "light",
      requested_density: "comfortable",
      observed_density: "comfortable",
    },
    browser_version: "Chrome/2",
    readiness: { ready_state: "loading", network_failures: 2 },
    fonts: [{ family: "x", status: "error" }],
    resources: ["http://127.0.0.1:2/z.css"],
  });
  const result = compare.compareExplorationManifests(left, right);
  assert.equal(result.comparable, false);
  for (const name of [
    "case_id",
    "recipe_revision",
    "requested_theme",
    "observed_theme",
    "requested_density",
    "observed_density",
    "viewport",
    "device_pixel_ratio",
    "browser_version",
    "readiness",
    "fonts",
    "resources",
  ]) {
    assert.ok(result.incompatibilities.includes(name), name);
  }
});

test("classifier ignores origin differences in resource URLs", () => {
  const left = base();
  const right = base({ resources: ["http://other-host:9999/x.css", "http://other-host:9999/y.js"] });
  const result = compare.compareExplorationManifests(left, right);
  assert.equal(result.comparable, true);
});
