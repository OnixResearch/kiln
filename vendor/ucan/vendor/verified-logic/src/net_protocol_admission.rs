//! Exact finite network-protocol primitive for ProtocolAdmissionEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProtocolAdmissionEnvelope {
    pub checks_ok_count: u8,
    pub expected_checks: u8,
    pub failure_code: u8,
    pub external_claim: bool,
}

// r[depends domain-primitives.network-protocol.admission]
pub open spec fn net_protocol_admission_admitted_spec(x: ProtocolAdmissionEnvelope) -> bool {
    x.checks_ok_count == x.expected_checks && x.expected_checks == 7 && x.failure_code == 0 && !x.external_claim
}

// r[impl domain-primitives.network-protocol.admission]
pub fn net_protocol_admission_admitted(x: ProtocolAdmissionEnvelope) -> (result: bool)
    ensures result == net_protocol_admission_admitted_spec(x)
{
    x.checks_ok_count == x.expected_checks && x.expected_checks == 7 && x.failure_code == 0 && !x.external_claim
}

// r[verify domain-primitives.network-protocol.admission]
pub proof fn admitted_net_protocol_admission_has_all_finite_checks(x: ProtocolAdmissionEnvelope)
    requires net_protocol_admission_admitted_spec(x)
    ensures x.checks_ok_count == 7, x.expected_checks == 7, x.failure_code == 0, !x.external_claim
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_protocol_admission_accepts_and_rejects() {
        assert!(net_protocol_admission_admitted(ProtocolAdmissionEnvelope {
            checks_ok_count: 7,
            expected_checks: 7,
            failure_code: 0,
            external_claim: false,
        }));
        assert!(!net_protocol_admission_admitted(ProtocolAdmissionEnvelope {
            checks_ok_count: 6,
            expected_checks: 7,
            failure_code: 1,
            external_claim: false,
        }));
    }
}
