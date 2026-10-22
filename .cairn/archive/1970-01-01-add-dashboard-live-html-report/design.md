# Design: a live HTML dashboard report

## Context

The shell already re-reads its input on the profile's watch interval. The
terminal report redraws; the HTML report was written once.

## Goal and Success Contract

- A watched HTML report is rewritten on the watch interval.
- The document asks the reader to reload, without a script.
- A reader never observes a half-written document.
- An unwatched report stays a snapshot.

False completion includes these results:

- A document that refreshes with script.
- A file rewritten in place, so a reader can see half a report.
- A document whose reload instruction contradicts the watch interval.

## Decisions

### Decision: A document instruction, not script

**Choice:** The document carries `<meta http-equiv="refresh" content="N">`.

**Rationale:** The report's contract is that the document carries no script and
no external resource. A reload instruction is a document instruction, so the
contract holds, and the file still opens without a network.

### Decision: Replace the file atomically

**Choice:** The shell writes a sibling temporary file and renames it over the
report.

**Rationale:** The report is rewritten repeatedly while a reader may be reading
it. A rename replaces the file in one step, so the reader sees either the
previous report or the next one, never a partial document.

### Decision: Derive the interval from the profile

**Choice:** The reload interval is the profile's watch interval, rounded up to
whole seconds and never below one.

**Rationale:** The instruction must not contradict the interval the shell
actually uses. Deriving it from the same bound keeps the two in step, and the
profile remains the single place an operator sets the cadence.

## Risks / Trade-offs

- Whether a browser honours a reload instruction for a local file is the
  browser's behaviour. The shell's guarantee is that the file is rewritten on
  the interval; a reader that ignores the instruction still gets the new
  report on its next manual reload.
- The temporary file briefly sits beside the report. It is replaced on the
  next write.

## Verification Strategy

- The crate's test asserts that an unwatched document carries no reload
  instruction, that a watched document carries the interval derived from the
  profile, and that neither carries a script.
- The workspace gates and the Cairn gates run on the change.
