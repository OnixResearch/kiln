//! Calendar (cron) schedules in IANA time zones with deterministic `H`
//! spreading.
//!
//! Pure: callers supply every clock reading. Time zones come only from the
//! exactly pinned bundled database (`jiff` `tzdb-bundle-always`), never from
//! host zoneinfo or `TZ`, so every replica computes identical instants.
//!
//! Grammar: five fields (minute, hour, day of month, month, day of week). Items
//! are `*`, `N`, `N-M`, `*/S`, `N-M/S`, `N/S` and comma lists; month and day
//! names (`JAN`, `MON`) are accepted. `H`, `H(a-b)`, `H/S` and `H(a-b)/S` pick
//! values from a BLAKE3 hash of the schedule identity. Day of month and day of
//! week combine with the Vixie rule: when both are restricted, either matches.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::{AmbiguousOffset, TimeZone, TimeZoneDatabase};
use serde::{Deserialize, Serialize};

/// Time zone used when an entry names none.
pub const DEFAULT_TIMEZONE: &str = "UTC";
/// Bytes per cron expression.
pub const MAX_CRON_BYTES: usize = 256;
/// Comma-separated items per cron field.
pub const MAX_CRON_ITEMS: usize = 64;
/// Bytes per IANA time-zone name.
pub const MAX_TIMEZONE_BYTES: usize = 64;
/// Lowest admitted minimum interval: cron resolution is one minute.
pub const CRON_MIN_INTERVAL_FLOOR_MS: u64 = 60_000;
/// Default minimum interval between occurrences (five minutes).
pub const DEFAULT_MIN_INTERVAL_MS: u64 = 300_000;
/// Highest admitted minimum interval (one year).
pub const MAX_MIN_INTERVAL_MS: u64 = 31_536_000_000;
/// Local days searched for the next occurrence: four years and slack, so a
/// 29 February entry is always found.
pub const MAX_SEARCH_DAYS: u32 = 1_470;
/// Review horizon for the minimum-interval check.
pub const CHECK_HORIZON_DAYS: u64 = 400;
/// Occurrences examined by one minimum-interval check.
pub const CHECK_MAX_OCCURRENCES: usize = 200_000;
/// Largest admitted timestamp (the JSON-safe integer bound used by retained state).
pub const MAX_TIMESTAMP_MS: u64 = 9_007_199_254_740_991;
const MILLISECONDS_PER_DAY: u64 = 86_400_000;
const HASH_DOMAIN: &[u8] = b"kiln.schedule.hash.v1";
/// `H` in day of month spans 1-28, so the value exists in every month.
const HASHED_DAY_OF_MONTH_MAX: u8 = 28;

fn default_timezone() -> String {
    DEFAULT_TIMEZONE.into()
}

/// One reviewed calendar entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CronSchedule {
    pub expression: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

/// Admission or evaluation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleError {
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

fn error<T>(code: &'static str, message: impl Into<String>) -> Result<T, ScheduleError> {
    Err(ScheduleError {
        code,
        message: message.into(),
    })
}

#[derive(Clone, Copy)]
struct Field {
    name: &'static str,
    min: u8,
    max: u8,
    names: &'static [&'static str],
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const FIELDS: [Field; 5] = [
    Field {
        name: "minute",
        min: 0,
        max: 59,
        names: &[],
    },
    Field {
        name: "hour",
        min: 0,
        max: 23,
        names: &[],
    },
    Field {
        name: "day of month",
        min: 1,
        max: 31,
        names: &[],
    },
    Field {
        name: "month",
        min: 1,
        max: 12,
        names: &MONTHS,
    },
    // 7 is an alias of Sunday (0).
    Field {
        name: "day of week",
        min: 0,
        max: 7,
        names: &WEEKDAYS,
    },
];
const DAY_OF_MONTH: usize = 2;
const DAY_OF_WEEK: usize = 4;

/// The pinned bundled IANA database; host zoneinfo is never consulted.
static BUNDLED: LazyLock<TimeZoneDatabase> = LazyLock::new(TimeZoneDatabase::bundled);

