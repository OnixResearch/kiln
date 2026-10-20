//! Verified calendar primitives.
//!
//! Leap year detection, days-in-month lookup, date validation, and
//! day-of-week computation (Sakamoto's algorithm). All operate on
//! decomposed (year, month, day) as u64. Month is 1-indexed
//! (1=January, 12=December). Day-of-week returns 0=Sunday through
//! 6=Saturday.

use vstd::prelude::*;

verus! {

// ============================================================
// is_leap_year
// ============================================================

// r[depends calendar.leap-year]
/// Spec: Gregorian leap year rule.
/// Divisible by 4 but not 100, OR divisible by 400.
pub open spec fn is_leap_year_spec(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// r[impl calendar.leap-year]
/// Returns true if `year` is a Gregorian leap year.
pub fn is_leap_year(year: u64) -> (result: bool)
    ensures result == is_leap_year_spec(year)
{
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ============================================================
// days_in_month
// ============================================================

// r[depends calendar.days-in-month]
/// Spec: number of days in the given month (1-indexed).
/// Returns 0 for invalid months (0 or >12).
pub open spec fn days_in_month_spec(year: u64, month: u64) -> u64 {
    if month == 1 { 31 }
    else if month == 2 {
        if is_leap_year_spec(year) { 29 } else { 28 }
    }
    else if month == 3 { 31 }
    else if month == 4 { 30 }
    else if month == 5 { 31 }
    else if month == 6 { 30 }
    else if month == 7 { 31 }
    else if month == 8 { 31 }
    else if month == 9 { 30 }
    else if month == 10 { 31 }
    else if month == 11 { 30 }
    else if month == 12 { 31 }
    else { 0 }
}

// r[impl calendar.days-in-month]
/// Returns the number of days in the given month.
/// Returns 0 for invalid months.
pub fn days_in_month(year: u64, month: u64) -> (result: u64)
    ensures result == days_in_month_spec(year, month)
{
    if month == 1 { 31 }
    else if month == 2 {
        if is_leap_year(year) { 29 } else { 28 }
    }
    else if month == 3 { 31 }
    else if month == 4 { 30 }
    else if month == 5 { 31 }
    else if month == 6 { 30 }
    else if month == 7 { 31 }
    else if month == 8 { 31 }
    else if month == 9 { 30 }
    else if month == 10 { 31 }
    else if month == 11 { 30 }
    else if month == 12 { 31 }
    else { 0 }
}

// ============================================================
// is_valid_date
// ============================================================

// r[depends calendar.date-validation]
/// Spec: month in [1, 12] and day in [1, days_in_month(year, month)].
pub open spec fn is_valid_date_spec(year: u64, month: u64, day: u64) -> bool {
    1 <= month && month <= 12
    && 1 <= day && day <= days_in_month_spec(year, month)
}

// r[impl calendar.date-validation]
/// Returns true if (year, month, day) is a valid Gregorian date.
pub fn is_valid_date(year: u64, month: u64, day: u64) -> (result: bool)
    ensures result == is_valid_date_spec(year, month, day)
{
    if month < 1 || month > 12 {
        return false;
    }
    if day < 1 {
        return false;
    }
    day <= days_in_month(year, month)
}

// ============================================================
// day_of_week — Sakamoto's algorithm
// ============================================================

// r[depends calendar.day-of-week]
/// Spec: month offset table for Sakamoto's algorithm.
/// Table: [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4] for months 1-12.
pub open spec fn month_offset_spec(month: u64) -> u64 {
    if month == 1 { 0 }
    else if month == 2 { 3 }
    else if month == 3 { 2 }
    else if month == 4 { 5 }
    else if month == 5 { 0 }
    else if month == 6 { 3 }
    else if month == 7 { 5 }
    else if month == 8 { 1 }
    else if month == 9 { 4 }
    else if month == 10 { 6 }
    else if month == 11 { 2 }
    else { 4 } // month == 12
}

// r[depends calendar.day-of-week]
/// Spec: adjusted year for Sakamoto's algorithm.
/// Subtract 1 for January and February.
pub open spec fn adjusted_year_spec(year: u64, month: u64) -> u64 {
    if month <= 2 {
        (year - 1) as u64
    } else {
        year
    }
}

// r[depends calendar.day-of-week]
/// Spec: day of week via Sakamoto's algorithm.
/// Returns 0=Sunday, 1=Monday, ..., 6=Saturday.
///
/// Formula: (y + y/4 - y/100 + y/400 + month_offset + day) % 7
/// where y is adjusted (year - 1 for Jan/Feb).
pub open spec fn day_of_week_spec(year: u64, month: u64, day: u64) -> u64 {
    let y: int = adjusted_year_spec(year, month) as int;
    let t: int = month_offset_spec(month) as int;
    let sum: int = y + y / 4 - y / 100 + y / 400 + t + day as int;
    (sum % 7) as u64
}

// r[impl calendar.day-of-week]
/// Returns the day of week for a valid date.
/// 0=Sunday, 1=Monday, ..., 6=Saturday.
///
/// Uses Sakamoto's algorithm. Requires year >= 1 (to avoid underflow
/// on the January/February adjustment).
///
/// Reduces each term modulo 7 to prevent overflow on large years.
pub fn day_of_week(year: u64, month: u64, day: u64) -> (result: u64)
    requires
        year >= 1,
        1 <= month && month <= 12,
        1 <= day && day <= 31,
    ensures result == day_of_week_spec(year, month, day)
{
    let y: u64 = if month <= 2 { year - 1 } else { year };

    let t: u64 = if month == 1 { 0 }
    else if month == 2 { 3 }
    else if month == 3 { 2 }
    else if month == 4 { 5 }
    else if month == 5 { 0 }
    else if month == 6 { 3 }
    else if month == 7 { 5 }
    else if month == 8 { 1 }
    else if month == 9 { 4 }
    else if month == 10 { 6 }
    else if month == 11 { 2 }
    else { 4 };

    // Reduce each term mod 7 to prevent overflow on large years.
    // (a + b) % 7 == ((a % 7) + (b % 7)) % 7
    // For subtraction: (a - b) % 7 == ((a % 7) + 7 - (b % 7)) % 7
    let positive: u64 = (y % 7) + ((y / 4) % 7) + ((y / 400) % 7) + t + day;
    let negative: u64 = (y / 100) % 7;

    // positive is at most 6 + 6 + 6 + 6 + 31 = 55, plus 7 = 62, fits in u64.
    proof {
        mod7_sum_lemma(y as int, t as int, day as int);
    }
    (positive + 7 - negative) % 7
}

// ============================================================
// Proofs
// ============================================================

// r[verify calendar.leap-year]
/// Year 2000 is a leap year (divisible by 400).
proof fn leap_year_2000()
    ensures is_leap_year_spec(2000)
{}

// r[verify calendar.leap-year]
/// Year 1900 is not a leap year (divisible by 100 but not 400).
proof fn leap_year_1900()
    ensures !is_leap_year_spec(1900)
{}

// r[verify calendar.leap-year]
/// Year 2024 is a leap year (divisible by 4, not by 100).
proof fn leap_year_2024()
    ensures is_leap_year_spec(2024)
{}

// r[verify calendar.leap-year]
/// Year 2023 is not a leap year (not divisible by 4).
proof fn leap_year_2023()
    ensures !is_leap_year_spec(2023)
{}

// r[verify calendar.days-in-month]
/// Days in month is always in [28, 31] for valid months.
proof fn days_in_month_bounded(year: u64, month: u64)
    requires 1 <= month && month <= 12
    ensures
        28 <= days_in_month_spec(year, month),
        days_in_month_spec(year, month) <= 31,
{}

// r[verify calendar.days-in-month]
/// February has 29 days iff leap year.
proof fn february_leap_day(year: u64)
    ensures
        is_leap_year_spec(year) ==> days_in_month_spec(year, 2) == 29,
        !is_leap_year_spec(year) ==> days_in_month_spec(year, 2) == 28,
{}

// r[verify calendar.date-validation]
/// Day 0 is never valid.
proof fn day_zero_invalid(year: u64, month: u64)
    ensures !is_valid_date_spec(year, month, 0)
{}

// r[verify calendar.date-validation]
/// Month 0 is never valid.
proof fn month_zero_invalid(year: u64, day: u64)
    ensures !is_valid_date_spec(year, 0, day)
{}

// r[verify calendar.date-validation]
/// Month 13 is never valid.
proof fn month_thirteen_invalid(year: u64, day: u64)
    ensures !is_valid_date_spec(year, 13, day)
{}

// r[verify calendar.date-validation]
/// Feb 29 is valid iff leap year.
proof fn feb_29_iff_leap(year: u64)
    ensures
        is_valid_date_spec(year, 2, 29) <==> is_leap_year_spec(year),
{}

/// Helper spec: the exec-side modular reduction formula.
/// Computes (positive + 7 - negative) % 7 where terms are reduced mod 7.
pub open spec fn day_of_week_mod7_spec(y: int, t: int, day: int) -> int {
    let positive: int = (y % 7) + ((y / 4) % 7) + ((y / 400) % 7) + t + day;
    let negative: int = (y / 100) % 7;
    (positive + 7 - negative) % 7
}

/// Helper: modular arithmetic distributes over the Sakamoto sum.
proof fn mod7_sum_lemma(y: int, t: int, day: int)
    requires
        y >= 0,
        0 <= t <= 6,
        1 <= day <= 31,
    ensures
        day_of_week_mod7_spec(y, t, day) == (y + y / 4 - y / 100 + y / 400 + t + day) % 7,
{
    let y4: int = y / 4;
    let y100: int = y / 100;
    let y400: int = y / 400;
    let sum: int = y + y4 - y100 + y400 + t + day;
    let positive: int = (y % 7) + (y4 % 7) + (y400 % 7) + t + day;
    let negative: int = y100 % 7;
    let reduced: int = positive + 7 - negative;

    // Division of non-negative by positive is non-negative
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 4);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 100);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 400);

    // reduced >= 0: positive >= 1 (day >= 1, all mod terms >= 0), negative <= 6
    assert(reduced >= 2) by(nonlinear_arith)
        requires
            y >= 0, y4 >= 0, y100 >= 0, y400 >= 0,
            0 <= t <= 6, 1 <= day <= 31,
            positive == (y % 7) + (y4 % 7) + (y400 % 7) + t + day,
            negative == y100 % 7,
            reduced == positive + 7 - negative,
    ;

    // The key modular identity
    assert(sum % 7 == reduced % 7) by(nonlinear_arith)
        requires
            y >= 0, y4 >= 0, y100 >= 0, y400 >= 0,
            y4 == y / 4, y100 == y / 100, y400 == y / 400,
            0 <= t <= 6, 1 <= day <= 31,
            reduced == (y % 7) + (y4 % 7) + (y400 % 7) + t + day + 7 - (y100 % 7),
            sum == y + y4 - y100 + y400 + t + day,
    ;
}

