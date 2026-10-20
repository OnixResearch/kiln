//! Exact finite observability/metrics primitive for MetricHistogramEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricHistogramEnvelope { pub bucket_count: u8, pub count_count: u8, pub max_count: u64, pub ordered: bool, pub counts_bounded: bool }

// r[depends domain-primitives.observability-metrics.histogram]
pub open spec fn metric_histogram_admitted_spec(x: MetricHistogramEnvelope) -> bool { x.bucket_count > 0 && x.bucket_count == x.count_count && x.ordered && x.counts_bounded }

// r[impl domain-primitives.observability-metrics.histogram]
pub fn metric_histogram_admitted(x: MetricHistogramEnvelope) -> (result: bool)
    ensures result == metric_histogram_admitted_spec(x)
{
    x.bucket_count > 0 && x.bucket_count == x.count_count && x.ordered && x.counts_bounded
}

// r[verify domain-primitives.observability-metrics.histogram]
pub proof fn admitted_metric_histogram_has_required_metadata(x: MetricHistogramEnvelope)
    requires metric_histogram_admitted_spec(x)
    ensures x.bucket_count > 0, x.bucket_count == x.count_count, x.ordered, x.counts_bounded
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_histogram_accepts_and_rejects() {
        assert!(metric_histogram_admitted(MetricHistogramEnvelope {
            bucket_count: 2,
            count_count: 2,
            max_count: 10,
            ordered: true,
            counts_bounded: true
        }));
        assert!(!metric_histogram_admitted(MetricHistogramEnvelope {
            bucket_count: 2,
            count_count: 2,
            max_count: 10,
            ordered: false,
            counts_bounded: true
        }));
    }
}
