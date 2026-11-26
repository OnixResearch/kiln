## Why

Move eligible workloads and split independent units across selected hosts. Today the console lacks this control.

## What Changes

Keep placement with the owner. Use exact current attempt, checkpoint, destination admission, and fencing for a move, never duplicate an unresolved execution. Split into non-overlapping units with explicit dependencies and destination admission. Distinguish plan, requested effect, and observed placement.

## Impact

Kiln owns the operator surface and receipts; the named provider retains authority over effects.
