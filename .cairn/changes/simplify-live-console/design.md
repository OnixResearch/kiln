## Context

The deployed overview uses seven large counter cards and framed project cards. Branding, workspace promotion and repeated headings occupy navigation space. The operator's next action is usually to identify a failed project and read its execution output.

## Decisions

### Decision: Scan-first overview

Use a single compact retained-count strip, preserving every count and existing outcome route. Render latest-per-project rows in the existing failure/unknown/active priority order, with explicit outcome, recorded duration/completion and separate Pipeline/Logs links. Do not invent liveness, success, timing or new run selection. Put the existing duration trend in a closed native disclosure.

The existing run-list API has no active-only outcome filter: Pending also includes queued requests. Active therefore opens the existing busy execution-target view, labelled by its link title, rather than sending an unsupported outcome or showing queued requests as active. Other count links use admitted run outcomes; backend contracts remain unchanged.

### Decision: Progressive disclosure and quieter chrome

Remove the decorative workspace card, brand caption and redundant page eyebrow. Keep Issues, inbox, Planning, Overview, Runs, CI projects and Workers directly visible; retain other work destinations under More work and source/review/activity/evidence under Inspect. Use native details and auto-open the selected navigation group. Keep project/outcome/search visible in the run browser; put exact-source/order under More filters and auto-open when restored non-default values are active. Preserve all element IDs, URL fields, form handlers, confirmations and read/write admission.

### Decision: Bounded activation

Use repository-owned Nix packaging and the established private-backup/preflight/user-profile activation procedure. The request authorizes this existing service deployment, not remote source publication, lifecycle acceptance or release eligibility. Keep rollback and exact executable/asset identities in evidence.

## Risks / Trade-offs

Secondary destinations require one more click but remain labelled, keyboard reachable and deep-link visible. Compact rows must wrap at 320px and preserve 44px touch targets. No CSS-only hiding of errors, safety boundaries or authorization. Remove obsolete live-only styles rather than appending a second visual layer; leave shared offline catalog styles untouched.

## Verification Strategy

Observe the current surface before editing. Exercise the real candidate over retained data: all counts, ordering, failed-run filters, exact direct-log selection, source/order filter restoration, deep-linked collapsed navigation, timing disclosure, issue capture without submission, appearance, keyboard and mobile containment. Run required repository gates once after integration; verify actual packaged/deployed bytes and unchanged live data after activation.
