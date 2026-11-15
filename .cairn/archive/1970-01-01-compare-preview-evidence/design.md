## Context

`preview/service.rs` serves one admitted descriptor on a disposable loopback
origin behind a secret capability path; `preview-shell.js` fills
`window.kilnPreviewObservation` with the four phases and a bounded result. The
review detail already projects launchable previews from an owner-controlled
catalog and canonical build records; `code-review.js` renders them through
`preview-review.js`.

## Decisions

### Decision: the server composes the retained record

The browser posts only `{phases, result}` to
`/<capability>/<descriptor>/observation` on its own origin. The server admits
it only with a matching `Origin`, JSON content, a declared length within a named
limit, the four known phase names in order without repetition, known outcomes,
bounded elapsed times, details, errors and output within the descriptor's output
limit. Every identity field comes from the admitted descriptor, never from the
browser. The record identity is BLAKE3 over a domain tag and the canonical
serialized body. Records are written atomically under their identity in an
explicitly configured owner-controlled directory with a named maximum count; a
full directory refuses instead of evicting. Each record carries a finite expiry.
Engine-only qualification has no CI binding and cannot retain review records.

### Decision: read-only projection and explicit currency

The review service validates the directory and every record (owner, mode, size,
schema, identity) and ignores expired records. It projects only records whose
binding matches the change's project, repository and one of its revision
commits. A record is current only when it binds the current revision commit and
its canonical build record derives from the selected successful CI run.

### Decision: deterministic comparison with reasons

A pure comparison derives its identity from both record identities and the rule
version. It is compatible only for the same engine identity, platform, argument
list and executable path within the product; every mismatch is reported. The
candidate must be current; the baseline may be historical. Unknown or
unprojected identities are refused without substitution.

### Decision: route-bound UI

`review_preview_compare=<baseline>~<candidate>` restores the pair. Responses
for a previous selection or moved candidate are ignored through the existing
epoch checks. Only the current candidate keeps the existing click-time checked
launch link. Lanes stack on narrow screens and focus moves to the result.

## Verification Strategy

Rust unit tests cover intake admission, retention bounds, record verification,
projection currency and comparison reasons with positive and negative cases.
Node tests cover the rendering helpers. The canonical preview cohort is rerun
with an observation directory: two real launches are retained and compared
through the review route in a browser.

## Risks and Trade-offs

Browser-reported phases and output are untrusted observations: the record says
so in its provenance and never becomes CI evidence. Records outlive the preview
session by design; retention is finite and bounded by count.
