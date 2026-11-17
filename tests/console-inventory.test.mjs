import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

// Cross-checks docs/console-port-inventory.json against the served console, the
// reference for the Rats/Meteorite port (#68). Every problem names its entry and
// the missing reference; an empty list means the inventory matches the console.
const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const asset = (name) => `crates/kiln-web-service/src/${name}`;

function loadConsole() {
  const context = { window: {} };
  vm.runInNewContext(read(asset("ui.js")), context);
  const sources = new Map();
  const source = (name) => {
    if (!sources.has(name)) sources.set(name, existsSync(new URL(asset(name), root)) ? read(asset(name)) : null);
    return sources.get(name);
  };
  return {
    destinations: JSON.parse(JSON.stringify(context.window.KilnUi.destinations)),
    html: source("web.html"),
    server: source("server.rs"),
    web: source("web.js"),
    source,
    file: (path) => (existsSync(new URL(path, root)) ? read(path) : null),
  };
}

// API paths a module names, as the console's fetch helpers spell them.
const apiPaths = (text) =>
  new Set([...text.matchAll(/["'`](\/api\/[a-z0-9_/-]+)/g)].map(([, path]) => path.replace(/\/+$/, "")));
const hasAnchor = (html, anchor) =>
  anchor.startsWith("#")
    ? html.includes(`id="${anchor.slice(1)}"`)
    : new RegExp(`class="[^"]*(?<![\\w-])${anchor.slice(1)}(?![\\w-])[^"]*"`).test(html);

function problems(inventory, kiln) {
  const found = [];
  const served = (file) => kiln.server.includes(`"/${file}" =>`);
  const stylesheet = (owner, css) => {
    if (!served(css)) found.push(`${owner}: stylesheet ${css} is not served`);
    if (!kiln.html.includes(`href="/${css}"`)) found.push(`${owner}: stylesheet ${css} is not linked by web.html`);
  };
  if (inventory.schema !== "kiln.console-inventory.v1") found.push(`schema ${inventory.schema} is not kiln.console-inventory.v1`);
  const cohorts = new Map();
  for (const cohort of inventory.cohorts) {
    if (cohorts.has(cohort.id)) found.push(`cohort ${cohort.id} is declared twice`);
    cohorts.set(cohort.id, cohort);
  }
  const orders = [...cohorts.values()].map((cohort) => cohort.order).sort((a, b) => a - b);
  if (orders.some((order, index) => order !== index + 1)) found.push(`cohort orders ${orders.join(",")} are not 1..${orders.length}`);
  const vocabulary = new Set(inventory.states);
  for (const css of inventory.shared_stylesheets) stylesheet("shared", css);

  const registered = new Map(kiln.destinations.map((destination) => [destination.page, destination]));
  const routes = new Map();
  for (const route of inventory.routes) {
    const name = `route ${route.page}`;
    if (routes.has(route.page)) found.push(`${name} is inventoried twice`);
    routes.set(route.page, route);
    const destination = registered.get(route.page);
    if (!destination) {
      found.push(`${name} is not a registered destination`);
      continue;
    }
    if (route.number !== destination.number) found.push(`${name}: number ${route.number} differs from registered ${destination.number}`);
    if (!cohorts.has(route.cohort)) found.push(`${name}: cohort ${route.cohort} is not declared`);
    if (!kiln.html.includes(`data-panel="${route.panel}"`)) found.push(`${name}: panel ${route.panel} is not in web.html`);
    if (destination.parent === null && !kiln.html.includes(`data-page="${route.page}"`)) found.push(`${name}: no rail link in web.html`);
    if (route.modules.filter((module) => module.renderer === true).length !== 1) found.push(`${name}: exactly one module must render the page`);
    const sources = [];
    for (const module of route.modules) {
      const text = kiln.source(module.file);
      if (text === null) {
        found.push(`${name}: module ${module.file} does not exist`);
        continue;
      }
      sources.push(text);
      if (!served(module.file)) found.push(`${name}: module ${module.file} is not served`);
      if (module.loaded_by) {
        if (!route.modules.some((other) => other.file === module.loaded_by)) found.push(`${name}: loader ${module.loaded_by} of ${module.file} is not one of its modules`);
        else if (!(kiln.source(module.loaded_by) || "").includes(`"/${module.file}"`)) found.push(`${name}: ${module.loaded_by} does not load ${module.file}`);
      } else if (!kiln.html.includes(`src="/${module.file}"`)) found.push(`${name}: module ${module.file} is not loaded by web.html`);
      if (module.global) {
        if (!text.includes(`window.${module.global} =`)) found.push(`${name}: ${module.file} does not define window.${module.global}`);
        if (module.renderer && !kiln.web.includes(`window.${module.global}`)) found.push(`${name}: web.js does not dispatch to window.${module.global}`);
      }
      for (const check of module.node_tests) {
        const testText = kiln.file(check);
        if (testText === null) found.push(`${name}: Node test ${check} does not exist`);
        else if (!testText.includes(module.file)) found.push(`${name}: Node test ${check} does not load ${module.file}`);
      }
    }
    for (const css of route.stylesheets) stylesheet(name, css);
    const consumed = new Set(sources.flatMap((text) => [...apiPaths(text)]));
    for (const path of route.api) if (!consumed.has(path)) found.push(`${name}: API path ${path} is not named by its modules`);
    for (const parameter of route.url_state) {
      if (!kiln.web.includes(`params.get("${parameter}")`)) found.push(`${name}: URL parameter ${parameter} is not read by web.js`);
    }
    for (const state of route.states) if (!vocabulary.has(state)) found.push(`${name}: state ${state} is not in the state vocabulary`);
  }
  for (const page of registered.keys()) if (!routes.has(page)) found.push(`destination ${page} is not inventoried`);
  for (const cohort of cohorts.values()) {
    if (![...routes.values()].some((route) => route.cohort === cohort.id)) found.push(`cohort ${cohort.id} has no route`);
  }
  const pilot = [...cohorts.values()].find((cohort) => cohort.order === 1);
  if (routes.get("runs")?.cohort !== pilot?.id) found.push("route runs is not in the first cohort");

  for (const component of inventory.shell) {
    const name = `shell ${component.component}`;
    for (const anchor of component.anchors) if (!hasAnchor(kiln.html, anchor)) found.push(`${name}: anchor ${anchor} is not in web.html`);
    const owner = kiln.source(component.owner);
    if (owner === null || !served(component.owner)) found.push(`${name}: owner ${component.owner} is not a served module`);
    for (const css of component.stylesheets) stylesheet(name, css);
    for (const runtime of component.runtime_classes || []) {
      if (!(owner || "").includes(runtime)) found.push(`${name}: ${component.owner} does not create ${runtime}`);
      if (!component.stylesheets.some((css) => (kiln.source(css) || "").includes(`.${runtime}`))) found.push(`${name}: no listed stylesheet styles .${runtime}`);
    }
  }
  return found;
}

const kiln = loadConsole();
const inventory = JSON.parse(read("docs/console-port-inventory.json"));
const copy = () => JSON.parse(JSON.stringify(inventory));
const route = (value, page) => value.routes.find((entry) => entry.page === page);

// r[verify kiln.web.console_inventory] r[verify kiln.web.console_inventory.scenario.complete]
test("the inventory matches the served console", () => {
  assert.deepEqual(problems(inventory, kiln), []);
  assert.equal(inventory.routes.length, kiln.destinations.length);
});

// r[verify kiln.web.console_inventory.scenario.complete]
test("omitted, unknown, duplicated or misfiled destinations fail", () => {
  const cases = [
    [(value) => { value.routes = value.routes.filter((entry) => entry.page !== "feeds"); }, "destination feeds is not inventoried"],
    [(value) => { value.routes.push({ ...route(value, "feeds"), page: "ledger" }); }, "route ledger is not a registered destination"],
    [(value) => { value.routes.push(route(value, "cobs")); }, "route cobs is inventoried twice"],
    [(value) => { route(value, "source").number = "3.9"; }, "route source: number 3.9 differs from registered 3.1"],
    [(value) => { route(value, "source").cohort = "graphics"; }, "route source: cohort graphics is not declared"],
    [(value) => { value.cohorts.push({ id: "native", order: 7, title: "Native client" }); }, "cohort native has no route"],
    [(value) => { route(value, "runs").cohort = "receipts-administration"; }, "route runs is not in the first cohort"],
    [(value) => { route(value, "settings").states = ["frozen"]; }, "route settings: state frozen is not in the state vocabulary"],
  ];
  for (const [mutate, expected] of cases) {
    const value = copy();
    mutate(value);
    assert.ok(problems(value, kiln).includes(expected), expected);
  }
});

// r[verify kiln.web.console_inventory.scenario.drift]
test("a reference the console no longer has fails with its entry", () => {
  const cases = [
    [(value) => { route(value, "activity").panel = "ledger"; }, "route activity: panel ledger is not in web.html"],
    [(value) => { route(value, "source").modules[0].file = "source-tree.js"; }, "route source: module source-tree.js does not exist"],
    [(value) => { route(value, "workers").modules[0].global = "KilnFleet"; }, "route workers: workers.js does not define window.KilnFleet"],
    [(value) => { route(value, "code-review").modules[1].loaded_by = "web.js"; }, "route code-review: loader web.js of preview-review.js is not one of its modules"],
    [(value) => { route(value, "project").stylesheets = ["project-history.css"]; }, "route project: stylesheet project-history.css is not served"],
    [(value) => { route(value, "operations").api = ["/api/capacity"]; }, "route operations: API path /api/capacity is not named by its modules"],
    [(value) => { route(value, "reviews").url_state = ["receipt_page"]; }, "route reviews: URL parameter receipt_page is not read by web.js"],
    [(value) => { route(value, "source").modules[0].node_tests = ["tests/source-tree.test.mjs"]; }, "route source: Node test tests/source-tree.test.mjs does not exist"],
    [(value) => { route(value, "workers").modules[0].node_tests = ["tests/work-views.test.mjs"]; }, "route workers: Node test tests/work-views.test.mjs does not load workers.js"],
    [(value) => { value.shell[0].anchors = ["#brand-door"]; }, "shell brand-mark: anchor #brand-door is not in web.html"],
    [(value) => { value.shell.find((entry) => entry.component === "command-palette").runtime_classes = ["work-search-sheet"]; }, "shell command-palette: work-interaction.js does not create work-search-sheet"],
  ];
  for (const [mutate, expected] of cases) {
    const value = copy();
    mutate(value);
    assert.ok(problems(value, kiln).includes(expected), expected);
  }
});

// The same checks catch a console that changed under an unchanged inventory.
test("a console change without an inventory update fails", () => {
  const renamed = { ...kiln, html: kiln.html.replace('data-panel="feeds"', 'data-panel="subscriptions"') };
  assert.ok(problems(inventory, renamed).includes("route feeds: panel feeds is not in web.html"));
  const added = { ...kiln, destinations: [...kiln.destinations, { page: "ledger", number: "4.5", parent: null }] };
  assert.ok(problems(inventory, added).includes("destination ledger is not inventoried"));
});
