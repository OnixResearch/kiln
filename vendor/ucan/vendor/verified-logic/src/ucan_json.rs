//! Verified structural admission for decoded UCAN JSON object bytes.
//!
//! This module proves only byte-level JSON object envelope facts over decoded
//! bytes and shell-supplied significant-byte indexes. Runtime JSON parsing,
//! canonicalization, field semantics, base64url decoding, BLAKE3, and Ed25519
//! remain explicit trusted/runtime boundaries in callers.

use vstd::prelude::*;

verus! {

pub const JSON_OBJECT_OPEN: u8 = 123;
pub const JSON_OBJECT_CLOSE: u8 = 125;
pub const JSON_SPACE: u8 = 32;
pub const JSON_LINE_FEED: u8 = 10;
pub const JSON_CARRIAGE_RETURN: u8 = 13;
pub const JSON_TAB: u8 = 9;

/// JSON whitespace bytes accepted around a top-level value by RFC 8259.
// r[impl ucan-compact.json-object-admission]
pub open spec fn is_json_whitespace_byte_spec(byte: u8) -> bool {
    byte == JSON_SPACE || byte == JSON_LINE_FEED || byte == JSON_CARRIAGE_RETURN || byte == JSON_TAB
}

/// All bytes before `end` are JSON whitespace.
// r[impl ucan-compact.json-object-admission]
pub open spec fn prefix_is_json_whitespace_spec(bytes: Seq<u8>, end: int) -> bool {
    0 <= end <= bytes.len() && forall|i: int| 0 <= i < end ==> is_json_whitespace_byte_spec(bytes[i])
}

/// All bytes after `start` are JSON whitespace.
// r[impl ucan-compact.json-object-admission]
pub open spec fn suffix_is_json_whitespace_spec(bytes: Seq<u8>, start: int) -> bool {
    0 <= start < bytes.len() && forall|i: int| start < i < bytes.len() ==> is_json_whitespace_byte_spec(bytes[i])
}

/// Decoded JSON object envelope admission over shell-supplied first/last
/// significant byte indexes.
// r[impl ucan-compact.json-object-admission]
pub open spec fn json_object_admission_spec(bytes: Seq<u8>, first: int, last: int) -> bool {
    0 <= first <= last < bytes.len()
        && prefix_is_json_whitespace_spec(bytes, first)
        && suffix_is_json_whitespace_spec(bytes, last)
        && bytes[first] == JSON_OBJECT_OPEN
        && bytes[last] == JSON_OBJECT_CLOSE
}

// r[impl ucan-compact.json-object-admission]
pub fn is_json_whitespace_byte(byte: u8) -> (result: bool)
    ensures result == is_json_whitespace_byte_spec(byte),
{
    byte == JSON_SPACE || byte == JSON_LINE_FEED || byte == JSON_CARRIAGE_RETURN || byte == JSON_TAB
}

// r[impl ucan-compact.json-object-admission]
pub fn prefix_is_json_whitespace(bytes: &Vec<u8>, end: usize) -> (result: bool)
    ensures result == prefix_is_json_whitespace_spec(bytes@, end as int),
{
    if end > bytes.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < end
        invariant
            0 <= i <= end <= bytes.len(),
            forall|k: int| 0 <= k < i ==> is_json_whitespace_byte_spec(bytes@[k]),
        decreases end - i,
    {
        if !is_json_whitespace_byte(bytes[i]) {
            assert(!is_json_whitespace_byte_spec(bytes@[i as int]));
            assert(!prefix_is_json_whitespace_spec(bytes@, end as int));
            return false;
        }
        i = i + 1;
    }
    assert(prefix_is_json_whitespace_spec(bytes@, end as int));
    true
}

// r[impl ucan-compact.json-object-admission]
pub fn suffix_is_json_whitespace(bytes: &Vec<u8>, start: usize) -> (result: bool)
    ensures result == suffix_is_json_whitespace_spec(bytes@, start as int),
{
    if bytes.len() == 0 || start >= bytes.len() {
        return false;
    }
    let mut i: usize = start + 1;
    while i < bytes.len()
        invariant
            0 <= start < bytes.len(),
            start < i <= bytes.len(),
            forall|k: int| start < k < i ==> is_json_whitespace_byte_spec(bytes@[k]),
        decreases bytes.len() - i,
    {
        if !is_json_whitespace_byte(bytes[i]) {
            assert(!is_json_whitespace_byte_spec(bytes@[i as int]));
            assert(!suffix_is_json_whitespace_spec(bytes@, start as int));
            return false;
        }
        i = i + 1;
    }
    assert(suffix_is_json_whitespace_spec(bytes@, start as int));
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonObjectSignificantIndexes {
    pub first: usize,
    pub last: usize,
}

/// Check decoded JSON object envelope bytes before callers rely on serde-derived semantics.
///
/// This does not parse JSON, verify UTF-8/canonicalization, inspect fields,
/// decode base64url, hash, sign, or verify cryptographic signatures.
// r[impl ucan-compact.json-object-admission]
pub fn json_object_admission_is_valid(bytes: &Vec<u8>, first: usize, last: usize) -> (result: bool)
    ensures result == json_object_admission_spec(bytes@, first as int, last as int),
{
    if bytes.len() == 0 || first > last || last >= bytes.len() {
        return false;
    }
    if !prefix_is_json_whitespace(bytes, first) {
        return false;
    }
    if !suffix_is_json_whitespace(bytes, last) {
        return false;
    }
    if bytes[first] != JSON_OBJECT_OPEN {
        assert(bytes@[first as int] != JSON_OBJECT_OPEN);
        assert(!json_object_admission_spec(bytes@, first as int, last as int));
        return false;
    }
    if bytes[last] != JSON_OBJECT_CLOSE {
        assert(bytes@[last as int] != JSON_OBJECT_CLOSE);
        assert(!json_object_admission_spec(bytes@, first as int, last as int));
        return false;
    }
    true
}

/// Discover significant decoded JSON object envelope indexes before callers rely on serde-derived semantics.
///
/// A successful result carries the first and last non-whitespace byte indexes
/// and proves they satisfy [`json_object_admission_spec`]. This does not parse
/// JSON, verify UTF-8/canonicalization, inspect fields, decode base64url, hash,
/// sign, or verify cryptographic signatures.
// r[impl ucan-compact.json-object-admission]
pub fn json_object_significant_indexes(bytes: &Vec<u8>) -> (result: Option<JsonObjectSignificantIndexes>)
    ensures
        result matches Some(indexes) ==> json_object_admission_spec(bytes@, indexes.first as int, indexes.last as int),
{
    if bytes.len() == 0 {
        return None;
    }

    let mut first: usize = 0;
    while first < bytes.len() && is_json_whitespace_byte(bytes[first])
        invariant
            first <= bytes.len(),
            forall|k: int| 0 <= k < first ==> is_json_whitespace_byte_spec(bytes@[k]),
        decreases bytes.len() - first,
    {
        first = first + 1;
    }

    if first >= bytes.len() {
        return None;
    }

    let mut last: usize = bytes.len() - 1;
    while last > first && is_json_whitespace_byte(bytes[last])
        invariant
            first < bytes.len(),
            first <= last < bytes.len(),
            forall|k: int| last < k < bytes.len() ==> is_json_whitespace_byte_spec(bytes@[k]),
        decreases last - first,
    {
        last = last - 1;
    }

    if json_object_admission_is_valid(bytes, first, last) {
        Some(JsonObjectSignificantIndexes { first, last })
    } else {
        None
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_object_admission_accepts_braced_objects_with_whitespace() {
        let bare = br#"{"alg":"EdDSA","typ":"JWT"}"#.to_vec();
        assert!(json_object_admission_is_valid(&bare, 0, bare.len() - 1));

        let padded = b" \n\t{}\r\n".to_vec();
        assert!(json_object_admission_is_valid(&padded, 3, 4));
    }

    #[test]
    fn json_object_admission_rejects_arrays_scalars_and_empty_bytes() {
        let array = b"[]".to_vec();
        assert!(!json_object_admission_is_valid(&array, 0, 1));

        let scalar = b"true".to_vec();
        assert!(!json_object_admission_is_valid(&scalar, 0, 3));

        let empty = Vec::new();
        assert!(!json_object_admission_is_valid(&empty, 0, 0));
    }

    #[test]
    fn json_object_admission_rejects_inconsistent_shell_indexes() {
        let object = b" {} ".to_vec();
        assert!(!json_object_admission_is_valid(&object, 0, 2));
        assert!(!json_object_admission_is_valid(&object, 1, 3));
        assert!(!json_object_admission_is_valid(&object, 2, 1));
        assert!(!json_object_admission_is_valid(&object, 1, object.len()));
    }

    #[test]
    fn json_object_significant_indexes_discovers_object_bounds() {
        let bare = br#"{"alg":"EdDSA","typ":"JWT"}"#.to_vec();
        assert_eq!(
            json_object_significant_indexes(&bare),
            Some(JsonObjectSignificantIndexes {
                first: 0,
                last: bare.len() - 1,
            })
        );

        let padded = b" \n\t{}\r\n".to_vec();
        assert_eq!(json_object_significant_indexes(&padded), Some(JsonObjectSignificantIndexes { first: 3, last: 4 }));
    }

    #[test]
    fn json_object_significant_indexes_rejects_non_objects_and_whitespace_only() {
        assert_eq!(json_object_significant_indexes(&b" [] ".to_vec()), None);
        assert_eq!(json_object_significant_indexes(&b" true ".to_vec()), None);
        assert_eq!(json_object_significant_indexes(&b" \n\t\r".to_vec()), None);
        assert_eq!(json_object_significant_indexes(&Vec::new()), None);
    }
}
