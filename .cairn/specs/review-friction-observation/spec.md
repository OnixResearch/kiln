# Review Friction Observation Specification

## Purpose

Defines the `review-friction-observation` capability.

## Requirements

### Requirement: Review window admission

r[kiln.observe.review.window]
Review observation views MUST project only over an explicit shell-supplied review window that passes admission against the profile's review bounds. Admission MUST reject an empty, inverted or over-span range; too many changes; unsorted or duplicate change ids; unordered or over-bound patch sets; a first patch set outside the window; any fact at or after the window end or before the change's first patch set; an approval not bound to the latest admitted patch set; too many suggestions; and text over the profile text bound. Review views MUST reject event and log windows, and other views MUST reject a review window.

#### Scenario: Window violation fails closed

r[kiln.observe.review.window.scenario.violation]
- GIVEN a review window that violates any admission rule
- WHEN a review view is projected
- THEN projection MUST fail with a named diagnostic and return no view

### Requirement: Review friction view

r[kiln.observe.review.friction]
The `review_friction` view MUST report, for each authorship class in the fixed order human, agent, mixed, undeclared:
- the changes in the window;
- the approved changes;
- deterministic integer distributions of first-patch-set-to-revision-bound-approval latency, patch sets before approval, and anchored suggestions per change;
- the merged changes, the merged changes with no anchored suggestion, and that share in per mille.

It MUST carry a `kiln.observe.view.v1` identity and grant no authority.

#### Scenario: Fixture journal yields known values

r[kiln.observe.review.friction.scenario.fixture]
- GIVEN the checked-in review journal fixture with known human and agent changes
- WHEN it is replayed, derived into a review window and projected
- THEN the derived window MUST equal the checked-in window fixture
- AND the friction view MUST report the expected latency, patch-set, suggestion and merged-share values

### Requirement: Suggestion anchor view

r[kiln.observe.review.anchors]
The `suggestion_anchors` view MUST rank anchored suggestions grouped by path and owning rule by suggestion count descending, then distinct changes descending, then path and owner ascending. It MUST page by an offset cursor under the profile page bound. The profile redaction policy MUST be able to redact anchor paths and owners after aggregation and name them in `redacted_fields`.

#### Scenario: Paging bounds

r[kiln.observe.review.anchors.scenario.paging]
- GIVEN a review window with more anchors than the page bound
- WHEN anchors are requested with a zero limit, an over-bound limit, or a cursor past the total
- THEN a zero limit MUST fail, an over-bound limit MUST be capped and reported, and a cursor past the total MUST fail with `cursor_ahead`

### Requirement: Review view surface parity

r[kiln.observe.review.parity]
The HTTPS observation shell, the iroh observation shell and the offline dashboard MUST serve or render review views produced by the same projection. For the same profile, state, request and review window, they MUST yield byte-identical views and identities. A missing review window MUST fail as an unknown target.

#### Scenario: Equal views across surfaces

r[kiln.observe.review.parity.scenario.equal]
- GIVEN one snapshot carrying the fixture review window
- WHEN `review_friction` and `suggestion_anchors` are requested over HTTPS and iroh, and rendered by the dashboard
- THEN every surface MUST carry the same view and the same view identity

### Requirement: Self-declared, non-authoritative review observation

r[kiln.observe.review.nonclaims]
Documentation and views MUST state that authorship is self-declared and unverified. Review views and the review-window export MUST NOT gate, approve, merge, notify or alter review state.
