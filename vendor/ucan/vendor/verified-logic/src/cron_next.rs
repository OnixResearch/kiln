//! Verified cron next-occurrence search.
//!
//! Given a cron schedule and a current timestamp, finds the earliest
//! future timestamp matching the schedule. Uses field-skipping to
//! avoid scanning all 525,600 minutes in a year. Bounded by max_years
//! to guarantee termination.

#![allow(clippy::too_many_arguments, clippy::too_many_lines)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::calendar::*;
#[cfg(verus_keep_ghost)]
use crate::cron_field::*;
#[cfg(verus_keep_ghost)]
use crate::cron_match::*;

verus! {

// ============================================================
// timestamp_less_than — strict ordering on decomposed timestamps
// ============================================================

// r[depends cron-next.result-after-input]
/// Spec: is timestamp A strictly before timestamp B?
/// Lexicographic comparison: year, month, day, hour, minute.
pub open spec fn timestamp_lt_spec(
    y1: u64, mo1: u64, d1: u64, h1: u64, m1: u64,
    y2: u64, mo2: u64, d2: u64, h2: u64, m2: u64,
) -> bool {
    if y1 != y2 { y1 < y2 }
    else if mo1 != mo2 { mo1 < mo2 }
    else if d1 != d2 { d1 < d2 }
    else if h1 != h2 { h1 < h2 }
    else { m1 < m2 }
}

// r[impl cron-next.result-after-input]
/// Returns true if timestamp A is strictly before timestamp B.
pub fn timestamp_lt(
    y1: u64, mo1: u64, d1: u64, h1: u64, m1: u64,
    y2: u64, mo2: u64, d2: u64, h2: u64, m2: u64,
) -> (result: bool)
    ensures result == timestamp_lt_spec(y1, mo1, d1, h1, m1, y2, mo2, d2, h2, m2)
{
    if y1 != y2 { y1 < y2 }
    else if mo1 != mo2 { mo1 < mo2 }
    else if d1 != d2 { d1 < d2 }
    else if h1 != h2 { h1 < h2 }
    else { m1 < m2 }
}

// ============================================================
// next_occurrence — bounded forward search
// ============================================================

// r[depends cron-next.bounded-search]
// r[depends cron-next.result-matches]
// r[depends cron-next.result-after-input]
// r[depends cron-next.month-rollover]
// r[depends cron-next.leap-year-awareness]
/// Spec: result of next_occurrence.
/// Returns (found, year, month, day, hour, minute).
/// If found is false, no matching time exists within the year horizon.
///
/// This spec is opaque — the algorithm is verified through its postconditions
/// (result after input, result matches schedule) rather than through a
/// recursive spec definition.
pub open spec fn next_occurrence_succeeds_spec(
    found: bool,
    ry: u64, rmo: u64, rd: u64, rh: u64, rm: u64,
    iy: u64, imo: u64, id: u64, ih: u64, im: u64,
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
) -> bool {
    found ==> (
        timestamp_lt_spec(iy, imo, id, ih, im, ry, rmo, rd, rh, rm)
        && matches_schedule_spec(
            ry, rmo, rd, rh, rm,
            min_min, min_max, min_step,
            hr_min, hr_max, hr_step,
            dom_min, dom_max, dom_step,
            mon_min, mon_max, mon_step,
            dow_min, dow_max, dow_step,
        )
        && is_valid_date_spec(ry, rmo, rd)
    )
}

// r[impl cron-next.bounded-search]
// r[impl cron-next.result-matches]
// r[impl cron-next.result-after-input]
// r[impl cron-next.month-rollover]
// r[impl cron-next.leap-year-awareness]
/// Find the next timestamp after (iy, imo, id, ih, im) matching the schedule.
///
/// Returns (found, year, month, day, hour, minute).
/// Searches up to max_years into the future.
///
/// Uses field-skipping: jumps to the first matching month, then day,
/// then hour, then minute within each enclosing unit.
pub fn next_occurrence(
    iy: u64, imo: u64, id: u64, ih: u64, im: u64,
    min_min: u64, min_max: u64, min_step: u64,
    hr_min: u64, hr_max: u64, hr_step: u64,
    dom_min: u64, dom_max: u64, dom_step: u64,
    mon_min: u64, mon_max: u64, mon_step: u64,
    dow_min: u64, dow_max: u64, dow_step: u64,
    max_years: u64,
) -> (result: (bool, u64, u64, u64, u64, u64))
    requires
        iy >= 1,
        1 <= imo && imo <= 12,
        1 <= id && id <= 31,
        ih <= 23,
        im <= 59,
        min_step >= 1, hr_step >= 1, dom_step >= 1, mon_step >= 1, dow_step >= 1,
        min_min <= min_max && min_max <= 59,
        hr_min <= hr_max && hr_max <= 23,
        dom_min <= dom_max && dom_max <= 31 && dom_min >= 1,
        mon_min <= mon_max && mon_max <= 12 && mon_min >= 1,
        dow_min <= dow_max && dow_max <= 6,
        max_years >= 1,
        (iy as int) + (max_years as int) <= u64::MAX as int,
    ensures ({
        let (found, ry, rmo, rd, rh, rm) = result;
        next_occurrence_succeeds_spec(
            found, ry, rmo, rd, rh, rm,
            iy, imo, id, ih, im,
            min_min, min_max, min_step,
            hr_min, hr_max, hr_step,
            dom_min, dom_max, dom_step,
            mon_min, mon_max, mon_step,
            dow_min, dow_max, dow_step,
        )
    })
{
    let max_year = iy + max_years;

    // Start scanning from the next minute after the input.
    let (mut cy, mut cmo, mut cd, mut ch, mut cm) = advance_one_minute(iy, imo, id, ih, im);

    while cy <= max_year
        invariant
            iy >= 1,
            1 <= imo && imo <= 12,
            1 <= id && id <= 31,
            ih <= 23,
            im <= 59,
            min_step >= 1, hr_step >= 1, dom_step >= 1, mon_step >= 1, dow_step >= 1,
            min_min <= min_max && min_max <= 59,
            hr_min <= hr_max && hr_max <= 23,
            dom_min <= dom_max && dom_max <= 31 && dom_min >= 1,
            mon_min <= mon_max && mon_max <= 12 && mon_min >= 1,
            dow_min <= dow_max && dow_max <= 6,
            max_years >= 1,
            max_year == iy + max_years,
            cy >= 1,
            1 <= cmo && cmo <= 12,
            1 <= cd && cd <= 31,
            ch <= 23,
            cm <= 59,
            timestamp_lt_spec(iy, imo, id, ih, im, cy, cmo, cd, ch, cm),
        decreases
            (if max_year >= cy { (max_year - cy) as int } else { 0 }),
            (12 - cmo) as int,
            (31 - cd) as int,
            (23 - ch) as int,
            (59 - cm) as int,
    {
        // Skip to first matching month.
        proof {
            crate::cron_field::first_match_correct(cmo, mon_min, mon_max, mon_step);
        }
        let fm = crate::cron_field::first_match(cmo, mon_min, mon_max, mon_step);
        if fm > mon_max {
            // No matching month this year — advance to next year.
            if cy >= max_year {
                return (false, 0, 0, 0, 0, 0);
            }
            let next_year: u64 = cy + 1u64;
            proof {
                assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
            }
            cy = cy + 1u64;
            cmo = 1;
            cd = 1;
            ch = 0;
            cm = 0;
            continue;
        }
        proof {
            crate::cron_field::first_match_ge_from(cmo, mon_min, mon_max, mon_step);
        }
        if fm > cmo {
            proof {
                assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, fm, 1, 0, 0));
            }
            cmo = fm;
            cd = 1;
            ch = 0;
            cm = 0;
        } else {
            proof {
                assert(fm == cmo);
            }
        }
        proof {
            assert(match_field_spec(cmo, mon_min, mon_max, mon_step));
        }

        // How many days in this month?
        let dim = crate::calendar::days_in_month(cy, cmo);
        if cd > dim {
            // Invalid day for this month — advance to next month.
            if cmo >= 12 {
                if cy >= max_year {
                    return (false, 0, 0, 0, 0, 0);
                }
                let next_year: u64 = cy + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                }
                cy = cy + 1u64;
                cmo = 1;
            } else {
                let next_month: u64 = cmo + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                }
                cmo = cmo + 1u64;
            }
            cd = 1;
            ch = 0;
            cm = 0;
            continue;
        }
        proof {
            assert(is_valid_date_spec(cy, cmo, cd));
        }

        // Skip to first matching day.
        // Clamp dom_max to actual days in this month.
        let max_dom = if dom_max < dim { dom_max } else { dim };
        if dom_min > max_dom {
            // Schedule requires a day this month doesn't have
            // (e.g., dom=29 but Feb has 28 days).
            if cmo >= 12 {
                if cy >= max_year {
                    return (false, 0, 0, 0, 0, 0);
                }
                let next_year: u64 = cy + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                }
                cy = cy + 1u64;
                cmo = 1;
            } else {
                let next_month: u64 = cmo + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                }
                cmo = cmo + 1u64;
            }
            cd = 1;
            ch = 0;
            cm = 0;
            continue;
        }
        proof {
            crate::cron_field::first_match_correct(cd, dom_min, max_dom, dom_step);
        }
        let fd = crate::cron_field::first_match(cd, dom_min, max_dom, dom_step);
        if fd > max_dom {
            // No matching day this month — advance to next month.
            if cmo >= 12 {
                if cy >= max_year {
                    return (false, 0, 0, 0, 0, 0);
                }
                let next_year: u64 = cy + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                }
                cy = cy + 1u64;
                cmo = 1;
            } else {
                let next_month: u64 = cmo + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                }
                cmo = cmo + 1u64;
            }
            cd = 1;
            ch = 0;
            cm = 0;
            continue;
        }
        proof {
            crate::cron_field::first_match_ge_from(cd, dom_min, max_dom, dom_step);
        }
        if fd > cd {
            proof {
                assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, fd, 0, 0));
            }
            cd = fd;
            ch = 0;
            cm = 0;
        } else {
            proof {
                assert(fd == cd);
            }
        }
        proof {
            assert(match_field_spec(cd, dom_min, max_dom, dom_step));
            assert(is_valid_date_spec(cy, cmo, cd));
        }

        // Check day-of-week.
        let dow = crate::calendar::day_of_week(cy, cmo, cd);
        if !crate::cron_field::match_field(dow, dow_min, dow_max, dow_step) {
            // Advance to next day.
            if cd >= dim {
                if cmo >= 12 {
                    if cy >= max_year {
                        return (false, 0, 0, 0, 0, 0);
                    }
                    let next_year: u64 = cy + 1u64;
                    proof {
                        assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                    }
                    cy = cy + 1u64;
                    cmo = 1;
                } else {
                    let next_month: u64 = cmo + 1u64;
                    proof {
                        assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                    }
                    cmo = cmo + 1u64;
                }
                cd = 1;
            } else {
                let next_day: u64 = cd + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, next_day, 0, 0));
                }
                cd = cd + 1u64;
            }
            ch = 0;
            cm = 0;
            continue;
        }
        proof {
            assert(match_field_spec(dow, dow_min, dow_max, dow_step));
        }

        // Skip to first matching hour.
        proof {
            crate::cron_field::first_match_correct(ch, hr_min, hr_max, hr_step);
        }
        let fh = crate::cron_field::first_match(ch, hr_min, hr_max, hr_step);
        if fh > hr_max {
            // No matching hour today — advance to next day.
            if cd >= dim {
                if cmo >= 12 {
                    if cy >= max_year {
                        return (false, 0, 0, 0, 0, 0);
                    }
                    let next_year: u64 = cy + 1u64;
                    proof {
                        assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                    }
                    cy = cy + 1u64;
                    cmo = 1;
                } else {
                    let next_month: u64 = cmo + 1u64;
                    proof {
                        assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                    }
                    cmo = cmo + 1u64;
                }
                cd = 1;
                ch = 0;
            } else {
                let next_day: u64 = cd + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, next_day, 0, 0));
                }
                cd = cd + 1u64;
                ch = 0;
            }
            cm = 0;
            continue;
        }
        proof {
            crate::cron_field::first_match_ge_from(ch, hr_min, hr_max, hr_step);
        }
        if fh > ch {
            proof {
                assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, cd, fh, 0));
            }
            ch = fh;
            cm = 0;
        } else {
            proof {
                assert(fh == ch);
            }
        }
        proof {
            assert(match_field_spec(ch, hr_min, hr_max, hr_step));
        }

        // Skip to first matching minute.
        proof {
            crate::cron_field::first_match_correct(cm, min_min, min_max, min_step);
        }
        let fmin = crate::cron_field::first_match(cm, min_min, min_max, min_step);
        if fmin > min_max {
            // No matching minute this hour — advance to next hour.
            if ch >= hr_max {
                // No more hours today — next day.
                if cd >= dim {
                    if cmo >= 12 {
                        if cy >= max_year {
                            return (false, 0, 0, 0, 0, 0);
                        }
                        let next_year: u64 = cy + 1u64;
                        proof {
                            assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, next_year, 1, 1, 0, 0));
                        }
                        cy = cy + 1u64;
                        cmo = 1;
                    } else {
                        let next_month: u64 = cmo + 1u64;
                        proof {
                            assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, next_month, 1, 0, 0));
                        }
                        cmo = cmo + 1u64;
                    }
                    cd = 1;
                } else {
                    let next_day: u64 = cd + 1u64;
                    proof {
                        assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, next_day, 0, 0));
                    }
                    cd = cd + 1u64;
                }
                ch = 0;
            } else {
                let next_hour: u64 = ch + 1u64;
                proof {
                    assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, cd, next_hour, 0));
                }
                ch = ch + 1u64;
            }
            cm = 0;
            continue;
        }
        proof {
            crate::cron_field::first_match_ge_from(cm, min_min, min_max, min_step);
            if fmin > cm {
                assert(timestamp_lt_spec(cy, cmo, cd, ch, cm, cy, cmo, cd, ch, fmin));
            } else {
                assert(fmin == cm);
            }
            assert(match_field_spec(fmin, min_min, min_max, min_step));
            assert(matches_schedule_spec(
                cy, cmo, cd, ch, fmin,
                min_min, min_max, min_step,
                hr_min, hr_max, hr_step,
                dom_min, dom_max, dom_step,
                mon_min, mon_max, mon_step,
                dow_min, dow_max, dow_step,
            ));
            assert(next_occurrence_succeeds_spec(
                true, cy, cmo, cd, ch, fmin,
                iy, imo, id, ih, im,
                min_min, min_max, min_step,
                hr_min, hr_max, hr_step,
                dom_min, dom_max, dom_step,
                mon_min, mon_max, mon_step,
                dow_min, dow_max, dow_step,
            ));
        }

        // All five fields match!
        return (true, cy, cmo, cd, ch, fmin);
    }

    (false, 0, 0, 0, 0, 0)
}

