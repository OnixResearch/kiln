//! Verified Result combinators.
//!
//! Spec fns use concrete `Result<u64, u64>` for Verus verification.
//! Generic exec fns (outside `verus!`) handle arbitrary types at runtime.

use vstd::prelude::*;

verus! {

// --- unwrap_or ---

// r[depends result.unwrap-or]
/// Spec: returns the Ok value or the provided default.
pub open spec fn unwrap_or_spec(res: Result<u64, u64>, default: u64) -> u64 {
    match res {
        Ok(v) => v,
        Err(_) => default,
    }
}

// r[impl result.unwrap-or]
/// Returns the contained Ok value or the provided default.
pub fn unwrap_or(res: Result<u64, u64>, default: u64) -> (result: u64)
    ensures result == unwrap_or_spec(res, default)
{
    match res {
        Ok(v) => v,
        Err(_) => default,
    }
}

// r[verify result.unwrap-or]
proof fn unwrap_or_ok(v: u64, default: u64)
    ensures unwrap_or_spec(Ok(v), default) == v
{}

// r[verify result.unwrap-or]
proof fn unwrap_or_err(e: u64, default: u64)
    ensures unwrap_or_spec(Err(e), default) == default
{}

// --- map_ok ---

// r[depends result.map-ok]
/// Spec: applies `f` to the Ok value, leaves Err untouched.
pub open spec fn map_ok_spec(res: Result<u64, u64>, f: spec_fn(u64) -> u64) -> Result<u64, u64> {
    match res {
        Ok(v) => Ok(f(v)),
        Err(e) => Err(e),
    }
}

// r[verify result.map-ok]
proof fn map_ok_preserves_err(e: u64, f: spec_fn(u64) -> u64)
    ensures map_ok_spec(Err(e), f) == Err::<u64, u64>(e)
{}

// r[verify result.map-ok]
proof fn map_ok_applies(v: u64, f: spec_fn(u64) -> u64)
    ensures map_ok_spec(Ok(v), f) == Ok::<u64, u64>(f(v))
{}

// r[verify result.map-ok]
/// Functor law: map_ok(map_ok(res, f), g) == map_ok(res, g∘f).
proof fn map_ok_compose(
    res: Result<u64, u64>,
    f: spec_fn(u64) -> u64,
    g: spec_fn(u64) -> u64,
)
    ensures map_ok_spec(map_ok_spec(res, f), g) == map_ok_spec(res, |x: u64| g(f(x)))
{}

// --- map_err ---

// r[depends result.map-err]
/// Spec: applies `f` to the Err value, leaves Ok untouched.
pub open spec fn map_err_spec(res: Result<u64, u64>, f: spec_fn(u64) -> u64) -> Result<u64, u64> {
    match res {
        Ok(v) => Ok(v),
        Err(e) => Err(f(e)),
    }
}

// r[verify result.map-err]
proof fn map_err_preserves_ok(v: u64, f: spec_fn(u64) -> u64)
    ensures map_err_spec(Ok(v), f) == Ok::<u64, u64>(v)
{}

// r[verify result.map-err]
proof fn map_err_applies(e: u64, f: spec_fn(u64) -> u64)
    ensures map_err_spec(Err(e), f) == Err::<u64, u64>(f(e))
{}

// r[verify result.map-err]
/// map_ok and map_err commute: order doesn't matter.
proof fn map_ok_map_err_commute(
    res: Result<u64, u64>,
    f: spec_fn(u64) -> u64,
    g: spec_fn(u64) -> u64,
)
    ensures
        map_err_spec(map_ok_spec(res, f), g)
            == map_ok_spec(map_err_spec(res, g), f)
{}

// --- and_then ---

// r[depends result.and-then]
/// Spec: monadic bind on Ok — applies `f` to Ok value, propagates Err.
pub open spec fn and_then_spec(
    res: Result<u64, u64>,
    f: spec_fn(u64) -> Result<u64, u64>,
) -> Result<u64, u64> {
    match res {
        Ok(v) => f(v),
        Err(e) => Err(e),
    }
}

// r[verify result.and-then]
proof fn and_then_ok_applies(v: u64, f: spec_fn(u64) -> Result<u64, u64>)
    ensures and_then_spec(Ok(v), f) == f(v)
{}

// r[verify result.and-then]
proof fn and_then_err_propagates(e: u64, f: spec_fn(u64) -> Result<u64, u64>)
    ensures and_then_spec(Err(e), f) == Err::<u64, u64>(e)
{}

// r[verify result.and-then]
/// Monad right identity: and_then(res, Ok) == res.
proof fn and_then_right_identity(res: Result<u64, u64>)
    ensures and_then_spec(res, |v: u64| Ok::<u64, u64>(v)) == res
{}

// --- or_else ---

// r[depends result.or-else]
/// Spec: returns Ok if present, otherwise uses the fallback.
pub open spec fn or_else_spec(
    res: Result<u64, u64>,
    fallback: Result<u64, u64>,
) -> Result<u64, u64> {
    match res {
        Ok(v) => Ok(v),
        Err(_) => fallback,
    }
}

// r[verify result.or-else]
proof fn or_else_ok_unchanged(v: u64, fallback: Result<u64, u64>)
    ensures or_else_spec(Ok(v), fallback) == Ok::<u64, u64>(v)
{}

// r[verify result.or-else]
proof fn or_else_err_returns_fallback(e: u64, fallback: Result<u64, u64>)
    ensures or_else_spec(Err(e), fallback) == fallback
{}

// r[verify result.or-else]
/// or_else with Ok fallback always yields Ok.
proof fn or_else_ok_fallback_is_ok(res: Result<u64, u64>, v: u64)
    ensures or_else_spec(res, Ok(v)).is_ok()
{}

} // verus!

