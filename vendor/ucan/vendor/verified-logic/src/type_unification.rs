//! Verified first-order type unification primitives.
//!
//! This module now includes a fuel-bounded Robinson/Martelli-Montanari style
//! solver over equation lists, plus proofs that any returned substitution is a
//! real unifier of the input equations.

use vstd::prelude::*;

verus! {

/// First-order type syntax.
#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    /// Type variable.
    Var(u32),
    /// Concrete type constructor.
    Con(u32),
    /// Type application.
    App(Box<Type>, Box<Type>),
}

/// Spec-level substitution map.
pub type Substitution = Map<u32, Type>;

/// Empty substitution.
pub open spec fn empty_subst() -> Substitution {
    map![]
}

/// Singleton substitution.
pub open spec fn singleton_subst(var: u32, ty: Type) -> Substitution {
    map![var => ty]
}

/// Structural size of a type expression.
pub open spec fn type_size(ty: Type) -> nat
    decreases ty
{
    match ty {
        Type::Var(_) | Type::Con(_) => 1,
        Type::App(fun, arg) => 1 + type_size(*fun) + type_size(*arg),
    }
}

// r[depends unify.occurs-check]
// r[impl unify.occurs-check]
/// Spec: does `var` occur anywhere in `ty`?
pub open spec fn occurs_in(var: u32, ty: Type) -> bool
    decreases ty
{
    match ty {
        Type::Var(v) => v == var,
        Type::Con(_) => false,
        Type::App(fun, arg) => occurs_in(var, *fun) || occurs_in(var, *arg),
    }
}

/// Apply a substitution to a type.
///
/// Variable lookup is simultaneous, not recursive-through-the-range:
/// if `var` is bound, the mapped type is returned directly.
// r[depends unify.unifier-correct]
// r[impl unify.unifier-correct]
pub open spec fn apply_subst_spec(subst: Substitution, ty: Type) -> Type
    decreases ty
{
    match ty {
        Type::Var(var) => {
            if subst.contains_key(var) {
                subst[var]
            } else {
                Type::Var(var)
            }
        }
        Type::Con(con) => Type::Con(con),
        Type::App(fun, arg) => Type::App(
            Box::new(apply_subst_spec(subst, *fun)),
            Box::new(apply_subst_spec(subst, *arg)),
        ),
    }
}

/// Compose substitutions so application matches sequential application.
///
/// `compose_subst_spec(s2, s1)` means: first apply `s1`, then apply `s2`.
// r[depends unify.subst-compose]
// r[impl unify.subst-compose]
pub open spec fn compose_subst_spec(s2: Substitution, s1: Substitution) -> Substitution {
    Map::new(
        |var: u32| s1.contains_key(var) || s2.contains_key(var),
        |var: u32| {
            if s1.contains_key(var) {
                apply_subst_spec(s2, s1[var])
            } else {
                s2[var]
            }
        },
    )
}

/// Apply a substitution to every pending equation.
pub open spec fn apply_subst_to_eqs(
    subst: Substitution,
    eqs: Seq<(Type, Type)>,
) -> Seq<(Type, Type)>
    decreases eqs.len()
{
    if eqs.len() == 0 {
        seq![]
    } else {
        let rest = eqs.subrange(1, eqs.len() as int);
        seq![
            (
                apply_subst_spec(subst, eqs[0].0),
                apply_subst_spec(subst, eqs[0].1),
            ),
        ] + apply_subst_to_eqs(subst, rest)
    }
}

/// Sum of equation sizes.
pub open spec fn equations_size(eqs: Seq<(Type, Type)>) -> nat
    decreases eqs.len()
{
    if eqs.len() == 0 {
        0
    } else {
        type_size(eqs[0].0) + type_size(eqs[0].1)
            + equations_size(eqs.subrange(1, eqs.len() as int))
    }
}

/// One equation holds under a substitution.
pub open spec fn equation_holds(subst: Substitution, eq: (Type, Type)) -> bool {
    apply_subst_spec(subst, eq.0) == apply_subst_spec(subst, eq.1)
}

/// All equations hold under a substitution.
pub open spec fn equations_hold(subst: Substitution, eqs: Seq<(Type, Type)>) -> bool {
    forall|i: int| 0 <= i < eqs.len() ==> #[trigger] equation_holds(subst, eqs[i])
}

/// A substitution is a unifier for two types.
pub open spec fn is_unifier(subst: Substitution, left: Type, right: Type) -> bool {
    apply_subst_spec(subst, left) == apply_subst_spec(subst, right)
}

