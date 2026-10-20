//! Verified Option combinators.
//!
//! Spec fns use concrete `Option<u64>` so Verus can verify properties.
//! Generic exec fns (outside `verus!`) handle arbitrary types at runtime.

use vstd::prelude::*;

verus! {

// --- unwrap_or_default ---

// r[depends option.unwrap-or-default]
/// Spec: returns the contained value or the default.
pub open spec fn unwrap_or_default_spec(opt: Option<u64>, default: u64) -> u64 {
    match opt {
        Some(v) => v,
        None => default,
    }
}

// r[impl option.unwrap-or-default]
/// Returns the contained value or the provided default.
pub fn unwrap_or_default(opt: Option<u64>, default: u64) -> (result: u64)
    ensures result == unwrap_or_default_spec(opt, default)
{
    match opt {
        Some(v) => v,
        None => default,
    }
}

// r[verify option.unwrap-or-default]
proof fn unwrap_or_default_some(v: u64, default: u64)
    ensures unwrap_or_default_spec(Some(v), default) == v
{}

// r[verify option.unwrap-or-default]
proof fn unwrap_or_default_none(default: u64)
    ensures unwrap_or_default_spec(None, default) == default
{}

// --- map ---

// r[depends option.map-preserves-none]
/// Spec: mapping over an option.
pub open spec fn map_spec(opt: Option<u64>, f: spec_fn(u64) -> u64) -> Option<u64> {
    match opt {
        Some(v) => Some(f(v)),
        None => None,
    }
}

// r[verify option.map-preserves-none]
proof fn map_none_is_none(f: spec_fn(u64) -> u64)
    ensures map_spec(None, f) == None::<u64>
{}

// r[verify option.map-preserves-none]
proof fn map_some_applies(v: u64, f: spec_fn(u64) -> u64)
    ensures map_spec(Some(v), f) == Some(f(v))
{}

// r[verify option.map-preserves-none]
/// Functor law: map(map(opt, f), g) == map(opt, g∘f).
proof fn map_compose(opt: Option<u64>, f: spec_fn(u64) -> u64, g: spec_fn(u64) -> u64)
    ensures map_spec(map_spec(opt, f), g) == map_spec(opt, |x: u64| g(f(x)))
{}

// --- and_then ---

// r[depends option.and-then]
/// Spec: monadic bind — applies `f` to the inner value, or returns None.
pub open spec fn and_then_spec(opt: Option<u64>, f: spec_fn(u64) -> Option<u64>) -> Option<u64> {
    match opt {
        Some(v) => f(v),
        None => None,
    }
}

// r[verify option.and-then]
proof fn and_then_none_is_none(f: spec_fn(u64) -> Option<u64>)
    ensures and_then_spec(None, f) == None::<u64>
{}

// r[verify option.and-then]
proof fn and_then_some_applies(v: u64, f: spec_fn(u64) -> Option<u64>)
    ensures and_then_spec(Some(v), f) == f(v)
{}

// r[verify option.and-then]
/// Monad left identity: and_then(Some(v), f) == f(v).
proof fn and_then_left_identity(v: u64, f: spec_fn(u64) -> Option<u64>)
    ensures and_then_spec(Some(v), f) == f(v)
{}

// r[verify option.and-then]
/// Monad right identity: and_then(opt, Some) == opt.
proof fn and_then_right_identity(opt: Option<u64>)
    ensures and_then_spec(opt, |v: u64| Some(v)) == opt
{}

// --- or_else ---

// r[depends option.or-else]
/// Spec: returns the option if Some, or the fallback otherwise.
pub open spec fn or_else_spec(opt: Option<u64>, fallback: Option<u64>) -> Option<u64> {
    match opt {
        Some(v) => Some(v),
        None => fallback,
    }
}

// r[verify option.or-else]
proof fn or_else_some_unchanged(v: u64, fallback: Option<u64>)
    ensures or_else_spec(Some(v), fallback) == Some(v)
{}

// r[verify option.or-else]
proof fn or_else_none_returns_fallback(fallback: Option<u64>)
    ensures or_else_spec(None, fallback) == fallback
{}

// r[verify option.or-else]
/// or_else with Some fallback always yields Some.
proof fn or_else_some_fallback_is_some(opt: Option<u64>, v: u64)
    ensures or_else_spec(opt, Some(v)).is_some()
{}

// --- flatten ---

// r[depends option.flatten]
/// Spec: collapses nested Option.
pub open spec fn flatten_spec(opt: Option<Option<u64>>) -> Option<u64> {
    match opt {
        Some(inner) => inner,
        None => None,
    }
}

// r[impl option.flatten]
/// Collapses `Option<Option<u64>>` into `Option<u64>`.
pub fn flatten(opt: Option<Option<u64>>) -> (result: Option<u64>)
    ensures result == flatten_spec(opt)
{
    match opt {
        Some(inner) => inner,
        None => None,
    }
}

// r[verify option.flatten]
proof fn flatten_none_is_none()
    ensures flatten_spec(None::<Option<u64>>) == None::<u64>
{}

// r[verify option.flatten]
proof fn flatten_some_none_is_none()
    ensures flatten_spec(Some(None::<u64>)) == None::<u64>
{}

// r[verify option.flatten]
proof fn flatten_some_some(v: u64)
    ensures flatten_spec(Some(Some(v))) == Some(v)
{}

// --- zip ---

// r[depends option.zip]
/// Spec: pairs two options, yielding Some only if both are Some.
pub open spec fn zip_spec(a: Option<u64>, b: Option<u64>) -> Option<(u64, u64)> {
    match (a, b) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    }
}

