//! Policy decision engine facade.
//!
//! Validity boundary: allow is returned only when the delegation chain is
//! structurally valid, containment/scope checks hold, glob and integer-pattern
//! inputs match, authorization succeeds, and resolved trust permits the resource.
//! Malformed delegation chains reject; scope, pattern, and trust mismatches are
//! ordinary deny results.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDecisionEngineOutcome {
    Allow,
    Deny,
}

verus! {

// r[impl composite-primitives.policy-decision-engine]
pub open spec fn policy_decision_engine_spec(
    chain: Seq<crate::delegation::ChainLevel>,
    cap_variant: u8,
    cap_scope: Seq<u8>,
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    cap_mode: u8,
    child_variant: u8,
    child_scope: Seq<u8>,
    child_mount: Seq<u8>,
    child_prefix: Seq<u8>,
    child_mode: u8,
    op_variant: u8,
    op_target: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
    op_mode: u8,
    glob_pattern: Seq<u8>,
    glob_input: Seq<u8>,
    allowed_pattern: Seq<int>,
    requested_pattern: Seq<int>,
    requested_item: int,
    in_blocked: bool,
    in_trusted: bool,
    resource_is_public: bool,
) -> bool {
    crate::delegation::chain_fully_valid_spec(chain)
    && crate::containment::contains_spec(
        cap_variant, cap_scope, cap_mode,
        child_variant, child_scope, child_mode,
    )
    && crate::containment::contains_compound_spec(
        cap_variant, cap_mount, cap_prefix, cap_mode,
        child_variant, child_mount, child_prefix, child_mode,
    )
    && crate::scope::covers_spec(child_scope, op_target)
    && crate::scope::compound_covers_spec(child_mount, child_prefix, op_mount, op_path)
    && crate::authorize::authorizes_spec(
        child_variant, child_scope, child_mode,
        op_variant, op_target, op_mode,
    )
    && crate::authorize::authorizes_compound_spec(
        child_variant, child_mount, child_prefix, child_mode,
        op_variant, op_mount, op_path, op_mode,
    )
    && crate::glob::glob_match_spec(glob_pattern, glob_input)
    && crate::pattern::pattern_contains_spec(allowed_pattern, requested_pattern)
    && crate::pattern::match_pattern_spec(requested_pattern, requested_item)
    && crate::trust_level::can_access_spec(
        crate::trust_level::resolve_trust_spec(in_blocked, in_trusted),
        resource_is_public,
    )
}

// r[verify composite-primitives.policy-decision-engine]
pub proof fn policy_decision_engine_refines_sources(
    chain: Seq<crate::delegation::ChainLevel>,
    cap_variant: u8,
    cap_scope: Seq<u8>,
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    cap_mode: u8,
    child_variant: u8,
    child_scope: Seq<u8>,
    child_mount: Seq<u8>,
    child_prefix: Seq<u8>,
    child_mode: u8,
    op_variant: u8,
    op_target: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
    op_mode: u8,
    glob_pattern: Seq<u8>,
    glob_input: Seq<u8>,
    allowed_pattern: Seq<int>,
    requested_pattern: Seq<int>,
    requested_item: int,
    in_blocked: bool,
    in_trusted: bool,
    resource_is_public: bool,
)
    ensures
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::delegation::chain_fully_valid_spec(chain),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::containment::contains_spec(
            cap_variant, cap_scope, cap_mode,
            child_variant, child_scope, child_mode,
        ),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::scope::covers_spec(child_scope, op_target),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::authorize::authorizes_spec(
            child_variant, child_scope, child_mode,
            op_variant, op_target, op_mode,
        ),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::glob::glob_match_spec(glob_pattern, glob_input),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::pattern::pattern_contains_spec(allowed_pattern, requested_pattern),
        policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ) ==> crate::trust_level::can_access_spec(
            crate::trust_level::resolve_trust_spec(in_blocked, in_trusted),
            resource_is_public,
        ),
{
}

// r[verify composite-primitives.policy-decision-engine]
pub proof fn policy_decision_engine_rejects_mismatches(
    chain: Seq<crate::delegation::ChainLevel>,
    cap_variant: u8,
    cap_scope: Seq<u8>,
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    cap_mode: u8,
    child_variant: u8,
    child_scope: Seq<u8>,
    child_mount: Seq<u8>,
    child_prefix: Seq<u8>,
    child_mode: u8,
    op_variant: u8,
    op_target: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
    op_mode: u8,
    glob_pattern: Seq<u8>,
    glob_input: Seq<u8>,
    allowed_pattern: Seq<int>,
    requested_pattern: Seq<int>,
    requested_item: int,
    in_blocked: bool,
    in_trusted: bool,
    resource_is_public: bool,
)
    ensures
        (!crate::delegation::chain_fully_valid_spec(chain)
            || !crate::scope::covers_spec(child_scope, op_target)
            || !crate::glob::glob_match_spec(glob_pattern, glob_input)
            || !crate::pattern::match_pattern_spec(requested_pattern, requested_item)
            || !crate::trust_level::can_access_spec(
                crate::trust_level::resolve_trust_spec(in_blocked, in_trusted),
                resource_is_public,
            )) ==> !policy_decision_engine_spec(
            chain, cap_variant, cap_scope, cap_mount, cap_prefix, cap_mode,
            child_variant, child_scope, child_mount, child_prefix, child_mode,
            op_variant, op_target, op_mount, op_path, op_mode,
            glob_pattern, glob_input, allowed_pattern, requested_pattern, requested_item,
            in_blocked, in_trusted, resource_is_public,
        ),
{
}

} // verus!

