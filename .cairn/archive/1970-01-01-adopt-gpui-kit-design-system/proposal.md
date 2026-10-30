## Why

The operator asked to use GPUI Kit across Kiln, and a WebAssembly spike found that a console replacement is not viable. Its web backend exposes no accessibility tree, the first load is about 4x heavier, and rows appear 5–6x later; upstream also labels the target showcase-only. The operator then chose to adopt GPUI Kit's design system in the existing HTML console.

The console currently uses hand-drawn inline SVG icons and ad-hoc token names. Its interaction states and dialog copy are not checked against a written standard.

## What Changes

- Import Lucide icons from a pinned GPUI Kit revision through a repo-owned import script with hash checks and recorded provenance. Replace hand-drawn icons with them, served locally.
- Rename console design tokens to GPUI Kit's semantic theme vocabulary, keeping Kiln's palette values. Examples: background/foreground, muted, accent, primary, secondary, border, input, ring, danger, success, warning, info, popover, list and table states. Migrate every live-console stylesheet and remove the old names.
- Apply GPUI Kit's Design Guides to every console page. Document each place where web conventions deliberately differ from the desktop guidance.

## Impact

Consumers are authenticated Kiln developers and CI operators using the live console. Kiln web-service maintainers own the design system. The durable contribution is one external, maintained design standard and icon set, instead of ad-hoc local choices.

Affected areas:

- `crates/kiln-web-service/src` web assets;
- a new import script;
- `LICENSES.md`;
- tests and documentation that assert changed strings.

## Out of Scope

- No GPUI, Rust UI dependency or WebAssembly.
- No API, route, authority or execution changes.
- No remote assets.
- The offline report (`catalog.css`) and the hosted bridge UI.
- Changing the palette values or the accepted readable-observation and layout behaviour.
