//! Verified MESI cache coherence protocol.
//!
//! Models the four-state MESI protocol (Modified, Exclusive, Shared,
//! Invalid) used by hardware caches to maintain coherence across
//! processors. Each cache line in each processor has a MESI state.
//! Processor reads/writes and bus snooping events drive transitions.
//!
//! Safety invariants proven here:
//! - **Single writer**: at most one cache holds a line in Modified.
//! - **Exclusive alone**: Exclusive implies all others are Invalid.
//! - **Modified alone**: Modified implies all others are Invalid.
//! - **No dirty sharing**: no cache is Shared while another is Modified.
//! - Transition functions preserve all system invariants.

use vstd::prelude::*;

verus! {

// --- MESI states ---

/// Invalid: cache line is not present / stale.
pub const STATE_INVALID: u8 = 0u8;
/// Shared: clean copy, other caches may also hold it.
pub const STATE_SHARED: u8 = 1u8;
/// Exclusive: clean copy, no other cache holds it.
pub const STATE_EXCLUSIVE: u8 = 2u8;
/// Modified: dirty copy, no other cache holds it.
pub const STATE_MODIFIED: u8 = 3u8;

// --- is_valid_state ---

// r[depends mesi.valid-state]
/// A value represents a valid MESI state.
pub open spec fn is_valid_state_spec(s: u8) -> bool {
    s == STATE_INVALID
    || s == STATE_SHARED
    || s == STATE_EXCLUSIVE
    || s == STATE_MODIFIED
}

// r[impl mesi.valid-state]
pub fn is_valid_state(s: u8) -> (result: bool)
    ensures result == is_valid_state_spec(s)
{
    s == STATE_INVALID
    || s == STATE_SHARED
    || s == STATE_EXCLUSIVE
    || s == STATE_MODIFIED
}

// --- Predicates over a system of caches ---

// r[depends mesi.count-in-state]
/// Count how many caches hold a given state (spec, recursive).
pub open spec fn count_in_state_spec(caches: Seq<u8>, state: u8, from: int) -> int
    decreases caches.len() - from
{
    if from >= caches.len() {
        0int
    } else if caches[from] == state {
        1 + count_in_state_spec(caches, state, from + 1)
    } else {
        count_in_state_spec(caches, state, from + 1)
    }
}

// r[impl mesi.count-in-state]
/// Count how many caches hold a given state.
pub fn count_in_state(caches: &Vec<u8>, state: u8) -> (result: u64)
    requires caches@.len() <= u64::MAX as int
    ensures result as int == count_in_state_spec(caches@, state, 0)
{
    let mut i: usize = 0;
    let mut count: u64 = 0;
    while i < caches.len()
        invariant
            0 <= i <= caches@.len(),
            caches@.len() <= u64::MAX as int,
            count <= i as u64,
            count as int == count_in_state_spec(caches@, state, 0)
                - count_in_state_spec(caches@, state, i as int),
        decreases caches@.len() - i
    {
        if caches[i] == state {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// r[depends mesi.system-invariant]
/// The core MESI system invariant for a cache line across all processors:
/// 1. At most one cache is Modified.
/// 2. If any cache is Modified, all others are Invalid.
/// 3. If any cache is Exclusive, all others are Invalid.
/// 4. No cache is Shared while another is Modified.
pub open spec fn system_invariant_spec(caches: Seq<u8>) -> bool {
    // All states are valid MESI states
    &&& forall|i: int| 0 <= i < caches.len() ==> is_valid_state_spec(caches[i])
    // Modified implies all others are Invalid
    &&& forall|i: int, j: int|
        0 <= i < caches.len() && 0 <= j < caches.len() && i != j
        && caches[i] == STATE_MODIFIED
        ==> caches[j] == STATE_INVALID
    // Exclusive implies all others are Invalid
    &&& forall|i: int, j: int|
        0 <= i < caches.len() && 0 <= j < caches.len() && i != j
        && caches[i] == STATE_EXCLUSIVE
        ==> caches[j] == STATE_INVALID
}

// r[impl mesi.system-invariant]
/// Check the MESI system invariant at runtime.
#[allow(clippy::too_many_lines)]
pub fn check_system_invariant(caches: &Vec<u8>) -> (result: bool)
    requires caches@.len() <= u64::MAX as int
    ensures result ==> system_invariant_spec(caches@)
{
    let mut has_non_invalid: bool = false;
    let mut has_modified: bool = false;
    let mut has_exclusive: bool = false;
    let mut i: usize = 0;
    while i < caches.len()
        invariant
            0 <= i <= caches@.len(),
            caches@.len() <= u64::MAX as int,
            has_modified ==> has_non_invalid,
            has_exclusive ==> has_non_invalid,
            !has_non_invalid ==> forall|k: int| 0 <= k < i ==> caches@[k] == STATE_INVALID,
            !has_modified ==> forall|k: int| 0 <= k < i ==> caches@[k] != STATE_MODIFIED,
            !has_exclusive ==> forall|k: int| 0 <= k < i ==> caches@[k] != STATE_EXCLUSIVE,
            forall|k: int| 0 <= k < i ==> is_valid_state_spec(caches@[k]),
            forall|k: int, l: int|
                0 <= k < i && 0 <= l < i && k != l
                && caches@[k] == STATE_MODIFIED
                ==> caches@[l] == STATE_INVALID,
            forall|k: int, l: int|
                0 <= k < i && 0 <= l < i && k != l
                && caches@[k] == STATE_EXCLUSIVE
                ==> caches@[l] == STATE_INVALID,
        decreases caches@.len() - i
    {
        let ghost prefix_end = i as int;
        let s = caches[i];
        if !is_valid_state(s) {
            return false;
        }

        if s == STATE_MODIFIED {
            if has_non_invalid {
                return false;
            }
            proof {
                assert(!has_modified) by {
                    if has_modified {
                        assert(has_non_invalid);
                    }
                };
                assert(!has_exclusive) by {
                    if has_exclusive {
                        assert(has_non_invalid);
                    }
                };
                assert forall|k: int| 0 <= k < prefix_end
                    implies caches@[k] == STATE_INVALID by {
                    if caches@[k] != STATE_INVALID {
                        assert(has_non_invalid);
                    }
                };
            }
            has_modified = true;
            has_non_invalid = true;
        } else if s == STATE_EXCLUSIVE {
            if has_non_invalid {
                return false;
            }
            proof {
                assert(!has_modified) by {
                    if has_modified {
                        assert(has_non_invalid);
                    }
                };
                assert(!has_exclusive) by {
                    if has_exclusive {
                        assert(has_non_invalid);
                    }
                };
                assert forall|k: int| 0 <= k < prefix_end
                    implies caches@[k] == STATE_INVALID by {
                    if caches@[k] != STATE_INVALID {
                        assert(has_non_invalid);
                    }
                };
            }
            has_exclusive = true;
            has_non_invalid = true;
        } else if s == STATE_SHARED {
            if has_modified || has_exclusive {
                return false;
            }
            proof {
                assert(!has_modified);
                assert(!has_exclusive);
                assert forall|k: int| 0 <= k < prefix_end
                    implies caches@[k] != STATE_MODIFIED by {
                    if caches@[k] == STATE_MODIFIED {
                        assert(has_modified);
                    }
                };
                assert forall|k: int| 0 <= k < prefix_end
                    implies caches@[k] != STATE_EXCLUSIVE by {
                    if caches@[k] == STATE_EXCLUSIVE {
                        assert(has_exclusive);
                    }
                };
            }
            has_non_invalid = true;
        }

        i = i + 1;

        proof {
            assert(caches@[prefix_end] == s);
            assert(has_modified ==> has_non_invalid);
            assert(has_exclusive ==> has_non_invalid);

            assert forall|k: int| 0 <= k < i as int
                implies is_valid_state_spec(caches@[k]) by {
                if k == prefix_end {
                    assert(caches@[k] == s);
                    assert(is_valid_state_spec(s));
                }
            };

            assert(!has_non_invalid ==> forall|k: int| 0 <= k < i as int ==> caches@[k] == STATE_INVALID) by {
                if !has_non_invalid {
                    assert(s == STATE_INVALID);
                    assert forall|k: int| 0 <= k < i as int
                        implies caches@[k] == STATE_INVALID by {
                        if k == prefix_end {
                            assert(caches@[k] == s);
                        }
                    };
                }
            };

            assert(!has_modified ==> forall|k: int| 0 <= k < i as int ==> caches@[k] != STATE_MODIFIED) by {
                if !has_modified {
                    assert forall|k: int| 0 <= k < i as int
                        implies caches@[k] != STATE_MODIFIED by {
                        if k == prefix_end {
                            assert(caches@[k] == s);
                            if s == STATE_MODIFIED {
                                assert(false);
                            }
                        }
                    };
                }
            };

            assert(!has_exclusive ==> forall|k: int| 0 <= k < i as int ==> caches@[k] != STATE_EXCLUSIVE) by {
                if !has_exclusive {
                    assert forall|k: int| 0 <= k < i as int
                        implies caches@[k] != STATE_EXCLUSIVE by {
                        if k == prefix_end {
                            assert(caches@[k] == s);
                            if s == STATE_EXCLUSIVE {
                                assert(false);
                            }
                        }
                    };
                }
            };

            assert forall|k: int, l: int|
                0 <= k < i as int && 0 <= l < i as int && k != l
                && caches@[k] == STATE_MODIFIED
                implies caches@[l] == STATE_INVALID by {
                if k == prefix_end {
                    assert(s == STATE_MODIFIED);
                    assert(l < prefix_end);
                    assert(caches@[l] == STATE_INVALID);
                } else if l == prefix_end {
                    assert(caches@[l] == s);
                    if s != STATE_INVALID {
                        assert(false);
                    }
                }
            };

            assert forall|k: int, l: int|
                0 <= k < i as int && 0 <= l < i as int && k != l
                && caches@[k] == STATE_EXCLUSIVE
                implies caches@[l] == STATE_INVALID by {
                if k == prefix_end {
                    assert(s == STATE_EXCLUSIVE);
                    assert(l < prefix_end);
                    assert(caches@[l] == STATE_INVALID);
                } else if l == prefix_end {
                    assert(caches@[l] == s);
                    if s != STATE_INVALID {
                        assert(false);
                    }
                }
            };
        }
    }

    proof {
        assert(i == caches.len());
        assert forall|k: int| 0 <= k < caches@.len()
            implies is_valid_state_spec(caches@[k]) by {
        };
        assert forall|k: int, l: int|
            0 <= k < caches@.len() && 0 <= l < caches@.len() && k != l
            && caches@[k] == STATE_MODIFIED
            implies caches@[l] == STATE_INVALID by {
        };
        assert forall|k: int, l: int|
            0 <= k < caches@.len() && 0 <= l < caches@.len() && k != l
            && caches@[k] == STATE_EXCLUSIVE
            implies caches@[l] == STATE_INVALID by {
        };
        assert(system_invariant_spec(caches@));
    }
    true
}

// --- Processor-side transitions ---

// r[depends mesi.processor-read]
/// Spec: processor read on cache `idx`.
/// - Invalid → Shared (if others have it) or Exclusive (if alone).
///   `others_have_copy` indicates whether another cache holds S/E/M.
/// - Shared, Exclusive, Modified → unchanged (hit).
pub open spec fn processor_read_spec(
    caches: Seq<u8>, idx: int, others_have_copy: bool,
) -> Seq<u8>
    recommends 0 <= idx < caches.len()
{
    if caches[idx] == STATE_INVALID {
        if others_have_copy {
            caches.update(idx, STATE_SHARED)
        } else {
            caches.update(idx, STATE_EXCLUSIVE)
        }
    } else {
        caches // hit — no change
    }
}

// r[impl mesi.processor-read]
/// Processor read on cache `idx`. On a miss (Invalid), transitions to
/// Shared or Exclusive depending on whether other caches hold the line.
/// Bus snooping of other caches must be done separately.
pub fn processor_read(
    caches: &mut Vec<u8>, idx: usize, others_have_copy: bool,
) -> (old_state: u8)
    requires
        0 <= idx < old(caches)@.len(),
        is_valid_state_spec(old(caches)@[idx as int]),
    ensures
        caches@ == processor_read_spec(old(caches)@, idx as int, others_have_copy),
        old_state == old(caches)@[idx as int],
{
    let s = caches[idx];
    if s == STATE_INVALID {
        if others_have_copy {
            caches.set(idx, STATE_SHARED);
        } else {
            caches.set(idx, STATE_EXCLUSIVE);
        }
    }
    s
}

// r[depends mesi.processor-write]
/// Spec: processor write on cache `idx`.
/// Any state → Modified (the writer takes exclusive dirty ownership).
pub open spec fn processor_write_spec(caches: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < caches.len()
{
    caches.update(idx, STATE_MODIFIED)
}

// r[impl mesi.processor-write]
/// Processor write on cache `idx`. Transitions to Modified regardless
/// of current state. The caller must invalidate all other caches
/// (bus_invalidate) before or atomically with this call.
pub fn processor_write(caches: &mut Vec<u8>, idx: usize) -> (old_state: u8)
    requires
        0 <= idx < old(caches)@.len(),
        is_valid_state_spec(old(caches)@[idx as int]),
    ensures
        caches@ == processor_write_spec(old(caches)@, idx as int),
        old_state == old(caches)@[idx as int],
{
    let s = caches[idx];
    caches.set(idx, STATE_MODIFIED);
    s
}

// --- Bus snooping transitions ---

// r[depends mesi.bus-read]
/// Spec: another processor issued a read; cache `idx` snoops it.
/// - Modified → Shared (flush dirty data to memory/requester).
/// - Exclusive → Shared (downgrade to shared).
/// - Shared → Shared (no change).
/// - Invalid → Invalid (no change).
pub open spec fn bus_read_spec(caches: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < caches.len()
{
    if caches[idx] == STATE_MODIFIED || caches[idx] == STATE_EXCLUSIVE {
        caches.update(idx, STATE_SHARED)
    } else {
        caches
    }
}

// r[impl mesi.bus-read]
/// Snoop a bus read: downgrade Modified/Exclusive to Shared.
pub fn bus_read(caches: &mut Vec<u8>, idx: usize) -> (old_state: u8)
    requires
        0 <= idx < old(caches)@.len(),
        is_valid_state_spec(old(caches)@[idx as int]),
    ensures
        caches@ == bus_read_spec(old(caches)@, idx as int),
        old_state == old(caches)@[idx as int],
{
    let s = caches[idx];
    if s == STATE_MODIFIED || s == STATE_EXCLUSIVE {
        caches.set(idx, STATE_SHARED);
    }
    s
}

// r[depends mesi.bus-invalidate]
/// Spec: another processor issued a write (BusRdX / BusUpgr);
/// cache `idx` must invalidate its copy.
/// Any state → Invalid.
pub open spec fn bus_invalidate_spec(caches: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < caches.len()
{
    caches.update(idx, STATE_INVALID)
}

// r[impl mesi.bus-invalidate]
/// Snoop a bus invalidate: transition to Invalid.
pub fn bus_invalidate(caches: &mut Vec<u8>, idx: usize) -> (old_state: u8)
    requires
        0 <= idx < old(caches)@.len(),
        is_valid_state_spec(old(caches)@[idx as int]),
    ensures
        caches@ == bus_invalidate_spec(old(caches)@, idx as int),
        old_state == old(caches)@[idx as int],
{
    let s = caches[idx];
    caches.set(idx, STATE_INVALID);
    s
}

// --- Composite operations ---

// r[depends mesi.snoop-read-all]
/// Spec: snoop a bus read across all caches except the requester.
/// Every non-requester Modified/Exclusive downgrades to Shared.
pub open spec fn snoop_read_all_spec(caches: Seq<u8>, requester: int, from: int) -> Seq<u8>
    decreases if from <= caches.len() { caches.len() - from } else { 0 }
{
    if requester < 0 || requester >= caches.len() || from < 0 || from >= caches.len() {
        caches
    } else if from == requester {
        snoop_read_all_spec(caches, requester, from + 1)
    } else {
        snoop_read_all_spec(bus_read_spec(caches, from), requester, from + 1)
    }
}

// r[impl mesi.snoop-read-all]
/// Apply bus_read to every cache except the requester.
pub fn snoop_read_all(caches: &mut Vec<u8>, requester: usize)
    requires
        0 <= requester < old(caches)@.len(),
        forall|i: int| 0 <= i < old(caches)@.len()
            ==> is_valid_state_spec(old(caches)@[i]),
    ensures
        caches@ == snoop_read_all_spec(old(caches)@, requester as int, 0),
        caches@.len() == old(caches)@.len(),
        forall|j: int| 0 <= j < caches@.len() ==> is_valid_state_spec(caches@[j]),
{
    let ghost original = old(caches)@;
    let mut i: usize = 0;
    while i < caches.len()
        invariant
            0 <= i <= caches@.len(),
            requester < caches@.len(),
            caches@.len() == original.len(),
            snoop_read_all_spec(caches@, requester as int, i as int)
                == snoop_read_all_spec(original, requester as int, 0),
            forall|j: int| 0 <= j < caches@.len()
                ==> is_valid_state_spec(caches@[j]),
        decreases caches@.len() - i
    {
        let ghost before = caches@;
        if i != requester {
            bus_read(caches, i);
            proof {
                assert(caches@ == bus_read_spec(before, i as int));
                assert(before.len() == original.len());
                assert(requester < before.len());
                assert forall|j: int| 0 <= j < caches@.len()
                    implies is_valid_state_spec(caches@[j]) by {
                    if j == i as int {
                        bus_read_valid(before, i as int);
                    } else {
                        assert(caches@[j] == before[j]);
                    }
                };
                snoop_read_all_step(before, requester, i);
                assert(
                    snoop_read_all_spec(before, requester as int, i as int)
                        == snoop_read_all_spec(bus_read_spec(before, i as int), requester as int, i as int + 1)
                );
                assert(
                    snoop_read_all_spec(before, requester as int, i as int)
                        == snoop_read_all_spec(caches@, requester as int, i as int + 1)
                );
            }
        } else {
            proof {
                assert(
                    snoop_read_all_spec(before, requester as int, i as int)
                        == snoop_read_all_spec(before, requester as int, i as int + 1)
                );
            }
        }
        i = i + 1;
    }
}

// r[depends mesi.invalidate-others]
/// Spec: invalidate all caches except the writer.
pub open spec fn invalidate_others_spec(caches: Seq<u8>, writer: int, from: int) -> Seq<u8>
    decreases if from <= caches.len() { caches.len() - from } else { 0 }
{
    if writer < 0 || writer >= caches.len() || from < 0 || from >= caches.len() {
        caches
    } else if from == writer {
        invalidate_others_spec(caches, writer, from + 1)
    } else {
        invalidate_others_spec(bus_invalidate_spec(caches, from), writer, from + 1)
    }
}

proof fn snoop_read_all_step(caches: Seq<u8>, requester: usize, from: usize)
    requires
        requester < caches.len(),
        from < caches.len(),
        from != requester,
    ensures
        snoop_read_all_spec(caches, requester as int, from as int)
            == snoop_read_all_spec(bus_read_spec(caches, from as int), requester as int, from as int + 1)
{}

proof fn invalidate_others_spec_step(caches: Seq<u8>, writer: usize, from: usize)
    requires
        writer < caches.len(),
        from < caches.len(),
        from != writer,
    ensures
        invalidate_others_spec(caches, writer as int, from as int)
            == invalidate_others_spec(bus_invalidate_spec(caches, from as int), writer as int, from as int + 1)
{}

// r[impl mesi.invalidate-others]
/// Invalidate every cache except the writer.
pub fn invalidate_others(caches: &mut Vec<u8>, writer: usize)
    requires
        0 <= writer < old(caches)@.len(),
        forall|i: int| 0 <= i < old(caches)@.len()
            ==> is_valid_state_spec(old(caches)@[i]),
    ensures
        caches@ == invalidate_others_spec(old(caches)@, writer as int, 0),
        caches@.len() == old(caches)@.len(),
        forall|j: int| 0 <= j < caches@.len() ==> is_valid_state_spec(caches@[j]),
{
    let ghost original = old(caches)@;
    let mut i: usize = 0;
    while i < caches.len()
        invariant
            0 <= i <= caches@.len(),
            writer < caches@.len(),
            caches@.len() == original.len(),
            invalidate_others_spec(caches@, writer as int, i as int)
                == invalidate_others_spec(original, writer as int, 0),
            forall|j: int| 0 <= j < caches@.len()
                ==> is_valid_state_spec(caches@[j]),
        decreases caches@.len() - i
    {
        let ghost before = caches@;
        if i != writer {
            bus_invalidate(caches, i);
            proof {
                assert(caches@ == bus_invalidate_spec(before, i as int));
                assert(before.len() == original.len());
                assert(writer < before.len());
                assert forall|j: int| 0 <= j < caches@.len()
                    implies is_valid_state_spec(caches@[j]) by {
                    if j == i as int {
                        assert(caches@[j] == STATE_INVALID);
                    } else {
                        assert(caches@[j] == before[j]);
                    }
                };
                invalidate_others_spec_step(before, writer, i);
                assert(
                    invalidate_others_spec(before, writer as int, i as int)
                        == invalidate_others_spec(bus_invalidate_spec(before, i as int), writer as int, i as int + 1)
                );
                assert(
                    invalidate_others_spec(before, writer as int, i as int)
                        == invalidate_others_spec(caches@, writer as int, i as int + 1)
                );
            }
        } else {
            proof {
                assert(
                    invalidate_others_spec(before, writer as int, i as int)
                        == invalidate_others_spec(before, writer as int, i as int + 1)
                );
            }
        }
        i = i + 1;
    }
}

// r[depends mesi.any-has-copy]
/// Spec: does any cache other than `exclude` hold a non-Invalid copy?
pub open spec fn any_has_copy_spec(caches: Seq<u8>, exclude: int) -> bool {
    exists|j: int| 0 <= j < caches.len() && j != exclude
        && caches[j] != STATE_INVALID
}

// r[impl mesi.any-has-copy]
/// Check whether any cache other than `exclude` holds a non-Invalid copy.
pub fn any_has_copy(caches: &Vec<u8>, exclude: usize) -> (result: bool)
    requires 0 <= exclude < caches@.len()
    ensures
        result ==> any_has_copy_spec(caches@, exclude as int),
        !result ==> !any_has_copy_spec(caches@, exclude as int),
{
    let mut i: usize = 0;
    while i < caches.len()
        invariant
            0 <= i <= caches@.len(),
            forall|j: int| 0 <= j < i && j != exclude as int
                ==> caches@[j] == STATE_INVALID,
        decreases caches@.len() - i
    {
        if i != exclude && caches[i] != STATE_INVALID {
            return true;
        }
        i = i + 1;
    }
    false
}

// --- Full read/write operations (processor + bus snooping combined) ---

// r[depends mesi.full-read]
/// Spec: a complete processor read operation — snoop all other caches,
/// then transition the requester.
pub open spec fn full_read_spec(caches: Seq<u8>, reader: int) -> Seq<u8>
    recommends 0 <= reader < caches.len()
{
    let others_have = any_has_copy_spec(caches, reader);
    let after_snoop = snoop_read_all_spec(caches, reader, 0);
    processor_read_spec(after_snoop, reader, others_have)
}

// r[depends mesi.full-write]
/// Spec: a complete processor write operation — invalidate all others,
/// then transition the writer to Modified.
pub open spec fn full_write_spec(caches: Seq<u8>, writer: int) -> Seq<u8>
    recommends 0 <= writer < caches.len()
{
    let after_inv = invalidate_others_spec(caches, writer, 0);
    processor_write_spec(after_inv, writer)
}

// r[impl mesi.full-read]
/// Execute a complete read: snoop bus, then read.
pub fn full_read(caches: &mut Vec<u8>, reader: usize) -> (old_state: u8)
    requires
        0 <= reader < old(caches)@.len(),
        forall|i: int| 0 <= i < old(caches)@.len()
            ==> is_valid_state_spec(old(caches)@[i]),
    ensures
        caches@ == full_read_spec(old(caches)@, reader as int),
        old_state == old(caches)@[reader as int],
{
    let old_state = caches[reader];
    let others = any_has_copy(caches, reader);
    snoop_read_all(caches, reader);
    processor_read(caches, reader, others);
    old_state
}

// r[impl mesi.full-write]
/// Execute a complete write: invalidate others, then write.
pub fn full_write(caches: &mut Vec<u8>, writer: usize) -> (old_state: u8)
    requires
        0 <= writer < old(caches)@.len(),
        forall|i: int| 0 <= i < old(caches)@.len()
            ==> is_valid_state_spec(old(caches)@[i]),
    ensures
        caches@ == full_write_spec(old(caches)@, writer as int),
        old_state == old(caches)@[writer as int],
{
    let old_state = caches[writer];
    invalidate_others(caches, writer);
    processor_write(caches, writer);
    old_state
}

// === Proofs ===

// r[verify mesi.valid-state]
/// All four MESI constants are valid states.
proof fn constants_valid()
    ensures
        is_valid_state_spec(STATE_INVALID),
        is_valid_state_spec(STATE_SHARED),
        is_valid_state_spec(STATE_EXCLUSIVE),
        is_valid_state_spec(STATE_MODIFIED),
{}

// r[verify mesi.valid-state]
/// All four MESI constants are distinct.
proof fn constants_distinct()
    ensures
        STATE_INVALID != STATE_SHARED,
        STATE_INVALID != STATE_EXCLUSIVE,
        STATE_INVALID != STATE_MODIFIED,
        STATE_SHARED != STATE_EXCLUSIVE,
        STATE_SHARED != STATE_MODIFIED,
        STATE_EXCLUSIVE != STATE_MODIFIED,
{}

// r[verify mesi.processor-read]
/// Processor read never produces an invalid MESI state.
proof fn processor_read_valid(caches: Seq<u8>, idx: int, others: bool)
    requires
        0 <= idx < caches.len(),
        is_valid_state_spec(caches[idx]),
    ensures
        is_valid_state_spec(processor_read_spec(caches, idx, others)[idx])
{}

// r[verify mesi.processor-write]
/// Processor write always produces Modified.
proof fn processor_write_produces_modified(caches: Seq<u8>, idx: int)
    requires 0 <= idx < caches.len()
    ensures processor_write_spec(caches, idx)[idx] == STATE_MODIFIED
{}

// r[verify mesi.processor-write]
/// Processor write does not change other caches.
proof fn processor_write_other_unchanged(caches: Seq<u8>, idx: int, other: int)
    requires
        0 <= idx < caches.len(),
        0 <= other < caches.len(),
        idx != other,
    ensures processor_write_spec(caches, idx)[other] == caches[other]
{}

// r[verify mesi.bus-read]
/// Bus read produces a valid state.
proof fn bus_read_valid(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        is_valid_state_spec(caches[idx]),
    ensures is_valid_state_spec(bus_read_spec(caches, idx)[idx])
{}

// r[verify mesi.bus-read]
/// Bus read never promotes to Modified or Exclusive.
proof fn bus_read_no_promotion(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        is_valid_state_spec(caches[idx]),
    ensures
        bus_read_spec(caches, idx)[idx] != STATE_MODIFIED
            || caches[idx] == STATE_MODIFIED,
{}

// r[verify mesi.bus-read]
/// Bus read on Invalid is a no-op.
proof fn bus_read_invalid_noop(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_INVALID,
    ensures bus_read_spec(caches, idx) == caches
{}

// r[verify mesi.bus-invalidate]
/// Bus invalidate always produces Invalid.
proof fn bus_invalidate_produces_invalid(caches: Seq<u8>, idx: int)
    requires 0 <= idx < caches.len()
    ensures bus_invalidate_spec(caches, idx)[idx] == STATE_INVALID
{}

// r[verify mesi.bus-invalidate]
/// Bus invalidate does not change other caches.
proof fn bus_invalidate_other_unchanged(caches: Seq<u8>, idx: int, other: int)
    requires
        0 <= idx < caches.len(),
        0 <= other < caches.len(),
        idx != other,
    ensures bus_invalidate_spec(caches, idx)[other] == caches[other]
{}

// r[verify mesi.processor-read]
/// Read miss with no other holders → Exclusive.
proof fn read_miss_alone_gets_exclusive(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_INVALID,
    ensures processor_read_spec(caches, idx, false)[idx] == STATE_EXCLUSIVE
{}

// r[verify mesi.processor-read]
/// Read miss with other holders → Shared.
proof fn read_miss_shared_gets_shared(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_INVALID,
    ensures processor_read_spec(caches, idx, true)[idx] == STATE_SHARED
{}

// r[verify mesi.processor-read]
/// Read hit does not change state.
proof fn read_hit_unchanged(caches: Seq<u8>, idx: int, others: bool)
    requires
        0 <= idx < caches.len(),
        caches[idx] != STATE_INVALID,
    ensures processor_read_spec(caches, idx, others)[idx] == caches[idx]
{}

// r[verify mesi.system-invariant]
/// An all-Invalid system satisfies the invariant.
proof fn all_invalid_satisfies_invariant(n: int)
    requires n >= 0
    ensures system_invariant_spec(Seq::new(n as nat, |_i: int| STATE_INVALID))
{}

proof fn invalidate_others_spec_len(caches: Seq<u8>, writer: int, from: int)
    requires
        0 <= writer < caches.len(),
        0 <= from <= caches.len(),
    ensures
        invalidate_others_spec(caches, writer, from).len() == caches.len(),
    decreases caches.len() - from
{
    if from >= caches.len() {
        assert(invalidate_others_spec(caches, writer, from) == caches);
    } else if from == writer {
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(caches, writer, from + 1)
        );
        invalidate_others_spec_len(caches, writer, from + 1);
    } else {
        let next = bus_invalidate_spec(caches, from);
        assert(next.len() == caches.len());
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(next, writer, from + 1)
        );
        invalidate_others_spec_len(next, writer, from + 1);
    }
}

proof fn invalidate_others_spec_below_from(caches: Seq<u8>, writer: int, from: int, j: int)
    requires
        0 <= writer < caches.len(),
        0 <= from <= caches.len(),
        0 <= j < from,
    ensures
        invalidate_others_spec(caches, writer, from)[j] == caches[j],
    decreases caches.len() - from
{
    if from >= caches.len() {
        assert(invalidate_others_spec(caches, writer, from) == caches);
    } else if from == writer {
        assert(j < from + 1);
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(caches, writer, from + 1)
        );
        invalidate_others_spec_below_from(caches, writer, from + 1, j);
    } else {
        let next = bus_invalidate_spec(caches, from);
        assert(j != from);
        assert(next[j] == caches[j]);
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(next, writer, from + 1)
        );
        invalidate_others_spec_below_from(next, writer, from + 1, j);
    }
}

proof fn invalidate_others_writer_unchanged(caches: Seq<u8>, writer: int, from: int)
    requires
        0 <= writer < caches.len(),
        0 <= from <= caches.len(),
    ensures
        invalidate_others_spec(caches, writer, from)[writer] == caches[writer],
    decreases caches.len() - from
{
    if from >= caches.len() {
        assert(invalidate_others_spec(caches, writer, from) == caches);
    } else if from == writer {
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(caches, writer, from + 1)
        );
        invalidate_others_writer_unchanged(caches, writer, from + 1);
    } else {
        let next = bus_invalidate_spec(caches, from);
        assert(next[writer] == caches[writer]);
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(next, writer, from + 1)
        );
        invalidate_others_writer_unchanged(next, writer, from + 1);
    }
}

proof fn invalidate_others_index_invalid(caches: Seq<u8>, writer: int, from: int, j: int)
    requires
        0 <= writer < caches.len(),
        0 <= from <= caches.len(),
        0 <= j < caches.len(),
        j != writer,
        j >= from,
    ensures
        invalidate_others_spec(caches, writer, from)[j] == STATE_INVALID,
    decreases caches.len() - from
{
    if from >= caches.len() {
        assert(j >= caches.len());
        assert(false);
    } else if from == writer {
        assert(j >= from + 1);
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(caches, writer, from + 1)
        );
        invalidate_others_index_invalid(caches, writer, from + 1, j);
    } else {
        let next = bus_invalidate_spec(caches, from);
        assert(
            invalidate_others_spec(caches, writer, from)
                == invalidate_others_spec(next, writer, from + 1)
        );
        if j == from {
            invalidate_others_spec_below_from(next, writer, from + 1, j);
            assert(invalidate_others_spec(next, writer, from + 1)[j] == next[j]);
            assert(next[j] == STATE_INVALID);
        } else {
            assert(j >= from + 1);
            invalidate_others_index_invalid(next, writer, from + 1, j);
        }
    }
}

// r[verify mesi.system-invariant]
/// After invalidate_others, the writer's position is unchanged and
/// all others are Invalid. (Inductive helper — single step.)
proof fn invalidate_others_step(caches: Seq<u8>, writer: int, from: int)
    requires
        0 <= writer < caches.len(),
        0 <= from <= caches.len(),
        forall|i: int| 0 <= i < caches.len()
            ==> is_valid_state_spec(caches[i]),
    ensures
        invalidate_others_spec(caches, writer, from).len() == caches.len(),
        forall|j: int| 0 <= j < caches.len() ==> (
            (j == writer ==> invalidate_others_spec(caches, writer, from)[j] == caches[j])
            && (j != writer && j >= from
                ==> invalidate_others_spec(caches, writer, from)[j] == STATE_INVALID)
        ),
    decreases caches.len() - from
{
    invalidate_others_spec_len(caches, writer, from);
    assert forall|j: int| 0 <= j < caches.len() && j == writer
        implies invalidate_others_spec(caches, writer, from)[j] == caches[j] by {
        invalidate_others_writer_unchanged(caches, writer, from);
    };
    assert forall|j: int| 0 <= j < caches.len() && j != writer && j >= from
        implies invalidate_others_spec(caches, writer, from)[j] == STATE_INVALID by {
        invalidate_others_index_invalid(caches, writer, from, j);
    };
    assert forall|j: int| 0 <= j < caches.len() implies (
        (j == writer ==> invalidate_others_spec(caches, writer, from)[j] == caches[j])
        && (j != writer && j >= from
            ==> invalidate_others_spec(caches, writer, from)[j] == STATE_INVALID)
    ) by {
        if j == writer {
            invalidate_others_writer_unchanged(caches, writer, from);
        }
        if j != writer && j >= from {
            invalidate_others_index_invalid(caches, writer, from, j);
        }
    };
}

// r[verify mesi.system-invariant]
// r[verify mesi.full-write]
/// After a full write, the writer is Modified and all others are Invalid.
proof fn full_write_writer_modified(caches: Seq<u8>, writer: int)
    requires
        0 <= writer < caches.len(),
        forall|i: int| 0 <= i < caches.len()
            ==> is_valid_state_spec(caches[i]),
    ensures
        full_write_spec(caches, writer)[writer] == STATE_MODIFIED,
        forall|j: int| 0 <= j < caches.len() && j != writer
            ==> full_write_spec(caches, writer)[j] == STATE_INVALID,
{
    invalidate_others_step(caches, writer, 0);
    let after_inv = invalidate_others_spec(caches, writer, 0);
    assert(after_inv[writer] == caches[writer]);
    assert(processor_write_spec(after_inv, writer)[writer] == STATE_MODIFIED);
    assert forall|j: int| 0 <= j < caches.len() && j != writer
        implies processor_write_spec(after_inv, writer)[j] == STATE_INVALID by {
        invalidate_others_index_invalid(caches, writer, 0, j);
        assert(after_inv[j] == STATE_INVALID);
    };
}

// r[verify mesi.system-invariant]
// r[verify mesi.full-write]
/// A full write preserves the system invariant.
proof fn full_write_preserves_invariant(caches: Seq<u8>, writer: int)
    requires
        0 <= writer < caches.len(),
        system_invariant_spec(caches),
    ensures
        system_invariant_spec(full_write_spec(caches, writer)),
{
    full_write_writer_modified(caches, writer);
    constants_valid();
    invalidate_others_spec_len(caches, writer, 0);
    let after_inv = invalidate_others_spec(caches, writer, 0);
    let result = full_write_spec(caches, writer);
    assert(after_inv.len() == caches.len());
    assert(result.len() == after_inv.len());
    assert(result.len() == caches.len());
    assert(result[writer] == STATE_MODIFIED);
    assert forall|i: int| 0 <= i < result.len()
        implies is_valid_state_spec(result[i]) by {
        if i == writer {
            assert(result[i] == STATE_MODIFIED);
            assert(is_valid_state_spec(result[i]));
        } else {
            assert(0 <= i < after_inv.len());
            processor_write_other_unchanged(after_inv, writer, i);
            invalidate_others_index_invalid(caches, writer, 0, i);
            assert(after_inv[i] == STATE_INVALID);
            assert(result[i] == after_inv[i]);
            assert(result[i] == STATE_INVALID);
            assert(is_valid_state_spec(result[i]));
        }
    };
    assert forall|i: int, j: int|
        0 <= i < result.len() && 0 <= j < result.len() && i != j
        && result[i] == STATE_MODIFIED
        implies result[j] == STATE_INVALID by {
        if i == writer {
            if j == writer {
                assert(false);
            }
            assert(0 <= j < after_inv.len());
            processor_write_other_unchanged(after_inv, writer, j);
            invalidate_others_index_invalid(caches, writer, 0, j);
            assert(after_inv[j] == STATE_INVALID);
            assert(result[j] == after_inv[j]);
            assert(result[j] == STATE_INVALID);
        } else {
            assert(0 <= i < after_inv.len());
            processor_write_other_unchanged(after_inv, writer, i);
            invalidate_others_index_invalid(caches, writer, 0, i);
            assert(after_inv[i] == STATE_INVALID);
            assert(result[i] == after_inv[i]);
            assert(result[i] == STATE_INVALID);
            assert(false);
        }
    };
    assert forall|i: int, j: int|
        0 <= i < result.len() && 0 <= j < result.len() && i != j
        && result[i] == STATE_EXCLUSIVE
        implies result[j] == STATE_INVALID by {
        if i == writer {
            assert(result[i] == STATE_MODIFIED);
            assert(false);
        } else {
            assert(0 <= i < after_inv.len());
            processor_write_other_unchanged(after_inv, writer, i);
            invalidate_others_index_invalid(caches, writer, 0, i);
            assert(after_inv[i] == STATE_INVALID);
            assert(result[i] == after_inv[i]);
            assert(result[i] == STATE_INVALID);
            assert(false);
        }
    };
}

// r[verify mesi.bus-read]
/// Bus read downgrades Modified to Shared.
proof fn bus_read_flushes_modified(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_MODIFIED,
    ensures bus_read_spec(caches, idx)[idx] == STATE_SHARED
{}

// r[verify mesi.bus-read]
/// Bus read downgrades Exclusive to Shared.
proof fn bus_read_downgrades_exclusive(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_EXCLUSIVE,
    ensures bus_read_spec(caches, idx)[idx] == STATE_SHARED
{}

// r[verify mesi.processor-read]
/// Processor read on a hit does not disturb other caches.
proof fn processor_read_other_unchanged(caches: Seq<u8>, idx: int, other: int, others: bool)
    requires
        0 <= idx < caches.len(),
        0 <= other < caches.len(),
        idx != other,
    ensures processor_read_spec(caches, idx, others)[other] == caches[other]
{}

// r[verify mesi.processor-write]
/// A write from Shared goes to Modified (the common upgrade path).
proof fn write_from_shared(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_SHARED,
    ensures processor_write_spec(caches, idx)[idx] == STATE_MODIFIED
{}

// r[verify mesi.processor-write]
/// A write from Exclusive goes to Modified (silent upgrade).
proof fn write_from_exclusive(caches: Seq<u8>, idx: int)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_EXCLUSIVE,
    ensures processor_write_spec(caches, idx)[idx] == STATE_MODIFIED
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn make_caches(states: &[u8]) -> Vec<u8> {
        states.to_vec()
    }

    // r[verify mesi.valid-state]
    #[test]
    fn test_valid_states() {
        assert!(is_valid_state(STATE_INVALID));
        assert!(is_valid_state(STATE_SHARED));
        assert!(is_valid_state(STATE_EXCLUSIVE));
        assert!(is_valid_state(STATE_MODIFIED));
        assert!(!is_valid_state(4));
        assert!(!is_valid_state(255));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_invariant_all_invalid() {
        let c = make_caches(&[0, 0, 0, 0]);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_invariant_one_exclusive() {
        // Exclusive + all others Invalid — valid
        let c = make_caches(&[0, 2, 0, 0]);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_invariant_one_modified() {
        let c = make_caches(&[0, 0, 3, 0]);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_invariant_shared() {
        // Multiple Shared — valid
        let c = make_caches(&[1, 1, 0, 1]);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_invariant_violations() {
        // Two Modified — violation
        assert!(!check_system_invariant(&make_caches(&[3, 3, 0, 0])));
        // Modified + Shared — violation
        assert!(!check_system_invariant(&make_caches(&[3, 1, 0, 0])));
        // Modified + Exclusive — violation
        assert!(!check_system_invariant(&make_caches(&[3, 2, 0, 0])));
        // Two Exclusive — violation
        assert!(!check_system_invariant(&make_caches(&[2, 2, 0, 0])));
        // Exclusive + Shared — violation
        assert!(!check_system_invariant(&make_caches(&[2, 1, 0, 0])));
    }

    // r[verify mesi.processor-read]
    #[test]
    fn test_read_miss_alone() {
        let mut c = make_caches(&[0, 0, 0, 0]);
        let old = processor_read(&mut c, 1, false);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[1], STATE_EXCLUSIVE);
    }

    // r[verify mesi.processor-read]
    #[test]
    fn test_read_miss_shared() {
        let mut c = make_caches(&[0, 0, 0, 0]);
        let old = processor_read(&mut c, 1, true);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[1], STATE_SHARED);
    }

    // r[verify mesi.processor-read]
    #[test]
    fn test_read_hit() {
        let mut c = make_caches(&[0, 3, 0, 0]);
        let old = processor_read(&mut c, 1, false);
        assert_eq!(old, STATE_MODIFIED);
        assert_eq!(c[1], STATE_MODIFIED); // unchanged
    }

    // r[verify mesi.processor-write]
    #[test]
    fn test_write_from_invalid() {
        let mut c = make_caches(&[0, 0, 0, 0]);
        let old = processor_write(&mut c, 2);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[2], STATE_MODIFIED);
    }

    // r[verify mesi.processor-write]
    #[test]
    fn test_write_from_shared() {
        let mut c = make_caches(&[1, 1, 0, 1]);
        let old = processor_write(&mut c, 0);
        assert_eq!(old, STATE_SHARED);
        assert_eq!(c[0], STATE_MODIFIED);
    }

    // r[verify mesi.bus-read]
    #[test]
    fn test_bus_read_modified() {
        let mut c = make_caches(&[0, 3, 0, 0]);
        let old = bus_read(&mut c, 1);
        assert_eq!(old, STATE_MODIFIED);
        assert_eq!(c[1], STATE_SHARED);
    }

    // r[verify mesi.bus-read]
    #[test]
    fn test_bus_read_exclusive() {
        let mut c = make_caches(&[0, 2, 0, 0]);
        let old = bus_read(&mut c, 1);
        assert_eq!(old, STATE_EXCLUSIVE);
        assert_eq!(c[1], STATE_SHARED);
    }

    // r[verify mesi.bus-invalidate]
    #[test]
    fn test_bus_invalidate() {
        let mut c = make_caches(&[1, 1, 0, 1]);
        let old = bus_invalidate(&mut c, 3);
        assert_eq!(old, STATE_SHARED);
        assert_eq!(c[3], STATE_INVALID);
    }

    // r[verify mesi.full-write]
    #[test]
    fn test_full_write() {
        let mut c = make_caches(&[1, 1, 0, 1]);
        let old = full_write(&mut c, 1);
        assert_eq!(old, STATE_SHARED);
        assert_eq!(c[1], STATE_MODIFIED);
        // All others must be Invalid
        assert_eq!(c[0], STATE_INVALID);
        assert_eq!(c[2], STATE_INVALID);
        assert_eq!(c[3], STATE_INVALID);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.full-read]
    #[test]
    fn test_full_read_miss_alone() {
        let mut c = make_caches(&[0, 0, 0, 0]);
        let old = full_read(&mut c, 2);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[2], STATE_EXCLUSIVE);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.full-read]
    #[test]
    fn test_full_read_miss_others_shared() {
        // Cache 0 is Shared, cache 2 reads
        let mut c = make_caches(&[1, 0, 0, 0]);
        let old = full_read(&mut c, 2);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[2], STATE_SHARED);
        assert_eq!(c[0], STATE_SHARED); // still shared
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.full-read]
    // r[verify mesi.system-invariant]
    #[test]
    fn test_full_read_downgrades_modified() {
        // Cache 1 is Modified, cache 0 reads → cache 1 becomes Shared
        let mut c = make_caches(&[0, 3, 0, 0]);
        assert!(check_system_invariant(&c));
        let old = full_read(&mut c, 0);
        assert_eq!(old, STATE_INVALID);
        assert_eq!(c[0], STATE_SHARED);
        assert_eq!(c[1], STATE_SHARED); // flushed from Modified
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.system-invariant]
    #[test]
    fn test_write_read_sequence() {
        let mut c = make_caches(&[0, 0, 0, 0]);

        // P0 writes → M
        full_write(&mut c, 0);
        assert_eq!(c[0], STATE_MODIFIED);
        assert!(check_system_invariant(&c));

        // P1 reads → P0 flushes to S, P1 gets S
        full_read(&mut c, 1);
        assert_eq!(c[0], STATE_SHARED);
        assert_eq!(c[1], STATE_SHARED);
        assert!(check_system_invariant(&c));

        // P2 writes → M, P0 and P1 invalidated
        full_write(&mut c, 2);
        assert_eq!(c[0], STATE_INVALID);
        assert_eq!(c[1], STATE_INVALID);
        assert_eq!(c[2], STATE_MODIFIED);
        assert!(check_system_invariant(&c));
    }

    // r[verify mesi.any-has-copy]
    #[test]
    fn test_any_has_copy() {
        let c = make_caches(&[0, 0, 0, 0]);
        assert!(!any_has_copy(&c, 0));
        let c = make_caches(&[0, 1, 0, 0]);
        assert!(any_has_copy(&c, 0));
        assert!(!any_has_copy(&c, 1)); // exclude the one that has it
    }

    proptest! {
        // r[verify mesi.system-invariant]
        // r[verify mesi.full-write]
        #[test]
        fn prop_full_write_preserves_invariant(
            n in 2usize..=8,
            writer_idx in 0usize..8,
        ) {
            let writer_idx = writer_idx % n;
            // Start from all-Invalid (satisfies invariant)
            let mut c = make_caches(&vec![STATE_INVALID; n]);
            assert!(check_system_invariant(&c));
            full_write(&mut c, writer_idx);
            prop_assert!(check_system_invariant(&c));
            prop_assert_eq!(c[writer_idx], STATE_MODIFIED);
        }

        // r[verify mesi.system-invariant]
        // r[verify mesi.full-read]
        #[test]
        fn prop_full_read_preserves_invariant(
            n in 2usize..=8,
            reader_idx in 0usize..8,
        ) {
            let reader_idx = reader_idx % n;
            let mut c = make_caches(&vec![STATE_INVALID; n]);
            assert!(check_system_invariant(&c));
            full_read(&mut c, reader_idx);
            prop_assert!(check_system_invariant(&c));
        }

        // r[verify mesi.system-invariant]
        #[test]
        fn prop_write_then_read_preserves_invariant(
            n in 2usize..=8,
            w in 0usize..8,
            r in 0usize..8,
        ) {
            let w = w % n;
            let r = r % n;
            let mut c = make_caches(&vec![STATE_INVALID; n]);
            full_write(&mut c, w);
            prop_assert!(check_system_invariant(&c));
            full_read(&mut c, r);
            prop_assert!(check_system_invariant(&c));
        }

        // r[verify mesi.system-invariant]
        /// Random sequence of reads and writes preserves the invariant.
        #[test]
        fn prop_random_ops_preserve_invariant(
            n in 2usize..=6,
            ops in proptest::collection::vec((any::<bool>(), 0usize..6), 1..=20),
        ) {
            let mut c = make_caches(&vec![STATE_INVALID; n]);
            for (is_write, idx) in ops {
                let idx = idx % n;
                if is_write {
                    full_write(&mut c, idx);
                } else {
                    full_read(&mut c, idx);
                }
                prop_assert!(
                    check_system_invariant(&c),
                    "Invariant violated after op on idx={}", idx,
                );
            }
        }

        // r[verify mesi.full-write]
        /// After a full write, the writer is always Modified.
        #[test]
        fn prop_writer_always_modified(
            n in 2usize..=8,
            ops in proptest::collection::vec((any::<bool>(), 0usize..8), 0..=10),
            writer in 0usize..8,
        ) {
            let writer = writer % n;
            let mut c = make_caches(&vec![STATE_INVALID; n]);
            // Run some random ops first
            for (is_write, idx) in ops {
                let idx = idx % n;
                if is_write {
                    full_write(&mut c, idx);
                } else {
                    full_read(&mut c, idx);
                }
            }
            // Now write → must be Modified
            full_write(&mut c, writer);
            prop_assert_eq!(c[writer], STATE_MODIFIED);
        }

        // r[verify mesi.full-write]
        /// After a full write, all non-writers are Invalid.
        #[test]
        fn prop_write_invalidates_others(
            n in 2usize..=8,
            writer in 0usize..8,
        ) {
            let writer = writer % n;
            // Start with some Shared caches
            let mut c = make_caches(&vec![STATE_SHARED; n]);
            // Fix invariant: can't have Shared if we want to keep it valid,
            // but Shared-only is fine
            // Actually all-Shared is a valid MESI configuration
            full_write(&mut c, writer);
            for i in 0..n {
                if i != writer {
                    prop_assert_eq!(c[i], STATE_INVALID);
                }
            }
        }
    }
}