// r[verify calendar.day-of-week]
/// Day of week result is always in [0, 6].
proof fn day_of_week_bounded(year: u64, month: u64, day: u64)
    requires
        year >= 1,
        1 <= month && month <= 12,
        1 <= day && day <= 31,
    ensures
        day_of_week_spec(year, month, day) <= 6
{
    let y: int = adjusted_year_spec(year, month) as int;
    let t: int = month_offset_spec(month) as int;
    let sum: int = y + y / 4 - y / 100 + y / 400 + t + day as int;
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 4);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 100);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(y, 400);
    // y/4 >= y/100 because 4 < 100
    vstd::arithmetic::div_mod::lemma_div_is_ordered(y, y, 4);
    assert(sum >= 0) by(nonlinear_arith)
        requires
            y >= 0,
            0 <= t <= 6,
            day >= 1,
            y / 4 >= 0,
            y / 100 >= 0,
            y / 400 >= 0,
            sum == y + y / 4 - y / 100 + y / 400 + t + day as int,
    ;
    assert(sum % 7 >= 0 && sum % 7 < 7) by(nonlinear_arith)
        requires sum >= 0
    ;
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify calendar.leap-year]
    #[test]
    fn test_leap_years() {
        // Divisible by 4, not 100
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2028));
        assert!(is_leap_year(1996));
        // Divisible by 100, not 400
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(1800));
        assert!(!is_leap_year(2100));
        // Divisible by 400
        assert!(is_leap_year(2000));
        assert!(is_leap_year(1600));
        assert!(is_leap_year(2400));
        // Not divisible by 4
        assert!(!is_leap_year(2023));
        assert!(!is_leap_year(2025));
        assert!(!is_leap_year(2027));
    }

    // r[verify calendar.days-in-month]
    #[test]
    fn test_days_in_month() {
        // 31-day months
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 3), 31);
        assert_eq!(days_in_month(2026, 5), 31);
        assert_eq!(days_in_month(2026, 7), 31);
        assert_eq!(days_in_month(2026, 8), 31);
        assert_eq!(days_in_month(2026, 10), 31);
        assert_eq!(days_in_month(2026, 12), 31);
        // 30-day months
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 6), 30);
        assert_eq!(days_in_month(2026, 9), 30);
        assert_eq!(days_in_month(2026, 11), 30);
        // February
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        // Invalid months
        assert_eq!(days_in_month(2026, 0), 0);
        assert_eq!(days_in_month(2026, 13), 0);
    }

    // r[verify calendar.date-validation]
    #[test]
    fn test_valid_date() {
        assert!(is_valid_date(2026, 4, 4));
        assert!(is_valid_date(2024, 2, 29));
        assert!(is_valid_date(2026, 1, 31));
        assert!(is_valid_date(2026, 12, 31));
        // Invalid
        assert!(!is_valid_date(2026, 1, 0));
        assert!(!is_valid_date(2023, 2, 29));
        assert!(!is_valid_date(2026, 13, 1));
        assert!(!is_valid_date(2026, 0, 1));
        assert!(!is_valid_date(2026, 4, 31));
        assert!(!is_valid_date(2026, 2, 30));
    }

    // r[verify calendar.day-of-week]
    #[test]
    fn test_day_of_week() {
        // 2026-04-01 is Wednesday (3)
        assert_eq!(day_of_week(2026, 4, 1), 3);
        // 2026-04-04 is Saturday (6)
        assert_eq!(day_of_week(2026, 4, 4), 6);
        // 2024-02-29 is Thursday (4)
        assert_eq!(day_of_week(2024, 2, 29), 4);
        // 2000-01-01 is Saturday (6)
        assert_eq!(day_of_week(2000, 1, 1), 6);
        // 1970-01-01 is Thursday (4)
        assert_eq!(day_of_week(1970, 1, 1), 4);
        // 2026-01-01 is Thursday (4)
        assert_eq!(day_of_week(2026, 1, 1), 4);
    }

    proptest! {
        // r[verify calendar.leap-year]
        #[test]
        fn prop_leap_year_consistency(year in 1u64..10000) {
            let leap = is_leap_year(year);
            let by_rule = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            prop_assert_eq!(leap, by_rule);
        }

        // r[verify calendar.days-in-month]
        #[test]
        fn prop_days_in_month_bounded(year in 1u64..10000, month in 1u64..=12) {
            let d = days_in_month(year, month);
            prop_assert!(d >= 28);
            prop_assert!(d <= 31);
        }

        // r[verify calendar.days-in-month]
        #[test]
        fn prop_february_leap_day(year in 1u64..10000) {
            let d = days_in_month(year, 2);
            if is_leap_year(year) {
                prop_assert_eq!(d, 29);
            } else {
                prop_assert_eq!(d, 28);
            }
        }

        // r[verify calendar.date-validation]
        #[test]
        fn prop_day_one_always_valid(year in 1u64..10000, month in 1u64..=12) {
            prop_assert!(is_valid_date(year, month, 1));
        }

        // r[verify calendar.date-validation]
        #[test]
        fn prop_max_day_valid(year in 1u64..10000, month in 1u64..=12) {
            let d = days_in_month(year, month);
            prop_assert!(is_valid_date(year, month, d));
            prop_assert!(!is_valid_date(year, month, d + 1));
        }

        // r[verify calendar.day-of-week]
        #[test]
        fn prop_day_of_week_bounded(year in 1u64..10000, month in 1u64..=12, day in 1u64..=31) {
            let dow = day_of_week(year, month, day);
            prop_assert!(dow <= 6);
        }

        // r[verify calendar.day-of-week]
        #[test]
        fn prop_next_day_advances_weekday(year in 1u64..9999, month in 1u64..=12, day in 1u64..=30) {
            // Only test when both day and day+1 are valid in the same month
            if is_valid_date(year, month, day) && is_valid_date(year, month, day + 1) {
                let dow1 = day_of_week(year, month, day);
                let dow2 = day_of_week(year, month, day + 1);
                prop_assert_eq!(dow2, (dow1 + 1) % 7);
            }
        }
    }
}
