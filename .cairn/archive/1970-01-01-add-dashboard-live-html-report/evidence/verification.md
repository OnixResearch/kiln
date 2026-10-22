# Live HTML report verification

Date: 2026-10-22
Scope: the dashboard's watched HTML report, its reload instruction, and the
atomic replacement of the report file.
Base revision: `5adc05a5` plus the uncommitted change working tree.

## What was added

- The HTML renderer accepts a reload interval and emits a document-level reload
  instruction when one is given.
- A watched HTML report is rewritten on the profile's watch interval, and the
  report file is replaced atomically.
- A crate test that asserts an unwatched document stays a snapshot, that a
  watched document carries the interval derived from the profile, and that
  neither carries a script.

## Results

- `cargo test -p kiln-dashboard`: 27 passed, 0 failed, including
  `a_watched_html_report_asks_the_reader_to_reload`.
- `cargo clippy -p kiln-dashboard --all-targets -- -D warnings`: pass.
- `cargo fmt -p kiln-dashboard -- --check`: pass.
- The watched document was rendered locally and carries
  `<meta http-equiv="refresh" content="1">` for the profile's one-second watch
  interval; the unwatched document carries none.
- `cairn gate proposal|design|tasks add-dashboard-live-html-report --root .`:
  PASS.
- `cairn sync add-dashboard-live-html-report --root . --execute`: applied the
  requirements to `.cairn/specs/dashboard-live-html-report/spec.md`.
- `cairn traceability coverage --root .`: pass, 871 of 871 requirements
  referenced.
- `cairn validate --root .` after the archive: `valid: true`.

## Limits and non-claims

- The shell's guarantee is that the file is rewritten on the interval. Whether
  a browser honours a reload instruction for a local file is the browser's
  behaviour, and a reader that ignores it still sees the newer report on its
  next manual reload.
- The reload instruction is a document instruction, so the document still
  carries no script and no external resource, and it still opens without a
  network.
- The live report states exactly the revision it was rendered from. It does not
  claim that a reader observed a newer revision, and it states the same
  non-claims as the terminal report.
