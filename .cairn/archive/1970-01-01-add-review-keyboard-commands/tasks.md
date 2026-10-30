## Phase 1: Resolution and table

- [x] [serial] Add `window.KilnCodeReview.keys` with the ordered command table and `resolve(event, { editable, dialogOpen })`, covering modifiers, IME composition, dialogs and editable targets. r[kiln.review.keyboard] r[kiln.review.keyboard.scenario.inert]

## Phase 2: Surface wiring

- [x] [serial] Wire navigation (files, hunks, unresolved threads via `data-resolved`, unviewed hunks), the layout toggle and the submit-requirements jump to existing controls. r[kiln.review.keyboard.navigation] r[kiln.review.keyboard.navigation.scenario.targets]
- [x] [serial] Wire actions to existing controls only: hunk and file viewed toggles, draft editor seeding, publication confirmation, and the `?` help overlay. r[kiln.review.keyboard.actions] r[kiln.review.keyboard.actions.scenario.cancel] r[kiln.review.keyboard]

## Phase 3: Verification

- [x] [serial] Add node tests for resolution, suppression and the command table; run the console and Rust rails; run a disposable-instance browser check of every shortcut including typing in a field and cancelling the publication confirmation; document the shortcuts in `docs/operator.md`; record evidence. r[kiln.review.keyboard] r[kiln.review.keyboard.navigation] r[kiln.review.keyboard.actions] r[kiln.review.keyboard.scenario.inert] r[kiln.review.keyboard.actions.scenario.cancel]
