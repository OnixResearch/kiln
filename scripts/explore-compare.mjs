#!/usr/bin/env node
// Pure comparison classifier for exploration capture manifests.
// Usage: node scripts/explore-compare.mjs LEFT/manifest.json RIGHT/manifest.json
import { readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

function pathnames(values) {
  return (values || []).map((value) => {
    try { return new URL(value).pathname || String(value); }
    catch (error) { return String(value); }
  }).sort();
}

function readyOk(manifest) {
  const readiness = manifest.readiness || {};
  return readiness.ready_state === "complete" && Number(readiness.network_failures || 0) === 0;
}

function noFontErrors(manifest) {
  return !(manifest.fonts || []).some((font) => font && font.status === "error");
}

export function compareExplorationManifests(left, right) {
  const incompatibilities = [];
  if (left.case_id !== right.case_id) incompatibilities.push("case_id");
  if (left.recipe_revision !== right.recipe_revision) incompatibilities.push("recipe_revision");

  const la = left.appearance || {};
  const ra = right.appearance || {};
  if (la.requested_theme !== ra.requested_theme) incompatibilities.push("requested_theme");
  if (la.observed_theme !== ra.observed_theme) incompatibilities.push("observed_theme");
  if (la.requested_density !== ra.requested_density) incompatibilities.push("requested_density");
  if (la.observed_density !== ra.observed_density) incompatibilities.push("observed_density");

  const lv = left.viewport || {};
  const rv = right.viewport || {};
  if (lv.width !== rv.width || lv.height !== rv.height) incompatibilities.push("viewport");
  if (lv.device_pixel_ratio !== rv.device_pixel_ratio) incompatibilities.push("device_pixel_ratio");

  if (left.browser_version !== right.browser_version) incompatibilities.push("browser_version");

  if (!(readyOk(left) && readyOk(right))) incompatibilities.push("readiness");
  if (!(noFontErrors(left) && noFontErrors(right))) incompatibilities.push("fonts");

  const leftPaths = pathnames(left.resources || []);
  const rightPaths = pathnames(right.resources || []);
  const leftFailures = Number((left.readiness || {}).network_failures || 0);
  const rightFailures = Number((right.readiness || {}).network_failures || 0);
  if (leftPaths.join("\u0000") !== rightPaths.join("\u0000") || leftFailures !== 0 || rightFailures !== 0) {
    incompatibilities.push("resources");
  }

  const subjectChanges = {};
  const keys = new Set([...Object.keys(left.resolved_subjects || {}), ...Object.keys(right.resolved_subjects || {})]);
  for (const key of keys) {
    const l = (left.resolved_subjects || {})[key];
    const r = (right.resolved_subjects || {})[key];
    if (l !== r) subjectChanges[key] = { left: l, right: r };
  }

  return {
    comparable: incompatibilities.length === 0,
    incompatibilities,
    subject_changes: subjectChanges,
    normalization: "none",
  };
}

if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url))) {
  if (process.argv.length !== 4) {
    console.error("usage: node scripts/explore-compare.mjs LEFT/manifest.json RIGHT/manifest.json");
    process.exit(2);
  }
  const [left, right] = process.argv.slice(2).map((path) => JSON.parse(readFileSync(path, "utf8")));
  console.log(JSON.stringify(compareExplorationManifests(left, right), null, 2));
}
