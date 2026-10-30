## ADDED Requirements

### Requirement: Review keyboard navigation

r[kiln.review.keyboard.navigation]
While the interactive change-review surface is mounted and no dialog is open, the
page MUST offer keyboard navigation between changed files, hunks, unresolved
threads and unviewed hunks, a toggle between the unified and side-by-side diff
layout, and a jump to the submit requirements card. File and thread navigation
MUST follow the same route as the corresponding pointer link. Hunk and unviewed
navigation MUST move focus to the corresponding existing control without changing
the selected revision, base, interdiff, structural mode or filters.

#### Scenario: Navigation reaches each reviewed target

r[kiln.review.keyboard.navigation.scenario.targets]
- GIVEN a change with several changed files, at least two hunks, one unresolved
  thread and one unviewed hunk on the displayed revision
- WHEN the reviewer presses next/previous file, next/previous hunk,
  next/previous unresolved thread and next/previous unviewed hunk
- THEN each shortcut MUST select or focus the corresponding existing control,
  AND the layout toggle MUST change only the diff layout

### Requirement: Review keyboard actions reuse confirmed controls

r[kiln.review.keyboard.actions]
Keyboard actions MUST invoke the existing controls instead of sending commands
directly. Toggling viewed for the focused hunk MUST use that hunk's existing
toggle; toggling viewed for the displayed file MUST use the file's existing
toggle; the draft shortcut MUST only open and seed the existing anchored draft
editor; the publish shortcut MUST open the existing publication confirmation.
Disabled or blocked controls MUST remain inert. Every keyboard-reachable write
MUST carry the same confirmation, request identity and disabled state as its
pointer control, and MUST add no new command, authority or ambient write path.

#### Scenario: Cancelling the publication confirmation sends nothing

r[kiln.review.keyboard.actions.scenario.cancel]
- GIVEN a writable principal on the current revision with no pending command
- WHEN the publish shortcut opens the confirmation and the reviewer cancels it
- THEN no command MUST be sent, AND no request identity MUST be reserved

### Requirement: Shortcuts yield to text entry and overlays

r[kiln.review.keyboard]
Shortcuts MUST be inert while focus is in an input, textarea, select or editable
region, while any dialog is open, during IME composition, and while Control, Meta
or Alt is held. A `?` shortcut MUST open a help overlay that lists every command.
The overlay MUST reuse the shared review dialog behaviour.

#### Scenario: Typing never triggers a shortcut

r[kiln.review.keyboard.scenario.inert]
- GIVEN focus in a review text field
- WHEN the reviewer types characters that are shortcuts elsewhere
- THEN the field MUST keep its text, AND no navigation, layout change or command
  MUST occur
