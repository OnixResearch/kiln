// Bombadil browser specification for the Kiln operator dashboard HTML report.
//
// The check renders the packaged dashboard to a self-contained HTML document
// and drives it in a browser. The properties state what the document must
// show, and what it must not contain.
//
// r[impl kiln.observe.dashboard.html.browser_rail]
// r[verify kiln.observe.dashboard.html.browser_rail]
// r[verify kiln.observe.dashboard.html.browser_rail.scenario.match]

import { always } from "@antithesishq/bombadil";
import { ActionGenerator, branch } from "@antithesishq/bombadil/actions";
import { extract } from "@antithesishq/bombadil/browser";
export {
  noHttpErrorCodes,
  noUncaughtExceptions,
  noUnhandledPromiseRejections,
  noConsoleErrors,
} from "@antithesishq/bombadil/browser/defaults";
import {
  scroll,
  waitOnce,
} from "@antithesishq/bombadil/browser/defaults/actions";

const documentText = extract((state) => state.document.body.innerText);

const scriptCount = extract(
  (state) => state.document.querySelectorAll("script").length,
);

const externalResourceCount = extract(
  (state) =>
    state.document.querySelectorAll(
      "link[rel='stylesheet'], img, iframe, script[src]",
    ).length,
);

export const statesTheObservedFacts = always(
  () =>
    documentText.current.includes("state revision") &&
    documentText.current.includes("non-claims") &&
    documentText.current.includes("kiln.observe.view.v1 identity"),
);

export const carriesNoScript = always(() => scriptCount.current === 0);

export const loadsNoExternalResource = always(
  () => externalResourceCount.current === 0,
);

export const explore = new ActionGenerator(() =>
  branch([
    [100, scroll.generate()],
    [10, waitOnce.generate()],
  ]),
);
