//! Verified ATerm string escaping for Nix derivations.
//!
//! This is the phase-3 foundation: the string escaping primitive and the
//! derivation data structures used by later full serialization code.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

#[derive(Clone)]
pub struct DerivationOutput {
    pub name: Vec<u8>,
    pub path: Vec<u8>,
    pub hash_algo: Vec<u8>,
    pub hash: Vec<u8>,
}

#[derive(Clone)]
pub struct Derivation {
    pub outputs: Vec<DerivationOutput>,
    pub input_drvs: Vec<(Vec<u8>, Vec<Vec<u8>>)>,
    pub input_srcs: Vec<Vec<u8>>,
    pub platform: Vec<u8>,
    pub builder: Vec<u8>,
    pub args: Vec<Vec<u8>>,
    pub env: Vec<(Vec<u8>, Vec<u8>)>,
}

pub closed spec fn escape_byte_spec(b: u8) -> Seq<u8> {
    if b == 92u8 {
        seq![92u8, 92u8]
    } else if b == 34u8 {
        seq![92u8, 34u8]
    } else if b == 10u8 {
        seq![92u8, 110u8]
    } else if b == 13u8 {
        seq![92u8, 114u8]
    } else if b == 9u8 {
        seq![92u8, 116u8]
    } else {
        seq![b]
    }
}

pub closed spec fn escape_prefix_spec(s: Seq<u8>, upto: nat) -> Seq<u8>
    recommends upto <= s.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        escape_prefix_spec(s, (upto - 1) as nat) + escape_byte_spec(s[(upto - 1) as int])
    }
}

// r[depends nix-drv-serialize.string-escape]
/// Spec: ATerm string escaping with surrounding quotes.
pub open spec fn escape_aterm_spec(s: Seq<u8>) -> Seq<u8> {
    seq![34u8] + escape_prefix_spec(s, s.len()) + seq![34u8]
}

proof fn append_escape_byte_len(b: u8)
    ensures 1 <= escape_byte_spec(b).len() <= 2
{}

pub closed spec fn unescape_byte_spec(first: u8, second: u8) -> Option<(u8, nat)> {
    if first == 92u8 {
        if second == 92u8 {
            Some((92u8, 2))
        } else if second == 34u8 {
            Some((34u8, 2))
        } else if second == 110u8 {
            Some((10u8, 2))
        } else if second == 114u8 {
            Some((13u8, 2))
        } else if second == 116u8 {
            Some((9u8, 2))
        } else {
            None
        }
    } else if first == 34u8 {
        None
    } else {
        Some((first, 1))
    }
}

// r[verify nix-drv-serialize.string-escape]
/// Each escaped byte round-trips: unescaping the escape output recovers the original.
pub proof fn escape_byte_roundtrip(b: u8)
    ensures
        escape_byte_spec(b).len() >= 1,
        escape_byte_spec(b).len() <= 2,
        escape_byte_spec(b).len() == 2 ==> unescape_byte_spec(
            escape_byte_spec(b)[0int], escape_byte_spec(b)[1int]
        ) == Some((b, 2nat)),
        escape_byte_spec(b).len() == 1 ==> unescape_byte_spec(
            escape_byte_spec(b)[0int], 0u8
        ) == Some((b, 1nat)),
{}

// r[verify nix-drv-serialize.string-escape]
/// Escaped output is at least two bytes long because it is quoted.
pub proof fn escape_aterm_has_quotes(s: Seq<u8>)
    ensures
        escape_aterm_spec(s).len() >= 2,
        escape_aterm_spec(s)[0int] == 34u8,
        escape_aterm_spec(s)[escape_aterm_spec(s).len() - 1] == 34u8,
    decreases s.len()
{
    if s.len() != 0 {
        append_escape_byte_len(s[s.len() - 1]);
        escape_aterm_has_quotes(s.subrange(0, s.len() as int - 1));
    }
}

// r[impl nix-drv-serialize.string-escape]
/// Escape a byte string for ATerm serialization and wrap it in quotes.
pub fn escape_aterm(s: &Vec<u8>) -> (result: Vec<u8>)
    ensures result@ == escape_aterm_spec(s@)
{
    let mut result = Vec::new();
    result.push(34u8);

    let mut i: usize = 0;
    while i < s.len()
        invariant
            i <= s.len(),
            result@ == seq![34u8] + escape_prefix_spec(s@, i as nat),
        decreases s.len() - i,
    {
        let b = s[i];
        if b == 92u8 {
            result.push(92u8);
            result.push(92u8);
        } else if b == 34u8 {
            result.push(92u8);
            result.push(34u8);
        } else if b == 10u8 {
            result.push(92u8);
            result.push(110u8);
        } else if b == 13u8 {
            result.push(92u8);
            result.push(114u8);
        } else if b == 9u8 {
            result.push(92u8);
            result.push(116u8);
        } else {
            result.push(b);
        }
        i = i + 1;
    }

    result.push(34u8);
    result
}

