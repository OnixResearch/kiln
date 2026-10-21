# Operator dashboard

`kiln-dashboard` renders the accepted `kiln-observe-core` projection of one
canonical Kiln journal as a local terminal report. It is a read-only shell:
it adds no view vocabulary, no projection logic, no control intake, no
canonical state, and no transport.

## Why a local surface

The remote observation shells are correct for their purpose, and neither
answers a local question. The HTTPS shell needs a certificate chain, a private
key, and a per-request authorization token. The Iroh shell needs an enrolled
peer, an endpoint key, and reachability. An operator debugging a run on the
machine that holds the journal reads raw journal JSON instead.

The dashboard reads the journal that already exists and renders the same views
the remote surfaces serve, with the same identities, bounds, paging, and
redaction.

## Profile

`config/dashboard-profile.ncl` declares the observation profile the dashboard
projects through, the requested run page limit, the requested run page count,
the requested event tail limit, the watch interval, and the dashboard
non-claims. It embeds the accepted observation profile, so the dashboard
projects through the same bounds the remote surfaces use.

Admission rejects a profile whose schema version is unsupported, whose
observation profile is not admitted, whose page limit, page count, event limit,
or watch interval is zero, or whose non-claims omit a required dashboard
non-claim. The exported profile is `fixtures/dashboard/profile-v1.json`.

## Journal admission

The dashboard reads a journal document: a JSON envelope with a `records` array
of canonical journal records. The envelope is shell owned, so unknown envelope
fields are ignored; every journal record must carry exactly the canonical
record fields.

Admission applies three checks before any view is projected:

- every record's event identity is reproduced by recomputation over the
  record's own canonical event encoding, under the encoding convention that the
  record's identity kind declares. `pilot-event-blake3` and `ci-event-blake3`
  name the packed Preserves encoding; `event-blake3` names the canonical JSON
  encoding. An identity kind with no admitted convention is rejected;
- the whole record set replays canonically from the initial state through
  `kiln_core::operations::replay`, which enforces contiguous revisions from
  revision zero and canonical event evolution;
- the record count and document size respect their named bounds.

The journal document's own content identity is a writer-owned seal over a
shell-specific tuple, so the dashboard reports it as a declared fact and does
not claim to have reproduced it. A journal that is a canonical prefix of a
longer journal therefore replays canonically and is admitted: the report
states the revision it observed and does not claim the journal is complete.

## Views

Every reported fact comes from `kiln_observe_core::project_observation`, so the
dashboard's view for a request and window has the same content and the same
`kiln.observe.view.v1` identity the remote surfaces serve.

| Panel | Request |
|---|---|
| summary | `state_summary` |
| runs | `run_list`, paged with the accepted cursor up to the profile's page count |
| run detail | `run_detail` for the selected run, or the first observed run |
| attempt detail | `attempt_detail` for the selected attempt, or the selected run's active attempt |
| events | `event_tail` over the tail of the admitted record stream |

The event window is the tail of the record stream, so it ends at the admitted
state revision and stays inside the observation profile's record bound. The
report states the page facts it observed, including `capped`, `more`, and
`earlier`, so an operator can tell that the report is a bounded view rather
than the whole history.

## Shell

```console
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json --run run-1 --watch
```

The shell reads only the two files the operator names, writes the report to
standard output, and exits non-zero with a bounded diagnostic when a file is
missing, malformed, or fails admission. `--watch` re-reads the journal on the
profile's declared interval and redraws; the interval is bounded and the shell
keeps no history between frames. The report is ANSI styled when standard
output is a terminal and plain text otherwise.

`cargo run -p kiln-dashboard --example sample_journal -- /tmp/journal.json 4`
writes a sample canonical journal for rehearsal.

## Non-claims

Passing dashboard checks do not prove CI success, provider correctness, source
trust, journal completeness, release readiness, or that the observed revision is
the newest one in existence. They prove only the checked journal admission,
projection, bounds, and observed revision.
