# Design: an HTML report for the operator dashboard

## Context

The dashboard's pipeline is: admit an input, project the accepted observation
views, render. Only the last step is terminal-shaped. The HTML surface is a
second renderer over the same report, so the two surfaces cannot drift on what
they state.

The Bombadil tool already drives the terminal surface. Its browser driver
drives a page in Chromium, which is the right driver for a document.

## Goal and Success Contract

Completion requires these results:

- An HTML report that states the same facts as the terminal report.
- A document that opens without a network: inline style, no script, no
  external resource.
- A deterministic document: the same admitted state renders the same bytes.
- Escaped dynamic content.
- A browser rail that states those properties over the rendered document.
- The terminal report unchanged and still the default.

False completion includes these results:

- A second projection that computes facts the terminal report does not state.
- A document that loads a stylesheet or a script from elsewhere.
- A document whose content depends on the clock or the environment.
- A rail that only checks that the page loads.

## Decisions

### Decision: Render over the admitted report, not over the state

**Choice:** The HTML renderer takes the same `DashboardReport` the terminal
renderer takes.

**Rationale:** The report is the single admitted projection of the state. Two
renderers over one report can differ in presentation; two renderers over the
state could differ in meaning. The parity test then binds the surfaces by
construction.

### Decision: No script and no external resource

**Choice:** The document carries inline style only.

**Rationale:** The dashboard reads a local file and writes a local file. A
document that fetched a stylesheet would make the report's meaning depend on
the network, and a document with script would make its content depend on an
interpreter. The browser rail asserts both absences, so the property is
checked rather than asserted in prose.

### Decision: Keep the rendered text faithful to the identifiers

**Choice:** A table header that names a schema keeps its exact case.

**Rationale:** The browser rail caught a case defect: an uppercase transform
on the header changed the rendered text of an identifier, so the document
stated a different string than the terminal report. A style rule that rewrites
an identifier is a correctness problem, not a presentation one.

### Decision: The browser driver, not the terminal driver

**Choice:** The HTML report is checked with Bombadil's browser driver.

**Rationale:** The browser driver renders the document in a real browser, which
is where the surface is read. It also caught the two defects above that the
Rust tests did not: the missing section headings and the rewritten header.

## Risks / Trade-offs

- The browser rail needs a browser it can reach. The build sandbox gives the
  browser no loopback the driver can connect to, and the builder refuses a
  derivation that asks to run without the sandbox, so the rail is a local
  command with the exact invocation recorded in the operator documentation
  rather than a Nix check. The report's contract stays gated by the crate's
  tests.
- A document is a snapshot. It does not refresh; the watch mode still redraws
  the terminal report, and an operator re-renders the document for a newer
  revision.
- The HTML report is larger than the terminal report. That is the cost of
  carrying the style inline.

## Verification Strategy

- Rust tests bind the HTML report to the terminal report, assert
  determinism, self-containment, and escaping.
- The browser rail drives the rendered document and states the facts, the
  absence of script, and the absence of external resources. The Rust tests
  carry the same properties into the gated suite.
- The negative controls for the browser rail are the defects it found: a
  missing heading and a rewritten header both produced violations.
- The workspace gates and the Cairn gates run on the change.