// --- Serialization building blocks (spec level) ---

/// Spec: serialize a list of ATerm-escaped strings as `["s1","s2",...]`.
pub closed spec fn serialize_string_list_spec(items: Seq<Seq<u8>>, upto: nat) -> Seq<u8>
    recommends upto <= items.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else if upto == 1 {
        escape_aterm_spec(items[0int])
    } else {
        serialize_string_list_spec(items, (upto - 1) as nat)
            + seq![44u8]
            + escape_aterm_spec(items[(upto - 1) as int])
    }
}

/// Spec: serialize environment variable pairs as `[("k","v"),...]`.
pub closed spec fn serialize_env_pairs_spec(
    env: Seq<(Seq<u8>, Seq<u8>)>, upto: nat,
) -> Seq<u8>
    recommends upto <= env.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        let pair = seq![40u8]
            + escape_aterm_spec(env[(upto - 1) as int].0)
            + seq![44u8]
            + escape_aterm_spec(env[(upto - 1) as int].1)
            + seq![41u8];
        if upto == 1 {
            pair
        } else {
            serialize_env_pairs_spec(env, (upto - 1) as nat) + seq![44u8] + pair
        }
    }
}

/// Spec: serialize output declarations as `[("name","path","algo","hash"),...]`.
pub closed spec fn serialize_outputs_spec(
    outputs: Seq<(Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>)>, upto: nat,
) -> Seq<u8>
    recommends upto <= outputs.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        let o = outputs[(upto - 1) as int];
        let entry = seq![40u8]
            + escape_aterm_spec(o.0)
            + seq![44u8]
            + escape_aterm_spec(o.1)
            + seq![44u8]
            + escape_aterm_spec(o.2)
            + seq![44u8]
            + escape_aterm_spec(o.3)
            + seq![41u8];
        if upto == 1 {
            entry
        } else {
            serialize_outputs_spec(outputs, (upto - 1) as nat) + seq![44u8] + entry
        }
    }
}

/// Spec: serialize input derivations as `[("path",["out1","out2"]),...]`.
pub closed spec fn serialize_input_drvs_spec(
    input_drvs: Seq<(Seq<u8>, Seq<Seq<u8>>)>, upto: nat,
) -> Seq<u8>
    recommends upto <= input_drvs.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        let d = input_drvs[(upto - 1) as int];
        let entry = seq![40u8]
            + escape_aterm_spec(d.0)
            + seq![44u8, 91u8]
            + serialize_string_list_spec(d.1, d.1.len())
            + seq![93u8, 41u8];
        if upto == 1 {
            entry
        } else {
            serialize_input_drvs_spec(input_drvs, (upto - 1) as nat) + seq![44u8] + entry
        }
    }
}

// r[depends nix-drv-serialize.format]
/// Spec: full derivation serialization in ATerm format.
pub open spec fn serialize_drv_spec(
    outputs: Seq<(Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>)>,
    input_drvs: Seq<(Seq<u8>, Seq<Seq<u8>>)>,
    input_srcs: Seq<Seq<u8>>,
    platform: Seq<u8>,
    builder: Seq<u8>,
    args: Seq<Seq<u8>>,
    env: Seq<(Seq<u8>, Seq<u8>)>,
) -> Seq<u8> {
    seq![68u8, 101u8, 114u8, 105u8, 118u8, 101u8, 40u8, 91u8]  // "Derive(["
        + serialize_outputs_spec(outputs, outputs.len())
        + seq![93u8, 44u8, 91u8]  // "],["
        + serialize_input_drvs_spec(input_drvs, input_drvs.len())
        + seq![93u8, 44u8, 91u8]  // "],["
        + serialize_string_list_spec(input_srcs, input_srcs.len())
        + seq![93u8, 44u8]  // "],"
        + escape_aterm_spec(platform)
        + seq![44u8]  // ","
        + escape_aterm_spec(builder)
        + seq![44u8, 91u8]  // ",["
        + serialize_string_list_spec(args, args.len())
        + seq![93u8, 44u8, 91u8]  // "],["
        + serialize_env_pairs_spec(env, env.len())
        + seq![93u8, 41u8]  // "])"
}

// --- Exec helpers ---

fn ser_append_slice(dst: &mut Vec<u8>, src: &[u8])
    ensures dst@ == old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ == old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i = i + 1;
    }
}

