//! Exact finite observability/metrics primitive for MetricWindowEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricWindowEnvelope { pub start: u64, pub end: u64, pub sample: u64 }

// r[depends domain-primitives.observability-metrics.window]
pub open spec fn metric_window_admitted_spec(x: MetricWindowEnvelope) -> bool { x.start <= x.sample && x.sample < x.end }

// r[impl domain-primitives.observability-metrics.window]
pub fn metric_window_admitted(x: MetricWindowEnvelope) -> (result: bool)
    ensures result == metric_window_admitted_spec(x)
{
    x.start <= x.sample && x.sample < x.end
}

// r[verify domain-primitives.observability-metrics.window]
pub proof fn admitted_metric_window_has_required_metadata(x: MetricWindowEnvelope)
    requires metric_window_admitted_spec(x)
    ensures x.start <= x.sample, x.sample < x.end
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_window_accepts_and_rejects() {
        assert!(metric_window_admitted(MetricWindowEnvelope {
            start: 1,
            end: 3,
            sample: 2
        }));
        assert!(!metric_window_admitted(MetricWindowEnvelope {
            start: 1,
            end: 3,
            sample: 3
        }));
    }
}
