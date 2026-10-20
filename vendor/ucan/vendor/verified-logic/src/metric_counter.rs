//! Exact finite observability/metrics primitive for MetricCounterEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricCounterEnvelope { pub previous: u64, pub current: u64, pub reset: bool }

// r[depends domain-primitives.observability-metrics.counter]
pub open spec fn metric_counter_admitted_spec(x: MetricCounterEnvelope) -> bool { x.previous <= x.current || x.reset }

// r[impl domain-primitives.observability-metrics.counter]
pub fn metric_counter_admitted(x: MetricCounterEnvelope) -> (result: bool)
    ensures result == metric_counter_admitted_spec(x)
{
    x.previous <= x.current || x.reset
}

// r[verify domain-primitives.observability-metrics.counter]
pub proof fn admitted_metric_counter_has_required_metadata(x: MetricCounterEnvelope)
    requires metric_counter_admitted_spec(x)
    ensures x.previous <= x.current || x.reset
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_counter_accepts_and_rejects() {
        assert!(metric_counter_admitted(MetricCounterEnvelope {
            previous: 1,
            current: 2,
            reset: false
        }));
        assert!(!metric_counter_admitted(MetricCounterEnvelope {
            previous: 2,
            current: 1,
            reset: false
        }));
    }
}