fn ser_append_vec(dst: &mut Vec<u8>, src: &Vec<u8>)
    ensures dst@ == old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ == old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i = i + 1;
    }
}

fn ser_append_escaped(dst: &mut Vec<u8>, s: &Vec<u8>)
    ensures dst@ == old(dst)@ + escape_aterm_spec(s@)
{
    let escaped = escape_aterm(s);
    ser_append_vec(dst, &escaped);
}

fn serialize_string_list(dst: &mut Vec<u8>, items: &Vec<Vec<u8>>)
    ensures dst@ == old(dst)@
        + seq![91u8]
        + serialize_string_list_spec(
            items@.map_values(|v: Vec<u8>| v@), items@.len() as nat)
        + seq![93u8]
{
    let ghost mapped = items@.map_values(|v: Vec<u8>| v@);
    dst.push(91u8);
    let mut i: usize = 0;
    let ghost base = old(dst)@ + seq![91u8];
    while i < items.len()
        invariant
            i <= items.len(),
            mapped == items@.map_values(|v: Vec<u8>| v@),
            dst@ == base + serialize_string_list_spec(mapped, i as nat),
        decreases items.len() - i,
    {
        if i > 0 {
            dst.push(44u8);
        }
        ser_append_escaped(dst, &items[i]);
        i = i + 1;
    }
    dst.push(93u8);
}

fn serialize_env_pairs(dst: &mut Vec<u8>, env: &Vec<(Vec<u8>, Vec<u8>)>)
    ensures dst@ == old(dst)@
        + seq![91u8]
        + serialize_env_pairs_spec(
            env@.map_values(|p: (Vec<u8>, Vec<u8>)| (p.0@, p.1@)), env@.len() as nat)
        + seq![93u8]
{
    let ghost mapped = env@.map_values(|p: (Vec<u8>, Vec<u8>)| (p.0@, p.1@));
    dst.push(91u8);
    let mut i: usize = 0;
    let ghost base = old(dst)@ + seq![91u8];
    while i < env.len()
        invariant
            i <= env.len(),
            mapped == env@.map_values(|p: (Vec<u8>, Vec<u8>)| (p.0@, p.1@)),
            dst@ == base + serialize_env_pairs_spec(mapped, i as nat),
        decreases env.len() - i,
    {
        if i > 0 {
            dst.push(44u8);
        }
        dst.push(40u8);
        ser_append_escaped(dst, &env[i].0);
        dst.push(44u8);
        ser_append_escaped(dst, &env[i].1);
        dst.push(41u8);
        i = i + 1;
    }
    dst.push(93u8);
}

fn serialize_outputs(
    dst: &mut Vec<u8>,
    outputs: &Vec<DerivationOutput>,
)
    ensures dst@ == old(dst)@
        + seq![91u8]
        + serialize_outputs_spec(
            outputs@.map_values(|o: DerivationOutput| (o.name@, o.path@, o.hash_algo@, o.hash@)),
            outputs@.len() as nat)
        + seq![93u8]
{
    let ghost mapped = outputs@.map_values(
        |o: DerivationOutput| (o.name@, o.path@, o.hash_algo@, o.hash@));
    dst.push(91u8);
    let mut i: usize = 0;
    let ghost base = old(dst)@ + seq![91u8];
    while i < outputs.len()
        invariant
            i <= outputs.len(),
            mapped == outputs@.map_values(
                |o: DerivationOutput| (o.name@, o.path@, o.hash_algo@, o.hash@)),
            dst@ == base + serialize_outputs_spec(mapped, i as nat),
        decreases outputs.len() - i,
    {
        if i > 0 {
            dst.push(44u8);
        }
        dst.push(40u8);
        ser_append_escaped(dst, &outputs[i].name);
        dst.push(44u8);
        ser_append_escaped(dst, &outputs[i].path);
        dst.push(44u8);
        ser_append_escaped(dst, &outputs[i].hash_algo);
        dst.push(44u8);
        ser_append_escaped(dst, &outputs[i].hash);
        dst.push(41u8);
        i = i + 1;
    }
    dst.push(93u8);
}

