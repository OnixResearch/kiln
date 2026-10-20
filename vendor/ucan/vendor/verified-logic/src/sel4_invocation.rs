//! Verified seL4-style invocation decode for a small abstract starter set.
//!
//! Models decode as a local check over an abstract message and an explicit
//! capability context. Scope is intentionally small: endpoint send, endpoint
//! receive, frame map, and untyped retype.
//!
//! For this starter set, frame-map authorization is checked against the primary
//! frame capability only; the secondary VSpace capability is a typed object
//! reference that selects the address-space root. Exact mapping alignment,
//! page-size constraints, and untyped layout constraints stay in the downstream
//! `sel4_vspace` / `sel4_untyped` modules rather than in decode. Decode only
//! treats zero size / zero alignment as malformed message shapes.

use vstd::prelude::*;

use crate::sel4_cap_rights::can_control;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::can_control_spec;
use crate::sel4_cap_rights::can_map;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::can_map_spec;
use crate::sel4_cap_rights::can_receive;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::can_receive_spec;
use crate::sel4_cap_rights::can_send;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::can_send_spec;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CapKind {
    Endpoint,
    Frame,
    VSpace,
    Untyped,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    pub kind: CapKind,
    pub object: usize,
    pub rights: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DecodeContext {
    pub primary: Capability,
    pub secondary: Option<Capability>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InvocationLabel {
    EndpointSend,
    EndpointReceive,
    FrameMap,
    UntypedRetype,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InvocationMessage {
    pub label: InvocationLabel,
    pub arg_count: u8,
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
    pub has_extra_cap: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Invocation {
    EndpointSend { endpoint: usize, badge: u64 },
    EndpointReceive { endpoint: usize },
    FrameMap { frame: usize, vspace: usize, vaddr: u64, size: u64, requested_rights: u64 },
    UntypedRetype { untyped: usize, base: u64, size: u64, align: u64, kind: u64 },
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn endpoint_send_shape_spec(message: InvocationMessage) -> bool {
    &&& message.arg_count == 1u8
    &&& !message.has_extra_cap
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn endpoint_receive_shape_spec(message: InvocationMessage) -> bool {
    &&& message.arg_count == 0u8
    &&& !message.has_extra_cap
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
/// Decode-level shape check only: downstream `sel4_vspace` handles exact
/// alignment and page-size validity.
pub open spec fn frame_map_shape_spec(message: InvocationMessage) -> bool {
    &&& message.arg_count == 3u8
    &&& message.has_extra_cap
    &&& message.word1 > 0
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
/// Decode-level shape check only: downstream `sel4_untyped` handles exact
/// layout and alignment discipline.
pub open spec fn untyped_retype_shape_spec(message: InvocationMessage) -> bool {
    &&& message.arg_count == 4u8
    &&& !message.has_extra_cap
    &&& message.word1 > 0
    &&& message.word2 > 0
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn is_endpoint_kind_spec(kind: CapKind) -> bool {
    match kind {
        CapKind::Endpoint => true,
        _ => false,
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn is_frame_kind_spec(kind: CapKind) -> bool {
    match kind {
        CapKind::Frame => true,
        _ => false,
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn is_vspace_kind_spec(kind: CapKind) -> bool {
    match kind {
        CapKind::VSpace => true,
        _ => false,
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn is_untyped_kind_spec(kind: CapKind) -> bool {
    match kind {
        CapKind::Untyped => true,
        _ => false,
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
pub open spec fn invocation_type_correct_spec(inv: Invocation, ctx: DecodeContext) -> bool {
    match inv {
        Invocation::EndpointSend { endpoint, .. } => {
            &&& is_endpoint_kind_spec(ctx.primary.kind)
            &&& endpoint == ctx.primary.object
        }
        Invocation::EndpointReceive { endpoint } => {
            &&& is_endpoint_kind_spec(ctx.primary.kind)
            &&& endpoint == ctx.primary.object
        }
        Invocation::FrameMap { frame, vspace, .. } => {
            &&& is_frame_kind_spec(ctx.primary.kind)
            &&& frame == ctx.primary.object
            &&& match ctx.secondary {
                Some(cap) => is_vspace_kind_spec(cap.kind) && vspace == cap.object,
                None => false,
            }
        }
        Invocation::UntypedRetype { untyped, .. } => {
            &&& is_untyped_kind_spec(ctx.primary.kind)
            &&& untyped == ctx.primary.object
        }
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[depends sel4-invocation.decode-soundness]
/// Authorization stays local to the acting capability for this starter set.
/// `FrameMap` checks the frame capability's map right; the secondary VSpace
/// capability is modeled only as a typed root reference.
pub open spec fn invocation_authorized_spec(inv: Invocation, ctx: DecodeContext) -> bool {
    match inv {
        Invocation::EndpointSend { .. } => can_send_spec(ctx.primary.rights),
        Invocation::EndpointReceive { .. } => can_receive_spec(ctx.primary.rights),
        Invocation::FrameMap { .. } => can_map_spec(ctx.primary.rights),
        Invocation::UntypedRetype { .. } => can_control_spec(ctx.primary.rights),
    }
}

// r[impl sel4-invocation.no-invented-authority]
// r[depends sel4-invocation.no-invented-authority]
pub open spec fn invocation_uses_only_context_authority_spec(inv: Invocation, ctx: DecodeContext) -> bool {
    match inv {
        Invocation::EndpointSend { endpoint, .. } => endpoint == ctx.primary.object,
        Invocation::EndpointReceive { endpoint } => endpoint == ctx.primary.object,
        Invocation::FrameMap { frame, vspace, .. } => {
            &&& frame == ctx.primary.object
            &&& match ctx.secondary {
                Some(cap) => vspace == cap.object,
                None => false,
            }
        }
        Invocation::UntypedRetype { untyped, .. } => untyped == ctx.primary.object,
    }
}

// r[impl sel4-invocation.reject]
// r[depends sel4-invocation.reject]
pub open spec fn malformed_message_spec(message: InvocationMessage) -> bool {
    match message.label {
        InvocationLabel::EndpointSend => !endpoint_send_shape_spec(message),
        InvocationLabel::EndpointReceive => !endpoint_receive_shape_spec(message),
        InvocationLabel::FrameMap => !frame_map_shape_spec(message),
        InvocationLabel::UntypedRetype => !untyped_retype_shape_spec(message),
    }
}

// r[impl sel4-invocation.reject]
// r[depends sel4-invocation.reject]
pub open spec fn wrong_type_request_spec(message: InvocationMessage, ctx: DecodeContext) -> bool {
    match message.label {
        InvocationLabel::EndpointSend | InvocationLabel::EndpointReceive => {
            !is_endpoint_kind_spec(ctx.primary.kind)
        }
        InvocationLabel::FrameMap => {
            ||| !is_frame_kind_spec(ctx.primary.kind)
            ||| match ctx.secondary {
                Some(cap) => !is_vspace_kind_spec(cap.kind),
                None => true,
            }
        }
        InvocationLabel::UntypedRetype => !is_untyped_kind_spec(ctx.primary.kind),
    }
}

// r[impl sel4-invocation.reject]
// r[depends sel4-invocation.reject]
pub open spec fn under_authorized_request_spec(message: InvocationMessage, ctx: DecodeContext) -> bool {
    match message.label {
        InvocationLabel::EndpointSend => {
            &&& endpoint_send_shape_spec(message)
            &&& is_endpoint_kind_spec(ctx.primary.kind)
            &&& !can_send_spec(ctx.primary.rights)
        }
        InvocationLabel::EndpointReceive => {
            &&& endpoint_receive_shape_spec(message)
            &&& is_endpoint_kind_spec(ctx.primary.kind)
            &&& !can_receive_spec(ctx.primary.rights)
        }
        InvocationLabel::FrameMap => {
            &&& frame_map_shape_spec(message)
            &&& is_frame_kind_spec(ctx.primary.kind)
            &&& match ctx.secondary {
                Some(cap) => is_vspace_kind_spec(cap.kind),
                None => false,
            }
            &&& !can_map_spec(ctx.primary.rights)
        }
        InvocationLabel::UntypedRetype => {
            &&& untyped_retype_shape_spec(message)
            &&& is_untyped_kind_spec(ctx.primary.kind)
            &&& !can_control_spec(ctx.primary.rights)
        }
    }
}

// r[impl sel4-invocation.reject]
// r[depends sel4-invocation.reject]
pub open spec fn invalid_decode_request_spec(message: InvocationMessage, ctx: DecodeContext) -> bool {
    ||| malformed_message_spec(message)
    ||| wrong_type_request_spec(message, ctx)
    ||| under_authorized_request_spec(message, ctx)
}

// r[impl sel4-invocation.decode-soundness]
// r[impl sel4-invocation.reject]
// r[impl sel4-invocation.no-invented-authority]
// r[depends sel4-invocation.decode-soundness]
// r[depends sel4-invocation.reject]
// r[depends sel4-invocation.no-invented-authority]
pub open spec fn decode_spec(message: InvocationMessage, ctx: DecodeContext) -> Option<Invocation> {
    match message.label {
        InvocationLabel::EndpointSend => {
            if endpoint_send_shape_spec(message)
                && is_endpoint_kind_spec(ctx.primary.kind)
                && can_send_spec(ctx.primary.rights)
            {
                Some(Invocation::EndpointSend {
                    endpoint: ctx.primary.object,
                    badge: message.word0,
                })
            } else {
                None
            }
        }
        InvocationLabel::EndpointReceive => {
            if endpoint_receive_shape_spec(message)
                && is_endpoint_kind_spec(ctx.primary.kind)
                && can_receive_spec(ctx.primary.rights)
            {
                Some(Invocation::EndpointReceive {
                    endpoint: ctx.primary.object,
                })
            } else {
                None
            }
        }
        InvocationLabel::FrameMap => {
            match ctx.secondary {
                Some(cap) => {
                    if frame_map_shape_spec(message)
                        && is_frame_kind_spec(ctx.primary.kind)
                        && is_vspace_kind_spec(cap.kind)
                        && can_map_spec(ctx.primary.rights)
                    {
                        Some(Invocation::FrameMap {
                            frame: ctx.primary.object,
                            vspace: cap.object,
                            vaddr: message.word0,
                            size: message.word1,
                            requested_rights: message.word2,
                        })
                    } else {
                        None
                    }
                }
                None => None,
            }
        }
        InvocationLabel::UntypedRetype => {
            if untyped_retype_shape_spec(message)
                && is_untyped_kind_spec(ctx.primary.kind)
                && can_control_spec(ctx.primary.rights)
            {
                Some(Invocation::UntypedRetype {
                    untyped: ctx.primary.object,
                    base: message.word0,
                    size: message.word1,
                    align: message.word2,
                    kind: message.word3,
                })
            } else {
                None
            }
        }
    }
}

// r[impl sel4-invocation.decode-soundness]
// r[impl sel4-invocation.reject]
// r[impl sel4-invocation.no-invented-authority]
pub fn decode(message: InvocationMessage, ctx: DecodeContext) -> (result: Option<Invocation>)
    ensures
        result == decode_spec(message, ctx),
{
    match message.label {
        InvocationLabel::EndpointSend => {
            if message.arg_count == 1u8
                && !message.has_extra_cap
                && matches!(ctx.primary.kind, CapKind::Endpoint)
                && can_send(ctx.primary.rights)
            {
                Some(Invocation::EndpointSend {
                    endpoint: ctx.primary.object,
                    badge: message.word0,
                })
            } else {
                None
            }
        }
        InvocationLabel::EndpointReceive => {
            if message.arg_count == 0u8
                && !message.has_extra_cap
                && matches!(ctx.primary.kind, CapKind::Endpoint)
                && can_receive(ctx.primary.rights)
            {
                Some(Invocation::EndpointReceive {
                    endpoint: ctx.primary.object,
                })
            } else {
                None
            }
        }
        InvocationLabel::FrameMap => {
            match ctx.secondary {
                Some(cap) => {
                    if message.arg_count == 3u8
                        && message.has_extra_cap
                        && message.word1 > 0
                        && matches!(ctx.primary.kind, CapKind::Frame)
                        && matches!(cap.kind, CapKind::VSpace)
                        && can_map(ctx.primary.rights)
                    {
                        Some(Invocation::FrameMap {
                            frame: ctx.primary.object,
                            vspace: cap.object,
                            vaddr: message.word0,
                            size: message.word1,
                            requested_rights: message.word2,
                        })
                    } else {
                        None
                    }
                }
                None => None,
            }
        }
        InvocationLabel::UntypedRetype => {
            if message.arg_count == 4u8
                && !message.has_extra_cap
                && message.word1 > 0
                && message.word2 > 0
                && matches!(ctx.primary.kind, CapKind::Untyped)
                && can_control(ctx.primary.rights)
            {
                Some(Invocation::UntypedRetype {
                    untyped: ctx.primary.object,
                    base: message.word0,
                    size: message.word1,
                    align: message.word2,
                    kind: message.word3,
                })
            } else {
                None
            }
        }
    }
}

// r[verify sel4-invocation.decode-soundness]
pub proof fn decode_success_is_type_correct_and_authorized(message: InvocationMessage, ctx: DecodeContext)
    ensures
        match decode_spec(message, ctx) {
            Some(inv) => {
                &&& invocation_type_correct_spec(inv, ctx)
                &&& invocation_authorized_spec(inv, ctx)
            }
            None => true,
        },
{
}

// r[verify sel4-invocation.reject]
pub proof fn decode_rejects_invalid_requests(message: InvocationMessage, ctx: DecodeContext)
    ensures
        invalid_decode_request_spec(message, ctx) ==> decode_spec(message, ctx) == None::<Invocation>,
{
}

// r[verify sel4-invocation.no-invented-authority]
pub proof fn decode_uses_only_context_authority(message: InvocationMessage, ctx: DecodeContext)
    ensures
        match decode_spec(message, ctx) {
            Some(inv) => invocation_uses_only_context_authority_spec(inv, ctx),
            None => true,
        },
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::sel4_cap_rights::SEL4_RIGHT_CONTROL;
    use crate::sel4_cap_rights::SEL4_RIGHT_MAP;
    use crate::sel4_cap_rights::SEL4_RIGHT_RECEIVE;
    use crate::sel4_cap_rights::SEL4_RIGHT_SEND;

    fn cap(kind: CapKind, object: usize, rights: u64) -> Capability {
        Capability { kind, object, rights }
    }

    fn send_message(badge: u64) -> InvocationMessage {
        InvocationMessage {
            label: InvocationLabel::EndpointSend,
            arg_count: 1,
            word0: badge,
            word1: 0,
            word2: 0,
            word3: 0,
            has_extra_cap: false,
        }
    }

    fn receive_message() -> InvocationMessage {
        InvocationMessage {
            label: InvocationLabel::EndpointReceive,
            arg_count: 0,
            word0: 0,
            word1: 0,
            word2: 0,
            word3: 0,
            has_extra_cap: false,
        }
    }

    fn map_message(vaddr: u64, size: u64, requested_rights: u64) -> InvocationMessage {
        InvocationMessage {
            label: InvocationLabel::FrameMap,
            arg_count: 3,
            word0: vaddr,
            word1: size,
            word2: requested_rights,
            word3: 0,
            has_extra_cap: true,
        }
    }

    fn retype_message(base: u64, size: u64, align: u64, kind: u64) -> InvocationMessage {
        InvocationMessage {
            label: InvocationLabel::UntypedRetype,
            arg_count: 4,
            word0: base,
            word1: size,
            word2: align,
            word3: kind,
            has_extra_cap: false,
        }
    }

    // r[verify sel4-invocation.decode-soundness]
    #[test]
    fn test_endpoint_send_decodes_when_authorized() {
        let ctx = DecodeContext {
            primary: cap(CapKind::Endpoint, 11, SEL4_RIGHT_SEND),
            secondary: None,
        };
        let result = decode(send_message(0xfeed), ctx);
        match result {
            Some(Invocation::EndpointSend { endpoint, badge }) => {
                assert_eq!(endpoint, 11);
                assert_eq!(badge, 0xfeed);
            }
            _ => panic!("expected endpoint send invocation"),
        }
    }

    // r[verify sel4-invocation.reject]
    #[test]
    fn test_endpoint_send_rejects_missing_send_right() {
        let ctx = DecodeContext {
            primary: cap(CapKind::Endpoint, 11, SEL4_RIGHT_RECEIVE),
            secondary: None,
        };
        assert!(decode(send_message(9), ctx).is_none());
    }

    // r[verify sel4-invocation.reject]
    #[test]
    fn test_frame_map_rejects_missing_secondary_vspace() {
        let ctx = DecodeContext {
            primary: cap(CapKind::Frame, 7, SEL4_RIGHT_MAP),
            secondary: None,
        };
        assert!(decode(map_message(0x4000, 0x1000, 0x3), ctx).is_none());
    }

    // r[verify sel4-invocation.reject]
    #[test]
    fn test_untyped_retype_rejects_malformed_zero_size() {
        let ctx = DecodeContext {
            primary: cap(CapKind::Untyped, 5, SEL4_RIGHT_CONTROL),
            secondary: None,
        };
        assert!(decode(retype_message(0x1000, 0, 0x1000, 3), ctx).is_none());
    }

    // r[verify sel4-invocation.no-invented-authority]
    #[test]
    fn test_frame_map_uses_only_context_objects() {
        let ctx = DecodeContext {
            primary: cap(CapKind::Frame, 17, SEL4_RIGHT_MAP),
            secondary: Some(cap(CapKind::VSpace, 23, 0)),
        };
        let result = decode(map_message(0x8000, 0x1000, 0x5), ctx);
        match result {
            Some(Invocation::FrameMap {
                frame,
                vspace,
                vaddr,
                size,
                requested_rights,
            }) => {
                assert_eq!(frame, 17);
                assert_eq!(vspace, 23);
                assert_eq!(vaddr, 0x8000);
                assert_eq!(size, 0x1000);
                assert_eq!(requested_rights, 0x5);
            }
            _ => panic!("expected frame map invocation"),
        }
    }

    proptest! {
        // r[verify sel4-invocation.decode-soundness]
        #[test]
        fn prop_endpoint_receive_decodes_to_primary_endpoint(endpoint: usize, rights: u64) {
            let ctx = DecodeContext {
                primary: cap(CapKind::Endpoint, endpoint, rights | SEL4_RIGHT_RECEIVE),
                secondary: None,
            };
            let result = decode(receive_message(), ctx);
            match result {
                Some(Invocation::EndpointReceive { endpoint: decoded }) => {
                    prop_assert_eq!(decoded, endpoint);
                }
                _ => prop_assert!(false, "expected endpoint receive invocation"),
            }
        }

        // r[verify sel4-invocation.reject]
        #[test]
        fn prop_endpoint_send_without_send_right_is_rejected(endpoint: usize, badge: u64, rights: u64) {
            let ctx = DecodeContext {
                primary: cap(CapKind::Endpoint, endpoint, rights & !SEL4_RIGHT_SEND),
                secondary: None,
            };
            prop_assert!(decode(send_message(badge), ctx).is_none());
        }

        // r[verify sel4-invocation.no-invented-authority]
        #[test]
        fn prop_frame_map_reuses_context_objects(frame: usize, vspace: usize, vaddr: u64, size in 1u64..=0x2000, requested_rights: u64) {
            let ctx = DecodeContext {
                primary: cap(CapKind::Frame, frame, SEL4_RIGHT_MAP),
                secondary: Some(cap(CapKind::VSpace, vspace, 0)),
            };
            let result = decode(map_message(vaddr, size, requested_rights), ctx);
            match result {
                Some(Invocation::FrameMap { frame: decoded_frame, vspace: decoded_vspace, vaddr: decoded_vaddr, size: decoded_size, requested_rights: decoded_rights }) => {
                    prop_assert_eq!(decoded_frame, frame);
                    prop_assert_eq!(decoded_vspace, vspace);
                    prop_assert_eq!(decoded_vaddr, vaddr);
                    prop_assert_eq!(decoded_size, size);
                    prop_assert_eq!(decoded_rights, requested_rights);
                }
                _ => prop_assert!(false, "expected frame map invocation"),
            }
        }
    }
}