// r[impl kiln.schedules.cron]
/// Resolve an exact IANA name from the bundled database. Names must be spelled
/// canonically (case-sensitive), so one entry has one identity.
pub fn time_zone(name: &str) -> Result<TimeZone, ScheduleError> {
    if name.is_empty() || name.len() > MAX_TIMEZONE_BYTES {
        return error(
            "schedule_timezone",
            "time zone name length is outside 1..=64",
        );
    }
    let zone = BUNDLED.get(name).or_else(|_| {
        error(
            "schedule_timezone",
            format!("unknown IANA time zone {name}"),
        )
    })?;
    if zone.iana_name() != Some(name) {
        return error(
            "schedule_timezone",
            format!("time zone {name} must be spelled exactly as the IANA name"),
        );
    }
    Ok(zone)
}

/// The deterministic `H` source: BLAKE3 over a domain, the length-prefixed
/// identity parts, and the field and item position.
fn hashed(identity: &[&str], field: usize, item: usize) -> u64 {
    let mut hasher = blake3::Hasher::new();
    let mut part = |bytes: &[u8]| {
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    part(HASH_DOMAIN);
    for value in identity {
        part(value.as_bytes());
    }
    part(&(field as u64).to_le_bytes());
    part(&(item as u64).to_le_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(digest.as_bytes().get(..8).unwrap_or(&[0; 8]));
    u64::from_le_bytes(bytes)
}

fn value(field: &Field, text: &str) -> Result<u8, ScheduleError> {
    if let Some(position) = field
        .names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text))
    {
        return u8::try_from(position)
            .map(|position| position + field.min)
            .or_else(|_| error("schedule_syntax", "name index overflow"));
    }
    let parsed: u8 = text.parse().or_else(|_| {
        error(
            "schedule_syntax",
            format!("{} value {text} is invalid", field.name),
        )
    })?;
    if !(field.min..=field.max).contains(&parsed) {
        return error(
            "schedule_syntax",
            format!(
                "{} value {parsed} is outside {}-{}",
                field.name, field.min, field.max
            ),
        );
    }
    Ok(parsed)
}

fn range(field: &Field, text: &str) -> Result<(u8, u8), ScheduleError> {
    match text.split_once('-') {
        Some((low, high)) => {
            let (low, high) = (value(field, low)?, value(field, high)?);
            if low > high {
                return error(
                    "schedule_syntax",
                    format!("{} range {text} is reversed", field.name),
                );
            }
            Ok((low, high))
        }
        None => {
            let single = value(field, text)?;
            Ok((single, single))
        }
    }
}

fn step(field: &Field, text: Option<&str>) -> Result<Option<u8>, ScheduleError> {
    text.map(|text| {
        let step: u8 = text.parse().or_else(|_| {
            error(
                "schedule_syntax",
                format!("{} step {text} is invalid", field.name),
            )
        })?;
        if step == 0 || step > field.max - field.min + 1 {
            return error(
                "schedule_syntax",
                format!("{} step {step} is outside its range", field.name),
            );
        }
        Ok(step)
    })
    .transpose()
}

/// One parsed field: the value bitset, whether it starts with `*`, and its
/// rendering with every `H` resolved.
struct Parsed {
    bits: u64,
    star: bool,
    resolved: String,
}