/// Advance a timestamp by one minute, handling rollover.
fn advance_one_minute(y: u64, mo: u64, d: u64, h: u64, m: u64) -> (result: (u64, u64, u64, u64, u64))
    requires
        y >= 1,
        y < u64::MAX,
        1 <= mo && mo <= 12,
        1 <= d && d <= 31,
        h <= 23,
        m <= 59,
    ensures ({
        let (ny, nmo, nd, nh, nm) = result;
        &&& ny >= 1
        &&& 1 <= nmo <= 12
        &&& 1 <= nd <= 31
        &&& nh <= 23
        &&& nm <= 59
        &&& timestamp_lt_spec(y, mo, d, h, m, ny, nmo, nd, nh, nm)
    })
{
    if m < 59 {
        return (y, mo, d, h, m + 1);
    }
    // m == 59, roll hour
    if h < 23 {
        return (y, mo, d, h + 1, 0);
    }
    // h == 23, roll day
    let dim = crate::calendar::days_in_month(y, mo);
    if d < dim {
        return (y, mo, d + 1, 0, 0);
    }
    // Roll month
    if mo < 12 {
        return (y, mo + 1, 1, 0, 0);
    }
    // Roll year
    (y + 1, 1, 1, 0, 0)
}

// ============================================================
// Proofs
// ============================================================

