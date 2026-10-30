# Observation projection

`kiln-observe-core` is a pure `no_std + alloc` crate that turns admitted Kiln
state and explicit shell-supplied windows into bounded, deterministic, read-only
observation views. It is not a transport, runtime, control surface, store, or
authority. HTTP and Iroh shells are later changes.

## View vocabulary

| View kind | Request facts | Output |
|---|---|---|
| `state_summary` | state revision | run, attempt, hold, run-outcome, and attempt-outcome counts |
| `run_list` | after cursor, page limit | paged run summaries with truncation and continuation facts |
| `run_detail` | run identity | run summary plus bounded attempt summaries |
| `attempt_detail` | attempt identity | attempt facts, bounded products and metrics, redaction facts |
| `event_tail` | record limit, admitted event window | newest journal records, revision range, earlier-history fact |
| `log_range` | stream identity, cursor, max bytes, admitted log window | retained-window facts, bounded bytes, gap or completion fact |
| `review_friction` | admitted review window | per-authorship-class approval latency, patch sets before approval, suggestions per change, merged-without-suggestion share |
| `suggestion_anchors` | cursor, page limit, admitted review window | ranked suggestion anchors by path and owning rule with paging facts |

An unknown view kind, a view kind absent from the profile vocabulary, or an
unknown state revision fails before projection. An unknown run, attempt, or log
stream returns a bounded `NotFound` view whose target names the exact identity.

## Profile

`ObservationProfile` declares:

- the supported view kinds;
- the exact bounds below;
- the redaction policy's visible field set;
- every required observation non-claim.

The typed Nickel form is `config/observation-profile.ncl`. The checked-in
`fixtures/observation/profile-v1.json` is generated from it and admitted by the
Rust profile test.

| Bound | Meaning |
|---|---|
| `max_page_items` | maximum run summaries or event records returned by one page |
| `max_attempts_per_run` | maximum attempt summaries in a run detail |
| `max_products_per_attempt` | maximum products in an attempt detail |
| `max_metrics_per_attempt` | maximum metrics in an attempt detail |
| `max_event_records` | maximum records admitted in one event window |
| `max_log_bytes` | maximum bytes returned by one log range |
| `max_log_window_bytes` | maximum retained bytes admitted in one log window |
| `max_log_segments` | maximum segment metadata entries admitted in one log window |
| `max_text_bytes` | maximum bytes for one admitted profile, reference, or attribute text |
| `max_encoded_bytes` | maximum canonical view-body bytes |
| `review.max_review_changes` | maximum changes admitted in one review window |
| `review.max_review_window_ms` | maximum review window span in milliseconds |
| `review.max_review_patch_sets` | maximum patch sets admitted for one change |
| `review.max_review_suggestions_per_change` | maximum anchored suggestions admitted for one change |

`bounds.review` is optional. A profile that declares `review_friction` or
`suggestion_anchors` without it fails admission with `missing_review_bounds`.
Profiles that omit it keep their earlier identity and document form.

Zero bounds fail profile admission. A page limit of zero fails request
admission. A requested limit above the profile bound is capped, and the view
reports `capped`, `has_more` or `has_earlier`, and the effective limit.

## Request and window contract

Every request carries `state_revision` and one view request. It must equal the
admitted state revision. Every projection also receives an explicit window:

- `event_tail` requires an event window and rejects a log window;
- `log_range` requires a log window and rejects an event window;
- `review_friction` and `suggestion_anchors` require a review window and
  reject event and log windows; every other view rejects a review window;
- summary, run, and attempt views admit no window facts.

An event window must use schema version one, contain contiguous
`expected_revision` values, and end exactly at the admitted state revision. It
may start after revision zero, in which case the view reports `has_earlier`.
A non-empty state with an empty event window, a gap, a record beyond the state,
or a window that does not end at the state fails before projection.

A log window must name the requested stream. Its retained bytes must exactly
cover `retained_start..retained_end`. A cursor ahead of `retained_end` fails; a
cursor before `retained_start` returns `RetentionView::Gap` with the exact
retained start and no claimed bytes. A complete terminal cursor returns an empty
terminal range.

A review window is the half-open millisecond range `[start_ms, end_ms)` and
the facts of every change whose first patch set lies in it, ordered by change
identifier. Each change carries its authorship class, patch-set creation times,
an optional approval, anchored suggestions with path and optional owner, and an
optional merge time. Admission fails before projection on:

