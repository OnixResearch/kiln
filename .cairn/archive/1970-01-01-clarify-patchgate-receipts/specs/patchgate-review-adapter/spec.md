## MODIFIED Requirements

### Requirement: Retained review receipts in the live workspace
r[kiln.dashboard.reviews.view]
The live workspace MUST provide a Reviews page over an operator-declared receipt directory and a Reviews tab over a run's retained reports, each selecting only `kiln.patchgate.review.v1` receipts and rendering them with one receipt presentation. That presentation MUST render the normalized per-change decisions with their distinct states, each in its own words with its meaning; the producer's isolation and acceptance-authority claims; the preserved request, candidate, policy, binding, report and effect identities; and the verbatim native report. Facts read from the native report (change messages, paths, deciders, reasons and decision times) MUST be labeled as recorded by the producer, MUST NOT replace a normalized decision, and MUST read as unavailable when the native report cannot be read. The Reviews page MUST list receipts newest first by their latest recorded decision, name each by its first change's recorded message where one exists, and filter them by text and decision without substituting the selected receipt. Neither surface MAY grant review, merge, gate or release authority.

#### Scenario: A retained receipt is inspected
r[kiln.dashboard.reviews.view.scenario.retained]
- GIVEN a run with one retained review report whose document schema is `kiln.patchgate.review.v1`
- WHEN an admitted reader opens the Reviews view
- THEN every recorded decision MUST remain distinct and inspectable, acceptance authority MUST read as none unless the producer claimed otherwise, and the native report MUST be shown verbatim

#### Scenario: No receipt is retained
r[kiln.dashboard.reviews.view.scenario.absent]
- GIVEN no configured receipt directory, a run with no retained Patchgate receipt, or a directory entry that is not a valid receipt
- WHEN an admitted reader opens the Reviews page or tab
- THEN the view MUST report the absence or skip reason explicitly rather than implying a clean review, and unrelated reports MUST remain under the existing structured-report view

#### Scenario: Every decision reads in its own words
r[kiln.dashboard.reviews.view.scenario.decisions]
- GIVEN receipts whose normalized decisions cover accepted, rejected, escalated, blocked, waiting, pending, removed, reverted and unknown, plus one unrecognized value and one missing value
- WHEN they are shown on the Reviews page, in a run's Reviews tab and in the decision filter
- THEN each of the nine MUST read with its own label and meaning, the unrecognized value MUST read as unrecognized with its recorded text, the missing value MUST read as not recorded, and none of these MAY read as accepted

#### Scenario: Recorded context beside each decision
r[kiln.dashboard.reviews.view.scenario.context]
- GIVEN one receipt whose native report records change messages, paths, deciders, reasons and decision times, and one receipt whose native report is not readable JSON
- WHEN each is opened
- THEN the first MUST show those facts beside each normalized decision as recorded by the producer, and the second MUST still show every normalized decision, mark the recorded context unavailable and show its native report verbatim

#### Scenario: Finding a receipt
r[kiln.dashboard.reviews.view.scenario.find]
- GIVEN a receipt directory whose receipts have different decision times and decisions
- WHEN the reader opens the Reviews page, types part of a change hash or message, and chooses a decision filter
- THEN receipts MUST be listed newest first, only matching receipts MUST be listed together with their count, the filters MUST be restored from the URL, and a selected receipt that the filters hide MUST stay selected rather than be replaced
