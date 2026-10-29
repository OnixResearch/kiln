# Operator dashboard

`kiln-dashboard` renders the accepted `kiln-observe-core` projection of a canonical
Kiln journal or store stream as a local terminal report or offline interactive
HTML page. A repository catalog groups independently admitted histories without
merging canonical state. The dashboard remains read only: no new view vocabulary,
projection semantics, control intake, canonical state or provider effect.

For authenticated enrollment, source-bound execution, logs, artifacts and controls,
use the separate [single-tenant live web service](operator.md#single-tenant-live-web-service).
Its authority and runtime prerequisites do not change this offline report's
read-only contract.

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

### SaaS-style workspace interface

Catalog reports open in one responsive application shell. **Overview** shows
declared project and independently admitted history inventory, explicit history
shortcuts and the selected history’s summary. These are not global CI totals:
overlapping journals are never combined into a new revision or observation.
**Projects** contains the real Rats index, declared metadata cards and accessible
HTML navigation. **Runs**, **Activity** and **Evidence** present the selected
report’s bounded run/attempt inspector, event tail and provenance respectively,
without nesting another application sidebar inside the report.

The header search matches declared project/history identifiers and labels,
source and revision metadata. Results open an exact existing history; no-match
queries display an explicit empty state. Run-ID/active-attempt search and outcome
filters remain inside **Runs**. Changing the project selector requires an explicit
history choice; a missing saved history remains missing rather than selecting a
replacement. Metadata and event text remain inert, including Unicode and markup.

**Settings** changes system/light/dark appearance and comfortable/compact spacing.
Route, metadata query, appearance, exact project/history and per-history run
filters live in the URL fragment and survive reload and browser Back. No account,
cookie or local-storage service is needed. `/` focuses metadata search outside
editable fields, `Esc` clears/closes search, and `?` opens keyboard help. The mobile
menu exposes the same navigation. **Reload report** rereads the current artifact;
it does not collect observations or run CI.

Reports retain opaque-origin `sandbox="allow-scripts"` isolation. A source-checked
bridge exchanges only inert saved state, admitted presentation choices and bounded
content-height messages; it grants neither same-origin access nor provider
authority. Without JavaScript, ordinary navigation and complete reports remain
readable. Unavailable WASM is reported visibly on Projects with HTML navigation
and native history controls still available.

This is a **SaaS-style read-only interface**, not a hosted SaaS service: no accounts,
tenants, billing, collaboration backend or CI execution controls are simulated.
Kiln dashboard maintainers own the shell and presentation bridge. Ordinary native
builds and catalog rendering reproduce it; unchanged pinned Rats assets require no
regeneration. The finite Aspen3 product preview is separate from `index.html` and
does not reconfigure or activate a publisher.

<!-- r[impl kiln.dashboard.product.scope] -->
<!-- r[impl kiln.dashboard.product.scope.scenario.delivery] -->

### Rats browser index

The catalog's primary index is the pinned Subwayrat `DataTable`, rendered by
Ratzilla 0.3.1's DOM backend through WebAssembly. Focus the table and use
Up/Down or Home/End to choose a row, then Enter to open its report. Pointer
selection chooses a row; **Open chosen history** activates it. Choosing a row
does not silently replace a missing active history. Repository/history identity
and each report's filters remain in the existing URL fragment.

The table receives only admitted metadata, never journal records or credentials.
Unicode table cells use ASCII escapes because this upstream widget truncates byte
slices; the chosen-row details and HTML navigation retain full Unicode labels.
At narrow widths the table shows one repository/history column. **HTML repository
and history navigation** remains an accessible alternative. If WASM initialization
fails, the page reports the failure and opens that navigation. Without JavaScript,
all report sections and ordinary links remain readable.

JavaScript, WASM and dependency notices are embedded in the same bounded HTML;
opening the file does not fetch modules, fonts, WASM or data. Existing atomic
publication transports that file unchanged. Native Kiln builds embed committed
assets and do not require a browser compiler, sibling checkout or private ratcore.

The isolated `web/dashboard-index/` workspace pins Rust 1.90.0 and wasm32; the
native workspace stays on Rust 1.88. Browser compilation also pins wasm-bindgen
0.2.121 and esbuild 0.27.2 through the existing Nix cohort. After changing browser
Rust, the loader or the reviewed source import, regenerate and check assets:

```console
nix develop path:.#dashboard-index -c python3 scripts/build-dashboard-index.py
nix develop path:.#dashboard-index -c python3 scripts/build-dashboard-index.py --check
nix build path:.#checks.x86_64-linux.dashboard-index-assets --no-link -L
```

Rebuild `kiln-dashboard` after regeneration, then rerender the catalog. Do not edit
`crates/kiln-dashboard/src/render/generated/` by hand. The sandboxed asset check
rebuilds and compares all three assets, runs the pure selection/boundary tests,
checks the actual wasm32 code with Clippy, and formats only Kiln-owned browser
source. It deliberately leaves pristine imported widget source unchanged.

Kiln dashboard maintainers own the browser shell, asset lock and repeatability
rail; Subwayrat owns the widget. [The retained integration evidence](../.cairn/archive/1970-01-01-use-rats-browser-index/evidence/verification.json)
covers the exercised offline Chromium inputs, not arbitrary Rats widgets, CI
success, deployment or release eligibility. Independent Patchgate holds remain.
<!-- r[impl kiln.dashboard.rats.claims] -->
<!-- r[impl kiln.dashboard.rats.claims.scenario.scope] -->
<!-- r[verify kiln.dashboard.rats.claims] -->
<!-- r[verify kiln.dashboard.rats.claims.scenario.scope] -->
<!-- r[verify kiln.dashboard.rats.offline] -->
<!-- r[verify kiln.dashboard.rats.offline.scenario.fallback] -->

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

## Producer-local supervised collection and publication

`kiln-dashboard-refresh --config PATH` performs **one finite cycle**: admit closed
configuration, scan a bounded immediate candidate root, atomically replace an
observation catalog, and call the matching publisher to render and deliver HTML.
It has no watcher loop, dispatch, reconciliation, signing, provider retry or status
write. Use the dedicated maintained Radicle fork self-CI rail to produce journals;
this observer does not re-enable retired GitHub/Iroh services or credentials.

Run it on the **producer that owns the canonical journals**, not on Aspen3. Its
only remote payload is self-contained HTML delivered through the existing enrolled
SSH route to
`brittonr@aspen3:/home/brittonr/.local/share/kiln/dashboard/index.html`. Candidate
journals, catalogs, profiles and CI credentials remain on the producer. Both local
report and remote destination parents must already exist; neither tool provisions
the remote host. The operator must separately establish noninteractive SSH, a
trusted host key, and remote `rsync` before enabling supervision.

### Immutable tools and explicit configuration

Build the small observation-tools package independently of Rust CI products:

```sh
set -eu
umask 077
mkdir -p "$HOME/.local/share/kiln" "$HOME/.local/bin" \
  "$HOME/.config/kiln" "$HOME/.config/systemd/user" \
  "$HOME/.local/state/kiln/dashboard"
nix build path:.#dashboard-tools \
  --out-link "$HOME/.local/share/kiln/dashboard-tools"
tools=$(realpath -e "$HOME/.local/share/kiln/dashboard-tools")
ln -sfn "$tools/bin/kiln-dashboard-refresh" "$HOME/.local/bin/kiln-dashboard-refresh"
ln -sfn "$tools/bin/kiln-dashboard-publish" "$HOME/.local/bin/kiln-dashboard-publish"
install -m 0644 "$tools/share/systemd/user/kiln-dashboard-refresh.service" \
  "$HOME/.config/systemd/user/kiln-dashboard-refresh.service"
install -m 0644 "$tools/share/systemd/user/kiln-dashboard-refresh.timer" \
  "$HOME/.config/systemd/user/kiln-dashboard-refresh.timer"
```

The out-link retains the immutable closure as a Nix GC root. The package contains
real `bin/kiln-dashboard-refresh` and `bin/kiln-dashboard-publish` executable
wrappers, Python, local `rsync` and OpenSSH runtime paths. Matching script sources
remain siblings under `libexec/kiln-dashboard/` for publisher reuse. Its only unit
files are under `share/systemd/user/`; installing them does not start anything.
Apps `nix run path:.#kiln-dashboard-refresh -- --config PATH` and
`nix run path:.#kiln-dashboard-publish -- --config PATH` select the same wrappers.
The existing root/default package and `kiln-dashboard` renderer retain their
meanings; the tools package does not build or select a renderer for the operator.

Select an existing absolute source root from the dedicated runner's actual
configuration and a separately reviewed immutable renderer executable. Do not
create an empty candidate directory and call that a CI observation. The following
procedure prompts for those real paths, copies the profile, and prepares typed
Nickel applications around explicit JSON inputs. Run it from the reviewed checkout:

```sh
set -eu
printf 'Existing producer-local self-CI candidate root: '
IFS= read -r candidate_root
printf 'Reviewed immutable kiln-dashboard executable: '
IFS= read -r dashboard_executable
case "$candidate_root" in /*) ;; *) echo 'absolute candidate root required' >&2; exit 1;; esac
case "$dashboard_executable" in /nix/store/*/bin/kiln-dashboard) ;; *) echo 'immutable renderer required' >&2; exit 1;; esac
test -d "$candidate_root" && test -x "$dashboard_executable"
install -m 0600 fixtures/dashboard/profile-v1.json "$HOME/.config/kiln/dashboard-profile.json"
python3 - "$PWD" "$candidate_root" "$dashboard_executable" <<'PY'
import json
from pathlib import Path
import sys

repo, root, executable = sys.argv[1:]
config = Path.home() / ".config/kiln"
state = Path.home() / ".local/state/kiln/dashboard"
publication = {
    "schema_version": 1,
    "dashboard_executable": executable,
    "catalog": str(state / "catalog.json"),
    "profile": str(config / "dashboard-profile.json"),
    "local_report": str(state / "last-good.html"),
    "destination": {
        "kind": "ssh", "host": "brittonr@aspen3",
        "path": "/home/brittonr/.local/share/kiln/dashboard/index.html",
    },
    "max_report_bytes": 16777216,
    "subprocess_timeout_seconds": 30,
    "interval_ms": 30000,
}
refresh = {
    "schema_version": 1,
    "candidate_root": root,
    "catalog_output": publication["catalog"],
    "publication_config": str(config / "dashboard-publication.json"),
    "project_id": "kiln-self-ci",
    "project_label": "Kiln dedicated Radicle fork self-CI",
    "max_candidates": 128,
    "max_histories": 32,
    "max_journal_bytes": 1048576,
}
for name, value in (("dashboard-publication", publication), ("dashboard-refresh", refresh)):
    inputs = config / (name + "-inputs.json")
    inputs.write_text(json.dumps(value, indent=2) + "\n")
    builder = Path(repo) / "config" / (name + ".ncl")
    (config / (name + ".ncl")).write_text(
        f"(import {json.dumps(str(builder))}) (import {json.dumps(str(inputs))})\n"
    )
PY
nickel export --format json --output "$HOME/.config/kiln/dashboard-publication.json" \
  "$HOME/.config/kiln/dashboard-publication.ncl"
nickel export --format json --output "$HOME/.config/kiln/dashboard-refresh.json" \
  "$HOME/.config/kiln/dashboard-refresh.ncl"
```

Review the exported JSON before executing it. The `.ncl` files retain absolute
references to the reviewed checkout's typed builders for subsequent re-export;
runtime requires only the exported JSON, profile, immutable renderer and tools.
Keep the separately selected renderer closure rooted too. The example bounds are
explicit operating choices, not claims about every producer's journal sizes or
retained history; choose bounds that admit the actual dedicated root.

The refresh configuration is closed version 1 with **exactly** these fields:

| Field | Meaning |
|---|---|
| `schema_version` | Integer `1` |
| `candidate_root` | Absolute existing producer-local immediate candidate directory |
| `catalog_output` | Absolute generated catalog file; must equal the publication config's resolved `catalog` |
| `publication_config` | Absolute existing publisher JSON configuration file |
| `project_id` | Catalog project identity, not a forge or source identity |
| `project_label` | Operator-declared display label |
| `max_candidates` | Positive bound on all immediate root entries, at most 1024 |
| `max_histories` | Positive retained-history bound, at most 128 and no greater than `max_candidates` |
| `max_journal_bytes` | Positive per-journal byte bound, at most 16777216; `max_candidates * max_journal_bytes` must not exceed 268435456 |

Only non-symlink 64-lowercase-hex candidate directories are eligible. Original
`<candidate_root>/<candidate>/ci-journal.json` files with schema
`kiln.radicle-ci.journal.v1`, `source = {provider, repository, revision}` and canonical
`records` feed the existing renderer; no journal is copied into a new canonical
state or changed. Missing journals are not completed runs. Present malformed,
oversized or symlink journals and input/output collisions fail visibly. Eligible
histories sort by observed journal `mtime_ns` descending and candidate identity
ascending for ties. Source metadata is an operator declaration, not source trust;
observed update order is not a global newest-source guarantee. Excess root entries
fail rather than silently choosing an arbitrary scan prefix. An owned no-follow
lock serializes finite cycles, including manually invoked collectors.

### Supervise only the named observation units

First exercise the real installed finite path with the reviewed JSON:

```sh
"$HOME/.local/bin/kiln-dashboard-refresh" --config "$HOME/.config/kiln/dashboard-refresh.json"
systemctl --user daemon-reload
systemctl --user enable --now kiln-dashboard-refresh.timer
systemctl --user status kiln-dashboard-refresh.timer kiln-dashboard-refresh.service
systemctl --user list-timers kiln-dashboard-refresh.timer
journalctl --user -u kiln-dashboard-refresh.service -n 40 --no-pager
```

The oneshot executes `%h/.local/bin/kiln-dashboard-refresh --config
%h/.config/kiln/dashboard-refresh.json`. Each activation finishes or fails within
five minutes; stop grants ten seconds before killing the service control group.
`UMask=0077` protects producer-local artifacts. `Restart=no` preserves visible
failures without an automatic service retry. The timer first activates after one
second, then waits `OnUnitInactiveSec=30s` after a cycle finishes; long cycles do
not overlap or accumulate catch-up work. `Persistent=false` provides no downtime
catch-up queue. The publisher's `interval_ms=30000` is the matching HTML/browser
cadence, not a second scheduling loop. A later timer observation is not a retry or
redispatch of CI.

When the SSH route uses an agent, explicitly import its current socket into the
user manager before starting the service:

```sh
systemctl --user import-environment SSH_AUTH_SOCK
```

The units intentionally do not hide the home directory or private temporary
directories, which could make the operator's enrolled SSH key/agent unavailable.
Agent availability after logout/reboot and user-manager persistence are operator
prerequisites, not supplied by this package; enabling the timer does not enable
lingering or provision credentials.

Restart only these units after reviewing restored inputs or updating the immutable
tool symlinks, and inspect the next actual cycle:

```sh
systemctl --user daemon-reload
systemctl --user restart kiln-dashboard-refresh.timer kiln-dashboard-refresh.service
systemctl --user status kiln-dashboard-refresh.timer kiln-dashboard-refresh.service
journalctl --user -u kiln-dashboard-refresh.service -n 40 --no-pager
```

Stopping the timer propagates to the active service through its `PartOf=` binding.
Stopping only the service leaves the timer armed and is not a durable shutdown.
To stop observation and prevent login-time reactivation:

```sh
systemctl --user disable --now kiln-dashboard-refresh.timer
systemctl --user stop kiln-dashboard-refresh.service
```

These commands never stop or restart CI, provider, forge or unrelated user units.
The observer rebuilds solely from durable journals and exact configuration after
restart. Collection, generation or transfer failures remain nonzero/journal-visible
and retain last-good HTML; a generated catalog is observation state, not evidence
that delivery succeeded. A lost remote acknowledgement may leave new complete HTML
on Aspen3 and the prior local report; cross-host atomicity is not claimed. An
interrupted cycle can leave temporary files but never a partially replaced final
HTML file. Stopped or failed publication leaves readable **stale** HTML: browser
reload is not proof of new collection, freshness or success. Fresh success/failure
proof requires separately executed dedicated self-CI and exact newly written
canonical histories, not retained runs or report timestamps.

The maintained `scripts/radicle-ci-live-self-ci.py` retires each completed case's
disposable `eval-home/.cache/nix` after its existing canonical, status and recovery
checks. It retains journals, exact source identities and logs, and never removes
the operator's original cache. Reserve scratch for one copied operator cache plus
build/evidence state; a failed case can retain its cache for explicit owned-state
cleanup. Stopping the dedicated canary retires its CI processes and credentials,
not the read-only dashboard timer. The timer does not provision a continuously
running production CI controller or make retained observations fresh again.

The [operated fresh proof](../.cairn/archive/1970-01-01-operate-dashboard-live/evidence/fresh-publication.json)
binds the dedicated fork's exact success, genuine compile failure and unresolved
provider-unavailable histories to the Aspen3 report. Publisher restart preserved
all canonical journal bytes; credential-free replay of the retained inputs
reproduced the delivered HTML exactly. This is a completed canary, not an ongoing
production CI service; the independent Patchgate/release holds remain in force.

Focused integration checks are `nix build path:.#checks.x86_64-linux.dashboard-publication`
and `nix build path:.#checks.x86_64-linux.nickel` (substitute the producer system
where appropriate). These cover collector/publisher behavior and typed positive
and expected-negative configuration admission; they do not install/start units,
qualify a runtime cohort, or waive separate Patchgate/release holds.
