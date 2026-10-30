## Why

Kiln's interactive change-review workspace already records exact patch sets, anchored review threads, path ownership, revision-bound approvals and required-CI submission. Its observation layer already serves bounded, read-only `kiln.observe.view.v1` views. A contribution has no authorship class, so Kiln cannot measure whether agent-authored changes cost more review than human ones (issue #3). It also cannot show which anchored suggestions recur and should become required CI checks.

## What Changes

- Add an optional, self-declared `authorship` to change creation, branch contribution intake and patch-set upload: `human`, `agent { harness, model? }` or `mixed`. The default is `undeclared`. Kiln never infers it. It travels in the durable review journal on the patch set, and older journals and state documents replay as `undeclared`.
- Validate the field in the pure review core and in the web shell: unknown kinds, empty or control-character text, and harness or model text over a named byte bound are rejected before any provider effect.
- Add two read-only observation views to `kiln-observe-core` over a shell-supplied, admitted review window:
  - `review_friction`: per authorship class, time from first patch set to revision-bound approval, patch sets before approval, anchored suggestions per change, and the share of merged changes with no suggestion (merged on required CI alone);
  - `suggestion_anchors`: a paged ranking of suggestion anchors by path and owning ownership rule.
- Serve both views through the existing HTTPS and iroh observation shells and render them in the offline dashboard, with the existing redaction, paging, window admission and view identity rules.
- Provide a pure derivation from admitted review state to the review window, and an authenticated read-only export of that window from the web service.
- The console create and contribution forms gain a labelled authorship select (default Undeclared), and change detail shows the declared authorship.

## Impact

- **Consumers:** Onix maintainers measuring review friction on their own projects; reviewers and contributors using the console.
- **Owners:** Kiln maintainers own the review core and observation core; Kiln web-service maintainers own intake, export and console; observation shell maintainers own the HTTPS, iroh and dashboard legs.
- **Affected:** `kiln-core` change review, `kiln-observe-core`, `kiln-adapter-http-observe`, `kiln-adapter-iroh-observe`, `kiln-dashboard`, `kiln-web-service` code review and console assets, observation Nickel profiles and generated fixtures, and the observation, HTTP, iroh, dashboard and operator documentation.

## Out of Scope

- No inference or verification of authorship from commit metadata, trailers, diff shape or principals. Authorship is self-declared and unverified.
- No authority: the views and the export never gate, approve, merge, notify or alter review state.
- No new CI checks generated from the anchor ranking; it is input for humans.
- No per-harness or per-model aggregation in the views; they aggregate by authorship class only.
- No change to existing view identities, request routes or profile documents that do not declare the new views.
