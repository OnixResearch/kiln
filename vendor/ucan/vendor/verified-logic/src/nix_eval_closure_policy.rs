//! Nix evaluation closure-policy composite facade.
//!
//! This facade composes the existing Nix/Nickel verified primitives into a
//! single admission policy for a merged evaluation closure.  It is deliberately
//! small: acceptance requires sorted Nix attrsets, a valid right-biased attrset
//! merge, normalized/evaluable metadata, a closed unique-key generic closure,
//! and a disjoint sorted Nickel record merge side condition.  Rejection is
//! modeled as state-preserving except for explicitly read-only keys.

#![allow(clippy::ptr_arg, clippy::too_many_arguments)]

use std::collections::HashSet;

use vstd::prelude::*;

use crate::nickel_record_merge::ExecField;
use crate::nix_closure::ClosureItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixEvalClosurePolicyOutcome {
    Accepted {
        merged_attrs: Vec<(Vec<u8>, u64)>,
        closure: Vec<ClosureItem>,
        normalized_name: Vec<u8>,
    },
    Rejected {
        attrs: Vec<(Vec<u8>, u64)>,
    },
}

verus! {

pub open spec fn nix_eval_closure_policy_attr_values_spec(
    attrs: Seq<(Seq<u8>, u64)>,
) -> Seq<u64> {
    Seq::new(attrs.len(), |i: int| attrs[i].1)
}

pub open spec fn nix_eval_closure_policy_closure_keys_spec(
    closure: Seq<crate::nix_closure::ClosureItem>,
) -> Seq<u64> {
    Seq::new(closure.len(), |i: int| closure[i].key)
}

pub open spec fn nix_eval_closure_policy_key_in_attrs_spec(
    attrs: Seq<(Seq<u8>, u64)>,
    key: Seq<u8>,
) -> bool {
    crate::nix_attrset::get_attr_spec(attrs, key) is Some
}

pub open spec fn nix_eval_closure_policy_all_required_spec(
    required: Seq<u64>,
    closure: Seq<crate::nix_closure::ClosureItem>,
) -> bool {
    let keys = nix_eval_closure_policy_closure_keys_spec(closure);
    forall|i: int| 0 <= i < required.len()
        ==> crate::nix_list::elem_spec(keys, required[i])
}

pub open spec fn nix_eval_closure_policy_all_attrs_reachable_spec(
    merged_attrs: Seq<(Seq<u8>, u64)>,
    closure: Seq<crate::nix_closure::ClosureItem>,
) -> bool {
    let values = nix_eval_closure_policy_attr_values_spec(merged_attrs);
    let keys = nix_eval_closure_policy_closure_keys_spec(closure);
    forall|i: int| 0 <= i < values.len()
        ==> crate::nix_list::elem_spec(keys, values[i])
}

pub open spec fn nix_eval_closure_policy_contains_read_only_key_spec(
    read_only_keys: Seq<Seq<u8>>,
    key: Seq<u8>,
) -> bool {
    exists|i: int| 0 <= i < read_only_keys.len() && read_only_keys[i] == key
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub open spec fn nix_eval_closure_policy_accepted_spec(
    base_attrs: Seq<(Seq<u8>, u64)>,
    overlay_attrs: Seq<(Seq<u8>, u64)>,
    required_closure_keys: Seq<u64>,
    dependency_batches: Seq<Seq<u64>>,
    raw_name: Seq<u8>,
    substring_start: nat,
    substring_len: nat,
    replace_from: Seq<Seq<u8>>,
    replace_to: Seq<Seq<u8>>,
    version: Seq<u8>,
    minimum_version: Seq<u8>,
    start: Seq<crate::nix_closure::ClosureItem>,
    operator: Seq<(u64, Seq<crate::nix_closure::ClosureItem>)>,
    evaluated_closure: Seq<crate::nix_closure::ClosureItem>,
    left_record: Seq<crate::nickel_record_merge::Field>,
    right_record: Seq<crate::nickel_record_merge::Field>,
) -> bool {
    let merged_attrs = crate::nix_attrset::merge_spec(base_attrs, overlay_attrs);
    let normalized_name = crate::nix_string::replace_strings_spec(
        crate::nix_string::substring_spec(raw_name, substring_start, substring_len),
        replace_from,
        replace_to,
    );
    let dependency_list = crate::nix_list::concat_lists_spec(dependency_batches);
    &&& replace_from.len() == replace_to.len()
    &&& crate::nix_attrset::is_sorted_keys(base_attrs)
    &&& crate::nix_attrset::is_sorted_keys(overlay_attrs)
    &&& crate::nix_attrset::is_sorted_keys(merged_attrs)
    &&& crate::nix_list::length_spec(required_closure_keys) <= crate::nix_list::length_spec(dependency_list)
    &&& forall|i: int| 0 <= i < required_closure_keys.len()
        ==> crate::nix_list::elem_spec(dependency_list, required_closure_keys[i])
    &&& crate::nix_string::substring_spec(raw_name, substring_start, substring_len).len() <= raw_name.len()
    &&& crate::nix_string::replace_strings_spec(
        crate::nix_string::substring_spec(raw_name, substring_start, substring_len),
        replace_from,
        replace_to,
    ) == normalized_name
    &&& crate::nix_version::well_formed_version(version)
    &&& crate::nix_version::well_formed_version(minimum_version)
    &&& crate::nix_version::compare_versions_spec(version, minimum_version) >= 0
    &&& crate::nix_closure::generic_closure_spec(start, operator, evaluated_closure)
    &&& crate::nix_closure::keys_unique(evaluated_closure)
    &&& forall|i: int| 0 <= i < evaluated_closure.len()
        ==> crate::nix_closure::reachable_key_spec(start, operator, evaluated_closure[i].key)
    &&& nix_eval_closure_policy_all_required_spec(required_closure_keys, evaluated_closure)
    &&& nix_eval_closure_policy_all_attrs_reachable_spec(merged_attrs, evaluated_closure)
    &&& crate::nickel_record_merge::is_sorted_record(left_record)
    &&& crate::nickel_record_merge::is_sorted_record(right_record)
    &&& crate::nickel_record_merge::disjoint_keys(left_record, right_record)
    &&& crate::nickel_record_merge::record_merge_spec(left_record, right_record).len()
        == left_record.len() + right_record.len()
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub open spec fn nix_eval_closure_policy_reject_preserves_spec(
    before_attrs: Seq<(Seq<u8>, u64)>,
    after_attrs: Seq<(Seq<u8>, u64)>,
    read_only_exempt_keys: Seq<Seq<u8>>,
) -> bool {
    &&& before_attrs.len() == after_attrs.len()
    &&& forall|i: int| 0 <= i < before_attrs.len() ==> (
        #[trigger] after_attrs[i] == before_attrs[i] || (
            before_attrs[i].0 == after_attrs[i].0
            && nix_eval_closure_policy_contains_read_only_key_spec(read_only_exempt_keys, before_attrs[i].0)
        )
    )
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub proof fn nix_eval_closure_policy_refines_sources(
    base_attrs: Seq<(Seq<u8>, u64)>,
    overlay_attrs: Seq<(Seq<u8>, u64)>,
    required_closure_keys: Seq<u64>,
    dependency_batches: Seq<Seq<u64>>,
    raw_name: Seq<u8>,
    substring_start: nat,
    substring_len: nat,
    replace_from: Seq<Seq<u8>>,
    replace_to: Seq<Seq<u8>>,
    version: Seq<u8>,
    minimum_version: Seq<u8>,
    start: Seq<crate::nix_closure::ClosureItem>,
    operator: Seq<(u64, Seq<crate::nix_closure::ClosureItem>)>,
    evaluated_closure: Seq<crate::nix_closure::ClosureItem>,
    left_record: Seq<crate::nickel_record_merge::Field>,
    right_record: Seq<crate::nickel_record_merge::Field>,
)
    ensures
        nix_eval_closure_policy_accepted_spec(
            base_attrs,
            overlay_attrs,
            required_closure_keys,
            dependency_batches,
            raw_name,
            substring_start,
            substring_len,
            replace_from,
            replace_to,
            version,
            minimum_version,
            start,
            operator,
            evaluated_closure,
            left_record,
            right_record,
        ) ==> ({
            let merged_attrs = crate::nix_attrset::merge_spec(base_attrs, overlay_attrs);
            &&& crate::nix_attrset::is_sorted_keys(base_attrs)
            &&& crate::nix_attrset::is_sorted_keys(overlay_attrs)
            &&& crate::nix_attrset::is_sorted_keys(merged_attrs)
            &&& crate::nix_version::well_formed_version(version)
            &&& crate::nix_version::well_formed_version(minimum_version)
            &&& crate::nix_version::compare_versions_spec(version, minimum_version) >= 0
            &&& crate::nix_closure::generic_closure_spec(start, operator, evaluated_closure)
            &&& crate::nix_closure::keys_unique(evaluated_closure)
            &&& nix_eval_closure_policy_all_required_spec(required_closure_keys, evaluated_closure)
            &&& nix_eval_closure_policy_all_attrs_reachable_spec(merged_attrs, evaluated_closure)
            &&& crate::nickel_record_merge::is_sorted_record(left_record)
            &&& crate::nickel_record_merge::is_sorted_record(right_record)
            &&& crate::nickel_record_merge::disjoint_keys(left_record, right_record)
        }),
{
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub proof fn nix_eval_closure_policy_reject_refines_sources(
    attrs: Seq<(Seq<u8>, u64)>,
    read_only_exempt_keys: Seq<Seq<u8>>,
)
    ensures nix_eval_closure_policy_reject_preserves_spec(
        attrs,
        attrs,
        read_only_exempt_keys,
    ),
{
}

// r[verify composite-primitives.nix-eval-closure-policy]
pub proof fn nix_eval_closure_policy_missing_dependency_rejected(
    base_attrs: Seq<(Seq<u8>, u64)>,
    overlay_attrs: Seq<(Seq<u8>, u64)>,
    required_closure_keys: Seq<u64>,
    dependency_batches: Seq<Seq<u64>>,
    raw_name: Seq<u8>,
    substring_start: nat,
    substring_len: nat,
    replace_from: Seq<Seq<u8>>,
    replace_to: Seq<Seq<u8>>,
    version: Seq<u8>,
    minimum_version: Seq<u8>,
    start: Seq<crate::nix_closure::ClosureItem>,
    operator: Seq<(u64, Seq<crate::nix_closure::ClosureItem>)>,
    evaluated_closure: Seq<crate::nix_closure::ClosureItem>,
    left_record: Seq<crate::nickel_record_merge::Field>,
    right_record: Seq<crate::nickel_record_merge::Field>,
    missing_key: u64,
)
    requires
        crate::nix_list::elem_spec(required_closure_keys, missing_key),
        !crate::nix_list::elem_spec(nix_eval_closure_policy_closure_keys_spec(evaluated_closure), missing_key),
    ensures !nix_eval_closure_policy_accepted_spec(
        base_attrs,
        overlay_attrs,
        required_closure_keys,
        dependency_batches,
        raw_name,
        substring_start,
        substring_len,
        replace_from,
        replace_to,
        version,
        minimum_version,
        start,
        operator,
        evaluated_closure,
        left_record,
        right_record,
    ),
{
    let idx = choose|i: int| 0 <= i < required_closure_keys.len() && required_closure_keys[i] == missing_key;
    assert(0 <= idx < required_closure_keys.len());
}

// r[verify composite-primitives.nix-eval-closure-policy]
pub proof fn nix_eval_closure_policy_invalid_merge_rejected(
    base_attrs: Seq<(Seq<u8>, u64)>,
    overlay_attrs: Seq<(Seq<u8>, u64)>,
    required_closure_keys: Seq<u64>,
    dependency_batches: Seq<Seq<u64>>,
    raw_name: Seq<u8>,
    substring_start: nat,
    substring_len: nat,
    replace_from: Seq<Seq<u8>>,
    replace_to: Seq<Seq<u8>>,
    version: Seq<u8>,
    minimum_version: Seq<u8>,
    start: Seq<crate::nix_closure::ClosureItem>,
    operator: Seq<(u64, Seq<crate::nix_closure::ClosureItem>)>,
    evaluated_closure: Seq<crate::nix_closure::ClosureItem>,
    left_record: Seq<crate::nickel_record_merge::Field>,
    right_record: Seq<crate::nickel_record_merge::Field>,
)
    requires !crate::nix_attrset::is_sorted_keys(crate::nix_attrset::merge_spec(base_attrs, overlay_attrs)),
    ensures !nix_eval_closure_policy_accepted_spec(
        base_attrs,
        overlay_attrs,
        required_closure_keys,
        dependency_batches,
        raw_name,
        substring_start,
        substring_len,
        replace_from,
        replace_to,
        version,
        minimum_version,
        start,
        operator,
        evaluated_closure,
        left_record,
        right_record,
    ),
{
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub open spec fn nix_eval_closure_policy_outcome_alias_spec(accepted: bool) -> crate::composite_outcome::CompositeOutcomeTag {
    if accepted {
        crate::composite_outcome::CompositeOutcomeTag::Accept
    } else {
        crate::composite_outcome::CompositeOutcomeTag::Reject
    }
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub fn nix_eval_closure_policy_outcome_alias(accepted: bool) -> (outcome: crate::composite_outcome::CompositeOutcomeTag)
    ensures outcome == nix_eval_closure_policy_outcome_alias_spec(accepted)
{
    if accepted {
        crate::composite_outcome::composite_accept()
    } else {
        crate::composite_outcome::composite_reject()
    }
}

} // verus!

fn keys_unique_runtime(items: &[ClosureItem]) -> bool {
    let mut seen = HashSet::new();
    items.iter().all(|item| seen.insert(item.key))
}

fn contains_key_runtime(items: &[ClosureItem], key: u64) -> bool {
    items.iter().any(|item| item.key == key)
}

fn sorted_attrs_runtime(attrs: &[(Vec<u8>, u64)]) -> bool {
    let mut seen = HashSet::new();
    attrs.iter().all(|(key, _)| seen.insert(key.clone()))
}

fn sorted_record_runtime(record: &[ExecField]) -> bool {
    record.windows(2).all(|w| w[0].key < w[1].key)
}

fn disjoint_record_keys_runtime(left: &[ExecField], right: &[ExecField]) -> bool {
    left.iter().all(|l| right.iter().all(|r| l.key != r.key))
}

fn well_formed_version_runtime(version: &[u8]) -> bool {
    version.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'.' || *b == b'-')
}

// r[impl composite-primitives.nix-eval-closure-policy]
pub fn nix_eval_closure_policy(
    base_attrs: &Vec<(Vec<u8>, u64)>,
    overlay_attrs: &Vec<(Vec<u8>, u64)>,
    required_closure_keys: &Vec<u64>,
    dependency_batches: &Vec<Vec<u64>>,
    raw_name: &Vec<u8>,
    substring_start: usize,
    substring_len: usize,
    replace_from: &Vec<Vec<u8>>,
    replace_to: &Vec<Vec<u8>>,
    version: &Vec<u8>,
    minimum_version: &Vec<u8>,
    start: &Vec<ClosureItem>,
    operator: &Vec<(u64, Vec<ClosureItem>)>,
    left_record: &Vec<ExecField>,
    right_record: &Vec<ExecField>,
) -> NixEvalClosurePolicyOutcome {
    if replace_from.len() != replace_to.len()
        || !sorted_attrs_runtime(base_attrs)
        || !sorted_attrs_runtime(overlay_attrs)
        || !well_formed_version_runtime(version)
        || !well_formed_version_runtime(minimum_version)
        || !sorted_record_runtime(left_record)
        || !sorted_record_runtime(right_record)
        || !disjoint_record_keys_runtime(left_record, right_record)
    {
        return NixEvalClosurePolicyOutcome::Rejected {
            attrs: base_attrs.clone(),
        };
    }

    let merged_attrs = crate::nix_attrset::merge(base_attrs, overlay_attrs);
    if !sorted_attrs_runtime(&merged_attrs) {
        return NixEvalClosurePolicyOutcome::Rejected {
            attrs: base_attrs.clone(),
        };
    }

    let dependencies = crate::nix_list::concat_lists(dependency_batches);
    if required_closure_keys.len() > dependencies.len()
        || required_closure_keys.iter().any(|key| !dependencies.contains(key))
    {
        return NixEvalClosurePolicyOutcome::Rejected {
            attrs: base_attrs.clone(),
        };
    }

    if crate::nix_version::compare_versions(version, minimum_version) < 0 {
        return NixEvalClosurePolicyOutcome::Rejected {
            attrs: base_attrs.clone(),
        };
    }

    let trimmed_name = crate::nix_string::substring(raw_name, substring_start, substring_len);
    let normalized_name = crate::nix_string::replace_strings(&trimmed_name, replace_from, replace_to);
    let closure = crate::nix_closure::generic_closure(start, operator);

    if !keys_unique_runtime(&closure)
        || required_closure_keys.iter().any(|key| !contains_key_runtime(&closure, *key))
        || merged_attrs.iter().any(|(_, value)| !contains_key_runtime(&closure, *value))
    {
        return NixEvalClosurePolicyOutcome::Rejected {
            attrs: base_attrs.clone(),
        };
    }

    let _merged_record = crate::nickel_record_merge::record_merge(left_record, right_record);

    NixEvalClosurePolicyOutcome::Accepted {
        merged_attrs,
        closure,
        normalized_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nickel_priority::MergePriority;

    fn item(key: u64) -> ClosureItem {
        ClosureItem { key, data: key * 10 }
    }

    fn field(key: &[u8], value: u64) -> ExecField {
        ExecField {
            key: key.to_vec(),
            value,
            priority: MergePriority::Neutral,
        }
    }

    // r[verify composite-primitives.nix-eval-closure-policy]
    #[test]
    fn accepts_valid_merged_closure() {
        let base_attrs = vec![(b"dep-a".to_vec(), 1)];
        let overlay_attrs = vec![(b"dep-b".to_vec(), 2)];
        let required = vec![1, 2, 3];
        let batches = vec![vec![1, 2], vec![3]];
        let start = vec![item(1)];
        let operator = vec![(1, vec![item(2)]), (2, vec![item(3)])];
        let left_record = vec![field(b"a", 10)];
        let right_record = vec![field(b"b", 20)];

        let outcome = nix_eval_closure_policy(
            &base_attrs,
            &overlay_attrs,
            &required,
            &batches,
            &b"pkg-name".to_vec(),
            0,
            8,
            &vec![b"pkg".to_vec()],
            &vec![b"drv".to_vec()],
            &b"2.0".to_vec(),
            &b"1.0".to_vec(),
            &start,
            &operator,
            &left_record,
            &right_record,
        );

        match outcome {
            NixEvalClosurePolicyOutcome::Accepted {
                merged_attrs,
                closure,
                normalized_name,
            } => {
                assert_eq!(merged_attrs.len(), 2);
                assert!(closure.iter().any(|i| i.key == 3));
                assert_eq!(normalized_name, b"drv-name".to_vec());
            }
            other @ NixEvalClosurePolicyOutcome::Rejected { .. } => panic!("expected accepted outcome, got {other:?}"),
        }
    }

    // r[verify composite-primitives.nix-eval-closure-policy]
    #[test]
    fn rejects_invalid_merge_duplicate_overlay_key() {
        let base_attrs = vec![(b"dep-a".to_vec(), 1)];
        let overlay_attrs = vec![(b"dep-b".to_vec(), 2), (b"dep-b".to_vec(), 3)];
        let outcome = nix_eval_closure_policy(
            &base_attrs,
            &overlay_attrs,
            &vec![1],
            &vec![vec![1]],
            &b"pkg".to_vec(),
            0,
            3,
            &vec![],
            &vec![],
            &b"1.0".to_vec(),
            &b"1.0".to_vec(),
            &vec![item(1)],
            &vec![],
            &vec![field(b"a", 10)],
            &vec![field(b"b", 20)],
        );
        assert_eq!(outcome, NixEvalClosurePolicyOutcome::Rejected { attrs: base_attrs });
    }

    // r[verify composite-primitives.nix-eval-closure-policy]
    #[test]
    fn rejects_missing_dependency() {
        let base_attrs = vec![(b"dep-a".to_vec(), 1)];
        let overlay_attrs = vec![(b"dep-b".to_vec(), 2)];
        let outcome = nix_eval_closure_policy(
            &base_attrs,
            &overlay_attrs,
            &vec![1, 2, 99],
            &vec![vec![1, 2, 99]],
            &b"pkg".to_vec(),
            0,
            3,
            &vec![],
            &vec![],
            &b"1.0".to_vec(),
            &b"1.0".to_vec(),
            &vec![item(1)],
            &vec![(1, vec![item(2)])],
            &vec![field(b"a", 10)],
            &vec![field(b"b", 20)],
        );
        assert_eq!(outcome, NixEvalClosurePolicyOutcome::Rejected { attrs: base_attrs });
    }
}
