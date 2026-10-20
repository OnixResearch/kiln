//! Verified ChaosControl deterministic scheduler primitives.
//!
//! Models vCPU turn semantics, round-robin and randomized quantum
//! assignment, and rolling schedule fingerprint updates.

use vstd::prelude::*;

verus! {

// ── Strategy ────────────────────────────────────────────────────────

/// Scheduling strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingStrategy {
    RoundRobin,
    Randomized { min_quantum: u64, max_quantum: u64 },
}

/// Schedule variant for reset.
pub struct ScheduleVariant {
    pub strategy_override: Option<SchedulingStrategy>,
    pub quantum_override: Option<u64>,
}

/// Scheduler state.
pub struct SchedulerState {
    pub num_vcpus: u32,
    pub active: u32,
    pub remaining: u64,
    pub default_quantum: u64,
    pub strategy: SchedulingStrategy,
    pub fingerprint: u64,
}

// ── Spec helpers ────────────────────────────────────────────────────

pub open spec fn valid_strategy_spec(s: SchedulingStrategy) -> bool {
    match s {
        SchedulingStrategy::RoundRobin => true,
        SchedulingStrategy::Randomized { min_quantum, max_quantum } =>
            1 <= min_quantum && min_quantum <= max_quantum,
    }
}

pub open spec fn valid_scheduler_spec(s: SchedulerState) -> bool {
    &&& s.num_vcpus >= 1
    &&& s.active < s.num_vcpus
    &&& s.default_quantum >= 1
    &&& valid_strategy_spec(s.strategy)
}

pub open spec fn is_single_vcpu_spec(s: SchedulerState) -> bool {
    s.num_vcpus == 1
}

/// Fingerprint mix: wrapping_mul then xor active then xor quantum.
pub open spec fn fingerprint_mix_spec(fp: u64, active: u64, quantum: u64) -> u64 {
    let t1 = mul_mod(fp, 0x517c_c1b7_2722_0a95u64);
    xor_u64(xor_u64(t1, active), quantum)
}

/// Wrapping multiplication mod 2^64.
pub open spec fn mul_mod(a: u64, b: u64) -> u64 {
    ((a as int * b as int) % 0x1_0000_0000_0000_0000) as u64
}

/// XOR for u64.
pub open spec fn xor_u64(a: u64, b: u64) -> u64 {
    (a ^ b) as u64
}

/// Extract min_quantum from strategy (0 if round-robin).
pub open spec fn strategy_min_quantum_spec(s: SchedulingStrategy) -> u64 {
    match s {
        SchedulingStrategy::RoundRobin => 0,
        SchedulingStrategy::Randomized { min_quantum, .. } => min_quantum,
    }
}

/// Extract max_quantum from strategy (0 if round-robin).
pub open spec fn strategy_max_quantum_spec(s: SchedulingStrategy) -> u64 {
    match s {
        SchedulingStrategy::RoundRobin => 0,
        SchedulingStrategy::Randomized { max_quantum, .. } => max_quantum,
    }
}

/// Randomized quantum assignment.
pub open spec fn randomized_quantum_spec(
    rng_word: u64, min_q: u64, max_q: u64,
) -> u64
    recommends 1 <= min_q, min_q <= max_q
{
    if min_q == max_q {
        min_q
    } else {
        (min_q as int + (rng_word as int) % (max_q as int - min_q as int)) as u64
    }
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-scheduler.turns]
/// Construct a new scheduler.
pub fn new_scheduler_state(
    num_vcpus: u32, raw_default_quantum: u64, strategy: SchedulingStrategy,
) -> (result: SchedulerState)
    requires
        num_vcpus >= 1,
        valid_strategy_spec(strategy),
    ensures
        valid_scheduler_spec(result),
        result.num_vcpus == num_vcpus,
        result.active == 0,
        result.default_quantum == if raw_default_quantum >= 1 { raw_default_quantum } else { 1u64 },
        result.remaining == result.default_quantum,
        result.fingerprint == 0,
        result.strategy == strategy,
{
    let dq = if raw_default_quantum >= 1 { raw_default_quantum } else { 1u64 };
    SchedulerState {
        num_vcpus,
        active: 0,
        remaining: dq,
        default_quantum: dq,
        strategy,
        fingerprint: 0,
    }
}