/// Fuel-bounded equation-list solver.
///
/// Returns `Some(subst)` when the current fuel budget is enough to derive a
/// unifier, and `None` on clash, occurs-check failure, or exhausted fuel.
// r[depends unify.unifier-exists]
// r[depends unify.failure]
// r[impl unify.unifier-exists]
// r[impl unify.failure]
pub open spec fn solve_fuel(
    eqs: Seq<(Type, Type)>,
    fuel: nat,
) -> Option<Substitution>
    decreases fuel
{
    if eqs.len() == 0 {
        Some(empty_subst())
    } else if fuel == 0 {
        None
    } else {
        let left = eqs[0].0;
        let right = eqs[0].1;
        let rest = eqs.subrange(1, eqs.len() as int);

        if left == right {
            solve_fuel(rest, (fuel - 1) as nat)
        } else {
            match (left, right) {
                (Type::Var(var), ty) => {
                    if occurs_in(var, ty) {
                        None
                    } else {
                        let bind = singleton_subst(var, ty);
                        match solve_fuel(apply_subst_to_eqs(bind, rest), (fuel - 1) as nat) {
                            Some(tail_subst) => Some(compose_subst_spec(tail_subst, bind)),
                            None => None,
                        }
                    }
                }
                (ty, Type::Var(var)) => {
                    if occurs_in(var, ty) {
                        None
                    } else {
                        let bind = singleton_subst(var, ty);
                        match solve_fuel(apply_subst_to_eqs(bind, rest), (fuel - 1) as nat) {
                            Some(tail_subst) => Some(compose_subst_spec(tail_subst, bind)),
                            None => None,
                        }
                    }
                }
                (Type::App(fun1, arg1), Type::App(fun2, arg2)) => {
                    solve_fuel(
                        seq![(*fun1, *fun2), (*arg1, *arg2)] + rest,
                        (fuel - 1) as nat,
                    )
                }
                _ => None,
            }
        }
    }
}

/// Public unifier: choose any successful fuel witness if one exists.
// r[depends unify.unifier-exists]
// r[depends unify.unifier-correct]
// r[impl unify.unifier-exists]
// r[impl unify.unifier-correct]
pub open spec fn unify_spec(left: Type, right: Type) -> Option<Substitution> {
    if exists|fuel: nat| solve_fuel(seq![(left, right)], fuel).is_some() {
        let fuel = choose|fuel: nat| solve_fuel(seq![(left, right)], fuel).is_some();
        solve_fuel(seq![(left, right)], fuel)
    } else {
        None
    }
}

// r[verify unify.subst-compose]
/// Empty substitution is identity.
pub proof fn empty_subst_identity(ty: Type)
    ensures apply_subst_spec(empty_subst(), ty) == ty
    decreases ty
{
    match ty {
        Type::Var(_) | Type::Con(_) => {}
        Type::App(fun, arg) => {
            empty_subst_identity(*fun);
            empty_subst_identity(*arg);
        }
    }
}

// r[verify unify.subst-compose]
/// Singleton substitution rewrites its own variable.
pub proof fn singleton_subst_correct(var: u32, ty: Type)
    ensures apply_subst_spec(singleton_subst(var, ty), Type::Var(var)) == ty
{
}

// r[verify unify.subst-compose]
/// Singleton substitution leaves other variables unchanged.
pub proof fn singleton_subst_other_var(var: u32, other: u32, ty: Type)
    requires var != other
    ensures apply_subst_spec(singleton_subst(var, ty), Type::Var(other)) == Type::Var(other)
{
}

// r[verify unify.occurs-check]
/// Every variable occurs in itself.
pub proof fn occurs_in_self(var: u32)
    ensures occurs_in(var, Type::Var(var))
{
}

// r[verify unify.occurs-check]
/// Constructors contain no variables by themselves.
pub proof fn occurs_not_in_constructor(var: u32, con: u32)
    ensures !occurs_in(var, Type::Con(con))
{
}

