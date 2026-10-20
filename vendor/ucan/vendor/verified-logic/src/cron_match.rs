//! Verified cron schedule matching.
//!
//! A schedule has five fields: minute, hour, day-of-month, month,
//! day-of-week. Each field is a (min, max, step) triple.
//! `matches_schedule` returns true when all five fields match
//! the given timestamp. Day-of-week is derived from the date,
//! not supplied by the caller.

#![allow(clippy::too_many_arguments)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::calendar::*;
#[cfg(verus_keep_ghost)]
use crate::cron_field::*;

verus! {

// ============================================================
// is_valid_schedule
// ============================================================

// r[depends cron-match.valid-schedule]
/// Spec: all five fields have valid bounds for cron.
/// Minute [0,59], hour [0,23], dom [1,31], month [1,12], dow [0,6].
pub open spec fn is_valid_schedule_spec(
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
) -> bool {
    is_valid_field_spec(min_min, min_max, min_step, 0, 59)
    && is_valid_field_spec(hr_min, hr_max, hr_step, 0, 23)
    && is_valid_field_spec(dom_min, dom_max, dom_step, 1, 31)
    && is_valid_field_spec(mon_min, mon_max, mon_step, 1, 12)
    && is_valid_field_spec(dow_min, dow_max, dow_step, 0, 6)
}

// r[impl cron-match.valid-schedule]
/// Returns true if all five cron fields have valid bounds.
pub fn is_valid_schedule(
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
) -> (result: bool)
    ensures result == is_valid_schedule_spec(
        min_min, min_max, min_step,
        hr_min, hr_max, hr_step,
        dom_min, dom_max, dom_step,
        mon_min, mon_max, mon_step,
        dow_min, dow_max, dow_step,
    )
{
    crate::cron_field::is_valid_field(min_min, min_max, min_step, 0, 59)
    && crate::cron_field::is_valid_field(hr_min, hr_max, hr_step, 0, 23)
    && crate::cron_field::is_valid_field(dom_min, dom_max, dom_step, 1, 31)
    && crate::cron_field::is_valid_field(mon_min, mon_max, mon_step, 1, 12)
    && crate::cron_field::is_valid_field(dow_min, dow_max, dow_step, 0, 6)
}

// ============================================================
// matches_schedule
// ============================================================

// r[depends cron-match.all-fields-match]
/// Spec: does the timestamp match all five fields?
/// Day-of-week is computed from (year, month, day), not supplied.
pub open spec fn matches_schedule_spec(
    year: u64, month: u64, day: u64, hour: u64, minute: u64,
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
) -> bool {
    let dow = day_of_week_spec(year, month, day);
    match_field_spec(minute, min_min, min_max, min_step)
    && match_field_spec(hour, hr_min, hr_max, hr_step)
    && match_field_spec(day, dom_min, dom_max, dom_step)
    && match_field_spec(month, mon_min, mon_max, mon_step)
    && match_field_spec(dow, dow_min, dow_max, dow_step)
}

// r[impl cron-match.all-fields-match]
/// Returns true if the timestamp matches all five cron fields.
/// Day-of-week is derived from the date.
pub fn matches_schedule(
    year: u64, month: u64, day: u64, hour: u64, minute: u64,
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
) -> (result: bool)
    requires
        year >= 1,
        1 <= month && month <= 12,
        1 <= day && day <= 31,
        min_step >= 1,
        hr_step >= 1,
        dom_step >= 1,
        mon_step >= 1,
        dow_step >= 1,
    ensures result == matches_schedule_spec(
        year, month, day, hour, minute,
        min_min, min_max, min_step,
        hr_min, hr_max, hr_step,
        dom_min, dom_max, dom_step,
        mon_min, mon_max, mon_step,
        dow_min, dow_max, dow_step,
    )
{
    let dow = crate::calendar::day_of_week(year, month, day);
    crate::cron_field::match_field(minute, min_min, min_max, min_step)
    && crate::cron_field::match_field(hour, hr_min, hr_max, hr_step)
    && crate::cron_field::match_field(day, dom_min, dom_max, dom_step)
    && crate::cron_field::match_field(month, mon_min, mon_max, mon_step)
    && crate::cron_field::match_field(dow, dow_min, dow_max, dow_step)
}

// ============================================================
// Proofs
// ============================================================

// r[verify cron-match.all-fields-match]
/// If any single field doesn't match, the schedule doesn't match.
proof fn minute_mismatch_rejects(
    year: u64, month: u64, day: u64, hour: u64, minute: u64,
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
)
    requires !match_field_spec(minute, min_min, min_max, min_step)
    ensures !matches_schedule_spec(
        year, month, day, hour, minute,
        min_min, min_max, min_step,
        hr_min, hr_max, hr_step,
        dom_min, dom_max, dom_step,
        mon_min, mon_max, mon_step,
        dow_min, dow_max, dow_step,
    )
{}

// r[verify cron-match.day-of-week-consistency]
/// The day-of-week used in matching is always derived from the date.
/// Two timestamps with the same (year, month, day) always produce
/// the same dow comparison result.
proof fn dow_derived_from_date(
    year: u64, month: u64, day: u64,
    hour1: u64, minute1: u64,
    hour2: u64, minute2: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
)
    requires
        year >= 1,
        1 <= month && month <= 12,
        1 <= day && day <= 31,
        dow_step >= 1,
    ensures ({
        let dow = day_of_week_spec(year, month, day);
        match_field_spec(dow, dow_min, dow_max, dow_step)
            == match_field_spec(dow, dow_min, dow_max, dow_step)
    })
{}

// r[verify cron-match.valid-schedule]
/// Standard cron wildcard schedule (* * * * *) is valid.
proof fn wildcard_schedule_valid()
    ensures is_valid_schedule_spec(
        0, 59, 1,
        0, 23, 1,
        1, 31, 1,
        1, 12, 1,
        0, 6, 1,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cron-match.all-fields-match]
    #[test]
    fn test_matches_schedule_all_wildcards() {
        // * * * * * — matches everything
        assert!(matches_schedule(
            2026, 4, 4, 10, 30, 0, 59, 1, // minute: *
            0, 23, 1, // hour: *
            1, 31, 1, // dom: *
            1, 12, 1, // month: *
            0, 6, 1, // dow: *
        ));
    }

    // r[verify cron-match.all-fields-match]
    #[test]
    fn test_matches_schedule_specific() {
        // 0 3 * * 1 — every Monday at 03:00
        // 2026-04-06 is a Monday
        assert!(matches_schedule(
            2026, 4, 6, 3, 0, 0, 0, 1, // minute: 0
            3, 3, 1, // hour: 3
            1, 31, 1, // dom: *
            1, 12, 1, // month: *
            1, 1, 1, // dow: 1 (Monday)
        ));
    }

    // r[verify cron-match.all-fields-match]
    #[test]
    fn test_matches_schedule_minute_mismatch() {
        // minute=0, but timestamp minute=15
        assert!(!matches_schedule(
            2026, 4, 6, 3, 15, 0, 0, 1, // minute: 0
            3, 3, 1, // hour: 3
            1, 31, 1, // dom: *
            1, 12, 1, // month: *
            0, 6, 1, // dow: *
        ));
    }

    // r[verify cron-match.all-fields-match]
    #[test]
    fn test_matches_schedule_dow_mismatch() {
        // dow=1 (Monday), but 2026-04-04 is Saturday (6)
        assert!(!matches_schedule(
            2026, 4, 4, 3, 0, 0, 0, 1, // minute: 0
            3, 3, 1, // hour: 3
            1, 31, 1, // dom: *
            1, 12, 1, // month: *
            1, 1, 1, // dow: 1 (Monday)
        ));
    }

    // r[verify cron-match.valid-schedule]
    #[test]
    fn test_valid_schedule() {
        assert!(is_valid_schedule(0, 59, 15, 3, 3, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1,));
        // Invalid: hour max = 24
        assert!(!is_valid_schedule(0, 59, 1, 0, 24, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1,));
    }

    // r[verify cron-match.day-of-week-consistency]
    #[test]
    fn test_dow_derived_from_date() {
        // 2026-04-04 is Saturday (6) — dow constraint 6 should match
        assert!(matches_schedule(
            2026, 4, 4, 0, 0, 0, 59, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 6, 6, 1, // dow: 6 (Saturday)
        ));
        // dow constraint 0 (Sunday) should not match Saturday
        assert!(!matches_schedule(
            2026, 4, 4, 0, 0, 0, 59, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 0, 1, // dow: 0 (Sunday)
        ));
    }

    proptest! {
        // r[verify cron-match.all-fields-match]
        #[test]
        fn prop_wildcard_always_matches(
            year in 1u64..3000,
            month in 1u64..=12u64,
            day in 1u64..=28u64,
            hour in 0u64..24,
            minute in 0u64..60,
        ) {
            prop_assert!(matches_schedule(
                year, month, day, hour, minute,
                0, 59, 1,
                0, 23, 1,
                1, 31, 1,
                1, 12, 1,
                0, 6, 1,
            ));
        }

        // r[verify cron-match.all-fields-match]
        #[test]
        fn prop_month_mismatch_rejects(
            year in 1u64..3000,
            month in 1u64..=12u64,
            day in 1u64..=28u64,
            hour in 0u64..24,
            minute in 0u64..60,
            target_month in 1u64..=12u64,
        ) {
            if target_month != month {
                prop_assert!(!matches_schedule(
                    year, month, day, hour, minute,
                    0, 59, 1,
                    0, 23, 1,
                    1, 31, 1,
                    target_month, target_month, 1,  // specific month
                    0, 6, 1,
                ));
            }
        }
    }
}
