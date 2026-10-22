// Bombadil terminal specification for the Kiln operator dashboard.
//
// The check stages the journal fixture next to this file as `journal.json`
// and runs the packaged `kiln-dashboard` binary against it. Every expectation
// is derived from the staged journal and the checked-in dashboard profile, so
// the properties do not rot when a fixture or a bound changes.
//
// The dashboard prints one report and exits, so the terminal trace ends with
// the program gone. A property therefore states its expectation only once the
// program has finished; the states captured while it is still running are
// exempt. The check runs Bombadil with a terminal tall enough to hold the
// whole report, so the finished screen carries every rendered line.
//
// r[impl kiln.observe.dashboard.property_rail]
// r[impl kiln.observe.dashboard.property_rail.scenario.match]
// r[impl kiln.observe.dashboard.property_rail.scenario.bounded]
// r[verify kiln.observe.dashboard.property_rail]
// r[verify kiln.observe.dashboard.property_rail.scenario.match]
// r[verify kiln.observe.dashboard.property_rail.scenario.bounded]
// r[impl kiln.observe.dashboard.property_rail.nonclaims]
// r[impl kiln.observe.dashboard.property_rail.nonclaims.scenario.scope]
//
// The rail checks the rendered surface only. It does not claim CI success,
// provider correctness, journal completeness, release readiness, or that the
// exploration covered every possible journal, profile, or terminal input.

import { always } from "@antithesishq/bombadil";
import { extract } from "@antithesishq/bombadil/terminal";
export {
  exitSuccess,
  noReplacementChars,
  typeBasicInput,
} from "@antithesishq/bombadil/terminal/defaults";

import profile from "../../fixtures/dashboard/profile-v1.json" with { type: "json" };
import journal from "./journal.json" with { type: "json" };

const dashboardProfile = profile.profile;
const journalRecords = journal.records;

const reportRows = (state: {
  scrollback: { size: { rows: number }; rowText(index: number): string };
  grid: { size: { rows: number }; rowText(index: number): string };
}): string[] => {
  const rows: string[] = [];
  for (let index = 0; index < state.scrollback.size.rows; index++) {
    rows.push(state.scrollback.rowText(index));
  }
  for (let index = 0; index < state.grid.size.rows; index++) {
    rows.push(state.grid.rowText(index));
  }
  return rows;
};

const renderedText = extract((state) => reportRows(state).join("\n"));

const exitCode = extract((state) =>
  state.exitStatus === null ? null : state.exitStatus.code,
);

const renderedRunRows = extract(
  (state) => reportRows(state).filter((row) => /^ {2}run-\S+\s/.test(row)).length,
);

const summaryRunCount = extract((state) => {
  for (const row of reportRows(state)) {
    const match = /^ {2}runs\s+(\d+)\s/.exec(row);
    if (match) {
      return Number(match[1]);
    }
  }
  return -1;
});

// The number of run rows the admitted profile admits: one page size per
// consulted page.
const admittedRunRows =
  dashboardProfile.observation.bounds.max_page_items * dashboardProfile.max_run_pages;

export const finishedReportStatesItsFacts = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  const text = renderedText.current;
  return (
    new RegExp(`state revision\\s+${journalRecords.length}\\b`).test(text) &&
    new RegExp(`profile\\s+${dashboardProfile.profile_id}\\b`).test(text) &&
    dashboardProfile.non_claims.every((nonClaim) => text.includes(nonClaim))
  );
});

export const finishedReportRespectsTheAdmittedRunBound = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  const rendered = renderedRunRows.current;
  return rendered >= 0 && rendered <= admittedRunRows;
});

export const finishedReportAgreesWithItsOwnSummary = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  const rendered = renderedRunRows.current;
  const summary = summaryRunCount.current;
  if (rendered < 0 || summary < 0) {
    return false;
  }
  if (rendered > summary) {
    return false;
  }
  if (renderedText.current.includes("truncated")) {
    return rendered < summary;
  }
  return rendered === summary;
});