fn parse_field(index: usize, text: &str, identity: &[&str]) -> Result<Parsed, ScheduleError> {
    let field = FIELDS.get(index).copied().ok_or_else(|| ScheduleError {
        code: "schedule_syntax",
        message: "field index".into(),
    })?;
    let items: Vec<&str> = text.split(',').collect();
    if items.len() > MAX_CRON_ITEMS {
        return error(
            "schedule_syntax",
            format!("{} has more than {MAX_CRON_ITEMS} items", field.name),
        );
    }
    let mut bits = 0_u64;
    let mut resolved = Vec::with_capacity(items.len());
    for (item_index, item) in items.iter().enumerate() {
        let (base, step_text) = match item.split_once('/') {
            Some((base, step)) => (base, Some(step)),
            None => (*item, None),
        };
        let step = step(&field, step_text)?;
        let hash_max = if index == DAY_OF_MONTH {
            HASHED_DAY_OF_MONTH_MAX
        } else if index == DAY_OF_WEEK {
            6
        } else {
            field.max
        };
        let (low, high, rendered) = if let Some(hashed_range) = base.strip_prefix('H') {
            let (low, high) = if hashed_range.is_empty() {
                (field.min, hash_max)
            } else {
                let inner = hashed_range
                    .strip_prefix('(')
                    .and_then(|v| v.strip_suffix(')'))
                    .ok_or_else(|| ScheduleError {
                        code: "schedule_syntax",
                        message: format!("{} hashed range must be H(a-b)", field.name),
                    })?;
                let (low, high) = range(&field, inner)?;
                if !inner.contains('-') {
                    return error(
                        "schedule_syntax",
                        format!("{} hashed range must be H(a-b)", field.name),
                    );
                }
                (low, high)
            };
            let span = u64::from(high - low) + 1;
            let seed = hashed(identity, index, item_index);
            match step {
                None => {
                    let chosen = low + u8::try_from(seed % span).unwrap_or(0);
                    (chosen, chosen, chosen.to_string())
                }
                Some(step) => {
                    let offset = u8::try_from(seed % u64::from(step)).unwrap_or(0);
                    let start = low.saturating_add(offset).min(high);
                    (start, high, format!("{start}-{high}/{step}"))
                }
            }
        } else if base == "*" {
            let high = if index == DAY_OF_WEEK { 6 } else { field.max };
            (field.min, high, (*item).to_string())
        } else if step.is_some() && !base.contains('-') {
            // `N/S` runs from N to the field maximum.
            (value(&field, base)?, field.max, (*item).to_string())
        } else {
            let (low, high) = range(&field, base)?;
            (low, high, (*item).to_string())
        };
        let mut current = low;
        loop {
            let mapped = if index == DAY_OF_WEEK && current == 7 {
                0
            } else {
                current
            };
            bits |= 1_u64 << mapped;
            let Some(next) = current.checked_add(step.unwrap_or(1)) else {
                break;
            };
            if next > high || step.is_none() && high == low {
                break;
            }
            current = next;
        }
        resolved.push(rendered);
    }
    Ok(Parsed {
        bits,
        star: text.starts_with('*'),
        resolved: resolved.join(","),
    })
}

/// A compiled schedule bound to its time zone and `H` identity.
#[derive(Clone, Debug)]
pub struct Cron {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    day_star: bool,
    weekday_star: bool,
    zone: TimeZone,
    resolved: String,
}