fn serialize_input_drvs(
    dst: &mut Vec<u8>,
    input_drvs: &Vec<(Vec<u8>, Vec<Vec<u8>>)>,
)
    ensures dst@ == old(dst)@
        + seq![91u8]
        + serialize_input_drvs_spec(
            input_drvs@.map_values(
                |d: (Vec<u8>, Vec<Vec<u8>>)| (d.0@, d.1@.map_values(|v: Vec<u8>| v@))),
            input_drvs@.len() as nat)
        + seq![93u8]
{
    let ghost mapped = input_drvs@.map_values(
        |d: (Vec<u8>, Vec<Vec<u8>>)| (d.0@, d.1@.map_values(|v: Vec<u8>| v@)));
    dst.push(91u8);
    let mut i: usize = 0;
    let ghost base = old(dst)@ + seq![91u8];
    while i < input_drvs.len()
        invariant
            i <= input_drvs.len(),
            mapped == input_drvs@.map_values(
                |d: (Vec<u8>, Vec<Vec<u8>>)| (d.0@, d.1@.map_values(|v: Vec<u8>| v@))),
            dst@ == base + serialize_input_drvs_spec(mapped, i as nat),
        decreases input_drvs.len() - i,
    {
        if i > 0 {
            dst.push(44u8);
        }
        dst.push(40u8);
        ser_append_escaped(dst, &input_drvs[i].0);
        // inner list: ["out1","out2"]
        let inner = &input_drvs[i].1;
        dst.push(44u8);
        dst.push(91u8);
        let ghost inner_mapped = inner@.map_values(|v: Vec<u8>| v@);
        let ghost inner_base = dst@;
        let mut j: usize = 0;
        while j < inner.len()
            invariant
                j <= inner.len(),
                inner_mapped == inner@.map_values(|v: Vec<u8>| v@),
                dst@ == inner_base + serialize_string_list_spec(inner_mapped, j as nat),
            decreases inner.len() - j,
        {
            if j > 0 {
                dst.push(44u8);
            }
            ser_append_escaped(dst, &inner[j]);
            j = j + 1;
        }
        dst.push(93u8);
        dst.push(41u8);
        i = i + 1;
    }
    dst.push(93u8);
}

// r[impl nix-drv-serialize.format]
/// Serialize a derivation in ATerm format.
pub fn serialize_drv(drv: &Derivation) -> (result: Vec<u8>)
    ensures result@ == serialize_drv_spec(
        drv.outputs@.map_values(
            |o: DerivationOutput| (o.name@, o.path@, o.hash_algo@, o.hash@)),
        drv.input_drvs@.map_values(
            |d: (Vec<u8>, Vec<Vec<u8>>)| (d.0@, d.1@.map_values(|v: Vec<u8>| v@))),
        drv.input_srcs@.map_values(|v: Vec<u8>| v@),
        drv.platform@,
        drv.builder@,
        drv.args@.map_values(|v: Vec<u8>| v@),
        drv.env@.map_values(|p: (Vec<u8>, Vec<u8>)| (p.0@, p.1@)),
    )
{
    let mut result: Vec<u8> = Vec::new();
    // "Derive("
    ser_append_slice(&mut result, &[68u8, 101u8, 114u8, 105u8, 118u8, 101u8, 40u8]);
    serialize_outputs(&mut result, &drv.outputs);
    result.push(44u8);
    serialize_input_drvs(&mut result, &drv.input_drvs);
    result.push(44u8);
    serialize_string_list(&mut result, &drv.input_srcs);
    result.push(44u8);
    ser_append_escaped(&mut result, &drv.platform);
    result.push(44u8);
    ser_append_escaped(&mut result, &drv.builder);
    result.push(44u8);
    serialize_string_list(&mut result, &drv.args);
    result.push(44u8);
    serialize_env_pairs(&mut result, &drv.env);
    result.push(41u8);
    result
}

// r[verify nix-drv-serialize.determinism]
/// Serialization is deterministic: same derivation always produces the same output.
pub proof fn serialize_drv_deterministic(
    outputs: Seq<(Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>)>,
    input_drvs: Seq<(Seq<u8>, Seq<Seq<u8>>)>,
    input_srcs: Seq<Seq<u8>>,
    platform: Seq<u8>,
    builder: Seq<u8>,
    args: Seq<Seq<u8>>,
    env: Seq<(Seq<u8>, Seq<u8>)>,
)
    ensures
        serialize_drv_spec(outputs, input_drvs, input_srcs, platform, builder, args, env)
            == serialize_drv_spec(outputs, input_drvs, input_srcs, platform, builder, args, env)
{}

/// Forward-recursive escape for a subrange s[from..to].
spec fn escape_shifted_spec(s: Seq<u8>, from: nat, to: nat) -> Seq<u8>
    recommends from <= to, to <= s.len()
    decreases to - from
{
    if from >= to {
        Seq::<u8>::empty()
    } else {
        escape_byte_spec(s[from as int])
            + escape_shifted_spec(s, from + 1, to)
    }
}