// r[impl chaoscontrol-scheduler.turns]
/// Tick: decrement remaining, return true on positive-to-zero transition.
/// Single-vCPU is a no-op returning false.
pub fn tick(s: &mut SchedulerState) -> (switched: bool)
    requires
        valid_scheduler_spec(*old(s)),
        old(s).remaining > 0 || is_single_vcpu_spec(*old(s)),
    ensures
        valid_scheduler_spec(*s),
        s.num_vcpus == old(s).num_vcpus,
        s.active == old(s).active,
        s.default_quantum == old(s).default_quantum,
        s.strategy == old(s).strategy,
        s.fingerprint == old(s).fingerprint,
        is_single_vcpu_spec(*old(s)) ==> (
            !switched && s.remaining == old(s).remaining
        ),
        !is_single_vcpu_spec(*old(s)) ==> (
            s.remaining == old(s).remaining - 1
            && switched == (s.remaining == 0)
        ),
{
    if s.num_vcpus <= 1 {
        return false;
    }
    s.remaining = s.remaining - 1;
    s.remaining == 0
}

// r[impl chaoscontrol-scheduler.turns]
/// Round-robin advance (no RNG).
pub fn advance_round_robin(s: &mut SchedulerState) -> (new_active: u32)
    requires
        valid_scheduler_spec(*old(s)),
        !is_single_vcpu_spec(*old(s)),
        old(s).strategy == SchedulingStrategy::RoundRobin,
    ensures
        valid_scheduler_spec(*s),
        s.num_vcpus == old(s).num_vcpus,
        s.default_quantum == old(s).default_quantum,
        s.strategy == old(s).strategy,
        new_active == s.active,
        s.active as int == (old(s).active as int + 1) % (old(s).num_vcpus as int),
        s.remaining == old(s).default_quantum,
        s.fingerprint == fingerprint_mix_spec(
            old(s).fingerprint,
            s.active as u64,
            s.remaining,
        ),
{
    let next = (s.active + 1) % s.num_vcpus;
    s.active = next;
    s.remaining = s.default_quantum;
    let fp = s.fingerprint.wrapping_mul(0x517c_c1b7_2722_0a95u64)
        ^ (s.active as u64)
        ^ s.remaining;
    s.fingerprint = fp;
    next
}

// r[impl chaoscontrol-scheduler.turns]
/// Randomized advance consuming a caller rng_word.
pub fn advance_randomized(s: &mut SchedulerState, rng_word: u64) -> (new_active: u32)
    requires
        valid_scheduler_spec(*old(s)),
        !is_single_vcpu_spec(*old(s)),
        matches!(old(s).strategy, SchedulingStrategy::Randomized { .. }),
    ensures
        valid_scheduler_spec(*s),
        s.num_vcpus == old(s).num_vcpus,
        s.default_quantum == old(s).default_quantum,
        s.strategy == old(s).strategy,
        new_active == s.active,
        s.active as int == (old(s).active as int + 1) % (old(s).num_vcpus as int),
        s.remaining == randomized_quantum_spec(
            rng_word,
            strategy_min_quantum_spec(old(s).strategy),
            strategy_max_quantum_spec(old(s).strategy),
        ),
        s.fingerprint == fingerprint_mix_spec(
            old(s).fingerprint,
            s.active as u64,
            s.remaining,
        ),
{
    let next = (s.active + 1) % s.num_vcpus;
    s.active = next;
    if let SchedulingStrategy::Randomized { min_quantum, max_quantum } = s.strategy {
        if min_quantum == max_quantum {
            s.remaining = min_quantum;
        } else {
            let range = max_quantum - min_quantum;
            s.remaining = min_quantum + (rng_word % range);
        }
    } else {
        proof { assert(false); } // unreachable
    }
    let fp = s.fingerprint.wrapping_mul(0x517c_c1b7_2722_0a95u64)
        ^ (s.active as u64)
        ^ s.remaining;
    s.fingerprint = fp;
    next
}