// r[impl kiln.schedules.cron]
// r[impl kiln.schedules.hash]
/// Compile an entry. `identity` names the owning project and schedule (for
/// example `[project_id, schedule_id]`); `H` depends on nothing else.
pub fn compile(spec: &CronSchedule, identity: &[&str]) -> Result<Cron, ScheduleError> {
    if spec.expression.is_empty()
        || spec.expression.len() > MAX_CRON_BYTES
        || !spec.expression.is_ascii()
        || spec.expression.chars().any(|c| c.is_ascii_control())
    {
        return error(
            "schedule_syntax",
            "expression must be 1..=256 printable ASCII bytes",
        );
    }
    let fields: Vec<&str> = spec.expression.split_ascii_whitespace().collect();
    if fields.len() != FIELDS.len() {
        return error(
            "schedule_syntax",
            "expression needs exactly five fields: minute hour day-of-month month day-of-week",
        );
    }
    let zone = time_zone(&spec.timezone)?;
    let mut parsed = Vec::with_capacity(FIELDS.len());
    for (index, text) in fields.iter().enumerate() {
        parsed.push(parse_field(index, text, identity)?);
    }
    let bits = |index: usize| parsed.get(index).map_or(0, |p| p.bits);
    let star = |index: usize| parsed.get(index).is_some_and(|p| p.star);
    Ok(Cron {
        minutes: bits(0),
        hours: bits(1),
        days: bits(DAY_OF_MONTH),
        months: bits(3),
        weekdays: bits(DAY_OF_WEEK),
        day_star: star(DAY_OF_MONTH),
        weekday_star: star(DAY_OF_WEEK),
        zone,
        resolved: parsed
            .iter()
            .map(|p| p.resolved.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    })
}

fn set(bits: u64) -> impl Iterator<Item = i8> {
    (0_i8..64).filter(move |value| bits & (1_u64 << value) != 0)
}

impl Cron {
    /// The expression with every `H` resolved; exact expressions are unchanged.
    #[must_use]
    pub fn resolved(&self) -> &str {
        &self.resolved
    }

    fn day_matches(&self, date: Date) -> bool {
        let day = self.days & (1_u64 << date.day()) != 0;
        let weekday = self.weekdays & (1_u64 << date.weekday().to_sunday_zero_offset()) != 0;
        if self.months & (1_u64 << date.month()) == 0 {
            return false;
        }
        match (self.day_star, self.weekday_star) {
            (true, true) => true,
            (true, false) => weekday,
            (false, true) => day,
            (false, false) => day || weekday,
        }
    }

    /// Occurrences strictly after `after_ms`, ascending, within [`MAX_SEARCH_DAYS`].
    #[must_use]
    pub fn occurrences(&self, after_ms: u64) -> Occurrences<'_> {
        let start = i64::try_from(after_ms)
            .ok()
            .and_then(|ms| Timestamp::from_millisecond(ms).ok())
            .map(|ts| ts.to_zoned(self.zone.clone()).date());
        Occurrences {
            cron: self,
            after: i64::try_from(after_ms).unwrap_or(i64::MAX),
            date: start,
            days: 0,
            pending: BTreeSet::new(),
            watermark: i64::MIN,
            last: i64::try_from(after_ms).unwrap_or(i64::MAX),
        }
    }

    // r[impl kiln.schedules.dst]
    /// The first occurrence strictly after `after_ms`.
    #[must_use]
    pub fn next_after(&self, after_ms: u64) -> Option<u64> {
        self.occurrences(after_ms).next()
    }

    // r[impl kiln.schedules.min_interval]
    /// Reject consecutive occurrences closer than `min_interval_ms` across
    /// [`CHECK_HORIZON_DAYS`] from `start_ms` (at most [`CHECK_MAX_OCCURRENCES`]),
    /// and entries that never fire within [`MAX_SEARCH_DAYS`].
    pub fn check_min_interval(
        &self,
        start_ms: u64,
        min_interval_ms: u64,
    ) -> Result<(), ScheduleError> {
        if !(CRON_MIN_INTERVAL_FLOOR_MS..=MAX_MIN_INTERVAL_MS).contains(&min_interval_ms) {
            return error(
                "schedule_min_interval",
                format!(
                    "minimum interval must be {CRON_MIN_INTERVAL_FLOOR_MS}..={MAX_MIN_INTERVAL_MS} ms"
                ),
            );
        }
        let horizon = start_ms.saturating_add(CHECK_HORIZON_DAYS * MILLISECONDS_PER_DAY);
        let mut occurrences = self.occurrences(start_ms);
        let Some(mut previous) = occurrences.next() else {
            return error("schedule_never_fires", "expression has no occurrence");
        };
        for _ in 1..CHECK_MAX_OCCURRENCES {
            let Some(next) = occurrences.next() else {
                break;
            };
            if next.saturating_sub(previous) < min_interval_ms {
                return error(
                    "schedule_min_interval",
                    format!(
                        "occurrences at {previous} and {next} ms are closer than {min_interval_ms} ms"
                    ),
                );
            }
            if next > horizon {
                break;
            }
            previous = next;
        }
        Ok(())
    }

    // r[impl kiln.schedules.no_catch_up]
    // r[impl kiln.schedules.min_interval]
    /// Decide a retained due occurrence at `now_ms`. Every outcome other than
    /// `NotDue` advances to the first occurrence strictly after `now_ms`, so
    /// occurrences missed while the service was down are never replayed.
    pub fn decide(
        &self,
        due_ms: u64,
        last_fired_ms: Option<u64>,
        now_ms: u64,
        min_interval_ms: u64,
    ) -> Result<Reservation, ScheduleError> {
        if now_ms < due_ms {
            return Ok(Reservation::NotDue);
        }
        let Some(next_due_ms) = self
            .next_after(now_ms)
            .filter(|next| *next <= MAX_TIMESTAMP_MS)
        else {
            return error("schedule_never_fires", "no later occurrence");
        };
        // An occurrence older than one minimum interval was missed (downtime
        // or a slow poll); running it now would be catch-up.
        if now_ms - due_ms >= min_interval_ms {
            return Ok(Reservation::Missed {
                occurrence_ms: due_ms,
                next_due_ms,
            });
        }
        if let Some(previous_ms) =
            last_fired_ms.filter(|previous| due_ms.saturating_sub(*previous) < min_interval_ms)
        {
            return Ok(Reservation::TooClose {
                occurrence_ms: due_ms,
                previous_ms,
                next_due_ms,
            });
        }
        Ok(Reservation::Fire {
            occurrence_ms: due_ms,
            next_due_ms,
        })
    }
}

