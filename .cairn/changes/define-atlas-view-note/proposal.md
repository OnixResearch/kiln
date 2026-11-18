## Why

Kiln issue 92 asks for a short note, before any implementation, that defines a
default-off isometric atlas view and logs system-atlas as related work. The
note fixes the safety rule first: every payload drawn on a route must say
whether it is declared by the atlas author or observed from retained Kiln
evidence, so representative shapes cannot pass as measured traffic.

The current consumers are the Kiln web-service and dashboard maintainers who
will decide whether a later change adds the view during the console port
(issue 68). They own the note. Repeatability comes from fixing the data
contract, provenance labels, navigation rules and bounds once, so a later
implementation is reviewed against a written contract rather than a recording.

## What Changes

- Add `docs/atlas-view.md`: the related-work record for system-atlas, one
  caller-supplied atlas document (nodes, routes, optional chapters, optional
  open questions, optional packets), declared and observed payload rules with
  a visible qualifier, chapters and a pin list as navigation over that
  document, a generated prose twin, and default-off bounds with the rats issue
  24 flat fallback.
- Index the note in `docs/README.md`.

## Impact

- **Files**: `docs/atlas-view.md`, `docs/README.md` and this change package.
- **Testing**: relative-link resolution, absence of added media or upstream
  assets, and the Cairn gates and validation.
- No code, console route, profile option, asset, accepted requirement or
  configuration changes. A later implementation needs its own spec-driven
  change and does not inherit authority from this note.
