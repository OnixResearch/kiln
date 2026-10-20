//! Exact finite override-precedence metadata.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigOverrideCandidate { pub source: u8, pub priority: u8, pub present: bool }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigOverrideDecision { pub admitted: bool, pub selected_source: u8, pub selected_priority: u8 }

// r[depends domain-primitives.configuration-feature-flag.override-precedence]
pub open spec fn config_override_select_spec(a: ConfigOverrideCandidate, b: ConfigOverrideCandidate) -> ConfigOverrideDecision { if a.present && b.present && a.source == b.source { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } else if a.present && b.present && a.priority == b.priority { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } else if a.present && (!b.present || a.priority > b.priority) { ConfigOverrideDecision{admitted:true,selected_source:a.source,selected_priority:a.priority} } else if b.present { ConfigOverrideDecision{admitted:true,selected_source:b.source,selected_priority:b.priority} } else { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } }
// r[impl domain-primitives.configuration-feature-flag.override-precedence]
pub fn config_override_select(a: ConfigOverrideCandidate, b: ConfigOverrideCandidate) -> (result: ConfigOverrideDecision) ensures result == config_override_select_spec(a,b) { if a.present && b.present && a.source == b.source { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } else if a.present && b.present && a.priority == b.priority { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } else if a.present && (!b.present || a.priority > b.priority) { ConfigOverrideDecision{admitted:true,selected_source:a.source,selected_priority:a.priority} } else if b.present { ConfigOverrideDecision{admitted:true,selected_source:b.source,selected_priority:b.priority} } else { ConfigOverrideDecision{admitted:false,selected_source:0,selected_priority:0} } }
// r[verify domain-primitives.configuration-feature-flag.override-precedence]
pub proof fn duplicate_source_rejected(a: ConfigOverrideCandidate, b: ConfigOverrideCandidate) requires a.present, b.present, a.source == b.source ensures !config_override_select_spec(a,b).admitted {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_selects_highest_unique() {
        let a = ConfigOverrideCandidate {
            source: 1,
            priority: 4,
            present: true,
        };
        let b = ConfigOverrideCandidate {
            source: 2,
            priority: 9,
            present: true,
        };
        let d = config_override_select(a, b);
        assert!(d.admitted);
        assert_eq!(d.selected_source, 2);
    }
}