/// The reservation-time decision for one retained due occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reservation {
    NotDue,
    Fire {
        occurrence_ms: u64,
        next_due_ms: u64,
    },
    Missed {
        occurrence_ms: u64,
        next_due_ms: u64,
    },
    TooClose {
        occurrence_ms: u64,
        previous_ms: u64,
        next_due_ms: u64,
    },
}

/// Ascending occurrences. Civil candidates are walked in local order; a
/// candidate inside a DST gap fires at the wall time shifted by the gap and is
/// held until every earlier instant was emitted. Fold candidates fire at the
/// earlier instant. Equal instants are emitted once.
#[derive(Debug)]
pub struct Occurrences<'a> {
    cron: &'a Cron,
    after: i64,
    date: Option<Date>,
    days: u32,
    pending: BTreeSet<i64>,
    /// Instant of the latest unshifted candidate: later candidates never map
    /// below it, so pending instants up to it are final.
    watermark: i64,
    last: i64,
}

impl Occurrences<'_> {
    fn emit(&mut self) -> Option<u64> {
        while let Some(first) = self.pending.first().copied() {
            if first > self.watermark {
                return None;
            }
            self.pending.remove(&first);
            if first > self.last {
                self.last = first;
                return u64::try_from(first).ok();
            }
        }
        None
    }

    fn scan_day(&mut self, date: Date) -> Option<()> {
        for hour in set(self.cron.hours) {
            for minute in set(self.cron.minutes) {
                let civil =
                    DateTime::new(date.year(), date.month(), date.day(), hour, minute, 0, 0)
                        .ok()?;
                let ambiguous = self.cron.zone.to_ambiguous_timestamp(civil);
                let shifted = matches!(ambiguous.offset(), AmbiguousOffset::Gap { .. });
                let instant = ambiguous.compatible().ok()?.as_millisecond();
                if instant > self.after {
                    self.pending.insert(instant);
                }
                if !shifted {
                    self.watermark = self.watermark.max(instant);
                }
            }
        }
        Some(())
    }
}