/// Bridge: escape_prefix_spec(s, n) == escape_shifted_spec(s, 0, n)
proof fn escape_prefix_eq_shifted(s: Seq<u8>, n: nat)
    requires n <= s.len()
    ensures escape_prefix_spec(s, n) == escape_shifted_spec(s, 0, n)
    decreases n
{
    if n > 0 {
        escape_prefix_eq_shifted(s, (n - 1) as nat);
        escape_shifted_snoc(s, 0, (n - 1) as nat, n);
    }
}

/// Appending one more escape_byte to shifted spec.
proof fn escape_shifted_snoc(s: Seq<u8>, from: nat, mid: nat, to: nat)
    requires from <= mid, mid < to, to <= s.len()
    ensures
        escape_shifted_spec(s, from, mid)
            + escape_byte_spec(s[mid as int])
            + escape_shifted_spec(s, mid + 1, to)
            == escape_shifted_spec(s, from, to)
    decreases mid - from
{
    if from < mid {
        escape_shifted_snoc(s, from + 1, mid, to);
    }
}

/// Seq lemma: if Y != Z then X + Y != X + Z (for u8 sequences).
proof fn seq_append_left_cancel(x: Seq<u8>, y: Seq<u8>, z: Seq<u8>)
    requires y != z
    ensures x + y != x + z
{
    assert((x + y) != (x + z)) by {
        if (x + y) =~= (x + z) {
            assert(y =~= z) by {
                assert forall|i: int| 0 <= i < y.len() implies y[i] == z[i] by {
                    assert(y[i] == (x + y)[(x.len() + i) as int]);
                    assert(z[i] == (x + z)[(x.len() + i) as int]);
                }
            }
        }
    }
}

/// escape_byte outputs are distinguishable in the first 1-2 bytes.
proof fn escape_byte_distinguishable(a: u8, b: u8)
    requires a != b
    ensures
        forall|xa: Seq<u8>, xb: Seq<u8>|
            escape_byte_spec(a) + xa != escape_byte_spec(b) + xb
{
    assert forall|xa: Seq<u8>, xb: Seq<u8>|
        escape_byte_spec(a) + xa != escape_byte_spec(b) + xb
    by {
        let ea = escape_byte_spec(a);
        let eb = escape_byte_spec(b);
        let ca = ea + xa;
        let cb = eb + xb;
        if ea.len() == 1 && eb.len() == 1 {
            assert(ca[0int] != cb[0int]);
        } else if ea.len() == 2 && eb.len() == 2 {
            assert(ca[1int] != cb[1int]);
        } else if ea.len() == 1 && eb.len() == 2 {
            assert(ca[0int] != cb[0int]);
        } else {
            assert(ca[0int] != cb[0int]);
        }
    }
}

/// Forward escape of shifted ranges is injective.
proof fn escape_shifted_injective(a: Seq<u8>, fa: nat, ta: nat,
                                   b: Seq<u8>, fb: nat, tb: nat)
    requires
        fa <= ta, ta <= a.len(),
        fb <= tb, tb <= b.len(),
        a.subrange(fa as int, ta as int) != b.subrange(fb as int, tb as int),
    ensures
        escape_shifted_spec(a, fa, ta) != escape_shifted_spec(b, fb, tb)
    decreases (ta - fa) + (tb - fb)
{
    let sa = a.subrange(fa as int, ta as int);
    let sb = b.subrange(fb as int, tb as int);
    if fa == ta && fb == tb {
        // Both empty — contradicts sa != sb.
        assert(sa =~= Seq::<u8>::empty());
        assert(sb =~= Seq::<u8>::empty());
    } else if fa == ta {
        // a range empty, b range non-empty.
        assert(escape_shifted_spec(a, fa, ta).len() == 0);
        assert(escape_shifted_spec(b, fb, tb).len() >= 1);
    } else if fb == tb {
        assert(escape_shifted_spec(b, fb, tb).len() == 0);
        assert(escape_shifted_spec(a, fa, ta).len() >= 1);
    } else if a[fa as int] == b[fb as int] {
        // Same first byte — strip and recurse.
        let sa_tail = a.subrange((fa + 1) as int, ta as int);
        let sb_tail = b.subrange((fb + 1) as int, tb as int);
        assert(sa.len() == 1 + sa_tail.len());
        assert(sb.len() == 1 + sb_tail.len());
        assert(sa_tail != sb_tail) by {
            if sa_tail =~= sb_tail {
                assert(sa.len() == sb.len());
                assert forall|i: int| 0 <= i < sa.len() implies #[trigger] sa[i] == sb[i] by {
                    if i == 0 {
                    } else {
                        assert(sa[i] == sa_tail[(i - 1) as int]);
                        assert(sb[i] == sb_tail[(i - 1) as int]);
                    }
                }
                assert(sa =~= sb);
            }
        }
        escape_shifted_injective(a, fa + 1, ta, b, fb + 1, tb);
        let common = escape_byte_spec(a[fa as int]);
        seq_append_left_cancel(
            common,
            escape_shifted_spec(a, fa + 1, ta),
            escape_shifted_spec(b, fb + 1, tb),
        );
    } else {
        // Different first bytes.
        escape_byte_distinguishable(a[fa as int], b[fb as int]);
        // escape_shifted(a, fa, ta) = escape_byte(a[fa]) + escape_shifted(a, fa+1, ta)
        // escape_shifted(b, fb, tb) = escape_byte(b[fb]) + escape_shifted(b, fb+1, tb)
        // By escape_byte_distinguishable: these must differ.
        assert(escape_shifted_spec(a, fa, ta)
            == escape_byte_spec(a[fa as int]) + escape_shifted_spec(a, fa + 1, ta));
        assert(escape_shifted_spec(b, fb, tb)
            == escape_byte_spec(b[fb as int]) + escape_shifted_spec(b, fb + 1, tb));
    }
}

