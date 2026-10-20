//! Verified virtual address decomposition.
//!
//! Page-level address arithmetic: split a virtual address into frame
//! number and offset, check alignment, compute page start addresses,
//! and calculate how many pages a region needs. The core invariant is
//! the decomposition roundtrip: `frame_number * page_size + offset == addr`.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
use vstd::prelude::*;

verus! {

// --- frame_number ---

// r[depends page.frame-number]
/// Spec: page frame number for the given address.
pub open spec fn frame_number_spec(addr: u64, page_size: u64) -> u64
    recommends page_size > 0
{
    (addr as int / page_size as int) as u64
}

// r[impl page.frame-number]
/// Returns `addr / page_size`.
pub fn frame_number(addr: u64, page_size: u64) -> (result: u64)
    requires page_size > 0
    ensures result == frame_number_spec(addr, page_size)
{
    addr / page_size
}

// r[verify page.frame-number]
/// Frame number is bounded: frame * page_size <= addr.
proof fn frame_number_bounded(addr: u64, page_size: u64)
    requires page_size > 0
    ensures frame_number_spec(addr, page_size) as int * page_size as int <= addr as int
{
    lemma_fundamental_div_mod(addr as int, page_size as int);
    // fundamental_div_mod: addr == page_size * (addr / page_size) + addr % page_size
    // We need: (addr / page_size) * page_size <= addr
    // Since addr % page_size >= 0, we have page_size * (addr / page_size) <= addr
    // Commutativity: page_size * q == q * page_size
    let q = addr as int / page_size as int;
    let ps = page_size as int;
    assert(q * ps <= addr as int) by(nonlinear_arith)
        requires ps > 0, addr as int == ps * q + addr as int % ps,
            0 <= addr as int % ps;
}

// r[verify page.frame-number]
/// Frame number of zero is always zero.
proof fn frame_number_of_zero(page_size: u64)
    requires page_size > 0
    ensures frame_number_spec(0, page_size) == 0
{
    lemma_fundamental_div_mod(0int, page_size as int);
}

// --- page_offset ---

// r[depends page.offset]
/// Spec: byte offset within the page.
pub open spec fn page_offset_spec(addr: u64, page_size: u64) -> u64
    recommends page_size > 0
{
    (addr as int % page_size as int) as u64
}

// r[impl page.offset]
/// Returns `addr % page_size`.
pub fn page_offset(addr: u64, page_size: u64) -> (result: u64)
    requires page_size > 0
    ensures result == page_offset_spec(addr, page_size)
{
    addr % page_size
}

// r[verify page.offset]
/// Offset is strictly less than page_size.
proof fn page_offset_bounded(addr: u64, page_size: u64)
    requires page_size > 0
    ensures page_offset_spec(addr, page_size) < page_size
{
    lemma_fundamental_div_mod(addr as int, page_size as int);
}

// r[verify page.offset]
/// Offset of zero is always zero.
proof fn page_offset_of_zero(page_size: u64)
    requires page_size > 0
    ensures page_offset_spec(0, page_size) == 0
{
    lemma_fundamental_div_mod(0int, page_size as int);
}

// --- is_page_aligned ---

// r[depends page.is-aligned]
/// Spec: address is a multiple of page_size.
pub open spec fn is_page_aligned_spec(addr: u64, page_size: u64) -> bool
    recommends page_size > 0
{
    addr as int % page_size as int == 0
}

// r[impl page.is-aligned]
/// Returns true if addr is page-aligned.
pub fn is_page_aligned(addr: u64, page_size: u64) -> (result: bool)
    requires page_size > 0
    ensures result == is_page_aligned_spec(addr, page_size)
{
    addr % page_size == 0
}

// r[verify page.is-aligned]
/// Zero is always page-aligned.
proof fn zero_is_aligned(page_size: u64)
    requires page_size > 0
    ensures is_page_aligned_spec(0, page_size)
{
    lemma_fundamental_div_mod(0int, page_size as int);
}

// r[verify page.is-aligned]
/// Aligned addresses have zero offset.
proof fn aligned_iff_zero_offset(addr: u64, page_size: u64)
    requires page_size > 0
    ensures is_page_aligned_spec(addr, page_size)
        == (page_offset_spec(addr, page_size) == 0)
{}

// --- page_start ---

// r[depends page.start-address]
/// Spec: start address of the page containing frame.
pub open spec fn page_start_spec(frame: u64, page_size: u64) -> u64
    recommends page_size > 0, frame as int * page_size as int <= u64::MAX as int
{
    (frame as int * page_size as int) as u64
}

// r[impl page.start-address]
/// Returns `frame * page_size`.
pub fn page_start(frame: u64, page_size: u64) -> (result: u64)
    requires
        page_size > 0,
        frame as int * page_size as int <= u64::MAX as int,
    ensures result == page_start_spec(frame, page_size)
{
    frame * page_size
}

// r[verify page.start-address]
/// Page start is always page-aligned.
proof fn page_start_aligned(frame: u64, page_size: u64)
    requires page_size > 0, frame as int * page_size as int <= u64::MAX as int
    ensures is_page_aligned_spec(page_start_spec(frame, page_size), page_size)
{
    let start = frame as int * page_size as int;
    assert(start % page_size as int == 0) by(nonlinear_arith)
        requires page_size as int > 0, start == frame as int * page_size as int;
}

// r[verify page.start-address]
/// Frame 0 starts at address 0.
proof fn frame_zero_starts_at_zero(page_size: u64)
    requires page_size > 0
    ensures page_start_spec(0, page_size) == 0
{}

// --- pages_needed ---

// r[depends page.pages-needed]
/// Spec: minimum number of pages to cover size bytes.
pub open spec fn pages_needed_spec(size: u64, page_size: u64) -> u64
    recommends page_size > 0
{
    if size == 0 { 0 } else { ((size as int - 1) / page_size as int + 1) as u64 }
}

// r[impl page.pages-needed]
/// Returns the minimum number of pages to cover size bytes.
pub fn pages_needed(size: u64, page_size: u64) -> (result: u64)
    requires page_size > 0
    ensures result == pages_needed_spec(size, page_size)
{
    if size == 0 {
        0
    } else {
        (size - 1) / page_size + 1
    }
}

// r[verify page.pages-needed]
/// Zero bytes needs zero pages.
proof fn pages_needed_zero(page_size: u64)
    requires page_size > 0
    ensures pages_needed_spec(0, page_size) == 0
{}

// r[verify page.pages-needed]
/// Result * page_size >= size (pages cover the full region).
proof fn pages_needed_covers(size: u64, page_size: u64)
    requires page_size > 0, size > 0
    ensures pages_needed_spec(size, page_size) as int * page_size as int >= size as int
{
    let ps = page_size as int;
    let s = size as int;
    let n = (s - 1) / ps + 1;
    lemma_fundamental_div_mod(s - 1, ps);
    assert(n * ps >= s) by(nonlinear_arith)
        requires ps > 0, s > 0, n == (s - 1) / ps + 1,
            s - 1 == ps * ((s - 1) / ps) + (s - 1) % ps,
            0 <= (s - 1) % ps < ps;
}

// r[verify page.pages-needed]
/// One byte needs exactly one page.
proof fn one_byte_one_page(page_size: u64)
    requires page_size > 0
    ensures pages_needed_spec(1, page_size) == 1
{
    lemma_fundamental_div_mod(0int, page_size as int);
}

// r[verify page.pages-needed]
/// Exact page_size bytes needs one page.
proof fn exact_page_one_page(page_size: u64)
    requires page_size > 0
    ensures pages_needed_spec(page_size, page_size) == 1
{
    let ps = page_size as int;
    lemma_fundamental_div_mod(ps - 1, ps);
    // (page_size - 1) / page_size == 0 because 0 <= page_size - 1 < page_size
    assert((ps - 1) / ps == 0) by(nonlinear_arith)
        requires ps > 0, ps - 1 == ps * ((ps - 1) / ps) + (ps - 1) % ps,
            0 <= (ps - 1) % ps < ps;
}

// --- decomposition roundtrip ---

// r[verify page.frame-number]
// r[verify page.offset]
/// Fundamental decomposition: frame_number * page_size + offset == addr.
proof fn decomposition_roundtrip(addr: u64, page_size: u64)
    requires page_size > 0
    ensures
        frame_number_spec(addr, page_size) as int * page_size as int
            + page_offset_spec(addr, page_size) as int
            == addr as int
{
    lemma_fundamental_div_mod(addr as int, page_size as int);
    assert(
        (addr as int / page_size as int) * page_size as int
        + addr as int % page_size as int == addr as int
    ) by(nonlinear_arith)
        requires
            addr as int == page_size as int * (addr as int / page_size as int)
                + addr as int % page_size as int,
            page_size as int > 0;
}

// r[verify page.frame-number]
// r[verify page.start-address]
// r[verify page.is-aligned]
/// Page start of frame_number(addr) is page-aligned and <= addr.
proof fn page_start_of_frame(addr: u64, page_size: u64)
    requires page_size > 0
    ensures
        frame_number_spec(addr, page_size) as int * page_size as int <= addr as int,
        is_page_aligned_spec(
            (frame_number_spec(addr, page_size) as int * page_size as int) as u64,
            page_size,
        ),
{
    lemma_fundamental_div_mod(addr as int, page_size as int);
    let frame = addr as int / page_size as int;
    let start = frame * page_size as int;
    assert(start <= addr as int) by(nonlinear_arith)
        requires
            page_size as int > 0,
            addr as int == page_size as int * frame + addr as int % page_size as int,
            0 <= addr as int % page_size as int,
            start == frame * page_size as int;
    assert(start % page_size as int == 0) by(nonlinear_arith)
        requires page_size as int > 0, start == frame * page_size as int;
}

// --- radix_index ---

// r[depends page.radix-index]
/// Spec: extract the level-th group of bits_per_level bits from value,
/// starting at base_offset. For Sv39 VPN extraction: radix_index(va, i, 9, 12).
pub open spec fn radix_index_spec(
    value: u64, level: u64, bits_per_level: u64, base_offset: u64,
) -> u64
    recommends
        bits_per_level > 0,
        base_offset + (level + 1) * bits_per_level <= 64,
{
    // Inline extract_field_spec to avoid cross-module ghost import issues
    let offset = base_offset + level * bits_per_level;
    if bits_per_level == 0 {
        0u64
    } else {
        (value >> offset) & sub(1u64 << bits_per_level, 1u64)
    }
}

// r[impl page.radix-index]
/// Returns the level-th group of bits_per_level bits from value.
pub fn radix_index(
    value: u64, level: u64, bits_per_level: u64, base_offset: u64,
) -> (result: u64)
    requires
        bits_per_level > 0,
        bits_per_level <= 63,
        base_offset + (level + 1) * bits_per_level <= 64,
    ensures result == radix_index_spec(value, level, bits_per_level, base_offset)
{
    proof {
        assert((level + 1) * bits_per_level <= 64u64) by(nonlinear_arith)
            requires
                bits_per_level > 0,
                base_offset + (level + 1) * bits_per_level <= 64u64,
                base_offset >= 0u64;
        assert(level * bits_per_level <= 64u64) by(nonlinear_arith)
            requires
                bits_per_level > 0,
                (level + 1) * bits_per_level <= 64u64;
        assert(base_offset + level * bits_per_level < 64u64) by(nonlinear_arith)
            requires
                base_offset + (level + 1) * bits_per_level <= 64u64,
                bits_per_level > 0;
        assert(bits_per_level <= 63u64 ==> (1u64 << bits_per_level) >= 1u64) by(bit_vector);
    }
    let offset = base_offset + level * bits_per_level;
    let mask = (1u64 << bits_per_level) - 1u64;
    (value >> offset) & mask
}

proof fn and_mask_lt_shift(value: u64, offset: u64, width: u64)
    requires
        0 < width <= 63,
    ensures
        ((value >> offset) & sub(1u64 << width, 1u64)) < (1u64 << width)
{
    assert(((value >> offset) & sub(1u64 << width, 1u64)) < (1u64 << width)) by(bit_vector)
        requires 0u64 < width <= 63u64;
}

// r[verify page.radix-index]
/// Radix index is bounded: result < 2^bits_per_level.
proof fn radix_index_bounded(
    value: u64, level: u64, bits_per_level: u64, base_offset: u64,
)
    requires
        bits_per_level > 0,
        bits_per_level <= 63,
        base_offset + (level + 1) * bits_per_level <= 64,
    ensures radix_index_spec(value, level, bits_per_level, base_offset)
        < (1u64 << bits_per_level)
{
    assert(base_offset + level * bits_per_level < 64) by(nonlinear_arith)
        requires
            bits_per_level > 0,
            base_offset + (level + 1) * bits_per_level <= 64;
    let offset = (base_offset + level * bits_per_level) as u64;
    assert(bits_per_level != 0u64);
    and_mask_lt_shift(value, offset, bits_per_level);
}

// r[verify page.radix-index]
/// For 9-bit levels, result < 512.
proof fn radix_index_bounded_9bit(
    value: u64, level: u64, base_offset: u64,
)
    requires base_offset + (level + 1) * 9 <= 64
    ensures radix_index_spec(value, level, 9, base_offset) < 512
{
    radix_index_bounded(value, level, 9, base_offset);
    assert(1u64 << 9u64 == 512u64) by(bit_vector);
}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify page.frame-number]
    #[test]
    fn test_frame_number() {
        assert_eq!(frame_number(0, 4096), 0);
        assert_eq!(frame_number(4095, 4096), 0);
        assert_eq!(frame_number(4096, 4096), 1);
        assert_eq!(frame_number(8191, 4096), 1);
        assert_eq!(frame_number(8192, 4096), 2);
    }

    // r[verify page.offset]
    #[test]
    fn test_page_offset() {
        assert_eq!(page_offset(0, 4096), 0);
        assert_eq!(page_offset(100, 4096), 100);
        assert_eq!(page_offset(4095, 4096), 4095);
        assert_eq!(page_offset(4096, 4096), 0);
        assert_eq!(page_offset(4097, 4096), 1);
    }

    // r[verify page.is-aligned]
    #[test]
    fn test_is_page_aligned() {
        assert!(is_page_aligned(0, 4096));
        assert!(!is_page_aligned(1, 4096));
        assert!(!is_page_aligned(4095, 4096));
        assert!(is_page_aligned(4096, 4096));
        assert!(is_page_aligned(8192, 4096));
    }

    // r[verify page.start-address]
    #[test]
    fn test_page_start() {
        assert_eq!(page_start(0, 4096), 0);
        assert_eq!(page_start(1, 4096), 4096);
        assert_eq!(page_start(2, 4096), 8192);
        assert_eq!(page_start(0, 1), 0);
        assert_eq!(page_start(100, 1), 100);
    }

    // r[verify page.pages-needed]
    #[test]
    fn test_pages_needed() {
        assert_eq!(pages_needed(0, 4096), 0);
        assert_eq!(pages_needed(1, 4096), 1);
        assert_eq!(pages_needed(4095, 4096), 1);
        assert_eq!(pages_needed(4096, 4096), 1);
        assert_eq!(pages_needed(4097, 4096), 2);
        assert_eq!(pages_needed(8192, 4096), 2);
        assert_eq!(pages_needed(8193, 4096), 3);
    }

    // r[verify page.radix-index]
    #[test]
    fn test_radix_index() {
        // Sv39: 3 levels of 9-bit VPN at base_offset 12
        // Address 0: all VPN fields are 0
        assert_eq!(radix_index(0, 0, 9, 12), 0);
        assert_eq!(radix_index(0, 1, 9, 12), 0);
        assert_eq!(radix_index(0, 2, 9, 12), 0);

        // Address 0x80400000:
        // bits 38-30 (VPN[2]) = 0b000000010 = 2
        // bits 29-21 (VPN[1]) = 0b000000010 = 2
        // bits 20-12 (VPN[0]) = 0b000000000 = 0
        assert_eq!(radix_index(0x80400000, 0, 9, 12), 0);
        assert_eq!(radix_index(0x80400000, 1, 9, 12), 2);
        assert_eq!(radix_index(0x80400000, 2, 9, 12), 2);

        // VPN[0] at max (511): bits 20-12 all set
        let va = 0x1FF000u64; // bits 20-12 = 0x1FF = 511
        assert_eq!(radix_index(va, 0, 9, 12), 511);
    }

    proptest! {
        // r[verify page.frame-number]
        // r[verify page.offset]
        #[test]
        fn prop_decomposition_roundtrip(addr: u64, page_size in 1u64..=65536) {
            let frame = frame_number(addr, page_size);
            let offset = page_offset(addr, page_size);
            // frame * page_size might overflow u64 in the test,
            // but the mathematical identity must hold
            let reconstructed = u128::from(frame) * u128::from(page_size) + u128::from(offset);
            prop_assert_eq!(reconstructed, u128::from(addr));
        }

        // r[verify page.offset]
        #[test]
        fn prop_offset_bounded(addr: u64, page_size in 1u64..=65536) {
            prop_assert!(page_offset(addr, page_size) < page_size);
        }

        // r[verify page.is-aligned]
        #[test]
        fn prop_aligned_iff_zero_offset(addr: u64, page_size in 1u64..=65536) {
            prop_assert_eq!(
                is_page_aligned(addr, page_size),
                page_offset(addr, page_size) == 0
            );
        }

        // r[verify page.is-aligned]
        #[test]
        fn prop_zero_is_aligned(page_size in 1u64..=65536) {
            prop_assert!(is_page_aligned(0, page_size));
        }

        // r[verify page.start-address]
        #[test]
        fn prop_page_start_aligned(frame in 0u64..=10000, page_size in 1u64..=65536) {
            if u128::from(frame) * u128::from(page_size) <= u128::from(u64::MAX) {
                let start = page_start(frame, page_size);
                prop_assert!(is_page_aligned(start, page_size));
            }
        }

        // r[verify page.pages-needed]
        #[test]
        fn prop_pages_needed_covers(size in 1u64..=1_000_000, page_size in 1u64..=65536) {
            let n = pages_needed(size, page_size);
            let covered = u128::from(n) * u128::from(page_size);
            prop_assert!(covered >= u128::from(size));
        }

        // r[verify page.pages-needed]
        #[test]
        fn prop_pages_needed_minimal(size in 1u64..=1_000_000, page_size in 1u64..=65536) {
            let n = pages_needed(size, page_size);
            // n-1 pages would not be enough
            if n > 0 {
                let insufficient = u128::from(n - 1) * u128::from(page_size);
                prop_assert!(insufficient < u128::from(size));
            }
        }

        // r[verify page.pages-needed]
        #[test]
        fn prop_pages_needed_zero(page_size in 1u64..=65536) {
            prop_assert_eq!(pages_needed(0, page_size), 0);
        }

        // r[verify page.frame-number]
        // r[verify page.offset]
        // r[verify page.is-aligned]
        // r[verify page.start-address]
        // r[verify page.pages-needed]
        // r[verify page.radix-index]
        #[test]
        fn prop_radix_index_bounded(value: u64, level in 0u64..4, bpl in 1u64..=16) {
            let base = 0u64;
            if base + (level + 1) * bpl <= 64 && bpl <= 63 {
                let idx = radix_index(value, level, bpl, base);
                prop_assert!(idx < (1u64 << bpl));
            }
        }

        // r[verify page.radix-index]
        #[test]
        fn prop_radix_index_sv39_bounded(va: u64, level in 0u64..3) {
            let idx = radix_index(va, level, 9, 12);
            prop_assert!(idx < 512);
        }

        // r[verify page.radix-index]
        #[test]
        fn prop_radix_index_roundtrip_sv39(va: u64) {
            // Extract 3 VPN levels (9 bits each at offset 12) + 12-bit page offset
            let vpn0 = radix_index(va, 0, 9, 12);
            let vpn1 = radix_index(va, 1, 9, 12);
            let vpn2 = radix_index(va, 2, 9, 12);
            let offset = va & 0xFFF; // bits [11:0]
            // Recombine the 39-bit value
            let recombined = (vpn2 << 30) | (vpn1 << 21) | (vpn0 << 12) | offset;
            let mask_39 = (1u64 << 39) - 1;
            prop_assert_eq!(recombined, va & mask_39);
        }

        /// Model oracle: verify page arithmetic against u128 multiplication.
        #[test]
        fn prop_model_oracle(addr: u64, page_size in 1u64..=4096) {
            let frame = frame_number(addr, page_size);
            let offset = page_offset(addr, page_size);

            // Decomposition
            let wide_start = u128::from(frame) * u128::from(page_size);
            prop_assert_eq!(wide_start + u128::from(offset), u128::from(addr));

            // Offset < page_size
            prop_assert!(offset < page_size);

            // Alignment
            prop_assert_eq!(is_page_aligned(addr, page_size), offset == 0);

            // Pages needed
            let n = pages_needed(addr, page_size);
            if addr == 0 {
                prop_assert_eq!(n, 0);
            } else {
                let covered = u128::from(n) * u128::from(page_size);
                prop_assert!(covered >= u128::from(addr));
            }
        }
    }
}
