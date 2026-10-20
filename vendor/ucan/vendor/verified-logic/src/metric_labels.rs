//! Exact finite observability/metrics primitive for MetricLabelEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricLabelEnvelope { pub key_count: u8, pub max_keys: u8, pub keys_bounded: bool, pub values_bounded: bool, pub duplicate_key: bool }

// r[depends domain-primitives.observability-metrics.labels]
pub open spec fn metric_labels_admitted_spec(x: MetricLabelEnvelope) -> bool { x.key_count <= x.max_keys && x.keys_bounded && x.values_bounded && !x.duplicate_key }

// r[impl domain-primitives.observability-metrics.labels]
pub fn metric_labels_admitted(x: MetricLabelEnvelope) -> (result: bool)
    ensures result == metric_labels_admitted_spec(x)
{
    x.key_count <= x.max_keys && x.keys_bounded && x.values_bounded && !x.duplicate_key
}

// r[verify domain-primitives.observability-metrics.labels]
pub proof fn admitted_metric_labels_has_required_metadata(x: MetricLabelEnvelope)
    requires metric_labels_admitted_spec(x)
    ensures x.key_count <= x.max_keys, x.keys_bounded, x.values_bounded, !x.duplicate_key
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_labels_accepts_and_rejects() {
        assert!(metric_labels_admitted(MetricLabelEnvelope {
            key_count: 2,
            max_keys: 4,
            keys_bounded: true,
            values_bounded: true,
            duplicate_key: false
        }));
        assert!(!metric_labels_admitted(MetricLabelEnvelope {
            key_count: 2,
            max_keys: 4,
            keys_bounded: true,
            values_bounded: true,
            duplicate_key: true
        }));
    }
}
