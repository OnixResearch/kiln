//! Verified composable attenuation constraints (caveats) for UCAN capabilities.
//!
//! Models typed caveats (time bound, rate limit) with conjunction-based
//! satisfaction, containment monotonicity, delegation-only-adds semantics,
//! and intersection soundness/minimality.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// A caveat restricting capability usage.
pub enum Caveat {
    /// Time bound: must be used before `not_after`.
    TimeBound { not_after: u64 },
    /// Rate limit: at most `max_count` uses within `window_secs`.
    RateLimit { max_count: u64, window_secs: u64 },
}

/// Runtime context for checking caveat satisfaction.
pub struct CaveatContext {
    /// Current timestamp.
    pub now: u64,
    /// Number of uses in the current rate-limit window.
    pub count_in_window: u64,
}

// --- Single caveat satisfaction ---

/// A caveat is satisfied against a context.
// r[impl ucan-caveat.satisfaction-decidable]
// r[verify ucan-caveat.satisfaction-decidable]
pub open spec fn caveat_satisfied_spec(c: Caveat, ctx: CaveatContext) -> bool {
    match c {
        Caveat::TimeBound { not_after } => ctx.now <= not_after,
        Caveat::RateLimit { max_count, window_secs: _ } => ctx.count_in_window <= max_count,
    }
}

/// Check caveat satisfaction at runtime.
// r[impl ucan-caveat.satisfaction-decidable]
pub fn caveat_satisfied(c: &Caveat, ctx: &CaveatContext) -> (result: bool)
    ensures result == caveat_satisfied_spec(*c, *ctx),
{
    match c {
        Caveat::TimeBound { not_after } => ctx.now <= *not_after,
        Caveat::RateLimit { max_count, window_secs: _ } => ctx.count_in_window <= *max_count,
    }
}

// --- Caveat set satisfaction (conjunction) ---

/// A caveat set is satisfied iff every caveat is satisfied.
// r[impl ucan-caveat.set-conjunction]
// r[verify ucan-caveat.set-conjunction]
pub open spec fn caveat_set_satisfied_spec(set: Seq<Caveat>, ctx: CaveatContext) -> bool {
    forall|i: int| #![auto] 0 <= i < set.len() ==> caveat_satisfied_spec(set[i], ctx)
}

/// Check caveat set satisfaction at runtime.
// r[impl ucan-caveat.set-conjunction]
pub fn caveat_set_satisfied(set: &Vec<Caveat>, ctx: &CaveatContext) -> (result: bool)
    ensures result ==> caveat_set_satisfied_spec(set@, *ctx),
{
    let mut i: usize = 0;
    while i < set.len()
        invariant
            0 <= i <= set.len(),
            forall|k: int| #![auto] 0 <= k < i ==> caveat_satisfied_spec(set@[k], *ctx),
        decreases set.len() - i,
    {
        if !caveat_satisfied(&set[i], ctx) {
            return false;
        }
        i = i + 1;
    }
    true
}

// --- Typed caveat payload-shape admission ---

/// ASCII decimal digit byte used by typed caveat payloads.
// r[impl ucan-caveat.satisfaction-decidable]
pub open spec fn ascii_decimal_digit_spec(byte: u8) -> bool {
    48u8 <= byte && byte <= 57u8
}

/// Non-empty ASCII decimal payload shape used before runtime numeric parsing.
// r[impl ucan-caveat.satisfaction-decidable]
pub open spec fn ascii_decimal_payload_spec(payload: Seq<u8>) -> bool {
    payload.len() > 0 && forall|i: int| #![auto] 0 <= i < payload.len() ==> ascii_decimal_digit_spec(payload[i])
}

/// `max_count:window_seconds` byte-shape admission for the supported rate-limit caveat.
// r[impl ucan-caveat.satisfaction-decidable]
pub open spec fn rate_limit_payload_shape_spec(payload: Seq<u8>) -> bool {
    payload.len() >= 3 && exists|colon: int|
        0 < colon < payload.len() - 1
            && payload[colon] == 58u8
            && (forall|i: int| #![auto] 0 <= i < payload.len() ==> if i == colon {
                payload[i] == 58u8
            } else {
                ascii_decimal_digit_spec(payload[i])
            })
}