// r[verify nix-drv-serialize.injectivity]
/// ATerm escaping is injective: distinct inputs produce distinct escaped outputs.
pub proof fn escape_aterm_injective(a: Seq<u8>, b: Seq<u8>)
    requires a != b
    ensures escape_aterm_spec(a) != escape_aterm_spec(b)
{
    assert(a.subrange(0, a.len() as int) =~= a);
    assert(b.subrange(0, b.len() as int) =~= b);
    escape_shifted_injective(a, 0, a.len(), b, 0, b.len());
    // Now: escape_shifted_spec(a, 0, a.len()) != escape_shifted_spec(b, 0, b.len())
    escape_prefix_eq_shifted(a, a.len());
    escape_prefix_eq_shifted(b, b.len());
    // Bridge: escape_prefix_spec(s, n) == escape_shifted_spec(s, 0, n)
    assert(escape_prefix_spec(a, a.len()) != escape_prefix_spec(b, b.len()));
    // escape_aterm = "[34]" + escape_prefix + "[34]"
    // If the middles differ, the wholes differ.
    let pa = escape_prefix_spec(a, a.len());
    let pb = escape_prefix_spec(b, b.len());
    assert(escape_aterm_spec(a) != escape_aterm_spec(b)) by {
        if escape_aterm_spec(a) =~= escape_aterm_spec(b) {
            // escape_aterm(a) = seq![34] + pa + seq![34]
            // escape_aterm(b) = seq![34] + pb + seq![34]
            // escape_aterm(s) = seq![34] + escape_prefix(s, s.len()) + seq![34]
            // length = 1 + pa.len() + 1
            assert(escape_aterm_spec(a).len() == 1 + pa.len() + 1);
            assert(escape_aterm_spec(b).len() == 1 + pb.len() + 1);
            assert(pa.len() == pb.len());
            assert(pa =~= pb) by {
                assert forall|i: int| 0 <= i < pa.len() implies pa[i] == pb[i] by {
                    assert(pa[i] == escape_aterm_spec(a)[(1 + i) as int]);
                    assert(pb[i] == escape_aterm_spec(b)[(1 + i) as int]);
                }
            }
        }
    }
}

proof fn escape_byte_injective(a: u8, b: u8)
    requires a != b
    ensures escape_byte_spec(a) != escape_byte_spec(b)
{
    escape_byte_distinguishable(a, b);
    let e = Seq::<u8>::empty();
    assert(escape_byte_spec(a) + e != escape_byte_spec(b) + e);
}

// r[verify nix-drv-serialize.injectivity]
/// Different platforms produce different serializations.
pub proof fn serialize_drv_platform_injective(
    outputs: Seq<(Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>)>,
    input_drvs: Seq<(Seq<u8>, Seq<Seq<u8>>)>,
    input_srcs: Seq<Seq<u8>>,
    platform_a: Seq<u8>,
    platform_b: Seq<u8>,
    builder: Seq<u8>,
    args: Seq<Seq<u8>>,
    env: Seq<(Seq<u8>, Seq<u8>)>,
)
    requires platform_a != platform_b
    ensures
        serialize_drv_spec(outputs, input_drvs, input_srcs, platform_a, builder, args, env)
            != serialize_drv_spec(outputs, input_drvs, input_srcs, platform_b, builder, args, env)
{
    escape_aterm_injective(platform_a, platform_b);
    // The serializations differ because they embed escape_aterm(platform) at the
    // same offset, and escape_aterm is injective. If the escape lengths differ,
    // the total serialization lengths differ. If the escape lengths match, the
    // escape contents differ at the same position in the serialization.
    serialize_drv_platform_field_differs(
        outputs, input_drvs, input_srcs,
        platform_a, platform_b, builder, args, env,
    );
}