// r[impl composite-primitives.policy-decision-engine]
#[allow(clippy::too_many_arguments)]
pub fn policy_decision_engine(
    chain: &[crate::delegation::ChainLevel],
    cap_variant: u8,
    cap_scope: &[u8],
    cap_mount: &[u8],
    cap_prefix: &[u8],
    cap_mode: u8,
    child_variant: u8,
    child_scope: &[u8],
    child_mount: &[u8],
    child_prefix: &[u8],
    child_mode: u8,
    op_variant: u8,
    op_target: &[u8],
    op_mount: &[u8],
    op_path: &[u8],
    op_mode: u8,
    glob_pattern: &[u8],
    glob_input: &[u8],
    allowed_pattern: &[i64],
    requested_pattern: &[i64],
    requested_item: i64,
    in_blocked: bool,
    in_trusted: bool,
    resource_is_public: bool,
) -> PolicyDecisionEngineOutcome {
    if !crate::delegation::chain_valid(chain) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::containment::contains(cap_variant, cap_scope, cap_mode, child_variant, child_scope, child_mode) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::containment::contains_compound(
        cap_variant,
        cap_mount,
        cap_prefix,
        cap_mode,
        child_variant,
        child_mount,
        child_prefix,
        child_mode,
    ) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::scope::covers(child_scope, op_target) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::scope::compound_covers(child_mount, child_prefix, op_mount, op_path) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::authorize::authorizes(child_variant, child_scope, child_mode, op_variant, op_target, op_mode) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::authorize::authorizes_compound(
        child_variant,
        child_mount,
        child_prefix,
        child_mode,
        op_variant,
        op_mount,
        op_path,
        op_mode,
    ) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::glob::glob_match(glob_pattern, glob_input) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::pattern::pattern_contains(allowed_pattern, requested_pattern) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    if !crate::pattern::match_pattern(requested_pattern, requested_item) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    let trust = crate::trust_level::resolve_trust(in_blocked, in_trusted);
    if !crate::trust_level::can_access(trust, resource_is_public) {
        return PolicyDecisionEngineOutcome::Deny;
    }
    PolicyDecisionEngineOutcome::Allow
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegation::ChainLevel;

    fn valid_chain() -> Vec<ChainLevel> {
        vec![
            ChainLevel {
                depth: 0,
                has_delegate: true,
                variant: 4,
                mode: 3,
            },
            ChainLevel {
                depth: 1,
                has_delegate: false,
                variant: 0,
                mode: 0,
            },
        ]
    }

    fn decision(
        chain: &[ChainLevel],
        child_scope: &[u8],
        op_target: &[u8],
        glob_pattern: &[u8],
        glob_input: &[u8],
        requested_pattern: &[i64],
        requested_item: i64,
        in_blocked: bool,
        in_trusted: bool,
        resource_is_public: bool,
    ) -> PolicyDecisionEngineOutcome {
        policy_decision_engine(
            chain,
            4,
            b"/tenant",
            b"fs",
            b"/tenant",
            3,
            0,
            child_scope,
            b"fs",
            b"/tenant/app",
            0,
            0,
            op_target,
            b"fs",
            b"/tenant/app/data",
            0,
            glob_pattern,
            glob_input,
            &[-1],
            requested_pattern,
            requested_item,
            in_blocked,
            in_trusted,
            resource_is_public,
        )
    }

    #[test]
    fn contained_delegation_allows_scoped_action() {
        let chain = valid_chain();
        assert_eq!(
            decision(
                &chain,
                b"/tenant/app",
                b"/tenant/app/data",
                b"tenant/*",
                b"tenant/app",
                &[7],
                7,
                false,
                true,
                false,
            ),
            PolicyDecisionEngineOutcome::Allow,
        );
    }

    #[test]
    fn scope_pattern_and_trust_mismatches_deny() {
        let chain = valid_chain();
        assert_eq!(
            decision(
                &chain,
                b"/tenant/app",
                b"/other/app/data",
                b"tenant/*",
                b"tenant/app",
                &[7],
                7,
                false,
                true,
                false,
            ),
            PolicyDecisionEngineOutcome::Deny,
        );
        assert_eq!(
            decision(
                &chain,
                b"/tenant/app",
                b"/tenant/app/data",
                b"tenant/*",
                b"other/app",
                &[7],
                7,
                false,
                true,
                false,
            ),
            PolicyDecisionEngineOutcome::Deny,
        );
        assert_eq!(
            decision(
                &chain,
                b"/tenant/app",
                b"/tenant/app/data",
                b"tenant/*",
                b"tenant/app",
                &[7],
                9,
                false,
                true,
                false,
            ),
            PolicyDecisionEngineOutcome::Deny,
        );
        assert_eq!(
            decision(
                &chain,
                b"/tenant/app",
                b"/tenant/app/data",
                b"tenant/*",
                b"tenant/app",
                &[7],
                7,
                true,
                true,
                false,
            ),
            PolicyDecisionEngineOutcome::Deny,
        );
    }
}
