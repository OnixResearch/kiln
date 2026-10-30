## Context

- Web automation schedules (`crates/kiln-web-service/src/automation.rs`, `engine/automation.rs`) carry `interval_ms` with reviewed `min_interval_ms`/`max_interval_ms` bounds and a retained `next_due_ms`. The pure `next_effect` gives registrations round-robin turns within `max_tick_effects`. A due schedule is reserved durably as a `SourceRecord` and dispatched through `/api/runs` with its reference. Missed intervals advance `next_due_ms` past `now`.
- Hosted pipelines (`crates/kiln-hosted/src/pipelines.rs`) have no schedule trigger and no scheduler. Hosted execution runs a customer workspace on explicit request; there is no hosted source port that observes a default branch.
- `kiln-core` is `no_std` and must not carry time-zone data. `kiln-hosted` already depends on `kiln-web-service`.
- The lock file already contains `jiff 0.2.35` and `jiff-tzdb 0.1.8` (IANA 2026c) through another dependency.

## Goals and Success Contract

A cron schedule fires at most once per intended wall-clock occurrence in its time zone, at an instant every replica computes identically from the expression, time zone and identity. `H` values are stable across restarts and change only when the project or schedule identity changes. Entries closer than their minimum interval are rejected at review and again at reservation. A restart never produces runs for occurrences missed while down.

False completion includes:
- a duplicate or missing run at a DST transition;
- `H` derived from process state or wall-clock time;
- a catch-up burst after downtime;
- an interval schedule whose retained state or fingerprint changes;
- a time-zone lookup that depends on the host's zoneinfo;
- a hosted schedule that silently runs an unreviewed entry.

## Decisions

### Decision: Pinned bundled time-zone database

**Choice:** `jiff = { version = "=0.2.35", default-features = false, features = ["std", "tzdb-bundle-always"] }` in `kiln-web-service` only.
- Jiff resolves IANA names from the bundled `jiff-tzdb 0.1.8` data (tzdb `2026c`). The host's `/usr/share/zoneinfo` and `TZ` are never read, so occurrences are identical on every host and reproducible from the lock file.
- Jiff exposes explicit gap and fold disambiguation.
- Both crates are `Unlicense OR MIT`, and the tz data is public domain.

**Alternatives considered:**
- `chrono-tz` also bundles data, but adds `chrono`, `phf` and a build-time parser, and has weaker disambiguation APIs.
- System zoneinfo is not reproducible.

### Decision: Cron grammar and H

**Choice:** five fields:

| Field | Range | Names |
| --- | --- | --- |
| Minute | 0–59 | none |
| Hour | 0–23 | none |
| Day of month | 1–31 | none |
| Month | 1–12 | `JAN`–`DEC` |
| Day of week | 0–7, with 7 = Sunday | `SUN`–`SAT` |

- **Items:** `*`, `N`, `N-M`, `*/S`, `N-M/S`, `N/S` and comma lists. Items are bounded: `MAX_CRON_BYTES = 256` and `MAX_CRON_ITEMS = 64` per field.
- **H forms:** `H`, `H(a-b)`, `H/S` and `H(a-b)/S`:
  - `H` picks `start + h mod len`.
  - `H/S` picks the offset `h mod S`, then every `S`.
  - `h` is the first 8 bytes of the BLAKE3 hash of the domain `kiln.schedule.hash.v1`, each length-prefixed identity part, the field index and the item index.
  - `H` in day of month spans 1–28, so every month has the value.
- **Day matching:** Vixie rule. When both day of month and day of week are restricted (their text does not start with `*`), a day matches if either matches.
- **Exactness:** exact expressions are never moved. `resolved()` renders the expression with `H` replaced, for display and audit.

### Decision: Occurrences and DST

**Choice:** `next_after(after)` walks local civil dates forward for at most `MAX_SEARCH_DAYS = 1470` days, enough to reach any 29 February. For each matching day it walks the matching hours and minutes in civil order, converting each to an instant:
- A civil time in a gap (spring forward) fires once, at the wall time shifted by the gap (jiff `compatible`).
- A civil time in a fold (fall back) fires once, at the earlier instant.

The first candidate instant after `after` wins. When that candidate came from a gap, the scan continues across the shifted window and the earliest instant wins. Candidates that collapse onto one instant fire once, because occurrences are strictly increasing.

### Decision: Minimum interval at review and at reservation

