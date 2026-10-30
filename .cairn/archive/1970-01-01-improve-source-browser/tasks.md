## Phase 1: Implementation

- [x] [serial] Move the Source page into its own module and add the following, keeping every accepted source-view contract. r[kiln.dashboard.source.navigation] r[kiln.dashboard.source.navigation.scenario.anchor] r[kiln.dashboard.source.navigation.scenario.changes]
  - Files, History and Changes views
  - A two-pane tree and file viewer
  - Line anchors
  - Bounded syntax highlighting
  - A structured diff

## Phase 2: Verification

- [x] [serial] Verify the new Source page and record evidence. r[kiln.dashboard.source.navigation]
  - Add tests for the helper functions.
  - Review on real data at two widths, in both appearances and with the keyboard.
  - Run the repository gates.
  - Deploy and record bounded evidence.

Evidence: `evidence/verification.json`, `evidence/findings.json` and viewport screenshots. Accepted on operator request 2026-10-30.