/// Check whether one byte is an ASCII decimal digit.
// r[impl ucan-caveat.satisfaction-decidable]
pub fn is_ascii_decimal_digit(byte: u8) -> (result: bool)
    ensures result == ascii_decimal_digit_spec(byte),
{
    48u8 <= byte && byte <= 57u8
}

/// Check non-empty ASCII decimal payload shape before runtime numeric parsing.
// r[impl ucan-caveat.satisfaction-decidable]
pub fn ascii_decimal_payload_is_valid(payload: &Vec<u8>) -> (result: bool)
    ensures result == ascii_decimal_payload_spec(payload@),
{
    if payload.len() == 0 {
        return false;
    }
    let mut i: usize = 0;
    while i < payload.len()
        invariant
            0 <= i <= payload.len(),
            forall|k: int| #![auto] 0 <= k < i ==> ascii_decimal_digit_spec(payload@[k]),
        decreases payload.len() - i,
    {
        if !is_ascii_decimal_digit(payload[i]) {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Check supported rate-limit `max_count:window_seconds` payload shape before runtime parsing.
// r[impl ucan-caveat.satisfaction-decidable]
pub fn rate_limit_payload_shape_is_valid(payload: &Vec<u8>) -> (result: bool)
    ensures result ==> rate_limit_payload_shape_spec(payload@),
{
    if payload.len() < 3 {
        return false;
    }
    let mut i: usize = 0;
    let mut colon_seen = false;
    let mut colon_index: usize = 0;
    while i < payload.len()
        invariant
            0 <= i <= payload.len(),
            colon_seen ==> colon_index < i,
            colon_seen ==> payload@[colon_index as int] == 58u8,
            forall|k: int| #![auto] 0 <= k < i ==> payload@[k] == 58u8 || ascii_decimal_digit_spec(payload@[k]),
            forall|k: int| #![auto] 0 <= k < i && payload@[k] == 58u8 ==> colon_seen && k == colon_index as int,
        decreases payload.len() - i,
    {
        let byte = payload[i];
        if byte == 58u8 {
            if colon_seen {
                return false;
            }
            colon_seen = true;
            colon_index = i;
        } else if !is_ascii_decimal_digit(byte) {
            return false;
        }
        i = i + 1;
    }
    if !colon_seen {
        return false;
    }
    if colon_index == 0 {
        return false;
    }
    if colon_index + 1 >= payload.len() {
        return false;
    }
    assert(payload@.len() >= 3);
    assert(0 < colon_index as int);
    assert((colon_index as int) < payload@.len() - 1);
    assert(payload@[colon_index as int] == 58u8);
    assert forall|k: int| #![auto] 0 <= k < payload@.len() implies if k == colon_index as int {
        payload@[k] == 58u8
    } else {
        ascii_decimal_digit_spec(payload@[k])
    } by {
        if k == colon_index as int {
        } else {
            assert(payload@[k] != 58u8);
        }
    }
    true
}

// --- Single caveat containment ---

/// Parent caveat contains child caveat: satisfying child implies satisfying parent.
// r[impl ucan-caveat.containment-monotone]
pub open spec fn caveat_contains_spec(parent: Caveat, child: Caveat) -> bool {
    match (parent, child) {
        (Caveat::TimeBound { not_after: p_na }, Caveat::TimeBound { not_after: c_na }) =>
            c_na <= p_na,
        (Caveat::RateLimit { max_count: p_mc, window_secs: p_ws },
         Caveat::RateLimit { max_count: c_mc, window_secs: c_ws }) =>
            c_mc <= p_mc && c_ws == p_ws,
        _ => false,
    }
}

// --- Caveat set containment ---

/// Parent set contains child set: for each parent caveat, some child caveat contains it.
// r[impl ucan-caveat.set-containment-monotone]
pub open spec fn caveat_set_contains_spec(parent_set: Seq<Caveat>, child_set: Seq<Caveat>) -> bool {
    forall|i: int| 0 <= i < parent_set.len() ==> (#[trigger] caveat_set_contains_at(parent_set, child_set, i))
}

/// Helper: parent_set[i] is contained by some child_set[j].
pub open spec fn caveat_set_contains_at(parent_set: Seq<Caveat>, child_set: Seq<Caveat>, i: int) -> bool {
    exists|j: int| 0 <= j < child_set.len() && caveat_contains_spec(parent_set[i], #[trigger] child_set[j])
}

// --- Intersection ---

/// Intersection of two caveat sets: concatenation.
// r[impl ucan-caveat.intersection-sound]
// r[impl ucan-caveat.intersection-minimal]
pub open spec fn caveat_intersect_spec(a: Seq<Caveat>, b: Seq<Caveat>) -> Seq<Caveat> {
    a + b
}

/// Compute intersection at runtime.
// r[impl ucan-caveat.intersection-sound]
pub fn caveat_intersect(a: &Vec<Caveat>, b: &Vec<Caveat>) -> (result: Vec<Caveat>)
    ensures result@.len() == a@.len() + b@.len(),
{
    let mut result = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant 0 <= i <= a.len(), result.len() == i,
        decreases a.len() - i,
    {
        result.push(clone_caveat(&a[i]));
        i = i + 1;
    }
    let mut j: usize = 0;
    while j < b.len()
        invariant 0 <= j <= b.len(), result.len() == a.len() + j,
        decreases b.len() - j,
    {
        result.push(clone_caveat(&b[j]));
        j = j + 1;
    }
    result
}

fn clone_caveat(c: &Caveat) -> (result: Caveat)
    ensures result == *c,
{
    match c {
        Caveat::TimeBound { not_after } => Caveat::TimeBound { not_after: *not_after },
        Caveat::RateLimit { max_count, window_secs } =>
            Caveat::RateLimit { max_count: *max_count, window_secs: *window_secs },
    }
}

// --- Proofs ---

/// Containment monotonicity: parent contains child implies satisfaction flows.
// r[verify ucan-caveat.containment-monotone]
pub proof fn containment_implies_satisfaction(parent: Caveat, child: Caveat, ctx: CaveatContext)
    requires
        caveat_contains_spec(parent, child),
        caveat_satisfied_spec(child, ctx),
    ensures caveat_satisfied_spec(parent, ctx),
{
    match (parent, child) {
        (Caveat::TimeBound { not_after: p_na }, Caveat::TimeBound { not_after: c_na }) => {
            // c_na <= p_na, ctx.now <= c_na => ctx.now <= p_na
        },
        (Caveat::RateLimit { max_count: p_mc, .. }, Caveat::RateLimit { max_count: c_mc, .. }) => {
            // c_mc <= p_mc, ctx.count <= c_mc => ctx.count <= p_mc
        },
        _ => {},
    }
}

/// Set containment monotonicity: parent set satisfied when child set satisfied.
// r[verify ucan-caveat.set-containment-monotone]
pub proof fn set_containment_monotone(
    parent_set: Seq<Caveat>,
    child_set: Seq<Caveat>,
    ctx: CaveatContext,
)
    requires
        caveat_set_contains_spec(parent_set, child_set),
        caveat_set_satisfied_spec(child_set, ctx),
    ensures caveat_set_satisfied_spec(parent_set, ctx),
{
    assert forall|i: int| #![auto] 0 <= i < parent_set.len()
        implies caveat_satisfied_spec(parent_set[i], ctx)
    by {
        assert(caveat_set_contains_at(parent_set, child_set, i));
        let j = choose|j: int| 0 <= j < child_set.len()
            && caveat_contains_spec(parent_set[i], #[trigger] child_set[j]);
        containment_implies_satisfaction(parent_set[i], child_set[j], ctx);
    }
}

/// Intersection soundness: satisfying the intersection implies both original sets.
// r[verify ucan-caveat.intersection-sound]
pub proof fn intersection_sound(a: Seq<Caveat>, b: Seq<Caveat>, ctx: CaveatContext)
    requires caveat_set_satisfied_spec(caveat_intersect_spec(a, b), ctx),
    ensures
        caveat_set_satisfied_spec(a, ctx),
        caveat_set_satisfied_spec(b, ctx),
{
    let combined = a + b;
    // Elements of a are in combined[0..a.len()]
    assert forall|i: int| #![auto] 0 <= i < a.len()
        implies caveat_satisfied_spec(a[i], ctx)
    by {
        assert(combined[i] == a[i]);
    }
    // Elements of b are in combined[a.len()..a.len()+b.len()]
    assert forall|i: int| #![auto] 0 <= i < b.len()
        implies caveat_satisfied_spec(b[i], ctx)
    by {
        let idx = a.len() as int + i;
        assert(combined[idx] == b[i]);
    }
}

/// Intersection minimality: satisfying both implies satisfying the intersection.
// r[verify ucan-caveat.intersection-minimal]
pub proof fn intersection_minimal(a: Seq<Caveat>, b: Seq<Caveat>, ctx: CaveatContext)
    requires
        caveat_set_satisfied_spec(a, ctx),
        caveat_set_satisfied_spec(b, ctx),
    ensures caveat_set_satisfied_spec(caveat_intersect_spec(a, b), ctx),
{
    let combined = a + b;
    assert forall|i: int| #![auto] 0 <= i < combined.len()
        implies caveat_satisfied_spec(combined[i], ctx)
    by {
        if i < a.len() as int {
            assert(combined[i] == a[i]);
        } else {
            let bi = i - a.len() as int;
            assert(combined[i] == b[bi]);
        }
    }
}

/// Delegation only adds: child set must be at least as restrictive.
// r[impl ucan-caveat.delegation-only-adds]
// r[verify ucan-caveat.delegation-only-adds]
pub proof fn delegation_only_adds(
    parent_set: Seq<Caveat>,
    child_set: Seq<Caveat>,
    ctx: CaveatContext,
)
    requires
        caveat_set_contains_spec(parent_set, child_set),
        caveat_set_satisfied_spec(child_set, ctx),
    ensures caveat_set_satisfied_spec(parent_set, ctx),
{
    set_containment_monotone(parent_set, child_set, ctx);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_bound_satisfied() {
        let c = Caveat::TimeBound { not_after: 1000 };
        let ctx = CaveatContext {
            now: 500,
            count_in_window: 0,
        };
        assert!(caveat_satisfied(&c, &ctx));
    }

    #[test]
    fn test_time_bound_expired() {
        let c = Caveat::TimeBound { not_after: 1000 };
        let ctx = CaveatContext {
            now: 1500,
            count_in_window: 0,
        };
        assert!(!caveat_satisfied(&c, &ctx));
    }

    #[test]
    fn test_rate_limit_satisfied() {
        let c = Caveat::RateLimit {
            max_count: 100,
            window_secs: 3600,
        };
        let ctx = CaveatContext {
            now: 0,
            count_in_window: 50,
        };
        assert!(caveat_satisfied(&c, &ctx));
    }

    #[test]
    fn test_rate_limit_exceeded() {
        let c = Caveat::RateLimit {
            max_count: 100,
            window_secs: 3600,
        };
        let ctx = CaveatContext {
            now: 0,
            count_in_window: 150,
        };
        assert!(!caveat_satisfied(&c, &ctx));
    }

    #[test]
    fn test_set_conjunction() {
        let set = vec![Caveat::TimeBound { not_after: 2000 }, Caveat::RateLimit {
            max_count: 100,
            window_secs: 3600,
        }];
        // Both satisfied
        let ctx1 = CaveatContext {
            now: 1000,
            count_in_window: 50,
        };
        assert!(caveat_set_satisfied(&set, &ctx1));
        // Time bound failed
        let ctx2 = CaveatContext {
            now: 3000,
            count_in_window: 50,
        };
        assert!(!caveat_set_satisfied(&set, &ctx2));
        // Rate limit failed
        let ctx3 = CaveatContext {
            now: 1000,
            count_in_window: 200,
        };
        assert!(!caveat_set_satisfied(&set, &ctx3));
    }

    #[test]
    fn test_intersection() {
        let a = vec![Caveat::TimeBound { not_after: 2000 }];
        let b = vec![Caveat::RateLimit {
            max_count: 50,
            window_secs: 3600,
        }];
        let combined = caveat_intersect(&a, &b);
        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn test_typed_caveat_payload_shape_admission() {
        assert!(ascii_decimal_payload_is_valid(&b"12345".to_vec()));
        assert!(!ascii_decimal_payload_is_valid(&Vec::new()));
        assert!(!ascii_decimal_payload_is_valid(&b"12a45".to_vec()));

        assert!(rate_limit_payload_shape_is_valid(&b"100:3600".to_vec()));
        assert!(!rate_limit_payload_shape_is_valid(&b":3600".to_vec()));
        assert!(!rate_limit_payload_shape_is_valid(&b"100:".to_vec()));
        assert!(!rate_limit_payload_shape_is_valid(&b"100:3600:extra".to_vec()));
        assert!(!rate_limit_payload_shape_is_valid(&b"100/window".to_vec()));
    }
}
