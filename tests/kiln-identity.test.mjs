import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// The shared Kiln identity: its declared palette must stay readable in both appearances, and no surface
// stylesheet may carry a colour of its own or use a token nothing defines.
const source = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const IDENTITY = "crates/kiln-dashboard/src/render/identity.css";
const CATALOG = "crates/kiln-dashboard/src/render/catalog.css";
const WEB = "crates/kiln-web-service/src";
// Each surface with the stylesheets it loads after the identity.
const SURFACES = Object.freeze({
  "live console": [CATALOG, `${WEB}/web.css`, `${WEB}/work.css`, `${WEB}/work-interaction.css`, `${WEB}/code-review.css`, `${WEB}/project.css`, `${WEB}/source.css`],
  "offline catalog": [CATALOG],
  "offline report": ["crates/kiln-dashboard/src/render/html.css"],
  "hosted bridge page": ["crates/kiln-hosted/src/web.css"],
});

// WCAG 2.x contrast floors for text and for boundaries that identify a control.
const TEXT_RATIO = 4.5;
const BOUNDARY_RATIO = 3;
// OKLab distance between action emphasis and each status colour. A just-noticeable difference is about 0.02;
// the floor keeps ember several of those away from failure red and warning yellow.
const ACTION_STATUS_DISTANCE = 0.12;
const STATUSES = Object.freeze(["success", "danger", "info", "warning", "queued", "cancelled", "unknown"]);
const SYNTAX = Object.freeze(["keyword", "string", "comment", "number", "constant", "type", "function", "macro", "attribute", "property", "variable", "operator", "punctuation", "tag", "escape", "heading", "link"]);
const SCHEMES = Object.freeze(["light", "dark"]);

