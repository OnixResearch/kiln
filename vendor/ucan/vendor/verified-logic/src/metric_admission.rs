//! Exact finite observability/metrics primitive for MetricAdmissionEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetricAdmissionEnvelope { pub checks_ok_count: u8, pub expected_checks: u8, pub reject_code: u8, pub external_claim: bool }

// r[depends domain-primitives.observability-metrics.admission]
pub open spec fn metric_admission_admitted_spec(x: MetricAdmissionEnvelope) -> bool { x.checks_ok_count == x.expected_checks && x.expected_checks == 8 && x.reject_code == 0 && !x.external_claim }

// r[impl domain-primitives.observability-metrics.admission]
pub fn metric_admission_admitted(x: MetricAdmissionEnvelope) -> (result: bool)
    ensures result == metric_admission_admitted_spec(x)
{
    x.checks_ok_count == x.expected_checks && x.expected_checks == 8 && x.reject_code == 0 && !x.external_claim
}

// r[verify domain-primitives.observability-metrics.admission]
pub proof fn admitted_metric_admission_has_required_metadata(x: MetricAdmissionEnvelope)
    requires metric_admission_admitted_spec(x)
    ensures x.checks_ok_count == 8, x.expected_checks == 8, x.reject_code == 0, !x.external_claim
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_admission_accepts_and_rejects() {
        assert!(metric_admission_admitted(MetricAdmissionEnvelope {
            checks_ok_count: 8,
            expected_checks: 8,
            reject_code: 0,
            external_claim: false
        }));
        assert!(!metric_admission_admitted(MetricAdmissionEnvelope {
            checks_ok_count: 7,
            expected_checks: 8,
            reject_code: 1,
            external_claim: false
        }));
    }
}
