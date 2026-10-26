// Bombadil browser specification for the Kiln operator dashboard HTML report.
//
// Drive a generated local report, including its native filtering and drilldown
// controls. Properties inspect rendered facts and visible results, never the
// interaction script's source. This rail does not exercise CI or remote input.
//
// r[impl kiln.observe.dashboard.html.browser_rail]
// r[verify kiln.observe.dashboard.html.browser_rail]
// r[verify kiln.observe.dashboard.html.browser_rail.scenario.match]

import { always } from "@antithesishq/bombadil";
import { ActionGenerator, branch } from "@antithesishq/bombadil/actions";
import {
  actions,
  extract,
  registerCustomAction,
} from "@antithesishq/bombadil/browser";
export {
  noHttpErrorCodes,
  noUncaughtExceptions,
  noUnhandledPromiseRejections,
  noConsoleErrors,
} from "@antithesishq/bombadil/browser/defaults";
import {
  clicks,
  scroll,
  waitOnce,
} from "@antithesishq/bombadil/browser/defaults/actions";

const dashboard = extract((state) => {
  const document = state.document;
  const value = (id: string) =>
    (document.getElementById(id) as HTMLInputElement | HTMLSelectElement | null)
      ?.value ?? null;
  const options = (id: string) =>
    Array.from(document.querySelectorAll<HTMLOptionElement>(`#${id} option`))
      .map((option) => option.value);
  const rows = Array.from(
    document.querySelectorAll<HTMLTableRowElement>(
      "#runs-table tbody tr[data-run-row]",
    ),
  ).map((row) => ({
    id: row.dataset.runId ?? "",
    active: row.dataset.activeAttempt ?? "",
    outcome: row.dataset.outcome ?? "",
    attempts: Number(row.dataset.attemptCount),
    visible: !row.hidden,
  }));
  const runs = Array.from(
    document.querySelectorAll<HTMLElement>("[data-run-detail]"),
  ).map((panel) => ({
    id: panel.dataset.runId ?? "",
    active: panel.dataset.activeAttempt ?? "",
    initial: panel.hasAttribute("data-initial"),
    visible: !panel.hidden,
    text: panel.innerText,
  }));
  const attempts = Array.from(
    document.querySelectorAll<HTMLElement>("[data-attempt-detail]"),
  ).map((panel) => ({
    id: panel.dataset.attemptId ?? "",
    run: panel.dataset.runId ?? "",
    found: panel.dataset.found === "true",
    initial: panel.hasAttribute("data-initial"),
    visible: !panel.hidden,
    text: panel.innerText,
  }));
  const attemptChoices = Array.from(
    document.querySelectorAll<HTMLButtonElement>("button[data-select-attempt]"),
  ).filter((button) => !button.closest("[hidden]"))
    .map((button) => button.dataset.attemptId ?? "");
  const fragment = new URLSearchParams(state.window.location.hash.slice(1));
  return {
    query: value("run-search"),
    outcome: value("outcome-filter"),
    sort: value("run-sort"),
    direction: value("sort-direction"),
    fragmentQuery: fragment.get("q") ?? "",
    fragmentOutcome: fragment.get("outcome") ?? "",
    fragmentSort: fragment.get("sort") ?? "id",
    fragmentDirection: fragment.get("direction") ?? "asc",
    outcomes: options("outcome-filter"),
    sorts: options("run-sort"),
    directions: options("sort-direction"),
    rows,
    runs,
    attempts,
    attemptChoices,
    run: state.window.location.hash.slice(1)
      ? fragment.get("run") ?? ""
      : runs.find((panel) => panel.initial)?.id ?? "",
    attempt: state.window.location.hash.slice(1)
      ? fragment.get("attempt") ?? ""
      : attempts.find((panel) => panel.initial)?.id ?? "",
    runStatus: document.getElementById("selection-status")?.innerText ?? "",
    attemptStatus:
      document.getElementById("attempt-selection-status")?.innerText ?? "",
    empty: document.getElementById("run-empty")?.hidden === false,
    resourceReferences: document.querySelectorAll(
      "link[href], script[src], img[src], img[srcset], iframe[src], " +
      "object[data], embed[src], audio[src], video[src], source[src], " +
      "source[srcset], track[src], input[type=image][src]",
    ).length,
    resourceLoads: state.window.performance.getEntriesByType("resource")
      .map((entry) => entry.name),
  };
});

// Offline resource isolation during control exploration; projection parity and
// canonical-state immutability are separate checks, not inferred from the DOM.
// r[verify kiln.observe.dashboard.html.scenario.self_contained]
export const loadsNoExternalResource = always(() =>
  dashboard.current.resourceReferences === 0 &&
  dashboard.current.resourceLoads.length === 0
);

// The browser portion of live update: fragment controls survive reload actions.
// Atomic file replacement and the profile-derived interval require shell proof.
// r[verify kiln.observe.dashboard.html.live_update]
// r[verify kiln.observe.dashboard.html.live_update.scenario.match]
export const restoresTheFragmentControls = always(() => {
  const view = dashboard.current;
  return view.query === view.fragmentQuery &&
    view.outcome === view.fragmentOutcome &&
    view.sort === view.fragmentSort &&
    view.direction === view.fragmentDirection;
});

