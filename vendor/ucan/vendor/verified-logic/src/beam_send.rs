//! Verified BEAM asynchronous send semantics.
//!
//! Sends are fire-and-forget from the sender's point of view: sending never
//! blocks or reports failure. Live recipients append a copied message to
//! their mailbox; dead recipients silently drop it.

use vstd::prelude::*;

use crate::beam_mailbox::BeamMessage;
use crate::beam_mailbox::mailbox_append;
#[cfg(verus_keep_ghost)]
use crate::beam_mailbox::mailbox_append_spec;
use crate::beam_process::BEAM_DEAD;

verus! {

fn clone_mailbox(mailbox: &[BeamMessage]) -> (result: Vec<BeamMessage>)
    ensures result@ =~= mailbox@
{
    let mut result: Vec<BeamMessage> = Vec::new();
    let mut i: usize = 0;
    while i < mailbox.len()
        invariant
            0 <= i <= mailbox@.len(),
            result@ =~= mailbox@.subrange(0, i as int),
        decreases mailbox@.len() - i,
    {
        result.push(mailbox[i]);
        i = i + 1;
    }
    result
}

// r[depends beam-send.async]
/// Spec: sending to a dead process drops the message; otherwise append.
pub open spec fn beam_send_spec(recipient_state: u64, mailbox: Seq<BeamMessage>, msg: BeamMessage) -> Seq<BeamMessage> {
    if recipient_state == BEAM_DEAD {
        mailbox
    } else {
        mailbox_append_spec(mailbox, msg)
    }
}

// r[impl beam-send.async]
/// Send a message to a recipient mailbox. The operation is total and never
/// fails from the sender's point of view.
pub fn beam_send(recipient_state: u64, mailbox: &[BeamMessage], msg: BeamMessage) -> (result: Vec<BeamMessage>)
    ensures result@ == beam_send_spec(recipient_state, mailbox@, msg)
{
    if recipient_state == BEAM_DEAD {
        clone_mailbox(mailbox)
    } else {
        mailbox_append(mailbox, msg)
    }
}

// r[verify beam-send.async]
proof fn send_to_live_appends(mailbox: Seq<BeamMessage>, msg: BeamMessage, recipient_state: u64)
    requires recipient_state != BEAM_DEAD
    ensures beam_send_spec(recipient_state, mailbox, msg) == mailbox_append_spec(mailbox, msg)
{}

// r[verify beam-send.async]
proof fn send_to_dead_drops(mailbox: Seq<BeamMessage>, msg: BeamMessage)
    ensures beam_send_spec(BEAM_DEAD, mailbox, msg) =~= mailbox
{}

// r[verify beam-send.async]
proof fn send_is_total(recipient_state: u64, mailbox: Seq<BeamMessage>, msg: BeamMessage)
    ensures beam_send_spec(recipient_state, mailbox, msg).len() >= 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::beam_mailbox::BeamMessage;
    use crate::beam_mailbox::MESSAGE_DATA;
    use crate::beam_process::BEAM_ALIVE;
    use crate::beam_process::BEAM_DEAD;

    fn data(sender: u64, payload: u64) -> BeamMessage {
        BeamMessage {
            sender,
            kind: MESSAGE_DATA,
            payload,
            ref_id: 0,
            observed_pid: 0,
            reason: 0,
        }
    }

    // r[verify beam-send.async]
    #[test]
    fn test_send_to_live_process() {
        let mailbox = vec![data(1, 10)];
        let next = beam_send(BEAM_ALIVE, &mailbox, data(2, 20));
        assert_eq!(next, vec![data(1, 10), data(2, 20)]);
    }

    // r[verify beam-send.async]
    #[test]
    fn test_send_to_dead_process() {
        let mailbox = vec![data(1, 10)];
        let next = beam_send(BEAM_DEAD, &mailbox, data(2, 20));
        assert_eq!(next, mailbox);
    }

    proptest! {
        // r[verify beam-send.async]
        #[test]
        fn prop_send_either_keeps_or_grows(
            mailbox in proptest::collection::vec((any::<u64>(), any::<u64>()), 0..12),
            sender: u64,
            payload: u64,
            dead: bool,
        ) {
            let mailbox: Vec<BeamMessage> = mailbox.into_iter().map(|(s, p)| data(s, p)).collect();
            let state = if dead { BEAM_DEAD } else { BEAM_ALIVE };
            let next = beam_send(state, &mailbox, data(sender, payload));
            if dead {
                prop_assert_eq!(next, mailbox);
            } else {
                prop_assert_eq!(next.len(), mailbox.len() + 1);
                prop_assert_eq!(next.last().unwrap().payload, payload);
            }
        }
    }
}
