## Why

The operator asked for a review of source rendering and navigation against current UI/UX practice. A real-data audit of the live Source page (the Kiln Git source, directory `crates/kiln-web-service/src`, file `ui.js`) found these problems:

- The page is one 4,768px-tall column. The selected file renders below a 43-entry directory list, inside a small inner scroll box with visibly double-spaced lines.
- Directories are mixed among files instead of listed first. Directories show a size of 0, file sizes are raw byte counts, and entry kinds are "FILE"/"DIR" text rather than icons.
- Repository selection is a select box followed by four repeated fact disclosures.
- Each revision repeats "select revision …" text and shows the author's email. The current head is not marked.
- A revision's changes appear only after the reader picks a revision. They are one raw unified-diff block, with no per-file structure and no line numbers.
- There are no line anchors, copy-path, raw download, wrap toggle or syntax highlighting.

## What Changes

- **Layout.** A code-host-style header with a repository picker, a revision picker (head, or a pinned revision with its summary) and a breadcrumb path. Files, History and Changes are separate views.
- **Files view.** Two panes: the directory tree on the left (directories first, icons, humanized sizes, filter) and the file or directory listing on the right.
- **File viewer.**
  - A sticky header with the path, size and line count, plus copy-path, copy-link, download and wrap controls.
  - A non-selectable line-number gutter with linkable line anchors that the URL restores.
  - Correct line spacing, and bounded, presentation-only syntax highlighting.
- **History view.** Commits with summary, author name, relative time and short id. The pinned revision and the head are marked. Newer/older stepping and paging are kept.
- **Changes view.** A per-file changed-files list with +/- counts, and collapsible per-file diffs with old and new line numbers. Truncation is stated explicitly, and Pijul's historical limits are stated in place.
- **COB links.** A COB link that opens a revision lands on that revision's Changes view.

## Impact

Consumers are authenticated Kiln developers and CI operators who inspect admitted sources; Kiln web-service maintainers own the view. The durable contribution is a source viewer that matches developer expectations on the existing bounded read APIs.

Affected:

- the console web assets;
- a new source view asset pair served from the static route table;
- browser regression tests for the pure parsing and highlighting helpers;
- operator documentation.

## Out of Scope

- No API, bounds or source authority policy changes.
- No repository write or mutation paths.
- No remote assets or highlighting libraries.
- No per-entry last-commit data, which the API does not provide, and no README rendering beyond plain text.
- No changes to the code-review diff viewer.
