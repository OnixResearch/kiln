//! Exact finite time-series primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimestampIndexPolicy {
    pub min_ts: u64,
    pub max_ts: u64,
    pub allow_duplicates: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SampleUpdateRequest {
    pub len: usize,
    pub index: usize,
    pub value: i64,
    pub min_value: i64,
    pub max_value: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimeWindow {
    pub start: u64,
    pub end: u64,
    pub include_start: bool,
    pub include_end: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResampleEnvelope {
    pub range_start: u64,
    pub range_end: u64,
    pub period: u64,
    pub bucket_count: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RollingAggregateEnvelope {
    pub count: u64,
    pub sum: i64,
    pub min: i64,
    pub max: i64,
    pub count_bound: u64,
    pub min_bound: i64,
    pub max_bound: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GapInterval {
    pub left: u64,
    pub right: u64,
    pub threshold: u64,
}

// r[depends domain-primitives.timeseries-timestamp-index]
pub open spec fn timestamp_index_valid_spec(ts: Seq<u64>, policy: TimestampIndexPolicy) -> bool {
    policy.min_ts <= policy.max_ts
        && (forall|i: int| #![trigger ts[i]] 0 <= i < ts.len() ==> policy.min_ts <= ts[i] <= policy.max_ts)
        && (forall|i: int| #![trigger ts[i]] 1 <= i < ts.len() ==> if policy.allow_duplicates { ts[i - 1] <= ts[i] } else { ts[i - 1] < ts[i] })
}

// r[depends domain-primitives.timeseries-timestamp-index]
pub open spec fn timestamp_partition_spec(ts: Seq<u64>, query: u64, pos: usize) -> bool {
    pos as int <= ts.len()
        && (forall|i: int| #![trigger ts[i]] 0 <= i < pos as int ==> ts[i] < query)
        && (forall|i: int| #![trigger ts[i]] pos as int <= i < ts.len() ==> query <= ts[i])
}

// r[impl domain-primitives.timeseries-timestamp-index]
pub fn timestamp_index_valid(ts: &Vec<u64>, policy: TimestampIndexPolicy) -> (result: bool)
    ensures result == timestamp_index_valid_spec(ts@, policy)
{
    if policy.min_ts > policy.max_ts {
        return false;
    }
    let mut i: usize = 0;
    while i < ts.len()
        invariant
            policy.min_ts <= policy.max_ts,
            0 <= i <= ts.len(),
            forall|k: int| #![trigger ts@[k]] 0 <= k < i ==> policy.min_ts <= ts@[k] <= policy.max_ts,
            forall|k: int| #![trigger ts@[k]] 1 <= k < i ==> if policy.allow_duplicates { ts@[k - 1] <= ts@[k] } else { ts@[k - 1] < ts@[k] },
        decreases ts.len() - i,
    {
        if ts[i] < policy.min_ts || ts[i] > policy.max_ts {
            assert(!timestamp_index_valid_spec(ts@, policy));
            return false;
        }
        if i > 0 {
            if policy.allow_duplicates {
                if ts[i - 1] > ts[i] {
                    assert(!timestamp_index_valid_spec(ts@, policy));
                    return false;
                }
            } else if ts[i - 1] >= ts[i] {
                assert(!timestamp_index_valid_spec(ts@, policy));
                return false;
            }
        }
        i = i + 1;
    }
    true
}

// r[impl domain-primitives.timeseries-timestamp-index]
pub fn timestamp_partition_valid(ts: &Vec<u64>, query: u64, pos: usize) -> (result: bool)
    ensures result == timestamp_partition_spec(ts@, query, pos)
{
    if pos > ts.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < pos
        invariant
            pos <= ts.len(),
            0 <= i <= pos,
            forall|k: int| #![trigger ts@[k]] 0 <= k < i ==> ts@[k] < query,
        decreases pos - i,
    {
        if ts[i] >= query {
            assert(!timestamp_partition_spec(ts@, query, pos));
            return false;
        }
        i = i + 1;
    }
    let mut j: usize = pos;
    while j < ts.len()
        invariant
            pos <= ts.len(),
            0 <= pos <= j <= ts.len(),
            forall|k: int| #![trigger ts@[k]] 0 <= k < pos as int ==> ts@[k] < query,
            forall|k: int| #![trigger ts@[k]] pos as int <= k < j ==> query <= ts@[k],
        decreases ts.len() - j,
    {
        if ts[j] < query {
            assert(!timestamp_partition_spec(ts@, query, pos));
            return false;
        }
        j = j + 1;
    }
    true
}

// r[verify domain-primitives.timeseries-timestamp-index]
pub proof fn timestamp_partition_position_bounded(ts: Seq<u64>, query: u64, pos: usize)
    requires timestamp_partition_spec(ts, query, pos)
    ensures pos as int <= ts.len()
{}

// r[depends domain-primitives.timeseries-sample-series]
pub open spec fn sample_series_valid_spec(ts: Seq<u64>, values: Seq<i64>, min_value: i64, max_value: i64) -> bool {
    min_value <= max_value
        && ts.len() == values.len()
        && (forall|i: int| #![trigger values[i]] 0 <= i < values.len() ==> min_value <= values[i] <= max_value)
}

// r[depends domain-primitives.timeseries-sample-series]
pub open spec fn sample_update_admitted_spec(req: SampleUpdateRequest) -> bool {
    req.min_value <= req.max_value && req.index < req.len && req.min_value <= req.value <= req.max_value
}

// r[impl domain-primitives.timeseries-sample-series]
pub fn sample_series_valid(ts: &Vec<u64>, values: &Vec<i64>, min_value: i64, max_value: i64) -> (result: bool)
    ensures result == sample_series_valid_spec(ts@, values@, min_value, max_value)
{
    if min_value > max_value || ts.len() != values.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < values.len()
        invariant
            min_value <= max_value,
            ts.len() == values.len(),
            0 <= i <= values.len(),
            forall|k: int| #![trigger values@[k]] 0 <= k < i ==> min_value <= values@[k] <= max_value,
        decreases values.len() - i,
    {
        if values[i] < min_value || values[i] > max_value {
            assert(!sample_series_valid_spec(ts@, values@, min_value, max_value));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl domain-primitives.timeseries-sample-series]
pub fn sample_update_admitted(req: SampleUpdateRequest) -> (result: bool)
    ensures result == sample_update_admitted_spec(req)
{
    req.min_value <= req.max_value && req.index < req.len && req.min_value <= req.value && req.value <= req.max_value
}

// r[verify domain-primitives.timeseries-sample-series]
pub proof fn sample_update_index_in_bounds(req: SampleUpdateRequest)
    requires sample_update_admitted_spec(req)
    ensures req.index < req.len
{}

// r[depends domain-primitives.timeseries-window]
pub open spec fn window_valid_spec(w: TimeWindow) -> bool {
    w.start <= w.end
}

// r[depends domain-primitives.timeseries-window]
pub open spec fn timestamp_in_window_spec(ts: u64, w: TimeWindow) -> bool {
    window_valid_spec(w)
        && (if w.include_start { w.start <= ts } else { w.start < ts })
        && (if w.include_end { ts <= w.end } else { ts < w.end })
}

// r[depends domain-primitives.timeseries-window]
pub open spec fn window_selection_spec(timestamps: Seq<u64>, selected: Seq<bool>, w: TimeWindow) -> bool {
    window_valid_spec(w)
        && timestamps.len() == selected.len()
        && (forall|i: int| #![trigger timestamps[i]] 0 <= i < timestamps.len() ==> selected[i] == timestamp_in_window_spec(timestamps[i], w))
}

// r[impl domain-primitives.timeseries-window]
pub fn window_valid(w: TimeWindow) -> (result: bool)
    ensures result == window_valid_spec(w)
{
    w.start <= w.end
}

// r[impl domain-primitives.timeseries-window]
pub fn timestamp_in_window(ts: u64, w: TimeWindow) -> (result: bool)
    ensures result == timestamp_in_window_spec(ts, w)
{
    if w.start > w.end {
        return false;
    }
    let lower = if w.include_start { w.start <= ts } else { w.start < ts };
    let upper = if w.include_end { ts <= w.end } else { ts < w.end };
    lower && upper
}

// r[impl domain-primitives.timeseries-window]
pub fn window_selection_valid(timestamps: &Vec<u64>, selected: &Vec<bool>, w: TimeWindow) -> (result: bool)
    ensures result == window_selection_spec(timestamps@, selected@, w)
{
    if !window_valid(w) || timestamps.len() != selected.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < timestamps.len()
        invariant
            window_valid_spec(w),
            timestamps.len() == selected.len(),
            0 <= i <= timestamps.len(),
            forall|k: int| #![trigger timestamps@[k]] 0 <= k < i ==> selected@[k] == timestamp_in_window_spec(timestamps@[k], w),
        decreases timestamps.len() - i,
    {
        if selected[i] != timestamp_in_window(timestamps[i], w) {
            assert(!window_selection_spec(timestamps@, selected@, w));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify domain-primitives.timeseries-window]
pub proof fn selected_timestamp_is_in_window(timestamps: Seq<u64>, selected: Seq<bool>, w: TimeWindow, i: int)
    requires window_selection_spec(timestamps, selected, w), 0 <= i < selected.len(), selected[i]
    ensures timestamp_in_window_spec(timestamps[i], w)
{}

// r[depends domain-primitives.timeseries-resample-envelope]
pub open spec fn resample_envelope_valid_spec(e: ResampleEnvelope) -> bool {
    e.range_start <= e.range_end && e.period > 0 && e.bucket_count > 0
}

// r[depends domain-primitives.timeseries-resample-envelope]
pub open spec fn resample_bucket_spec(ts: u64, e: ResampleEnvelope, bucket: u64) -> bool {
    resample_envelope_valid_spec(e)
        && e.range_start <= ts <= e.range_end
        && bucket < e.bucket_count
        && bucket as int == (ts - e.range_start) / (e.period as int)
}

// r[impl domain-primitives.timeseries-resample-envelope]
pub fn resample_envelope_valid(e: ResampleEnvelope) -> (result: bool)
    ensures result == resample_envelope_valid_spec(e)
{
    e.range_start <= e.range_end && e.period > 0 && e.bucket_count > 0
}

// r[impl domain-primitives.timeseries-resample-envelope]
pub fn resample_bucket_admitted(ts: u64, e: ResampleEnvelope, bucket: u64) -> (result: bool)
    ensures result == resample_bucket_spec(ts, e, bucket)
{
    if !resample_envelope_valid(e) || ts < e.range_start || ts > e.range_end || bucket >= e.bucket_count {
        return false;
    }
    bucket == (ts - e.range_start) / e.period
}

// r[verify domain-primitives.timeseries-resample-envelope]
pub proof fn admitted_resample_bucket_is_bounded(ts: u64, e: ResampleEnvelope, bucket: u64)
    requires resample_bucket_spec(ts, e, bucket)
    ensures bucket < e.bucket_count
{}

// r[depends domain-primitives.timeseries-rolling-aggregate]
pub open spec fn rolling_aggregate_valid_spec(a: RollingAggregateEnvelope) -> bool {
    a.count > 0
        && a.count <= a.count_bound
        && a.min <= a.max
        && a.min_bound <= a.min
        && a.max <= a.max_bound
        && a.min_bound <= a.sum <= a.max_bound
}

// r[depends domain-primitives.timeseries-rolling-aggregate]
pub open spec fn aggregate_sum_step_admitted_spec(current: i64, value: i64, bound: i64) -> bool {
    current.checked_add(value).is_some() && current.checked_add(value).unwrap() <= bound
}

// r[impl domain-primitives.timeseries-rolling-aggregate]
pub fn rolling_aggregate_valid(a: RollingAggregateEnvelope) -> (result: bool)
    ensures result == rolling_aggregate_valid_spec(a)
{
    a.count > 0
        && a.count <= a.count_bound
        && a.min <= a.max
        && a.min_bound <= a.min
        && a.max <= a.max_bound
        && a.min_bound <= a.sum
        && a.sum <= a.max_bound
}

// r[impl domain-primitives.timeseries-rolling-aggregate]
pub fn aggregate_sum_step_admitted(current: i64, value: i64, bound: i64) -> (result: bool)
    ensures result == aggregate_sum_step_admitted_spec(current, value, bound)
{
    match current.checked_add(value) {
        Some(sum) => sum <= bound,
        None => false,
    }
}

// r[verify domain-primitives.timeseries-rolling-aggregate]
pub proof fn rolling_aggregate_count_bounded(a: RollingAggregateEnvelope)
    requires rolling_aggregate_valid_spec(a)
    ensures a.count <= a.count_bound
{}

// r[depends domain-primitives.timeseries-gap-detection]
pub open spec fn gap_interval_valid_spec(g: GapInterval) -> bool {
    g.threshold > 0 && g.left <= g.right && g.right - g.left > g.threshold
}

// r[depends domain-primitives.timeseries-gap-detection]
pub open spec fn gap_report_spec(timestamps: Seq<u64>, threshold: u64, gaps: Seq<bool>) -> bool {
    threshold > 0
        && gaps.len() < usize::MAX as int
        && timestamps.len() == gaps.len() + 1
        && (forall|i: int| #![trigger timestamps[i]] 0 <= i < gaps.len() ==> timestamps[i] <= timestamps[i + 1] && gaps[i] == (timestamps[i + 1] - timestamps[i] > threshold))
}

// r[impl domain-primitives.timeseries-gap-detection]
pub fn gap_interval_valid(g: GapInterval) -> (result: bool)
    ensures result == gap_interval_valid_spec(g)
{
    g.threshold > 0 && g.left <= g.right && g.right - g.left > g.threshold
}

// r[impl domain-primitives.timeseries-gap-detection]
pub fn gap_report_valid(timestamps: &Vec<u64>, threshold: u64, gaps: &Vec<bool>) -> (result: bool)
    ensures result == gap_report_spec(timestamps@, threshold, gaps@)
{
    if threshold == 0 || gaps.len() == usize::MAX || timestamps.len() != gaps.len() + 1 {
        return false;
    }
    let mut i: usize = 0;
    while i < gaps.len()
        invariant
            threshold > 0,
            timestamps.len() == gaps.len() + 1,
            0 <= i <= gaps.len(),
            forall|k: int| #![trigger timestamps@[k]] 0 <= k < i ==> timestamps@[k] <= timestamps@[k + 1] && gaps@[k] == (timestamps@[k + 1] - timestamps@[k] > threshold),
        decreases gaps.len() - i,
    {
        if timestamps[i] > timestamps[i + 1] {
            assert(!gap_report_spec(timestamps@, threshold, gaps@));
            return false;
        }
        let is_gap = timestamps[i + 1] - timestamps[i] > threshold;
        if gaps[i] != is_gap {
            assert(!gap_report_spec(timestamps@, threshold, gaps@));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify domain-primitives.timeseries-gap-detection]
pub proof fn valid_gap_exceeds_threshold(g: GapInterval)
    requires gap_interval_valid_spec(g)
    ensures g.right - g.left > g.threshold
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_index_accepts_sorted_and_rejects_unsorted() {
        let policy = TimestampIndexPolicy {
            min_ts: 0,
            max_ts: 10,
            allow_duplicates: true,
        };
        assert!(timestamp_index_valid(&vec![1, 2, 2, 8], policy));
        assert!(!timestamp_index_valid(&vec![1, 3, 2], policy));
        assert!(timestamp_partition_valid(&vec![1, 2, 4, 7], 4, 2));
        assert!(!timestamp_partition_valid(&vec![1, 2, 4, 7], 4, 3));
    }

    #[test]
    fn sample_series_alignment_and_update_admission() {
        assert!(sample_series_valid(&vec![1, 2], &vec![-1, 5], -10, 10));
        assert!(!sample_series_valid(&vec![1], &vec![1, 2], -10, 10));
        assert!(sample_update_admitted(SampleUpdateRequest {
            len: 2,
            index: 1,
            value: 7,
            min_value: 0,
            max_value: 10
        }));
        assert!(!sample_update_admitted(SampleUpdateRequest {
            len: 2,
            index: 2,
            value: 7,
            min_value: 0,
            max_value: 10
        }));
    }

    #[test]
    fn window_selection_matches_boundary_policy() {
        let w = TimeWindow {
            start: 10,
            end: 20,
            include_start: true,
            include_end: false,
        };
        assert!(window_selection_valid(&vec![9, 10, 19, 20], &vec![false, true, true, false], w));
        assert!(!window_selection_valid(&vec![9, 10, 19, 20], &vec![false, false, true, false], w));
    }

    #[test]
    fn resample_bucket_is_deterministic_and_bounded() {
        let e = ResampleEnvelope {
            range_start: 10,
            range_end: 30,
            period: 5,
            bucket_count: 5,
        };
        assert!(resample_bucket_admitted(22, e, 2));
        assert!(!resample_bucket_admitted(22, e, 3));
        assert!(!resample_envelope_valid(ResampleEnvelope { period: 0, ..e }));
    }

    #[test]
    fn rolling_aggregate_bounds_and_overflow() {
        assert!(rolling_aggregate_valid(RollingAggregateEnvelope {
            count: 3,
            sum: 12,
            min: 2,
            max: 6,
            count_bound: 5,
            min_bound: 0,
            max_bound: 20
        }));
        assert!(!rolling_aggregate_valid(RollingAggregateEnvelope {
            count: 6,
            sum: 12,
            min: 2,
            max: 6,
            count_bound: 5,
            min_bound: 0,
            max_bound: 20
        }));
        assert!(aggregate_sum_step_admitted(5, 6, 20));
        assert!(!aggregate_sum_step_admitted(i64::MAX, 1, i64::MAX));
    }

    #[test]
    fn gap_report_matches_adjacent_thresholds() {
        assert!(gap_interval_valid(GapInterval {
            left: 10,
            right: 21,
            threshold: 10
        }));
        assert!(gap_report_valid(&vec![1, 3, 10, 11], 3, &vec![false, true, false]));
        assert!(!gap_report_valid(&vec![1, 10, 3], 3, &vec![true, false]));
    }
}
