//! Verified BEAM mailbox ordering and selective receive.
//!
//! Mailboxes are modeled as FIFO sequences of `BeamMessage`. Selective
//! receive scans from a saved offset, returns the first matching message,
//! preserves unmatched messages in order, resets the cursor on success,
//! and advances it to the mailbox end when no match exists.

use vstd::prelude::*;

verus! {

pub const MESSAGE_DATA: u64 = 0;
pub const MESSAGE_EXIT: u64 = 1;
pub const MESSAGE_DOWN: u64 = 2;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BeamMessage {
    pub sender: u64,
    pub kind: u64,
    pub payload: u64,
    pub ref_id: u64,
    pub observed_pid: u64,
    pub reason: u64,
}

/// Spec: exact BEAM message equality.
pub open spec fn same_message_spec(a: BeamMessage, b: BeamMessage) -> bool {
    a.sender == b.sender
        && a.kind == b.kind
        && a.payload == b.payload
        && a.ref_id == b.ref_id
        && a.observed_pid == b.observed_pid
        && a.reason == b.reason
}

/// Runtime exact BEAM message equality.
fn same_message(a: BeamMessage, b: BeamMessage) -> (result: bool)
    ensures result == same_message_spec(a, b)
{
    a.sender == b.sender
        && a.kind == b.kind
        && a.payload == b.payload
        && a.ref_id == b.ref_id
        && a.observed_pid == b.observed_pid
        && a.reason == b.reason
}

// r[depends beam-mailbox.ordering]
/// Spec: append to the tail of the mailbox.
pub open spec fn mailbox_append_spec(mailbox: Seq<BeamMessage>, msg: BeamMessage) -> Seq<BeamMessage> {
    mailbox.push(msg)
}

// r[depends beam-mailbox.ordering]
/// Spec: same-sender FIFO ordering.
pub open spec fn sender_before_spec(mailbox: Seq<BeamMessage>, sender: u64, earlier_payload: u64, later_payload: u64) -> bool {
    forall|i: int, j: int|
        #![trigger mailbox[i], mailbox[j]]
        0 <= i < j < mailbox.len()
            && mailbox[i].sender == sender
            && mailbox[j].sender == sender
            && mailbox[i].payload == later_payload
            && mailbox[j].payload == earlier_payload
        ==> false
}

/// Spec: mailbox contains the given message.
pub open spec fn mailbox_contains_spec(mailbox: Seq<BeamMessage>, msg: BeamMessage) -> bool {
    exists|i: int| #![trigger mailbox[i]] 0 <= i < mailbox.len() && same_message_spec(mailbox[i], msg)
}

/// Spec: message matches one of the exact patterns.
pub open spec fn message_in_patterns_spec(patterns: Seq<BeamMessage>, msg: BeamMessage) -> bool {
    exists|i: int| #![trigger patterns[i]] 0 <= i < patterns.len() && same_message_spec(patterns[i], msg)
}

/// Spec: first matching mailbox index at or after `from`, or -1 if absent.
pub open spec fn first_match_from_spec(mailbox: Seq<BeamMessage>, patterns: Seq<BeamMessage>, from: int) -> int
    decreases if from <= mailbox.len() { mailbox.len() - from } else { 0 }
{
    if from < 0 || from >= mailbox.len() {
        -1
    } else if message_in_patterns_spec(patterns, mailbox[from]) {
        from
    } else {
        first_match_from_spec(mailbox, patterns, from + 1)
    }
}

/// Spec: remove the message at index `idx`.
pub open spec fn mailbox_remove_index_spec(mailbox: Seq<BeamMessage>, idx: int) -> Seq<BeamMessage>
    recommends 0 <= idx < mailbox.len()
{
    mailbox.subrange(0, idx) + mailbox.subrange(idx + 1, mailbox.len() as int)
}

/// Spec: selective receive result.
pub open spec fn selective_receive_spec(mailbox: Seq<BeamMessage>, patterns: Seq<BeamMessage>, save_offset: int) -> (Option<BeamMessage>, Seq<BeamMessage>, int)
    recommends 0 <= save_offset <= mailbox.len()
{
    let idx = first_match_from_spec(mailbox, patterns, save_offset);
    if idx >= 0 {
        (Some(mailbox[idx]), mailbox_remove_index_spec(mailbox, idx), 0)
    } else {
        (None, mailbox, mailbox.len() as int)
    }
}

/// Helper: clone a mailbox.
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

// r[impl beam-mailbox.ordering]
/// Append a message to the mailbox tail.
pub fn mailbox_append(mailbox: &[BeamMessage], msg: BeamMessage) -> (result: Vec<BeamMessage>)
    ensures result@ == mailbox_append_spec(mailbox@, msg)
{
    let mut result = clone_mailbox(mailbox);
    result.push(msg);
    result
}

/// Helper: returns true if `msg` matches one of the exact patterns.
fn matches_any(patterns: &[BeamMessage], msg: BeamMessage) -> (result: bool)
    ensures result == message_in_patterns_spec(patterns@, msg)
{
    let mut i: usize = 0;
    while i < patterns.len()
        invariant
            0 <= i <= patterns@.len(),
            forall|j: int| #![trigger patterns@[j]] 0 <= j < i as int ==> !same_message_spec(patterns@[j], msg),
        decreases patterns@.len() - i,
    {
        if same_message(patterns[i], msg) {
            return true;
        }
        i = i + 1;
    }
    false
}

/// Spec helper: prefix `[0, upto)` with exactly `skip` removed.
pub open spec fn remove_prefix_index_spec(mailbox: Seq<BeamMessage>, skip: int, upto: int) -> Seq<BeamMessage>
    decreases if upto >= 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_prefix_index_spec(mailbox, skip, upto - 1);
        if upto - 1 == skip {
            prefix
        } else {
            prefix.push(mailbox[upto - 1])
        }
    }
}

