## Context

The Source page is rendered by `loadSource` and its helpers in `web.js`, with markup in `web.html` and styles in `web.css`. It reads these endpoints:

| Endpoint | Returns |
| --- | --- |
| `/api/sources` | Inventory: id, label, kind, status, revision_scope, channel, head, reason |
| `/api/sources/{id}/log?limit&cursor` | Entries (id, summary, author, recorded_at_ms) and next_cursor |
| `/api/sources/{id}/tree?revision&path` | Entries (name, path, kind directory/file/…, bytes), plus label, kind, revision_scope, revision, channel |
| `/api/sources/{id}/file?revision&path` | path, oid, bytes, binary, text |
| `/api/sources/{id}/diff?revision` | Unified diff, files_changed, insertions, deletions, truncated, revision, parent |

Pijul sources report current-channel scope and refuse historical diffs. The URL parameters are `src`, `rev`, `spath`, `sfile` and `sq`. The COB view links to `page=source&src&rev`.

The accepted `kiln.dashboard.source.view` requirement covers:

- source selection;
- bounded revision paging;
- breadcrumb;
- line-numbered text;
- diff highlighting;
- entry filtering;
- up-level and newer/older navigation;
- URL restoration without substitution.

Archived changes established the console design system: GPUI Kit semantic tokens, Lucide icons via `KilnUi.icon`, the page frame, readable observations and 44px touch targets.

## Goals and Success Contract

Within seconds a reader can:

- pick a repository and see its tree at head;
- open a file beside the tree and read it with line numbers;
- link to a line;
- step through history and pin any revision;
- see exactly what that revision changed, file by file.

URLs restore every selection exactly.

False completion includes:

- highlighting or diff parsing that alters text;
- a default file or revision chosen silently;
- a line anchor that points elsewhere;
- hidden truncation;
- copy or download of bytes other than the retained text;
- desktop-only behaviour.

## Decisions

### Decision: Separate source module

**Choice:** Move the Source page into `source.js` and `source.css`, served at `/source.js` and `/source.css`. `window.KilnSource = { load, clear }` follows the `project.js` pattern. Remove the source code from `web.js` and the old markup from `web.html`.

Pure helpers live on `KilnSource` (or `KilnUi`) so Node tests can exercise them:

- unified diff parsing into files and hunks with old/new line numbers;
- language detection from the path;
- a bounded tokenizer producing inert spans;
- humanized byte sizes;
- entry sorting.

### Decision: Views and URL

**Choice:** Add URL parameters `sview` (`files`, `history` or `changes`; default `files`) and `sline` (a line, or a range `a-b`).

- **Files** is a two-pane split view at 60rem and wider, stacked below that width.
  - The left pane lists the current directory: directories first, then files, case-insensitive.
  - Each entry has a Lucide folder or file icon and a humanized size; directories show no size.
  - The right pane shows the selected file, or a directory summary when no file is selected.
- **History** is the commit list.
- **Changes** shows the selected revision's diff. When no revision is pinned, it explains how to pin one rather than choosing one.
- A revision opened from the COB view goes to Changes.

### Decision: File viewer

**Choice:**

- **Sticky toolbar:**
  - path (copyable), humanized size, line count, and short object id with the exact id in the title;
  - "Copy path" and "Copy link" (current URL with `sline`);
  - "Download" (a Blob of the retained text, using the file name);
  - a "Wrap lines" toggle.
- **Layout:** a table with a gutter column and a code column.
  - Gutter cells are line-number links with `aria-label` "Line N" and an href carrying `sline`.
  - The gutter is excluded from text selection, and line height is single.
- **Selection:**
  - Clicking a line number selects it; Shift-click extends a range.
  - The selection is highlighted and, when restored from the URL, scrolled into view.
- **Binary files** show an explicit state.

### Decision: Bounded, presentation-only highlighting

**Choice:**

- **Languages:** a small local tokenizer covers comments, strings, numbers and keywords for Rust, JavaScript/TypeScript, Python, Nix, Nickel, TOML, JSON, YAML, CSS, HTML, shell and Markdown headings.
- **Bounds:**
  - Highlighting applies only up to 200,000 bytes and 5,000 lines.
  - Beyond that the file renders plain, and the toolbar says highlighting was skipped.
- **Method:** the tokenizer runs line by line and carries state across lines for multi-line comments and strings.
- **Text preservation:** it only wraps substrings in spans, so concatenated rendered text equals the original exactly. Tests check this.

### Decision: Changes as a structured diff

**Choice:**

- **Parsing:** the unified diff is parsed per file into path, status (added, deleted, renamed, modified or binary) and +/- counts.
- **Layout:**
  - a summary of files changed, insertions and deletions;
  - a changed-files list linking to per-file anchors;
  - collapsible per-file sections with old and new line-number columns.
- **Markers:** added and deleted lines get both styling and a non-colour +/− marker.
- **Truncation:** stated at the top and on the affected file.
- **Unparseable lines** render as context instead of disappearing.

## Risks / Trade-offs

- **Tokenizer:** it can mis-highlight. This is cosmetic only, and the exact text is preserved.
- **Large files:** the API bounds file size, and highlighting is bounded further.
- **URL surface:** two more parameter keys.

## Verification Strategy

**Real-data review:**

- Sources: the Kiln Git source, the Lattice Jujutsu source and the Redox Pijul source.
- Tree navigation:
  - breadcrumb and filtering;
  - opening `ui.js`, a Rust file and `flake.nix`;
  - restoring a line anchor and a range.
- History paging and newer/older stepping.
- Changes for a recent revision with multiple files.
- The COB-to-source link.
- Pijul limits.

**Conditions:** 320 and 1440px, light and dark, keyboard only. Every page must show zero errors, zero overflow and 44px targets.

**Node tests:**

- diff parsing on added, deleted, renamed, binary, truncated and unparseable input;
- tokenizer text preservation across languages and size bounds;
- size humanization;
- sorting.

**Release:** repository gates, then deployment and live verification.
