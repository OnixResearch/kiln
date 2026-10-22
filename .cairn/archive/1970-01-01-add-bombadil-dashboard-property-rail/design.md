# Design: property-based terminal rail for the operator dashboard

## Context

The dashboard renders one bounded report and exits. Its properties are
invariants over the rendered screen, which is the shape a property-based
tester is built for: explore inputs, capture states, assert invariants.

Bombadil's terminal driver runs a program under a pseudo terminal, samples the
screen, and evaluates exported properties over the captured states. Its
specification language is TypeScript with an embedded runtime, so a
specification is a plain source file in the repository and the tool needs no
JavaScript toolchain in the build.

## Goal and Success Contract

The goal is a repeatable property rail over the dashboard's rendered surface.

Completion requires these results:

- The rail runs the packaged dashboard under a pseudo terminal and checks
  properties derived from the fixture it was given.
- The rail covers an admitted journal, a journal larger than the admitted run
  bound, and a journal that does not admit.
- Every property can fail. A deliberately wrong expectation must be reported
  as a violation.
- The tool is pinned to one exact revision, is separately executed, and is
  never linked into a Kiln package.
- The expectations are derived from the fixture and the profile, not copied
  from a previous run of the tool.

False completion includes these results:

- Properties that cannot fail, such as an expectation over a trace that never
  reaches the state it inspects.
- Expectations that restate a constant instead of deriving it from the
  fixture or the profile.
- A rail that only covers the happy path.
- Linking the tool into a Kiln package.

## Decisions

### Decision: Drive the packaged binary through the terminal driver

**Choice:** The check runs `${packageFor system}/bin/kiln-dashboard` under
Bombadil's terminal driver with the journal and profile passed as ordinary
arguments.

**Rationale:** The dashboard's surface is a terminal. Driving the real
packaged binary through a pseudo terminal exercises the same path an operator
uses, including the ANSI styling path that only a terminal enables. The
alternative, calling the rendering core in-process, would not exercise the
shell at all.

### Decision: Derive every expectation from the fixture and the profile

**Choice:** The specification imports the staged journal and the checked-in
dashboard profile, and derives the expected revision, the profile identity, the
non-claims, and the admitted run bound from them.

**Rationale:** A property that restates a constant stops being a property when
the constant moves. Deriving the expectations from the inputs keeps the rail
meaningful when a bound or a fixture changes, and makes the fixture and the
profile the only places a value is written down.

### Decision: State the expectation only after the program has finished

**Choice:** Each property is an `always` formula that holds trivially while
the program is still running and states its expectation once the captured
state carries the program's exit status.

**Rationale:** The dashboard prints its report and exits, so the terminal
trace ends with the program gone. On a terminated trace Bombadil's
`eventually` operator is vacuously true, which would make every property
unfalsifiable. The exit-status condition keeps the property falsifiable and
still exempts the partial screens captured while the report is being written.

### Decision: Size the terminal so the whole report is on screen

**Choice:** The check runs Bombadil with a tall and wide terminal.

**Rationale:** The terminal driver keeps the scrolled-off lines in a
scrollback buffer, but the captured scrollback was empty in practice, so the
visible screen is the only reliable source of the report text. A tall
terminal keeps the whole report on one screen, which is what makes the header
facts checkable.

### Decision: Keep the tool separately executed and pinned

**Choice:** The flake pins one Bombadil revision as an input and runs its
binary from the Nix check. No Bombadil crate is a Cargo dependency of any
Kiln package, and the license map records the revision and its terms.

**Rationale:** Kiln's license map already distinguishes linked dependencies
from separately executed tools. Bombadil is a test tool that observes the
dashboard from outside; linking it would change the dependency and license
surface of every Kiln package for no product reason.

### Decision: Extend the local traceability policy for TypeScript evidence

**Choice:** The local policy counts `checks/bombadil` TypeScript sources and
the flake as evidence roots.

**Rationale:** The properties are implemented in TypeScript and pinned in the
flake. Without the evidence roots the requirements would have no traceable
implementation, and the coverage rail would report them as dangling.

## Risks / Trade-offs

- The rail covers three fixtures. It is not an exhaustive exploration of the
  journal space, and the report says so.
- The rail depends on the terminal driver's sampling. A program that exits
  before the first sample would produce an empty trace; the exit-status
  condition would then leave the properties unfalsified. The packaged
  dashboard prints before it exits, and the check fails loudly if Bombadil
  reports no state.
- The rail adds an external tool to the build. The pin and the license map
  row keep that acquisition explicit.
- A property that holds only because the trace is short is a real risk. Each
  property was checked against a deliberately wrong expectation to confirm
  that it can fail.

## Verification Strategy

- The Nix check runs the specification over the three fixtures and keeps the
  traces.
- Each property was exercised against a wrong expectation and reported a
  violation, which is recorded in the change evidence.
- The existing workspace gates, the Nickel gate, and the Cairn gates run
  unchanged.
