## Why

The structural review diff had only hand-written unit cases and no measurement
against real changes. The issue asks for per-language fallback and line
agreement before the opt-in mode widens.

## What Changes

- Add the out-of-tree `checks/structural-corpus` harness.
- Fetch OmniDiff at its pinned revision and project `text_mappings` and Kiln's
  novel rows to per-line touched labels.
- Emit a per-fixture CSV and a baseline regression gate.
- Commit the measured baseline and document its bounds.

## Impact

- **Files**: `checks/structural-corpus/**`.
- **Testing**: build, run against the pinned corpus, and re-run with the
  committed baseline.
- **Authority**: line agreement and fallback only. Moves, latency and
  correctness claims remain open.