// r[verify unify.subst-compose]
/// Composition matches sequential application.
pub proof fn compose_subst_equivalent(s2: Substitution, s1: Substitution, ty: Type)
    ensures
        apply_subst_spec(compose_subst_spec(s2, s1), ty)
            == apply_subst_spec(s2, apply_subst_spec(s1, ty))
    decreases ty
{
    match ty {
        Type::Var(var) => {
            if s1.contains_key(var) {
                assert(apply_subst_spec(s1, Type::Var(var)) == s1[var]);
                assert(compose_subst_spec(s2, s1).contains_key(var));
                assert(compose_subst_spec(s2, s1)[var] == apply_subst_spec(s2, s1[var]));
            } else if s2.contains_key(var) {
                assert(apply_subst_spec(s1, Type::Var(var)) == Type::Var(var));
                assert(compose_subst_spec(s2, s1).contains_key(var));
                assert(compose_subst_spec(s2, s1)[var] == s2[var]);
            } else {
                assert(apply_subst_spec(s1, Type::Var(var)) == Type::Var(var));
                assert(!compose_subst_spec(s2, s1).contains_key(var));
            }
        }
        Type::Con(_) => {}
        Type::App(fun, arg) => {
            compose_subst_equivalent(s2, s1, *fun);
            compose_subst_equivalent(s2, s1, *arg);
        }
    }
}

// r[verify unify.subst-compose]
/// Composition is associative as an action on types.
pub proof fn compose_subst_associative_apply(
    s3: Substitution,
    s2: Substitution,
    s1: Substitution,
    ty: Type,
)
    ensures
        apply_subst_spec(compose_subst_spec(s3, compose_subst_spec(s2, s1)), ty)
            == apply_subst_spec(compose_subst_spec(compose_subst_spec(s3, s2), s1), ty)
{
    compose_subst_equivalent(s3, compose_subst_spec(s2, s1), ty);
    compose_subst_equivalent(s2, s1, ty);
    compose_subst_equivalent(compose_subst_spec(s3, s2), s1, ty);
    compose_subst_equivalent(s3, s2, apply_subst_spec(s1, ty));
}

proof fn subrange_drop_first_index<A>(s: Seq<A>, i: int)
    requires s.len() > 0, 0 <= i < s.subrange(1, s.len() as int).len()
    ensures s.subrange(1, s.len() as int)[i] == s[i + 1]
{}

proof fn seq_cons_index<A>(head: A, tail: Seq<A>, i: int)
    requires 0 <= i < tail.len()
    ensures (seq![head] + tail)[i + 1] == tail[i]
{}

proof fn apply_subst_to_eqs_len(subst: Substitution, eqs: Seq<(Type, Type)>)
    ensures apply_subst_to_eqs(subst, eqs).len() == eqs.len()
    decreases eqs.len()
{
    if eqs.len() > 0 {
        apply_subst_to_eqs_len(subst, eqs.subrange(1, eqs.len() as int));
    }
}

proof fn singleton_subst_irrelevant_if_absent(var: u32, replacement: Type, ty: Type)
    requires !occurs_in(var, ty)
    ensures apply_subst_spec(singleton_subst(var, replacement), ty) == ty
    decreases ty
{
    match ty {
        Type::Var(other) => {
            assert(other != var);
            singleton_subst_other_var(var, other, replacement);
        }
        Type::Con(_) => {}
        Type::App(fun, arg) => {
            singleton_subst_irrelevant_if_absent(var, replacement, *fun);
            singleton_subst_irrelevant_if_absent(var, replacement, *arg);
        }
    }
}

proof fn apply_subst_to_eqs_index(subst: Substitution, eqs: Seq<(Type, Type)>, i: int)
    requires 0 <= i < eqs.len()
    ensures apply_subst_to_eqs(subst, eqs)[i] == (
        apply_subst_spec(subst, eqs[i].0),
        apply_subst_spec(subst, eqs[i].1),
    )
    decreases eqs.len()
{
    if i == 0 {
    } else {
        let rest = eqs.subrange(1, eqs.len() as int);
        subrange_drop_first_index(eqs, i - 1);
        apply_subst_to_eqs_index(subst, rest, i - 1);
        assert(eqs[i] == rest[i - 1]);
        assert(apply_subst_to_eqs(subst, eqs) == seq![
            (
                apply_subst_spec(subst, eqs[0].0),
                apply_subst_spec(subst, eqs[0].1),
            ),
        ] + apply_subst_to_eqs(subst, rest));
        apply_subst_to_eqs_len(subst, rest);
        assert(0 <= i - 1 < apply_subst_to_eqs(subst, rest).len());
        seq_cons_index(
            (
                apply_subst_spec(subst, eqs[0].0),
                apply_subst_spec(subst, eqs[0].1),
            ),
            apply_subst_to_eqs(subst, rest),
            i - 1,
        );
        assert(apply_subst_to_eqs(subst, eqs)[i] == apply_subst_to_eqs(subst, rest)[i - 1]);
    }
}

