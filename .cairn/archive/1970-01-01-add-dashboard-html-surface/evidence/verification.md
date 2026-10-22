# HTML report verification

Date: 2026-10-22
Scope: the dashboard's HTML report, its format and output options, and the
browser property rail over the rendered document.
Base revision: `298e1e27` plus the uncommitted change working tree.

## What was added

- `kiln-dashboard/src/render/html.rs`: a self-contained HTML renderer over the
  same admitted report the terminal renderer takes.
- `kiln-dashboard/src/shell.rs`: a report format selection of text or html,
  defaulting to text, and an output path that writes the report to a file.
- `checks/bombadil/kiln-dashboard-html.spec.ts`: the browser specification.
- Three crate tests that bind the HTML report to the terminal report and assert
  determinism, self-containment, and escaping.

## Results

- `cargo test -p kiln-dashboard`: 26 passed, 0 failed. The new tests assert
  that the HTML report states the observed revision, the profile identities, the
  view identities, the redacted fields, and every declared non-claim, that the
  same admitted state renders the same bytes, that the document carries no
  script and no external resource reference, and that dynamic content is
  escaped.
- `cargo clippy -p kiln-dashboard -p kiln-cli --all-targets -- -D warnings`:
  pass.
- `cargo fmt -p kiln-dashboard -p kiln-cli -- --check`: pass.
- `nix build path:$PWD#checks.x86_64-linux.bombadil -L`: pass, with the
  terminal rail cases unchanged.
- The browser rail over the rendered document: pass, no violation.

## What the browser rail found

The browser rail caught two defects that the Rust tests did not.

- The section headings were missing. The renderer had lost the helper that
  wrapped each section, so the document had the facts but no headings, and the
  rail's property on the visible facts failed in every captured state.
- A style rule that uppercased the table headers rewrote the case of the
  identity schema the document states. The rendered text no longer matched the
  string the terminal report states, so the rail's property on the identity
  header failed in every captured state. The rule now leaves an identifier's
  case alone.

Both defects were fixed and the rail passes.

## The browser rail is not a Nix check

The build sandbox gives the browser no loopback the driver can connect to, and
the builder refuses a derivation that asks to run without the sandbox, so the
browser rail cannot be a Nix check here. It is a local command with the exact
invocation recorded in the operator documentation, and the report's contract is
gated by the crate's tests instead.

## Limits and non-claims

- The HTML report is a snapshot of one admitted revision. It does not refresh.
- The document carries no script and no external resource, so it says what it
  says without an interpreter or a network.
- The browser rail is not an exhaustive exploration of the document. It states
  properties over the document it drives.
- The HTML report does not claim CI success, provider correctness, source
  trust, journal or store completeness, release readiness, or that the observed
  revision is the newest one. It states the same non-claims as the terminal
  report.
