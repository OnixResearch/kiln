// Real live-console properties; actions are same-origin, server-read-only.
// r[impl kiln.web.exploration.browser_property_rail.scenario.properties]
// r[verify kiln.web.exploration.browser_property_rail.scenario.properties]
import { always, eventually } from "@antithesishq/bombadil";
import { actions, extract, registerCustomAction, weighted } from "@antithesishq/bombadil/browser";
import context from "./web-exploration-context.json" with { type: "json" };
export {
  noHttpErrorCodes,
  noUncaughtExceptions,
  noUnhandledPromiseRejections,
  noConsoleErrors,
} from "@antithesishq/bombadil/browser/defaults";

const page = extract((state) => {
  const document = state.document;
  const window = state.window;
  const visible = (node: Element | null) => Boolean(node && node.getClientRects().length && !node.closest("[hidden]"));
  const url = new URL(window.location.href);
  const index = url.pathname === "/examples";
  const routePage = url.searchParams.get("page") ?? "overview";
  const run = url.searchParams.get("run") ?? "";
  const panels = Array.from(document.querySelectorAll<HTMLElement>("[data-panel]")).filter(visible);
  const loading = Array.from(document.querySelectorAll(".loading-state, [aria-busy=true]")).some(visible);
  const detail = document.getElementById("run-detail");
  const exactRun = document.querySelector("#run-exact-facts dd")?.textContent?.trim() ?? "";
  const cards = Array.from(document.querySelectorAll<HTMLElement>("#scenarios > article")).map((card) => ({
    text: `${card.querySelector("h3")?.textContent ?? ""} ${card.querySelector("p")?.textContent ?? ""}`,
    visible: !card.hidden,
    origin: card.querySelector<HTMLAnchorElement>("a")?.origin ?? "",
  }));
  const ready = document.readyState === "complete" && (index
    ? cards.length > 0 && document.getElementById("example-filter") !== null
    : visible(document.getElementById("workspace")) && panels.length === 1 && !loading &&
      (routePage !== "runs" || !run || (visible(detail) && exactRun !== "")));
  const safeHistory = (entries: { url: string }[]) => entries.length > 0 && entries.every((entry) => {
    try {
      const target = new URL(entry.url);
      return target.origin === url.origin && ["/", "/examples"].includes(target.pathname);
    } catch { return false; }
  });
  return {
    href: url.href, origin: url.origin, path: url.pathname, index, routePage, run,
    view: url.searchParams.get("view") ?? "pipeline", ready,
    theme: document.documentElement.dataset.theme ?? null,
    requestedTheme: url.searchParams.get("theme"),
    panels: panels.map((panel) => panel.dataset.panel),
    exactRun,
    currentTabs: Array.from(document.querySelectorAll<HTMLElement>("#run-tabs [aria-current=page]")).map((tab) => tab.dataset.view),
    runPanels: Array.from(document.querySelectorAll<HTMLElement>("[data-run-panel]")).filter(visible).map((panel) => panel.dataset.runPanel),
    tabLinks: Array.from(document.querySelectorAll<HTMLAnchorElement>("#run-tabs a")).map((link) => link.href),
    appearanceLinks: ["light", "dark"].map((theme) => {
      const target = new URL(url.href);
      target.searchParams.set("theme", theme);
      return target.href;
    }),
    query: (document.getElementById("example-filter") as HTMLInputElement | null)?.value ?? "",
    widePrimarySplit: document.querySelector(".run-primary-panels")?.classList.contains("is-split") ?? false,
    cards,
    errors: Array.from(document.querySelectorAll(".error-state, #notice[data-kind=error]")).filter(visible).map((node) => node.textContent),
    width: window.innerWidth, scrollWidth: document.documentElement.scrollWidth,
    back: safeHistory(state.navigationHistory.back), forward: safeHistory(state.navigationHistory.forward),
  };
});

