//! Verified guest-physical address translation.
//!
//! Translates a GPA through a validated memslot table, preserving byte
//! offsets.  Access classification distinguishes `Translated`, `Unmapped`,
//! and `ReadonlyWrite` outcomes.

#![allow(clippy::needless_pass_by_value)]

use vstd::prelude::*;

use crate::hypervisor_memslot::*;

verus! {

// ────────────────────────────────────────────────────────────────────
//  AccessMode
// ────────────────────────────────────────────────────────────────────

/// Read or Write access for GPA translation.
pub enum AccessMode {
    Read,
    Write,
}

/// Spec: is the access mode a write?
pub open spec fn is_write_spec(mode: AccessMode) -> bool {
    mode == AccessMode::Write
}

// r[impl hypervisor-gpa.access]
/// Returns true for write access.
pub fn is_write(mode: &AccessMode) -> (result: bool)
    ensures result == is_write_spec(*mode)
{
    match mode {
        AccessMode::Write => true,
        AccessMode::Read => false,
    }
}

// ────────────────────────────────────────────────────────────────────
//  GpaResult
// ────────────────────────────────────────────────────────────────────

/// Result of a GPA access attempt.
pub enum GpaResult {
    /// Successfully translated; carries the host address.
    Translated { host_addr: u64 },
    /// No memslot covers this GPA.
    Unmapped,
    /// Write into a readonly memslot.
    ReadonlyWrite,
}

// ────────────────────────────────────────────────────────────────────
//  translate_gpa — spec
// ────────────────────────────────────────────────────────────────────

/// Spec: GPA to host address via a specific memslot.
/// `gpa_base = slot.base_gfn * page_size`, `host_base = slot.host_base`.
/// Result: `host_base + (gpa - gpa_base)`.
///
/// For simplicity we model GFN-granularity: `gpa` is a guest frame number
/// and `host_base` is a host frame address. The offset is `gfn - base_gfn`.

// r[depends hypervisor-gpa.translate]
/// Spec: translated host address.
pub open spec fn translate_gpa_spec(slot: Memslot, gfn: u64) -> int {
    slot.host_base as int + (gfn as int - slot.base_gfn as int)
}

// r[verify hypervisor-gpa.translate]
/// Offset is preserved: translate(gfn) - host_base == gfn - base_gfn.
proof fn translate_preserves_offset(slot: Memslot, gfn: u64)
    requires slot_contains_gfn_spec(slot, gfn)
    ensures
        translate_gpa_spec(slot, gfn) - slot.host_base as int
            == gfn as int - slot.base_gfn as int
{}

// ────────────────────────────────────────────────────────────────────
//  access_gpa — spec
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-gpa.access]
/// Spec: classify a GPA access.
pub open spec fn access_gpa_spec(
    slots: Seq<Memslot>,
    gfn: u64,
    mode: AccessMode,
) -> GpaResult {
    match lookup_memslot_spec(slots, gfn) {
        None => GpaResult::Unmapped,
        Some(idx) => {
            let slot = slots[idx as int];
            if slot.readonly && mode == AccessMode::Write {
                GpaResult::ReadonlyWrite
            } else {
                GpaResult::Translated {
                    host_addr: translate_gpa_spec(slot, gfn) as u64,
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────
//  access_gpa — exec
// ────────────────────────────────────────────────────────────────────

// r[impl hypervisor-gpa.translate]
// r[impl hypervisor-gpa.access]
/// Translates `gfn` through `slots`, returning the access outcome.
/// Scans the memslot table for `gfn`, checks readonly/write,
/// and computes the host address.
pub fn access_gpa(
    slots: &Vec<Memslot>,
    gfn: u64,
    mode: AccessMode,
) -> (result: GpaResult)
    requires
        table_valid_spec(slots@),
        forall|i: int| 0 <= i < slots@.len() as int ==>
            slots@[i].host_base as int + slots@[i].npages as int <= u64::MAX as int,
    ensures
        result == access_gpa_spec(slots@, gfn, mode),
{
    // Linear scan for gfn, reusing the containment check inline
    // so Verus has the arithmetic bounds directly.
    let len = slots.len();
    let mut i: usize = 0;
    while i < len
        invariant
            0 <= i <= len,
            len == slots@.len(),
            table_valid_spec(slots@),
            forall|j: int| 0 <= j < slots@.len() as int ==>
                slots@[j].host_base as int + slots@[j].npages as int <= u64::MAX as int,
            forall|k: int| 0 <= k < i as int ==>
                !slot_contains_gfn_spec(slots@[k], gfn),
        decreases len - i,
    {
        if slot_contains_gfn(&slots[i], gfn) {
            // Found the unique slot.
            proof {
                lookup_from_skips_to(slots@, gfn, 0, i as int);
            }
            if slots[i].readonly && is_write(&mode) {
                return GpaResult::ReadonlyWrite;
            }
            let offset = gfn - slots[i].base_gfn;
            let host_addr = slots[i].host_base + offset;
            return GpaResult::Translated { host_addr };
        }
        // Early exit: sorted table, all remaining slots start after gfn.
        if slots[i].base_gfn > gfn {
            proof {
                assert forall|k: int| i as int <= k < slots@.len() as int implies
                    !slot_contains_gfn_spec(slots@[k], gfn)
                by {
                    if k == i as int {
                    } else {
                        sorted_end_le_base(slots@, i as int, k);
                    }
                }
                lookup_miss_when_no_slot(slots@, gfn, 0);
            }
            return GpaResult::Unmapped;
        }
        i += 1;
    }
    proof {
        lookup_miss_when_no_slot(slots@, gfn, 0);
    }
    GpaResult::Unmapped
}

// ────────────────────────────────────────────────────────────────────
//  Cross-module: translated address stays inside host region
// ────────────────────────────────────────────────────────────────────

// r[verify hypervisor-gpa.translate]
/// The translated host address is within the slot's host-backed region:
/// `host_base <= translated < host_base + npages`.
proof fn translate_in_host_region(slot: Memslot, gfn: u64)
    requires
        slot_contains_gfn_spec(slot, gfn),
        slot.host_base as int + slot.npages as int <= u64::MAX as int,
    ensures
        slot.host_base as int <= translate_gpa_spec(slot, gfn),
        translate_gpa_spec(slot, gfn) < slot.host_base as int + slot.npages as int,
{
    // From slot_contains_gfn_spec:
    //   base_gfn <= gfn < base_gfn + npages
    // So 0 <= gfn - base_gfn < npages
    // translate = host_base + (gfn - base_gfn)
    // host_base <= translate < host_base + npages
}

// r[verify hypervisor-gpa.access]
/// Unmapped GPA: no slot contains it.
proof fn unmapped_means_no_slot(slots: Seq<Memslot>, gfn: u64)
    requires
        access_gpa_spec(slots, gfn, AccessMode::Read) == GpaResult::Unmapped,
    ensures
        lookup_memslot_spec(slots, gfn) is None,
{}

// r[verify hypervisor-gpa.access]
/// ReadonlyWrite: slot exists but is readonly.
proof fn readonly_write_means_readonly_slot(
    slots: Seq<Memslot>,
    gfn: u64,
)
    requires
        access_gpa_spec(slots, gfn, AccessMode::Write) == GpaResult::ReadonlyWrite,
    ensures ({
        let idx = lookup_memslot_spec(slots, gfn).unwrap() as int;
        slots[idx].readonly
    })
{}

} // verus!

// ────────────────────────────────────────────────────────────────────
//  Tests
// ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(base_gfn: u64, npages: u64, host_base: u64, readonly: bool) -> Memslot {
        Memslot {
            base_gfn,
            npages,
            host_base,
            readonly,
        }
    }

    fn slots() -> Vec<Memslot> {
        vec![mk(0, 16, 0x8000, false), mk(16, 16, 0xA000, true)]
    }

    // r[test hypervisor-gpa.translate]
    #[test]
    fn test_translate_first_byte() {
        let s = slots();
        assert!(matches!(access_gpa(&s, 0, AccessMode::Read), GpaResult::Translated { host_addr: 0x8000 }));
    }

    // r[test hypervisor-gpa.translate]
    #[test]
    fn test_translate_offset_preserved() {
        let s = slots();
        assert!(matches!(access_gpa(&s, 5, AccessMode::Read), GpaResult::Translated { host_addr: 0x8005 }));
    }

    // r[test hypervisor-gpa.access]
    #[test]
    fn test_unmapped() {
        let s = slots();
        assert!(matches!(access_gpa(&s, 100, AccessMode::Read), GpaResult::Unmapped));
    }

    // r[test hypervisor-gpa.access]
    #[test]
    fn test_readonly_write() {
        let s = slots();
        assert!(matches!(access_gpa(&s, 20, AccessMode::Write), GpaResult::ReadonlyWrite));
    }

    // r[test hypervisor-gpa.access]
    #[test]
    fn test_readonly_read_ok() {
        let s = slots();
        assert!(matches!(access_gpa(&s, 20, AccessMode::Read), GpaResult::Translated { host_addr: 0xA004 }));
    }
}
