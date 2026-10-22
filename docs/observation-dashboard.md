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

## Inputs

The dashboard observes two kinds of input.

| Input | Command | Admission |
|---|---|---|
| canonical Kiln journal document | `--journal <path>` | every record's event identity is recomputed under the convention its identity kind declares, and the whole record set replays canonically |
| canonical SQLite event store | `--store <path> --stream <ns:kind:id>` | the store is opened read only, the retained stream is loaded, its event identities are verified, and the history replays canonically |

The journal document path reads the envelope that the writing shell owns and
ignores unknown envelope fields. The store path opens the database without
writing to it: it admits the stored schema version instead of initializing one,
never creates a missing store, and never changes the file. Both paths use the
store's and the document's own identity conventions, so the same dashboard
reads a pilot journal, a Radicle CI journal, and a SQLite event store.

The Celld worker event store is not a dashboard input. That store is a remote
authenticated HTTP service, and reading it would give the dashboard a network
path, which its read-only local boundary forbids.

## Shell

```console
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json --run run-1 --watch
$ kiln-dashboard --store path/to/events.db --stream kiln:event-stream:main --profile path/to/profile.json
$ kiln dashboard --journal path/to/journal.json --profile path/to/profile.json
```

The same surface is reachable from the operator CLI as `kiln dashboard`. The
shell reads only the files the operator names, writes the report to standard
output, and exits non-zero with a bounded diagnostic when a file is missing,
malformed, or fails admission. `--watch` re-reads the input on the profile's
declared interval and redraws; the interval is bounded and the shell keeps no
history between frames. The report is ANSI styled when standard output is a
terminal and plain text otherwise.

`cargo run -p kiln-dashboard --example sample_journal -- /tmp/journal.json 4`
writes a sample canonical journal for rehearsal, and
`cargo run -p kiln-dashboard --example sample_store -- /tmp/events.db` writes a
sample SQLite event store. `cargo run -p kiln-onixos-pilot --example
write_sample_journal -- /tmp/pilot/journal.json` writes a journal with the
pilot's own writer, which is how the checked-in pilot fixture was produced.

## HTML report

The dashboard can also write the same admitted report as a self-contained HTML
document.

```console
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json \
    --format html --output dashboard.html
$ kiln dashboard --journal path/to/journal.json --profile path/to/profile.json \
    --format html --output dashboard.html
```

The document carries its style inline, contains no script, and references no
external resource, so it opens without a network and states what it states
without an interpreter. It renders the same admitted views as the terminal
report: the observed state revision, the profile identities, the declared
journal identity, the recorded view identities, the redacted fields, and the
declared non-claims. The same admitted state renders the same bytes.

The HTML report updates live when the shell watches: the shell rewrites the
file on the profile's watch interval, replaces it atomically so a reader never
sees a half-written document, and the document carries a reload instruction at
that interval. The instruction is a document instruction, not a script, so the
document still carries no script and no external resource. Without `--watch`
the report is a snapshot of one admitted revision.

```console
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json \
    --format html --output dashboard.html --watch
```

## Property-based terminal rail

The dashboard's rendered surface is checked by a property-based rail. The rail
runs the packaged `kiln-dashboard` binary under a pseudo terminal with
[Bombadil](https://github.com/antithesishq/bombadil), a property-based testing
tool for web and terminal interfaces, pinned at revision
`832faf4d953e2c56d1d901aca1bb2c3c3170221d`. The tool is separately executed;
no Bombadil code is linked into a Kiln package.

The specifications live in `checks/bombadil/`. Each one derives its
expectations from the staged journal fixture and the checked-in dashboard
profile, so a property does not restate a constant:

- the finished report states the revision it observed, the profile it
  projected through, and every declared non-claim;
- the report renders at most the run bound the profile admits, and its
  rendered rows agree with its own summary unless it reports truncation;
- a journal whose event bytes do not match their recorded identity is refused
  with a named admission failure and no report.

```console
$ nix build path:$PWD#checks.x86_64-linux.bombadil -L
```

The check keeps the captured terminal traces as build outputs. The fixtures
are a small admitted journal, an admitted journal larger than the admitted run
bound, a journal written by the OnixOS pilot's own writer, a tampered journal,
and the `kiln` CLI's demo and evidence surfaces.

The rendered HTML report is checked by the same tool's browser driver, which
drives the document in a browser and states that the document shows the
observed facts, carries no script, and loads no external resource. That rail
found two defects the Rust tests did not: a missing section heading and a
style rule that rewrote the case of an identifier the document states.

The browser rail runs where a browser is reachable. It is not a Nix check
because the build sandbox gives the browser no loopback it can reach, and the
builder refuses a derivation that asks to run without the sandbox. The
report's contract is gated by the crate's tests instead: they bind the HTML
report to the terminal report and assert determinism, self-containment, and
escaping. Run the browser rail locally with:

```console
$ kiln-dashboard --journal checks/bombadil/journal-pilot.json \
    --profile fixtures/dashboard/profile-v1.json --format html --output dashboard.html
$ bombadil browser test --headless --time-limit 30s \
    "file://$PWD/dashboard.html" checks/bombadil/kiln-dashboard-html.spec.ts
```

Each property states its expectation only once the captured state carries the
program's exit status. A terminated terminal trace satisfies Bombadil's
`eventually` operator vacuously, which would make the properties
unfalsifiable, so the rail uses `always` with the exit-status condition
instead. Every property was exercised against a deliberately wrong
expectation and reported a violation.

The rail covers three fixtures. It does not claim to have explored every
journal, profile, or terminal input.

## Non-claims

Passing dashboard checks do not prove CI success, provider correctness, source
trust, journal completeness, release readiness, or that the observed revision is
the newest one in existence. They prove only the checked journal admission,
projection, bounds, and observed revision.