const withoutComments = (css) => css.replace(/\/\*[\s\S]*?\*\//g, "");
// Quoted strings (generated content, attribute-selector patterns) are not colour declarations.
const withoutStrings = (css) => css.replace(/"[^"]*"|'[^']*'/g, '""');

// Custom properties declared anywhere in a stylesheet, in order; a later declaration of one name wins.
function declaredTokens(css) {
  const tokens = new Map();
  for (const match of withoutComments(css).matchAll(/(--[a-z0-9-]+)\s*:\s*([^;{}]+)[;}]/g)) tokens.set(match[1], match[2].trim());
  return tokens;
}
// Resolve one token to an opaque #rrggbb in one appearance, following var() and light-dark().
function resolveColour(tokens, name, scheme, seen = new Set()) {
  if (seen.has(name)) throw new Error(`${name} refers to itself`);
  if (!tokens.has(name)) throw new Error(`${name} is not declared`);
  let value = tokens.get(name);
  const pair = /^light-dark\(\s*([^,]+?)\s*,\s*([^,]+?)\s*\)$/.exec(value);
  if (pair) value = scheme === "light" ? pair[1] : pair[2];
  const reference = /^var\(\s*(--[a-z0-9-]+)\s*\)$/.exec(value);
  if (reference) return resolveColour(tokens, reference[1], scheme, new Set([...seen, name]));
  if (!/^#[0-9a-f]{6}$/i.test(value)) throw new Error(`${name} is not an opaque hex colour in ${scheme}: ${value}`);
  return value.toLowerCase();
}
const channels = (hex) => [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16) / 255);
const linear = (value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
function luminance(hex) {
  const [red, green, blue] = channels(hex).map(linear);
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}
function contrast(first, second) {
  const [lighter, darker] = [luminance(first), luminance(second)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}
function oklab(hex) {
  const [red, green, blue] = channels(hex).map(linear);
  const long = Math.cbrt(0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue);
  const medium = Math.cbrt(0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue);
  const short = Math.cbrt(0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue);
  return [0.2104542553 * long + 0.793617785 * medium - 0.0040720468 * short, 1.9779984951 * long - 2.428592205 * medium + 0.4505937099 * short, 0.0259040371 * long + 0.7827717662 * medium - 0.808675766 * short];
}
const distance = (first, second) => Math.hypot(...oklab(first).map((value, index) => value - oklab(second)[index]));

// Every pair the identity promises, as [foreground token, surface token, floor, appearances].
function contrastPairs() {
  const pairs = [];
  const add = (foreground, surfaces, floor, schemes = SCHEMES) => { for (const surface of surfaces) pairs.push([foreground, surface, floor, schemes]); };
  const pageSurfaces = ["--background", "--group-box", "--secondary", "--popover", "--muted", "--accent", "--list-hover", "--list-active", "--table-head"];
  add("--foreground", pageSurfaces, TEXT_RATIO);
  add("--muted-foreground", pageSurfaces, TEXT_RATIO);
  add("--link", pageSurfaces, TEXT_RATIO);
  // The navigation rail, hero and log panel are dark in both appearances, so only dark values are drawn there.
  for (const text of ["--foreground", "--muted-foreground"]) add(text, ["--sidebar", "--soot"], TEXT_RATIO, ["dark"]);
  add("--editor-foreground", ["--editor-background"], TEXT_RATIO);
  add("--editor-muted-foreground", ["--editor-background"], TEXT_RATIO);
  for (const status of STATUSES) add(`--${status}`, [`--${status}-background`, "--background", "--group-box", "--secondary", "--muted"], TEXT_RATIO);
  // Status text also appears inside the dark log panel.
  for (const status of ["success", "danger", "info", "warning"]) add(`--${status}`, ["--soot", "--editor-background"], TEXT_RATIO, ["dark"]);
  add("--primary-foreground", ["--primary", "--primary-hover", "--ember-bright"], TEXT_RATIO);
  add("--ring", ["--background", "--group-box", "--secondary", "--popover", "--muted"], BOUNDARY_RATIO);
  add("--ring", ["--sidebar", "--soot"], BOUNDARY_RATIO, ["dark"]);
  add("--input-border", ["--background", "--group-box", "--secondary", "--popover"], BOUNDARY_RATIO);
  for (const kind of SYNTAX) add(`--syntax-${kind}`, ["--group-box", "--secondary", "--muted", "--success-background", "--danger-background", "--code-selection"], TEXT_RATIO);
  return pairs;
}
function contrastFailures(css) {
  const tokens = declaredTokens(css);
  const failures = [];
  for (const [foreground, surface, floor, schemes] of contrastPairs()) {
    for (const scheme of schemes) {
      const ratio = contrast(resolveColour(tokens, foreground, scheme), resolveColour(tokens, surface, scheme));
      if (ratio < floor) failures.push(`${foreground} on ${surface} (${scheme}): ${ratio.toFixed(2)} < ${floor}`);
    }
  }
  for (const scheme of SCHEMES) {
    for (const status of ["success", "danger", "info", "warning"]) {
      const apart = distance(resolveColour(tokens, "--primary", scheme), resolveColour(tokens, `--${status}`, scheme));
      if (apart < ACTION_STATUS_DISTANCE) failures.push(`--primary and --${status} (${scheme}): distance ${apart.toFixed(3)} < ${ACTION_STATUS_DISTANCE}`);
    }
  }
  return failures;
}

const COLOUR_LITERAL = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/i;
const NAMED_COLOUR = /(?<![-\w])(?:white|black|red|green|blue|yellow|orange|purple|pink|gray|grey|silver|maroon|navy|teal|olive|lime|aqua|fuchsia)(?![-\w])/i;
// Declarations of one surface stylesheet that carry a colour of their own, or use a token nothing defines.
function ownershipFailures(name, css, definedElsewhere) {
  const text = withoutStrings(withoutComments(css));
  const defined = new Set([...definedElsewhere, ...declaredTokens(css).keys()]);
  const failures = [];
  for (const match of text.matchAll(/([a-z-]+)\s*:\s*([^;{}]+)[;}]/gi)) {
    const [property, value] = [match[1], match[2].trim()];
    if (COLOUR_LITERAL.test(value) || NAMED_COLOUR.test(value)) failures.push(`${name}: colour literal in "${property}: ${value}"`);
    for (const use of value.matchAll(/var\(\s*(--[a-z0-9-]+)/g)) if (!defined.has(use[1])) failures.push(`${name}: undefined token ${use[1]} in "${property}: ${value}"`);
  }
  return failures;
}

const identity = source(IDENTITY);

// r[verify kiln.identity.contrast]
// r[verify kiln.identity.contrast.scenario.palette]
test("the declared palette meets its contrast floor in light and dark", () => {
  assert.deepEqual(contrastFailures(identity), []);
  const tokens = declaredTokens(identity);
  // The floors are measured, not assumed: the ink-on-porcelain pair is far above the floor and the primary pair is real.
  assert.ok(contrast(resolveColour(tokens, "--foreground", "light"), resolveColour(tokens, "--background", "light")) > 12);
  assert.ok(contrast(resolveColour(tokens, "--primary-foreground", "dark"), resolveColour(tokens, "--primary", "dark")) >= TEXT_RATIO);
  assert.equal(contrast("#000000", "#ffffff"), 21);
  assert.equal(contrast("#777777", "#777777"), 1);
});

// r[verify kiln.identity.contrast.scenario.regression]
test("a palette edit below the floor is reported with its pair, appearance and ratio", () => {
  const lowered = identity.replace(/--muted-foreground:\s*light-dark\([^)]*\)/, "--muted-foreground: light-dark(#a39a90, #ab9f93)");
  assert.notEqual(lowered, identity);
  const failures = contrastFailures(lowered);
  assert.ok(failures.length > 0);
  assert.ok(failures.every((failure) => failure.startsWith("--muted-foreground on ") && failure.includes("(light)")), failures.join("\n"));
  assert.match(failures[0], /^--muted-foreground on --background \(light\): 2\.\d\d < 4\.5$/);
  // Only the lowered appearance fails; a status colour pulled onto ember is reported as too close to action emphasis.
  const collided = identity.replace(/--danger:\s*light-dark\([^)]*\)/, "--danger: light-dark(#a8182d, #ff7a45)");
  assert.deepEqual(contrastFailures(collided).filter((failure) => failure.startsWith("--primary and ")), ["--primary and --danger (dark): distance 0.031 < 0.12"]);
  // A token that stops being an opaque colour cannot be measured, so it is refused rather than skipped.
  assert.throws(() => contrastFailures(identity.replace(/--link:\s*[^;]+;/, "--link: rgb(0 0 0 / 50%);")), /--link is not an opaque hex colour in light/);
  assert.throws(() => contrastFailures(identity.replace(/--ring:\s*[^;]+;/, "")), /--ring is not declared/);
});

