## Why

Large or repeated patch sets are navigated mostly by pointer: the review page
has no keyboard handlers beyond dialog focus handling. Issue #9 asks for a
keyboard-first path to the navigation and actions the page already offers. The
hunk viewed markers from #5 exposed the hunk toggle for exactly this use, but
nothing consumes it yet.

## What Changes

- Add a keyboard layer to the interactive change-review surface:
  - navigation between changed files, hunks, unresolved threads and unviewed
    hunks, plus a unified/side-by-side layout toggle and a jump to the submit
    requirements card;
  - actions that reuse the existing controls: toggle viewed for the focused hunk
    or the displayed file, open and seed the anchored private draft editor, and
    open the existing publication confirmation;
  - a `?` help overlay that lists every command.
- Shortcuts are inert while focus is in a text field, while a dialog is open,
  during IME composition, and with Control, Meta or Alt held.
- No new commands, authority or ambient writes: every write is performed by the
  same control the pointer uses, with the same confirmation, request identity and
  disabled state.

## Impact

- **Files**: `crates/kiln-web-service/src/code-review.js` (keyboard layer, help
  overlay, `data-resolved` on discussion threads), `crates/kiln-web-service/src/code-review.css`
  (help list), `tests/live-console-ui.test.mjs` (pure resolution and command-table
  tests), `docs/operator.md` (shortcut reference), and the changed-page
  documentation paragraph that lists review interactions.
- **Testing**: `node --test tests/live-console-ui.test.mjs`; focused Rust checks
  for the crate that serves the assets (`cargo test -p kiln-web-service`);
  `cargo clippy -p kiln-web-service --all-targets -- -D warnings`; a
  disposable-instance browser check of every shortcut, including typing in a
  field and cancelling the publication confirmation; `nix flake check` for the
  asset and console rails.
