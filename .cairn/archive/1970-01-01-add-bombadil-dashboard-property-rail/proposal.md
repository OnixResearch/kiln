# Add a property-based terminal rail for the operator dashboard

## Why

The operator dashboard added a terminal surface with no property coverage. Its
existing tests are example-based: one journal with one run, one journal with a
tampered event, and a rendering comparison. They check that the code does what
the test author expected on that day. They do not state the properties the
surface is supposed to hold, and they do not explore the terminal input space
at all.

The surface is exactly where a property is cheap. The dashboard renders one
report and exits, so the properties are invariants over the rendered screen:
the report states the revision it observed, the report never shows more runs
than the admitted profile admits, the summary and the rendered rows agree
unless the report says it truncated, and a journal that does not admit is
refused with a named failure and no report.

Antithesis's Bombadil is a property-based testing tool for web and terminal
interfaces. Its terminal driver runs the program under a pseudo terminal,
explores the input space, captures the screen, and checks exported properties
over the captured states. It is the tool the workspace research brief points
at for property-based testing integration.

## What Changes

- Pin the Bombadil flake at one exact revision and record its license in the
  license map. The tool is separately executed; no Bombadil code is linked
  into a Kiln package.
- Add a Bombadil terminal specification for the dashboard that derives every
  expectation from the staged journal fixture and the checked-in dashboard
  profile, so the properties do not rot when a fixture or a bound changes.
- Add three journal fixtures: a small admitted journal, an admitted journal
  larger than the profile's admitted run bound, and a journal whose event
  identity does not match its bytes.
- Add a Nix check that runs the specification over all three fixtures and
  keeps the captured traces as build outputs.
- Extend the local traceability policy so the TypeScript specifications count
  as evidence for the requirements they implement.

## Impact

- **Affected specs:** new `bombadil-dashboard-rail` capability.
- **Planned files:** the pinned flake input and check, the TypeScript
  specifications and journal fixtures, the local traceability policy and its
  exported form, the license map, and the dashboard documentation.
- **Compatibility:** additive only. No Kiln package changes behavior, no
  dependency is linked, and the dashboard binary is unchanged.
- **Testing:** the new Nix check, the existing workspace gates, and the
  Cairn gates.

## Non-Goals

- No new Kiln package, no new Kiln dependency, and no linked Bombadil code.
  The tool runs the packaged dashboard as a separate program.
- No claim that the exploration covered every journal, profile, or terminal
  input. The rail states properties over the fixtures it runs.
- No change to the dashboard's behavior to make it easier to test. The rail
  observes the surface as it is.
- No property coverage of the browser surface. Kiln has no browser surface.
- No claim that the rail proves CI success, provider correctness, journal
  completeness, or release readiness.