export const filtersTheAdmittedRuns = always(() => {
  const view = dashboard.current;
  if (view.query === null || view.outcome === null) return false;
  const query = view.query.trim().toLowerCase();
  const expected = view.rows.filter((row) =>
    (row.id.toLowerCase().includes(query) ||
      row.active.toLowerCase().includes(query)) &&
    (!view.outcome || row.outcome === view.outcome)
  );
  return view.rows.every((row) => row.visible === expected.includes(row)) &&
    view.empty === (expected.length === 0);
});

export const sortsTheVisibleRuns = always(() => {
  const view = dashboard.current;
  if (!view.sort || !view.direction) return false;
  const rows = view.rows.filter((row) => row.visible);
  const compareText = (left: string, right: string) =>
    left < right ? -1 : left > right ? 1 : 0;
  const sorted = [...rows].sort((left, right) => {
    const primary = view.sort === "attempts"
      ? left.attempts - right.attempts
      : compareText(
        view.sort === "outcome" ? left.outcome : left.id,
        view.sort === "outcome" ? right.outcome : right.id,
      );
    return primary * (view.direction === "desc" ? -1 : 1) ||
      compareText(left.id, right.id);
  });
  return rows.every((row, index) => row.id === sorted[index].id);
});

// r[verify kiln.observe.dashboard.html.interactive.scenario.navigation]
export const showsTheSelectedCanonicalDetails = always(() => {
  const view = dashboard.current;
  const run = view.runs.find((panel) => panel.id === view.run);
  const attempt = view.attempts.find((panel) => panel.id === view.attempt);
  const visibleRuns = view.runs.filter((panel) => panel.visible);
  const visibleAttempts = view.attempts.filter((panel) => panel.visible);
  const runMatches = run
    ? visibleRuns.length === 1 && visibleRuns[0].id === run.id &&
      run.text.includes(run.id)
    : visibleRuns.length === 0 &&
      (!view.run || view.runStatus.includes(view.run));
  const attemptMatches = attempt && (!attempt.found || attempt.run === view.run)
    ? visibleAttempts.length === 1 && visibleAttempts[0].id === attempt.id &&
      attempt.text.includes(attempt.id)
    : visibleAttempts.length === 0 &&
      (!view.attempt || view.attemptStatus.includes(view.attempt));
  return runMatches && attemptMatches;
});

// Pinned Bombadil exposes native input/select changes through custom actions.
// These handlers touch controls only; they do not invoke dashboard internals.
const changeControl = registerCustomAction(
  "dashboard-change-control",
  async (document, window, id: string, value: string) => {
    const control = document.getElementById(id) as
      HTMLInputElement | HTMLSelectElement;
    control.focus();
    control.value = value;
    control.dispatchEvent(new Event("input", { bubbles: true }));
    control.dispatchEvent(new Event("change", { bubbles: true }));
  },
);

const selectDetail = registerCustomAction(
  "dashboard-select-detail",
  async (document, window, kind: string, id: string) => {
    const attribute = kind === "run" ? "data-run-id" : "data-attempt-id";
    const button = Array.from(document.querySelectorAll<HTMLButtonElement>(
      kind === "run" ? "button[data-select-run]" : "button[data-select-attempt]",
    )).find((candidate) => candidate.getAttribute(attribute) === id &&
      !candidate.closest<HTMLElement>("[hidden]"));
    if (!button) throw new Error(`Dashboard ${kind} control is missing: ${id}`);
    button.click();
    const fragment = new URLSearchParams(window.location.hash.slice(1));
    const expectedRun = button.dataset.runId;
    const expectedAttempt = kind === "attempt" ? id :
      document.querySelector<HTMLElement>("[data-run-detail]:not([hidden])")
        ?.dataset.activeAttempt;
    if (fragment.get("run") !== expectedRun ||
      fragment.get("attempt") !== expectedAttempt) {
      throw new Error(`Dashboard did not select the requested ${kind}: ${id}`);
    }
  },
);

const changeFragment = registerCustomAction(
  "dashboard-change-fragment",
  async (document, window, run: string, attempt: string) => {
    const fragment = new URLSearchParams(window.location.hash.slice(1));
    fragment.set("run", run);
    fragment.set("attempt", attempt);
    window.location.hash = fragment.toString();
  },
);

const controls = actions(() => {
  const view = dashboard.current;
  const absentRun = `${view.runs.map((run) => run.id).join("|")}|missing-run`;
  const absentAttempt =
    `${view.attempts.map((attempt) => attempt.id).join("|")}|missing-attempt`;
  const queries = ["", absentRun, ...view.rows.flatMap((row) =>
    [row.id, row.id.toUpperCase(), row.active].filter(Boolean)
  )];
  return [
    ...queries.map((value) => changeControl("run-search", value)),
    ...view.outcomes.map((value) => changeControl("outcome-filter", value)),
    ...view.sorts.map((value) => changeControl("run-sort", value)),
    ...view.directions.map((value) => changeControl("sort-direction", value)),
    ...view.rows.filter((row) => row.visible)
      .map((row) => selectDetail("run", row.id)),
    ...view.attemptChoices.map((id) => selectDetail("attempt", id)),
    changeFragment(absentRun, absentAttempt),
    ...view.runs.flatMap((run) => view.attempts
      .filter((attempt) => attempt.run !== run.id)
      .map((attempt) => changeFragment(run.id, attempt.id))),
    "Reload" as const,
  ];
});

export const explore = new ActionGenerator(() =>
  branch([
    [100, controls.generate()],
    [30, clicks.generate()],
    [20, scroll.generate()],
    [10, waitOnce.generate()],
  ])
);
