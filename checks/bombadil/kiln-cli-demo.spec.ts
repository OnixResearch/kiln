// Bombadil terminal specification for the `kiln` operator CLI demo surface.
//
// The check drives the packaged `kiln demo` command under a pseudo terminal.
// The demo prints the canonical state of one committed run, so the properties
// state what the operator must be able to read from it.
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

// The terminal wraps long lines, so the properties read the compact text: the
// rendered characters without the terminal's own line breaks and padding.
const renderedText = extract((state) =>
  reportRows(state)
    .join("")
    .replace(/\s+/g, ""),
);

const exitCode = extract((state) =>
  state.exitStatus === null ? null : state.exitStatus.code,
);

export const demoExitsSuccessfully = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  return exitCode.current === 0;
});

export const demoPrintsTheCanonicalRun = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  const text = renderedText.current;
  return (
    text.includes("demo-run") &&
    text.includes("demo-attempt") &&
    text.includes("standalone-build-v1")
  );
});

export const demoPrintsOneCanonicalState = always(() => {
  if (exitCode.current === null) {
    return true;
  }
  const text = renderedText.current;
  return text.startsWith("{") && text.endsWith("}") && text.includes("\"revision\":");
});
