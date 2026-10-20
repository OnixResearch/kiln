//! Verified set-associative cache lookup.
//!
//! Models the address decomposition used by a conventional set-associative
//! cache: split a 64-bit address into tag/index/offset fields, then search a
//! selected set for the requested tag. Proofs cover decomposition roundtrip,
//! component bounds, hit/miss classification, duplicate-tag exclusion,
//! false-hit prevention, and cross-set isolation.

use vstd::prelude::*;

verus! {

// r[depends set-assoc.decomposition]
/// Bitmask for the low `bits` bits.
pub open spec fn low_mask_spec(bits: u64) -> u64
    recommends bits <= 64
{
    if bits == 0 {
        0
    } else if bits == 64 {
        u64::MAX
    } else {
        sub(1u64 << bits, 1u64)
    }
}

// r[impl set-assoc.decomposition]
/// Runtime low-bit mask.
pub fn low_mask(bits: u64) -> (result: u64)
    requires bits <= 64
    ensures result == low_mask_spec(bits)
{
    if bits == 0 {
        0
    } else if bits == 64 {
        u64::MAX
    } else {
        proof {
            assert(bits > 0u64 && bits < 64u64);
            assert((1u64 << bits) >= 1u64) by(bit_vector)
                requires bits > 0u64, bits < 64u64;
        }
        (1u64 << bits) - 1u64
    }
}

// r[depends set-assoc.component-bounds]
/// Whether `value` fits in `bits` bits.
pub open spec fn fits_in_bits_spec(value: u64, bits: u64) -> bool
    recommends bits <= 64
{
    if bits == 64 { true } else { value < (1u64 << bits) }
}

// r[impl set-assoc.component-bounds]
/// Runtime bit-width check.
pub fn fits_in_bits(value: u64, bits: u64) -> (result: bool)
    requires bits <= 64
    ensures result == fits_in_bits_spec(value, bits)
{
    if bits == 64 { true } else { value < (1u64 << bits) }
}

/// Right shift that treats `shift == 64` as zero.
pub open spec fn safe_shr_spec(value: u64, shift: u64) -> u64
    recommends shift <= 64
{
    if shift == 64 { 0 } else { value >> shift }
}

// r[impl set-assoc.decomposition]
pub fn safe_shr(value: u64, shift: u64) -> (result: u64)
    requires shift <= 64
    ensures result == safe_shr_spec(value, shift)
{
    if shift == 64 { 0 } else { value >> shift }
}

/// Left shift that treats `shift == 64` as zero. Callers must ensure this only
/// happens for values that fit in zero bits.
pub open spec fn safe_shl_spec(value: u64, shift: u64) -> u64
    recommends shift <= 64
{
    if shift == 64 { 0 } else { value << shift }
}

// r[impl set-assoc.decomposition]
pub fn safe_shl(value: u64, shift: u64) -> (result: u64)
    requires shift <= 64
    ensures result == safe_shl_spec(value, shift)
{
    if shift == 64 { 0 } else { value << shift }
}

/// Number of tag bits left after offset and index.
pub open spec fn tag_bits_spec(offset_bits: u64, index_bits: u64) -> u64
    recommends offset_bits + index_bits <= 64
{
    let total_bits = (offset_bits as int + index_bits as int) as u64;
    if total_bits == 64 { 0 } else { (64int - total_bits as int) as u64 }
}

// r[depends set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
/// Extract low offset bits.
pub open spec fn extract_offset_spec(addr: u64, offset_bits: u64) -> u64
    recommends offset_bits <= 64
{
    addr & low_mask_spec(offset_bits)
}

// r[impl set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
pub fn extract_offset(addr: u64, offset_bits: u64) -> (result: u64)
    requires offset_bits <= 64
    ensures result == extract_offset_spec(addr, offset_bits)
{
    addr & low_mask(offset_bits)
}

// r[depends set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
/// Extract set index bits above the offset.
pub open spec fn extract_index_spec(addr: u64, offset_bits: u64, index_bits: u64) -> u64
    recommends offset_bits + index_bits <= 64
{
    if index_bits == 0 {
        0
    } else {
        safe_shr_spec(addr, offset_bits) & low_mask_spec(index_bits)
    }
}

// r[impl set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
pub fn extract_index(addr: u64, offset_bits: u64, index_bits: u64) -> (result: u64)
    requires offset_bits + index_bits <= 64
    ensures result == extract_index_spec(addr, offset_bits, index_bits)
{
    if index_bits == 0 {
        0
    } else {
        safe_shr(addr, offset_bits) & low_mask(index_bits)
    }
}

// r[depends set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
/// Extract tag bits above index+offset.
pub open spec fn extract_tag_spec(addr: u64, offset_bits: u64, index_bits: u64) -> u64
    recommends offset_bits + index_bits <= 64
{
    let shift = (offset_bits as int + index_bits as int) as u64;
    safe_shr_spec(addr, shift)
}

// r[impl set-assoc.decomposition]
// r[impl set-assoc.component-bounds]
pub fn extract_tag(addr: u64, offset_bits: u64, index_bits: u64) -> (result: u64)
    requires offset_bits + index_bits <= 64
    ensures result == extract_tag_spec(addr, offset_bits, index_bits)
{
    safe_shr(addr, offset_bits + index_bits)
}

proof fn fits_zero_bits_zero()
    ensures fits_in_bits_spec(0, 0)
{
    assert(0u64 < (1u64 << 0u64)) by(bit_vector);
}

proof fn tag_bits_zero(offset_bits: u64, index_bits: u64)
    requires offset_bits == 0, index_bits == 0
    ensures tag_bits_spec(offset_bits, index_bits) == 64
{}

proof fn tag_bits_full(offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits == 64
    ensures tag_bits_spec(offset_bits, index_bits) == 0
{}

proof fn tag_bits_partial(offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits < 64
    ensures tag_bits_spec(offset_bits, index_bits)
        == (64int - offset_bits as int - index_bits as int) as u64
{}

proof fn extract_offset_full_width(addr: u64)
    ensures extract_offset_spec(addr, 64) == addr
{
    assert(addr & u64::MAX == addr) by(bit_vector);
}

proof fn extract_offset_zero_bits(addr: u64)
    ensures extract_offset_spec(addr, 0) == 0
{
    assert(low_mask_spec(0) == 0);
    assert(addr & 0u64 == 0u64) by(bit_vector);
}

proof fn extract_index_zero_bits(addr: u64, offset_bits: u64)
    requires offset_bits <= 64
    ensures extract_index_spec(addr, offset_bits, 0) == 0
{}

proof fn extract_index_full_width(addr: u64)
    ensures extract_index_spec(addr, 0, 64) == addr
{
    assert((addr >> 0u64) == addr) by(bit_vector);
    assert(safe_shr_spec(addr, 0) == addr);
    assert(low_mask_spec(64) == u64::MAX);
    assert(addr & u64::MAX == addr) by(bit_vector);
}

proof fn extract_tag_zero_width(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits == 64
    ensures extract_tag_spec(addr, offset_bits, index_bits) == 0
{}

// r[depends set-assoc.decomposition]
/// Full address decomposition.
pub open spec fn decompose_address_spec(addr: u64, offset_bits: u64, index_bits: u64) -> (u64, u64, u64)
    recommends offset_bits + index_bits <= 64
{
    (
        extract_tag_spec(addr, offset_bits, index_bits),
        extract_index_spec(addr, offset_bits, index_bits),
        extract_offset_spec(addr, offset_bits),
    )
}

// r[impl set-assoc.decomposition]
pub fn decompose_address(addr: u64, offset_bits: u64, index_bits: u64) -> (result: (u64, u64, u64))
    requires offset_bits + index_bits <= 64
    ensures result == decompose_address_spec(addr, offset_bits, index_bits)
{
    (
        extract_tag(addr, offset_bits, index_bits),
        extract_index(addr, offset_bits, index_bits),
        extract_offset(addr, offset_bits),
    )
}

// r[depends set-assoc.decomposition]
/// Reconstruct a full address from (tag, index, offset).
pub open spec fn reconstruct_address_spec(
    tag: u64,
    index: u64,
    offset: u64,
    offset_bits: u64,
    index_bits: u64,
) -> u64
    recommends
        offset_bits + index_bits <= 64,
        fits_in_bits_spec(offset, offset_bits),
        fits_in_bits_spec(index, index_bits),
        fits_in_bits_spec(tag, tag_bits_spec(offset_bits, index_bits)),
{
    let total_bits = (offset_bits as int + index_bits as int) as u64;
    safe_shl_spec(tag, total_bits)
        | safe_shl_spec(index, offset_bits)
        | offset
}

// r[impl set-assoc.decomposition]
pub fn reconstruct_address(
    tag: u64,
    index: u64,
    offset: u64,
    offset_bits: u64,
    index_bits: u64,
) -> (result: u64)
    requires
        offset_bits + index_bits <= 64,
        fits_in_bits_spec(offset, offset_bits),
        fits_in_bits_spec(index, index_bits),
        fits_in_bits_spec(tag, tag_bits_spec(offset_bits, index_bits)),
    ensures result == reconstruct_address_spec(tag, index, offset, offset_bits, index_bits)
{
    safe_shl(tag, offset_bits + index_bits)
        | safe_shl(index, offset_bits)
        | offset
}

// r[verify set-assoc.component-bounds]
proof fn offset_component_fits(addr: u64, offset_bits: u64)
    requires offset_bits <= 64
    ensures fits_in_bits_spec(extract_offset_spec(addr, offset_bits), offset_bits)
{
    if offset_bits < 64 {
        assert((addr & low_mask_spec(offset_bits)) < (1u64 << offset_bits)) by(bit_vector)
            requires offset_bits < 64u64;
    }
}

// r[verify set-assoc.component-bounds]
proof fn index_component_fits(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits <= 64
    ensures fits_in_bits_spec(extract_index_spec(addr, offset_bits, index_bits), index_bits)
{
    if index_bits < 64 {
        if offset_bits == 64 {
            assert(index_bits == 0) by(nonlinear_arith)
                requires offset_bits + index_bits <= 64, offset_bits == 64;
            extract_index_zero_bits(addr, offset_bits);
            fits_zero_bits_zero();
        } else {
            assert((safe_shr_spec(addr, offset_bits) & low_mask_spec(index_bits)) < (1u64 << index_bits)) by(bit_vector)
                requires index_bits < 64u64, offset_bits < 64u64;
        }
    } else {
        assert(index_bits == 64) by(nonlinear_arith)
            requires index_bits >= 64u64, index_bits <= 64u64;
        assert(offset_bits == 0) by(nonlinear_arith)
            requires offset_bits + index_bits <= 64, index_bits == 64;
    }
}

// r[verify set-assoc.component-bounds]
proof fn tag_component_fits(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits <= 64
    ensures fits_in_bits_spec(extract_tag_spec(addr, offset_bits, index_bits), tag_bits_spec(offset_bits, index_bits))
{
    let total_bits: u64 = (offset_bits as int + index_bits as int) as u64;
    if total_bits == 0 {
        tag_bits_zero(offset_bits, index_bits);
    } else if total_bits == 64 {
        tag_bits_full(offset_bits, index_bits);
        extract_tag_zero_width(addr, offset_bits, index_bits);
        fits_zero_bits_zero();
    } else {
        tag_bits_partial(offset_bits, index_bits);
        let bits: u64 = (64int - total_bits as int) as u64;
        assert(bits > 0u64 && bits < 64u64) by(nonlinear_arith)
            requires total_bits > 0u64, total_bits < 64u64, bits == (64int - total_bits as int) as u64;
        assert((addr >> total_bits) < (1u64 << bits)) by(bit_vector)
            requires total_bits < 64u64, bits < 64u64, bits > 0u64, total_bits + bits == 64u64;
    }
}

// r[verify set-assoc.decomposition]
proof fn decompose_reconstruct_roundtrip(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits + index_bits <= 64
    ensures reconstruct_address_spec(
        extract_tag_spec(addr, offset_bits, index_bits),
        extract_index_spec(addr, offset_bits, index_bits),
        extract_offset_spec(addr, offset_bits),
        offset_bits,
        index_bits,
    ) == addr
{
    offset_component_fits(addr, offset_bits);
    index_component_fits(addr, offset_bits, index_bits);
    tag_component_fits(addr, offset_bits, index_bits);

    let total_bits: u64 = (offset_bits as int + index_bits as int) as u64;
    if total_bits < 64 {
        assert(offset_bits < 64) by(nonlinear_arith)
            requires total_bits < 64, offset_bits <= total_bits;
        assert(index_bits < 64) by(nonlinear_arith)
            requires total_bits < 64, index_bits <= total_bits;
        if index_bits == 0 {
            assert(total_bits == offset_bits);
            extract_index_zero_bits(addr, offset_bits);
            assert(
                (((addr >> offset_bits) << offset_bits)
                    | (addr & low_mask_spec(offset_bits)))
                == addr
            ) by(bit_vector)
                requires offset_bits < 64u64;
            assert(reconstruct_address_spec(
                extract_tag_spec(addr, offset_bits, index_bits),
                extract_index_spec(addr, offset_bits, index_bits),
                extract_offset_spec(addr, offset_bits),
                offset_bits,
                index_bits,
            ) == (((addr >> offset_bits) << offset_bits)
                | (addr & low_mask_spec(offset_bits)))) by(bit_vector)
                requires offset_bits < 64u64, index_bits == 0u64;
        } else if offset_bits == 0 {
            assert(total_bits == index_bits);
            extract_offset_zero_bits(addr);
            assert(
                (((addr >> index_bits) << index_bits)
                    | (addr & low_mask_spec(index_bits)))
                == addr
            ) by(bit_vector)
                requires index_bits < 64u64;
            assert(reconstruct_address_spec(
                extract_tag_spec(addr, offset_bits, index_bits),
                extract_index_spec(addr, offset_bits, index_bits),
                extract_offset_spec(addr, offset_bits),
                offset_bits,
                index_bits,
            ) == (((addr >> index_bits) << index_bits)
                | (addr & low_mask_spec(index_bits)))) by(bit_vector)
                requires offset_bits == 0u64, index_bits < 64u64;
        } else {
            assert(
                (((addr >> total_bits) << total_bits)
                    | (((addr >> offset_bits) & low_mask_spec(index_bits)) << offset_bits)
                    | (addr & low_mask_spec(offset_bits)))
                == addr
            ) by(bit_vector)
                requires total_bits < 64u64, offset_bits < 64u64, index_bits < 64u64, offset_bits > 0u64, index_bits > 0u64,
                    total_bits == (offset_bits + index_bits) as u64;
            assert(reconstruct_address_spec(
                extract_tag_spec(addr, offset_bits, index_bits),
                extract_index_spec(addr, offset_bits, index_bits),
                extract_offset_spec(addr, offset_bits),
                offset_bits,
                index_bits,
            ) == (((addr >> total_bits) << total_bits)
                | (((addr >> offset_bits) & low_mask_spec(index_bits)) << offset_bits)
                | (addr & low_mask_spec(offset_bits))));
        }
    } else if offset_bits == 64 {
        assert(index_bits == 0) by(nonlinear_arith)
            requires total_bits == 64, offset_bits == 64, offset_bits + index_bits == total_bits;
        extract_tag_zero_width(addr, offset_bits, index_bits);
        extract_index_zero_bits(addr, offset_bits);
        extract_offset_full_width(addr);
        assert(reconstruct_address_spec(
            extract_tag_spec(addr, offset_bits, index_bits),
            extract_index_spec(addr, offset_bits, index_bits),
            extract_offset_spec(addr, offset_bits),
            offset_bits,
            index_bits,
        ) == addr) by(bit_vector)
            requires offset_bits == 64u64, index_bits == 0u64;
    } else if offset_bits == 0 {
        assert(index_bits == 64) by(nonlinear_arith)
            requires total_bits == 64, offset_bits == 0, offset_bits + index_bits == total_bits;
        extract_tag_zero_width(addr, offset_bits, index_bits);
        extract_index_full_width(addr);
        extract_offset_zero_bits(addr);
        assert(reconstruct_address_spec(
            extract_tag_spec(addr, offset_bits, index_bits),
            extract_index_spec(addr, offset_bits, index_bits),
            extract_offset_spec(addr, offset_bits),
            offset_bits,
            index_bits,
        ) == addr) by(bit_vector)
            requires offset_bits == 0u64, index_bits == 64u64;
    } else {
        assert(0 < offset_bits < 64) by(nonlinear_arith)
            requires total_bits == 64, offset_bits != 0, offset_bits != 64, offset_bits + index_bits == total_bits;
        assert(index_bits < 64) by(nonlinear_arith)
            requires total_bits == 64, offset_bits > 0, offset_bits + index_bits == total_bits;
        assert(extract_tag_spec(addr, offset_bits, index_bits) == 0);
        assert(
            ((((addr >> offset_bits) & low_mask_spec(index_bits)) << offset_bits)
                | (addr & low_mask_spec(offset_bits)))
            == addr
        ) by(bit_vector)
            requires offset_bits > 0u64, offset_bits < 64u64, index_bits < 64u64, offset_bits + index_bits == 64u64;
        assert(reconstruct_address_spec(
            extract_tag_spec(addr, offset_bits, index_bits),
            extract_index_spec(addr, offset_bits, index_bits),
            extract_offset_spec(addr, offset_bits),
            offset_bits,
            index_bits,
        ) == ((((addr >> offset_bits) & low_mask_spec(index_bits)) << offset_bits)
            | (addr & low_mask_spec(offset_bits)))) by(bit_vector)
            requires offset_bits > 0u64, offset_bits < 64u64, index_bits < 64u64, offset_bits + index_bits == 64u64;
        assert(reconstruct_address_spec(
            extract_tag_spec(addr, offset_bits, index_bits),
            extract_index_spec(addr, offset_bits, index_bits),
            extract_offset_spec(addr, offset_bits),
            offset_bits,
            index_bits,
        ) == addr);
    }
}

/// Convert exec lookup results to the spec representation.
pub open spec fn option_usize_to_int_spec(value: Option<usize>) -> Option<int> {
    match value {
        Some(idx) => Some(idx as int),
        None => None,
    }
}

// r[depends set-assoc.hit]
// r[depends set-assoc.miss]
/// First matching way index at or after `from`.
pub open spec fn find_way_from_spec(set: Seq<Option<u64>>, tag: u64, from: int) -> Option<int>
    decreases set.len() - from
{
    if from < 0 || from >= set.len() {
        None
    } else if set[from] == Some(tag) {
        Some(from)
    } else {
        find_way_from_spec(set, tag, from + 1)
    }
}

// r[depends set-assoc.hit]
// r[depends set-assoc.miss]
/// First matching way in the set.
pub open spec fn find_way_spec(set: Seq<Option<u64>>, tag: u64) -> Option<int> {
    find_way_from_spec(set, tag, 0)
}

// r[depends set-assoc.hit]
// r[depends set-assoc.miss]
/// Lookup using the address-derived tag in an already-selected set.
pub open spec fn lookup_set_spec(set: Seq<Option<u64>>, addr: u64, offset_bits: u64, index_bits: u64) -> Option<int>
    recommends offset_bits + index_bits <= 64
{
    find_way_spec(set, extract_tag_spec(addr, offset_bits, index_bits))
}

proof fn find_way_skip_nonmatch(set: Seq<Option<u64>>, tag: u64, from: int)
    requires 0 <= from < set.len(), set[from] != Some(tag)
    ensures find_way_from_spec(set, tag, from) == find_way_from_spec(set, tag, from + 1)
{}

proof fn find_way_result_has_tag(set: Seq<Option<u64>>, tag: u64, from: int, idx: int)
    requires find_way_from_spec(set, tag, from) == Some(idx)
    ensures from <= idx < set.len(), set[idx] == Some(tag)
    decreases set.len() - from
{
    if 0 <= from < set.len() {
        if set[from] == Some(tag) {
            assert(idx == from);
        } else {
            find_way_result_has_tag(set, tag, from + 1, idx);
        }
    }
}

proof fn find_way_none_absent_from(set: Seq<Option<u64>>, tag: u64, from: int)
    requires
        0 <= from <= set.len(),
        find_way_from_spec(set, tag, from).is_none(),
    ensures forall|i: int| from <= i < set.len() ==> set[i] != Some(tag)
    decreases set.len() - from
{
    if from < set.len() {
        assert(set[from] != Some(tag));
        find_way_none_absent_from(set, tag, from + 1);
    }
}

// r[impl set-assoc.hit]
// r[impl set-assoc.miss]
pub fn find_way(set: &[Option<u64>], tag: u64) -> (result: Option<usize>)
    ensures option_usize_to_int_spec(result) == find_way_spec(set@, tag)
{
    let mut i: usize = 0;
    while i < set.len()
        invariant
            0 <= i <= set@.len(),
            forall|j: int| 0 <= j < i as int ==> set@[j] != Some(tag),
            find_way_spec(set@, tag) == find_way_from_spec(set@, tag, i as int),
        decreases set@.len() - i as int
    {
        if set[i] == Some(tag) {
            assert(find_way_from_spec(set@, tag, i as int) == Some(i as int));
            assert(find_way_spec(set@, tag) == Some(i as int));
            return Some(i);
        }
        proof {
            find_way_skip_nonmatch(set@, tag, i as int);
        }
        i = i + 1;
    }
    assert(find_way_from_spec(set@, tag, set@.len() as int) == None::<int>);
    None
}

// r[impl set-assoc.hit]
// r[impl set-assoc.miss]
pub fn lookup_set(set: &[Option<u64>], addr: u64, offset_bits: u64, index_bits: u64) -> (result: Option<usize>)
    requires offset_bits + index_bits <= 64
    ensures option_usize_to_int_spec(result) == lookup_set_spec(set@, addr, offset_bits, index_bits)
{
    let tag = extract_tag(addr, offset_bits, index_bits);
    let found = find_way(set, tag);
    assert(tag == extract_tag_spec(addr, offset_bits, index_bits));
    found
}

// r[verify set-assoc.hit]
proof fn hit_returns_matching_tag(set: Seq<Option<u64>>, tag: u64, idx: int)
    requires find_way_spec(set, tag) == Some(idx)
    ensures 0 <= idx < set.len(), set[idx] == Some(tag)
{
    find_way_result_has_tag(set, tag, 0, idx);
}

// r[verify set-assoc.miss]
proof fn miss_means_tag_absent(set: Seq<Option<u64>>, tag: u64)
    requires find_way_spec(set, tag).is_none()
    ensures forall|i: int| 0 <= i < set.len() ==> set[i] != Some(tag)
{
    find_way_none_absent_from(set, tag, 0);
}

/// x is strictly between lo and hi.
pub open spec fn in_open_range_spec(lo: int, x: int, hi: int) -> bool {
    lo < x && x < hi
}

/// A duplicate-tag pair inside a set.
pub open spec fn duplicate_pair_spec(set: Seq<Option<u64>>, i: int, j: int) -> bool {
    &&& 0 <= i < j < set.len()
    &&& set[i].is_some()
    &&& set[i] == set[j]
}

// r[impl set-assoc.no-duplicates]
/// No two distinct ways hold the same non-empty tag.
pub open spec fn no_duplicate_tags_spec(set: Seq<Option<u64>>) -> bool {
    forall|i: int, j: int| !duplicate_pair_spec(set, i, j)
}

// r[impl set-assoc.insert-preserves-uniqueness]
/// Insert `tag` into way `idx`.
pub open spec fn insert_tag_spec(set: Seq<Option<u64>>, idx: int, tag: u64) -> Seq<Option<u64>>
    recommends 0 <= idx < set.len()
{
    set.update(idx, Some(tag))
}

// r[impl set-assoc.no-duplicates]
pub fn check_no_duplicate_tags(set: &[Option<u64>]) -> (result: bool)
    ensures result == no_duplicate_tags_spec(set@)
{
    let mut i: usize = 0;
    while i < set.len()
        invariant
            0 <= i <= set@.len(),
            forall|a: int, b: int| (0 <= a && a < i as int && a < b && b < set@.len()) ==> !duplicate_pair_spec(set@, a, b),
        decreases set@.len() - i as int
    {
        if let Some(tag) = set[i] {
            let mut j: usize = i + 1;
            while j < set.len()
                invariant
                    0 <= i < set@.len(),
                    i as int + 1 <= j as int <= set@.len(),
                    set@[i as int] == Some(tag),
                    forall|a: int, b: int| (0 <= a && a < i as int && a < b && b < set@.len()) ==> !duplicate_pair_spec(set@, a, b),
                    forall|b: int| in_open_range_spec(i as int, b, j as int) ==> set@[b] != Some(tag),
                decreases set@.len() - j as int
            {
                if set[j] == Some(tag) {
                    assert(duplicate_pair_spec(set@, i as int, j as int));
                    assert(!no_duplicate_tags_spec(set@));
                    return false;
                }
                assert(set@[j as int] != Some(tag));
                j = j + 1;
            }
            assert(j == set.len());
            assert forall|b: int|
                in_open_range_spec(i as int, b, set@.len() as int)
                    implies !duplicate_pair_spec(set@, i as int, b) by {
                assert(set@[i as int] == Some(tag));
            };
        }
        assert forall|a: int, b: int|
            (0 <= a && a < i as int + 1 && a < b && b < set@.len())
                implies !duplicate_pair_spec(set@, a, b) by {
            if a < i as int {
            } else {
                assert(a == i as int);
                if set@[i as int].is_some() {
                } else {
                    assert(!set@[i as int].is_some());
                }
            }
        };
        i = i + 1;
    }
    true
}

// r[verify set-assoc.no-duplicates]
proof fn matching_way_is_unique(set: Seq<Option<u64>>, tag: u64, hit_idx: int, other_idx: int)
    requires
        no_duplicate_tags_spec(set),
        find_way_spec(set, tag) == Some(hit_idx),
        0 <= other_idx < set.len(),
        set[other_idx] == Some(tag),
    ensures hit_idx == other_idx
{
    hit_returns_matching_tag(set, tag, hit_idx);
    if hit_idx < other_idx {
        assert(duplicate_pair_spec(set, hit_idx, other_idx));
        assert(false);
    } else if other_idx < hit_idx {
        assert(duplicate_pair_spec(set, other_idx, hit_idx));
        assert(false);
    }
}

// r[verify set-assoc.insert-preserves-uniqueness]
proof fn insert_empty_preserves_no_duplicates(set: Seq<Option<u64>>, idx: int, tag: u64)
    requires
        0 <= idx < set.len(),
        no_duplicate_tags_spec(set),
        set[idx].is_none(),
        find_way_spec(set, tag).is_none(),
    ensures no_duplicate_tags_spec(insert_tag_spec(set, idx, tag))
{
    let updated = insert_tag_spec(set, idx, tag);
    find_way_none_absent_from(set, tag, 0);
    assert forall|i: int, j: int| duplicate_pair_spec(updated, i, j) implies false by {
        if i == idx {
            assert(updated[i] == Some(tag));
            assert(j != idx);
            assert(updated[j] == set[j]);
            assert(set[j] == Some(tag));
            assert(false);
        } else if j == idx {
            assert(updated[j] == Some(tag));
            assert(i != idx);
            assert(updated[i] == set[i]);
            assert(set[i] == Some(tag));
            assert(false);
        } else {
            assert(updated[i] == set[i]);
            assert(updated[j] == set[j]);
            assert(duplicate_pair_spec(set, i, j));
            assert(false);
        }
    };
}

// r[impl set-assoc.no-aliasing]
/// Different tags in the same set cannot share a hit.
pub open spec fn no_false_hit_spec(
    set: Seq<Option<u64>>,
    addr_hit: u64,
    addr_other: u64,
    offset_bits: u64,
    index_bits: u64,
) -> bool
    recommends
        offset_bits + index_bits <= 64,
        extract_index_spec(addr_hit, offset_bits, index_bits)
            == extract_index_spec(addr_other, offset_bits, index_bits),
        extract_tag_spec(addr_hit, offset_bits, index_bits)
            != extract_tag_spec(addr_other, offset_bits, index_bits),
{
    match lookup_set_spec(set, addr_hit, offset_bits, index_bits) {
        Some(idx) => {
            &&& set[idx] == Some(extract_tag_spec(addr_hit, offset_bits, index_bits))
            &&& set[idx] != Some(extract_tag_spec(addr_other, offset_bits, index_bits))
        }
        None => true,
    }
}

// r[impl set-assoc.no-aliasing]
pub open spec fn no_aliasing_spec(
    set: Seq<Option<u64>>,
    addr_a: u64,
    addr_b: u64,
    offset_bits: u64,
    index_bits: u64,
) -> bool
    recommends
        offset_bits + index_bits <= 64,
        extract_index_spec(addr_a, offset_bits, index_bits)
            == extract_index_spec(addr_b, offset_bits, index_bits),
        extract_tag_spec(addr_a, offset_bits, index_bits)
            != extract_tag_spec(addr_b, offset_bits, index_bits),
{
    no_false_hit_spec(set, addr_a, addr_b, offset_bits, index_bits)
        && no_false_hit_spec(set, addr_b, addr_a, offset_bits, index_bits)
}

// r[verify set-assoc.no-aliasing]
proof fn different_tags_same_index_no_false_hit(
    set: Seq<Option<u64>>,
    addr_a: u64,
    addr_b: u64,
    offset_bits: u64,
    index_bits: u64,
)
    requires
        offset_bits + index_bits <= 64,
        extract_index_spec(addr_a, offset_bits, index_bits)
            == extract_index_spec(addr_b, offset_bits, index_bits),
        extract_tag_spec(addr_a, offset_bits, index_bits)
            != extract_tag_spec(addr_b, offset_bits, index_bits),
    ensures no_aliasing_spec(set, addr_a, addr_b, offset_bits, index_bits)
{
    let tag_a = extract_tag_spec(addr_a, offset_bits, index_bits);
    let tag_b = extract_tag_spec(addr_b, offset_bits, index_bits);
    match lookup_set_spec(set, addr_a, offset_bits, index_bits) {
        Some(idx) => {
            hit_returns_matching_tag(set, tag_a, idx);
            assert(set[idx] != Some(tag_b));
        }
        None => {}
    }
    match lookup_set_spec(set, addr_b, offset_bits, index_bits) {
        Some(idx) => {
            hit_returns_matching_tag(set, tag_b, idx);
            assert(set[idx] != Some(tag_a));
        }
        None => {}
    }
}

// r[verify set-assoc.no-aliasing]
proof fn same_tag_same_lookup(
    set: Seq<Option<u64>>,
    addr_a: u64,
    addr_b: u64,
    offset_bits: u64,
    index_bits: u64,
)
    requires
        offset_bits + index_bits <= 64,
        extract_tag_spec(addr_a, offset_bits, index_bits)
            == extract_tag_spec(addr_b, offset_bits, index_bits),
    ensures lookup_set_spec(set, addr_a, offset_bits, index_bits)
        == lookup_set_spec(set, addr_b, offset_bits, index_bits)
{}

// r[impl set-assoc.set-isolation]
/// Selected set index for an address.
pub open spec fn selected_set_spec(addr: u64, offset_bits: u64, index_bits: u64) -> int
    recommends offset_bits + index_bits <= 64
{
    extract_index_spec(addr, offset_bits, index_bits) as int
}

// r[impl set-assoc.set-isolation]
/// Cache-of-sets lookup that only inspects the selected set.
pub open spec fn cache_lookup_spec(
    cache: Seq<Seq<Option<u64>>>,
    addr: u64,
    offset_bits: u64,
    index_bits: u64,
) -> Option<int>
    recommends offset_bits + index_bits <= 64
{
    let set_idx = selected_set_spec(addr, offset_bits, index_bits);
    if 0 <= set_idx < cache.len() {
        lookup_set_spec(cache[set_idx], addr, offset_bits, index_bits)
    } else {
        None
    }
}

// r[verify set-assoc.set-isolation]
proof fn different_indices_select_different_sets(
    addr_a: u64,
    addr_b: u64,
    offset_bits: u64,
    index_bits: u64,
)
    requires
        offset_bits + index_bits <= 64,
        extract_index_spec(addr_a, offset_bits, index_bits)
            != extract_index_spec(addr_b, offset_bits, index_bits),
    ensures selected_set_spec(addr_a, offset_bits, index_bits)
        != selected_set_spec(addr_b, offset_bits, index_bits)
{}

// r[verify set-assoc.set-isolation]
proof fn update_other_set_preserves_lookup(
    cache: Seq<Seq<Option<u64>>>,
    addr: u64,
    offset_bits: u64,
    index_bits: u64,
    other_idx: int,
    replacement: Seq<Option<u64>>,
)
    requires
        offset_bits + index_bits <= 64,
        0 <= other_idx < cache.len(),
        0 <= selected_set_spec(addr, offset_bits, index_bits) < cache.len(),
        other_idx != selected_set_spec(addr, offset_bits, index_bits),
    ensures cache_lookup_spec(cache.update(other_idx, replacement), addr, offset_bits, index_bits)
        == cache_lookup_spec(cache, addr, offset_bits, index_bits)
{
    let selected = selected_set_spec(addr, offset_bits, index_bits);
    assert(cache.update(other_idx, replacement)[selected] == cache[selected]);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn cache_lookup_runtime(cache: &[Vec<Option<u64>>], addr: u64, offset_bits: u64, index_bits: u64) -> Option<usize> {
        let set_idx = usize::try_from(extract_index(addr, offset_bits, index_bits)).ok()?;
        if set_idx >= cache.len() {
            None
        } else {
            lookup_set(&cache[set_idx], addr, offset_bits, index_bits)
        }
    }

    // r[verify set-assoc.decomposition]
    #[test]
    fn test_decompose_known_geometry() {
        let addr = 0x1234_5678_9ABC_DEF0u64;
        let (tag, index, offset) = decompose_address(addr, 6, 8);
        assert_eq!(offset, 0x30);
        assert_eq!(index, 0x7B);
        assert_eq!(tag, 0x48D1_59E2_6AF3);
        assert_eq!(reconstruct_address(tag, index, offset, 6, 8), addr);
    }

    // r[verify set-assoc.decomposition]
    #[test]
    fn test_decompose_full_width_edge_cases() {
        let addr = 0xDEAD_BEEF_CAFE_BABEu64;
        assert_eq!(decompose_address(addr, 64, 0), (0, 0, addr));
        assert_eq!(decompose_address(addr, 0, 64), (0, addr, 0));
    }

    // r[verify set-assoc.hit]
    #[test]
    fn test_lookup_hit() {
        let set = vec![None, Some(0x55), Some(0xAA), None];
        let addr = reconstruct_address(0xAA, 1, 0x1F, 6, 4);
        assert_eq!(lookup_set(&set, addr, 6, 4), Some(2));
    }

    // r[verify set-assoc.miss]
    #[test]
    fn test_lookup_miss() {
        let set = vec![None, Some(0x55), Some(0xAA), None];
        let addr = reconstruct_address(0x77, 1, 0x00, 6, 4);
        assert_eq!(lookup_set(&set, addr, 6, 4), None);
    }

    // r[verify set-assoc.miss]
    #[test]
    fn test_lookup_empty_set() {
        let set = vec![None, None, None, None];
        let addr = reconstruct_address(0x22, 0, 0, 6, 4);
        assert_eq!(lookup_set(&set, addr, 6, 4), None);
    }

    // r[verify set-assoc.no-duplicates]
    #[test]
    fn test_duplicate_checker() {
        assert!(check_no_duplicate_tags(&[None, Some(1), Some(2), None]));
        assert!(!check_no_duplicate_tags(&[Some(7), None, Some(7)]));
    }

    // r[verify set-assoc.set-isolation]
    #[test]
    fn test_cache_set_isolation() {
        let addr = reconstruct_address(0x33, 1, 0x12, 6, 2);
        let cache = vec![vec![Some(1), None], vec![None, Some(0x33)], vec![Some(9), None], vec![
            None, None,
        ]];
        let before = cache_lookup_runtime(&cache, addr, 6, 2);
        let mut changed = cache.clone();
        changed[2] = vec![Some(99), Some(100)];
        let after = cache_lookup_runtime(&changed, addr, 6, 2);
        assert_eq!(before, Some(1));
        assert_eq!(after, before);
    }

    // r[verify set-assoc.component-bounds]
    #[test]
    fn test_component_bounds_examples() {
        let addr = 0x1234_5678_9ABC_DEF0u64;
        let offset = extract_offset(addr, 6);
        let index = extract_index(addr, 6, 8);
        let tag = extract_tag(addr, 6, 8);
        assert!(fits_in_bits(offset, 6));
        assert!(fits_in_bits(index, 8));
        assert!(fits_in_bits(tag, 50));
    }

    proptest! {
        // r[verify set-assoc.decomposition]
        #[test]
        fn prop_decomposition_roundtrip(
            addr: u64,
            offset_bits in 0u32..=64,
            index_bits in 0u32..=64,
        ) {
            prop_assume!(u64::from(offset_bits) + u64::from(index_bits) <= 64);
            let offset_bits_u64 = u64::from(offset_bits);
            let index_bits_u64 = u64::from(index_bits);
            let (tag, index, offset) = decompose_address(addr, offset_bits_u64, index_bits_u64);
            let reconstructed = reconstruct_address(tag, index, offset, offset_bits_u64, index_bits_u64);
            prop_assert_eq!(reconstructed, addr);
        }

        // r[verify set-assoc.component-bounds]
        #[test]
        fn prop_component_bounds(
            addr: u64,
            offset_bits in 0u32..=64,
            index_bits in 0u32..=64,
        ) {
            prop_assume!(u64::from(offset_bits) + u64::from(index_bits) <= 64);
            let offset_bits_u64 = u64::from(offset_bits);
            let index_bits_u64 = u64::from(index_bits);
            let tag_bits = 64u64 - offset_bits_u64 - index_bits_u64;
            let offset = extract_offset(addr, offset_bits_u64);
            let index = extract_index(addr, offset_bits_u64, index_bits_u64);
            let tag = extract_tag(addr, offset_bits_u64, index_bits_u64);
            prop_assert!(fits_in_bits(offset, offset_bits_u64));
            prop_assert!(fits_in_bits(index, index_bits_u64));
            prop_assert!(fits_in_bits(tag, tag_bits));
        }

        // r[verify set-assoc.no-aliasing]
        #[test]
        fn prop_no_aliasing_same_index_different_tag(
            addr: u64,
            offset_bits in 0u32..=16,
            index_bits in 0u32..=16,
        ) {
            let sum = offset_bits + index_bits;
            prop_assume!(sum < 64);
            let offset_bits_u64 = u64::from(offset_bits);
            let index_bits_u64 = u64::from(index_bits);
            let addr_other = addr ^ (1u64 << sum);
            let tag_a = extract_tag(addr, offset_bits_u64, index_bits_u64);
            let tag_b = extract_tag(addr_other, offset_bits_u64, index_bits_u64);
            prop_assume!(extract_index(addr, offset_bits_u64, index_bits_u64)
                == extract_index(addr_other, offset_bits_u64, index_bits_u64));
            prop_assume!(tag_a != tag_b);
            let set = vec![Some(tag_a), None, None, None];
            prop_assert_eq!(lookup_set(&set, addr, offset_bits_u64, index_bits_u64), Some(0));
            prop_assert_ne!(lookup_set(&set, addr_other, offset_bits_u64, index_bits_u64), Some(0));
        }

        // r[verify set-assoc.miss]
        #[test]
        fn prop_miss_completeness(
            ways in proptest::collection::vec(proptest::option::of(any::<u64>()), 0..20),
            absent_tag: u64,
            offset_bits in 0u32..=12,
            index_bits in 0u32..=12,
        ) {
            prop_assume!(!ways.contains(&Some(absent_tag)));
            prop_assume!(u64::from(offset_bits) + u64::from(index_bits) <= 64);
            let addr = reconstruct_address(absent_tag, 0, 0, u64::from(offset_bits), u64::from(index_bits));
            prop_assert_eq!(find_way(&ways, absent_tag), None);
            prop_assert_eq!(lookup_set(&ways, addr, u64::from(offset_bits), u64::from(index_bits)), None);
        }

        // r[verify set-assoc.no-duplicates]
        #[test]
        fn prop_duplicate_checker_matches_model(
            ways in proptest::collection::vec(proptest::option::of(any::<u64>()), 0..20)
        ) {
            let model = {
                let mut ok = true;
                for i in 0..ways.len() {
                    if let Some(tag) = ways[i] {
                        for entry in ways.iter().skip(i + 1) {
                            if *entry == Some(tag) {
                                ok = false;
                            }
                        }
                    }
                }
                ok
            };
            prop_assert_eq!(check_no_duplicate_tags(&ways), model);
        }
    }
}
