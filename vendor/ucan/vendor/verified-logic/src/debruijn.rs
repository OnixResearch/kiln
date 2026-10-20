//! Verified de Bruijn shifting, substitution, and beta reduction.
//!
//! The proof-facing core lives over a pure ghost syntax (`SpecTerm`) with
//! unbounded indices. The runtime shell uses `u32` indices and returns `None`
//! when a mathematically valid result would exceed the executable index space.
//! This keeps the core algebra clean while making overflow explicit at the API
//! boundary.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

/// Number of representable runtime de Bruijn indices.
pub open spec fn index_domain_size() -> nat {
    u32::MAX as nat + 1
}

#[cfg_attr(not(verus_keep_ghost), derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub enum Term {
    Var(u32),
    Abs(Box<Term>),
    App(Box<Term>, Box<Term>),
}

/// Ghost syntax with unbounded indices for the mathematical core.
pub enum SpecTerm {
    Var(nat),
    Abs(Box<SpecTerm>),
    App(Box<SpecTerm>, Box<SpecTerm>),
}

pub open spec fn term_view(term: Term) -> SpecTerm
    decreases term
{
    match term {
        Term::Var(index) => SpecTerm::Var(index as nat),
        Term::Abs(body) => SpecTerm::Abs(Box::new(term_view(*body))),
        Term::App(fun, arg) => {
            SpecTerm::App(Box::new(term_view(*fun)), Box::new(term_view(*arg)))
        }
    }
}

pub open spec fn spec_term_size(term: SpecTerm) -> nat
    decreases term
{
    match term {
        SpecTerm::Var(_) => 1,
        SpecTerm::Abs(body) => 1 + spec_term_size(*body),
        SpecTerm::App(fun, arg) => 1 + spec_term_size(*fun) + spec_term_size(*arg),
    }
}

pub open spec fn spec_term_in_bounds(term: SpecTerm) -> bool
    decreases term
{
    match term {
        SpecTerm::Var(index) => index < index_domain_size(),
        SpecTerm::Abs(body) => spec_term_in_bounds(*body),
        SpecTerm::App(fun, arg) => spec_term_in_bounds(*fun) && spec_term_in_bounds(*arg),
    }
}

// r[depends debruijn.well-scoped]
// r[impl debruijn.well-scoped]
/// A term is well scoped at `depth` when every variable index is strictly
/// smaller than the number of enclosing binders in scope.
pub open spec fn well_scoped_spec(depth: nat, term: SpecTerm) -> bool
    decreases term
{
    match term {
        SpecTerm::Var(index) => index < depth,
        SpecTerm::Abs(body) => well_scoped_spec(depth + 1, *body),
        SpecTerm::App(fun, arg) => {
            well_scoped_spec(depth, *fun) && well_scoped_spec(depth, *arg)
        }
    }
}

// r[depends debruijn.shift-zero]
// r[depends debruijn.shift-compose]
// r[depends debruijn.shift-preserves-scope]
pub open spec fn shift_spec(delta: nat, cutoff: nat, term: SpecTerm) -> SpecTerm
    decreases term
{
    match term {
        SpecTerm::Var(index) => {
            if index < cutoff {
                SpecTerm::Var(index)
            } else {
                SpecTerm::Var(index + delta)
            }
        }
        SpecTerm::Abs(body) => SpecTerm::Abs(Box::new(shift_spec(delta, cutoff + 1, *body))),
        SpecTerm::App(fun, arg) => SpecTerm::App(
            Box::new(shift_spec(delta, cutoff, *fun)),
            Box::new(shift_spec(delta, cutoff, *arg)),
        ),
    }
}

