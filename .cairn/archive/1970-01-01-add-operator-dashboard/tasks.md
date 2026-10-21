# Tasks: add a local operator dashboard

## Phase 1: Profile and journal admission

- [x] [serial] T1 Define the typed dashboard profile with the observation profile, run page limit, run page count, event tail limit, watch interval, and non-claims, and admit it under the named bounds. r[kiln.observe.dashboard.profile]
- [x] [parallel] T2 Add positive and negative dashboard profile fixtures covering a complete profile, a zero bound, and an unknown field. r[kiln.observe.dashboard.profile.scenario.match] r[kiln.observe.dashboard.profile.scenario.zero_bound] r[kiln.observe.dashboard.profile.scenario.missing_non_claim]
- [x] [serial] T3 Admit a journal document by recomputing every record's event identity under the encoding convention its identity kind declares, by replaying the whole record set canonically, and by reporting the writer-owned document identity as an observed fact. r[kiln.observe.dashboard.journal]
- [x] [parallel] T4 Add negative journal fixtures covering a substituted event, an unsupported identity kind, and a record stream that does not replay, and a canonical prefix case. r[kiln.observe.dashboard.journal.scenario.match] r[kiln.observe.dashboard.journal.scenario.substituted_event] r[kiln.observe.dashboard.journal.scenario.unsupported_identity_kind] r[kiln.observe.dashboard.journal.scenario.replay_failure] r[kiln.observe.dashboard.journal.scenario.truncated_prefix]

## Phase 2: Projection and report

- [x] [serial] T5 Derive every reported fact from the accepted observation projection and carry the projected view identity. r[kiln.observe.dashboard.projection]
- [x] [parallel] T6 Add a parity test proving the dashboard view equals the directly projected view in content and identity, a redaction test proving a hidden field is absent and named, and an unknown-target test. r[kiln.observe.dashboard.projection.scenario.parity] r[kiln.observe.dashboard.projection.scenario.redaction] r[kiln.observe.dashboard.projection.scenario.unknown_target]
- [x] [serial] T7 Render a deterministic report that states the state revision, the declared journal identity, the observed bounds, and the bounded run, attempt, product, metric, and event counts. r[kiln.observe.dashboard.report]
- [x] [parallel] T8 Add deterministic-render and bounded-count tests. r[kiln.observe.dashboard.report.scenario.deterministic] r[kiln.observe.dashboard.report.scenario.bounded]

## Phase 3: Read-only shell and wiring

- [x] [serial] T9 Implement the dashboard shell so it reads only the named journal and profile, renders, and re-reads on the declared interval in watch mode. r[kiln.observe.dashboard.shell]
- [x] [parallel] T10 Add read-only and watch-mode tests. r[kiln.observe.dashboard.shell.scenario.read_only] r[kiln.observe.dashboard.shell.scenario.watch]
- [x] [serial] T11 Register the crate in the workspace, add the Nix app, add the crate to the conformance reference-boundary audit, audit the pure core against the shell runtime boundary, and document the dashboard in the operator guide, the dashboard guide, and the README. r[kiln.observe.dashboard.shell]

## Phase 4: Verification and completion evidence

- [x] [serial] V1 Run the focused crate tests, workspace tests, strict Clippy, formatting, and the Nickel gate. r[kiln.observe.dashboard.report]
- [x] [serial] V2 Run Cairn proposal, design, and task gates, validation, policy export, and traceability coverage and confirm the dashboard requirements are traced. r[kiln.observe.dashboard.nonclaims]
- [x] [serial] V3 Exercise the dashboard against a journal document and record the rendered report and the limits and non-claims it proves. r[kiln.observe.dashboard.nonclaims.scenario.scope]
