//! Verified VM-exit classification and disposition.
//!
//! Classifies raw exit reasons into stable semantic classes, then maps
//! each class to a disposition: `ResumeGuest`, `UserspaceHandle`,
//! `Block`, or `Terminate`.

use vstd::prelude::*;

verus! {

// ────────────────────────────────────────────────────────────────────
//  Exit class encoding
// ────────────────────────────────────────────────────────────────────

// Semantic exit classes:
pub const EXIT_IO:               u64 = 0;
pub const EXIT_MMIO:             u64 = 1;
pub const EXIT_HYPERCALL:        u64 = 2;
pub const EXIT_HALT:             u64 = 3;
pub const EXIT_INTERRUPT_WINDOW: u64 = 4;
pub const EXIT_SHUTDOWN:         u64 = 5;
pub const EXIT_INTERNAL_ERROR:   u64 = 6;

// r[depends hypervisor-exit.classify]
pub open spec fn is_valid_exit_class_spec(class: u64) -> bool {
    class <= 6
}

// ────────────────────────────────────────────────────────────────────
//  classify_exit — raw reason → semantic class
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-exit.classify]
/// Spec: map a raw exit reason to a semantic class.
/// We use a simple encoding: raw reason codes 0-6 map to the
/// corresponding semantic classes directly.
pub open spec fn classify_exit_spec(raw_reason: u64) -> u64 {
    if raw_reason <= 6 { raw_reason } else { EXIT_INTERNAL_ERROR }
}

// r[impl hypervisor-exit.classify]
/// Deterministic classification of raw exit reasons.
pub fn classify_exit(raw_reason: u64) -> (result: u64)
    ensures
        result == classify_exit_spec(raw_reason),
        is_valid_exit_class_spec(result),
{
    if raw_reason <= 6 { raw_reason } else { EXIT_INTERNAL_ERROR }
}

// r[verify hypervisor-exit.classify]
/// Classification is deterministic.
proof fn classify_deterministic(r: u64)
    ensures classify_exit_spec(r) == classify_exit_spec(r)
{}

// r[verify hypervisor-exit.classify]
/// Known reasons classify correctly.
proof fn classify_known()
    ensures
        classify_exit_spec(EXIT_IO) == EXIT_IO,
        classify_exit_spec(EXIT_MMIO) == EXIT_MMIO,
        classify_exit_spec(EXIT_SHUTDOWN) == EXIT_SHUTDOWN,
        classify_exit_spec(EXIT_INTERNAL_ERROR) == EXIT_INTERNAL_ERROR,
{}

// r[verify hypervisor-exit.classify]
/// Unknown reasons classify as internal error.
proof fn classify_unknown(r: u64)
    requires r > 6
    ensures classify_exit_spec(r) == EXIT_INTERNAL_ERROR
{}

// ────────────────────────────────────────────────────────────────────
//  Disposition encoding
// ────────────────────────────────────────────────────────────────────

pub const DISP_RESUME_GUEST:    u64 = 0;
pub const DISP_USERSPACE_HANDLE: u64 = 1;
pub const DISP_BLOCK:           u64 = 2;
pub const DISP_TERMINATE:       u64 = 3;

// r[depends hypervisor-exit.disposition]
pub open spec fn is_valid_disposition_spec(d: u64) -> bool {
    d <= 3
}

// ────────────────────────────────────────────────────────────────────
//  exit_disposition — class → action
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-exit.disposition]
/// Spec: map exit class to disposition.
///
///   IO, MMIO, Hypercall → UserspaceHandle
///   Halt → Block
///   InterruptWindow → ResumeGuest
///   Shutdown, InternalError → Terminate
pub open spec fn exit_disposition_spec(class: u64, has_wake_event: bool) -> u64 {
    if class == EXIT_IO || class == EXIT_MMIO || class == EXIT_HYPERCALL {
        DISP_USERSPACE_HANDLE
    } else if class == EXIT_HALT {
        if has_wake_event { DISP_RESUME_GUEST } else { DISP_BLOCK }
    } else if class == EXIT_INTERRUPT_WINDOW {
        DISP_RESUME_GUEST
    } else {
        // Shutdown, InternalError, or unknown
        DISP_TERMINATE
    }
}

