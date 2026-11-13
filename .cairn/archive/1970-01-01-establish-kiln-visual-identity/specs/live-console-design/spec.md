## MODIFIED Requirements

### Requirement: Cohesive live console presentation

r[kiln.web.visual_design]
The live workspace MUST present a cohesive console in the Kiln identity: warm porcelain and soot surfaces with ember action emphasis, built from one shared token layer (spacing, type, radii, elevation, motion and semantic status colors), consistent typography and clear hierarchy across CI, work and code-review views. Each view MUST expose a single primary action, with secondary and session actions visually subordinate. Light, dark, system appearance, compact density and reduced motion MUST remain functional. The Overview project list, the run list, a project's run history and the code-review change list MUST stack their rows into lines when their own region is 48rem or narrower, and a status label in them MUST be shown whole. The visual change MUST preserve existing routes, identity, state, authority, confirmation and keyboard contracts, and MUST NOT introduce remote asset requests or fabricated observations.

#### Scenario: Desktop console and appearance

- GIVEN an authenticated workspace with retained CI observations
- WHEN the operator navigates CI, work and code-review views and switches appearance or density
- THEN labels, statuses and real observations MUST remain readable with AA contrast, controls operable and semantic success/error distinct from action emphasis

#### Scenario: Narrow and keyboard interaction

- GIVEN a 320px viewport or keyboard-only navigation
- WHEN the operator opens navigation, the account menu, creation, filters or the command palette
- THEN controls MUST remain reachable, touch controls at least 44px high on mobile, focus visible, and content contained without document-level horizontal overflow

#### Scenario: Lists in a narrow region

r[kiln.web.visual_design.scenario.lists]
- GIVEN the Overview project list, the run list, a project's run history or the code-review change list at a viewport of 320px or wider
- WHEN the list's own region is 48rem or narrower: on a phone, on a portrait tablet or beside a second column
- THEN its rows MUST stack into lines instead of columns, each status label MUST be shown whole, and the document MUST NOT overflow horizontally

## ADDED Requirements

### Requirement: Truthful live activity signal

r[kiln.web.live_activity]
On every page of an authenticated session, the live console MUST state how many retained runs the last admitted live snapshot reports as running, as text that links to the pending-run list. It MAY light the brand mark while that count is above zero. The signal MUST derive only from the last admitted live snapshot. It MUST show no count before a snapshot is admitted, none while live observation is stale or disconnected and none after the session ends. It MUST NOT count queued, awaiting-approval, unknown or finished runs as running. A reduced-motion preference MUST remove the animation without removing the text.

#### Scenario: Runs become active and finish

r[kiln.web.live_activity.scenario.count]
- GIVEN an authenticated console whose live snapshot reports two running runs, one queued run and one run awaiting approval
- WHEN any page is shown, and again after a snapshot in which no run is running
- THEN the top bar MUST first read 2 running and link to the pending runs, and MUST then show no running count

#### Scenario: Stale observation

r[kiln.web.live_activity.scenario.stale]
- GIVEN a displayed running count
- WHEN live observation fails or the session is cleared
- THEN the count and the lit mark MUST be removed, and MUST stay removed until live observation is current again

### Requirement: Feedback for requests in flight

r[kiln.web.request_feedback]
While a read or mutation request other than a background observation poll (the live-observation poll and log following) stays in flight beyond a brief delay, the live console MUST show a non-blocking progress indication in the top bar and MUST expose the busy state to assistive technology. It MUST remove the indication when no such request remains, whether the request succeeded, failed, timed out or was aborted. The indication MUST NOT imply completion, success or progress toward a known total.

#### Scenario: A slow read and its completion

r[kiln.web.request_feedback.scenario.slow]
- GIVEN an authenticated console and a read that takes longer than the delay
- WHEN the operator opens a page and the read later settles or fails
- THEN the progress indication MUST appear while the read is in flight and MUST be gone afterwards, and the live-observation poll alone MUST NOT show it
