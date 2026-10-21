# Design: local operator dashboard

## Context

Kiln separates a pure CI domain from provider adapters and from the shells
that own files, clocks, and processes. Canonical state advances only as
committed journal records, and every shell rebuilds state by replaying those
records through `kiln_core::operations::replay`.

Read-only observation is already a first-class product concern. The accepted
`kiln-observe-core` crate projects admitted state plus explicit window facts
into six bounded view kinds, each carrying a `kiln.observe.view.v1` identity.
Two transport shells already serve those views: `kiln-adapter-http-observe`
over HTTPS and `kiln-adapter-iroh-observe` over Iroh.

Both remote shells are correct for their purpose, and neither answers the
local question. Each requires transport setup that a local debugging session
does not have: a certificate chain and a private key plus a per-request token,
or an enrolled peer endpoint key and reachability.

## Goal and Success Contract

The goal is a bounded, read-only, local report of canonical CI state.

Completion requires these results:

- A journal document is admitted only when every record's event identity is
  reproduced by recomputation and the whole record set replays canonically.
- The dashboard's facts come from the accepted observation projection, so a
  dashboard view and the corresponding remote view agree in content and
  identity.
- The report is deterministic: the same journal and profile always produce the
  same bytes.
- The report is bounded: run, attempt, and event counts respect the admitted
  profile bounds rather than the size of the journal.
- The dashboard performs no effect, mutates no state, and opens no network
  connection.
- Redaction stays a property of the profile, not of the renderer.

False completion includes these results:

- Rendering state that was not projected through the accepted core.
- Trusting a stored event identity instead of recomputing it.
- Treating a successful read as proof that the run succeeded.
- Claiming the displayed revision is the newest one in existence.
- Claiming that the dashboard reproduced a writer-owned document seal.

## Decisions

### Decision: Project through `kiln-observe-core` instead of reading state directly

**Choice:** The dashboard builds `ObservationRequest` and `ObservationWindow`
values and calls `kiln_observe_core::project_observation`, then renders the
resulting `ObservationView` bodies.

**Rationale:** The projection owns the view vocabulary, the bounds, the paging
policy, the redaction policy, and the view identity. Rendering raw `State`
would duplicate that vocabulary in the renderer and would let the dashboard
drift from the remote surfaces. Going through the accepted projection means one
view definition, and the dashboard's identity for a view is the same identity
the HTTPS shell serves for the same request and window.

### Decision: Verify event identity by recomputation

**Choice:** For every journal record, recompute the BLAKE3 identity of the
record's canonical event encoding and require it to equal the stored
`event_identity` native id, then replay the records through
`kiln_core::operations::replay`.

**Rationale:** A journal is a durable file that a shell writes and that other
processes read. The stored identity is an assertion about the event bytes; the
dashboard must check that assertion rather than repeat it. Recomputation
rejects a substituted event before it can influence what the operator is shown,
and canonical replay rejects a reordered or gapped record stream.

The `event_identity` native kind differs between the writers that exist today,
and so does the encoding convention behind it. `pilot-event-blake3` and
`ci-event-blake3` name the BLAKE3 of the packed Preserves encoding of the
event, while the SQLite store's `event-blake3` names the BLAKE3 of the
canonical JSON encoding. The dashboard therefore maps an admitted identity kind
onto its encoding convention and recomputes under that convention; an identity
kind with no admitted convention is rejected rather than guessed.

### Decision: Report the writer-owned document seal instead of reproducing it

**Choice:** The dashboard reads the journal document's `records` array and
reports the document's declared `identity` as an observed fact. It does not
recompute the document seal.

**Rationale:** The document seal covers a shell-specific tuple. The OnixOS
pilot seals the schema, request, ingress, records, native observations, and
worker result; the Radicle CI shell seals a different tuple over the same
records. A reader that does not know which shell wrote a document cannot
reproduce that seal, and a reader that guesses the tuple would verify a
different relation than the writer asserted.

The consequence is stated rather than hidden. A substituted or reordered
record is still rejected, because per-record identity recomputation and
canonical replay do not depend on the seal. A journal truncated to a canonical
prefix replays canonically and is admitted, so the report states the revision
it observed and the dashboard does not claim journal completeness.

### Decision: Keep the renderer pure and the crate dependency-free

**Choice:** The rendering core is a pure function from admitted views to text.
The binary reads files, owns the clock for `--watch`, and writes to a terminal.
The crate adds no third-party dependency.

**Rationale:** The pure core is directly testable without a terminal, a
process, or a clock, and the crate keeps Kiln's dependency surface unchanged.
The alternative, a terminal UI framework, would add a cohort and a license
notice for a report that is fundamentally text. The conformance crate audits
the split: the library sources may not reach the filesystem, the network, a
process, a clock, or the environment.

### Decision: Bound the report by the admitted profile

**Choice:** The dashboard requests views with limits derived from the profile
bounds, and the rendered report shows at most the admitted page size and event
tail size.

**Rationale:** The projection already caps page items and event records and
reports `capped`, `has_more`, and `has_earlier` facts. The dashboard surfaces
those facts instead of inventing its own bound, so an operator can tell that
the report is a bounded view rather than the whole history.

### Decision: Page the run list with the accepted cursor

**Choice:** The dashboard requests `run_list` pages with the profile's
requested page limit and follows the accepted `next` cursor until the list is
exhausted or the profile's page count is reached, and reports the pages it
consulted.

**Rationale:** The observation profile admits a small page size, so a single
`run_list` request returns a slice of the run set. Paging with the accepted
cursor is the only way to show more runs without a second projection, and the
dashboard reports truncation when it stops at its own page count.

### Decision: Keep the event window a tail of the record stream

**Choice:** The event window is the last `event_limit` records of the admitted
stream, clamped to the observation profile's record bound.

**Rationale:** The projection requires an event window that ends exactly at the
admitted state revision, and it rejects a window larger than its record bound.
The tail of a canonical stream satisfies both, keeps the window bounded for
any journal length, and reports `has_earlier` when the report omits earlier
history.

## Risks / Trade-offs

- The dashboard shows a bounded projection of one journal, so it can be stale
  relative to a running shell. The report states the state revision and the
  declared journal identity it read, and the watch interval is explicit and
  bounded.
- Journal documents are shell owned, so the dashboard reads a document shape
  rather than a database. It therefore does not cover the SQLite or Celld
  event stores, and it does not claim to be a general journal reader.
- A journal that is a canonical prefix of a longer journal is admitted,
  because the writer-owned document seal is not reproduced. The report states
  the observed revision and the dashboard does not claim completeness.
- A dashboard that renders only what the accepted projection admits cannot
  show a field the profile redacts. That is intended, and the report names the
  redacted fields so the omission is visible rather than silent.

## Verification Strategy

- Journal admission tests cover a valid document, a substituted event, an
  unsupported identity kind, a reordered record stream, and a canonical
  prefix.
- Projection tests assert that a dashboard view and the directly projected
  view have equal content and equal identity, that an unknown selection is
  reported as absent, and that a redacted field is named.
- Rendering tests assert deterministic output and profile-bounded counts.
- Shell tests run the built binary against a real journal document, assert a
  non-zero exit on a tampered journal, and assert that watch mode redraws.
- The conformance crate audits the pure core against the shell runtime
  boundary.
- An end-to-end run renders a real journal document and shows the report.