// r[verify kiln.identity.shared]
// r[verify kiln.identity.shared.scenario.ownership]
test("surface stylesheets carry no colour of their own and use only defined tokens", () => {
  const identityTokens = [...declaredTokens(identity).keys()];
  assert.ok(identityTokens.includes("--primary") && identityTokens.includes("--syntax-keyword"));
  for (const [surface, sheets] of Object.entries(SURFACES)) {
    // A token may come from the identity or from any stylesheet the same surface loads.
    const surfaceTokens = sheets.flatMap((path) => [...declaredTokens(source(path)).keys()]);
    for (const path of sheets) assert.deepEqual(ownershipFailures(path, source(path), [...identityTokens, ...surfaceTokens]), [], `${surface}: ${path}`);
  }
});

// r[verify kiln.identity.shared.scenario.ownership]
test("a palette literal or an undefined token in a surface stylesheet is named", () => {
  const known = ["--border", "--primary"];
  assert.deepEqual(ownershipFailures("fixture.css", ".a { color: #b0352c; border: 1px solid var(--border); }", known), ['fixture.css: colour literal in "color: #b0352c"']);
  assert.deepEqual(ownershipFailures("fixture.css", ".a { background: rgb(0 0 0 / 50%) }", known), ['fixture.css: colour literal in "background: rgb(0 0 0 / 50%)"']);
  assert.deepEqual(ownershipFailures("fixture.css", ".a { color: white; white-space: nowrap; }", known), ['fixture.css: colour literal in "color: white"']);
  assert.deepEqual(ownershipFailures("fixture.css", ".a { color: var(--signal); outline: 1px solid var(--primary); }", known), ['fixture.css: undefined token --signal in "color: var(--signal)"']);
  assert.deepEqual(ownershipFailures("fixture.css", ".a { color: var(--missing, var(--border)); }", known), ['fixture.css: undefined token --missing in "color: var(--missing, var(--border))"']);
  // Allowed: tokens the sheet defines itself, system colours, keywords, and colour text inside a quoted selector pattern.
  const allowed = ':root { --rail: 4rem; } .a { width: var(--rail); color: CanvasText; background: transparent; border-color: currentColor; } .b[style*="color: rgb(255, 255, 255)"]::before { content: "#fff"; color: var(--primary); }';
  assert.deepEqual(ownershipFailures("fixture.css", allowed, known), []);
});