// r[depends debruijn.identity-substitution]
// r[depends debruijn.subst-preserves-scope]
pub open spec fn subst_spec(index: nat, replacement: SpecTerm, term: SpecTerm) -> SpecTerm
    decreases term
{
    match term {
        SpecTerm::Var(var_index) => {
            if var_index < index {
                SpecTerm::Var(var_index)
            } else if var_index == index {
                replacement
            } else {
                SpecTerm::Var((var_index - 1) as nat)
            }
        }
        SpecTerm::Abs(body) => SpecTerm::Abs(Box::new(subst_spec(
            index + 1,
            shift_spec(1, 0, replacement),
            *body,
        ))),
        SpecTerm::App(fun, arg) => SpecTerm::App(
            Box::new(subst_spec(index, replacement, *fun)),
            Box::new(subst_spec(index, replacement, *arg)),
        ),
    }
}

// r[depends debruijn.beta-scope]
pub open spec fn is_beta_redex_spec(term: SpecTerm) -> bool {
    match term {
        SpecTerm::App(fun, _arg) => {
            match *fun {
                SpecTerm::Abs(_) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

// r[impl debruijn.beta-scope]
pub open spec fn beta_reduce_spec(term: SpecTerm) -> Option<SpecTerm> {
    match term {
        SpecTerm::App(fun, arg) => {
            match *fun {
                SpecTerm::Abs(body) => Some(subst_spec(0, *arg, *body)),
                _ => None,
            }
        }
        _ => None,
    }
}

// r[verify debruijn.well-scoped]
pub proof fn well_scoped_var_valid(depth: nat, index: nat)
    requires index < depth
    ensures well_scoped_spec(depth, SpecTerm::Var(index))
{}

// r[verify debruijn.well-scoped]
pub proof fn well_scoped_var_invalid(depth: nat, index: nat)
    requires depth <= index
    ensures !well_scoped_spec(depth, SpecTerm::Var(index))
{}

// r[verify debruijn.well-scoped]
pub proof fn well_scoped_abs_body(depth: nat, body: SpecTerm)
    ensures
        well_scoped_spec(depth, SpecTerm::Abs(Box::new(body)))
            == well_scoped_spec(depth + 1, body)
{}

// r[verify debruijn.well-scoped]
pub proof fn well_scoped_app_parts(depth: nat, fun: SpecTerm, arg: SpecTerm)
    ensures
        well_scoped_spec(depth, SpecTerm::App(Box::new(fun), Box::new(arg)))
            == (well_scoped_spec(depth, fun) && well_scoped_spec(depth, arg))
{}

// r[verify debruijn.shift-zero]
pub proof fn shift_zero_identity(term: SpecTerm, cutoff: nat)
    ensures shift_spec(0, cutoff, term) == term
    decreases term
{
    match term {
        SpecTerm::Var(index) => {
            if index < cutoff {
            } else {
                assert(index + 0 == index);
            }
        }
        SpecTerm::Abs(body) => {
            shift_zero_identity(*body, cutoff + 1);
        }
        SpecTerm::App(fun, arg) => {
            shift_zero_identity(*fun, cutoff);
            shift_zero_identity(*arg, cutoff);
        }
    }
}

// r[verify debruijn.shift-compose]
pub proof fn shift_composition(term: SpecTerm, first: nat, second: nat, cutoff: nat)
    ensures
        shift_spec(second, cutoff, shift_spec(first, cutoff, term))
            == shift_spec(first + second, cutoff, term)
    decreases term
{
    match term {
        SpecTerm::Var(index) => {
            if index < cutoff {
            } else {
                assert((index + first) + second == index + (first + second)) by (nonlinear_arith);
            }
        }
        SpecTerm::Abs(body) => {
            shift_composition(*body, first, second, cutoff + 1);
        }
        SpecTerm::App(fun, arg) => {
            shift_composition(*fun, first, second, cutoff);
            shift_composition(*arg, first, second, cutoff);
        }
    }
}

// r[verify debruijn.shift-preserves-scope]
pub proof fn shift_preserves_scope(term: SpecTerm, depth: nat, delta: nat, cutoff: nat)
    requires
        cutoff <= depth,
        well_scoped_spec(depth, term),
    ensures
        well_scoped_spec(depth + delta, shift_spec(delta, cutoff, term))
    decreases term
{
    match term {
        SpecTerm::Var(index) => {
            if index < cutoff {
                assert(index < depth + delta);
            } else {
                assert(index + delta < depth + delta) by (nonlinear_arith)
                    requires index < depth;
            }
        }
        SpecTerm::Abs(body) => {
            shift_preserves_scope(*body, depth + 1, delta, cutoff + 1);
        }
        SpecTerm::App(fun, arg) => {
            shift_preserves_scope(*fun, depth, delta, cutoff);
            shift_preserves_scope(*arg, depth, delta, cutoff);
        }
    }
}

// r[verify debruijn.identity-substitution]
pub proof fn subst_var_cancels_lift(term: SpecTerm, cutoff: nat)
    ensures
        subst_spec(cutoff, SpecTerm::Var(cutoff), shift_spec(1, cutoff, term)) == term
    decreases term
{
    match term {
        SpecTerm::Var(index) => {
            if index < cutoff {
            } else {
                assert(index + 1 > cutoff) by (nonlinear_arith)
                    requires index >= cutoff;
                assert((index + 1) - 1 == index) by (nonlinear_arith)
                    requires index + 1 > 0nat;
            }
        }
        SpecTerm::Abs(body) => {
            subst_var_cancels_lift(*body, cutoff + 1);
        }
        SpecTerm::App(fun, arg) => {
            subst_var_cancels_lift(*fun, cutoff);
            subst_var_cancels_lift(*arg, cutoff);
        }
    }
}

// r[verify debruijn.subst-preserves-scope]
pub proof fn subst_preserves_scope(
    term: SpecTerm,
    replacement: SpecTerm,
    index: nat,
    context_depth: nat,
)
    requires
        index <= context_depth,
        well_scoped_spec(context_depth + 1, term),
        well_scoped_spec(context_depth, replacement),
    ensures
        well_scoped_spec(context_depth, subst_spec(index, replacement, term))
    decreases term
{
    match term {
        SpecTerm::Var(var_index) => {
            if var_index < index {
                assert(var_index < context_depth) by (nonlinear_arith)
                    requires var_index < index, index <= context_depth;
            } else if var_index == index {
            } else {
                assert(var_index - 1 < context_depth) by (nonlinear_arith)
                    requires var_index > index, var_index < context_depth + 1;
            }
        }
        SpecTerm::Abs(body) => {
            shift_preserves_scope(replacement, context_depth, 1, 0);
            subst_preserves_scope(*body, shift_spec(1, 0, replacement), index + 1, context_depth + 1);
        }
        SpecTerm::App(fun, arg) => {
            subst_preserves_scope(*fun, replacement, index, context_depth);
            subst_preserves_scope(*arg, replacement, index, context_depth);
        }
    }
}

// r[verify debruijn.beta-scope]
pub proof fn beta_reduce_preserves_scope(body: SpecTerm, arg: SpecTerm, depth: nat)
    requires
        well_scoped_spec(depth + 1, body),
        well_scoped_spec(depth, arg),
    ensures
        beta_reduce_spec(SpecTerm::App(Box::new(SpecTerm::Abs(Box::new(body))), Box::new(arg)))
            == Some(subst_spec(0, arg, body)),
        well_scoped_spec(depth, subst_spec(0, arg, body)),
{
    subst_preserves_scope(body, arg, 0, depth);
}

// r[verify debruijn.beta-scope]
pub proof fn beta_reduce_non_redex_none(term: SpecTerm)
    requires !is_beta_redex_spec(term)
    ensures beta_reduce_spec(term).is_none()
{
    match term {
        SpecTerm::App(fun, _arg) => {
            match *fun {
                SpecTerm::Abs(_) => {
                    assert(false);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn clone_term(term: &Term) -> (result: Term)
    ensures term_view(result) == term_view(*term)
    decreases spec_term_size(term_view(*term))
{
    match term {
        Term::Var(index) => Term::Var(*index),
        Term::Abs(body) => {
            let cloned_body = clone_term(body);
            Term::Abs(Box::new(cloned_body))
        }
        Term::App(fun, arg) => {
            let cloned_fun = clone_term(fun);
            let cloned_arg = clone_term(arg);
            Term::App(Box::new(cloned_fun), Box::new(cloned_arg))
        }
    }
}

// r[impl debruijn.shift-zero]
// r[impl debruijn.shift-compose]
// r[impl debruijn.shift-preserves-scope]
pub fn shift(delta: u32, cutoff: u64, term: &Term) -> (result: Option<Term>)
    ensures
        match result {
            Some(shifted) => {
                term_view(shifted) == shift_spec(delta as nat, cutoff as nat, term_view(*term))
            }
            None => true,
        },
    decreases spec_term_size(term_view(*term))
{
    match term {
        Term::Var(index) => {
            if (*index as u64) < cutoff {
                Some(Term::Var(*index))
            } else {
                match index.checked_add(delta) {
                    Some(shifted) => Some(Term::Var(shifted)),
                    None => None,
                }
            }
        }
        Term::Abs(body) => {
            match cutoff.checked_add(1) {
                Some(next_cutoff) => {
                    match shift(delta, next_cutoff, body) {
                        Some(shifted_body) => Some(Term::Abs(Box::new(shifted_body))),
                        None => None,
                    }
                }
                None => None,
            }
        }
        Term::App(fun, arg) => {
            match shift(delta, cutoff, fun) {
                Some(shifted_fun) => {
                    match shift(delta, cutoff, arg) {
                        Some(shifted_arg) => Some(Term::App(Box::new(shifted_fun), Box::new(shifted_arg))),
                        None => None,
                    }
                }
                None => None,
            }
        }
    }
}

// r[impl debruijn.identity-substitution]
// r[impl debruijn.subst-preserves-scope]
pub fn subst(index: u64, replacement: &Term, term: &Term) -> (result: Option<Term>)
    ensures
        match result {
            Some(substituted) => {
                term_view(substituted) == subst_spec(index as nat, term_view(*replacement), term_view(*term))
            }
            None => true,
        },
    decreases spec_term_size(term_view(*term))
{
    match term {
        Term::Var(var_index) => {
            if (*var_index as u64) < index {
                Some(Term::Var(*var_index))
            } else if (*var_index as u64) == index {
                Some(clone_term(replacement))
            } else {
                Some(Term::Var(*var_index - 1))
            }
        }
        Term::Abs(body) => {
            match shift(1, 0, replacement) {
                Some(lifted_replacement) => {
                    match index.checked_add(1) {
                        Some(next_index) => {
                            match subst(next_index, &lifted_replacement, body) {
                                Some(substituted_body) => Some(Term::Abs(Box::new(substituted_body))),
                                None => None,
                            }
                        }
                        None => None,
                    }
                }
                None => None,
            }
        }
        Term::App(fun, arg) => {
            match subst(index, replacement, fun) {
                Some(substituted_fun) => {
                    match subst(index, replacement, arg) {
                        Some(substituted_arg) => Some(Term::App(Box::new(substituted_fun), Box::new(substituted_arg))),
                        None => None,
                    }
                }
                None => None,
            }
        }
    }
}

// r[impl debruijn.beta-scope]
pub fn beta_reduce(term: &Term) -> (result: Option<Term>)
    ensures
        match result {
            Some(reduced) => beta_reduce_spec(term_view(*term)) == Some(term_view(reduced)),
            None => true,
        },
{
    match term {
        Term::App(fun, arg) => {
            proof {
                assert(term_view(*term) == SpecTerm::App(Box::new(term_view(**fun)), Box::new(term_view(**arg))));
            }
            match &**fun {
                Term::Abs(body) => {
                    proof {
                        assert(term_view(**fun) == SpecTerm::Abs(Box::new(term_view(**body))));
                        assert(beta_reduce_spec(term_view(*term)) == Some(subst_spec(0, term_view(**arg), term_view(**body))));
                    }
                    subst(0, arg, body)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const GENERATOR_MAX_DEPTH: u32 = 4;
    const GENERATOR_MAX_INDEX: u32 = 6;

    fn runtime_is_well_scoped(depth: u64, term: &Term) -> bool {
        match term {
            Term::Var(index) => (*index as u64) < depth,
            Term::Abs(body) => runtime_is_well_scoped(depth + 1, body),
            Term::App(fun, arg) => runtime_is_well_scoped(depth, fun) && runtime_is_well_scoped(depth, arg),
        }
    }

    fn arb_term(max_depth: u32, max_index: u32) -> BoxedStrategy<Term> {
        let leaf = (0..=max_index).prop_map(Term::Var).boxed();
        leaf.prop_recursive(max_depth, 32, 2, move |inner| {
            prop_oneof![
                inner.clone().prop_map(|body| Term::Abs(Box::new(body))),
                (inner.clone(), inner).prop_map(|(fun, arg)| Term::App(Box::new(fun), Box::new(arg))),
            ]
        })
        .boxed()
    }

    fn arb_well_scoped_term(depth: u64, fuel: u32) -> BoxedStrategy<Term> {
        let max_var = if depth == 0 {
            0
        } else {
            match u32::try_from(depth - 1) {
                Ok(value) => value,
                Err(_) => u32::MAX,
            }
        };
        let leaf = if depth == 0 {
            Just(Term::Var(0)).boxed()
        } else {
            (0..=max_var).prop_map(Term::Var).boxed()
        };
        leaf.prop_recursive(fuel, 32, 2, move |inner| {
            let abs =
                arb_well_scoped_term(depth + 1, fuel.saturating_sub(1)).prop_map(|body| Term::Abs(Box::new(body)));
            let app = (inner.clone(), inner).prop_map(|(fun, arg)| Term::App(Box::new(fun), Box::new(arg)));
            prop_oneof![abs, app]
        })
        .boxed()
    }

    // r[verify debruijn.well-scoped]
    #[test]
    fn test_runtime_scope_examples() {
        assert!(runtime_is_well_scoped(1, &Term::Var(0)));
        assert!(!runtime_is_well_scoped(1, &Term::Var(1)));
        assert!(runtime_is_well_scoped(0, &Term::Abs(Box::new(Term::Var(0)))));
    }

    // r[verify debruijn.shift-zero]
    #[test]
    fn test_shift_zero_identity_runtime() {
        let term = Term::App(Box::new(Term::Abs(Box::new(Term::Var(0)))), Box::new(Term::Var(2)));
        assert_eq!(shift(0, 0, &term), Some(term));
    }

    // r[verify debruijn.shift-preserves-scope]
    #[test]
    fn test_shift_under_binder() {
        let term = Term::Abs(Box::new(Term::App(Box::new(Term::Var(0)), Box::new(Term::Var(2)))));
        let expected = Term::Abs(Box::new(Term::App(Box::new(Term::Var(0)), Box::new(Term::Var(3)))));
        assert_eq!(shift(1, 0, &term), Some(expected));
    }

    // r[verify debruijn.identity-substitution]
    #[test]
    fn test_subst_top_cancels_lift_runtime() {
        let term = Term::Abs(Box::new(Term::App(Box::new(Term::Var(1)), Box::new(Term::Var(0)))));
        let lifted = shift(1, 0, &term).unwrap();
        assert_eq!(subst(0, &Term::Var(0), &lifted), Some(term));
    }

    // r[verify debruijn.beta-scope]
    #[test]
    fn test_beta_reduce_identity() {
        let redex = Term::App(Box::new(Term::Abs(Box::new(Term::Var(0)))), Box::new(Term::Abs(Box::new(Term::Var(0)))));
        assert_eq!(beta_reduce(&redex), Some(Term::Abs(Box::new(Term::Var(0)))));
    }

    // r[verify debruijn.beta-scope]
    #[test]
    fn test_beta_reduce_nested_binder() {
        let redex = Term::App(
            Box::new(Term::Abs(Box::new(Term::Abs(Box::new(Term::Var(1)))))),
            Box::new(Term::Abs(Box::new(Term::Var(0)))),
        );
        let expected = Term::Abs(Box::new(Term::Abs(Box::new(Term::Var(0)))));
        assert_eq!(beta_reduce(&redex), Some(expected));
    }

    // r[verify debruijn.shift-zero]
    #[test]
    fn test_shift_overflow_returns_none() {
        assert_eq!(shift(1, 0, &Term::Var(u32::MAX)), None);
    }

    // r[verify debruijn.subst-preserves-scope]
    #[test]
    fn test_subst_overflow_returns_none() {
        let body = Term::Abs(Box::new(Term::Var(1)));
        let replacement = Term::Var(u32::MAX);
        assert_eq!(subst(0, &replacement, &body), None);
    }

    // r[verify debruijn.beta-scope]
    #[test]
    fn test_beta_reduce_non_redex_returns_none() {
        assert_eq!(beta_reduce(&Term::Var(0)), None);
        assert_eq!(beta_reduce(&Term::Abs(Box::new(Term::Var(0)))), None);
    }

    proptest! {
        // r[verify debruijn.shift-zero]
        #[test]
        fn prop_shift_zero_identity(term in arb_term(GENERATOR_MAX_DEPTH, GENERATOR_MAX_INDEX)) {
            prop_assert_eq!(shift(0, 0, &term), Some(term));
        }

        // r[verify debruijn.shift-compose]
        #[test]
        fn prop_shift_composition(
            term in arb_term(GENERATOR_MAX_DEPTH, GENERATOR_MAX_INDEX),
            cutoff in 0u64..4,
            first in 0u32..3,
            second in 0u32..3,
        ) {
            let left = shift(first, cutoff, &term)
                .and_then(|mid| shift(second, cutoff, &mid));
            let right = shift((first + second) as u32, cutoff, &term);
            prop_assert_eq!(left, right);
        }

        // r[verify debruijn.shift-preserves-scope]
        #[test]
        fn prop_shift_preserves_runtime_scope(
            depth in 1u64..4,
            term in arb_well_scoped_term(1, GENERATOR_MAX_DEPTH),
        ) {
            if runtime_is_well_scoped(depth, &term) {
                if let Some(shifted) = shift(1, 0, &term) {
                    prop_assert!(runtime_is_well_scoped(depth + 1, &shifted));
                }
            }
        }

        // r[verify debruijn.subst-preserves-scope]
        #[test]
        fn prop_subst_preserves_runtime_scope(
            depth in 1u64..4,
            body in arb_well_scoped_term(2, GENERATOR_MAX_DEPTH),
            replacement in arb_well_scoped_term(1, GENERATOR_MAX_DEPTH),
        ) {
            if runtime_is_well_scoped(depth + 1, &body) && runtime_is_well_scoped(depth, &replacement) {
                if let Some(substituted) = subst(0, &replacement, &body) {
                    prop_assert!(runtime_is_well_scoped(depth, &substituted));
                }
            }
        }

        // r[verify debruijn.beta-scope]
        #[test]
        fn prop_beta_reduce_preserves_runtime_scope(
            depth in 1u64..4,
            body in arb_well_scoped_term(2, GENERATOR_MAX_DEPTH),
            arg in arb_well_scoped_term(1, GENERATOR_MAX_DEPTH),
        ) {
            if runtime_is_well_scoped(depth + 1, &body) && runtime_is_well_scoped(depth, &arg) {
                let redex = Term::App(Box::new(Term::Abs(Box::new(body))), Box::new(arg));
                if let Some(reduced) = beta_reduce(&redex) {
                    prop_assert!(runtime_is_well_scoped(depth, &reduced));
                }
            }
        }
    }
}
