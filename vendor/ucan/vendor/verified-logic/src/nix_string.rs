//! Verified Nix string operations.
//!
//! Models `builtins.stringLength`, `builtins.substring`, and
//! `builtins.replaceStrings`. Uses `Seq<u8>` / `Vec<u8>` for strings.

// Verus requires &Vec for @ view, not &[T]
#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

// --- string_length ---

// r[depends nix-string.string-length]
/// Spec: length of a byte string.
pub open spec fn string_length_spec(s: Seq<u8>) -> nat {
    s.len()
}

// r[impl nix-string.string-length]
/// Returns the length of the string.
pub fn string_length(s: &Vec<u8>) -> (result: usize)
    ensures result as nat == string_length_spec(s@)
{
    s.len()
}

// --- substring ---

// r[depends nix-string.substring]
/// Spec: extract a substring. Clamps to string bounds (Nix semantics).
/// `start` is the starting index, `len` is the maximum length.
pub open spec fn substring_spec(s: Seq<u8>, start: nat, len: nat) -> Seq<u8> {
    let actual_start: nat = if start > s.len() { s.len() } else { start };
    let remaining: nat = (s.len() - actual_start) as nat;
    let actual_len: nat = if len > remaining { remaining } else { len };
    s.subrange(actual_start as int, (actual_start + actual_len) as int)
}

// r[impl nix-string.substring]
/// Extract a substring with Nix's clamping semantics.
pub fn substring(s: &Vec<u8>, start: usize, len: usize) -> (result: Vec<u8>)
    ensures result@ == substring_spec(s@, start as nat, len as nat)
{
    let actual_start: usize = if start > s.len() { s.len() } else { start };
    let remaining: usize = s.len() - actual_start;
    let actual_len: usize = if len > remaining { remaining } else { len };
    let mut result: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < actual_len
        invariant
            i <= actual_len,
            actual_start + actual_len <= s.len(),
            result.len() == i,
            forall|j: int| 0 <= j < i as int ==> result@[j] == s@[actual_start as int + j],
        decreases actual_len - i,
    {
        result.push(s[actual_start + i]);
        i += 1;
    }
    assert(result@ =~= substring_spec(s@, start as nat, len as nat));
    result
}

// --- replace_strings ---

// r[depends nix-string.replace-strings]
/// Spec: left-to-right non-overlapping string replacement.
/// `from` and `to` are parallel arrays of patterns and replacements.
///
/// Scans left-to-right, trying each pattern at each position.
/// First match wins. Non-overlapping: advances past the match.
/// Empty patterns insert replacement before each byte and at end.
pub open spec fn replace_strings_spec(
    s: Seq<u8>, from: Seq<Seq<u8>>, to: Seq<Seq<u8>>,
) -> Seq<u8>
    recommends from.len() == to.len()
{
    replace_at_spec(s, from, to, 0)
}

/// First pattern index matching at position `pos`. Returns -1 if none.
pub open spec fn first_match_idx(
    s: Seq<u8>, from: Seq<Seq<u8>>, pos: int, pidx: int,
) -> int
    decreases from.len() - pidx,
{
    if pidx >= from.len() {
        -1int
    } else if from[pidx].len() == 0 {
        pidx
    } else if pos + from[pidx].len() <= s.len()
        && forall|i: int| #![trigger from[pidx][i]]
            0 <= i < from[pidx].len() ==> s[pos + i] == from[pidx][i]
    {
        pidx
    } else {
        first_match_idx(s, from, pos, pidx + 1)
    }
}

/// Emit the trailing replacement for the first empty pattern, if any.
pub open spec fn trailing_empty(
    from: Seq<Seq<u8>>, to: Seq<Seq<u8>>, pidx: int,
) -> Seq<u8>
    decreases from.len() - pidx,
{
    if pidx >= from.len() {
        Seq::<u8>::empty()
    } else if from[pidx].len() == 0 {
        to[pidx]
    } else {
        trailing_empty(from, to, pidx + 1)
    }
}

