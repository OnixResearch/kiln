//! Verified seL4-style fault IPC metadata preservation.
//!
//! Models a small structured fault vocabulary, semantic IPC encode/decode, and
//! handler-facing delivery. Correctness focus is preserving fault class and
//! payload fields through encode/decode and delivery, not reproducing the raw
//! seL4 message-register ABI.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub enum AccessKind {
    Read,
    Write,
    Execute,
}

#[derive(Copy, Clone)]
pub enum FaultClass {
    PageFault,
    Generic,
}

#[derive(Copy, Clone)]
pub enum Fault {
    PageFault { address: u64, access: AccessKind, ip: u64 },
    Generic { code: u64, data: u64, ip: u64 },
}

#[derive(Copy, Clone)]
pub struct FaultMessage {
    pub class: FaultClass,
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
}

#[derive(Copy, Clone)]
pub struct HandlerContext {
    pub handler_tcb: Option<usize>,
    pub inbox: Option<FaultMessage>,
}

// r[impl sel4-fault-ipc.preservation]
// r[depends sel4-fault-ipc.preservation]
pub open spec fn access_word_spec(access: AccessKind) -> u64 {
    match access {
        AccessKind::Read => 0,
        AccessKind::Write => 1,
        AccessKind::Execute => 2,
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[depends sel4-fault-ipc.preservation]
pub open spec fn access_from_word_spec(word: u64) -> AccessKind {
    if word == 1 {
        AccessKind::Write
    } else if word == 2 {
        AccessKind::Execute
    } else {
        AccessKind::Read
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[depends sel4-fault-ipc.preservation]
pub open spec fn fault_class_spec(fault: Fault) -> FaultClass {
    match fault {
        Fault::PageFault { .. } => FaultClass::PageFault,
        Fault::Generic { .. } => FaultClass::Generic,
    }
}

// r[impl sel4-fault-ipc.payload]
// r[depends sel4-fault-ipc.payload]
pub open spec fn fault_ip_spec(fault: Fault) -> u64 {
    match fault {
        Fault::PageFault { ip, .. } => ip,
        Fault::Generic { ip, .. } => ip,
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[impl sel4-fault-ipc.payload]
// r[depends sel4-fault-ipc.preservation]
// r[depends sel4-fault-ipc.payload]
pub open spec fn encode_fault_spec(fault: Fault) -> FaultMessage {
    match fault {
        Fault::PageFault { address, access, ip } => FaultMessage {
            class: FaultClass::PageFault,
            word0: address,
            word1: access_word_spec(access),
            word2: ip,
        },
        Fault::Generic { code, data, ip } => FaultMessage {
            class: FaultClass::Generic,
            word0: code,
            word1: data,
            word2: ip,
        },
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[impl sel4-fault-ipc.payload]
// r[depends sel4-fault-ipc.preservation]
// r[depends sel4-fault-ipc.payload]
pub open spec fn decode_fault_spec(message: FaultMessage) -> Fault {
    match message.class {
        FaultClass::PageFault => Fault::PageFault {
            address: message.word0,
            access: access_from_word_spec(message.word1),
            ip: message.word2,
        },
        FaultClass::Generic => Fault::Generic {
            code: message.word0,
            data: message.word1,
            ip: message.word2,
        },
    }
}

// r[impl sel4-fault-ipc.delivery]
// r[depends sel4-fault-ipc.delivery]
pub open spec fn received_fault_spec(ctx: HandlerContext) -> Option<Fault> {
    match ctx.inbox {
        Some(message) => Some(decode_fault_spec(message)),
        None => None,
    }
}

// r[impl sel4-fault-ipc.delivery]
pub fn new_handler_context(handler_tcb: Option<usize>) -> (result: HandlerContext)
    ensures
        result.handler_tcb == handler_tcb,
        result.inbox.is_none(),
        received_fault_spec(result).is_none(),
{
    HandlerContext {
        handler_tcb,
        inbox: None,
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[impl sel4-fault-ipc.payload]
pub fn encode_fault(fault: Fault) -> (result: FaultMessage)
    ensures
        result == encode_fault_spec(fault),
{
    match fault {
        Fault::PageFault { address, access, ip } => FaultMessage {
            class: FaultClass::PageFault,
            word0: address,
            word1: match access {
                AccessKind::Read => 0,
                AccessKind::Write => 1,
                AccessKind::Execute => 2,
            },
            word2: ip,
        },
        Fault::Generic { code, data, ip } => FaultMessage {
            class: FaultClass::Generic,
            word0: code,
            word1: data,
            word2: ip,
        },
    }
}

// r[impl sel4-fault-ipc.preservation]
// r[impl sel4-fault-ipc.payload]
pub fn decode_fault(message: FaultMessage) -> (result: Fault)
    ensures
        result == decode_fault_spec(message),
{
    match message.class {
        FaultClass::PageFault => Fault::PageFault {
            address: message.word0,
            access: if message.word1 == 1 {
                AccessKind::Write
            } else if message.word1 == 2 {
                AccessKind::Execute
            } else {
                AccessKind::Read
            },
            ip: message.word2,
        },
        FaultClass::Generic => Fault::Generic {
            code: message.word0,
            data: message.word1,
            ip: message.word2,
        },
    }
}

// r[impl sel4-fault-ipc.delivery]
pub fn deliver_fault(ctx: &mut HandlerContext, fault: Fault) -> (result: bool)
    ensures
        ctx.handler_tcb == old(ctx).handler_tcb,
        if result {
            &&& old(ctx).handler_tcb.is_some()
            &&& ctx.inbox == Some(encode_fault_spec(fault))
            &&& received_fault_spec(*ctx) == Some(fault)
        } else {
            &&& old(ctx).handler_tcb.is_none()
            &&& ctx.inbox == old(ctx).inbox
            &&& received_fault_spec(*ctx) == received_fault_spec(*old(ctx))
        }
{
    if ctx.handler_tcb.is_none() {
        false
    } else {
        ctx.inbox = Some(encode_fault(fault));
        true
    }
}

// r[impl sel4-fault-ipc.delivery]
pub fn received_fault(ctx: &HandlerContext) -> (result: Option<Fault>)
    ensures
        result == received_fault_spec(*ctx),
{
    match ctx.inbox {
        Some(message) => Some(decode_fault(message)),
        None => None,
    }
}

// r[verify sel4-fault-ipc.preservation]
proof fn access_roundtrip(access: AccessKind)
    ensures
        access_from_word_spec(access_word_spec(access)) == access,
{
}

// r[verify sel4-fault-ipc.preservation]
proof fn encode_decode_roundtrip(fault: Fault)
    ensures
        decode_fault_spec(encode_fault_spec(fault)) == fault,
        fault_class_spec(decode_fault_spec(encode_fault_spec(fault))) == fault_class_spec(fault),
{
}

// r[verify sel4-fault-ipc.delivery]
proof fn delivery_to_configured_handler(handler: usize, fault: Fault)
    ensures
        received_fault_spec(HandlerContext {
            handler_tcb: Some(handler),
            inbox: Some(encode_fault_spec(fault)),
        }) == Some(fault),
{
}

// r[verify sel4-fault-ipc.payload]
proof fn encode_decode_preserves_ip(fault: Fault)
    ensures
        fault_ip_spec(decode_fault_spec(encode_fault_spec(fault))) == fault_ip_spec(fault),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn access_from_u8(value: u8) -> AccessKind {
        match value % 3 {
            0 => AccessKind::Read,
            1 => AccessKind::Write,
            _ => AccessKind::Execute,
        }
    }

    // r[verify sel4-fault-ipc.preservation]
    #[test]
    fn test_page_fault_roundtrip() {
        let fault = Fault::PageFault {
            address: 0x1234,
            access: AccessKind::Write,
            ip: 0xfeed,
        };
        let decoded = decode_fault(encode_fault(fault));
        match decoded {
            Fault::PageFault { address, access, ip } => {
                assert_eq!(address, 0x1234);
                assert!(matches!(access, AccessKind::Write));
                assert_eq!(ip, 0xfeed);
            }
            Fault::Generic { .. } => panic!("expected page fault"),
        }
    }

    // r[verify sel4-fault-ipc.payload]
    #[test]
    fn test_generic_fault_preserves_ip() {
        let fault = Fault::Generic {
            code: 7,
            data: 9,
            ip: 0xabc,
        };
        let decoded = decode_fault(encode_fault(fault));
        match decoded {
            Fault::Generic { code, data, ip } => {
                assert_eq!(code, 7);
                assert_eq!(data, 9);
                assert_eq!(ip, 0xabc);
            }
            Fault::PageFault { .. } => panic!("expected generic fault"),
        }
    }

    // r[verify sel4-fault-ipc.delivery]
    #[test]
    fn test_configured_handler_receives_fault() {
        let fault = Fault::PageFault {
            address: 0x4000,
            access: AccessKind::Execute,
            ip: 0x4010,
        };
        let mut ctx = new_handler_context(Some(42));
        assert!(deliver_fault(&mut ctx, fault));
        let received = received_fault(&ctx).expect("configured handler should receive fault");
        match received {
            Fault::PageFault { address, access, ip } => {
                assert_eq!(address, 0x4000);
                assert!(matches!(access, AccessKind::Execute));
                assert_eq!(ip, 0x4010);
            }
            Fault::Generic { .. } => panic!("expected page fault"),
        }
    }

    // r[verify sel4-fault-ipc.delivery]
    #[test]
    fn test_unconfigured_handler_drops_fault() {
        let fault = Fault::Generic {
            code: 1,
            data: 2,
            ip: 3,
        };
        let mut ctx = new_handler_context(None);
        assert!(!deliver_fault(&mut ctx, fault));
        assert!(received_fault(&ctx).is_none());
    }

    proptest! {
        // r[verify sel4-fault-ipc.preservation]
        #[test]
        fn prop_page_fault_roundtrip(address: u64, access_code in 0u8..=2, ip: u64) {
            let fault = Fault::PageFault {
                address,
                access: access_from_u8(access_code),
                ip,
            };
            let decoded = decode_fault(encode_fault(fault));
            match decoded {
                Fault::PageFault { address: decoded_address, access, ip: decoded_ip } => {
                    prop_assert_eq!(decoded_address, address);
                    prop_assert_eq!(decoded_ip, ip);
                    match (access, access_from_u8(access_code)) {
                        (AccessKind::Read, AccessKind::Read)
                        | (AccessKind::Write, AccessKind::Write)
                        | (AccessKind::Execute, AccessKind::Execute) => {}
                        _ => prop_assert!(false, "access kind mismatch"),
                    }
                }
                Fault::Generic { .. } => prop_assert!(false, "expected page fault"),
            }
        }

        // r[verify sel4-fault-ipc.payload]
        #[test]
        fn prop_generic_fault_roundtrip(code: u64, data: u64, ip: u64) {
            let fault = Fault::Generic { code, data, ip };
            let decoded = decode_fault(encode_fault(fault));
            match decoded {
                Fault::Generic { code: decoded_code, data: decoded_data, ip: decoded_ip } => {
                    prop_assert_eq!(decoded_code, code);
                    prop_assert_eq!(decoded_data, data);
                    prop_assert_eq!(decoded_ip, ip);
                }
                Fault::PageFault { .. } => prop_assert!(false, "expected generic fault"),
            }
        }

        // r[verify sel4-fault-ipc.delivery]
        #[test]
        fn prop_delivery_preserves_fault_metadata(handler: usize, address: u64, access_code in 0u8..=2, ip: u64) {
            let fault = Fault::PageFault {
                address,
                access: access_from_u8(access_code),
                ip,
            };
            let mut ctx = new_handler_context(Some(handler));
            prop_assert!(deliver_fault(&mut ctx, fault));
            let received = received_fault(&ctx).expect("configured handler should receive fault");
            match received {
                Fault::PageFault { address: decoded_address, access, ip: decoded_ip } => {
                    prop_assert_eq!(decoded_address, address);
                    prop_assert_eq!(decoded_ip, ip);
                    match (access, access_from_u8(access_code)) {
                        (AccessKind::Read, AccessKind::Read)
                        | (AccessKind::Write, AccessKind::Write)
                        | (AccessKind::Execute, AccessKind::Execute) => {}
                        _ => prop_assert!(false, "access kind mismatch"),
                    }
                }
                Fault::Generic { .. } => prop_assert!(false, "expected page fault"),
            }
        }
    }
}