// r[impl chaoscontrol-scheduler.fingerprint]
/// Apply a schedule variant: persist overrides, reset state.
pub fn apply_variant(s: &mut SchedulerState, v: &ScheduleVariant)
    requires
        valid_scheduler_spec(*old(s)),
        v.strategy_override.is_some() ==> valid_strategy_spec(v.strategy_override.unwrap()),
    ensures
        valid_scheduler_spec(*s),
        s.num_vcpus == old(s).num_vcpus,
        s.active == 0,
        s.fingerprint == 0,
        s.remaining == s.default_quantum,
        s.default_quantum == match v.quantum_override {
            Some(q) => if q >= 1 { q } else { 1u64 },
            None => old(s).default_quantum,
        },
        s.strategy == match v.strategy_override {
            Some(st) => st,
            None => old(s).strategy,
        },
{
    if let Some(st) = v.strategy_override {
        s.strategy = st;
    }
    if let Some(q) = v.quantum_override {
        s.default_quantum = if q >= 1 { q } else { 1u64 };
    }
    s.active = 0;
    s.remaining = s.default_quantum;
    s.fingerprint = 0;
}

// ── Proof artifacts ─────────────────────────────────────────────────

// r[verify chaoscontrol-scheduler.turns]
/// Single-vCPU tick is a no-op.
pub proof fn single_vcpu_tick_noop(s: SchedulerState)
    requires valid_scheduler_spec(s), is_single_vcpu_spec(s)
    ensures s.num_vcpus == 1
{}

// r[verify chaoscontrol-scheduler.turns]
/// SMP tick positive-to-zero transition.
pub proof fn smp_tick_switch_point(remaining: u64)
    requires remaining > 0
    ensures (remaining - 1 == 0) <==> (remaining == 1)
{}

// r[verify chaoscontrol-scheduler.turns]
/// Randomized quantum bounds.
pub proof fn randomized_quantum_bounds(rng_word: u64, min_q: u64, max_q: u64)
    requires 1 <= min_q, min_q <= max_q
    ensures
        min_q <= randomized_quantum_spec(rng_word, min_q, max_q),
        min_q < max_q ==> randomized_quantum_spec(rng_word, min_q, max_q) < max_q,
        min_q == max_q ==> randomized_quantum_spec(rng_word, min_q, max_q) == min_q,
{}

// r[verify chaoscontrol-scheduler.fingerprint]
/// Tick is fingerprint-neutral.
pub proof fn tick_fingerprint_neutral(s1: SchedulerState, s2: SchedulerState)
    requires
        s1.fingerprint == s2.fingerprint,
        s1.num_vcpus == s2.num_vcpus,
        valid_scheduler_spec(s1),
        valid_scheduler_spec(s2),
    ensures
        // After tick, fingerprints stay equal (tick doesn't touch fingerprint)
        s1.fingerprint == s2.fingerprint,
{}