// r[verify cron-next.result-after-input]
/// timestamp_lt is strict (irreflexive).
proof fn timestamp_lt_irreflexive(y: u64, mo: u64, d: u64, h: u64, m: u64)
    ensures !timestamp_lt_spec(y, mo, d, h, m, y, mo, d, h, m)
{}

// r[verify cron-next.result-after-input]
/// timestamp_lt is transitive.
proof fn timestamp_lt_transitive(
    y1: u64, mo1: u64, d1: u64, h1: u64, m1: u64,
    y2: u64, mo2: u64, d2: u64, h2: u64, m2: u64,
    y3: u64, mo3: u64, d3: u64, h3: u64, m3: u64,
)
    requires
        timestamp_lt_spec(y1, mo1, d1, h1, m1, y2, mo2, d2, h2, m2),
        timestamp_lt_spec(y2, mo2, d2, h2, m2, y3, mo3, d3, h3, m3),
    ensures
        timestamp_lt_spec(y1, mo1, d1, h1, m1, y3, mo3, d3, h3, m3)
{}

// r[verify cron-next.result-after-input]
/// Year advance means strictly later.
proof fn year_advance_is_later(
    y1: u64, mo1: u64, d1: u64, h1: u64, m1: u64,
    y2: u64, mo2: u64, d2: u64, h2: u64, m2: u64,
)
    requires y1 < y2
    ensures timestamp_lt_spec(y1, mo1, d1, h1, m1, y2, mo2, d2, h2, m2)
{}

