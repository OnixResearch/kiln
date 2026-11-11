## Decisions

A merge commit can be followed by later target commits, so exact base equality is
only valid immediately after merge. The compare endpoint provides a bounded,
read-only ancestry relation; requiring merge base equality prevents accepting an
unrelated branch that merely has more commits.

## Non-claims

The compare read is point-in-time and is not an atomic merge precondition or a
merge-queue guarantee.
