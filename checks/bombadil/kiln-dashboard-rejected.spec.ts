// Bombadil terminal specification for the Kiln operator dashboard when the
// staged journal does not admit.
//
// The check stages the tampered journal fixture next to this file as
// `journal.json`. The dashboard must refuse it, name the admission failure,
// and leave no report behind.
//
// r[impl kiln.observe.dashboard.property_rail.scenario.rejected]
// r[verify kiln.observe.dashboard.property_rail.scenario.rejected]

import { always } from "@antithesishq/bombadil";
import { extract } from "@antithesishq/bombadil/terminal";
export {
  noReplacementChars,
  typeBasicInput,
} from "@antithesishq/bombadil/terminal/defaults";

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

export const rejectedJournalExitsWithAFailure = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return exitCode.current !== 0;
});

export const rejectedJournalNamesTheAdmissionFailure = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return renderedText.current.includes("event_identity_mismatch");
});

export const rejectedJournalRendersNoReport = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return !renderedText.current.includes("state revision");
});
