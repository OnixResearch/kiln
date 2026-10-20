//! Verified ChaosControl coverage bitmap primitives.
//!
//! Models a 64 KiB AFL-style bitmap partitioned into code, assertion, and
//! schedule regions with pointwise-max merge, new-edge counting, and AFL
//! bucket classification.

use vstd::prelude::*;

verus! {

// ── Region constants ────────────────────────────────────────────────

pub const MAP_SIZE: usize = 65536;
pub const CODE_REGION_END: usize = 32768;
pub const ASSERTION_REGION_END: usize = 49152;

// ── Spec helpers ────────────────────────────────────────────────────

/// A valid coverage bitmap is exactly MAP_SIZE bytes.
pub open spec fn valid_coverage_bitmap_spec(bm: Seq<u8>) -> bool {
    bm.len() == MAP_SIZE as int
}

pub open spec fn code_region_spec() -> (int, int) { (0, CODE_REGION_END as int) }
pub open spec fn assertion_region_spec() -> (int, int) { (CODE_REGION_END as int, ASSERTION_REGION_END as int) }
pub open spec fn schedule_region_spec() -> (int, int) { (ASSERTION_REGION_END as int, MAP_SIZE as int) }

/// Region non-interference: indices outside `[lo, hi)` are unchanged.
pub open spec fn region_unchanged_spec(old_bm: Seq<u8>, new_bm: Seq<u8>, lo: int, hi: int) -> bool {
    &&& new_bm.len() == old_bm.len()
    &&& forall|i: int| 0 <= i < old_bm.len() && !(lo <= i < hi) ==> new_bm[i] == old_bm[i]
}

/// Pointwise max merge result.
pub open spec fn merge_spec(a: Seq<u8>, b: Seq<u8>) -> Seq<u8>
    recommends a.len() == b.len()
    decreases a.len()
{
    if a.len() == 0 {
        Seq::<u8>::empty()
    } else {
        let last = (a.len() - 1) as int;
        let max_val: u8 = if a[last] >= b[last] { a[last] } else { b[last] };
        merge_spec(a.subrange(0, last), b.subrange(0, last)).push(max_val)
    }
}

/// New-edge count spec: count of positions where branch > 0 and global == 0.
pub open spec fn new_edge_count_spec(branch: Seq<u8>, global: Seq<u8>) -> int
    recommends branch.len() == global.len()
    decreases branch.len()
{
    if branch.len() == 0 {
        0int
    } else {
        let last = (branch.len() - 1) as int;
        let tail = new_edge_count_spec(
            branch.subrange(0, last),
            global.subrange(0, last),
        );
        if branch[last] > 0 && global[last] == 0 { tail + 1 } else { tail }
    }
}

/// AFL bucket classification for a single counter.
pub open spec fn classify_bucket_spec(count: u8) -> u8 {
    if count == 0 { 0u8 }
    else if count == 1 { 1u8 }
    else if count == 2 { 2u8 }
    else if count == 3 { 3u8 }
    else if count <= 7 { 4u8 }
    else if count <= 15 { 8u8 }
    else if count <= 31 { 16u8 }
    else if count <= 127 { 32u8 }
    else { 128u8 }
}

/// Pointwise classification of an entire bitmap.
pub open spec fn classify_bitmap_spec(bm: Seq<u8>) -> Seq<u8>
    decreases bm.len()
{
    if bm.len() == 0 {
        Seq::<u8>::empty()
    } else {
        let last = (bm.len() - 1) as int;
        classify_bitmap_spec(bm.subrange(0, last)).push(classify_bucket_spec(bm[last]))
    }
}

// ── Proof helpers ───────────────────────────────────────────────────

/// Merge preserves length.
proof fn merge_spec_len(a: Seq<u8>, b: Seq<u8>)
    requires a.len() == b.len()
    ensures merge_spec(a, b).len() == a.len()
    decreases a.len()
{
    if a.len() > 0 {
        let last = (a.len() - 1) as int;
        merge_spec_len(a.subrange(0, last), b.subrange(0, last));
    }
}

/// Merge result at index i equals max(a[i], b[i]).
proof fn merge_spec_index(a: Seq<u8>, b: Seq<u8>, i: int)
    requires a.len() == b.len(), 0 <= i < a.len()
    ensures
        merge_spec(a, b).len() == a.len(),
        merge_spec(a, b)[i] == if a[i] >= b[i] { a[i] } else { b[i] },
    decreases a.len()
{
    merge_spec_len(a, b);
    let last = (a.len() - 1) as int;
    if i == last {
        if a.len() > 1 {
            merge_spec_len(a.subrange(0, last), b.subrange(0, last));
        }
    } else {
        merge_spec_index(a.subrange(0, last), b.subrange(0, last), i);
    }
}

/// New-edge count is nonneg.
proof fn new_edge_count_nonneg(branch: Seq<u8>, global: Seq<u8>)
    requires branch.len() == global.len()
    ensures new_edge_count_spec(branch, global) >= 0
    decreases branch.len()
{
    if branch.len() > 0 {
        let last = (branch.len() - 1) as int;
        new_edge_count_nonneg(
            branch.subrange(0, last),
            global.subrange(0, last),
        );
    }
}

/// New-edge count fits u32 when bitmap length <= 65536.
proof fn new_edge_count_bounded(branch: Seq<u8>, global: Seq<u8>)
    requires branch.len() == global.len(), branch.len() <= MAP_SIZE as int
    ensures 0 <= new_edge_count_spec(branch, global) <= branch.len()
    decreases branch.len()
{
    if branch.len() > 0 {
        let last = (branch.len() - 1) as int;
        new_edge_count_bounded(
            branch.subrange(0, last),
            global.subrange(0, last),
        );
    }
}

/// Classify preserves length.
proof fn classify_bitmap_spec_len(bm: Seq<u8>)
    ensures classify_bitmap_spec(bm).len() == bm.len()
    decreases bm.len()
{
    if bm.len() > 0 {
        let last = (bm.len() - 1) as int;
        classify_bitmap_spec_len(bm.subrange(0, last));
    }
}

/// Classify result at index i.
proof fn classify_bitmap_spec_index(bm: Seq<u8>, i: int)
    requires 0 <= i < bm.len()
    ensures
        classify_bitmap_spec(bm).len() == bm.len(),
        classify_bitmap_spec(bm)[i] == classify_bucket_spec(bm[i]),
    decreases bm.len()
{
    classify_bitmap_spec_len(bm);
    let last = (bm.len() - 1) as int;
    if i == last {
        if bm.len() > 1 {
            classify_bitmap_spec_len(bm.subrange(0, last));
        }
    } else {
        classify_bitmap_spec_index(bm.subrange(0, last), i);
    }
}

// ── Region proofs ───────────────────────────────────────────────────

// r[verify chaoscontrol-coverage.regions]
/// Proof that the three regions partition [0, MAP_SIZE) exactly.
pub proof fn region_partition_exact()
    ensures
        code_region_spec() == (0int, 32768int),
        assertion_region_spec() == (32768int, 49152int),
        schedule_region_spec() == (49152int, 65536int),
        // lengths
        32768 - 0 == 32768,
        49152 - 32768 == 16384,
        65536 - 49152 == 16384,
        // cover
        32768 + 16384 + 16384 == 65536int,
{
}

// r[verify chaoscontrol-coverage.union]
/// Proof that merge is pointwise max.
pub proof fn merge_is_pointwise_max(a: Seq<u8>, b: Seq<u8>)
    requires a.len() == b.len()
    ensures
        merge_spec(a, b).len() == a.len(),
        forall|i: int| 0 <= i < a.len() ==>
            merge_spec(a, b)[i] == (if a[i] >= b[i] { a[i] } else { b[i] }),
{
    merge_spec_len(a, b);
    assert forall|i: int| 0 <= i < a.len() implies
        merge_spec(a, b)[i] == (if a[i] >= b[i] { a[i] } else { b[i] })
    by {
        merge_spec_index(a, b, i);
    }
}

// r[verify chaoscontrol-coverage.classify]
/// Proof that classify is pointwise bucket map.
pub proof fn classify_is_pointwise(bm: Seq<u8>)
    ensures
        classify_bitmap_spec(bm).len() == bm.len(),
        forall|i: int| 0 <= i < bm.len() ==>
            classify_bitmap_spec(bm)[i] == classify_bucket_spec(bm[i]),
{
    classify_bitmap_spec_len(bm);
    assert forall|i: int| 0 <= i < bm.len() implies
        classify_bitmap_spec(bm)[i] == classify_bucket_spec(bm[i])
    by {
        classify_bitmap_spec_index(bm, i);
    }
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-coverage.regions]
/// Creates a new zeroed coverage bitmap.
pub fn new_coverage_bitmap() -> (result: Vec<u8>)
    ensures
        result@.len() == MAP_SIZE as int,
        forall|i: int| 0 <= i < MAP_SIZE as int ==> result@[i] == 0u8,
{
    let mut bm = Vec::new();
    let mut idx: usize = 0;
    while idx < MAP_SIZE
        invariant
            0 <= idx <= MAP_SIZE,
            bm@.len() == idx as int,
            forall|j: int| 0 <= j < idx as int ==> bm@[j] == 0u8,
        decreases MAP_SIZE - idx,
    {
        bm.push(0u8);
        idx = idx + 1;
    }
    bm
}

// r[impl chaoscontrol-coverage.regions]
/// Validates that a bitmap has the right length.
pub fn valid_coverage_bitmap(bm: &Vec<u8>) -> (result: bool)
    ensures result == valid_coverage_bitmap_spec(bm@),
{
    bm.len() == MAP_SIZE
}

/// Records a hit in the code region. Other regions unchanged.
// r[impl chaoscontrol-coverage.regions]
pub fn record_code_hit(bm: &mut Vec<u8>, index: usize)
    requires
        valid_coverage_bitmap_spec(old(bm)@),
        0 <= index < CODE_REGION_END,
    ensures
        valid_coverage_bitmap_spec(bm@),
        bm@[index as int] == old(bm)@[index as int].saturating_add(1),
        forall|i: int| 0 <= i < MAP_SIZE as int && i != index as int ==> bm@[i] == old(bm)@[i],
{
    let old_val = bm[index];
    let new_val = if old_val < 255 { (old_val + 1) as u8 } else { 255u8 };
    bm.set(index, new_val);
}

/// Records a hit in the assertion region. Other regions unchanged.
// r[impl chaoscontrol-coverage.regions]
pub fn record_assertion_hit(bm: &mut Vec<u8>, index: usize)
    requires
        valid_coverage_bitmap_spec(old(bm)@),
        CODE_REGION_END <= index < ASSERTION_REGION_END,
    ensures
        valid_coverage_bitmap_spec(bm@),
        bm@[index as int] == old(bm)@[index as int].saturating_add(1),
        forall|i: int| 0 <= i < MAP_SIZE as int && i != index as int ==> bm@[i] == old(bm)@[i],
{
    let old_val = bm[index];
    let new_val = if old_val < 255 { (old_val + 1) as u8 } else { 255u8 };
    bm.set(index, new_val);
}

/// Records a hit in the schedule region. Other regions unchanged.
// r[impl chaoscontrol-coverage.regions]
pub fn record_schedule_hit(bm: &mut Vec<u8>, index: usize)
    requires
        valid_coverage_bitmap_spec(old(bm)@),
        ASSERTION_REGION_END <= index < MAP_SIZE,
    ensures
        valid_coverage_bitmap_spec(bm@),
        bm@[index as int] == old(bm)@[index as int].saturating_add(1),
        forall|i: int| 0 <= i < MAP_SIZE as int && i != index as int ==> bm@[i] == old(bm)@[i],
{
    let old_val = bm[index];
    let new_val = if old_val < 255 { (old_val + 1) as u8 } else { 255u8 };
    bm.set(index, new_val);
}

// r[impl chaoscontrol-coverage.union]
/// Pointwise-max merge of two bitmaps.
pub fn merge_coverage(a: &mut Vec<u8>, b: &Vec<u8>)
    requires
        valid_coverage_bitmap_spec(old(a)@),
        valid_coverage_bitmap_spec(b@),
    ensures
        valid_coverage_bitmap_spec(a@),
        forall|i: int| 0 <= i < MAP_SIZE as int ==>
            a@[i] == (if old(a)@[i] >= b@[i] { old(a)@[i] } else { b@[i] }),
{
    let mut idx: usize = 0;
    while idx < MAP_SIZE
        invariant
            0 <= idx <= MAP_SIZE,
            a@.len() == MAP_SIZE as int,
            b@.len() == MAP_SIZE as int,
            forall|j: int| 0 <= j < idx as int ==>
                a@[j] == (if old(a)@[j] >= b@[j] { old(a)@[j] } else { b@[j] }),
            forall|j: int| idx as int <= j < MAP_SIZE as int ==>
                a@[j] == old(a)@[j],
        decreases MAP_SIZE - idx,
    {
        let av = a[idx];
        let bv = b[idx];
        if bv > av {
            a.set(idx, bv);
        }
        idx = idx + 1;
    }
}

// r[impl chaoscontrol-coverage.union]
/// Exact new-edge count.
pub fn count_new_edges(branch: &Vec<u8>, global: &Vec<u8>) -> (result: u32)
    requires
        valid_coverage_bitmap_spec(branch@),
        valid_coverage_bitmap_spec(global@),
    ensures
        result as int == new_edge_count_spec(branch@, global@),
{
    let mut count: u32 = 0;
    let mut idx: usize = 0;
    while idx < MAP_SIZE
        invariant
            0 <= idx <= MAP_SIZE,
            branch@.len() == MAP_SIZE as int,
            global@.len() == MAP_SIZE as int,
            0 <= count as int <= idx as int,
            count as int == new_edge_count_spec(
                branch@.subrange(0, idx as int),
                global@.subrange(0, idx as int),
            ),
        decreases MAP_SIZE - idx,
    {
        // Unfold new_edge_count_spec for prefix of length idx+1
        proof {
            let br_pre = branch@.subrange(0, idx as int + 1);
            let gl_pre = global@.subrange(0, idx as int + 1);
            assert(br_pre.subrange(0, idx as int) =~= branch@.subrange(0, idx as int));
            assert(gl_pre.subrange(0, idx as int) =~= global@.subrange(0, idx as int));
            new_edge_count_bounded(
                branch@.subrange(0, idx as int),
                global@.subrange(0, idx as int),
            );
        }
        let bv = branch[idx];
        let gv = global[idx];
        if bv > 0 && gv == 0 {
            count = count + 1;
        }
        idx = idx + 1;
    }
    proof {
        assert(branch@.subrange(0, MAP_SIZE as int) =~= branch@);
        assert(global@.subrange(0, MAP_SIZE as int) =~= global@);
    }
    count
}

// r[impl chaoscontrol-coverage.classify]
/// AFL bucket classification, applied in place.
pub fn classify_bitmap(bm: &mut Vec<u8>)
    requires valid_coverage_bitmap_spec(old(bm)@)
    ensures
        valid_coverage_bitmap_spec(bm@),
        forall|i: int| 0 <= i < MAP_SIZE as int ==>
            bm@[i] == classify_bucket_spec(old(bm)@[i]),
{
    let mut idx: usize = 0;
    while idx < MAP_SIZE
        invariant
            0 <= idx <= MAP_SIZE,
            bm@.len() == MAP_SIZE as int,
            forall|j: int| 0 <= j < idx as int ==>
                bm@[j] == classify_bucket_spec(old(bm)@[j]),
            forall|j: int| idx as int <= j < MAP_SIZE as int ==>
                bm@[j] == old(bm)@[j],
        decreases MAP_SIZE - idx,
    {
        let c = bm[idx];
        let bucket: u8 = if c == 0 { 0 }
            else if c == 1 { 1 }
            else if c == 2 { 2 }
            else if c == 3 { 3 }
            else if c <= 7 { 4 }
            else if c <= 15 { 8 }
            else if c <= 31 { 16 }
            else if c <= 127 { 32 }
            else { 128 };
        bm.set(idx, bucket);
        idx = idx + 1;
    }
}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bitmap_is_zeroed() {
        let bm = new_coverage_bitmap();
        assert_eq!(bm.len(), 65536);
        assert!(bm.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_region_boundaries() {
        assert_eq!(CODE_REGION_END, 32768);
        assert_eq!(ASSERTION_REGION_END, 49152);
        assert_eq!(MAP_SIZE, 65536);
        assert_eq!(
            CODE_REGION_END + (ASSERTION_REGION_END - CODE_REGION_END) + (MAP_SIZE - ASSERTION_REGION_END),
            MAP_SIZE
        );
    }

    #[test]
    fn test_record_hit_isolation() {
        let mut bm = new_coverage_bitmap();
        let snapshot: Vec<u8> = bm.clone();

        // Code region hit
        record_code_hit(&mut bm, 100);
        assert_eq!(bm[100], 1);
        // Assertion region unchanged
        for i in CODE_REGION_END..ASSERTION_REGION_END {
            assert_eq!(bm[i], 0);
        }
        // Schedule region unchanged
        for i in ASSERTION_REGION_END..MAP_SIZE {
            assert_eq!(bm[i], 0);
        }

        // Assertion region hit
        record_assertion_hit(&mut bm, 40000);
        assert_eq!(bm[40000], 1);
        // Code region only has the previous hit
        assert_eq!(bm[100], 1);
        for i in 0..CODE_REGION_END {
            if i != 100 {
                assert_eq!(bm[i], 0, "code region index {i} changed");
            }
        }

        // Schedule region hit
        record_schedule_hit(&mut bm, 60000);
        assert_eq!(bm[60000], 1);
    }

    #[test]
    fn test_hit_saturates() {
        let mut bm = new_coverage_bitmap();
        for _ in 0_i32..300_i32 {
            record_code_hit(&mut bm, 0);
        }
        assert_eq!(bm[0], 255);
    }

    #[test]
    fn test_merge_pointwise_max() {
        let mut a = new_coverage_bitmap();
        let mut b = new_coverage_bitmap();
        a[10] = 3;
        b[10] = 1;
        a[20] = 0;
        b[20] = 5;
        merge_coverage(&mut a, &b);
        assert_eq!(a[10], 3);
        assert_eq!(a[20], 5);
    }

    #[test]
    fn test_new_edge_count() {
        let mut branch = new_coverage_bitmap();
        let global = new_coverage_bitmap();
        branch[0] = 1;
        branch[1] = 5;
        branch[2] = 0;
        let count = count_new_edges(&branch, &global);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_new_edge_excludes_already_global() {
        let mut branch = new_coverage_bitmap();
        let mut global = new_coverage_bitmap();
        branch[0] = 1;
        global[0] = 1;
        branch[1] = 3;
        global[1] = 0;
        let count = count_new_edges(&branch, &global);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_classify_buckets() {
        let cases: Vec<(u8, u8)> = vec![
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 4),
            (7, 4),
            (8, 8),
            (10, 8),
            (15, 8),
            (16, 16),
            (20, 16),
            (31, 16),
            (32, 32),
            (50, 32),
            (127, 32),
            (128, 128),
            (200, 128),
            (255, 128),
        ];
        let mut bm = new_coverage_bitmap();
        for (i, &(input, _)) in cases.iter().enumerate() {
            bm[i] = input;
        }
        classify_bitmap(&mut bm);
        for (i, &(_, expected)) in cases.iter().enumerate() {
            assert_eq!(bm[i], expected, "classify({}) = {} but expected {}", cases[i].0, bm[i], expected);
        }
    }
}
