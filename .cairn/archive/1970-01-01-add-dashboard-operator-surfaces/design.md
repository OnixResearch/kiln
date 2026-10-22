# Design: dashboard operator surfaces

## Context

The dashboard is a local read-only surface over Kiln's canonical CI state. It
renders the accepted `kiln-observe-core` projection of one admitted record
stream. The record streams Kiln owns locally are the journal documents the
shells write and the SQLite event store the store adapter owns.

The operator CLI, `kiln`, is the documented entry point for operators. It
prints canonical state (`kiln demo`) and mints evidence identities
(`kiln evidence`).

## Goal and Success Contract

Completion requires these results:

- The dashboard observes a canonical SQLite event store without writing to it.
- The operator CLI reaches the dashboard surface through one implementation.
- The property rail covers the operator CLI's demo and evidence surfaces, and
  every property can fail.
- The dashboard reads a journal written by a Kiln shell, not only by its own
  example.
- The dashboard keeps its read-only local boundary: no network, no provider
  effect, no mutation of the observed input.

False completion includes these results:

- A store input that creates, migrates, or repairs the database it reads.
- A second rendering implementation behind the CLI command.
- Properties that cannot fail, or a rail that only covers the happy path.
- A dashboard that claims a shell-written journal was read when the fixture
  came from the dashboard's own generator.

## Decisions

### Decision: Open the store read only

**Choice:** The store adapter gains `open_read_only`, which opens the database
with a read-only flag, admits the stored schema version instead of
initializing one, and refuses a missing store. The dashboard loads the named
stream, verifies the records with the store's own identity admission, and
replays them with the store's own checked replay.

**Rationale:** The dashboard's boundary is read-only observation. The existing
`open` path creates the schema, sets the journal mode, and can migrate or
repair, which is a writer's job. A reader that reuses it would mutate the
observed store and would create a database where none existed. The schema
stays owned by the adapter: the dashboard never reads the tables itself.

### Decision: Admit the canonical store profile, not an operator-supplied one

**Choice:** The adapter publishes `canonical_profile`, mirroring
`config/sqlite-profile.ncl`, and the dashboard uses it. The operator names the
store and the stream, not a profile.

**Rationale:** The store was created under an admitted profile. A reader that
accepts a different profile could read a store under weaker bounds than the
writer used. The canonical profile is the profile the Nickel contract
declares, so a reader cannot weaken the admission.

### Decision: One implementation behind two entry points

**Choice:** The dashboard's imperative shell moves into the library as a named
shell module. The standalone binary and the `kiln dashboard` subcommand both
call it.

**Rationale:** Two copies of the watch loop and the argument parsing would
drift, and the operator would see two different reports. The conformance
boundary audit keeps the pure core free of the shell's reach, and the shell
module is named as a shell entry so the audit still holds.

### Decision: The rail reads the compact terminal text

**Choice:** The CLI specifications read the terminal rows with their line
breaks and padding removed before they assert.

**Rationale:** A terminal wraps long lines. The demo prints one long JSON
document, and a wrapped line splits a token across rows, so a property that
searched the raw rows would fail on a correct program. Reading the compact
text asserts the rendered content rather than the terminal's line breaking.

### Decision: Keep the Celld store out of scope

**Choice:** The dashboard does not read the Celld worker event store.

**Rationale:** That store is reached over authenticated HTTPS. Giving the
dashboard a network path would break the boundary this work just made
explicit, and it would add a transport to a local file reader. The exclusion
is stated in the operator documentation rather than left implicit.

## Risks / Trade-offs

- The pilot journal fixture carries a request timestamp, so it is a recorded
  artifact rather than a reproducible byte sequence. Its event identities are
  recomputed, so the dashboard admits it regardless of when it was written.
- The store input depends on the linked SQLite runtime version matching the
  admitted profile. A reader with a different runtime refuses the store rather
  than reading it under a different cohort.
- The property rail covers three fixtures and three CLI commands. It is not an
  exhaustive exploration of the journal space or of the CLI's argument space.

## Verification Strategy

- The dashboard crate's tests admit a store written by the store adapter's own
  writer and project it, and prove that a read neither creates nor changes a
  store.
- The pilot fixture is a journal written by the pilot's own writer, checked in
  and read by the rail.
- Every property was exercised against a deliberately wrong expectation and
  reported a violation, including the three new CLI specifications.
- The conformance boundary audit keeps the dashboard's pure core free of the
  filesystem, network, process, clock, environment, and SQLite reach.
- The workspace gates, the Nickel gate, and the Cairn gates run unchanged.