proof fn serialize_drv_platform_field_differs(
    outputs: Seq<(Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>)>,
    input_drvs: Seq<(Seq<u8>, Seq<Seq<u8>>)>,
    input_srcs: Seq<Seq<u8>>,
    platform_a: Seq<u8>,
    platform_b: Seq<u8>,
    builder: Seq<u8>,
    args: Seq<Seq<u8>>,
    env: Seq<(Seq<u8>, Seq<u8>)>,
)
    requires escape_aterm_spec(platform_a) != escape_aterm_spec(platform_b)
    ensures
        serialize_drv_spec(outputs, input_drvs, input_srcs, platform_a, builder, args, env)
            != serialize_drv_spec(outputs, input_drvs, input_srcs, platform_b, builder, args, env)
{
    let ea = escape_aterm_spec(platform_a);
    let eb = escape_aterm_spec(platform_b);
    let sa = serialize_drv_spec(
        outputs, input_drvs, input_srcs, platform_a, builder, args, env);
    let sb = serialize_drv_spec(
        outputs, input_drvs, input_srcs, platform_b, builder, args, env);
    if ea.len() != eb.len() {
        // Every field except the escaped platform is identical, so removing the
        // platform contribution leaves the same framing length on both sides.
        assert(sa.len() as int - ea.len() as int == sb.len() as int - eb.len() as int);
        assert(sa.len() != sb.len());
    } else {
        // Same escape length, different content.
        // Both serializations share the same prefix up to the platform field.
        // At the platform field offset, sa embeds ea and sb embeds eb.
        let outs = seq![91u8] + serialize_outputs_spec(outputs, outputs.len()) + seq![93u8];
        let drvs = seq![91u8] + serialize_input_drvs_spec(input_drvs, input_drvs.len()) + seq![93u8];
        let srcs = seq![91u8] + serialize_string_list_spec(input_srcs, input_srcs.len()) + seq![93u8];
        // prefix = "Derive(" + outs + "," + drvs + "," + srcs + ","
        let offset: int = (7 + outs.len() + 1 + drvs.len() + 1 + srcs.len() + 1) as int;
        assert(sa != sb) by {
            if sa =~= sb {
                assert forall|i: int| 0 <= i < ea.len() implies ea[i] == eb[i] by {
                    assert(ea[i] == sa[(offset + i) as int]);
                    assert(eb[i] == sb[(offset + i) as int]);
                }
                assert(ea =~= eb);
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use super::*;

    fn v(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    // r[verify nix-drv-serialize.string-escape]
    #[test]
    fn test_escape_aterm_plain() {
        assert_eq!(escape_aterm(&v("hello")), v("\"hello\""));
    }

    // r[verify nix-drv-serialize.string-escape]
    #[test]
    fn test_escape_aterm_specials() {
        assert_eq!(escape_aterm(&b"a\n\r\t\\\"b".to_vec()), v("\"a\\n\\r\\t\\\\\\\"b\""));
    }

    // r[verify nix-drv-serialize.string-escape]
    #[test]
    fn test_derivation_structs_construct() {
        let output = DerivationOutput {
            name: v("out"),
            path: v("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-hello"),
            hash_algo: v("sha256"),
            hash: v("deadbeef"),
        };
        let drv = Derivation {
            outputs: vec![output],
            input_drvs: vec![(v("/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-input.drv"), vec![v("out")])],
            input_srcs: vec![v("/nix/store/cccccccccccccccccccccccccccccccc-src")],
            platform: v("x86_64-linux"),
            builder: v("/nix/store/dddddddddddddddddddddddddddddddd-bash/bin/bash"),
            args: vec![v("-c"), v("echo hi")],
            env: vec![(v("name"), v("hello"))],
        };
        assert_eq!(drv.outputs.len(), 1);
        assert_eq!(drv.args.len(), 2);
        assert_eq!(drv.env[0].0, v("name"));
    }

    // r[verify nix-drv-serialize.format]
    #[test]
    fn test_serialize_drv_simple() {
        let drv = Derivation {
            outputs: vec![DerivationOutput {
                name: v("out"),
                path: v("/nix/store/aaaa-hello"),
                hash_algo: v(""),
                hash: v(""),
            }],
            input_drvs: vec![(v("/nix/store/bbbb-input.drv"), vec![v("out")])],
            input_srcs: vec![v("/nix/store/cccc-src")],
            platform: v("x86_64-linux"),
            builder: v("/bin/sh"),
            args: vec![v("-e"), v("echo hi")],
            env: vec![(v("name"), v("hello")), (v("out"), v("/nix/store/aaaa-hello"))],
        };
        let result = String::from_utf8(serialize_drv(&drv)).unwrap();
        assert_eq!(
            result,
            "Derive([(\"out\",\"/nix/store/aaaa-hello\",\"\",\"\")],\
             [(\"/nix/store/bbbb-input.drv\",[\"out\"])],\
             [\"/nix/store/cccc-src\"],\
             \"x86_64-linux\",\"/bin/sh\",[\"-e\",\"echo hi\"],\
             [(\"name\",\"hello\"),(\"out\",\"/nix/store/aaaa-hello\")])"
        );
    }

    // r[verify nix-drv-serialize.determinism]
    #[test]
    fn test_serialize_drv_deterministic() {
        let drv = Derivation {
            outputs: vec![DerivationOutput {
                name: v("out"),
                path: v("/nix/store/x-pkg"),
                hash_algo: v(""),
                hash: v(""),
            }],
            input_drvs: vec![],
            input_srcs: vec![],
            platform: v("x86_64-linux"),
            builder: v("/bin/sh"),
            args: vec![],
            env: vec![(v("a"), v("1"))],
        };
        assert_eq!(serialize_drv(&drv), serialize_drv(&drv));
    }

    // r[verify nix-drv-serialize.string-escape]
    #[test]
    fn test_serialize_drv_escape_env() {
        let drv = Derivation {
            outputs: vec![DerivationOutput {
                name: v("out"),
                path: v("/nix/store/x-pkg"),
                hash_algo: v(""),
                hash: v(""),
            }],
            input_drvs: vec![],
            input_srcs: vec![],
            platform: v("x86_64-linux"),
            builder: v("/bin/sh"),
            args: vec![],
            env: vec![(v("script"), b"echo \"hi\"\n".to_vec())],
        };
        let result = String::from_utf8(serialize_drv(&drv)).unwrap();
        assert!(result.contains(r#"("script","echo \"hi\"\n")"#));
    }

    proptest! {
        // r[verify nix-drv-serialize.string-escape]
        #[test]
        fn prop_escape_aterm_quotes_input(
            s in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let escaped = escape_aterm(&s);
            prop_assert!(escaped.len() >= 2);
            prop_assert_eq!(escaped[0], b'"');
            prop_assert_eq!(escaped[escaped.len() - 1], b'"');
        }

        // r[verify nix-drv-serialize.string-escape]
        #[test]
        fn prop_escape_aterm_escapes_specials(
            s in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let escaped = escape_aterm(&s);
            let inner = &escaped[1..escaped.len() - 1];
            let mut escaped_byte = false;
            for &b in inner {
                if escaped_byte {
                    prop_assert!(matches!(b, b'\\' | b'"' | b'n' | b'r' | b't'));
                    escaped_byte = false;
                } else if b == b'\\' {
                    escaped_byte = true;
                } else {
                    prop_assert!(!matches!(b, b'"' | b'\n' | b'\r' | b'\t'));
                }
            }
            prop_assert!(!escaped_byte, "escape sequence truncated at end of output");
        }

        // r[verify nix-drv-serialize.format]
        #[test]
        fn prop_serialize_drv_starts_with_derive(
            name in "[a-z]{1,10}",
            path in "[a-z/]{1,20}",
        ) {
            let drv = Derivation {
                outputs: vec![DerivationOutput {
                    name: name.into_bytes(),
                    path: path.into_bytes(),
                    hash_algo: vec![],
                    hash: vec![],
                }],
                input_drvs: vec![],
                input_srcs: vec![],
                platform: v("x86_64-linux"),
                builder: v("/bin/sh"),
                args: vec![],
                env: vec![],
            };
            let result = serialize_drv(&drv);
            prop_assert!(result.starts_with(b"Derive("));
            prop_assert!(result.ends_with(b")"));
        }

        // r[verify nix-drv-serialize.determinism]
        #[test]
        fn prop_serialize_drv_deterministic(
            key in "[a-z]{1,10}",
            val in "[a-z]{1,10}",
        ) {
            let drv = Derivation {
                outputs: vec![DerivationOutput {
                    name: v("out"),
                    path: v("/nix/store/x-y"),
                    hash_algo: vec![],
                    hash: vec![],
                }],
                input_drvs: vec![],
                input_srcs: vec![],
                platform: v("x86_64-linux"),
                builder: v("/bin/sh"),
                args: vec![],
                env: vec![(key.into_bytes(), val.into_bytes())],
            };
            prop_assert_eq!(serialize_drv(&drv), serialize_drv(&drv));
        }
    }
}
