//! Exact finite probability statistics primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsCountTable {
    pub category_count: usize,
    pub total_count: usize,
    pub max_total: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsFrequencyTable {
    pub numerator: usize,
    pub denominator: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsDiscreteDistribution {
    pub bucket_count: usize,
    pub max_buckets: usize,
    pub total_weight: usize,
    pub max_weight: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsCdfQuantile {
    pub bucket_count: usize,
    pub selected_index: usize,
    pub threshold: usize,
    pub total_weight: usize,
    pub cumulative_before: usize,
    pub cumulative_at: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsBoundedMoments {
    pub count: usize,
    pub sum_abs: usize,
    pub max_sum_abs: usize,
    pub mean_num_abs: usize,
    pub variance_num: usize,
    pub max_variance_num: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsDeterministicSampler {
    pub bucket_count: usize,
    pub selected_index: usize,
    pub threshold: usize,
    pub total_weight: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatsConfidenceIntervalEnvelope {
    pub lower: i64,
    pub estimate: i64,
    pub upper: i64,
    pub confidence_per_million: usize,
    pub sample_count: usize,
}

// r[depends domain-primitives.stats-count-table]
pub open spec fn stats_count_table_spec(e: StatsCountTable) -> bool {
    e.category_count <= e.total_count && e.total_count <= e.max_total
}

// r[impl domain-primitives.stats-count-table]
pub fn stats_count_table_valid(e: StatsCountTable) -> (result: bool)
    ensures result == stats_count_table_spec(e)
{
    e.category_count <= e.total_count && e.total_count <= e.max_total
}

// r[verify domain-primitives.stats-count-table]
pub proof fn valid_stats_count_table_is_admitted(e: StatsCountTable)
    requires stats_count_table_spec(e)
    ensures stats_count_table_spec(e)
{}

// r[depends domain-primitives.stats-frequency-table]
pub open spec fn stats_frequency_table_spec(e: StatsFrequencyTable) -> bool {
    e.denominator > 0 && e.numerator <= e.denominator
}

// r[impl domain-primitives.stats-frequency-table]
pub fn stats_frequency_table_valid(e: StatsFrequencyTable) -> (result: bool)
    ensures result == stats_frequency_table_spec(e)
{
    e.denominator > 0 && e.numerator <= e.denominator
}

// r[verify domain-primitives.stats-frequency-table]
pub proof fn valid_stats_frequency_table_is_admitted(e: StatsFrequencyTable)
    requires stats_frequency_table_spec(e)
    ensures stats_frequency_table_spec(e)
{}

// r[depends domain-primitives.stats-discrete-distribution]
pub open spec fn stats_discrete_distribution_spec(e: StatsDiscreteDistribution) -> bool {
    e.bucket_count > 0 && e.bucket_count <= e.max_buckets && e.total_weight > 0 && e.max_weight <= e.total_weight
}

// r[impl domain-primitives.stats-discrete-distribution]
pub fn stats_discrete_distribution_valid(e: StatsDiscreteDistribution) -> (result: bool)
    ensures result == stats_discrete_distribution_spec(e)
{
    e.bucket_count > 0 && e.bucket_count <= e.max_buckets && e.total_weight > 0 && e.max_weight <= e.total_weight
}

// r[verify domain-primitives.stats-discrete-distribution]
pub proof fn valid_stats_discrete_distribution_is_admitted(e: StatsDiscreteDistribution)
    requires stats_discrete_distribution_spec(e)
    ensures stats_discrete_distribution_spec(e)
{}

// r[depends domain-primitives.stats-cdf-quantile]
pub open spec fn stats_cdf_quantile_spec(e: StatsCdfQuantile) -> bool {
    e.bucket_count > 0 && e.selected_index < e.bucket_count && e.threshold < e.total_weight && e.cumulative_before <= e.threshold && e.threshold < e.cumulative_at && e.cumulative_before <= e.cumulative_at && e.cumulative_at <= e.total_weight
}

// r[impl domain-primitives.stats-cdf-quantile]
pub fn stats_cdf_quantile_valid(e: StatsCdfQuantile) -> (result: bool)
    ensures result == stats_cdf_quantile_spec(e)
{
    e.bucket_count > 0 && e.selected_index < e.bucket_count && e.threshold < e.total_weight && e.cumulative_before <= e.threshold && e.threshold < e.cumulative_at && e.cumulative_before <= e.cumulative_at && e.cumulative_at <= e.total_weight
}

// r[verify domain-primitives.stats-cdf-quantile]
pub proof fn valid_stats_cdf_quantile_is_admitted(e: StatsCdfQuantile)
    requires stats_cdf_quantile_spec(e)
    ensures stats_cdf_quantile_spec(e)
{}

// r[depends domain-primitives.stats-bounded-moments]
pub open spec fn stats_bounded_moments_spec(e: StatsBoundedMoments) -> bool {
    e.count > 0 && e.sum_abs <= e.max_sum_abs && e.mean_num_abs <= e.sum_abs && e.variance_num <= e.max_variance_num
}

// r[impl domain-primitives.stats-bounded-moments]
pub fn stats_bounded_moments_valid(e: StatsBoundedMoments) -> (result: bool)
    ensures result == stats_bounded_moments_spec(e)
{
    e.count > 0 && e.sum_abs <= e.max_sum_abs && e.mean_num_abs <= e.sum_abs && e.variance_num <= e.max_variance_num
}

// r[verify domain-primitives.stats-bounded-moments]
pub proof fn valid_stats_bounded_moments_is_admitted(e: StatsBoundedMoments)
    requires stats_bounded_moments_spec(e)
    ensures stats_bounded_moments_spec(e)
{}

// r[depends domain-primitives.stats-deterministic-sampler]
pub open spec fn stats_deterministic_sampler_spec(e: StatsDeterministicSampler) -> bool {
    e.bucket_count > 0 && e.selected_index < e.bucket_count && e.threshold < e.total_weight && e.total_weight > 0
}

// r[impl domain-primitives.stats-deterministic-sampler]
pub fn stats_deterministic_sampler_valid(e: StatsDeterministicSampler) -> (result: bool)
    ensures result == stats_deterministic_sampler_spec(e)
{
    e.bucket_count > 0 && e.selected_index < e.bucket_count && e.threshold < e.total_weight && e.total_weight > 0
}

// r[verify domain-primitives.stats-deterministic-sampler]
pub proof fn valid_stats_deterministic_sampler_is_admitted(e: StatsDeterministicSampler)
    requires stats_deterministic_sampler_spec(e)
    ensures stats_deterministic_sampler_spec(e)
{}

// r[depends domain-primitives.stats-confidence-interval-envelope]
pub open spec fn stats_confidence_interval_envelope_spec(e: StatsConfidenceIntervalEnvelope) -> bool {
    e.lower <= e.estimate && e.estimate <= e.upper && e.confidence_per_million <= 1000000 && e.sample_count > 0
}

// r[impl domain-primitives.stats-confidence-interval-envelope]
pub fn stats_confidence_interval_envelope_valid(e: StatsConfidenceIntervalEnvelope) -> (result: bool)
    ensures result == stats_confidence_interval_envelope_spec(e)
{
    e.lower <= e.estimate && e.estimate <= e.upper && e.confidence_per_million <= 1000000 && e.sample_count > 0
}

// r[verify domain-primitives.stats-confidence-interval-envelope]
pub proof fn valid_stats_confidence_interval_envelope_is_admitted(e: StatsConfidenceIntervalEnvelope)
    requires stats_confidence_interval_envelope_spec(e)
    ensures stats_confidence_interval_envelope_spec(e)
{}

} // verus!
