# Property-based terminal rail verification

Date: 2026-10-21
Scope: a property-based terminal rail over the operator dashboard's rendered
surface, driven by the pinned Bombadil property-testing tool.
Base revision: `c90ba3d` plus the uncommitted change working tree.

## What was added

- A pinned Bombadil flake input at revision
  `832faf4d953e2c56d1d901aca1bb2c3c3170221d`, nar hash
  `sha256-Yh0tzFQv9EMBXuHQRtKIwjOxpHi0jPls9Z8m1t79Kf4=`.
- `checks/bombadil/kiln-dashboard.spec.ts` and
  `checks/bombadil/kiln-dashboard-rejected.spec.ts`: the terminal
  specifications.
- `checks/bombadil/journal-4-runs.json`,
  `checks/bombadil/journal-20-runs.json`, and
  `checks/bombadil/journal-tampered.json`: the journal fixtures.
- A Nix check that runs the specifications over all three fixtures and keeps
  the captured terminal traces as build outputs.
- Two new evidence roots in the local traceability policy, for the
  TypeScript specifications and the flake, and the regenerated exported
  policy.
- A license map row for the separately executed tool.

## Results

- `nix build path:$PWD#checks.x86_64-linux.bombadil -L`: pass. The check
  drives the packaged `kiln-dashboard` binary under a pseudo terminal inside
  the Nix sandbox. The admitted fixture exits zero, the bounded fixture exits
  zero and reports its truncation, and the rejected fixture exits one with the
  admission failure named. The captured traces are kept as build outputs.
- Every property can fail. Each was exercised against a deliberately wrong
  expectation and reported a violation: a wrong expected revision violated
  `finishedReportStatesItsFacts`, an understated run bound violated
  `finishedReportRespectsTheAdmittedRunBound`, and a success expectation on
  the rejected fixture violated `rejectedJournalExitsWithAFailure`.
- `cargo fmt -- --check`: pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass.
- `cargo test --workspace`: pass.
- `cairn gate proposal|design|tasks add-bombadil-dashboard-property-rail
  --root .`: PASS.
- `cairn policy export --source cairn-policy/default.ncl --check`: fresh.
- `cairn sync add-bombadil-dashboard-property-rail --root . --execute`:
  applied the requirements to `.cairn/specs/bombadil-dashboard-rail/spec.md`.
- `cairn traceability coverage --root .`: pass, 846 of 846 requirements
  referenced with no missing or dangling references.
- `cairn archive add-bombadil-dashboard-property-rail --root . --execute`:
  archived to `.cairn/archive/1970-01-01-add-bombadil-dashboard-property-rail/`.
- `cairn validate --root .` after archive: `valid: true`.

## Why the properties use `always` with an exit-status condition

The dashboard prints one report and exits, so the terminal trace ends with the
program gone. On a terminated trace Bombadil's `eventually` operator is
vacuously true, so a property written with `eventually` cannot fail. The
rail therefore uses `always` with a condition that the captured state carries
the program's exit status: the expectation is stated once the program has
finished, and the partial screens captured while the report is being written
are exempt.

The terminal driver keeps the scrolled-off lines in a scrollback buffer, but
the captured scrollback was empty in practice, so the check runs the tool
with a terminal tall and wide enough to hold the whole report. That is what
makes the header facts checkable.

## The tool

- Source: `https://github.com/antithesishq/bombadil`
- Revision: `832faf4d953e2c56d1d901aca1bb2c3c3170221d`
- Pin: flake input `bombadil`, nar hash
  `sha256-Yh0tzFQv9EMBXuHQRtKIwjOxpHi0jPls9Z8m1t79Kf4=`
- Terms: `MIT`. The tool is separately executed and never linked into a Kiln
  package.

## Limits and non-claims

- The rail covers three fixtures. It does not claim to have explored every
  journal, profile, or terminal input.
- The rail checks the rendered surface only. It does not prove CI success,
  provider correctness, source trust, journal completeness, or release
  readiness.
- The rail does not prove the property-testing tool correct. It proves that
  the recorded properties held over the recorded terminal sessions.
- The terminal session depends on the driver's sampling. A program that
  exited before the first sample would leave the properties unfalsified; the
  packaged dashboard prints before it exits and the check fails loudly when
  Bombadil reports no state.
- No Bombadil code, crate, or library enters the Kiln source tree or any
  Kiln artifact.