/// Helper: clone mailbox while skipping one index.
fn clone_without_index(mailbox: &[BeamMessage], skip: usize) -> (result: Vec<BeamMessage>)
    requires skip < mailbox@.len()
    ensures result@.len() >= 0
{
    let mut result: Vec<BeamMessage> = Vec::new();
    let mut i: usize = 0;
    while i < mailbox.len()
        invariant
            0 <= i <= mailbox@.len(),
            skip < mailbox@.len(),
            result@ == remove_prefix_index_spec(mailbox@, skip as int, i as int),
        decreases mailbox@.len() - i,
    {
        if i != skip {
            result.push(mailbox[i]);
            proof {
                assert(remove_prefix_index_spec(mailbox@, skip as int, i as int + 1)
                    == remove_prefix_index_spec(mailbox@, skip as int, i as int).push(mailbox[i as int]));
            }
        } else {
            proof {
                assert(remove_prefix_index_spec(mailbox@, skip as int, i as int + 1)
                    == remove_prefix_index_spec(mailbox@, skip as int, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// r[impl beam-mailbox.selective-receive]
// r[impl beam-mailbox.save-offset]
/// Scan from `save_offset`, remove the first exact-match message, and return
/// the updated mailbox plus next save offset.
pub fn selective_receive(mailbox: &[BeamMessage], patterns: &[BeamMessage], save_offset: usize) -> (result: (Option<BeamMessage>, Vec<BeamMessage>, usize))
    requires save_offset <= mailbox@.len()
    ensures
        result.0.is_some() ==> message_in_patterns_spec(patterns@, result.0.unwrap()),
        result.0.is_some() ==> result.2 == 0,
        result.0.is_none() ==> result.1@ =~= mailbox@,
        result.0.is_none() ==> result.2 == mailbox@.len(),
{
    let mut i: usize = save_offset;
    while i < mailbox.len()
        invariant
            save_offset <= i <= mailbox@.len(),
            forall|j: int| #![trigger mailbox@[j]] save_offset as int <= j < i as int ==> !message_in_patterns_spec(patterns@, mailbox@[j]),
        decreases mailbox@.len() - i,
    {
        let msg = mailbox[i];
        if matches_any(patterns, msg) {
            return (Some(msg), clone_without_index(mailbox, i), 0);
        }
        i = i + 1;
    }
    (None, clone_mailbox(mailbox), mailbox.len())
}

// r[verify beam-mailbox.ordering]
proof fn append_preserves_prefix(mailbox: Seq<BeamMessage>, msg: BeamMessage)
    ensures
        mailbox_append_spec(mailbox, msg).len() == mailbox.len() + 1,
        forall|i: int| 0 <= i < mailbox.len() ==> mailbox_append_spec(mailbox, msg)[i] == mailbox[i],
{}

// r[verify beam-mailbox.ordering]
proof fn same_sender_fifo_on_empty(sender: u64, m1: u64, m2: u64)
    ensures sender_before_spec(seq![], sender, m1, m2)
{}

// r[verify beam-mailbox.selective-receive]
proof fn first_match_is_leftmost(mailbox: Seq<BeamMessage>, patterns: Seq<BeamMessage>, from: int)
    requires 0 <= from <= mailbox.len(), first_match_from_spec(mailbox, patterns, from) >= 0
    ensures
        message_in_patterns_spec(patterns, mailbox[first_match_from_spec(mailbox, patterns, from)]),
        forall|j: int| #![trigger mailbox[j]] from <= j < first_match_from_spec(mailbox, patterns, from) ==> !message_in_patterns_spec(patterns, mailbox[j]),
    decreases mailbox.len() - from
{
    if from < mailbox.len() {
        if message_in_patterns_spec(patterns, mailbox[from]) {
        } else {
            first_match_is_leftmost(mailbox, patterns, from + 1);
        }
    }
}

// r[verify beam-mailbox.selective-receive]
proof fn remove_index_shrinks_len(mailbox: Seq<BeamMessage>, idx: int)
    requires 0 <= idx < mailbox.len()
    ensures mailbox_remove_index_spec(mailbox, idx).len() < mailbox.len()
{}

// r[verify beam-mailbox.save-offset]
proof fn successful_receive_resets_cursor(mailbox: Seq<BeamMessage>, patterns: Seq<BeamMessage>, save_offset: int)
    requires 0 <= save_offset <= mailbox.len(), first_match_from_spec(mailbox, patterns, save_offset) >= 0
    ensures selective_receive_spec(mailbox, patterns, save_offset).2 == 0
{}

// r[verify beam-mailbox.save-offset]
proof fn failed_receive_advances_to_end(mailbox: Seq<BeamMessage>, patterns: Seq<BeamMessage>, save_offset: int)
    requires 0 <= save_offset <= mailbox.len(), first_match_from_spec(mailbox, patterns, save_offset) < 0
    ensures
        selective_receive_spec(mailbox, patterns, save_offset).0.is_none(),
        selective_receive_spec(mailbox, patterns, save_offset).2 == mailbox.len(),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

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

    // r[verify beam-mailbox.ordering]
    #[test]
    fn test_mailbox_fifo_append() {
        let mailbox = vec![data(1, 10)];
        let mailbox = mailbox_append(&mailbox, data(1, 20));
        assert_eq!(mailbox[0].payload, 10);
        assert_eq!(mailbox[1].payload, 20);
    }

    // r[verify beam-mailbox.selective-receive]
    #[test]
    fn test_selective_receive_first_match() {
        let mailbox = vec![data(1, 1), data(2, 2), data(3, 3)];
        let patterns = vec![data(2, 2)];
        let (msg, rest, next) = selective_receive(&mailbox, &patterns, 0);
        assert_eq!(msg.unwrap().payload, 2);
        assert_eq!(rest, vec![data(1, 1), data(3, 3)]);
        assert_eq!(next, 0);
    }

    // r[verify beam-mailbox.selective-receive]
    // r[verify beam-mailbox.save-offset]
    #[test]
    fn test_selective_receive_no_match_keeps_mailbox() {
        let mailbox = vec![data(1, 1), data(2, 2)];
        let patterns = vec![data(9, 9)];
        let (msg, rest, next) = selective_receive(&mailbox, &patterns, 1);
        assert!(msg.is_none());
        assert_eq!(rest, mailbox);
        assert_eq!(next, 2);
    }

    proptest! {
        // r[verify beam-mailbox.ordering]
        #[test]
        fn prop_append_keeps_previous_messages(
            msgs in proptest::collection::vec((any::<u64>(), any::<u64>()), 0..12),
            sender: u64,
            payload: u64,
        ) {
            let mailbox: Vec<BeamMessage> = msgs.into_iter().map(|(s, p)| data(s, p)).collect();
            let next = mailbox_append(&mailbox, data(sender, payload));
            prop_assert_eq!(&next[..mailbox.len()], &mailbox[..]);
            prop_assert_eq!(next.last().unwrap().payload, payload);
        }

        // r[verify beam-mailbox.selective-receive]
        #[test]
        fn prop_selective_receive_removes_one_match(
            payloads in proptest::collection::vec(any::<u64>(), 1..12),
            target_idx in 0usize..11,
        ) {
            let mailbox: Vec<BeamMessage> = payloads.iter().enumerate().map(|(i, p)| data(i as u64, *p)).collect();
            let idx = target_idx % mailbox.len();
            let patterns = vec![mailbox[idx]];
            let (msg, rest, next) = selective_receive(&mailbox, &patterns, 0);
            prop_assert_eq!(msg, Some(mailbox[idx]));
            prop_assert_eq!(rest.len() + 1, mailbox.len());
            prop_assert_eq!(next, 0);
        }
    }
}
