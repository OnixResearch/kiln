# Tasks: add the dashboard operator surfaces

## Phase 1: Store input

- [x] [serial] T1 Add a read-only open path and a canonical profile to the SQLite event store adapter. r[kiln.observe.dashboard.store]
- [x] [parallel] T2 Add the dashboard store input that loads a named stream, verifies its identities, and replays it canonically. r[kiln.observe.dashboard.store.scenario.match]
- [x] [parallel] T3 Add the tests that admit a store the adapter wrote and that prove a read never creates or changes a store. r[kiln.observe.dashboard.store.scenario.read_only]

## Phase 2: Operator surfaces

- [x] [serial] T4 Move the dashboard's imperative shell into the library and add the `kiln dashboard` subcommand over the same implementation. r[kiln.observe.dashboard.cli]
- [x] [parallel] T5 Add the CLI property specifications for the demo and evidence surfaces, including the malformed evidence rejection. r[kiln.observe.dashboard.cli_rail]
- [x] [parallel] T6 Write a journal with the OnixOS pilot's own writer, check it in, and add it to the rail. r[kiln.observe.dashboard.pilot_journal.scenario.match]

## Phase 3: Wiring, documentation, and evidence

- [x] [serial] T7 Extend the conformance boundary audit for the dashboard's shell entries and the SQLite runtime reach, and document the inputs, the CLI command, and the excluded Celld store. r[kiln.observe.dashboard.nonclaims]
- [x] [serial] V1 Run the focused tests, the workspace gates, the Nickel gate, and the property rail with its negative controls. r[kiln.observe.dashboard.cli_rail.scenario.match]
- [x] [serial] V2 Run the Cairn gates, validation, traceability coverage, and release readiness, then sync and archive the change with its evidence. r[kiln.observe.dashboard.nonclaims.scenario.scope]
