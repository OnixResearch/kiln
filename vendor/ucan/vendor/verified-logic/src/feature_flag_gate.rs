//! Exact finite feature-gate predicate envelopes.
use vstd::prelude::*;

use crate::config_key::ConfigKeySet;
use crate::config_key::config_key_known;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FeatureFlagGate { pub namespace: u8, pub key: u16, pub subject: u16, pub required_subject: u16, pub enabled: bool }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FeatureFlagGateDecision { pub admitted: bool, pub enabled: bool }

// r[depends domain-primitives.configuration-feature-flag.gate]
pub open spec fn feature_flag_gate_decide_spec(keys: ConfigKeySet, g: FeatureFlagGate) -> FeatureFlagGateDecision { if crate::config_key::config_key_known_spec(keys,g.namespace,g.key) && g.subject == g.required_subject { FeatureFlagGateDecision{admitted:true,enabled:g.enabled} } else { FeatureFlagGateDecision{admitted:false,enabled:false} } }
// r[impl domain-primitives.configuration-feature-flag.gate]
pub fn feature_flag_gate_decide(keys: ConfigKeySet, g: FeatureFlagGate) -> (result: FeatureFlagGateDecision) ensures result == feature_flag_gate_decide_spec(keys,g) { if config_key_known(keys,g.namespace,g.key) && g.subject == g.required_subject { FeatureFlagGateDecision{admitted:true,enabled:g.enabled} } else { FeatureFlagGateDecision{admitted:false,enabled:false} } }
// r[verify domain-primitives.configuration-feature-flag.gate]
pub proof fn unknown_subject_rejected(keys: ConfigKeySet, g: FeatureFlagGate) requires g.subject != g.required_subject ensures !feature_flag_gate_decide_spec(keys,g).admitted {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_key::ConfigKeySet;
    #[test]
    fn gate_rejects_unknown_subject() {
        let keys = ConfigKeySet {
            namespace: 1,
            key0: 2,
            key1: 0,
            len: 1,
        };
        let g = FeatureFlagGate {
            namespace: 1,
            key: 2,
            subject: 7,
            required_subject: 8,
            enabled: true,
        };
        assert!(!feature_flag_gate_decide(keys, g).admitted);
    }
}
