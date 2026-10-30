## ADDED Requirements

### Requirement: Structural comparison mode

r[kiln.review.structural]
Code review MUST offer an opt-in structural comparison mode that matches Tree-sitter trees from the pinned server-side grammars and displays only the lines holding changed tokens. Pure reformatting MUST show no change, and a moved unchanged top-level item MUST be reported as moved rather than as removed and added. The mode MUST be display only and MUST NOT affect ownership, CI, approvals, suggestions or semantic merge results. No grammar MUST run in the browser.

#### Scenario: Bounded with explicit fallback

r[kiln.review.structural.scenario.fallback]
- GIVEN a file without a pinned grammar, binary content, a parse error, or input beyond the byte, node, work or time bound
- WHEN structural mode is requested
- THEN the file MUST fall back to the plain line diff, labelled with the named reason, and no partial structural result MUST be shown

#### Scenario: Anchors equal line-mode anchors

r[kiln.review.structural.scenario.anchors]
- GIVEN a changed line shown in both line mode and structural mode
- WHEN a comment anchor is created on it in each mode
- THEN both anchors MUST be equal, and a structural region without exact line coordinates MUST disable commenting with a note
