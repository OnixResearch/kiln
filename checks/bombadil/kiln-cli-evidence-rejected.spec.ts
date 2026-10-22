// Bombadil terminal specification for the `kiln evidence` operator command
// when its input document does not admit.
//
// The check drives the packaged `kiln evidence` command under a pseudo
// terminal with a malformed input fixture. The command must refuse the input,
// name the failure, and write no evidence document.
//
// r[impl kiln.observe.dashboard.cli_rail]
// r[impl kiln.observe.dashboard.cli_rail.scenario.match]
// r[verify kiln.observe.dashboard.cli_rail]
// r[verify kiln.observe.dashboard.cli_rail.scenario.match]

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

const renderedText = extract((state) => reportRows(state).join("\n").trimEnd());

const exitCode = extract((state) =>
  state.exitStatus === null ? null : state.exitStatus.code,
);

export const rejectedEvidenceExitsWithAFailure = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return exitCode.current !== 0;
});

export const rejectedEvidenceNamesTheFailure = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return (
    renderedText.current.includes("evidence input read failed") ||
    renderedText.current.includes("evidence JSON parse failed")
  );
});
