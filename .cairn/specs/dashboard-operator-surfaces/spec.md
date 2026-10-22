# Dashboard Operator Surfaces Specification

## Purpose

Defines the `dashboard-operator-surfaces` capability.

## Requirements

### Requirement: Canonical event store input
r[kiln.observe.dashboard.store] The dashboard MUST admit a canonical SQLite event store as an observed input, MUST open it without writing to it, MUST verify the retained stream's event identities with the store's own admission, and MUST replay that stream canonically before projecting any view.

#### Scenario: Admitted store stream is projected
r[kiln.observe.dashboard.store.scenario.match]
- GIVEN a canonical SQLite event store holding one named stream
- WHEN the dashboard observes the store
- THEN the dashboard MUST report the stream's runs, attempts, and events under the admitted observation profile.

#### Scenario: The observed store is not changed
r[kiln.observe.dashboard.store.scenario.read_only]
- GIVEN an existing canonical SQLite event store and a missing store path
- WHEN the dashboard observes the store
- THEN the dashboard MUST NOT create, migrate, repair, or modify the store, and MUST refuse a missing store instead of creating one.

### Requirement: Shell-written journal input
r[kiln.observe.dashboard.pilot_journal] The dashboard MUST admit a journal document written by a Kiln shell under that shell's own event identity convention, and the property rail MUST cover such a journal.

#### Scenario: A shell-written journal is admitted
r[kiln.observe.dashboard.pilot_journal.scenario.match]
- GIVEN a journal document written by the OnixOS pilot's own writer
- WHEN the dashboard observes the journal
- THEN the dashboard MUST admit the journal, project its run, and report the state revision it observed.

### Requirement: Operator CLI dashboard command
r[kiln.observe.dashboard.cli] The operator CLI MUST expose the dashboard surface as `kiln dashboard` through the same implementation the standalone dashboard binary uses, and MUST NOT introduce a second rendering path.

#### Scenario: The CLI command renders the dashboard
r[kiln.observe.dashboard.cli.scenario.match]
- GIVEN an admitted journal or store and an admitted dashboard profile
- WHEN the operator runs `kiln dashboard`
- THEN the CLI MUST render the same bounded report as the standalone dashboard binary.

### Requirement: Operator CLI property rail
r[kiln.observe.dashboard.cli_rail] The property-based terminal rail MUST cover the operator CLI's demo and evidence surfaces, including the rejection of a malformed evidence input, and every property of that rail MUST be able to fail.

#### Scenario: The CLI surfaces satisfy the rail
r[kiln.observe.dashboard.cli_rail.scenario.match]
- GIVEN the packaged operator CLI and the checked-in evidence fixtures
- WHEN the rail drives the CLI's demo and evidence surfaces under a pseudo terminal
- THEN the demo MUST exit successfully and print the canonical state, the evidence command MUST print a content identity, a malformed evidence input MUST be refused with a named failure, and the rail MUST report no violation.

### Requirement: Operator surface non-claims
r[kiln.observe.dashboard.nonclaims] The operator surfaces MUST NOT claim CI success, provider correctness, source trust, journal or store completeness, release readiness, or that the rail explored every input.

#### Scenario: Operator surface evidence stays scoped
r[kiln.observe.dashboard.nonclaims.scenario.scope]
- GIVEN a passing dashboard or CLI rail run
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the checked inputs, properties, terminal session, and pinned tool revision.
