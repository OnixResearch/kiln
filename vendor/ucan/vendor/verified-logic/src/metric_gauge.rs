//! Exact finite observability/metrics primitive for MetricGaugeEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricGaugeEnvelope { pub min: i64, pub value: i64, pub max: i64 }

// r[depends domain-primitives.observability-metrics.gauge]
pub open spec fn metric_gauge_admitted_spec(x: MetricGaugeEnvelope) -> bool { x.min <= x.value && x.value <= x.max }

// r[impl domain-primitives.observability-metrics.gauge]
pub fn metric_gauge_admitted(x: MetricGaugeEnvelope) -> (result: bool)
    ensures result == metric_gauge_admitted_spec(x)
{
    x.min <= x.value && x.value <= x.max
}

// r[verify domain-primitives.observability-metrics.gauge]
pub proof fn admitted_metric_gauge_has_required_metadata(x: MetricGaugeEnvelope)
    requires metric_gauge_admitted_spec(x)
    ensures x.min <= x.value, x.value <= x.max
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_gauge_accepts_and_rejects() {
        assert!(metric_gauge_admitted(MetricGaugeEnvelope {
            min: -1,
            value: 0,
            max: 1
        }));
        assert!(!metric_gauge_admitted(MetricGaugeEnvelope {
            min: 0,
            value: 2,
            max: 1
        }));
    }
}
