//! Exact finite observability/metrics primitive for MetricAggregateEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricAggregateEnvelope { pub window_start: u64, pub window_end: u64, pub sample_count: u64, pub metric_declared: bool, pub labels_admitted: bool }

// r[depends domain-primitives.observability-metrics.aggregation]
pub open spec fn metric_aggregate_admitted_spec(x: MetricAggregateEnvelope) -> bool { x.window_start < x.window_end && x.sample_count > 0 && x.metric_declared && x.labels_admitted }

// r[impl domain-primitives.observability-metrics.aggregation]
pub fn metric_aggregate_admitted(x: MetricAggregateEnvelope) -> (result: bool)
    ensures result == metric_aggregate_admitted_spec(x)
{
    x.window_start < x.window_end && x.sample_count > 0 && x.metric_declared && x.labels_admitted
}

// r[verify domain-primitives.observability-metrics.aggregation]
pub proof fn admitted_metric_aggregate_has_required_metadata(x: MetricAggregateEnvelope)
    requires metric_aggregate_admitted_spec(x)
    ensures x.window_start < x.window_end, x.sample_count > 0, x.metric_declared, x.labels_admitted
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_aggregate_accepts_and_rejects() {
        assert!(metric_aggregate_admitted(MetricAggregateEnvelope {
            window_start: 1,
            window_end: 3,
            sample_count: 2,
            metric_declared: true,
            labels_admitted: true
        }));
        assert!(!metric_aggregate_admitted(MetricAggregateEnvelope {
            window_start: 1,
            window_end: 1,
            sample_count: 0,
            metric_declared: true,
            labels_admitted: true
        }));
    }
}
