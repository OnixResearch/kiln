# Repository Lifecycle

## ADDED Requirements

### Requirement: Native lifecycle store

r[kiln.lifecycle.native_store]

Kiln SHALL keep active and accepted Cairn lifecycle content in the native `.cairn/` store and SHALL NOT retain a parallel legacy `cairn/` store.

#### Scenario: Migrated content preserves bytes

r[kiln.lifecycle.native_store.scenario.parity]

- **GIVEN** a reviewed migration preview that binds every legacy source path and content hash
- **WHEN** the operator executes that exact plan
- **THEN** every destination file has the source file's bytes
- **AND** the execution receipt records the reviewed plan hash

#### Scenario: Active paths use the native store

r[kiln.lifecycle.native_store.scenario.active_paths]

- **GIVEN** a completed migration
- **WHEN** repository documentation, policy, and lifecycle commands select lifecycle content
- **THEN** they use `.cairn/`
- **AND** no legacy `cairn/` lifecycle directory remains

#### Scenario: A stale plan fails closed

r[kiln.lifecycle.native_store.scenario.stale_plan]

- **GIVEN** a migration plan whose source tree no longer matches the current repository
- **WHEN** an operator attempts to execute that plan
- **THEN** Cairn rejects the operation before mutation
- **AND** the diagnostic identifies the invalid source or plan state
