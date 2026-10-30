## Why

Web automation schedules are interval-only, and hosted pipelines have no schedule trigger at all (issue #16). Intervals cannot express "nightly at 02:00 New York time" or "weekdays at 09:00". Many schedules starting at the same minute cause load spikes. Operators need calendar schedules with time zones and deterministic spreading, without losing the existing durable, no-catch-up, fair semantics.

## What Changes

- **Pure calendar core:** a new `kiln_web_service::schedule` module:
  - parses five-field cron expressions (numbers, ranges, lists, steps, month and day names);
  - resolves `H`, `H(a-b)` and `H/n` from a BLAKE3 hash of the project and schedule identity;
  - computes occurrences in an IANA time zone (default `UTC`), with DST gaps and folds firing once per intended occurrence;
  - validates a minimum interval across a bounded horizon;
  - decides at reservation time whether an occurrence fires, is missed (no catch-up) or is too close to the previous run.
- **Time zones:** the time-zone database is pinned exactly: `jiff = "=0.2.35"` with only `std` and `tzdb-bundle-always`, bundling IANA tzdb 2026c via `jiff-tzdb 0.1.8`. Both are already in the lock file.
- **Web automation (`kiln.web-automation.v1`):** a schedule has either `interval_ms` (unchanged) or `cron = { expression, timezone }`.
  - Cron schedules are validated at policy check and again at reservation.
  - The exact revision of the schedule reference is resolved at reservation and recorded on the source record.
  - Missed occurrences are skipped, and the existing round-robin fairness within `max_tick_effects` is kept.
- **Hosted pipelines (`kiln.hosted.pipeline.v1`):**
  - reviewed `schedules` entries (`cron`, `timezone`, `min_interval_ms`, `reference`);
  - a `{ event = "schedule", schedule = "<id>" }` trigger;
  - review-time minimum-interval validation with the resolved `H` values;
  - a durable `pipelines.schedule` reservation ledger that enforces the interval again, never catches up, and binds the observed default-branch revision.
- **Configuration and docs:** Nickel contracts and fixtures, operator and hosted CI documentation, and the dependency record in `LICENSES.md`.

## Impact

- **Consumers:** operators authoring web automation schedules, hosted pipeline authors, and the Tangled schedule mapping (#14).
- **Owners:** web-service maintainers own `schedule` and the automation shell; hosted maintainers own the pipeline schedule entries and ledger.
- **Compatibility:** interval schedules, retained `RegistrationState`, policy fingerprints and pipeline plan identities serialize byte-identically. Every new field is serde-default and omitted when unused.
- **Dependencies:** one new direct dependency (`jiff`), exactly pinned and recorded in `LICENSES.md`. `kiln-core` stays `no_std` and dependency-free of time-zone data.

## Out of Scope

- Seconds or year fields, `L`, `W`, `#` and `?` cron extensions.
- Catch-up or backfill of missed occurrences.
- A hosted source-observation port that resolves the default branch on its own. The hosted reservation binds the revision its caller observes; see the design.
- Changing interval schedule semantics.