/// Proof: when first_match_idx returns a non-empty match, the match fits in bounds.
proof fn first_match_bounded(
    s: Seq<u8>, from: Seq<Seq<u8>>, pos: int, pidx: int,
)
    requires 0 <= pidx, pidx <= from.len(), 0 <= pos, pos <= s.len()
    ensures ({
        let idx = first_match_idx(s, from, pos, pidx);
        idx >= 0 && idx < from.len() && from[idx].len() > 0
            ==> pos + from[idx].len() <= s.len()
    })
    decreases from.len() - pidx,
{
    if pidx < from.len() && from[pidx].len() != 0 {
        if !(pos + from[pidx].len() <= s.len()
            && forall|i: int| #![trigger from[pidx][i]]
                0 <= i < from[pidx].len() ==> s[pos + i] == from[pidx][i])
        {
            first_match_bounded(s, from, pos, pidx + 1);
        }
    }
}

/// Core replacement from position `pos`.
pub open spec fn replace_at_spec(
    s: Seq<u8>, from: Seq<Seq<u8>>, to: Seq<Seq<u8>>, pos: int,
) -> Seq<u8>
    recommends from.len() == to.len(), 0 <= pos
    decreases s.len() - pos + 1,
    when from.len() == to.len() && 0 <= pos && pos <= s.len() + 1
{
    if pos > s.len() {
        Seq::<u8>::empty()
    } else if pos == s.len() {
        trailing_empty(from, to, 0)
    } else {
        let idx = first_match_idx(s, from, pos, 0);
        if idx >= 0 && (idx as int) < from.len() && from[idx].len() == 0 {
            // Empty match: emit replacement + byte, advance 1.
            to[idx] + seq![s[pos]] + replace_at_spec(s, from, to, pos + 1)
        } else if idx >= 0 && (idx as int) < from.len()
            && pos + (from[idx].len() as int) <= s.len() {
            // Non-empty match: emit replacement, advance past match.
            // Bound is always satisfied (first_match_idx guarantees it for non-empty
            // patterns), but making it explicit lets the termination checker see
            // that the decrease metric strictly decreases.
            to[idx] + replace_at_spec(s, from, to, pos + from[idx].len() as int)
        } else {
            // No match: emit byte, advance 1.
            seq![s[pos]] + replace_at_spec(s, from, to, pos + 1)
        }
    }
}

// ---- Exec helpers for replace_strings ----

/// Exec: append all bytes from src to dst.
fn append_vec(dst: &mut Vec<u8>, src: &Vec<u8>)
    ensures dst@ =~= old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ =~= old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i = i + 1;
    }
    assert(src@.subrange(0, src@.len() as int) =~= src@);
}

