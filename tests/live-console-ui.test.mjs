import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

// Load the served helper in an isolated context; only pure formatters are exercised.
const source = readFileSync(new URL("../crates/kiln-web-service/src/ui.js", import.meta.url), "utf8");
const context = { window: {} };
vm.runInNewContext(source, context);
const ui = context.window.KilnUi;

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
