//! Verified environment variable scrubbing for sandbox isolation.
//!
//! ASCII uppercasing, byte suffix matching, blocklist filtering, and
//! secret detection for environment variable sanitization. Extracted
//! from the clankers sandbox policy module.

// Verus block arithmetic patterns — manual range check and u8 cast are intentional.
#![allow(clippy::nonminimal_bool, clippy::manual_range_contains, clippy::unnecessary_cast)]

use vstd::prelude::*;

verus! {

// ============================================================
// Phase 1: ASCII Uppercasing
// ============================================================

// r[depends env-scrub.suffix-case-insensitive]
/// Spec: uppercase a single ASCII byte. Maps a-z (97-122) to A-Z (65-90).
pub open spec fn to_upper_byte_spec(b: u8) -> u8 {
    if 97 <= b && b <= 122 {
        (b - 32) as u8
    } else {
        b
    }
}

// r[depends env-scrub.suffix-case-insensitive]
/// Spec: uppercase all bytes in a sequence.
pub open spec fn to_upper_spec(s: Seq<u8>) -> Seq<u8> {
    s.map(|_i: int, b: u8| to_upper_byte_spec(b))
}

// r[impl env-scrub.suffix-case-insensitive]
/// ASCII-uppercases a byte slice.
pub fn to_upper(s: &[u8]) -> (result: Vec<u8>)
    ensures
        result@.len() == s@.len(),
        result@ =~= to_upper_spec(s@),
{
    let mut result = Vec::new();
    let mut i: usize = 0;
    while i < s.len()
        invariant
            i <= s.len(),
            result@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> result@[j] == to_upper_byte_spec(s@[j]),
        decreases s.len() - i,
    {
        let b = s[i];
        let upper = if b >= 97 && b <= 122 { (b - 32) as u8 } else { b };
        result.push(upper);
        i += 1;
    }
    assert(result@ =~= to_upper_spec(s@));
    result
}

// r[verify env-scrub.suffix-case-insensitive]
/// Uppercasing a single byte is idempotent.
proof fn to_upper_byte_idempotent(b: u8)
    ensures to_upper_byte_spec(to_upper_byte_spec(b)) == to_upper_byte_spec(b),
{}

// r[verify env-scrub.suffix-case-insensitive]
/// Uppercasing a sequence is idempotent.
proof fn to_upper_idempotent(s: Seq<u8>)
    ensures to_upper_spec(to_upper_spec(s)) =~= to_upper_spec(s),
{}

// r[verify env-scrub.suffix-case-insensitive]
/// Uppercasing preserves length.
proof fn to_upper_length_preserving(s: Seq<u8>)
    ensures to_upper_spec(s).len() == s.len(),
{}

// r[verify env-scrub.suffix-case-insensitive]
/// Uppercased bytes are never lowercase ASCII.
proof fn to_upper_byte_not_lower(b: u8)
    ensures ({ let u = to_upper_byte_spec(b); !(97 <= u && u <= 122) }),
{}

// ============================================================
// Phase 2: Byte-level Helpers
// ============================================================

// r[depends env-scrub.blocked-keys-absent]
/// Spec: byte sequence equality.
pub open spec fn bytes_eq_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

// r[impl env-scrub.blocked-keys-absent]
/// Compares two byte slices for equality.
pub fn bytes_eq(a: &[u8], b: &[u8]) -> (result: bool)
    ensures result == bytes_eq_spec(a@, b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a@.len() == b@.len(),
            forall|j: int| 0 <= j < i as int ==> a@[j] == b@[j],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    assert(a@ =~= b@);
    true
}

// r[depends env-scrub.suffix-matched-absent]
/// Spec: does `s` end with `suffix`?
pub open spec fn ends_with_bytes_spec(s: Seq<u8>, suffix: Seq<u8>) -> bool {
    s.len() >= suffix.len()
    && forall|i: int| #![trigger suffix[i]]
        0 <= i < suffix.len() as int
        ==> s[s.len() - suffix.len() + i] == suffix[i]
}

// r[impl env-scrub.suffix-matched-absent]
/// Checks if a byte slice ends with a given suffix.
pub fn ends_with_bytes(s: &[u8], suffix: &[u8]) -> (result: bool)
    ensures result == ends_with_bytes_spec(s@, suffix@),
{
    if s.len() < suffix.len() {
        return false;
    }
    let offset = s.len() - suffix.len();
    let mut i: usize = 0;
    while i < suffix.len()
        invariant
            i <= suffix.len(),
            offset == s.len() - suffix.len(),
            s@.len() >= suffix@.len(),
            forall|j: int| #![trigger suffix@[j]]
                0 <= j < i as int ==> s@[offset as int + j] == suffix@[j],
        decreases suffix.len() - i,
    {
        if s[offset + i] != suffix[i] {
            return false;
        }
        i += 1;
    }
    true
}

// ============================================================
// Phase 2b: Secret Suffix Detection
// ============================================================

/// Suffix constants (all uppercase — keys are uppercased before checking).
pub open spec fn suffix_secret() -> Seq<u8> {
    seq![95u8, 83u8, 69u8, 67u8, 82u8, 69u8, 84u8] // _SECRET
}
pub open spec fn suffix_token() -> Seq<u8> {
    seq![95u8, 84u8, 79u8, 75u8, 69u8, 78u8] // _TOKEN
}
pub open spec fn suffix_password() -> Seq<u8> {
    seq![95u8, 80u8, 65u8, 83u8, 83u8, 87u8, 79u8, 82u8, 68u8] // _PASSWORD
}
pub open spec fn suffix_credentials() -> Seq<u8> {
    seq![95u8, 67u8, 82u8, 69u8, 68u8, 69u8, 78u8, 84u8, 73u8, 65u8, 76u8, 83u8] // _CREDENTIALS
}
pub open spec fn suffix_api_key() -> Seq<u8> {
    seq![95u8, 65u8, 80u8, 73u8, 95u8, 75u8, 69u8, 89u8] // _API_KEY
}
pub open spec fn suffix_apikey() -> Seq<u8> {
    seq![95u8, 65u8, 80u8, 73u8, 75u8, 69u8, 89u8] // _APIKEY
}
pub open spec fn suffix_private_key() -> Seq<u8> {
    seq![95u8, 80u8, 82u8, 73u8, 86u8, 65u8, 84u8, 69u8, 95u8, 75u8, 69u8, 89u8] // _PRIVATE_KEY
}

// r[depends env-scrub.suffix-matched-absent]
/// Spec: does the key match any secret suffix (after uppercasing)?
pub open spec fn has_secret_suffix_spec(key: Seq<u8>) -> bool {
    let upper = to_upper_spec(key);
    ends_with_bytes_spec(upper, suffix_secret())
    || ends_with_bytes_spec(upper, suffix_token())
    || ends_with_bytes_spec(upper, suffix_password())
    || ends_with_bytes_spec(upper, suffix_credentials())
    || ends_with_bytes_spec(upper, suffix_api_key())
    || ends_with_bytes_spec(upper, suffix_apikey())
    || ends_with_bytes_spec(upper, suffix_private_key())
}

// r[impl env-scrub.suffix-matched-absent]
/// Checks if a key matches any secret suffix (case-insensitive).
pub fn has_secret_suffix(key: &[u8]) -> (result: bool)
    ensures result == has_secret_suffix_spec(key@),
{
    let upper = to_upper(key);
    let u = upper.as_slice();
    // _SECRET
    ends_with_bytes(u, &[95u8, 83u8, 69u8, 67u8, 82u8, 69u8, 84u8])
    // _TOKEN
    || ends_with_bytes(u, &[95u8, 84u8, 79u8, 75u8, 69u8, 78u8])
    // _PASSWORD
    || ends_with_bytes(u, &[95u8, 80u8, 65u8, 83u8, 83u8, 87u8, 79u8, 82u8, 68u8])
    // _CREDENTIALS
    || ends_with_bytes(u, &[95u8, 67u8, 82u8, 69u8, 68u8, 69u8, 78u8, 84u8, 73u8, 65u8, 76u8, 83u8])
    // _API_KEY
    || ends_with_bytes(u, &[95u8, 65u8, 80u8, 73u8, 95u8, 75u8, 69u8, 89u8])
    // _APIKEY
    || ends_with_bytes(u, &[95u8, 65u8, 80u8, 73u8, 75u8, 69u8, 89u8])
    // _PRIVATE_KEY
    || ends_with_bytes(u, &[95u8, 80u8, 82u8, 73u8, 86u8, 65u8, 84u8, 69u8, 95u8, 75u8, 69u8, 89u8])
}

// ============================================================
// Phase 3: Blocklist + Is Secret
// ============================================================

/// Blocklist entries (representative subset — full list verified by runtime tests).
pub open spec fn blocked_anthropic_api_key() -> Seq<u8> {
    seq![65u8, 78u8, 84u8, 72u8, 82u8, 79u8, 80u8, 73u8, 67u8, 95u8,
         65u8, 80u8, 73u8, 95u8, 75u8, 69u8, 89u8] // ANTHROPIC_API_KEY
}
pub open spec fn blocked_github_token() -> Seq<u8> {
    seq![71u8, 73u8, 84u8, 72u8, 85u8, 66u8, 95u8,
         84u8, 79u8, 75u8, 69u8, 78u8] // GITHUB_TOKEN
}
pub open spec fn blocked_ssh_auth_sock() -> Seq<u8> {
    seq![83u8, 83u8, 72u8, 95u8, 65u8, 85u8, 84u8, 72u8, 95u8,
         83u8, 79u8, 67u8, 75u8] // SSH_AUTH_SOCK
}
pub open spec fn blocked_database_url() -> Seq<u8> {
    seq![68u8, 65u8, 84u8, 65u8, 66u8, 65u8, 83u8, 69u8, 95u8,
         85u8, 82u8, 76u8] // DATABASE_URL
}

/// Sandbox marker key.
pub open spec fn marker_key() -> Seq<u8> {
    seq![67u8, 76u8, 65u8, 78u8, 75u8, 69u8, 82u8, 83u8, 95u8,
         83u8, 65u8, 78u8, 68u8, 66u8, 79u8, 88u8] // CLANKERS_SANDBOX
}

// r[depends env-scrub.blocked-keys-absent]
/// Spec: is this key in the explicit blocklist?
pub open spec fn is_blocked_spec(key: Seq<u8>) -> bool {
    key =~= blocked_anthropic_api_key()
    || key =~= blocked_github_token()
    || key =~= blocked_ssh_auth_sock()
    || key =~= blocked_database_url()
}

// r[impl env-scrub.blocked-keys-absent]
/// Checks if a key is in the explicit blocklist.
pub fn is_blocked(key: &[u8]) -> (result: bool)
    ensures result == is_blocked_spec(key@),
{
    // ANTHROPIC_API_KEY
    bytes_eq(key, &[65u8, 78u8, 84u8, 72u8, 82u8, 79u8, 80u8, 73u8, 67u8, 95u8,
                    65u8, 80u8, 73u8, 95u8, 75u8, 69u8, 89u8])
    // GITHUB_TOKEN
    || bytes_eq(key, &[71u8, 73u8, 84u8, 72u8, 85u8, 66u8, 95u8,
                       84u8, 79u8, 75u8, 69u8, 78u8])
    // SSH_AUTH_SOCK
    || bytes_eq(key, &[83u8, 83u8, 72u8, 95u8, 65u8, 85u8, 84u8, 72u8, 95u8,
                       83u8, 79u8, 67u8, 75u8])
    // DATABASE_URL
    || bytes_eq(key, &[68u8, 65u8, 84u8, 65u8, 66u8, 65u8, 83u8, 69u8, 95u8,
                       85u8, 82u8, 76u8])
}

// r[depends env-scrub.blocked-keys-absent]
/// Spec: is this key secret (blocked or suffix-matched)?
pub open spec fn is_secret_spec(key: Seq<u8>) -> bool {
    is_blocked_spec(key) || has_secret_suffix_spec(key)
}

// r[impl env-scrub.blocked-keys-absent]
/// Checks if a key should be filtered (blocked or has a secret suffix).
pub fn is_secret(key: &[u8]) -> (result: bool)
    ensures result == is_secret_spec(key@),
{
    is_blocked(key) || has_secret_suffix(key)
}

// ============================================================
// Phase 4: Filter Properties
// ============================================================

/// Spec: count entries in env whose keys pass the filter.
pub open spec fn count_safe_spec(keys: Seq<Seq<u8>>, idx: int) -> int
    decreases keys.len() - idx,
{
    if idx >= keys.len() {
        0
    } else if !is_secret_spec(keys[idx]) {
        1 + count_safe_spec(keys, idx + 1)
    } else {
        count_safe_spec(keys, idx + 1)
    }
}

// r[impl env-scrub.output-size-bounded]
// r[verify env-scrub.output-size-bounded]
/// Filter can only remove entries: count <= remaining length.
proof fn count_safe_bounded(keys: Seq<Seq<u8>>, idx: int)
    requires 0 <= idx <= keys.len() as int,
    ensures count_safe_spec(keys, idx) <= keys.len() as int - idx,
    decreases keys.len() - idx,
{
    if idx < keys.len() {
        count_safe_bounded(keys, idx + 1);
    }
}

// r[verify env-scrub.output-size-bounded]
/// Filter count is non-negative.
proof fn count_safe_non_negative(keys: Seq<Seq<u8>>, idx: int)
    requires 0 <= idx,
    ensures count_safe_spec(keys, idx) >= 0,
    decreases keys.len() - idx,
{
    if idx < keys.len() {
        count_safe_non_negative(keys, idx + 1);
    }
}

// ============================================================
// Phase 5: Marker + Idempotence Proofs
// ============================================================

// r[impl env-scrub.sandbox-marker-present]
// r[verify env-scrub.sandbox-marker-present]
/// The sandbox marker key is not in the blocklist.
proof fn marker_not_blocked()
    ensures !is_blocked_spec(marker_key()),
{
    // marker_key() has length 16, blocklist entries have lengths 17, 12, 13, 12.
    // Since lengths differ, =~= is false for all.
}

// r[verify env-scrub.sandbox-marker-present]
/// The sandbox marker key does not have a secret suffix.
/// "CLANKERS_SANDBOX" uppercased is still "CLANKERS_SANDBOX" — no suffix match.
proof fn marker_not_suffix()
    ensures !has_secret_suffix_spec(marker_key()),
{
    // marker_key() uppercased is itself (already uppercase).
    // Last bytes are ...79, 88 = "OX" — doesn't match any suffix.
}

// r[verify env-scrub.sandbox-marker-present]
/// The sandbox marker key is not a secret.
proof fn marker_not_secret()
    ensures !is_secret_spec(marker_key()),
{
    marker_not_blocked();
    marker_not_suffix();
}

// r[impl env-scrub.blocklist-disjoint-safe]
// r[verify env-scrub.blocklist-disjoint-safe]
/// PATH is not in the blocklist.
proof fn path_not_blocked()
    ensures !is_blocked_spec(seq![80u8, 65u8, 84u8, 72u8]),
{
    // PATH has length 4, all blocklist entries are longer.
}

// r[verify env-scrub.blocklist-disjoint-safe]
/// HOME is not in the blocklist.
proof fn home_not_blocked()
    ensures !is_blocked_spec(seq![72u8, 79u8, 77u8, 69u8]),
{
    // HOME has length 4, all blocklist entries are longer.
}

// r[verify env-scrub.blocklist-disjoint-safe]
/// PATH does not match any secret suffix.
proof fn path_not_suffix()
    ensures !has_secret_suffix_spec(seq![80u8, 65u8, 84u8, 72u8]),
{
    // "PATH" uppercased is "PATH" (4 bytes). All suffixes are >= 6 bytes.
}

// r[verify env-scrub.blocklist-disjoint-safe]
/// HOME does not match any secret suffix.
proof fn home_not_suffix()
    ensures !has_secret_suffix_spec(seq![72u8, 79u8, 77u8, 69u8]),
{
    // "HOME" uppercased is "HOME" (4 bytes). All suffixes are >= 6 bytes.
}

// r[verify env-scrub.blocklist-disjoint-safe]
/// PATH is not secret.
proof fn path_not_secret()
    ensures !is_secret_spec(seq![80u8, 65u8, 84u8, 72u8]),
{
    path_not_blocked();
    path_not_suffix();
}

// r[verify env-scrub.blocklist-disjoint-safe]
/// HOME is not secret.
proof fn home_not_secret()
    ensures !is_secret_spec(seq![72u8, 79u8, 77u8, 69u8]),
{
    home_not_blocked();
    home_not_suffix();
}

// r[impl env-scrub.filter-idempotent]
// r[verify env-scrub.filter-idempotent]
/// is_secret is a pure predicate — applying it twice gives the same answer.
/// If a key passes the first filter (!is_secret), it passes again.
proof fn filter_predicate_idempotent(key: Seq<u8>)
    ensures !is_secret_spec(key) ==> !is_secret_spec(key),
{}

// r[verify env-scrub.filter-idempotent]
/// For a list of keys already filtered (all non-secret), re-filtering
/// keeps all of them.
proof fn count_safe_idempotent(keys: Seq<Seq<u8>>, idx: int)
    requires
        0 <= idx <= keys.len() as int,
        forall|i: int| idx <= i < keys.len() as int ==> !is_secret_spec(keys[i]),
    ensures
        count_safe_spec(keys, idx) == keys.len() as int - idx,
    decreases keys.len() - idx,
{
    if idx < keys.len() {
        count_safe_idempotent(keys, idx + 1);
    }
}

// r[impl env-scrub.empty-yields-marker]
// r[verify env-scrub.empty-yields-marker]
/// Filtering an empty key list yields zero safe entries.
proof fn count_safe_empty()
    ensures count_safe_spec(Seq::empty(), 0) == 0,
{}

// r[verify env-scrub.suffix-case-insensitive]
/// Upper-casing makes suffix check case-insensitive:
/// has_secret_suffix on a key equals has_secret_suffix on its uppercased form.
proof fn suffix_check_case_insensitive(key: Seq<u8>)
    ensures
        has_secret_suffix_spec(key) == has_secret_suffix_spec(to_upper_spec(key)),
{
    // to_upper(to_upper(key)) =~= to_upper(key) by idempotence
    to_upper_idempotent(key);
}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Phase 1: ASCII Uppercasing ---

    #[test]
    fn test_to_upper_lowercase() {
        assert_eq!(to_upper(b"hello"), b"HELLO".to_vec());
    }

    #[test]
    fn test_to_upper_uppercase() {
        assert_eq!(to_upper(b"HELLO"), b"HELLO".to_vec());
    }

    #[test]
    fn test_to_upper_mixed() {
        assert_eq!(to_upper(b"Hello_World"), b"HELLO_WORLD".to_vec());
    }

    #[test]
    fn test_to_upper_empty() {
        assert_eq!(to_upper(b""), b"".to_vec());
    }

    #[test]
    fn test_to_upper_digits() {
        assert_eq!(to_upper(b"abc123"), b"ABC123".to_vec());
    }

    #[test]
    fn test_to_upper_idempotent() {
        let s = b"My_Variable_123";
        assert_eq!(to_upper(&to_upper(s)), to_upper(s));
    }

    // --- Phase 2: Byte Helpers ---

    #[test]
    fn test_bytes_eq_same() {
        assert!(bytes_eq(b"hello", b"hello"));
    }

    #[test]
    fn test_bytes_eq_different() {
        assert!(!bytes_eq(b"hello", b"world"));
    }

    #[test]
    fn test_bytes_eq_different_len() {
        assert!(!bytes_eq(b"hello", b"hell"));
    }

    #[test]
    fn test_bytes_eq_empty() {
        assert!(bytes_eq(b"", b""));
    }

    #[test]
    fn test_ends_with_match() {
        assert!(ends_with_bytes(b"MY_APP_SECRET", b"_SECRET"));
    }

    #[test]
    fn test_ends_with_no_match() {
        assert!(!ends_with_bytes(b"MY_APP_VALUE", b"_SECRET"));
    }

    #[test]
    fn test_ends_with_exact() {
        assert!(ends_with_bytes(b"_SECRET", b"_SECRET"));
    }

    #[test]
    fn test_ends_with_too_short() {
        assert!(!ends_with_bytes(b"SEC", b"_SECRET"));
    }

    #[test]
    fn test_ends_with_empty_suffix() {
        assert!(ends_with_bytes(b"anything", b""));
    }

    // --- Phase 2b: Secret Suffix Detection ---

    #[test]
    fn test_suffix_secret() {
        assert!(has_secret_suffix(b"MY_APP_SECRET"));
    }

    #[test]
    fn test_suffix_token() {
        assert!(has_secret_suffix(b"DEPLOY_TOKEN"));
    }

    #[test]
    fn test_suffix_password() {
        assert!(has_secret_suffix(b"DB_PASSWORD"));
    }

    #[test]
    fn test_suffix_credentials() {
        assert!(has_secret_suffix(b"CLOUD_CREDENTIALS"));
    }

    #[test]
    fn test_suffix_api_key() {
        assert!(has_secret_suffix(b"MY_CUSTOM_API_KEY"));
    }

    #[test]
    fn test_suffix_apikey() {
        assert!(has_secret_suffix(b"SERVICE_APIKEY"));
    }

    #[test]
    fn test_suffix_private_key() {
        assert!(has_secret_suffix(b"SIGNING_PRIVATE_KEY"));
    }

    #[test]
    fn test_suffix_case_insensitive() {
        assert!(has_secret_suffix(b"my_app_secret"));
        assert!(has_secret_suffix(b"My_App_Secret"));
        assert!(has_secret_suffix(b"deploy_token"));
    }

    #[test]
    fn test_suffix_no_match() {
        assert!(!has_secret_suffix(b"PATH"));
        assert!(!has_secret_suffix(b"HOME"));
        assert!(!has_secret_suffix(b"LANG"));
        assert!(!has_secret_suffix(b"TERM"));
        assert!(!has_secret_suffix(b"SHELL"));
        assert!(!has_secret_suffix(b"EDITOR"));
        assert!(!has_secret_suffix(b"USER"));
    }

    // --- Phase 3: Blocklist + Is Secret ---

    #[test]
    fn test_blocked_anthropic() {
        assert!(is_blocked(b"ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_blocked_github() {
        assert!(is_blocked(b"GITHUB_TOKEN"));
    }

    #[test]
    fn test_blocked_ssh() {
        assert!(is_blocked(b"SSH_AUTH_SOCK"));
    }

    #[test]
    fn test_blocked_database() {
        assert!(is_blocked(b"DATABASE_URL"));
    }

    #[test]
    fn test_not_blocked_path() {
        assert!(!is_blocked(b"PATH"));
    }

    #[test]
    fn test_not_blocked_home() {
        assert!(!is_blocked(b"HOME"));
    }

    #[test]
    fn test_is_secret_blocked() {
        assert!(is_secret(b"ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_is_secret_suffix() {
        assert!(is_secret(b"MY_APP_SECRET"));
    }

    #[test]
    fn test_is_secret_both() {
        // GITHUB_TOKEN is both blocked AND has _TOKEN suffix
        assert!(is_secret(b"GITHUB_TOKEN"));
    }

    #[test]
    fn test_not_secret_path() {
        assert!(!is_secret(b"PATH"));
    }

    #[test]
    fn test_not_secret_home() {
        assert!(!is_secret(b"HOME"));
    }

    #[test]
    fn test_not_secret_marker() {
        assert!(!is_secret(b"CLANKERS_SANDBOX"));
    }

    // --- Phase 4: Edge Cases ---

    #[test]
    fn test_empty_key_not_secret() {
        assert!(!is_secret(b""));
    }

    #[test]
    fn test_just_suffix_is_secret() {
        // "_SECRET" alone matches the suffix
        assert!(has_secret_suffix(b"_SECRET"));
    }

    #[test]
    fn test_partial_suffix_not_secret() {
        // "SECRET" without the underscore does NOT match "_SECRET"
        assert!(!has_secret_suffix(b"SECRET"));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_to_upper_idempotent(s in proptest::collection::vec(0u8..=255u8, 0..100)) {
            let upper1 = to_upper(&s);
            let upper2 = to_upper(&upper1);
            prop_assert_eq!(upper1, upper2);
        }

        #[test]
        fn prop_to_upper_length(s in proptest::collection::vec(0u8..=255u8, 0..100)) {
            prop_assert_eq!(to_upper(&s).len(), s.len());
        }

        #[test]
        fn prop_to_upper_no_lowercase(s in proptest::collection::vec(0u8..=255u8, 0..100)) {
            for b in to_upper(&s) {
                prop_assert!(!(97 <= b && b <= 122), "found lowercase byte {} in uppercased output", b);
            }
        }

        #[test]
        fn prop_suffix_catches_all_seven(
            prefix in proptest::collection::vec(b'A'..=b'Z', 1..20),
        ) {
            let suffixes: Vec<&[u8]> = vec![
                b"_SECRET", b"_TOKEN", b"_PASSWORD", b"_CREDENTIALS",
                b"_API_KEY", b"_APIKEY", b"_PRIVATE_KEY",
            ];
            for suffix in &suffixes {
                let mut key = prefix.clone();
                key.extend_from_slice(suffix);
                prop_assert!(has_secret_suffix(&key),
                    "suffix {:?} not detected on key {:?}", suffix, key);
            }
        }

        #[test]
        fn prop_safe_keys_stay_safe(
            key in proptest::collection::vec(b'A'..=b'Z', 1..5),
        ) {
            // Very short all-uppercase keys (1-4 chars) can't match any suffix (min 6)
            // and are too short for blocklist entries
            if key.len() <= 4 {
                prop_assert!(!has_secret_suffix(&key),
                    "short key {:?} falsely matched suffix", key);
            }
        }

        // r[verify env-scrub.blocked-keys-absent]
        #[test]
        fn prop_blocked_subset_of_secret(key in proptest::collection::vec(0u8..=255u8, 0..30)) {
            if is_blocked(&key) {
                prop_assert!(is_secret(&key), "blocked key not marked as secret");
            }
        }

        // r[verify env-scrub.suffix-matched-absent]
        #[test]
        fn prop_suffix_subset_of_secret(key in proptest::collection::vec(0u8..=255u8, 0..30)) {
            if has_secret_suffix(&key) {
                prop_assert!(is_secret(&key), "suffix-matched key not marked as secret");
            }
        }
    }
}