proof fn equations_hold_after_compose(s2: Substitution, s1: Substitution, eqs: Seq<(Type, Type)>)
    ensures
        equations_hold(s2, apply_subst_to_eqs(s1, eqs))
            ==> equations_hold(compose_subst_spec(s2, s1), eqs)
{
    if equations_hold(s2, apply_subst_to_eqs(s1, eqs)) {
        assert forall|i: int| 0 <= i < eqs.len()
            implies equation_holds(compose_subst_spec(s2, s1), eqs[i]) by {
            apply_subst_to_eqs_index(s1, eqs, i);
            apply_subst_to_eqs_len(s1, eqs);
            compose_subst_equivalent(s2, s1, eqs[i].0);
            compose_subst_equivalent(s2, s1, eqs[i].1);
            assert(0 <= i < apply_subst_to_eqs(s1, eqs).len());
            assert(equation_holds(s2, apply_subst_to_eqs(s1, eqs)[i]));
        };
    }
}

proof fn prefixed_two_tail_holds(
    subst: Substitution,
    first: (Type, Type),
    second: (Type, Type),
    rest: Seq<(Type, Type)>,
)
    ensures
        equations_hold(subst, seq![first, second] + rest)
            ==> equations_hold(subst, rest)
{
    if equations_hold(subst, seq![first, second] + rest) {
        assert forall|i: int| 0 <= i < rest.len()
            implies equation_holds(subst, rest[i]) by {
            assert((seq![first, second] + rest)[i + 2] == rest[i]);
            assert(equation_holds(subst, (seq![first, second] + rest)[i + 2]));
        };
    }
}

// r[verify unify.unifier-correct]
/// Any substitution returned by `solve_fuel` satisfies every input equation.
pub proof fn solve_fuel_correct(eqs: Seq<(Type, Type)>, fuel: nat)
    ensures
        match solve_fuel(eqs, fuel) {
            Some(subst) => equations_hold(subst, eqs),
            None => true,
        }
    decreases fuel
{
    if eqs.len() == 0 {
    } else if fuel == 0 {
    } else {
        let left = eqs[0].0;
        let right = eqs[0].1;
        let rest = eqs.subrange(1, eqs.len() as int);

        if left == right {
            solve_fuel_correct(rest, (fuel - 1) as nat);
            if let Some(subst) = solve_fuel(rest, (fuel - 1) as nat) {
                assert(equations_hold(subst, rest));
                assert forall|i: int| 0 <= i < eqs.len()
                    implies equation_holds(subst, eqs[i]) by {
                    if i == 0 {
                        assert(eqs[i] == (left, right));
                    } else {
                        assert(eqs[i] == rest[i - 1]);
                    }
                };
            }
        } else {
            match (left, right) {
                (Type::Var(var), ty) => {
                    if occurs_in(var, ty) {
                    } else {
                        let bind = singleton_subst(var, ty);
                        solve_fuel_correct(apply_subst_to_eqs(bind, rest), (fuel - 1) as nat);
                        if let Some(tail_subst) = solve_fuel(
                            apply_subst_to_eqs(bind, rest),
                            (fuel - 1) as nat,
                        ) {
                            let subst = compose_subst_spec(tail_subst, bind);
                            equations_hold_after_compose(tail_subst, bind, rest);
                            compose_subst_equivalent(tail_subst, bind, Type::Var(var));
                            compose_subst_equivalent(tail_subst, bind, ty);
                            singleton_subst_irrelevant_if_absent(var, ty, ty);
                            assert(apply_subst_spec(bind, Type::Var(var)) == ty);
                            assert(apply_subst_spec(bind, ty) == ty);
                            assert(equation_holds(subst, (Type::Var(var), ty)));
                            assert(equations_hold(subst, rest));
                            assert forall|i: int| 0 <= i < eqs.len()
                                implies equation_holds(subst, eqs[i]) by {
                                if i == 0 {
                                    assert(eqs[i] == (Type::Var(var), ty));
                                } else {
                                    assert(eqs[i] == rest[i - 1]);
                                }
                            };
                        }
                    }
                }
                (ty, Type::Var(var)) => {
                    if occurs_in(var, ty) {
                    } else {
                        let bind = singleton_subst(var, ty);
                        solve_fuel_correct(apply_subst_to_eqs(bind, rest), (fuel - 1) as nat);
                        if let Some(tail_subst) = solve_fuel(
                            apply_subst_to_eqs(bind, rest),
                            (fuel - 1) as nat,
                        ) {
                            let subst = compose_subst_spec(tail_subst, bind);
                            equations_hold_after_compose(tail_subst, bind, rest);
                            compose_subst_equivalent(tail_subst, bind, ty);
                            compose_subst_equivalent(tail_subst, bind, Type::Var(var));
                            singleton_subst_irrelevant_if_absent(var, ty, ty);
                            assert(apply_subst_spec(bind, Type::Var(var)) == ty);
                            assert(apply_subst_spec(bind, ty) == ty);
                            assert(equation_holds(subst, (ty, Type::Var(var))));
                            assert(equations_hold(subst, rest));
                            assert forall|i: int| 0 <= i < eqs.len()
                                implies equation_holds(subst, eqs[i]) by {
                                if i == 0 {
                                    assert(eqs[i] == (ty, Type::Var(var)));
                                } else {
                                    assert(eqs[i] == rest[i - 1]);
                                }
                            };
                        }
                    }
                }
                (Type::App(fun1, arg1), Type::App(fun2, arg2)) => {
                    let expanded = seq![(*fun1, *fun2), (*arg1, *arg2)] + rest;
                    solve_fuel_correct(expanded, (fuel - 1) as nat);
                    if let Some(subst) = solve_fuel(expanded, (fuel - 1) as nat) {
                        prefixed_two_tail_holds(subst, (*fun1, *fun2), (*arg1, *arg2), rest);
                        assert(equation_holds(subst, (*fun1, *fun2)));
                        assert(equation_holds(subst, (*arg1, *arg2)));
                        assert(equations_hold(subst, rest));
                        assert(equation_holds(
                            subst,
                            (Type::App(fun1, arg1), Type::App(fun2, arg2)),
                        ));
                        assert forall|i: int| 0 <= i < eqs.len()
                            implies equation_holds(subst, eqs[i]) by {
                            if i == 0 {
                                assert(eqs[i] == (Type::App(fun1, arg1), Type::App(fun2, arg2)));
                            } else {
                                assert(eqs[i] == rest[i - 1]);
                            }
                        };
                    }
                }
                _ => {
                }
            }
        }
    }
}

