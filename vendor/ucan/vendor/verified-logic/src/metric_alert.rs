//! Exact finite observability/metrics primitive for MetricAlertEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricAlertEnvelope { pub aggregate_admitted: bool, pub value: i64, pub threshold: i64, pub greater_equal: bool }

// r[depends domain-primitives.observability-metrics.alert]
pub open spec fn metric_alert_admitted_spec(x: MetricAlertEnvelope) -> bool { x.aggregate_admitted && ((x.greater_equal && x.value >= x.threshold) || (!x.greater_equal && x.value < x.threshold)) }

// r[impl domain-primitives.observability-metrics.alert]
pub fn metric_alert_admitted(x: MetricAlertEnvelope) -> (result: bool)
    ensures result == metric_alert_admitted_spec(x)
{
    x.aggregate_admitted && ((x.greater_equal && x.value >= x.threshold) || (!x.greater_equal && x.value < x.threshold))
}

// r[verify domain-primitives.observability-metrics.alert]
pub proof fn admitted_metric_alert_has_required_metadata(x: MetricAlertEnvelope)
    requires metric_alert_admitted_spec(x)
    ensures x.aggregate_admitted
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_alert_accepts_and_rejects() {
        assert!(metric_alert_admitted(MetricAlertEnvelope {
            aggregate_admitted: true,
            value: 5,
            threshold: 4,
            greater_equal: true
        }));
        assert!(!metric_alert_admitted(MetricAlertEnvelope {
            aggregate_admitted: false,
            value: 5,
            threshold: 4,
            greater_equal: true
        }));
    }
}
