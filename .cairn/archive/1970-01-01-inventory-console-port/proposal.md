## Why

Issue [#68](https://github.com/OnixResearch/kiln/issues/68) ports the live console to Rats/Meteorite while preserving its appearance and interactions exactly. Its first step is a checked inventory of the current console's routes, components and states, and its rollout requires a coverage matrix in which no route is silently omitted or treated as complete because its menu entry renders. The pinned Meteorite bundle (#67) and the capture parity gate (#66) are external prerequisites, but the inventory of the current console, which remains the reference, can be built and checked now.

## What Changes

- A machine-readable inventory, `docs/console-port-inventory.json` with schema `kiln.console-inventory.v1`, lists every registered console destination with its rollout cohort, page panel, renderer module and global, page stylesheets, consumed API paths, states the port must preserve and existing checks. It also lists the shared shell components (brand mark, navigation rail, top bar, page header, notice, confirmation dialog and sign-in form) with their DOM anchors, owner modules and stylesheets.
- A Node test cross-checks the inventory against the served console: the destination register, the page panels and rail links in `web.html`, the server's asset table, each module's global and its dispatch in `web.js`, the API path strings and the named test files. An omitted, unknown or drifted entry fails the test with the row and the missing reference.
- `docs/console-port-inventory.md` explains the cohorts, the coverage matrix, the recorded check gaps and the preservation rules the port must keep.

## Capabilities

### New Capabilities

- `console-port-inventory`: a checked route, component and state inventory of the current live console.

## Impact

New documentation and a Node test; `flake.nix` runs the test in the existing `live-console-ui` check. No console behavior, asset or API changes.

## Non-goals

Building, pinning or serving a Meteorite bundle (#67), porting any route, capture parity evidence (#66) and Rats component work. The current console stays the reference and the default entry point.
