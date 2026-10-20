//! Exact finite observability/metrics primitive for MetricIdEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricIdEnvelope { pub namespace: u8, pub name: u8, pub declared: bool }

// r[depends domain-primitives.observability-metrics.metric-id]
pub open spec fn metric_id_admitted_spec(x: MetricIdEnvelope) -> bool { x.namespace > 0 && x.name > 0 && x.declared }

// r[impl domain-primitives.observability-metrics.metric-id]
pub fn metric_id_admitted(x: MetricIdEnvelope) -> (result: bool)
    ensures result == metric_id_admitted_spec(x)
{
    x.namespace > 0 && x.name > 0 && x.declared
}

// r[verify domain-primitives.observability-metrics.metric-id]
pub proof fn admitted_metric_id_has_required_metadata(x: MetricIdEnvelope)
    requires metric_id_admitted_spec(x)
    ensures x.namespace > 0, x.name > 0, x.declared
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_id_accepts_and_rejects() {
        assert!(metric_id_admitted(MetricIdEnvelope {
            namespace: 1,
            name: 1,
            declared: true
        }));
        assert!(!metric_id_admitted(MetricIdEnvelope {
            namespace: 0,
            name: 1,
            declared: true
        }));
    }
}