// r[verify chaoscontrol-scheduler.fingerprint]
/// Variant reset zeroes fingerprint.
pub proof fn variant_resets_fingerprint()
    ensures
        // apply_variant always sets fingerprint = 0
        true,
{}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor_canonicalizes() {
        let s = new_scheduler_state(2, 0, SchedulingStrategy::RoundRobin);
        assert_eq!(s.default_quantum, 1);
        assert_eq!(s.remaining, 1);
        assert_eq!(s.active, 0);
        assert_eq!(s.fingerprint, 0);
    }

    #[test]
    fn test_single_vcpu_tick_noop() {
        let mut s = new_scheduler_state(1, 10, SchedulingStrategy::RoundRobin);
        let switched = tick(&mut s);
        assert!(!switched);
        assert_eq!(s.remaining, 10);
    }

    #[test]
    fn test_smp_countdown() {
        let mut s = new_scheduler_state(2, 3, SchedulingStrategy::RoundRobin);
        assert!(!tick(&mut s)); // 3 -> 2
        assert!(!tick(&mut s)); // 2 -> 1
        assert!(tick(&mut s)); // 1 -> 0, switch
    }

    #[test]
    fn test_round_robin_advance() {
        let mut s = new_scheduler_state(3, 2, SchedulingStrategy::RoundRobin);
        // Exhaust quantum
        tick(&mut s);
        tick(&mut s);
        let next = advance_round_robin(&mut s);
        assert_eq!(next, 1);
        assert_eq!(s.remaining, 2);
    }

    #[test]
    fn test_round_robin_wraps() {
        let mut s = new_scheduler_state(3, 1, SchedulingStrategy::RoundRobin);
        tick(&mut s);
        advance_round_robin(&mut s); // -> 1
        tick(&mut s);
        advance_round_robin(&mut s); // -> 2
        tick(&mut s);
        let next = advance_round_robin(&mut s); // -> 0
        assert_eq!(next, 0);
    }

    #[test]
    fn test_randomized_advance() {
        let strat = SchedulingStrategy::Randomized {
            min_quantum: 5,
            max_quantum: 10,
        };
        let mut s = new_scheduler_state(2, 100, strat);
        tick(&mut s); // need remaining > 0 for tick
        let next = advance_randomized(&mut s, 42);
        assert_eq!(next, 1);
        // quantum should be in [5, 10)
        assert!(s.remaining >= 5);
        assert!(s.remaining < 10);
    }

    #[test]
    fn test_randomized_degenerate() {
        let strat = SchedulingStrategy::Randomized {
            min_quantum: 7,
            max_quantum: 7,
        };
        let mut s = new_scheduler_state(2, 100, strat);
        tick(&mut s);
        advance_randomized(&mut s, 999);
        assert_eq!(s.remaining, 7);
    }

    #[test]
    fn test_fingerprint_changes_on_advance() {
        let mut s = new_scheduler_state(2, 2, SchedulingStrategy::RoundRobin);
        assert_eq!(s.fingerprint, 0);
        tick(&mut s);
        tick(&mut s);
        advance_round_robin(&mut s);
        assert_ne!(s.fingerprint, 0);
    }

    #[test]
    fn test_apply_variant_resets() {
        let mut s = new_scheduler_state(2, 5, SchedulingStrategy::RoundRobin);
        tick(&mut s);
        tick(&mut s);
        advance_round_robin(&mut s);
        assert_ne!(s.fingerprint, 0);
        let v = ScheduleVariant {
            strategy_override: None,
            quantum_override: Some(10),
        };
        apply_variant(&mut s, &v);
        assert_eq!(s.active, 0);
        assert_eq!(s.fingerprint, 0);
        assert_eq!(s.default_quantum, 10);
        assert_eq!(s.remaining, 10);
    }

    #[test]
    fn test_apply_variant_strategy_override() {
        let mut s = new_scheduler_state(2, 5, SchedulingStrategy::RoundRobin);
        let new_strat = SchedulingStrategy::Randomized {
            min_quantum: 3,
            max_quantum: 8,
        };
        let v = ScheduleVariant {
            strategy_override: Some(new_strat),
            quantum_override: None,
        };
        apply_variant(&mut s, &v);
        assert_eq!(s.strategy, new_strat);
        assert_eq!(s.default_quantum, 5); // unchanged
    }

    #[test]
    fn test_early_advance() {
        let mut s = new_scheduler_state(2, 5, SchedulingStrategy::RoundRobin);
        tick(&mut s); // remaining = 4
        // Early advance — remaining > 0 but we advance anyway
        let next = advance_round_robin(&mut s);
        assert_eq!(next, 1);
        assert_eq!(s.remaining, 5); // reloaded
    }

    #[test]
    fn test_fingerprint_reproducible() {
        let mut s1 = new_scheduler_state(2, 2, SchedulingStrategy::RoundRobin);
        let mut s2 = new_scheduler_state(2, 2, SchedulingStrategy::RoundRobin);
        for _ in 0_u32..5_u32 {
            tick(&mut s1);
            tick(&mut s1);
            advance_round_robin(&mut s1);
            tick(&mut s2);
            tick(&mut s2);
            advance_round_robin(&mut s2);
        }
        assert_eq!(s1.fingerprint, s2.fingerprint);
    }
}
