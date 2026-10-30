## Context

- `code-review.js` mounts one review surface into `state.container` and exposes
  pure helpers on `window.KilnCodeReview` (`load`, `refresh`, `clear`, `hunks`,
  `comparison`, `authorship`). It has no keyboard handling; the only related
  hook is the hunk toggle exposed as `.cr-hunk-mark[data-hunk-digest][data-file]`
  with `aria-pressed`, added for this issue by #5.
- Existing controls and their authority:
  - `.cr-file-row > a` routes to a file (`review_file`); `aria-current="true"`
    marks the displayed file.
  - `.cr-hunk-mark` toggles `mark_hunk` through `command()`; `.cr-mark` toggles
    `mark_reviewed` for the whole file. Both are private, reversible markers and
    are already disabled with a titled reason when blocked.
  - The diff layout is local state (`state.mode`, `unified`/`split`) switched by
    a `div.segmented` group whose buttons call `drawDiff()`.
  - `#cr-requirements` is the submit requirements card; its heading is focusable.
  - The anchored draft editor is a disclosure whose fields carry their
    form-memory keys as `data-cr-key` (`draft-file`, `draft-side`,
    `draft-start`, `draft-end`, `draft-body`); it opens automatically when
    `review_line` is routed.
  - `.cr-publish` holds the publication select and the "Publish review…" button,
    which opens the existing confirmation via `command(..., confirmation)`.
  - Discussion threads render as `article.cr-thread` with an `.cr-anchor` link to
    the exact line.
- The work workspace already has a reviewed key-handling pattern
  (`work-interaction.js`): one `document` keydown listener, an `editable()`
  guard, a dialog-open guard, modifier and IME guards, and movement that focuses
  existing rows rather than inventing a second navigation model.

## Goals and Success Contract

With the review surface mounted, a keyboard-only reviewer can move between files,
hunks, unresolved threads and unviewed hunks, switch layout, jump to the submit
requirements card, toggle viewed markers, seed the draft editor and open the
publication confirmation. Typing in any field never triggers a shortcut, and a
cancelled confirmation sends nothing. Pointer behaviour, accessible names, focus
order and screen-reader output are unchanged.

False completion includes:
- a shortcut that sends a command the pointer path does not send;
- a write that bypasses the existing confirmation or disabled state;
- a shortcut firing while typing, during IME composition, in a dialog, or with a
  modifier held;
- navigation that changes the selected revision, base, interdiff or filters.

## Decisions

### Decision: Pure key resolution, exported for tests

**Choice:** `window.KilnCodeReview.keys` exposes `commands` (the ordered
`{ id, keys, description }` table that the help overlay renders) and
`resolve(event, { editable, dialogOpen })`, which returns a command id or null.
The DOM layer owns targets and effects.

**Rationale:** suppression rules (typing, dialogs, IME, modifiers) and the
key-to-command table are the parts most likely to regress; they are testable
without a browser, and the help overlay cannot drift from the handler because
both read the same table.

### Decision: Keys

**Choice:** `j`/`↓` next file, `k`/`↑` previous file, `n`/`p` next/previous hunk,
`t`/`T` next/previous unresolved thread, `u`/`U` next/previous unviewed hunk,
`m` toggle layout, `v` toggle the focused hunk's viewed marker, `V` toggle the
displayed file's viewed marker, `d` seed and focus the anchored draft editor,
`P` open the publication confirmation, `s` jump to submit requirements, `?` open
the shortcut help.

Lowercase letters mirror the work workspace's `j`/`k` movement. Shifted letters
are the less frequent or more consequential variants, so a stray key cannot open
a publication confirmation. `v`/`V` act on the focused hunk and the displayed
file respectively because those are the two granularities the pointer offers.

**Rationale:** a small, documented set that maps onto existing controls; no
chord sequences to implement or explain.

### Decision: Actions reuse controls

**Choice:** the keyboard layer locates the existing control and invokes its
click handler (`v` → focused `.cr-hunk-mark`, `V` → displayed file's `.cr-mark`,
`P` → `.cr-publish` button, `m` → the pressed layout button). It never calls
`command()` itself. `d` only opens the disclosure and seeds the existing fields
from the routed line selection; it sends nothing. Disabled or blocked controls
stay inert because the click does nothing.

**Rationale:** confirmation copy, request identity, epoch checks, optimistic
refresh and disabled reasons remain single-sourced. A keyboard path cannot gain
authority the pointer lacks.

### Decision: Threads carry their resolved state in the DOM

**Choice:** discussion thread articles render `data-resolved="true|false"` and
the existing pill. Unresolved-thread navigation selects
`article.cr-thread[data-resolved="false"] .cr-anchor` in document order and
invokes its link, which routes to the anchored line.

**Rationale:** the rendered pill text is presentation; an attribute is a stable
selector that does not change pointer behaviour or accessible names.

### Decision: Help overlay reuses the shared dialog

**Choice:** `?` opens the existing `modal()` with the command table as a
definition list, a one-line note that shortcuts are inert in text fields and
never bypass confirmation, and the dialog's own close/cancel handling.

**Rationale:** one overlay system, so stacking, focus return and Escape behave
like every other review dialog.

## Risks / Trade-offs

- Single-letter shortcuts can collide with browser or assistive-technology keys.
  Mitigation: no modifiers are consumed, the handler ignores editable targets
  and open dialogs, and all bindings are listed in the help overlay.
- File navigation routes (as the pointer does), so focus returns to the document
  after the redraw. Consecutive presses still work because the next target is
  derived from `aria-current`, not from the previous focus.
- Toggling a viewed marker redraws the diff and drops focus to the document, as
  the pointer path already does; the control remains reachable by repeating the
  shortcut.
- The layer is inert when the review surface is not mounted, so it adds no
  global behaviour to other pages.
