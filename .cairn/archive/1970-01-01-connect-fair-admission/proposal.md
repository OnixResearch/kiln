## Why

Issue #61 requires opt-in bounded ready admission composed with existing claims, owner grants and fairness. Immediate held debits prevent child bursts outrunning usage reports.

## What Changes

Add deterministic bounded admission and finite lineage accounting, versioned provider grant/request ports and a transactional extension of the implementation store. Keep defaults unchanged and provider placement and serving authority external.

## Impact

Consumer: opt-in implementation coordinator and inventory explanations. Owner: Kiln core/implementer maintainers. Repeatability: pure and durable recovery tests. No live provider qualification, physical allocation, model routing, preemption or production guarantees.