// r[impl option.zip]
/// Returns `Some((a, b))` if both options are `Some`, else `None`.
pub fn zip(a: Option<u64>, b: Option<u64>) -> (result: Option<(u64, u64)>)
    ensures result == zip_spec(a, b)
{
    match (a, b) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    }
}

// r[verify option.zip]
proof fn zip_both_some(x: u64, y: u64)
    ensures zip_spec(Some(x), Some(y)) == Some((x, y))
{}

// r[verify option.zip]
proof fn zip_none_left(b: Option<u64>)
    ensures zip_spec(None::<u64>, b).is_none()
{}

// r[verify option.zip]
proof fn zip_none_right(a: Option<u64>)
    ensures zip_spec(a, None::<u64>).is_none()
{}

// --- filter ---

// r[depends option.filter]
/// Spec: keeps the value only if the predicate holds.
pub open spec fn filter_spec(opt: Option<u64>, pred: spec_fn(u64) -> bool) -> Option<u64> {
    match opt {
        Some(v) => if pred(v) { Some(v) } else { None },
        None => None,
    }
}

// r[verify option.filter]
proof fn filter_none_is_none(pred: spec_fn(u64) -> bool)
    ensures filter_spec(None, pred) == None::<u64>
{}

// r[verify option.filter]
proof fn filter_passes(v: u64, pred: spec_fn(u64) -> bool)
    requires pred(v)
    ensures filter_spec(Some(v), pred) == Some(v)
{}

// r[verify option.filter]
proof fn filter_rejects(v: u64, pred: spec_fn(u64) -> bool)
    requires !pred(v)
    ensures filter_spec(Some(v), pred) == None::<u64>
{}

// r[verify option.filter]
/// Filtering is idempotent: filter(filter(opt, p), p) == filter(opt, p).
proof fn filter_idempotent(opt: Option<u64>, pred: spec_fn(u64) -> bool)
    ensures filter_spec(filter_spec(opt, pred), pred) == filter_spec(opt, pred)
{}

} // verus!

// --- Generic exec fns (outside verus! for closure/generic support) ---

// r[impl option.map-preserves-none]
/// Maps an `Option<T>` by applying `f` to the contained value.
pub fn map<T, U>(opt: Option<T>, f: impl FnOnce(T) -> U) -> Option<U> {
    match opt {
        Some(v) => Some(f(v)),
        None => None,
    }
}

// r[impl option.and-then]
/// Applies `f` to the contained value (if any), or returns `None`.
pub fn and_then<T, U>(opt: Option<T>, f: impl FnOnce(T) -> Option<U>) -> Option<U> {
    match opt {
        Some(v) => f(v),
        None => None,
    }
}

// r[impl option.or-else]
/// Returns the option if it contains a value, otherwise calls `f`.
pub fn or_else<T>(opt: Option<T>, f: impl FnOnce() -> Option<T>) -> Option<T> {
    match opt {
        Some(v) => Some(v),
        None => f(),
    }
}