**Choice:** each cron schedule has `min_interval_ms`. The floor is `CRON_MIN_INTERVAL_FLOOR_MS = 60_000`, and the Nickel default is `300_000` (5 minutes, as in Tangled).
- **Review:** `check_min_interval(start)` walks consecutive occurrences from the review clock across `CHECK_HORIZON_DAYS = 400` days, bounded by `CHECK_MAX_OCCURRENCES = 200_000`. It rejects any pair closer than the minimum, and rejects an expression with no occurrence in `MAX_SEARCH_DAYS`. Web automation runs this when the policy is loaded and configured; hosted runs it when the pipeline is reviewed, with the resolved `H` of its exact identity.
- **Reservation:** `decide(next_due, last_fired, now, min_interval)` returns one of:
  - `NotDue`;
  - `Missed` (`now >= due + min_interval`: no run, advance);
  - `TooClose` (`due - last_fired < min_interval`: no run, advance, record);
  - `Fire`.

  Every outcome advances to `next_after(now)`, so missed occurrences are never replayed.

### Decision: Web automation cron schedules

**Choice:**
- **Profile:** `ScheduleProfile.interval_ms` becomes `Option<u64>`, serialized only when present. `ScheduleProfile.cron: Option<CronSchedule { expression, timezone = "UTC" }>` is omitted when absent. Exactly one of the two must be set.
- **State:** `RegistrationState.last_fired_ms` is serde-default and omitted when absent.
- **Configure:**
  - A cron schedule's retained `next_due_ms` is initialized to `next_after(now)`.
  - Retained cron state must carry no interval, and operators may only enable or disable it. Enabling recomputes the next occurrence.
  - Existing interval schedules take exactly the old code path.
- **Reservation:** resolves the schedule reference in the project repository to the exact commit and records it as `source_revision`, so the run uses the exact default-branch revision observed at the due time. An unresolvable reference records a `rejected` source without retry. The `H` identity is `(project_id, schedule id)`.
- **Fairness:** `next_effect` round-robin is unchanged; a cron fire is one effect.

### Decision: Hosted schedule trigger

**Choice:**
- **Pipeline schema:** `Pipeline.schedules` maps ids to `{ cron, timezone = "UTC", min_interval_ms = 300000, reference = "refs/heads/main" }`. The trigger `{ event = "schedule", schedule = id }` must reference an entry, and every entry must be referenced.
- **Plan:** the compiled plan carries `schedules`, which are omitted when empty, so existing plan identities are unchanged.
- **Review:** checks the minimum interval with `H` resolved from `(organization, project, pipeline name, schedule id)`.
- **Ledger:** the new `hosted_pipeline_schedules` table keys `(organization, project, name, schedule)` and stores the plan identity, `next_due_ms`, `last_fired_ms`, the last observed revision and the status.
- **Reservation:** `pipelines.schedule { project, name, schedule, reference, revision }` initializes the ledger at `next_after(now)` without firing. A new plan identity re-initializes it. Otherwise it applies `decide`, records a `Fire` reservation durably with the exact observed revision of the reviewed reference, and returns the reserved occurrence and plan identity for `execution.run`.

**Rationale:** hosted has no source port that resolves a default branch. The reservation therefore binds exactly what its caller observed for the reviewed reference, and records it durably.

## Risks / Trade-offs

- Hosted revisions are caller observations, consistent with today's customer-supplied hosted workspaces. A provider-backed default-branch observation is a follow-up hookup.
- The review horizon is bounded. A minimum-interval violation that first appears after 400 days, or after 200,000 occurrences, is caught at reservation as `TooClose`.
- A poll interval longer than a schedule's minimum interval makes occurrences `Missed`. The operator documentation says so.

## Verification Strategy

- Unit tests in `schedule`:
  - grammar acceptance and rejection;
  - `H` stability for an identical identity across fresh compilations;
  - `H` change for a changed project or schedule identity;
  - exact expressions unchanged;
  - DST spring-forward and fall-back in `America/New_York` firing once per intended occurrence;
  - minimum-interval rejection;
  - `decide` covering fire, missed, too-close and restart-in-window (no catch-up).
- Web engine tests:
  - cron configure, fire and reservation with exact revision;
  - restart after a missed window producing no run;
  - runtime too-close rejection;
  - unchanged interval profile fingerprint and state bytes.
- Hosted tests:
  - the schedule trigger compiles;
  - an unknown entry is rejected;
  - review rejects a too-close entry;
  - the ledger initializes, fires once, skips missed windows and rejects stale plans.
- Nickel positive and negative fixtures.
