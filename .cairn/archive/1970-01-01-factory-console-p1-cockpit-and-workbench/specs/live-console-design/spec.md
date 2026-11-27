## ADDED Requirements

### Requirement: Two-region Factory Cockpit with non-destructive Slide-Over Inspector

r[kiln.web.factory_cockpit_and_slideover]
The `1.1 Factory` landing view MUST render inside `.page-frame-wide` (`110rem`) using a persistent Global Live Strip, a Live Factory Telemetry Ribbon (dispatch state, swarm mirror health, open escalations, expiring claims, host occupancy), a main column (auto-collapsing Attention & Escalation Inbox + compact clickable `DeliverySpine` rows + 1-line-per-target Merge Train strip), and a `24rem` side rail (`Machine & Slot Census` + `Claims & Fair-Admission Budget` collapsing healthy non-expiring claims behind a count). Selecting any Attention item, Claim, Attempt, Run, or Review MUST open a `44rem` Slide-Over Inspector synchronized to `?inspect=<kind>:<id>` without tearing down page state or closing the SSE stream, and MUST stream live `herdr` pane output strictly while the Attempt drawer is open and `document.visibilityState === "visible"`.

#### Scenario: Zero blockers compacts Attention and Slide-Over preserves SSE

r[kiln.web.factory_cockpit_and_slideover.scenario.cockpit]
- GIVEN an operator viewing `1.1 Factory` with zero active escalations and one running agent attempt
- WHEN the page renders and the operator opens the running attempt in the Slide-Over Inspector
- THEN the Attention section MUST compact to a single healthy summary bar
- AND opening the Slide-Over MUST NOT close `/api/events`, MUST sync `?inspect=attempt:<id>`, and MUST pause pane tailing immediately on drawer close or tab hide

#### Scenario: Tiered control safety and CLI parity

r[kiln.web.factory_cockpit_and_slideover.scenario.tiered_safety]
- GIVEN an operator invoking a Tier 3 destructive action (`Cancel Attempt`, `Force-Release / Steal Foreign Claim`, `Drain Machine`, or bulk mutation)
- WHEN the action is triggered from the Cockpit, Slide-Over, or Command Palette
- THEN the console MUST require a reason note, explicit confirmation, and idempotency `request_id`
- AND MUST display a 1-click copyable `kiln-fleet` or `kiln-implementer` CLI equivalent command

### Requirement: Full-factory Command Palette, keyboard chords, and bulk controls

r[kiln.web.factory_command_palette_and_bulk]
The `Ctrl/Cmd+K` Command Palette (`work-interaction.js`) MUST index Destinations, Fleet Claims, Attempts, Open Decisions, Cairn Changes, Campaigns, Projects, server-searched Runs, Code Reviews, and Machines, and MUST support typed operational verbs (`> renew`, `> release`, `> approve`, `> reconcile`, `> wake`, `> drain`, `> cancel`) routed through the tiered safety rails. The console MUST provide consistent `g + <key>` navigation chords (`g f`, `g a`, `g d`, `g r`, `g c`, `g m`), `j`/`k`/`Enter`/`[`/`]`/`Esc` list and Slide-Over navigation, Attention triage shortcuts (`a`, `r`, `p`, `Shift+X`), and multi-select (`x`) bulk toolbars on Claims, Attempts, and Runs.

#### Scenario: Operator executes a Fleet verb via Command Palette

r[kiln.web.factory_command_palette_and_bulk.scenario.palette_verb]
- GIVEN an open expiring claim in Fleet
- WHEN the operator opens `Ctrl/Cmd+K` and selects `> renew <claim>` or `> release <claim>`
- THEN the palette MUST find the live Fleet claim and execute Tier 1 renewal or open the Tier 3 release modal without switching pages