| Diagnostic | Violation |
|---|---|
| `review_window_range` | empty or inverted range |
| `review_window_span` | span above `max_review_window_ms` |
| `review_window_change_bound` | more than `max_review_changes` changes |
| `review_window_order` | unsorted or duplicate change identifiers |
| `review_window_text_bound` | empty or over-`max_text_bytes` identifier, path, or owner |
| `review_window_patch_sets` | no, unordered, or more than `max_review_patch_sets` patch sets |
| `review_window_fact_outside` | first patch set before the start, or any fact at or after the end or before the first patch set |
| `review_window_approval` | approval not bound to the latest admitted patch set |
| `review_window_suggestion_bound` | more than `max_review_suggestions_per_change` suggestions |

A missing review window fails with `missing_review_window`.

## Review friction and authorship

`review_window(state, ownership, start_ms, end_ms)` derives the window from
admitted change-review state as a pure function. It includes changes whose
first patch set lies in the window, and it clips every later fact to before
`end_ms`:

- the authorship class is folded from declared patch sets only. No declaration
  gives `undeclared`, one shared class gives that class, and differing declared
  classes give `mixed`;
- the approval is the earliest `approve` vote bound to the latest patch set
  admitted in the window. Patch sets before approval is that patch set's
  number;
- an anchored suggestion is a review thread opened by someone other than the
  change owner. Its owner is the first ownership rule, in policy order, that
  applies to the anchor path and change target;
- a change is merged when its submission was recorded before `end_ms`.

`review_friction` reports one row per class in the fixed order `human`,
`agent`, `mixed`, `undeclared`. Each row has:

- `changes` and `approved_changes`;
- integer distributions (`count`, `total`, `min`, lower `median`, `max`) of
  approval latency in milliseconds, patch sets before approval, and anchored
  suggestions per change;
- `merged_changes`, `merged_without_suggestion` (merged on required CI alone),
  and `merged_without_suggestion_per_mille`.

`suggestion_anchors` groups suggestions by path and owner. It ranks them by
suggestion count descending, then distinct changes descending, then path and
owner ascending. It pages by an offset `cursor`: a zero limit fails, a limit
above `max_page_items` is capped and reported, and a cursor past the total
fails with `cursor_ahead`. The ranking is input for turning recurring review
comments into required CI checks. It creates no checks itself.

Authorship is self-declared by the contributor and unverified. Kiln never
infers it from commits, principals, or diffs, and every `review_friction` body
carries `authorship_basis: self_declared_unverified`. Neither view nor the
window gates, approves, merges, notifies, or alters review state.

## View identity

Every view carries a `kiln.observe.view.v1` BLAKE3 identity. The identity is
computed over a deterministic length-prefixed frame containing:

- the profile identity;
- the source state revision;
- the canonical request;
- the canonical admitted window, including the review window when present;
- the redacted field set;
- the canonical view body.

Enum tags and field order are fixed. Equal admitted state, profile, request, and
window facts produce equal views and identities. Rebuilding the same view from
the same journal facts reproduces the same identity. A view identity never
grants lifecycle, release, transport, or control authority.

## Redaction

The profile's `visible_fields` set decides whether provider profiles, external
references, products, metrics, command identities, log bytes, suggestion paths
(`suggestion_paths`), and suggestion owners (`suggestion_owners`) are populated.
Suggestion anchors are redacted after aggregation, so redaction never changes
their counts.
An omitted field is set to `None` or an empty collection and named in the
view's `redacted_fields` set before identity computation and encoding. Omission
is therefore visible and deterministic. Secret classification belongs to the
profile; the core does not discover secrets.

## Conformance

`ConformanceManifest` declares positive and negative case identifiers.
`admit_conformance` fails when a required positive or negative case is missing
or a case is duplicated. Maintained Rust tests cover every view, deterministic
rebuild, identity parity, read-only behavior, paging, cursors, redaction,
unknown targets and states, unexpected windows, encoded bounds, and non-claims.

## Non-claims

Passing observation checks do not prove transport delivery, live freshness,
control execution, provider correctness, CI success, or release readiness. They
prove only the checked projection over the supplied admitted state, profile,
request, window, bounds, and view identity.
