## Phase 1: Admission

- [x] [serial] Validate proposal, design and task gates before implementation. r[kiln.dashboard.cobs.read] r[kiln.dashboard.cobs.authority] r[kiln.dashboard.cobs.view]

## Phase 2: Implementation

- [x] [serial] Implement typed separate runtime policy and bounded native ref/history reads with paired success/failure regression coverage. r[kiln.dashboard.cobs.read] r[kiln.dashboard.cobs.authority]
- [x] [serial] Add the integrated Onix COBs browser, exact selection, filters, paging, content/identity inspection and session-safe URL restoration. r[kiln.dashboard.cobs.view]

## Phase 3: Verification

- [x] [serial] Exercise real local Git objects through authenticated HTTP and desktop/mobile browser interactions, run required checks and update operator documentation and retained evidence. r[kiln.dashboard.cobs.read] r[kiln.dashboard.cobs.authority] r[kiln.dashboard.cobs.view]

Observed outcomes and remaining repository holds are retained in
`evidence/verification.json`. The Nix package/deployment check is blocked by
`reports::reports_preserve_raw_bytes_bind_exact_attempt_and_compare_real_test_coverage_history`
reporting `service state already has an owner`; the VM did not execute.
No production activation, archive or publication was performed.
