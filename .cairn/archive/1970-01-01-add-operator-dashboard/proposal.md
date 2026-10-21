# Add a local operator dashboard

## Why

Kiln can project its canonical CI state into bounded, deterministic observation
views, but an operator currently has no local surface for reading them. The
available paths are all remote or awkward:

- The HTTPS observation shell requires a certificate chain, a private key, and
  a per-request authorization token. It is a good remote surface, but it is the
  wrong shape for "what is this Kiln doing right now?" on the machine that
  holds the journal.
- The Iroh observation shell requires an enrolled peer, an endpoint key, and
  reachability.
- `kiln-standalone` exposes `render_status`, `wait_once`, and `LogStore`, but
  no binary uses them, so they are unreachable for an operator.
- The remaining CLI, `kiln`, offers only `demo`, `evidence`, and
  `host-qualify`. None of them read a live journal.

An operator debugging a run therefore reads raw journal JSON, or starts a
server, or greps a log file. There is no single read-only command that answers
"show me the runs, their outcomes, and the recent events" from the local
canonical state.

A local dashboard closes that gap without adding a second projection, a
control surface, or a new persistence format.

## What Changes

- Add a `kiln-dashboard` crate with a pure rendering core that turns admitted
  observation views into a deterministic terminal report.
- Read a canonical Kiln journal document, verify every journal record's event
  identity by recomputation, and replay it through the existing canonical
  replay path to rebuild admitted state.
- Report the journal document's writer-owned content identity as a declared
  fact instead of claiming to reproduce a shell-specific seal.
- Project that state through the accepted `kiln-observe-core` projection so the
  dashboard shows exactly the views the remote observation surfaces serve, with
  the same identities, bounds, paging, and redaction.
- Render a bounded, deterministic report of run outcomes, attempt outcomes,
  product and metric counts, and the recent event tail.
- Support a single `--watch` mode that re-reads the journal on a fixed interval
  and redraws, with an explicit bounded interval and no unbounded buffering.
- Add positive and negative tests for journal admission, identity
  recomputation, projection parity with the accepted core, and rendering
  determinism.

## Impact

- **Affected specs:** new `observation-dashboard` capability.
- **Planned files:** dashboard crate, typed Nickel dashboard profile and its
  fixtures, workspace and flake registration, operator documentation, and
  conformance reference-boundary and shell-core registration.
- **Compatibility:** additive only. No existing crate changes behavior, no
  journal format changes, and no remote surface changes.
- **Testing:** focused crate tests, workspace tests, strict Clippy, formatting,
  the Octet gate, the Nickel gate, Cairn validation, traceability coverage, and
  an end-to-end run against a real journal document.

## Non-Goals

- No new CI meaning, view vocabulary, or state model. The dashboard renders
  views that `kiln-observe-core` already admits.
- No control, dispatch, cancellation, retry, or provider effect. The dashboard
  is strictly read-only.
- No second journal format and no new persistence. The dashboard reads the
  journals that already exist.
- No live transport. The dashboard does not become an observation server, does
  not use the Iroh or HTTPS shells, and does not claim freshness beyond the
  journal revision it read.
- No terminal framework dependency. The report is plain text and ANSI escapes
  so the crate stays dependency-free.
- No claim that the dashboard proves CI success, provider correctness, source
  trust, or release readiness.
