# Let the HTML dashboard report update live

## Why

The dashboard's terminal report follows its input when the shell watches. The
HTML report did not: it was a snapshot of one admitted revision, so an
operator who opened the document had to re-render it by hand to see a newer
revision. The document could carry a standard reload instruction and the shell
could rewrite the file on the watch interval, so the gap was avoidable.

## What Changes

- The HTML renderer accepts a reload interval and emits a document-level reload
  instruction when one is given. It stays script-free.
- A watched HTML report rewrites its file on the profile's watch interval and
  asks the reader to reload.
- The report file is replaced atomically, so a reader never sees a half-written
  document.

## Impact

- **Affected specs:** a new `dashboard-live-html-report` capability.
- **Planned files:** the dashboard crate's HTML renderer and shell, the crate's
  tests, the operator documentation, and the lifecycle package.
- **Compatibility:** additive only. An unwatched HTML report is unchanged, the
  terminal report is unchanged, and no bound or view changes.
- **Testing:** the crate's tests, the workspace gates, and the Cairn gates.

## Out of Scope

- No script, no server, and no network. The reload instruction is a document
  instruction.
- No claim that every browser honours a reload instruction for a local file.
  The shell's guarantee is that the file is rewritten on the interval.
- No change to the watch interval's bound or to the observation profile.
