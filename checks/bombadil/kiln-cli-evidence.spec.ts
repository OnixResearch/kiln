// Bombadil terminal specification for the `kiln evidence` operator command.
//
// The check drives the packaged `kiln evidence` command under a pseudo
// terminal with a checked-in input fixture. The command prints the BLAKE3
// content identity of the evidence document it wrote.
//
// r[impl kiln.observe.dashboard.cli_rail]
// r[impl kiln.observe.dashboard.cli_rail.scenario.match]
// r[verify kiln.observe.dashboard.cli_rail]
// r[verify kiln.observe.dashboard.cli_rail.scenario.match]

import { always } from "@antithesishq/bombadil";
import { extract } from "@antithesishq/bombadil/terminal";
export {
  exitSuccess,
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

// The terminal wraps long lines, so the property reads the compact text.
const renderedText = extract((state) =>
  reportRows(state)
    .join("")
    .replace(/\s+/g, ""),
);

const exitCode = extract((state) =>
  state.exitStatus === null ? null : state.exitStatus.code,
);

export const evidenceExitsSuccessfully = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return exitCode.current === 0;
});

export const evidencePrintsAContentIdentity = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return /^[0-9a-f]{64}$/.test(renderedText.current);
});
