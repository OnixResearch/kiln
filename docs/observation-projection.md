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

Zero bounds fail profile admission. A page limit of zero fails request
admission. A requested limit above the profile bound is capped, and the view
reports `capped`, `has_more` or `has_earlier`, and the effective limit.

## Request and window contract

Every request carries `state_revision` and one view request. It must equal the
admitted state revision. Every projection also receives an explicit window:

- `event_tail` requires an event window and rejects a log window;
- `log_range` requires a log window and rejects an event window;
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

## View identity

Every view carries a `kiln.observe.view.v1` BLAKE3 identity. The identity is
computed over a deterministic length-prefixed frame containing:

- the profile identity;
- the source state revision;
- the canonical request;
- the canonical admitted window;
- the redacted field set;
- the canonical view body.

Enum tags and field order are fixed. Equal admitted state, profile, request, and
window facts produce equal views and identities. Rebuilding the same view from
the same journal facts reproduces the same identity. A view identity never
grants lifecycle, release, transport, or control authority.

## Redaction

The profile's `visible_fields` set decides whether provider profiles, external
references, products, metrics, command identities, and log bytes are populated.
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