// r[impl hypervisor-exit.disposition]
/// Computes the disposition for a given exit class.
pub fn exit_disposition(class: u64, has_wake_event: bool) -> (result: u64)
    ensures result == exit_disposition_spec(class, has_wake_event)
{
    if class == EXIT_IO || class == EXIT_MMIO || class == EXIT_HYPERCALL {
        DISP_USERSPACE_HANDLE
    } else if class == EXIT_HALT {
        if has_wake_event { DISP_RESUME_GUEST } else { DISP_BLOCK }
    } else if class == EXIT_INTERRUPT_WINDOW {
        DISP_RESUME_GUEST
    } else {
        DISP_TERMINATE
    }
}

// r[verify hypervisor-exit.disposition]
/// IO exits go to userspace.
proof fn io_to_userspace()
    ensures exit_disposition_spec(EXIT_IO, false) == DISP_USERSPACE_HANDLE
{}

// r[verify hypervisor-exit.disposition]
/// MMIO exits go to userspace.
proof fn mmio_to_userspace()
    ensures exit_disposition_spec(EXIT_MMIO, false) == DISP_USERSPACE_HANDLE
{}

// r[verify hypervisor-exit.disposition]
/// Hypercall exits go to userspace.
proof fn hypercall_to_userspace()
    ensures exit_disposition_spec(EXIT_HYPERCALL, false) == DISP_USERSPACE_HANDLE
{}

// r[verify hypervisor-exit.disposition]
/// Shutdown terminates.
proof fn shutdown_terminates()
    ensures exit_disposition_spec(EXIT_SHUTDOWN, false) == DISP_TERMINATE
{}

// r[verify hypervisor-exit.disposition]
/// Internal error terminates.
proof fn internal_error_terminates()
    ensures exit_disposition_spec(EXIT_INTERNAL_ERROR, false) == DISP_TERMINATE
{}

// r[verify hypervisor-exit.disposition]
/// Halt without wake event blocks.
proof fn halt_blocks()
    ensures exit_disposition_spec(EXIT_HALT, false) == DISP_BLOCK
{}

// r[verify hypervisor-exit.disposition]
/// Halt with wake event resumes guest.
proof fn halt_with_wake_resumes()
    ensures exit_disposition_spec(EXIT_HALT, true) == DISP_RESUME_GUEST
{}

// r[verify hypervisor-exit.disposition]
/// Interrupt window resumes guest.
proof fn interrupt_window_resumes()
    ensures exit_disposition_spec(EXIT_INTERRUPT_WINDOW, false) == DISP_RESUME_GUEST
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[test hypervisor-exit.classify]
    #[test]
    fn test_classify_mmio() {
        assert_eq!(classify_exit(EXIT_MMIO), EXIT_MMIO);
    }

    // r[test hypervisor-exit.classify]
    #[test]
    fn test_classify_unknown() {
        assert_eq!(classify_exit(999), EXIT_INTERNAL_ERROR);
    }

    // r[test hypervisor-exit.disposition]
    #[test]
    fn test_io_userspace() {
        assert_eq!(exit_disposition(EXIT_IO, false), DISP_USERSPACE_HANDLE);
    }

    // r[test hypervisor-exit.disposition]
    #[test]
    fn test_shutdown_terminates() {
        assert_eq!(exit_disposition(EXIT_SHUTDOWN, false), DISP_TERMINATE);
    }

    // r[test hypervisor-exit.disposition]
    #[test]
    fn test_halt_blocks_without_wake() {
        assert_eq!(exit_disposition(EXIT_HALT, false), DISP_BLOCK);
    }

    // r[test hypervisor-exit.disposition]
    #[test]
    fn test_halt_resumes_with_wake() {
        assert_eq!(exit_disposition(EXIT_HALT, true), DISP_RESUME_GUEST);
    }
}