// Readiness is bounded, and actions wait instead of navigating away from a stuck load.
export const becomesReady = always(eventually(() => page.current.ready).within(15, "seconds"));
export const remainsInTheExampleOrigin = always(() =>
  page.current.origin === context.origin && ["/", "/examples"].includes(page.current.path)
);
export const showsNoVisibleLoadFailure = always(() => page.current.errors.length === 0);
export const fitsTheViewport = always(() => !page.current.ready || page.current.scrollWidth <= page.current.width + 1);
export const restoresTheSelectedPage = always(() => {
  const view = page.current;
  return !view.ready || view.index || (view.panels.length === 1 && view.panels[0] === view.routePage);
});
export const restoresExplicitAppearance = always(() => {
  const view = page.current;
  return !view.ready || view.index || !view.requestedTheme || view.theme === view.requestedTheme;
});
export const filtersTheScenarioIndex = always(() => {
  const view = page.current;
  if (!view.ready || !view.index) return true;
  const query = view.query.trim().toLowerCase();
  return view.cards.every((card) => card.visible === card.text.toLowerCase().includes(query));
});
export const indexLinksRemainInTheExampleOrigin = always(() => !page.current.ready || !page.current.index ||
  page.current.cards.every((card) => card.origin === context.origin)
);
export const restoresExactRunAndTab = always(() => {
  const view = page.current;
  if (!view.ready || view.index || view.routePage !== "runs" || !view.run) return true;
  return view.exactRun === view.run && view.currentTabs.length === 1 && view.currentTabs[0] === view.view &&
    view.runPanels.includes(view.view) && view.runPanels.every((panel) => panel === view.view ||
      ((view.widePrimarySplit || ["pipeline", "logs"].includes(view.view)) && ["pipeline", "logs"].includes(panel ?? "")));
});

// r[impl kiln.web.exploration.browser_property_rail.scenario.isolation]
// No unrestricted default click/input generator or application-private call.
const navigate = registerCustomAction("open-example-route", async (_document, window, href: string) => {
  const target = new URL(href, window.location.href);
  if (target.origin !== window.location.origin || !["/", "/examples"].includes(target.pathname)) {
    throw new Error("Refusing a route outside the example origin");
  }
  window.location.assign(target.href);
});
const filterIndex = registerCustomAction("filter-example-index", async (document, _window, query: string) => {
  const input = document.getElementById("example-filter") as HTMLInputElement | null;
  if (!input) throw new Error("Example index search is unavailable");
  input.focus();
  input.value = query;
  input.dispatchEvent(new Event("input", { bubbles: true }));
});
const routes = actions(() => context.examples.map((example) => navigate(`${context.origin}${example.href}`)));
const index = actions(() => [navigate(`${context.origin}/examples`)]);
const filterBoundaries = actions(() => {
  const view = page.current;
  if (!view.index) return [];
  let absent = "__bombadil_missing_feature__";
  while (view.cards.some((card) => card.text.toLowerCase().includes(absent))) absent += "_";
  return [filterIndex(""), filterIndex(absent)];
});
const filterMatches = actions(() => {
  if (!page.current.index) return [];
  const terms = [...new Set(context.examples.map((example) => example.title.split(/\s+/)[0]).filter(Boolean))];
  return terms.flatMap((term) => [term, `  ${term.toUpperCase()}  `]).map((query) => filterIndex(query));
});
const filters = weighted([[1, filterBoundaries], [1, filterMatches]]);
const runTabs = actions(() => page.current.routePage === "runs" && page.current.run
  ? page.current.tabLinks.map((href) => navigate(href)) : []);
const history = actions(() => [
  "Reload" as const,
  ...(page.current.back ? ["Back" as const] : []),
  ...(page.current.forward ? ["Forward" as const] : []),
]);
const viewport = actions(() => [
  { SetViewport: { width: 390, height: 844 } },
  { SetViewport: { width: 768, height: 900 } },
  { SetViewport: { width: 1440, height: 1000 } },
]);
const appearance = actions(() => page.current.index ? [] : page.current.appearanceLinks.map((href) => navigate(href)));
const exploreReady = weighted([[6, routes], [3, index], [5, filters], [4, runTabs], [1, history], [1, viewport], [1, appearance]]);
// r[impl kiln.web.exploration.browser_property_rail.scenario.evidence]
export const explore = actions(() => page.current.ready ? exploreReady.generate() : ["Wait"]);