// r[verify cron-next.result-after-input]
proof fn month_advance_is_later(
    y: u64, mo1: u64, d1: u64, h1: u64, m1: u64,
    mo2: u64, d2: u64, h2: u64, m2: u64,
)
    requires mo1 < mo2
    ensures timestamp_lt_spec(y, mo1, d1, h1, m1, y, mo2, d2, h2, m2)
{}

// r[verify cron-next.result-after-input]
proof fn day_advance_is_later(
    y: u64, mo: u64, d1: u64, h1: u64, m1: u64,
    d2: u64, h2: u64, m2: u64,
)
    requires d1 < d2
    ensures timestamp_lt_spec(y, mo, d1, h1, m1, y, mo, d2, h2, m2)
{}

// r[verify cron-next.result-after-input]
proof fn hour_advance_is_later(
    y: u64, mo: u64, d: u64, h1: u64, m1: u64,
    h2: u64, m2: u64,
)
    requires h1 < h2
    ensures timestamp_lt_spec(y, mo, d, h1, m1, y, mo, d, h2, m2)
{}

// r[verify cron-next.result-after-input]
proof fn minute_advance_is_later(
    y: u64, mo: u64, d: u64, h: u64, m1: u64,
    m2: u64,
)
    requires m1 < m2
    ensures timestamp_lt_spec(y, mo, d, h, m1, y, mo, d, h, m2)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cron-next.result-after-input]
    #[test]
    fn test_timestamp_lt() {
        assert!(timestamp_lt(2026, 4, 4, 10, 30, 2026, 4, 4, 10, 31));
        assert!(timestamp_lt(2026, 4, 4, 10, 59, 2026, 4, 4, 11, 0));
        assert!(timestamp_lt(2025, 12, 31, 23, 59, 2026, 1, 1, 0, 0));
        assert!(!timestamp_lt(2026, 4, 4, 10, 30, 2026, 4, 4, 10, 30));
        assert!(!timestamp_lt(2027, 1, 1, 0, 0, 2026, 12, 31, 23, 59));
    }

    // r[verify cron-next.result-matches]
    #[test]
    fn test_next_every_minute() {
        // * * * * * — should return next minute
        let (found, y, mo, d, h, m) =
            next_occurrence(2026, 4, 4, 10, 30, 0, 59, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1, 4);
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2026, 4, 4, 10, 31));
    }

    // r[verify cron-next.result-matches]
    #[test]
    fn test_next_hour_rollover() {
        // minute=0 — current minute is 30, should jump to 11:00
        let (found, y, mo, d, h, m) =
            next_occurrence(2026, 4, 4, 10, 30, 0, 0, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1, 4);
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2026, 4, 4, 11, 0));
    }

    // r[verify cron-next.result-matches]
    #[test]
    fn test_next_weekly_monday() {
        // 0 9 * * 1 — every Monday 9:00
        // 2026-04-04 is Saturday, next Monday is 2026-04-06
        let (found, y, mo, d, h, m) = next_occurrence(
            2026, 4, 4, 12, 0, 0, 0, 1, // minute=0
            9, 9, 1, // hour=9
            1, 31, 1, // dom=*
            1, 12, 1, // month=*
            1, 1, 1, // dow=1 (Monday)
            4,
        );
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2026, 4, 6, 9, 0));
    }

    // r[verify cron-next.bounded-search]
    #[test]
    fn test_next_unsatisfiable() {
        // dom=31, month=2 — February never has 31 days
        let (found, _, _, _, _, _) = next_occurrence(
            2026, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 31, 31, 1, // dom=31
            2, 2, 1, // month=2
            0, 6, 1, 10,
        );
        assert!(!found);
    }

    // r[verify cron-next.leap-year-awareness]
    #[test]
    fn test_next_feb29() {
        // dom=29, month=2 — should find next leap year
        let (found, y, mo, d, h, m) = next_occurrence(
            2025, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 29, 29, 1, // dom=29
            2, 2, 1, // month=2
            0, 6, 1, 4,
        );
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2028, 2, 29, 0, 0));
    }

    // r[verify cron-next.month-rollover]
    #[test]
    fn test_next_month_rollover() {
        // dom=31, timestamp in April (30 days) — should jump to May 31
        let (found, y, mo, d, h, m) =
            next_occurrence(2026, 4, 15, 0, 0, 0, 0, 1, 0, 0, 1, 31, 31, 1, 1, 12, 1, 0, 6, 1, 4);
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2026, 5, 31, 0, 0));
    }

    // r[verify cron-next.result-matches]
    #[test]
    fn test_next_step_minute() {
        // */15 — every 15 minutes. At 10:32, next is 10:45.
        let (found, y, mo, d, h, m) =
            next_occurrence(2026, 4, 4, 10, 32, 0, 59, 15, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1, 4);
        assert!(found);
        assert_eq!((y, mo, d, h, m), (2026, 4, 4, 10, 45));
    }

    proptest! {
        // r[verify cron-next.result-after-input]
        #[test]
        fn prop_result_after_input(
            year in 2020u64..2030,
            month in 1u64..=12u64,
            day in 1u64..=28u64,
            hour in 0u64..24,
            minute in 0u64..60,
        ) {
            let (found, ry, rmo, rd, rh, rm) = next_occurrence(
                year, month, day, hour, minute,
                0, 59, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1,
                4,
            );
            if found {
                prop_assert!(
                    timestamp_lt(year, month, day, hour, minute, ry, rmo, rd, rh, rm),
                    "result ({},{},{},{},{}) not after input ({},{},{},{},{})",
                    ry, rmo, rd, rh, rm, year, month, day, hour, minute,
                );
            }
        }

        // r[verify cron-next.result-matches]
        #[test]
        fn prop_result_matches_schedule(
            year in 2020u64..2030,
            month in 1u64..=12u64,
            day in 1u64..=28u64,
            hour in 0u64..24,
            minute in 0u64..60,
            min_step in 1u64..=30,
            hr_step in 1u64..=12,
        ) {
            let (found, ry, rmo, rd, rh, rm) = next_occurrence(
                year, month, day, hour, minute,
                0, 59, min_step,
                0, 23, hr_step,
                1, 31, 1,
                1, 12, 1,
                0, 6, 1,
                4,
            );
            if found {
                prop_assert!(
                    crate::cron_match::matches_schedule(
                        ry, rmo, rd, rh, rm,
                        0, 59, min_step,
                        0, 23, hr_step,
                        1, 31, 1,
                        1, 12, 1,
                        0, 6, 1,
                    ),
                    "result ({},{},{},{},{}) doesn't match schedule",
                    ry, rmo, rd, rh, rm,
                );
            }
        }

        // r[verify cron-next.result-matches]
        #[test]
        fn prop_result_is_valid_date(
            year in 2020u64..2030,
            month in 1u64..=12u64,
            day in 1u64..=28u64,
            hour in 0u64..24,
            minute in 0u64..60,
        ) {
            let (found, ry, rmo, rd, _rh, _rm) = next_occurrence(
                year, month, day, hour, minute,
                0, 59, 1, 0, 23, 1, 1, 31, 1, 1, 12, 1, 0, 6, 1,
                4,
            );
            if found {
                prop_assert!(
                    crate::calendar::is_valid_date(ry, rmo, rd),
                    "result date ({},{},{}) is not valid",
                    ry, rmo, rd,
                );
            }
        }
    }
}
