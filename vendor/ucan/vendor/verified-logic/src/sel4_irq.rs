//! Verified seL4-style IRQ control primitives.
//!
//! Models one IRQ line with at most one live handler binding plus abstract
//! pending and masked state. Delivery is eligible exactly when the line is
//! pending, unmasked, and bound. Acknowledge consumes pending state.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub struct IrqLine {
    pub irq: usize,
    pub handler: Option<usize>,
    pub pending: bool,
    pub masked: bool,
}

// r[impl sel4-irq.binding]
// r[depends sel4-irq.binding]
pub open spec fn can_bind_spec(line: IrqLine) -> bool {
    line.handler.is_none()
}

// r[impl sel4-irq.masking]
// r[depends sel4-irq.masking]
pub open spec fn deliverable_spec(line: IrqLine) -> bool {
    &&& line.handler.is_some()
    &&& line.pending
    &&& !line.masked
}

// r[impl sel4-irq.binding]
// r[depends sel4-irq.binding]
pub open spec fn bind_transition_spec(line: IrqLine, handler: usize) -> IrqLine {
    if can_bind_spec(line) {
        IrqLine {
            irq: line.irq,
            handler: Some(handler),
            pending: line.pending,
            masked: line.masked,
        }
    } else {
        line
    }
}

// r[impl sel4-irq.masking]
// r[depends sel4-irq.masking]
pub open spec fn mask_transition_spec(line: IrqLine) -> IrqLine {
    IrqLine {
        irq: line.irq,
        handler: line.handler,
        pending: line.pending,
        masked: true,
    }
}

// r[impl sel4-irq.masking]
// r[depends sel4-irq.masking]
pub open spec fn unmask_transition_spec(line: IrqLine) -> IrqLine {
    IrqLine {
        irq: line.irq,
        handler: line.handler,
        pending: line.pending,
        masked: false,
    }
}

// r[impl sel4-irq.ack]
// r[depends sel4-irq.ack]
pub open spec fn ack_transition_spec(line: IrqLine) -> IrqLine {
    IrqLine {
        irq: line.irq,
        handler: line.handler,
        pending: false,
        masked: line.masked,
    }
}

// r[impl sel4-irq.binding]
pub fn new_irq_line(irq: usize) -> (result: IrqLine)
    ensures
        result.irq == irq,
        result.handler.is_none(),
        !result.pending,
        !result.masked,
{
    IrqLine {
        irq,
        handler: None,
        pending: false,
        masked: false,
    }
}

// r[impl sel4-irq.binding]
pub fn bind_handler(line: &mut IrqLine, handler: usize) -> (result: bool)
    ensures
        *line == bind_transition_spec(*old(line), handler),
        result == can_bind_spec(*old(line)),
{
    if line.handler.is_none() {
        line.handler = Some(handler);
        true
    } else {
        false
    }
}

// r[impl sel4-irq.binding]
pub fn unbind_handler(line: &mut IrqLine)
    ensures
        line.irq == old(line).irq,
        line.handler.is_none(),
        line.pending == old(line).pending,
        line.masked == old(line).masked,
{
    line.handler = None;
}

// r[impl sel4-irq.masking]
pub fn mask(line: &mut IrqLine)
    ensures
        *line == mask_transition_spec(*old(line)),
{
    line.masked = true;
}

// r[impl sel4-irq.masking]
pub fn unmask(line: &mut IrqLine)
    ensures
        *line == unmask_transition_spec(*old(line)),
{
    line.masked = false;
}

// r[impl sel4-irq.ack]
pub fn set_pending(line: &mut IrqLine)
    ensures
        line.irq == old(line).irq,
        line.handler == old(line).handler,
        line.pending,
        line.masked == old(line).masked,
{
    line.pending = true;
}

// r[impl sel4-irq.ack]
pub fn acknowledge(line: &mut IrqLine)
    ensures
        *line == ack_transition_spec(*old(line)),
{
    line.pending = false;
}