// r[verify unify.unifier-exists]
/// Any successful fuel witness makes `unify_spec` return a substitution.
pub proof fn unify_spec_exists_from_fuel(left: Type, right: Type, fuel: nat)
    requires solve_fuel(seq![(left, right)], fuel).is_some()
    ensures unify_spec(left, right).is_some()
{
}

// r[verify unify.unifier-correct]
/// Any substitution returned by `unify_spec` really unifies the inputs.
pub proof fn unify_spec_correct(left: Type, right: Type)
    ensures
        match unify_spec(left, right) {
            Some(subst) => is_unifier(subst, left, right),
            None => true,
        }
{
    if exists|fuel: nat| solve_fuel(seq![(left, right)], fuel).is_some() {
        let fuel = choose|fuel: nat| solve_fuel(seq![(left, right)], fuel).is_some();
        solve_fuel_correct(seq![(left, right)], fuel);
        if let Some(subst) = solve_fuel(seq![(left, right)], fuel) {
            assert(equations_hold(subst, seq![(left, right)]));
            assert(equation_holds(subst, (left, right)));
        }
    }
}

// r[verify unify.occurs-check]
/// Direct self-reference fails immediately.
pub proof fn occurs_check_rejects_direct(var: u32, ty: Type, fuel: nat)
    requires occurs_in(var, ty), ty != Type::Var(var)
    ensures solve_fuel(seq![(Type::Var(var), ty)], fuel + 1) == None::<Substitution>
{
}

// r[verify unify.failure]
/// Distinct concrete constructors clash.
pub proof fn distinct_constructors_fail(left: u32, right: u32, fuel: nat)
    requires left != right
    ensures solve_fuel(seq![(Type::Con(left), Type::Con(right))], fuel + 1) == None::<Substitution>
{
}

// r[verify unify.failure]
/// Application-vs-constructor mismatch clashes immediately.
pub proof fn app_con_mismatch_fails(fun: Type, arg: Type, con: u32, fuel: nat)
    ensures solve_fuel(seq![(Type::App(Box::new(fun), Box::new(arg)), Type::Con(con))], fuel + 1)
        == None::<Substitution>
{
}

} // verus!
