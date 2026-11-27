## Context

Explicit workload placement spans an authenticated operator console and a native authority.

## Decisions

Keep placement with the owner. Use exact current attempt, checkpoint, destination admission, and fencing for a move, never duplicate an unresolved execution. Split into non-overlapping units with explicit dependencies and destination admission. Distinguish plan, requested effect, and observed placement.

## Risks / Trade-offs

Absent capabilities fail closed. Reusing provider admission avoids a second source of truth.