impl Iterator for Occurrences<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        loop {
            if let Some(found) = self.emit() {
                return Some(found);
            }
            let Some(date) = self.date.filter(|_| self.days < MAX_SEARCH_DAYS) else {
                // Search bound reached: flush held gap instants in order.
                self.watermark = i64::MAX;
                return self.emit();
            };
            self.days += 1;
            self.date = date.tomorrow().ok();
            if self.cron.day_matches(date) && self.scan_day(date).is_none() {
                self.date = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(expression: &str, timezone: &str) -> CronSchedule {
        CronSchedule {
            expression: expression.into(),
            timezone: timezone.into(),
        }
    }
    fn cron(expression: &str, timezone: &str, identity: &[&str]) -> Cron {
        compile(&spec(expression, timezone), identity)
            .unwrap_or_else(|e| unreachable!("fixture compiles: {e}"))
    }
    fn ms(text: &str) -> u64 {
        let parsed: Timestamp = text
            .parse()
            .unwrap_or_else(|e| unreachable!("fixture timestamp: {e}"));
        u64::try_from(parsed.as_millisecond()).unwrap_or_default()
    }
    fn rfc(instant: u64) -> String {
        Timestamp::from_millisecond(i64::try_from(instant).unwrap_or_default())
            .map(|ts| ts.to_string())
            .unwrap_or_default()
    }
    fn take(cron: &Cron, after: &str, count: usize) -> Vec<String> {
        cron.occurrences(ms(after)).take(count).map(rfc).collect()
    }

    #[test]
    fn exact_expressions_and_names_compile_unchanged() {
        let weekdays = cron("0 9 * * MON-FRI", "UTC", &["p", "s"]);
        assert_eq!(weekdays.resolved(), "0 9 * * MON-FRI");
        assert_eq!(
            take(&weekdays, "2026-10-30T09:00:00Z", 3),
            [
                "2026-11-02T09:00:00Z",
                "2026-11-03T09:00:00Z",
                "2026-11-04T09:00:00Z"
            ]
        );
        let stepped = cron("*/20 1-2 * JAN,feb *", "UTC", &[]);
        assert_eq!(
            take(&stepped, "2026-01-01T00:00:00Z", 4),
            [
                "2026-01-01T01:00:00Z",
                "2026-01-01T01:20:00Z",
                "2026-01-01T01:40:00Z",
                "2026-01-01T02:00:00Z"
            ]
        );
        // Vixie rule: restricted day of month and day of week combine with OR;
        // 7 is Sunday.
        let either = cron("0 0 13 * 7", "UTC", &[]);
        assert_eq!(
            take(&either, "2026-11-01T12:00:00Z", 3),
            [
                "2026-11-08T00:00:00Z",
                "2026-11-13T00:00:00Z",
                "2026-11-15T00:00:00Z"
            ]
        );
        // A leap-day entry is found beyond one year.
        let leap = cron("0 12 29 2 *", "UTC", &[]);
        assert_eq!(
            leap.next_after(ms("2026-03-01T00:00:00Z"))
                .map(rfc)
                .as_deref(),
            Some("2028-02-29T12:00:00Z")
        );
    }

    // r[verify kiln.schedules.cron]
    #[test]
    fn malformed_expressions_and_time_zones_are_rejected() {
        for bad in [
            "",
            "* * * *",
            "* * * * * *",
            "60 * * * *",
            "* 24 * * *",
            "* * 0 * *",
            "* * * 13 *",
            "* * * * 8",
            "5-1 * * * *",
            "*/0 * * * *",
            "*/61 * * * *",
            "H(5) * * * *",
            "H(9-1) * * * *",
            "L * * * *",
            "? * * * *",
            "1,,2 * * * *",
        ] {
            assert!(compile(&spec(bad, "UTC"), &[]).is_err(), "{bad}");
        }
        assert!(compile(&spec("0 0 * * *", "Mars/Olympus"), &[]).is_err());
        assert!(compile(&spec("0 0 * * *", "america/new_york"), &[]).is_err());
        assert!(compile(&spec("0 0 * * *", ""), &[]).is_err());
        assert!(compile(&spec("0 0 * * *", "America/New_York"), &[]).is_ok());
        let long = format!("{} * * * *", vec!["1"; 65].join(","));
        assert!(compile(&spec(&long, "UTC"), &[]).is_err());
        // Deserialization defaults the time zone and refuses unknown keys.
        let parsed: Result<CronSchedule, _> = serde_json::from_str(r#"{"expression":"0 0 * * *"}"#);
        assert!(parsed.is_ok_and(|spec| spec.timezone == "UTC"));
        assert!(
            serde_json::from_str::<CronSchedule>(r#"{"expression":"0 0 * * *","tz":"UTC"}"#)
                .is_err()
        );
    }

    // r[verify kiln.schedules.hash]
    // r[verify kiln.schedules.hash.scenario.restart]
    // r[verify kiln.schedules.hash.scenario.identity]
    #[test]
    fn hash_spreading_is_stable_and_follows_only_identity() {
        let expression = "H H(2-5) * * H";
        let first = cron(expression, "UTC", &["project", "nightly"]);
        // A fresh compilation (restart) resolves identically.
        let again = cron(expression, "UTC", &["project", "nightly"]);
        assert_eq!(first.resolved(), again.resolved());
        assert_eq!(
            take(&first, "2026-10-30T00:00:00Z", 5),
            take(&again, "2026-10-30T00:00:00Z", 5)
        );
        // Pinned values: BLAKE3 identity hashing is part of the contract, so
        // an upgrade can never silently move hashed schedules.
        assert_eq!(first.resolved(), "44 3 * * 5");
        let resolved: Vec<&str> = first.resolved().split(' ').collect();
        let hour: u8 = resolved.get(1).and_then(|v| v.parse().ok()).unwrap_or(99);
        assert!((2..=5).contains(&hour));
        // Changing the project or schedule identity recomputes the values.
        let identities = [
            ["project", "weekly"],
            ["other", "nightly"],
            ["projec", "tnightly"],
        ];
        let changed = identities
            .iter()
            .filter(|identity| {
                cron(expression, "UTC", identity.as_slice()).resolved() != first.resolved()
            })
            .count();
        assert!(changed >= 2, "distinct identities spread across values");
        // H/S keeps the step and hashes only the offset.
        let stepped = cron("H/15 * * * *", "UTC", &["project", "nightly"]);
        let minutes: Vec<u64> = stepped
            .occurrences(ms("2026-10-30T00:00:00Z"))
            .take(4)
            .collect();
        assert!(
            minutes
                .windows(2)
                .all(|pair| matches!(pair, [a, b] if b - a == 15 * 60_000))
        );
        // H in day of month stays within 1-28.
        for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            let monthly = cron("0 0 H * *", "UTC", &[name]);
            let day: u8 = monthly
                .resolved()
                .split(' ')
                .nth(2)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            assert!((1..=28).contains(&day), "{name}");
        }
    }

    // r[verify kiln.schedules.dst]
    // r[verify kiln.schedules.dst.scenario.new_york]
    #[test]
    fn new_york_transitions_fire_once_per_intended_occurrence() {
        // 2026-03-08: 02:00 EST jumps to 03:00 EDT; 02:30 does not exist.
        let spring = cron("30 2 * * *", "America/New_York", &[]);
        assert_eq!(
            take(&spring, "2026-03-06T12:00:00Z", 4),
            [
                "2026-03-07T07:30:00Z", // 02:30 EST
                "2026-03-08T07:30:00Z", // nonexistent 02:30 fires once at 03:30 EDT
                "2026-03-09T06:30:00Z", // 02:30 EDT
                "2026-03-10T06:30:00Z",
            ]
        );
        // Two civil times collapsing onto one instant fire once; a gap time
        // never hides a later real time on the same day.
        let collapse = cron("30 2,3 * * *", "America/New_York", &[]);
        assert_eq!(
            take(&collapse, "2026-03-08T05:00:00Z", 2),
            ["2026-03-08T07:30:00Z", "2026-03-09T06:30:00Z"]
        );
        // Lord Howe's 30-minute gap: the shifted 02:15 (02:45) must not hide
        // the real 02:30 that follows it in civil order.
        let lord_howe = cron("15,30 2 4 10 *", "Australia/Lord_Howe", &[]);
        assert_eq!(
            take(&lord_howe, "2026-10-01T00:00:00Z", 3),
            [
                "2026-10-03T15:30:00Z",
                "2026-10-03T15:45:00Z",
                "2027-10-03T15:15:00Z"
            ]
        );
        // 2026-11-01: 02:00 EDT falls back to 01:00 EST; 01:30 occurs twice.
        let fall = cron("30 1 * * *", "America/New_York", &[]);
        assert_eq!(
            take(&fall, "2026-10-30T12:00:00Z", 4),
            [
                "2026-10-31T05:30:00Z", // 01:30 EDT
                "2026-11-01T05:30:00Z", // first 01:30 (EDT) only
                "2026-11-02T06:30:00Z", // 01:30 EST
                "2026-11-03T06:30:00Z",
            ]
        );
        // Starting inside the repeated hour does not fire the second 01:30.
        assert_eq!(
            fall.next_after(ms("2026-11-01T06:10:00Z"))
                .map(rfc)
                .as_deref(),
            Some("2026-11-02T06:30:00Z")
        );
        // Hourly entries keep strictly increasing instants across both days.
        let hourly = cron("0 * * * *", "America/New_York", &[]);
        let spring_day: Vec<u64> = hourly
            .occurrences(ms("2026-03-08T04:30:00Z"))
            .take(4)
            .collect();
        assert!(
            spring_day
                .windows(2)
                .all(|pair| matches!(pair, [a, b] if b - a == 3_600_000))
        );
        let fall_day: Vec<u64> = hourly
            .occurrences(ms("2026-11-01T03:30:00Z"))
            .take(4)
            .collect();
        assert!(
            fall_day
                .windows(2)
                .all(|pair| matches!(pair, [a, b] if b > a))
        );
    }

    // r[verify kiln.schedules.min_interval]
    // r[verify kiln.schedules.min_interval.scenario.reject]
    #[test]
    fn minimum_interval_is_checked_across_the_horizon() {
        let start = ms("2026-10-30T00:00:00Z");
        let five = cron("*/5 * * * *", "UTC", &[]);
        assert!(five.check_min_interval(start, 300_000).is_ok());
        assert_eq!(
            five.check_min_interval(start, 600_000)
                .err()
                .map(|e| e.code),
            Some("schedule_min_interval")
        );
        // A list that is mostly spaced still fails on its closest pair.
        let close = cron("0,3,30 * * * *", "UTC", &[]);
        assert!(close.check_min_interval(start, 300_000).is_err());
        assert!(
            five.check_min_interval(start, 1_000).is_err(),
            "below the floor"
        );
        let never = cron("0 0 31 2 *", "UTC", &[]);
        assert_eq!(
            never
                .check_min_interval(start, 300_000)
                .err()
                .map(|e| e.code),
            Some("schedule_never_fires")
        );
        // DST does not create a spurious violation for a daily entry.
        let daily = cron("30 2 * * *", "America/New_York", &[]);
        assert!(daily.check_min_interval(start, 82_800_000).is_ok());
    }

    // r[verify kiln.schedules.no_catch_up]
    // r[verify kiln.schedules.no_catch_up.scenario.restart]
    #[test]
    fn reservation_fires_skips_missed_windows_and_rejects_close_runs() {
        let nightly = cron("0 2 * * *", "UTC", &[]);
        let due = ms("2026-10-30T02:00:00Z");
        let next = ms("2026-10-31T02:00:00Z");
        assert_eq!(
            nightly.decide(due, None, due - 1, 300_000),
            Ok(Reservation::NotDue)
        );
        assert_eq!(
            nightly.decide(due, None, due + 1_000, 300_000),
            Ok(Reservation::Fire {
                occurrence_ms: due,
                next_due_ms: next
            })
        );
        // A restart three days later neither fires the stale due occurrence
        // nor the ones missed in between; the next due is after the restart.
        let restart = ms("2026-11-02T08:00:00Z");
        assert_eq!(
            nightly.decide(due, None, restart, 300_000),
            Ok(Reservation::Missed {
                occurrence_ms: due,
                next_due_ms: ms("2026-11-03T02:00:00Z")
            })
        );
        // Runtime minimum interval: a previous run closer than the minimum.
        assert_eq!(
            nightly.decide(due, Some(due - 60_000), due, 300_000),
            Ok(Reservation::TooClose {
                occurrence_ms: due,
                previous_ms: due - 60_000,
                next_due_ms: next
            })
        );
    }
}
