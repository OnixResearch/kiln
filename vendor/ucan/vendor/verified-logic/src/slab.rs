//! Verified slab allocation arithmetic.
//!
//! Fixed-size slot allocation over a contiguous region. Given a base
//! address, slot size, and region capacity, compute which slot an
//! address belongs to, where a slot starts, and how many slots fit.
//! Generation counters for ABA-safe handle validation. Handle packing
//! combines an index and generation into a single u64.

// Handle packing uses u32→u64 casts verified by Verus bit_vector proofs.
#![allow(clippy::cast_lossless)]

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_div_is_ordered;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
use vstd::prelude::*;

verus! {

// --- slot_index ---

// r[depends slab.slot-index]
/// Spec: slot index for an address within the slab.
pub open spec fn slot_index_spec(addr: u64, base: u64, slot_size: u64) -> u64
    recommends slot_size > 0, addr >= base
{
    ((addr - base) as int / slot_size as int) as u64
}

// r[impl slab.slot-index]
/// Returns `(addr - base) / slot_size`.
pub fn slot_index(addr: u64, base: u64, slot_size: u64) -> (result: u64)
    requires slot_size > 0, addr >= base
    ensures result == slot_index_spec(addr, base, slot_size)
{
    (addr - base) / slot_size
}

// r[verify slab.slot-index]
/// Slot index at the base is zero.
proof fn slot_index_at_base(base: u64, slot_size: u64)
    requires slot_size > 0
    ensures slot_index_spec(base, base, slot_size) == 0
{
    lemma_fundamental_div_mod(0int, slot_size as int);
}

// r[verify slab.slot-index]
/// Slot index is bounded by capacity.
proof fn slot_index_bounded(addr: u64, base: u64, slot_size: u64, region_size: u64)
    requires
        slot_size > 0,
        addr >= base,
        (addr as int - base as int) < (region_size as int),
    ensures slot_index_spec(addr, base, slot_size) as int
        <= slab_capacity_spec(region_size, slot_size) as int
{
    let offset = (addr - base) as int;
    let ss = slot_size as int;
    lemma_fundamental_div_mod(offset, ss);
    lemma_fundamental_div_mod(region_size as int, ss);
    lemma_div_is_ordered(offset, region_size as int, ss);
}

// --- slot_offset ---

// r[depends slab.slot-offset]
/// Spec: byte offset within the slot.
pub open spec fn slot_offset_spec(addr: u64, base: u64, slot_size: u64) -> u64
    recommends slot_size > 0, addr >= base
{
    ((addr - base) as int % slot_size as int) as u64
}

// r[impl slab.slot-offset]
/// Returns `(addr - base) % slot_size`.
pub fn slot_offset(addr: u64, base: u64, slot_size: u64) -> (result: u64)
    requires slot_size > 0, addr >= base
    ensures result == slot_offset_spec(addr, base, slot_size)
{
    (addr - base) % slot_size
}

// r[verify slab.slot-offset]
/// Offset is strictly less than slot_size.
proof fn slot_offset_bounded(addr: u64, base: u64, slot_size: u64)
    requires slot_size > 0, addr >= base
    ensures slot_offset_spec(addr, base, slot_size) < slot_size
{
    lemma_fundamental_div_mod((addr - base) as int, slot_size as int);
}

// r[verify slab.slot-offset]
/// Offset at the base is zero.
proof fn slot_offset_at_base(base: u64, slot_size: u64)
    requires slot_size > 0
    ensures slot_offset_spec(base, base, slot_size) == 0
{
    lemma_fundamental_div_mod(0int, slot_size as int);
}

// --- is_slot_aligned ---

// r[depends slab.is-slot-aligned]
/// Spec: address is aligned to a slot boundary.
pub open spec fn is_slot_aligned_spec(addr: u64, base: u64, slot_size: u64) -> bool
    recommends slot_size > 0, addr >= base
{
    (addr - base) as int % slot_size as int == 0
}

// r[impl slab.is-slot-aligned]
/// Returns true if addr is on a slot boundary.
pub fn is_slot_aligned(addr: u64, base: u64, slot_size: u64) -> (result: bool)
    requires slot_size > 0, addr >= base
    ensures result == is_slot_aligned_spec(addr, base, slot_size)
{
    (addr - base) % slot_size == 0
}

// r[verify slab.is-slot-aligned]
/// Aligned iff zero offset.
proof fn aligned_iff_zero_offset(addr: u64, base: u64, slot_size: u64)
    requires slot_size > 0, addr >= base
    ensures is_slot_aligned_spec(addr, base, slot_size)
        == (slot_offset_spec(addr, base, slot_size) == 0)
{}

// r[verify slab.is-slot-aligned]
/// Base is always slot-aligned.
proof fn base_is_aligned(base: u64, slot_size: u64)
    requires slot_size > 0
    ensures is_slot_aligned_spec(base, base, slot_size)
{
    lemma_fundamental_div_mod(0int, slot_size as int);
}

// --- slot_address ---

// r[depends slab.slot-address]
/// Spec: start address of the given slot.
pub open spec fn slot_address_spec(index: u64, base: u64, slot_size: u64) -> u64
    recommends
        slot_size > 0,
        base as int + index as int * slot_size as int <= u64::MAX as int,
{
    (base as int + index as int * slot_size as int) as u64
}

// r[impl slab.slot-address]
/// Returns `base + index * slot_size`.
pub fn slot_address(index: u64, base: u64, slot_size: u64) -> (result: u64)
    requires
        slot_size > 0,
        base as int + index as int * slot_size as int <= u64::MAX as int,
    ensures result == slot_address_spec(index, base, slot_size)
{
    base + index * slot_size
}

// r[verify slab.slot-address]
/// Slot 0 starts at the base.
proof fn slot_zero_at_base(base: u64, slot_size: u64)
    requires slot_size > 0
    ensures slot_address_spec(0, base, slot_size) == base
{}

// r[verify slab.slot-address]
/// slot_address is always slot-aligned.
proof fn slot_address_aligned(index: u64, base: u64, slot_size: u64)
    requires
        slot_size > 0,
        base as int + index as int * slot_size as int <= u64::MAX as int,
    ensures is_slot_aligned_spec(
        slot_address_spec(index, base, slot_size), base, slot_size,
    )
{
    let addr = base as int + index as int * slot_size as int;
    let ss = slot_size as int;
    // addr - base == index * slot_size, so (addr - base) % slot_size == 0
    assert((index as int * ss) % ss == 0) by(nonlinear_arith)
        requires ss > 0;
}

// r[verify slab.slot-address]
// r[verify slab.slot-index]
/// Roundtrip: slot_address(slot_index(addr)) == addr for aligned addresses.
proof fn slot_address_index_roundtrip(addr: u64, base: u64, slot_size: u64)
    requires
        slot_size > 0,
        addr >= base,
        is_slot_aligned_spec(addr, base, slot_size),
        addr as int - base as int + slot_size as int <= u64::MAX as int,
    ensures slot_address_spec(
        slot_index_spec(addr, base, slot_size), base, slot_size,
    ) == addr
{
    let offset = (addr - base) as int;
    let ss = slot_size as int;
    lemma_fundamental_div_mod(offset, ss);
    // offset == ss * (offset / ss) + offset % ss
    // aligned: offset % ss == 0
    // so offset == ss * (offset / ss)
    // slot_address = base + (offset / ss) * ss = base + offset = addr
    assert((offset / ss) * ss == offset) by(nonlinear_arith)
        requires ss > 0, offset == ss * (offset / ss) + 0int, offset % ss == 0;
}

// r[verify slab.slot-index]
// r[verify slab.slot-address]
/// Roundtrip: slot_index(slot_address(i)) == i.
proof fn slot_index_address_roundtrip(index: u64, base: u64, slot_size: u64)
    requires
        slot_size > 0,
        base as int + index as int * slot_size as int <= u64::MAX as int,
    ensures slot_index_spec(
        slot_address_spec(index, base, slot_size), base, slot_size,
    ) == index
{
    let ss = slot_size as int;
    let product = index as int * ss;
    // slot_address - base = index * slot_size
    // slot_index = (index * slot_size) / slot_size = index
    lemma_fundamental_div_mod(product, ss);
    assert(product / ss == index as int) by(nonlinear_arith)
        requires ss > 0, product == ss * (product / ss) + product % ss,
            0 <= product % ss, product % ss < ss, product == index as int * ss;
}

// --- slab_capacity ---

// r[depends slab.slab-capacity]
/// Spec: number of whole slots that fit in the region.
pub open spec fn slab_capacity_spec(region_size: u64, slot_size: u64) -> u64
    recommends slot_size > 0
{
    if slot_size == 0 { 0 } else { (region_size as int / slot_size as int) as u64 }
}

// r[impl slab.slab-capacity]
/// Returns `region_size / slot_size`.
pub fn slab_capacity(region_size: u64, slot_size: u64) -> (result: u64)
    requires slot_size > 0
    ensures result == slab_capacity_spec(region_size, slot_size)
{
    region_size / slot_size
}

// r[verify slab.slab-capacity]
/// Zero region has zero capacity.
proof fn zero_region_zero_capacity(slot_size: u64)
    requires slot_size > 0
    ensures slab_capacity_spec(0, slot_size) == 0
{
    lemma_fundamental_div_mod(0int, slot_size as int);
}

// r[verify slab.slab-capacity]
/// Exact multiple: capacity * slot_size == region_size.
proof fn exact_capacity(region_size: u64, slot_size: u64)
    requires
        slot_size > 0,
        region_size as int % slot_size as int == 0,
    ensures slab_capacity_spec(region_size, slot_size) as int * slot_size as int
        == region_size as int
{
    lemma_fundamental_div_mod(region_size as int, slot_size as int);
    assert(
        (region_size as int / slot_size as int) * slot_size as int == region_size as int
    ) by(nonlinear_arith)
        requires
            slot_size as int > 0,
            region_size as int == slot_size as int * (region_size as int / slot_size as int)
                + region_size as int % slot_size as int,
            region_size as int % slot_size as int == 0;
}

// r[verify slab.slab-capacity]
/// Capacity is bounded: capacity * slot_size <= region_size.
proof fn capacity_bounded(region_size: u64, slot_size: u64)
    requires slot_size > 0
    ensures slab_capacity_spec(region_size, slot_size) as int * slot_size as int
        <= region_size as int
{
    lemma_fundamental_div_mod(region_size as int, slot_size as int);
    let q = region_size as int / slot_size as int;
    let ss = slot_size as int;
    assert(q * ss <= region_size as int) by(nonlinear_arith)
        requires ss > 0, region_size as int == ss * q + region_size as int % ss,
            0 <= region_size as int % ss;
}

// --- generation_valid ---

// r[depends slab.generation-valid]
/// Spec: generation counter is within valid range.
pub open spec fn generation_valid_spec(g: u32, max_gen: u32) -> bool {
    g < max_gen
}

// r[impl slab.generation-valid]
/// Returns true if g < `max_gen`.
pub fn generation_valid(g: u32, max_gen: u32) -> (result: bool)
    ensures result == generation_valid_spec(g, max_gen)
{
    g < max_gen
}

// r[depends slab.generation-valid]
/// Spec: next generation value.
pub open spec fn generation_increment_spec(g: u32) -> u32 {
    (g + 1) as u32
}

// r[impl slab.generation-valid]
/// Returns g + 1.
pub fn generation_increment(g: u32) -> (result: u32)
    requires g < u32::MAX
    ensures result == generation_increment_spec(g)
{
    g + 1
}

// r[verify slab.generation-valid]
/// Incremented generation is valid if there's room.
proof fn increment_preserves_valid(g: u32, max_gen: u32)
    requires
        generation_valid_spec(g, max_gen),
        (max_gen as int) >= 2,
    ensures generation_valid_spec(generation_increment_spec(g), max_gen)
        || generation_increment_spec(g) == max_gen
{}

// r[verify slab.generation-valid]
/// If g is invalid, g+1 is also invalid (monotonic invalidity).
proof fn invalid_stays_invalid(g: u32, max_gen: u32)
    requires !generation_valid_spec(g, max_gen), g < u32::MAX
    ensures !generation_valid_spec(generation_increment_spec(g), max_gen)
{}

// --- handle packing ---

// The handle layout: low 32 bits = index, high 32 bits = generation.

// r[depends slab.handle-pack]
/// Spec: pack index and generation into a u64 handle.
pub open spec fn handle_pack_spec(index: u32, generation: u32) -> u64 {
    index as u64 | ((generation as u64) << 32u64)
}

// r[impl slab.handle-pack]
/// Packs index (low 32) and generation (high 32) into a u64.
pub fn handle_pack(index: u32, generation: u32) -> (result: u64)
    ensures result == handle_pack_spec(index, generation)
{
    (index as u64) | ((generation as u64) << 32)
}

// r[depends slab.handle-pack]
/// Spec: extract index from a packed handle.
pub open spec fn handle_unpack_index_spec(handle: u64) -> u32 {
    (handle & 0xFFFF_FFFFu64) as u32
}

// r[impl slab.handle-pack]
/// Extracts the low 32 bits (index) from a handle.
pub fn handle_unpack_index(handle: u64) -> (result: u32)
    ensures result == handle_unpack_index_spec(handle)
{
    (handle & 0xFFFF_FFFF) as u32
}

// r[depends slab.handle-pack]
/// Spec: extract generation from a packed handle.
pub open spec fn handle_unpack_generation_spec(handle: u64) -> u32 {
    ((handle >> 32u64) & 0xFFFF_FFFFu64) as u32
}

// r[impl slab.handle-pack]
/// Extracts the high 32 bits (generation) from a handle.
pub fn handle_unpack_generation(handle: u64) -> (result: u32)
    ensures result == handle_unpack_generation_spec(handle)
{
    ((handle >> 32) & 0xFFFF_FFFF) as u32
}

// r[verify slab.handle-pack]
/// Pack-unpack roundtrip for index.
proof fn handle_roundtrip_index(index: u32, generation: u32)
    ensures handle_unpack_index_spec(handle_pack_spec(index, generation)) == index
{
    assert(
        (index as u64 | ((generation as u64) << 32u64)) & 0xFFFF_FFFFu64 == index as u64
    ) by(bit_vector);
}

// r[verify slab.handle-pack]
/// Pack-unpack roundtrip for generation.
proof fn handle_roundtrip_generation(index: u32, generation: u32)
    ensures handle_unpack_generation_spec(handle_pack_spec(index, generation)) == generation
{
    assert(
        ((index as u64 | ((generation as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64
            == generation as u64
    ) by(bit_vector);
}

// r[verify slab.handle-pack]
/// Different (index, generation) pairs produce different handles.
proof fn handle_pack_injective(i1: u32, g1: u32, i2: u32, g2: u32)
    requires handle_pack_spec(i1, g1) == handle_pack_spec(i2, g2)
    ensures i1 == i2, g1 == g2
{
    // Extract from same handle gives same components
    assert(
        (i1 as u64 | ((g1 as u64) << 32u64)) & 0xFFFF_FFFFu64
        == (i2 as u64 | ((g2 as u64) << 32u64)) & 0xFFFF_FFFFu64
    );
    assert(
        (i1 as u64 | ((g1 as u64) << 32u64)) & 0xFFFF_FFFFu64 == i1 as u64
    ) by(bit_vector);
    assert(
        (i2 as u64 | ((g2 as u64) << 32u64)) & 0xFFFF_FFFFu64 == i2 as u64
    ) by(bit_vector);
    assert(
        ((i1 as u64 | ((g1 as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64 == g1 as u64
    ) by(bit_vector);
    assert(
        ((i2 as u64 | ((g2 as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64 == g2 as u64
    ) by(bit_vector);
}

// --- decomposition roundtrip ---

// r[verify slab.slot-index]
// r[verify slab.slot-offset]
/// Fundamental decomposition: slot_index * slot_size + slot_offset == addr - base.
proof fn decomposition_roundtrip(addr: u64, base: u64, slot_size: u64)
    requires slot_size > 0, addr >= base
    ensures
        slot_index_spec(addr, base, slot_size) as int * slot_size as int
            + slot_offset_spec(addr, base, slot_size) as int
            == (addr - base) as int
{
    let offset = (addr - base) as int;
    let ss = slot_size as int;
    lemma_fundamental_div_mod(offset, ss);
    assert((offset / ss) * ss + offset % ss == offset) by(nonlinear_arith)
        requires ss > 0, offset == ss * (offset / ss) + offset % ss;
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify slab.slot-index]
    #[test]
    fn test_slot_index() {
        assert_eq!(slot_index(100, 100, 16), 0);
        assert_eq!(slot_index(116, 100, 16), 1);
        assert_eq!(slot_index(132, 100, 16), 2);
        assert_eq!(slot_index(108, 100, 16), 0); // within first slot
    }

    // r[verify slab.slot-offset]
    #[test]
    fn test_slot_offset() {
        assert_eq!(slot_offset(100, 100, 16), 0);
        assert_eq!(slot_offset(105, 100, 16), 5);
        assert_eq!(slot_offset(115, 100, 16), 15);
        assert_eq!(slot_offset(116, 100, 16), 0); // next slot boundary
    }

    // r[verify slab.is-slot-aligned]
    #[test]
    fn test_is_slot_aligned() {
        assert!(is_slot_aligned(100, 100, 16));
        assert!(!is_slot_aligned(105, 100, 16));
        assert!(is_slot_aligned(116, 100, 16));
        assert!(is_slot_aligned(132, 100, 16));
    }

    // r[verify slab.slot-address]
    #[test]
    fn test_slot_address() {
        assert_eq!(slot_address(0, 100, 16), 100);
        assert_eq!(slot_address(1, 100, 16), 116);
        assert_eq!(slot_address(2, 100, 16), 132);
        assert_eq!(slot_address(0, 0, 64), 0);
    }

    // r[verify slab.slab-capacity]
    #[test]
    fn test_slab_capacity() {
        assert_eq!(slab_capacity(0, 16), 0);
        assert_eq!(slab_capacity(16, 16), 1);
        assert_eq!(slab_capacity(32, 16), 2);
        assert_eq!(slab_capacity(31, 16), 1); // partial slot doesn't count
        assert_eq!(slab_capacity(1024, 64), 16);
    }

    // r[verify slab.generation-valid]
    #[test]
    fn test_generation() {
        assert!(generation_valid(0, 100));
        assert!(generation_valid(99, 100));
        assert!(!generation_valid(100, 100));
        assert!(!generation_valid(101, 100));
        assert_eq!(generation_increment(0), 1);
        assert_eq!(generation_increment(99), 100);
    }

    // r[verify slab.handle-pack]
    #[test]
    fn test_handle_pack_unpack() {
        let h = handle_pack(42, 7);
        assert_eq!(handle_unpack_index(h), 42);
        assert_eq!(handle_unpack_generation(h), 7);

        let h = handle_pack(0, 0);
        assert_eq!(handle_unpack_index(h), 0);
        assert_eq!(handle_unpack_generation(h), 0);

        let h = handle_pack(u32::MAX, u32::MAX);
        assert_eq!(handle_unpack_index(h), u32::MAX);
        assert_eq!(handle_unpack_generation(h), u32::MAX);
    }

    proptest! {
        // r[verify slab.slot-index]
        // r[verify slab.slot-offset]
        #[test]
        fn prop_decomposition_roundtrip(
            base in 0u64..=u64::MAX / 4,
            offset in 0u64..=10000,
            slot_size in 1u64..=256,
        ) {
            let addr = base + offset;
            let idx = slot_index(addr, base, slot_size);
            let off = slot_offset(addr, base, slot_size);
            let reconstructed = u128::from(idx) * u128::from(slot_size) + u128::from(off);
            prop_assert_eq!(reconstructed, u128::from(offset));
        }

        // r[verify slab.slot-offset]
        #[test]
        fn prop_offset_bounded(
            base in 0u64..=u64::MAX / 4,
            offset in 0u64..=10000,
            slot_size in 1u64..=256,
        ) {
            let addr = base + offset;
            prop_assert!(slot_offset(addr, base, slot_size) < slot_size);
        }

        // r[verify slab.is-slot-aligned]
        #[test]
        fn prop_aligned_iff_zero_offset(
            base in 0u64..=u64::MAX / 4,
            offset in 0u64..=10000,
            slot_size in 1u64..=256,
        ) {
            let addr = base + offset;
            prop_assert_eq!(
                is_slot_aligned(addr, base, slot_size),
                slot_offset(addr, base, slot_size) == 0
            );
        }

        // r[verify slab.slot-address]
        // r[verify slab.slot-index]
        #[test]
        fn prop_address_index_roundtrip(
            base in 0u64..=u64::MAX / 4,
            index in 0u64..=1000,
            slot_size in 1u64..=256,
        ) {
            if u128::from(base) + u128::from(index) * u128::from(slot_size) <= u128::from(u64::MAX) {
                let addr = slot_address(index, base, slot_size);
                prop_assert_eq!(slot_index(addr, base, slot_size), index);
            }
        }

        // r[verify slab.slot-address]
        #[test]
        fn prop_slot_address_aligned(
            base in 0u64..=u64::MAX / 4,
            index in 0u64..=1000,
            slot_size in 1u64..=256,
        ) {
            if u128::from(base) + u128::from(index) * u128::from(slot_size) <= u128::from(u64::MAX) {
                let addr = slot_address(index, base, slot_size);
                prop_assert!(is_slot_aligned(addr, base, slot_size));
            }
        }

        // r[verify slab.slab-capacity]
        #[test]
        fn prop_capacity_bounded(
            region_size in 0u64..=100_000,
            slot_size in 1u64..=256,
        ) {
            let cap = slab_capacity(region_size, slot_size);
            prop_assert!(u128::from(cap) * u128::from(slot_size) <= u128::from(region_size));
        }

        // r[verify slab.handle-pack]
        #[test]
        fn prop_handle_roundtrip(index: u32, generation: u32) {
            let h = handle_pack(index, generation);
            prop_assert_eq!(handle_unpack_index(h), index);
            prop_assert_eq!(handle_unpack_generation(h), generation);
        }

        // r[verify slab.handle-pack]
        #[test]
        fn prop_handle_injective(i1: u32, g1: u32, i2: u32, g2: u32) {
            if handle_pack(i1, g1) == handle_pack(i2, g2) {
                prop_assert_eq!(i1, i2);
                prop_assert_eq!(g1, g2);
            }
        }

        // r[verify slab.generation-valid]
        #[test]
        fn prop_invalid_stays_invalid(g: u32, max_gen: u32) {
            if !generation_valid(g, max_gen) && g < u32::MAX {
                prop_assert!(!generation_valid(generation_increment(g), max_gen));
            }
        }
    }
}
