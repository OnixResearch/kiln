# Cron Schedules Specification

## Purpose

Defines the `cron-schedules` capability.

## Requirements

### Requirement: Cron schedules with IANA time zones

r[kiln.schedules.cron]
Web automation schedules and hosted pipeline schedule entries MUST accept a five-field cron expression with an IANA time zone, defaulting to `UTC`. Time zones MUST resolve from an exactly pinned bundled database, never from host zoneinfo. Malformed expressions, unknown time zones and over-bound expressions MUST be rejected at admission. Exact expressions MUST never be moved.

### Requirement: Deterministic hashed spreading

r[kiln.schedules.hash]
`H`, `H(a-b)` and `H/n` MUST resolve from a BLAKE3 hash of the project and schedule identity only.

#### Scenario: Stable across restarts

r[kiln.schedules.hash.scenario.restart]
- GIVEN a schedule using `H`
- WHEN it is compiled again in a fresh process or after a restart
- THEN the resolved occurrences MUST be identical

#### Scenario: Identity change

r[kiln.schedules.hash.scenario.identity]
- GIVEN the same expression
- WHEN the project or schedule identity changes
- THEN the resolved values MUST be recomputed from the new identity, and an unchanged identity MUST keep them

### Requirement: DST transitions fire once

r[kiln.schedules.dst]
Occurrences in a non-UTC time zone MUST fire exactly once per intended wall-clock occurrence across DST transitions.

#### Scenario: Spring forward and fall back

r[kiln.schedules.dst.scenario.new_york]
- GIVEN `30 2 * * *` and `30 1 * * *` in `America/New_York`
- WHEN occurrences are enumerated across the March and November transitions
- THEN each day MUST yield exactly one occurrence. The nonexistent 02:30 MUST fire once at 03:30 EDT, and the repeated 01:30 MUST fire once at 01:30 EDT.

### Requirement: Minimum interval

r[kiln.schedules.min_interval]
Each cron schedule MUST declare a minimum interval no lower than a named floor. Entries with consecutive occurrences closer than the minimum MUST be rejected at review across a bounded horizon. They MUST be rejected again when a run is reserved.

#### Scenario: Review and runtime rejection

r[kiln.schedules.min_interval.scenario.reject]
- GIVEN `*/5 * * * *` with a ten-minute minimum interval, or a reservation whose occurrence is closer than the minimum to the last fired run
- WHEN the policy or pipeline is reviewed, or the reservation is decided
- THEN review MUST fail, and the reservation MUST be recorded as too close without a run

### Requirement: No catch-up

r[kiln.schedules.no_catch_up]
Schedules MUST reserve durably, MUST never replay occurrences missed while the service was down, and MUST keep fair turns within `max_tick_effects`.

#### Scenario: Restart during a missed window

r[kiln.schedules.no_catch_up.scenario.restart]
- GIVEN a retained next occurrence that passed more than one minimum interval before a restart
- WHEN the scheduler runs after the restart
- THEN no run MUST be produced for the missed occurrences, and the next occurrence MUST be strictly after the restart time

### Requirement: Hosted schedule trigger

r[kiln.schedules.hosted]
A hosted pipeline MUST be able to declare a `schedule` trigger that references a reviewed schedule entry. A reservation MUST bind the reviewed plan identity, the occurrence and the exact observed revision of the entry's reviewed reference. A changed plan MUST re-initialize the schedule without firing.

### Requirement: Interval schedules replay unchanged

r[kiln.schedules.replay]
Existing interval schedules, their retained state, policy fingerprints and pipeline plans without schedules MUST serialize byte-identically and behave as before.
