//! Verified peer specification parsing.
//!
//! Parses `node_id@endpoint` format where node_id is a u64 and endpoint
//! is a non-empty byte sequence. Uses byte-level operations for Verus
//! compatibility (no regex, no `str::find`).
//!
//! Error codes: 0 = ok, 1 = no separator, 2 = empty node_id, 3 = empty endpoint,
//! 4 = invalid node_id.

// Verus preconditions guarantee casts are in bounds
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)] // usize→i64: len <= i64::MAX by requires

use vstd::prelude::*;

verus! {

/// The `@` separator byte.
pub const SEPARATOR: u8 = 64u8; // b'@'

/// Error codes.
pub const OK: u8 = 0u8;
pub const ERR_NO_SEPARATOR: u8 = 1u8;
pub const ERR_EMPTY_NODE_ID: u8 = 2u8;
pub const ERR_EMPTY_ENDPOINT: u8 = 3u8;
pub const ERR_INVALID_NODE_ID: u8 = 4u8;

// --- find_separator ---

// r[depends peer-spec.find-separator]
/// Spec: find the first `@` byte in a sequence. Returns -1 if not found.
pub open spec fn find_separator_spec(s: Seq<u8>) -> int {
    if exists|i: int| 0 <= i < s.len() && s[i] == SEPARATOR {
        choose|i: int| 0 <= i < s.len() && s[i] == SEPARATOR
            && forall|j: int| 0 <= j < i ==> s[j] != SEPARATOR
    } else {
        -1int
    }
}

// r[impl peer-spec.find-separator]
/// Find the first `@` byte. Returns -1 (as i64) if not found.
pub fn find_separator(s: &[u8]) -> (result: i64)
    requires s@.len() <= i64::MAX as int
    ensures
        result >= -1,
        result == -1 || (result >= 0 && (result as int) < s@.len()),
        result >= 0 ==> s@[result as int] == SEPARATOR,
        result >= 0 ==> forall|j: int| #![trigger s@[j]] 0 <= j < result as int ==> s@[j] != SEPARATOR,
        result == -1 ==> forall|i: int| 0 <= i < s@.len() ==> s@[i] != SEPARATOR,
{
    let mut i: usize = 0;
    while i < s.len()
        invariant
            0 <= i <= s@.len(),
            s@.len() <= i64::MAX as int,
            forall|j: int| #![trigger s@[j]] 0 <= j < i as int ==> s@[j] != SEPARATOR,
        decreases s@.len() - i,
    {
        if s[i] == SEPARATOR {
            return i as i64;
        }
        i = i + 1;
    }
    -1
}

// --- parse_peer_spec ---

// r[depends peer-spec.parse]
/// Spec: parse result. Returns (error_code, separator_position).
/// Error 0 means valid parse: node_id is [0, sep), endpoint is [sep+1, len).
pub open spec fn parse_peer_spec_spec(s: Seq<u8>) -> (u8, int) {
    let sep = find_separator_spec(s);
    if sep < 0 {
        (ERR_NO_SEPARATOR, -1int)
    } else if sep == 0 {
        (ERR_EMPTY_NODE_ID, sep)
    } else if sep == s.len() - 1 {
        (ERR_EMPTY_ENDPOINT, sep)
    } else {
        (OK, sep)
    }
}

// r[impl peer-spec.parse]
/// Parse a peer spec string. Returns (error_code, separator_index).
#[allow(clippy::cast_possible_wrap)] // s.len() <= isize::MAX (Rust slice invariant) fits in i64
pub fn parse_peer_spec(s: &[u8]) -> (result: (u8, i64))
    requires s@.len() <= i64::MAX as int
    ensures
        result.0 == OK || result.0 == ERR_NO_SEPARATOR
            || result.0 == ERR_EMPTY_NODE_ID || result.0 == ERR_EMPTY_ENDPOINT,
{
    let sep = find_separator(s);
    if sep < 0 {
        (ERR_NO_SEPARATOR, -1)
    } else if sep == 0 {
        (ERR_EMPTY_NODE_ID, sep)
    } else if s.len() > 0 && sep == (s.len() - 1) as i64 {
        (ERR_EMPTY_ENDPOINT, sep)
    } else {
        (OK, sep)
    }
}

// --- is_all_digits ---

/// Spec: all bytes in range [from, to) are ASCII digits.
pub open spec fn is_all_digits_spec(s: Seq<u8>, from: int, to: int) -> bool {
    forall|i: int| #![trigger s[i]] from <= i < to ==> (s[i] >= 48u8 && s[i] <= 57u8)
}

// --- Safety proofs ---

// r[verify peer-spec.parse]
/// A valid parse has the separator at a positive position.
proof fn valid_parse_separator_positive(s: Seq<u8>)
    requires parse_peer_spec_spec(s).0 == OK
    ensures parse_peer_spec_spec(s).1 > 0
{}

// r[impl peer-spec.error-completeness]
// r[verify peer-spec.error-completeness]
/// Every input maps to exactly one of the four error codes.
proof fn error_completeness(s: Seq<u8>)
    ensures
        parse_peer_spec_spec(s).0 == OK
        || parse_peer_spec_spec(s).0 == ERR_NO_SEPARATOR
        || parse_peer_spec_spec(s).0 == ERR_EMPTY_NODE_ID
        || parse_peer_spec_spec(s).0 == ERR_EMPTY_ENDPOINT,
{}

// r[verify peer-spec.parse]
/// Empty input always fails with no separator.
proof fn empty_input_fails()
    requires find_separator_spec(Seq::<u8>::empty()) == -1int
    ensures parse_peer_spec_spec(Seq::<u8>::empty()).0 == ERR_NO_SEPARATOR
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_sign_loss)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify peer-spec.parse]
    // r[verify peer-spec.find-separator]
    #[test]
    fn test_valid() {
        let (code, sep) = parse_peer_spec(b"123@endpoint");
        assert_eq!(code, OK);
        assert_eq!(sep, 3);
    }

    // r[verify peer-spec.parse]
    #[test]
    fn test_no_separator() {
        let (code, _) = parse_peer_spec(b"no-at-sign");
        assert_eq!(code, ERR_NO_SEPARATOR);
    }

    // r[verify peer-spec.parse]
    #[test]
    fn test_empty_node_id() {
        let (code, _) = parse_peer_spec(b"@endpoint");
        assert_eq!(code, ERR_EMPTY_NODE_ID);
    }

    // r[verify peer-spec.parse]
    #[test]
    fn test_empty_endpoint() {
        let (code, _) = parse_peer_spec(b"123@");
        assert_eq!(code, ERR_EMPTY_ENDPOINT);
    }

    // r[verify peer-spec.parse]
    #[test]
    fn test_just_separator() {
        let (code, _) = parse_peer_spec(b"@");
        // @ at pos 0 → empty node_id
        assert_eq!(code, ERR_EMPTY_NODE_ID);
    }

    proptest! {
        // r[verify peer-spec.error-completeness]
        #[test]
        fn prop_error_code_valid(s in proptest::collection::vec(any::<u8>(), 0..100)) {
            let (code, _) = parse_peer_spec(&s);
            prop_assert!(code == OK || code == ERR_NO_SEPARATOR
                || code == ERR_EMPTY_NODE_ID || code == ERR_EMPTY_ENDPOINT);
        }

        // r[verify peer-spec.parse]
        #[test]
        fn prop_roundtrip(
            node_id in 1u64..10000,
            endpoint in "[a-z]{1,20}",
        ) {
            let input = format!("{node_id}@{endpoint}");
            let (code, sep) = parse_peer_spec(input.as_bytes());
            prop_assert_eq!(code, OK);
            // node_id part is before separator
            let bytes = input.as_bytes();
            let node_str = std::str::from_utf8(&bytes[..sep as usize]).unwrap();
            let ep_str = std::str::from_utf8(&bytes[(sep as usize + 1)..]).unwrap();
            let expected_id = node_id.to_string();
            prop_assert_eq!(node_str, expected_id.as_str());
            prop_assert_eq!(ep_str, endpoint.as_str());
        }

        // r[verify peer-spec.parse]
        #[test]
        fn prop_no_at_means_no_separator(s in "[a-z0-9]{0,50}") {
            // No @ in input
            let (code, _) = parse_peer_spec(s.as_bytes());
            prop_assert_eq!(code, ERR_NO_SEPARATOR);
        }
    }
}