// r[impl sel4-irq.masking]
pub fn is_deliverable(line: &IrqLine) -> (result: bool)
    ensures
        result == deliverable_spec(*line),
{
    line.handler.is_some() && line.pending && !line.masked
}

// r[verify sel4-irq.binding]
proof fn second_live_bind_rejected(irq: usize, h1: usize, h2: usize)
    ensures
        !can_bind_spec(IrqLine { irq, handler: Some(h1), pending: false, masked: false }),
        bind_transition_spec(
            IrqLine { irq, handler: Some(h1), pending: false, masked: false },
            h2,
        ) == (IrqLine { irq, handler: Some(h1), pending: false, masked: false }),
{
}

// r[verify sel4-irq.masking]
proof fn masked_irq_not_deliverable(irq: usize, handler: usize, pending: bool)
    ensures
        !deliverable_spec(IrqLine { irq, handler: Some(handler), pending, masked: true }),
{
}

// r[verify sel4-irq.ack]
proof fn acknowledge_consumes_pending(irq: usize, handler: Option<usize>, masked: bool)
    ensures
        !ack_transition_spec(IrqLine { irq, handler, pending: true, masked }).pending,
        !deliverable_spec(ack_transition_spec(IrqLine { irq, handler, pending: true, masked })),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-irq.binding]
    #[test]
    fn test_second_live_bind_rejected() {
        let mut line = new_irq_line(3);
        assert!(bind_handler(&mut line, 10));
        assert!(!bind_handler(&mut line, 11));
        assert_eq!(line.handler, Some(10));
    }

    // r[verify sel4-irq.masking]
    #[test]
    fn test_mask_blocks_delivery() {
        let mut line = new_irq_line(4);
        assert!(bind_handler(&mut line, 22));
        set_pending(&mut line);
        assert!(is_deliverable(&line));
        mask(&mut line);
        assert!(!is_deliverable(&line));
    }

    // r[verify sel4-irq.ack]
    #[test]
    fn test_ack_clears_pending_delivery() {
        let mut line = new_irq_line(5);
        assert!(bind_handler(&mut line, 33));
        set_pending(&mut line);
        assert!(is_deliverable(&line));
        acknowledge(&mut line);
        assert!(!line.pending);
        assert!(!is_deliverable(&line));
    }

    // r[verify sel4-irq.masking]
    #[test]
    fn test_unmask_restores_delivery_if_pending_and_bound() {
        let mut line = new_irq_line(6);
        assert!(bind_handler(&mut line, 44));
        set_pending(&mut line);
        mask(&mut line);
        assert!(!is_deliverable(&line));
        unmask(&mut line);
        assert!(is_deliverable(&line));
    }

    proptest! {
        // r[verify sel4-irq.binding]
        #[test]
        fn prop_only_first_bind_succeeds(irq: usize, h1: usize, h2: usize) {
            let mut line = new_irq_line(irq);
            prop_assert!(bind_handler(&mut line, h1));
            prop_assert!(!bind_handler(&mut line, h2));
            prop_assert_eq!(line.handler, Some(h1));
        }

        // r[verify sel4-irq.masking]
        #[test]
        fn prop_masked_pending_irq_not_deliverable(irq: usize, handler: usize) {
            let mut line = new_irq_line(irq);
            prop_assert!(bind_handler(&mut line, handler));
            set_pending(&mut line);
            mask(&mut line);
            prop_assert!(!is_deliverable(&line));
        }

        // r[verify sel4-irq.ack]
        #[test]
        fn prop_acknowledge_clears_pending(irq: usize, handler: usize, masked: bool) {
            let mut line = new_irq_line(irq);
            prop_assert!(bind_handler(&mut line, handler));
            if masked {
                mask(&mut line);
            }
            set_pending(&mut line);
            acknowledge(&mut line);
            prop_assert!(!line.pending);
            prop_assert!(!is_deliverable(&line));
        }
    }
}