// r[impl option.filter]
/// Returns `Some(v)` if the predicate returns true, otherwise `None`.
pub fn filter<T>(opt: Option<T>, pred: impl FnOnce(&T) -> bool) -> Option<T> {
    match opt {
        Some(v) => {
            if pred(&v) {
                Some(v)
            } else {
                None
            }
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify option.unwrap-or-default]
    #[test]
    fn test_unwrap_or_default() {
        assert_eq!(unwrap_or_default(Some(42), 0), 42);
        assert_eq!(unwrap_or_default(None, 99), 99);
    }

    // r[verify option.map-preserves-none]
    #[test]
    fn test_map() {
        let none: Option<u64> = None;
        assert_eq!(map(none, |x| x + 1), None);
        assert_eq!(map(Some(5u64), |x| x * 2), Some(10));
    }

    // r[verify option.and-then]
    #[test]
    fn test_and_then() {
        let checked = |x: u64| x.checked_add(1);
        assert_eq!(and_then(Some(5u64), checked), Some(6));
        assert_eq!(and_then(Some(u64::MAX), checked), None);
        assert_eq!(and_then(None, checked), None);
    }

    // r[verify option.or-else]
    #[test]
    fn test_or_else() {
        assert_eq!(or_else(Some(5u64), || Some(99)), Some(5));
        assert_eq!(or_else(None, || Some(99u64)), Some(99));
        assert_eq!(or_else(None::<u64>, || None), None);
    }

    // r[verify option.flatten]
    #[test]
    fn test_flatten() {
        assert_eq!(flatten(Some(Some(42))), Some(42));
        assert_eq!(flatten(Some(None::<u64>)), None);
        assert_eq!(flatten(None::<Option<u64>>), None);
    }

    // r[verify option.zip]
    #[test]
    fn test_zip() {
        assert_eq!(zip(Some(1), Some(2)), Some((1, 2)));
        assert_eq!(zip(Some(1), None), None);
        assert_eq!(zip(None, Some(2)), None);
        assert_eq!(zip(None::<u64>, None), None);
    }

    // r[verify option.filter]
    #[test]
    fn test_filter() {
        assert_eq!(filter(Some(4u64), |x| x % 2 == 0), Some(4));
        assert_eq!(filter(Some(3u64), |x| x % 2 == 0), None);
        assert_eq!(filter(None::<u64>, |_| true), None);
    }

    proptest! {
        // r[verify option.unwrap-or-default]
        #[test]
        fn prop_unwrap_or_default_some(v: u64, default: u64) {
            prop_assert_eq!(unwrap_or_default(Some(v), default), v);
        }

        // r[verify option.unwrap-or-default]
        #[test]
        fn prop_unwrap_or_default_none(default: u64) {
            prop_assert_eq!(unwrap_or_default(None, default), default);
        }

        // r[verify option.map-preserves-none]
        #[test]
        fn prop_map_none_is_none(seed: u64) {
            let none: Option<u64> = None;
            prop_assert_eq!(map(none, |x| x.wrapping_add(seed)), None);
        }

        // r[verify option.and-then]
        #[test]
        fn prop_and_then_none_is_none(seed: u64) {
            let none: Option<u64> = None;
            prop_assert_eq!(and_then(none, |x| Some(x.wrapping_add(seed))), None);
        }

        // r[verify option.and-then]
        #[test]
        fn prop_and_then_right_identity(v: u64) {
            // and_then(Some(v), Some) == Some(v)
            prop_assert_eq!(and_then(Some(v), Some), Some(v));
        }

        // r[verify option.or-else]
        #[test]
        fn prop_or_else_some_unchanged(v: u64, fallback: u64) {
            prop_assert_eq!(or_else(Some(v), || Some(fallback)), Some(v));
        }

        // r[verify option.or-else]
        #[test]
        fn prop_or_else_none_uses_fallback(fallback: u64) {
            prop_assert_eq!(or_else(None, || Some(fallback)), Some(fallback));
        }

        // r[verify option.flatten]
        #[test]
        fn prop_flatten_some_some(v: u64) {
            prop_assert_eq!(flatten(Some(Some(v))), Some(v));
        }

        // r[verify option.zip]
        #[test]
        fn prop_zip_both_some(a: u64, b: u64) {
            prop_assert_eq!(zip(Some(a), Some(b)), Some((a, b)));
        }

        // r[verify option.zip]
        #[test]
        fn prop_zip_none_left(b: u64) {
            prop_assert_eq!(zip(None, Some(b)), None);
        }

        // r[verify option.filter]
        #[test]
        fn prop_filter_idempotent(v: u64, threshold: u64) {
            let f = |x: &u64| *x > threshold;
            prop_assert_eq!(
                filter(filter(Some(v), f), f),
                filter(Some(v), f)
            );
        }
    }
}