/// Exec: does `pat` match at position `pos` in `s`?
fn pattern_at(s: &Vec<u8>, pat: &Vec<u8>, pos: usize) -> (result: bool)
    requires pos as int + pat@.len() <= s@.len()
    ensures result == (forall|i: int| #![trigger pat@[i]]
        0 <= i < pat@.len() ==> s@[pos as int + i] == pat@[i])
{
    let mut i: usize = 0;
    while i < pat.len()
        invariant
            i <= pat.len(),
            pos + pat.len() <= s.len(),
            forall|j: int| #![trigger pat@[j]] 0 <= j < i as int
                ==> s@[pos as int + j] == pat@[j],
        decreases pat.len() - i,
    {
        if s[pos + i] != pat[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Exec: find first matching pattern at `pos`. Returns `None` if no match.
fn first_match_exec(
    s: &Vec<u8>, from: &Vec<Vec<u8>>, pos: usize,
    fs: Ghost<Seq<Seq<u8>>>,
) -> (result: Option<usize>)
    requires
        pos as int <= s@.len(),
        from@.len() == fs@.len(),
        forall|i: int| #![trigger fs@[i]] 0 <= i < from@.len()
            ==> from@[i]@ =~= fs@[i],
    ensures
        result.is_some() ==> first_match_idx(s@, fs@, pos as int, 0)
            == result.unwrap() as int,
        result.is_some() ==> (result.unwrap() as int) < from@.len(),
        result.is_none() ==> first_match_idx(s@, fs@, pos as int, 0) < 0,
{
    let mut pidx: usize = 0;
    while pidx < from.len()
        invariant
            pidx <= from.len(),
            from@.len() == fs@.len(),
            forall|i: int| #![trigger fs@[i]] 0 <= i < from@.len()
                ==> from@[i]@ =~= fs@[i],
            pos as int <= s@.len(),
            first_match_idx(s@, fs@, pos as int, 0)
                == first_match_idx(s@, fs@, pos as int, pidx as int),
        decreases from.len() - pidx,
    {
        let pat: &Vec<u8> = &from[pidx];
        if pat.len() == 0 {
            return Some(pidx);
        }
        if s.len() - pos >= pat.len() {
            if pattern_at(s, pat, pos) {
                return Some(pidx);
            }
        }
        pidx = pidx + 1;
    }
    None
}

/// Exec: append the trailing empty-pattern replacement to result.
fn trailing_empty_append(
    from: &Vec<Vec<u8>>, to: &Vec<Vec<u8>>, result: &mut Vec<u8>,
    fs: Ghost<Seq<Seq<u8>>>, ts: Ghost<Seq<Seq<u8>>>,
)
    requires
        from@.len() == to@.len(),
        from@.len() == fs@.len(),
        to@.len() == ts@.len(),
        forall|i: int| #![trigger fs@[i]] 0 <= i < from@.len()
            ==> from@[i]@ =~= fs@[i],
        forall|i: int| #![trigger ts@[i]] 0 <= i < to@.len()
            ==> to@[i]@ =~= ts@[i],
    ensures result@ =~= old(result)@ + trailing_empty(fs@, ts@, 0)
{
    let mut pidx: usize = 0;
    while pidx < from.len()
        invariant
            pidx <= from.len(),
            from@.len() == to@.len(),
            from@.len() == fs@.len(),
            to@.len() == ts@.len(),
            forall|i: int| #![trigger fs@[i]] 0 <= i < from@.len()
                ==> from@[i]@ =~= fs@[i],
            forall|i: int| #![trigger ts@[i]] 0 <= i < to@.len()
                ==> to@[i]@ =~= ts@[i],
            result@ =~= old(result)@,
            trailing_empty(fs@, ts@, 0) =~= trailing_empty(fs@, ts@, pidx as int),
        decreases from.len() - pidx,
    {
        if from[pidx].len() == 0 {
            // trailing_empty(fs, ts, pidx) == ts[pidx] since fs[pidx] is empty
            assert(from@[pidx as int]@ =~= fs@[pidx as int]);
            assert(fs@[pidx as int].len() == 0);
            assert(trailing_empty(fs@, ts@, pidx as int) =~= ts@[pidx as int]);
            assert(to@[pidx as int]@ =~= ts@[pidx as int]);
            assert(trailing_empty(fs@, ts@, 0) =~= to@[pidx as int]@);
            append_vec(result, &to[pidx]);
            return;
        }
        pidx = pidx + 1;
    }
    // No empty pattern: trailing_empty returns empty.
}

// r[impl nix-string.replace-strings]
/// Replace all occurrences of `from[i]` with `to[i]`, left-to-right,
/// non-overlapping. First match wins when patterns overlap at a position.
pub fn replace_strings(
    s: &Vec<u8>, from: &Vec<Vec<u8>>, to: &Vec<Vec<u8>>,
) -> (result: Vec<u8>)
    requires from.len() == to.len()
    ensures result@ =~= replace_strings_spec(s@,
        from@.map_values(|v: Vec<u8>| v@),
        to@.map_values(|v: Vec<u8>| v@))
{
    let fs: Ghost<Seq<Seq<u8>>> = Ghost(from@.map_values(|v: Vec<u8>| v@));
    let ts: Ghost<Seq<Seq<u8>>> = Ghost(to@.map_values(|v: Vec<u8>| v@));
    let ghost full = replace_at_spec(s@, fs@, ts@, 0);

    let mut result: Vec<u8> = Vec::new();
    let mut pos: usize = 0;

    while pos < s.len()
        invariant
            pos <= s.len(),
            from@.len() == to@.len(),
            fs@.len() == from@.len(),
            ts@.len() == to@.len(),
            forall|i: int| #![trigger fs@[i]] 0 <= i < from@.len()
                ==> from@[i]@ =~= fs@[i],
            forall|i: int| #![trigger ts@[i]] 0 <= i < to@.len()
                ==> to@[i]@ =~= ts@[i],
            result@ + replace_at_spec(s@, fs@, ts@, pos as int) =~= full,
        decreases s.len() - pos,
    {
        let ghost old_result = result@;
        let matched: Option<usize> = first_match_exec(s, from, pos, fs);

        match matched {
            Some(pidx) => {
                let pat: &Vec<u8> = &from[pidx];
                if pat.len() == 0 {
                    // Empty match: append replacement + byte, advance 1.
                    append_vec(&mut result, &to[pidx]);
                    result.push(s[pos]);
                    pos = pos + 1;
                } else {
                    // Non-empty match: append replacement, advance by pat.len().
                    proof { first_match_bounded(s@, fs@, pos as int, 0); }
                    append_vec(&mut result, &to[pidx]);
                    pos = pos + pat.len();
                }
            }
            None => {
                // No match: append byte, advance 1.
                result.push(s[pos]);
                pos = pos + 1;
            }
        }
    }

    // Handle trailing empty pattern.
    trailing_empty_append(from, to, &mut result, fs, ts);
    result
}

// --- Proofs ---

// r[verify nix-string.string-length]
/// Length of empty string is 0.
proof fn string_length_empty()
    ensures string_length_spec(Seq::<u8>::empty()) == 0
{}

// r[verify nix-string.substring]
/// Substring of empty string is empty.
proof fn substring_empty(start: nat, len: nat)
    ensures substring_spec(Seq::<u8>::empty(), start, len) =~= Seq::<u8>::empty()
{}

// r[verify nix-string.substring]
/// Substring with start=0, len >= s.len() returns the whole string.
proof fn substring_full(s: Seq<u8>)
    ensures substring_spec(s, 0, s.len()) =~= s
{}

// r[verify nix-string.substring]
/// Substring length is at most the requested length.
proof fn substring_length_bounded(s: Seq<u8>, start: nat, len: nat)
    ensures substring_spec(s, start, len).len() <= len
{}

} // verus!

#[cfg(test)]
#[allow(clippy::naive_bytecount)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn v(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    // r[verify nix-string.string-length]
    #[test]
    fn test_string_length() {
        assert_eq!(string_length(&v("")), 0);
        assert_eq!(string_length(&v("hello")), 5);
    }

    // r[verify nix-string.substring]
    #[test]
    fn test_substring() {
        assert_eq!(substring(&v("hello"), 1, 3), v("ell"));
        assert_eq!(substring(&v("hello"), 0, 100), v("hello")); // clamp
        assert_eq!(substring(&v("hello"), 10, 5), v("")); // past end
        assert_eq!(substring(&v("hello"), 0, 0), v("")); // zero length
    }

    // r[verify nix-string.replace-strings]
    #[test]
    fn test_replace_strings() {
        // Simple replacement
        let result = replace_strings(&v("hello world"), &vec![v("world")], &vec![v("nix")]);
        assert_eq!(result, v("hello nix"));

        // Multiple patterns
        let result = replace_strings(&v("aabbcc"), &vec![v("aa"), v("bb")], &vec![v("x"), v("y")]);
        assert_eq!(result, v("xycc"));

        // No match
        let result = replace_strings(&v("hello"), &vec![v("xyz")], &vec![v("abc")]);
        assert_eq!(result, v("hello"));
    }

    proptest! {
        // r[verify nix-string.string-length]
        #[test]
        fn prop_string_length(s in ".{0,50}") {
            let v = s.into_bytes();
            prop_assert_eq!(string_length(&v), v.len());
        }

        // r[verify nix-string.substring]
        #[test]
        fn prop_substring_bounded(
            s in ".{0,20}",
            start in 0usize..30,
            len in 0usize..30,
        ) {
            let v = s.into_bytes();
            let result = substring(&v, start, len);
            prop_assert!(result.len() <= len);
            prop_assert!(result.len() <= v.len());
        }

        // r[verify nix-string.substring]
        #[test]
        fn prop_substring_full(s in ".{0,20}") {
            let v = s.into_bytes();
            prop_assert_eq!(substring(&v, 0, v.len()), v);
        }

        // r[verify nix-string.replace-strings]
        #[test]
        fn prop_replace_no_match(
            s in "[a-z]{1,20}",
        ) {
            let v = s.into_bytes();
            // Pattern that can't match lowercase
            let result = replace_strings(&v, &vec![b"999".to_vec()], &vec![b"X".to_vec()]);
            prop_assert_eq!(result, v);
        }

        // r[verify nix-string.replace-strings]
        /// Identity replacement: replacing pattern with itself is a no-op.
        #[test]
        fn prop_replace_identity(
            s in "[a-z]{1,30}",
            pat in "[a-z]{1,5}",
        ) {
            let sv = s.into_bytes();
            let pv = pat.into_bytes();
            let result = replace_strings(&sv, &vec![pv.clone()], &vec![pv]);
            prop_assert_eq!(result, sv);
        }

        // r[verify nix-string.replace-strings]
        /// Empty input produces empty output (no patterns match).
        #[test]
        fn prop_replace_empty_input(
            pat in "[a-z]{1,5}",
            repl in "[a-z]{0,5}",
        ) {
            let result = replace_strings(
                &vec![], &vec![pat.into_bytes()], &vec![repl.into_bytes()],
            );
            prop_assert!(result.is_empty());
        }

        // r[verify nix-string.replace-strings]
        /// No patterns means output equals input (no substitutions possible).
        #[test]
        fn prop_replace_no_patterns(s in ".{0,30}") {
            let sv = s.into_bytes();
            let result = replace_strings(&sv, &vec![], &vec![]);
            prop_assert_eq!(result, sv);
        }

        // r[verify nix-string.replace-strings]
        /// Single-char replacement: every occurrence is replaced.
        #[test]
        fn prop_replace_single_char_count(
            s in "[ab]{1,30}",
            repl in "[cd]{1,3}",
        ) {
            let sv = s.into_bytes();
            let orig_count = sv.iter().filter(|&&b| b == b'a').count();
            let rv = repl.into_bytes();
            let result = replace_strings(&sv, &vec![b"a".to_vec()], &vec![rv.clone()]);
            // After replacement, no 'a' bytes should remain
            let remaining = result.iter().filter(|&&b| b == b'a').count();
            prop_assert_eq!(remaining, 0,
                "expected no 'a' bytes after replacement, found {}", remaining);
            // Result length should be: original - count_a + count_a * repl.len()
            let expected_len = sv.len() - orig_count + orig_count * rv.len();
            prop_assert_eq!(result.len(), expected_len);
        }

        // r[verify nix-string.replace-strings]
        /// Replace-with-empty deletes all occurrences.
        #[test]
        fn prop_replace_delete(
            s in "[ab]{1,30}",
        ) {
            let sv = s.into_bytes();
            let result = replace_strings(&sv, &vec![b"a".to_vec()], &vec![vec![]]);
            // No 'a' should remain
            prop_assert!(!result.contains(&b'a'));
            // Only 'b' bytes should remain
            let b_count = sv.iter().filter(|&&b| b == b'b').count();
            prop_assert_eq!(result.len(), b_count);
        }

        // r[verify nix-string.replace-strings]
        /// First pattern wins: when both patterns could match, the earlier
        /// one in the from array takes priority.
        #[test]
        fn prop_replace_first_match_wins(
            prefix in "[c]{0,5}",
            suffix in "[c]{0,5}",
        ) {
            // Input: prefix + "ab" + suffix
            // Patterns: ["ab", "a"] → ["X", "Y"]
            // "ab" should match as "X", not "a" as "Y" + literal "b"
            let mut input = prefix.into_bytes();
            input.extend_from_slice(b"ab");
            input.extend_from_slice(&suffix.into_bytes());
            let result = replace_strings(
                &input,
                &vec![b"ab".to_vec(), b"a".to_vec()],
                &vec![b"X".to_vec(), b"Y".to_vec()],
            );
            // The "ab" should become "X"
            prop_assert!(result.windows(1).any(|w| w == b"X"),
                "expected X in result {:?}", String::from_utf8_lossy(&result));
            // No "Y" should appear (the "a" pattern shouldn't fire for the "ab" position)
            prop_assert!(!result.contains(&b'Y'),
                "unexpected Y in result {:?}", String::from_utf8_lossy(&result));
        }

        // r[verify nix-string.replace-strings]
        /// Non-overlapping: replacements don't re-scan replacement text.
        /// If we replace "a" with "ba", the output "ba" shouldn't trigger
        /// another replacement of the "a" inside "ba".
        #[test]
        fn prop_replace_non_overlapping(count in 1usize..=10) {
            let input: Vec<u8> = vec![b'a'; count];
            let result = replace_strings(
                &input,
                &vec![b"a".to_vec()],
                &vec![b"ba".to_vec()],
            );
            // Each "a" becomes "ba", so result should be "ba" repeated count times
            prop_assert_eq!(result.len(), count * 2);
            for i in 0..count {
                prop_assert_eq!(result[i * 2], b'b');
                prop_assert_eq!(result[i * 2 + 1], b'a');
            }
        }

        // r[verify nix-string.replace-strings]
        /// Empty pattern behavior (Nix semantics): empty pattern matches at every
        /// position, inserting the replacement before each character and after the last.
        #[test]
        fn prop_replace_empty_pattern(s in "[a-z]{1,10}") {
            let sv = s.into_bytes();
            let result = replace_strings(
                &sv,
                &vec![vec![]],        // empty pattern
                &vec![b"|".to_vec()],  // insert separator
            );
            // Empty pattern inserts replacement at each position + trailing
            // For input of length n: n insertions (before each char) + 1 trailing + n original chars
            let expected_len = sv.len() + sv.len() + 1;
            prop_assert_eq!(result.len(), expected_len,
                "input {:?} (len {}), result {:?} (len {}), expected len {}",
                String::from_utf8_lossy(&sv), sv.len(),
                String::from_utf8_lossy(&result), result.len(), expected_len);
        }

        // r[verify nix-string.replace-strings]
        /// Multiple parallel patterns: disjoint single-char swaps.
        /// Swapping 'a'↔'b' should be its own inverse.
        #[test]
        fn prop_replace_swap_involution(s in "[ab]{1,20}") {
            let sv = s.into_bytes();
            let from = vec![b"a".to_vec(), b"b".to_vec()];
            let to = vec![b"b".to_vec(), b"a".to_vec()];
            let swapped = replace_strings(&sv, &from, &to);
            let double_swapped = replace_strings(&swapped, &from, &to);
            let msg = format!(
                "swap should be involution: {:?} → {:?} → {:?}",
                String::from_utf8_lossy(&sv),
                String::from_utf8_lossy(&swapped),
                String::from_utf8_lossy(&double_swapped),
            );
            prop_assert_eq!(double_swapped, sv, "{}", msg);
        }

        // r[verify nix-string.replace-strings]
        /// Output length is predictable for fixed-length replacements.
        /// Replacing all "x" with "yy" in a string of x's and z's.
        #[test]
        fn prop_replace_length_predictable(s in "[xz]{1,20}") {
            let sv = s.into_bytes();
            let x_count = sv.iter().filter(|&&b| b == b'x').count();
            let z_count = sv.iter().filter(|&&b| b == b'z').count();
            let result = replace_strings(
                &sv,
                &vec![b"x".to_vec()],
                &vec![b"yy".to_vec()],
            );
            // Each 'x' becomes 'yy' (2 bytes), each 'z' stays (1 byte)
            prop_assert_eq!(result.len(), x_count * 2 + z_count);
        }

        // r[verify nix-string.replace-strings]
        /// Matches Nix builtins.replaceStrings reference for multi-pattern case.
        /// Uses a reference implementation to cross-check.
        #[test]
        fn prop_replace_matches_reference(
            s in "[a-d]{1,15}",
            p1 in "[a-b]{1,3}",
            r1 in "[e-f]{0,3}",
            p2 in "[c-d]{1,3}",
            r2 in "[g-h]{0,3}",
        ) {
            let sv = s.into_bytes();
            let from = vec![p1.into_bytes(), p2.into_bytes()];
            let to = vec![r1.into_bytes(), r2.into_bytes()];

            let result = replace_strings(&sv, &from, &to);

            // Reference implementation: left-to-right, first-match, non-overlapping
            let reference = reference_replace(&sv, &from, &to);
            prop_assert_eq!(result, reference,
                "mismatch for input {:?}",
                String::from_utf8_lossy(&sv));
        }
    }

    /// Reference implementation of Nix replaceStrings for cross-checking.
    fn reference_replace(s: &[u8], from: &[Vec<u8>], to: &[Vec<u8>]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut pos = 0;
        'outer: while pos < s.len() {
            for i in 0..from.len() {
                if from[i].is_empty() {
                    result.extend_from_slice(&to[i]);
                    result.push(s[pos]);
                    pos += 1;
                    continue 'outer;
                }
                if pos + from[i].len() <= s.len() && s[pos..pos + from[i].len()] == *from[i] {
                    result.extend_from_slice(&to[i]);
                    pos += from[i].len();
                    continue 'outer;
                }
            }
            result.push(s[pos]);
            pos += 1;
        }
        // Trailing empty-pattern match
        for i in 0..from.len() {
            if from[i].is_empty() {
                result.extend_from_slice(&to[i]);
                break;
            }
        }
        result
    }
}