// --- Generic exec fns ---

// r[impl result.map-ok]
/// Applies `f` to the Ok value, leaving Err untouched.
pub fn map_ok<T, U, E>(res: Result<T, E>, f: impl FnOnce(T) -> U) -> Result<U, E> {
    match res {
        Ok(v) => Ok(f(v)),
        Err(e) => Err(e),
    }
}

// r[impl result.map-err]
/// Applies `f` to the Err value, leaving Ok untouched.
pub fn map_err<T, E, F>(res: Result<T, E>, f: impl FnOnce(E) -> F) -> Result<T, F> {
    match res {
        Ok(v) => Ok(v),
        Err(e) => Err(f(e)),
    }
}

// r[impl result.and-then]
/// Applies `f` to the Ok value, or propagates the Err.
pub fn and_then<T, U, E>(res: Result<T, E>, f: impl FnOnce(T) -> Result<U, E>) -> Result<U, E> {
    match res {
        Ok(v) => f(v),
        Err(e) => Err(e),
    }
}

// r[impl result.or-else]
/// Returns the Ok value, or calls `f` on the Err.
pub fn or_else<T, E, F>(res: Result<T, E>, f: impl FnOnce(E) -> Result<T, F>) -> Result<T, F> {
    match res {
        Ok(v) => Ok(v),
        Err(e) => f(e),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify result.unwrap-or]
    #[test]
    fn test_unwrap_or() {
        assert_eq!(unwrap_or(Ok(42), 0), 42);
        assert_eq!(unwrap_or(Err(99), 0), 0);
    }

    // r[verify result.map-ok]
    #[test]
    fn test_map_ok() {
        assert_eq!(map_ok(Ok::<u64, u64>(5), |x| x * 2), Ok(10));
        assert_eq!(map_ok(Err::<u64, u64>(3), |x| x * 2), Err(3));
    }

    // r[verify result.map-err]
    #[test]
    fn test_map_err() {
        assert_eq!(map_err(Ok::<u64, u64>(5), |e| e + 1), Ok(5));
        assert_eq!(map_err(Err::<u64, u64>(3), |e| e + 1), Err(4));
    }

    // r[verify result.and-then]
    #[test]
    fn test_and_then() {
        let checked = |x: u64| x.checked_add(1).ok_or(0u64);
        assert_eq!(and_then(Ok(5), checked), Ok(6));
        assert_eq!(and_then(Ok(u64::MAX), checked), Err(0));
        assert_eq!(and_then(Err(99u64), checked), Err(99));
    }

    // r[verify result.or-else]
    #[test]
    fn test_or_else() {
        let fallback = |_e: u64| Ok::<u64, u64>(0);
        assert_eq!(or_else(Ok(5), fallback), Ok(5));
        assert_eq!(or_else(Err(99u64), fallback), Ok(0));
    }

    proptest! {
        // r[verify result.unwrap-or]
        #[test]
        fn prop_unwrap_or_ok(v: u64, default: u64) {
            prop_assert_eq!(unwrap_or(Ok(v), default), v);
        }

        // r[verify result.unwrap-or]
        #[test]
        fn prop_unwrap_or_err(e: u64, default: u64) {
            prop_assert_eq!(unwrap_or(Err(e), default), default);
        }

        // r[verify result.map-ok]
        #[test]
        fn prop_map_ok_preserves_err(e: u64, offset: u64) {
            let res: Result<u64, u64> = Err(e);
            prop_assert_eq!(map_ok(res, |x| x.wrapping_add(offset)), Err(e));
        }

        // r[verify result.map-err]
        #[test]
        fn prop_map_err_preserves_ok(v: u64, offset: u64) {
            let res: Result<u64, u64> = Ok(v);
            prop_assert_eq!(map_err(res, |e| e.wrapping_add(offset)), Ok(v));
        }

        // r[verify result.and-then]
        #[test]
        fn prop_and_then_right_identity(v: u64) {
            prop_assert_eq!(and_then(Ok::<u64, u64>(v), Ok), Ok::<u64, u64>(v));
        }

        // r[verify result.and-then]
        #[test]
        fn prop_and_then_err_propagates(e: u64) {
            let res: Result<u64, u64> = Err(e);
            prop_assert_eq!(and_then(res, |x| Ok(x.wrapping_add(1))), Err(e));
        }

        // r[verify result.or-else]
        #[test]
        fn prop_or_else_ok_unchanged(v: u64) {
            let res: Result<u64, u64> = Ok(v);
            prop_assert_eq!(or_else(res, |_| Ok::<u64, u64>(0)), Ok(v));
        }

        // r[verify result.or-else]
        #[test]
        fn prop_or_else_err_uses_fallback(e: u64) {
            let res: Result<u64, u64> = Err(e);
            prop_assert_eq!(or_else(res, |_| Ok::<u64, u64>(42)), Ok(42));
        }

        // r[verify result.map-ok]
        // r[verify result.map-err]
        #[test]
        fn prop_map_ok_map_err_commute(v: u64) {
            // map_ok then map_err == map_err then map_ok
            let res: Result<u64, u64> = Ok(v);
            let f = |x: u64| x.wrapping_add(1);
            let g = |e: u64| e.wrapping_add(2);
            prop_assert_eq!(
                map_err(map_ok(res, f), g),
                map_ok(map_err(res, g), f)
            );
        }
    }
}
