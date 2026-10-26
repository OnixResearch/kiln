# Operator dashboard

`kiln-dashboard` renders the accepted `kiln-observe-core` projection of a canonical
Kiln journal or store stream as a local terminal report or offline interactive
HTML page. A repository catalog groups independently admitted histories without
merging canonical state. The dashboard remains read only: no new view vocabulary,
projection semantics, control intake, canonical state or provider effect.

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

The document carries its style and trusted interaction JavaScript inline and
references no external resource. Open it directly from the filesystem; no
server, network connection, framework, or external font is needed. Run and
attempt identifiers and journal-derived text remain escaped, inert data, not
executable script or markup.

The page shows the same admitted facts as the terminal report: the observed
state revision, the profile identities, the declared journal identity, the
recorded canonical view identities, the redacted fields, and the declared
non-claims. HTML adds precomputed run and attempt detail views for local
drilldown; the projection, redaction, and identity rules do not change. The same
admitted input and selection render the same bytes. Terminal output and CLI
command usage are unchanged.

The responsive console places admitted state counts above a run list and a
selected-run/attempt inspector. On narrow screens these surfaces stack; wide
tables scroll within their own regions. Recent activity and expandable evidence
sections retain exact provenance, canonical view identities, profile redactions
and non-claims. The run list's paging bounds remain below the rows, and selected
details keep their own canonical identity and truncation facts.

Press `/` outside an editable field to focus search, `Esc` in search to clear
the query, and `?` to open shortcut help. The top-bar help button provides the
same dialog; `Esc` closes it and native Tab/Enter navigation remains available.
Section navigation and the skip link move focus without replacing the current
view fragment. The theme control cycles **system → light → dark**. Explicit
theme choice is part of the fragment, survives reload and browser navigation,
and does not require local storage.

Use the labelled search field to narrow the rendered runs, the outcome filter
to restrict them by recorded outcome, and the sort control to reorder them.
Select a run to inspect its detail and select one of its available attempts to
inspect that attempt. These controls only reveal, hide, or reorder facts
already embedded in the file. They do not fetch more history, change journal
state, trigger CI, retry or cancel work, or confer acceptance authority.

Search and filtering cover only the admitted run pages embedded in the report,
not the entire journal. Attempt drilldowns cover the bounded attempt listings,
active attempts, and explicit CLI selections. The page retains the run paging
and attempt truncation facts, including empty and missing selections; a filter
with no matches is not evidence that the journal contains no such run. No
interaction expands an observation bound or removes a redaction. With
JavaScript disabled, the document still exposes its admitted report and
embedded details for reading; the interactive controls are unavailable.

The HTML report updates live when the shell watches: the shell rewrites the
file on the profile's watch interval, replaces it atomically so a reader never
sees a half-written document. The fixed script reloads the current URL at that
interval; a meta-refresh instruction inside `noscript` provides the fallback
when JavaScript is disabled. Search, outcome, sort, run/attempt selection and
theme state are kept in the local URL fragment and restored after reload,
without local storage. A selection absent from the replacement report is reported as
missing rather than silently replaced with another identifier. Without
`--watch` the report is a snapshot of one admitted revision.

```console
$ kiln-dashboard --journal path/to/journal.json --profile path/to/profile.json \
    --format html --output dashboard.html --watch
```

## Real repository histories

`config/dashboard-catalog.ncl` admits a closed, versioned catalog of repository
declarations and explicit journal/store histories. Paths resolve against the
exported catalog, not the caller's working directory. Named bounds cover catalog
bytes, projects, histories, aggregate input bytes and rendered HTML bytes. Duplicate
identifiers, unsupported versions, unknown fields, malformed streams and exceeded
bounds fail before replacing the report. Output cannot replace a declared input.

`config/dashboard-kiln.ncl` selects seven retained real Kiln observations: GitHub
pass, assertion failure, controller/bridge recovery and an earlier unresolved run;
Radicle self-CI pass, genuine Nix build failure and unavailable worker. Export it
beside its Nickel source to preserve relative input resolution:

```console
$ nickel export --format json --output config/dashboard-kiln.json config/dashboard-kiln.ncl
$ kiln-dashboard --catalog config/dashboard-kiln.json \
    --profile fixtures/dashboard/profile-v1.json --format html --output dashboard.html
```

Repository, source and revision labels are operator declarations, not verified
source facts. Each embedded report retains its own canonical identities and
redactions; there is no combined revision or global observation identity. The
repository/history controls and per-history filters survive reload through the URL
fragment. Missing selections are reported without substituting another history.
Without JavaScript all histories remain readable through ordinary section links.
Use `--watch` for local regeneration, or `--refresh-seconds 30` for a finite render
that asks the browser to reload. The latter is HTML-only, accepts 1..86400 seconds
and cannot be combined with `--watch`.

### GitHub replay export

The GitHub replay database is not the canonical SQLite event-store schema. Export
one exact run through its provider shell, using the original nonsecret profile and
the run's 64-character BLAKE3 identity:

```console
$ kiln-adapter-github export --profile github-ci.json --run "$RUN_BLAKE3" \
    --output github-journal.json
```

`--database-snapshot retained.sqlite` selects a retained snapshot without changing
the original profile identity. The exporter opens an existing database read only,
checks its schema and profile-bound run, frames the original event sequence,
recomputes event identities and replays it before atomic output. Missing, malformed,
stale-profile or corrupt state fails without a successful report. It resolves no
credentials and does not dispatch, reconcile or publish a status. Canonical database,
profile and sidecar paths are protected against output replacement.

### Atomic delivery to Aspen3

`scripts/dashboard-publish.py` is a separate imperative shell; the dashboard itself
has no transfer capability. Apply `config/dashboard-publication.ncl` to reviewed
operator settings and export the result as JSON. For a JSON file under `config/`,
the following settings use a package built by `nix build path:.#kiln`:

```nickel
(import "dashboard-publication.ncl") {
  schema_version = 1,
  dashboard_executable = "../result/bin/kiln-dashboard",
  catalog = "dashboard-kiln.json",
  profile = "../fixtures/dashboard/profile-v1.json",
  local_report = "../dashboard-last-good.html",
  destination = {
    kind = "ssh",
    host = "brittonr@aspen3",
    path = "/home/brittonr/.local/share/kiln/dashboard/index.html",
  },
  max_report_bytes = 16777216,
  subprocess_timeout_seconds = 30,
  interval_ms = 30000,
}
```

Save the applied configuration as `config/dashboard-aspen3.ncl`, then run:

```console
$ nickel export --format json --output config/dashboard-aspen3.json config/dashboard-aspen3.ncl
$ python3 scripts/dashboard-publish.py --config config/dashboard-aspen3.json
$ python3 scripts/dashboard-publish.py --config config/dashboard-aspen3.json --watch
```

Both report parents must already exist. SSH delivery requires local/remote `rsync`,
an enrolled host key and noninteractive SSH access; it never provisions a service or
directory. Only admitted bounded HTML is transferred, not catalogs, journals,
databases or credentials. Generation and transfer have finite process deadlines.
`--watch` runs finite renders with browser refresh at the configured cadence; missed
ticks do not queue catch-up attempts. Generation/admission/transfer failure is
visible and retains the last uncommitted good report. Rsync uses temporary-file
replacement, never `--inplace`. A lost acknowledgement after remote rename can
leave the complete new remote report and the old local report: two-host atomicity
and directory-fsync durability are not claimed.

The current catalog connects retained executions, not a fresh branch-tip CI run.
Duplicate delivery and controller/bridge recovery facts belong to the cited retained
qualification receipts. Rendering, reloading or transferring them does not rerun CI,
prove current credentials/runtime availability, or authorize provider publication.
Kiln dashboard/provider maintainers own this composition; operators own input
freshness, SSH enrollment and delivery targets.

## Developer preview

From the repository root, generate the real HTML renderer's output with one command:

```sh
nix develop path:. -c cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview
```

Open `/tmp/kiln-dashboard-preview/dashboard.html` directly in a browser. The
example writes its synthetic canonical journal beside the HTML, admits that
journal through the production admission/report path, and uses the checked-in
Nickel-exported dashboard profile. Six named runs cover success, domain failure,
pending, unknown and cancellation; `api-checks` includes a failed attempt and a
successful retry. The example never dispatches its requested provider effects.
The sample is developer rehearsal, not evidence of real CI execution.

For the edit/rebuild/reload loop, the dev shell includes `watchexec`:

```sh
nix develop path:. -c watchexec --restart \
  --watch crates/kiln-dashboard/src \
  --watch crates/kiln-dashboard/examples \
  --watch fixtures/dashboard/profile-v1.json \
  -- cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview --live
```

Keep that command running and the generated file open. Saving renderer Rust,
CSS, JavaScript, example or exported profile source rebuilds and atomically
replaces the HTML. The page reloads at the admitted profile interval while its
fragment retains filters, selection and theme. There is no web server, bundler,
package-manager install or network request from the page. `--live` alone enables
page reload; `watchexec` is what rebuilds source. A failed build is reported by
Cargo and leaves the last successfully generated page in place. Stop the watcher
with Ctrl-C; the output remains a local file and no provider has been contacted.

Presentation sources are `crates/kiln-dashboard/src/render/html.rs`, `html.css`
and `html.js`. The preview generator is `crates/kiln-dashboard/examples/preview.rs`.
Use `cargo test --locked -p kiln-dashboard` and
`cargo clippy --locked -p kiln-dashboard -p kiln-cli --all-targets -- -D warnings`
inside the dev shell for focused checks. Inspect both color schemes, a narrow
viewport and no-script rendering in a real browser; Rust tests do not establish
layout or keyboard usability.


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

The rendered HTML report has a specification for the same tool's browser
driver. Its properties check the observed facts, the visible run filter and
sort results, selected run and attempt details, and the absence of external
resources while actions explore the local controls. This is a read-only
browser rail, not a CI control surface or proof of journal completeness.

The browser rail runs where a browser is reachable. It is not a Nix check
because the build sandbox gives the browser no loopback it can reach, and the
builder refuses a derivation that asks to run without the sandbox. The crate's
Rust tests check canonical bounded drilldown parity, redaction, deterministic
bytes, and hostile-identifier escaping. The browser properties cover
self-containment and local interactions. Run the browser rail locally with:

```console
$ kiln-dashboard --journal checks/bombadil/journal-pilot.json \
    --profile fixtures/dashboard/profile-v1.json --format html --output dashboard.html
$ bombadil browser test --headless --time-limit 30s \
    "file://$PWD/dashboard.html" checks/bombadil/kiln-dashboard-html.spec.ts
```

The terminal properties state their expectations only once the captured state
carries the program's exit status. A terminated terminal trace satisfies
Bombadil's `eventually` operator vacuously, which would make those properties
unfalsifiable, so the terminal rail uses `always` with the exit-status condition
instead. Browser properties inspect the current rendered document throughout
interaction. The commands above describe how to run the rails; they are not a
claim that a particular interactive report has passed them.

The terminal rail covers three journal fixtures. Neither rail claims to have
explored every journal, profile, browser, or operator input.

## Non-claims

Passing dashboard checks do not prove CI success, provider correctness, source
trust, journal completeness, release readiness, or that the observed revision is
the newest one in existence. They prove only the checked journal admission,
projection, bounds, and observed revision.
