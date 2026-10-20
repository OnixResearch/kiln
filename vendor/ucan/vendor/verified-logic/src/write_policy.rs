//! Verified write-back and write-through cache write policies.
//!
//! Models per-line policy state, dirty-bit evolution, eviction writeback,
//! write-through propagation, mixed-policy non-interference, and bulk flush.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WritePolicy {
    WriteBack,
    WriteThrough,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CacheLine {
    pub data: u64,
    pub dirty: bool,
    pub policy: WritePolicy,
}

fn clone_memory(memory: &[u64]) -> (result: Vec<u64>)
    ensures result@ == memory@
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < memory.len()
        invariant
            0 <= i <= memory@.len(),
            result@ == memory@.subrange(0, i as int),
        decreases memory@.len() - i,
    {
        result.push(memory[i]);
        i += 1;
    }
    result
}

fn clone_lines(lines: &[CacheLine]) -> (result: Vec<CacheLine>)
    ensures result@ == lines@
{
    let mut result: Vec<CacheLine> = Vec::new();
    let mut i: usize = 0;
    while i < lines.len()
        invariant
            0 <= i <= lines@.len(),
            result@ == lines@.subrange(0, i as int),
        decreases lines@.len() - i,
    {
        result.push(lines[i]);
        i += 1;
    }
    result
}

// r[depends write-policy.write-back-dirty]
/// Spec: freshly loaded line is clean under either policy.
pub open spec fn load_line_spec(data: u64, policy: WritePolicy) -> CacheLine {
    CacheLine { data, dirty: false, policy }
}

// r[impl write-policy.write-back-dirty]
// r[impl write-policy.write-through-clean]
/// Returns a freshly loaded clean line.
pub fn load_line(data: u64, policy: WritePolicy) -> (result: CacheLine)
    ensures result == load_line_spec(data, policy)
{
    CacheLine { data, dirty: false, policy }
}

// r[depends write-policy.write-back-dirty]
/// Spec: write-back store updates data and sets the dirty bit.
pub open spec fn write_back_store_spec(line: CacheLine, new_data: u64) -> CacheLine
    recommends line.policy == WritePolicy::WriteBack
{
    CacheLine { data: new_data, dirty: true, policy: line.policy }
}

// r[impl write-policy.write-back-dirty]
/// Write-back store updates data and records deferred writeback.
pub fn write_back_store(line: CacheLine, new_data: u64) -> (result: CacheLine)
    requires line.policy == WritePolicy::WriteBack
    ensures result == write_back_store_spec(line, new_data)
{
    CacheLine { data: new_data, dirty: true, policy: line.policy }
}

// r[depends write-policy.write-back-eviction]
/// Spec: evicting a line writes back only when it is dirty.
pub open spec fn evict_line_spec(line: CacheLine, addr: int, memory: Seq<u64>) -> Seq<u64>
    recommends 0 <= addr < memory.len()
{
    if line.dirty { memory.update(addr, line.data) } else { memory }
}

// r[impl write-policy.write-back-eviction]
/// Evicts one line into memory.
pub fn evict_line(line: CacheLine, addr: usize, memory: &[u64]) -> (result: Vec<u64>)
    requires addr < memory@.len()
    ensures result@ == evict_line_spec(line, addr as int, memory@)
{
    let mut result = clone_memory(memory);
    if line.dirty {
        result.set(addr, line.data);
    }
    result
}

// r[depends write-policy.write-through-clean]
/// Spec: write-through store updates cache data and stays clean.
pub open spec fn write_through_store_line_spec(line: CacheLine, new_data: u64) -> CacheLine
    recommends line.policy == WritePolicy::WriteThrough
{
    CacheLine { data: new_data, dirty: false, policy: line.policy }
}

// r[depends write-policy.write-through-memory]
/// Spec: write-through store updates memory immediately.
pub open spec fn write_through_store_mem_spec(line: CacheLine, new_data: u64, addr: int, memory: Seq<u64>) -> Seq<u64>
    recommends line.policy == WritePolicy::WriteThrough, 0 <= addr < memory.len()
{
    memory.update(addr, new_data)
}

// r[impl write-policy.write-through-clean]
// r[impl write-policy.write-through-memory]
/// Write-through store updates cache and memory together.
pub fn write_through_store(line: CacheLine, new_data: u64, addr: usize, memory: &[u64]) -> (result: (CacheLine, Vec<u64>))
    requires line.policy == WritePolicy::WriteThrough, addr < memory@.len()
    ensures
        result.0 == write_through_store_line_spec(line, new_data),
        result.1@ == write_through_store_mem_spec(line, new_data, addr as int, memory@)
{
    let new_line = CacheLine { data: new_data, dirty: false, policy: line.policy };
    let mut new_memory = clone_memory(memory);
    new_memory.set(addr, new_data);
    (new_line, new_memory)
}

// r[depends write-policy.policy-non-interference]
/// Spec: update one write-back line in a cache snapshot.
pub open spec fn write_back_line_at_spec(lines: Seq<CacheLine>, idx: int, new_data: u64) -> Seq<CacheLine>
    recommends 0 <= idx < lines.len(), lines[idx].policy == WritePolicy::WriteBack
{
    lines.update(idx, write_back_store_spec(lines[idx], new_data))
}

// r[impl write-policy.policy-non-interference]
/// Updates one write-back line in a cache snapshot.
pub fn write_back_line_at(lines: &[CacheLine], idx: usize, new_data: u64) -> (result: Vec<CacheLine>)
    requires idx < lines@.len(), lines@[idx as int].policy == WritePolicy::WriteBack
    ensures result@ == write_back_line_at_spec(lines@, idx as int, new_data)
{
    let mut result = clone_lines(lines);
    result.set(idx, write_back_store(lines[idx], new_data));
    result
}

// r[depends write-policy.policy-non-interference]
/// Spec: update one write-through line in a cache snapshot.
pub open spec fn write_through_line_at_spec(lines: Seq<CacheLine>, idx: int, new_data: u64) -> Seq<CacheLine>
    recommends 0 <= idx < lines.len(), lines[idx].policy == WritePolicy::WriteThrough
{
    lines.update(idx, write_through_store_line_spec(lines[idx], new_data))
}

// r[impl write-policy.policy-non-interference]
/// Updates one write-through line in a cache snapshot.
pub fn write_through_line_at(lines: &[CacheLine], idx: usize, new_data: u64) -> (result: Vec<CacheLine>)
    requires idx < lines@.len(), lines@[idx as int].policy == WritePolicy::WriteThrough
    ensures result@ == write_through_line_at_spec(lines@, idx as int, new_data)
{
    let mut result = clone_lines(lines);
    result.set(idx, CacheLine { data: new_data, dirty: false, policy: lines[idx].policy });
    result
}

/// Spec: clear dirty bits in the first `upto` lines, leaving later lines unchanged.
pub open spec fn flush_progress_lines_spec(lines: Seq<CacheLine>, upto: nat) -> Seq<CacheLine>
    recommends upto <= lines.len()
    decreases upto
{
    if upto == 0 {
        lines
    } else {
        let prev = flush_progress_lines_spec(lines, (upto - 1) as nat);
        prev.update((upto - 1) as int, CacheLine {
            data: lines[(upto - 1) as int].data,
            dirty: false,
            policy: lines[(upto - 1) as int].policy,
        })
    }
}

/// Spec: flush prefix `[0, upto)` into memory.
pub open spec fn flush_prefix_memory_spec(lines: Seq<CacheLine>, memory: Seq<u64>, upto: nat) -> Seq<u64>
    recommends upto <= lines.len(), lines.len() == memory.len()
    decreases upto
{
    if upto == 0 {
        memory
    } else {
        let prev = flush_prefix_memory_spec(lines, memory, (upto - 1) as nat);
        if lines[(upto - 1) as int].dirty {
            prev.update((upto - 1) as int, lines[(upto - 1) as int].data)
        } else {
            prev
        }
    }
}

/// Cache/memory relation needed before flushing: clean lines already match memory.
pub open spec fn flushable_cache_spec(lines: Seq<CacheLine>, memory: Seq<u64>) -> bool {
    &&& lines.len() == memory.len()
    &&& forall|i: int| 0 <= i < lines.len() ==> (!lines[i].dirty ==> memory[i] == lines[i].data)
}

// r[depends write-policy.flush-all]
/// Spec: flush all lines and clear every dirty bit.
pub open spec fn flush_all_lines_spec(lines: Seq<CacheLine>) -> Seq<CacheLine> {
    flush_progress_lines_spec(lines, lines.len())
}

// r[depends write-policy.flush-all]
/// Spec: memory after flushing all dirty lines.
pub open spec fn flush_all_memory_spec(lines: Seq<CacheLine>, memory: Seq<u64>) -> Seq<u64>
    recommends flushable_cache_spec(lines, memory)
{
    flush_prefix_memory_spec(lines, memory, lines.len())
}

// r[impl write-policy.flush-all]
/// Flushes all dirty lines into memory and clears every dirty bit.
pub fn flush_all(lines: &[CacheLine], memory: &[u64]) -> (result: (Vec<CacheLine>, Vec<u64>))
    requires flushable_cache_spec(lines@, memory@)
    ensures
        result.0@ == flush_all_lines_spec(lines@),
        result.1@ == flush_all_memory_spec(lines@, memory@)
{
    let mut new_lines = clone_lines(lines);
    let mut new_memory = clone_memory(memory);
    let mut i: usize = 0;
    while i < lines.len()
        invariant
            0 <= i <= lines@.len(),
            lines@.len() == memory@.len(),
            new_lines@.len() == lines@.len(),
            new_memory@.len() == memory@.len(),
            new_lines@ == flush_progress_lines_spec(lines@, i as nat),
            new_memory@ == flush_prefix_memory_spec(lines@, memory@, i as nat),
        decreases lines@.len() - i,
    {
        assert(i < new_memory@.len());
        assert(i < new_lines@.len());
        let line = lines[i];
        if line.dirty {
            new_memory.set(i, line.data);
        }
        new_lines.set(i, CacheLine { data: line.data, dirty: false, policy: line.policy });
        i += 1;
    }
    (new_lines, new_memory)
}

// r[impl write-policy.read-equivalence]
/// Spec: read hit returns the cached data.
pub open spec fn read_hit_spec(line: CacheLine) -> u64 {
    line.data
}

// r[impl write-policy.read-equivalence]
/// Read hit returns cached data regardless of policy.
pub fn read_hit(line: CacheLine) -> (result: u64)
    ensures result == read_hit_spec(line)
{
    line.data
}

// --- Proofs ---

// r[verify write-policy.write-back-dirty]
proof fn load_line_is_clean(data: u64)
    ensures
        !load_line_spec(data, WritePolicy::WriteBack).dirty,
        !load_line_spec(data, WritePolicy::WriteThrough).dirty,
{}

// r[verify write-policy.write-back-dirty]
proof fn write_back_store_sets_dirty(line: CacheLine, new_data: u64)
    requires line.policy == WritePolicy::WriteBack
    ensures write_back_store_spec(line, new_data).dirty
{}

// r[verify write-policy.write-back-eviction]
proof fn dirty_eviction_writes_back(line: CacheLine, addr: int, memory: Seq<u64>)
    requires line.dirty, 0 <= addr < memory.len()
    ensures evict_line_spec(line, addr, memory)[addr] == line.data
{}

// r[verify write-policy.write-back-eviction]
proof fn clean_eviction_leaves_memory(line: CacheLine, addr: int, memory: Seq<u64>)
    requires !line.dirty, 0 <= addr < memory.len()
    ensures evict_line_spec(line, addr, memory) == memory
{}

// r[verify write-policy.write-through-clean]
proof fn write_through_stays_clean(line: CacheLine, new_data: u64)
    requires line.policy == WritePolicy::WriteThrough
    ensures !write_through_store_line_spec(line, new_data).dirty
{}

// r[verify write-policy.write-through-memory]
proof fn write_through_updates_memory(line: CacheLine, new_data: u64, addr: int, memory: Seq<u64>)
    requires line.policy == WritePolicy::WriteThrough, 0 <= addr < memory.len()
    ensures
        write_through_store_mem_spec(line, new_data, addr, memory)[addr] == new_data,
        write_through_store_mem_spec(line, new_data, addr, memory)[addr]
            == write_through_store_line_spec(line, new_data).data,
{}

// r[verify write-policy.policy-non-interference]
proof fn write_back_preserves_other_lines(lines: Seq<CacheLine>, idx: int, other: int, new_data: u64)
    requires
        0 <= idx < lines.len(),
        0 <= other < lines.len(),
        idx != other,
        lines[idx].policy == WritePolicy::WriteBack,
    ensures write_back_line_at_spec(lines, idx, new_data)[other] == lines[other]
{}

// r[verify write-policy.policy-non-interference]
proof fn write_through_preserves_other_lines(lines: Seq<CacheLine>, idx: int, other: int, new_data: u64)
    requires
        0 <= idx < lines.len(),
        0 <= other < lines.len(),
        idx != other,
        lines[idx].policy == WritePolicy::WriteThrough,
    ensures write_through_line_at_spec(lines, idx, new_data)[other] == lines[other]
{}


// r[verify write-policy.read-equivalence]
proof fn read_hit_ignores_policy(data: u64, dirty: bool)
    ensures
        read_hit_spec(CacheLine { data, dirty, policy: WritePolicy::WriteBack })
            == read_hit_spec(CacheLine { data, dirty: false, policy: WritePolicy::WriteThrough }),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify write-policy.write-back-dirty]
    #[test]
    fn test_write_back_store_and_load() {
        let line = load_line(5, WritePolicy::WriteBack);
        assert!(!line.dirty);
        let dirty = write_back_store(line, 9);
        assert_eq!(dirty.data, 9);
        assert!(dirty.dirty);
        assert_eq!(dirty.policy, WritePolicy::WriteBack);
    }

    // r[verify write-policy.write-back-eviction]
    #[test]
    fn test_evict_line() {
        let dirty = CacheLine {
            data: 11,
            dirty: true,
            policy: WritePolicy::WriteBack,
        };
        let clean = CacheLine {
            data: 7,
            dirty: false,
            policy: WritePolicy::WriteBack,
        };
        assert_eq!(evict_line(dirty, 1, &[1, 2, 3]), vec![1, 11, 3]);
        assert_eq!(evict_line(clean, 1, &[1, 2, 3]), vec![1, 2, 3]);
    }

    // r[verify write-policy.write-through-clean]
    // r[verify write-policy.write-through-memory]
    #[test]
    fn test_write_through_store() {
        let line = load_line(4, WritePolicy::WriteThrough);
        let (new_line, memory) = write_through_store(line, 13, 0, &[4, 9]);
        assert_eq!(new_line.data, 13);
        assert!(!new_line.dirty);
        assert_eq!(memory, vec![13, 9]);
    }

    // r[verify write-policy.flush-all]
    #[test]
    fn test_flush_all() {
        let lines = vec![
            CacheLine {
                data: 5,
                dirty: true,
                policy: WritePolicy::WriteBack,
            },
            CacheLine {
                data: 7,
                dirty: false,
                policy: WritePolicy::WriteThrough,
            },
            CacheLine {
                data: 9,
                dirty: true,
                policy: WritePolicy::WriteBack,
            },
        ];
        let memory = vec![1, 7, 3];
        let (new_lines, new_memory) = flush_all(&lines, &memory);
        assert_eq!(new_memory, vec![5, 7, 9]);
        assert!(new_lines.iter().all(|line| !line.dirty));
    }

    // r[verify write-policy.read-equivalence]
    #[test]
    fn test_read_hit() {
        let wb = CacheLine {
            data: 42,
            dirty: true,
            policy: WritePolicy::WriteBack,
        };
        let wt = CacheLine {
            data: 42,
            dirty: false,
            policy: WritePolicy::WriteThrough,
        };
        assert_eq!(read_hit(wb), 42);
        assert_eq!(read_hit(wt), 42);
    }

    proptest! {
        // r[verify write-policy.write-back-dirty]
        #[test]
        fn prop_write_back_always_dirty(data: u64, new_data: u64) {
            let line = load_line(data, WritePolicy::WriteBack);
            let updated = write_back_store(line, new_data);
            prop_assert!(updated.dirty);
            prop_assert_eq!(updated.data, new_data);
        }

        // r[verify write-policy.write-through-memory]
        #[test]
        fn prop_write_through_memory_matches_cache(data: u64, new_data: u64, other: u64) {
            let line = load_line(data, WritePolicy::WriteThrough);
            let (updated, memory) = write_through_store(line, new_data, 0, &[data, other]);
            prop_assert!(!updated.dirty);
            prop_assert_eq!(memory[0], updated.data);
            prop_assert_eq!(memory[1], other);
        }

        // r[verify write-policy.policy-non-interference]
        #[test]
        fn prop_write_back_preserves_other_slot(a: u64, b: u64, new_data: u64) {
            let lines = vec![
                CacheLine { data: a, dirty: false, policy: WritePolicy::WriteBack },
                CacheLine { data: b, dirty: false, policy: WritePolicy::WriteThrough },
            ];
            let updated = write_back_line_at(&lines, 0, new_data);
            prop_assert_eq!(updated[1], lines[1]);
        }

        // r[verify write-policy.flush-all]
        #[test]
        fn prop_flush_all_cleans(entries: Vec<(u64, bool)>) {
            let mut lines = Vec::new();
            let mut memory = Vec::new();
            for (i, (data, dirty_flag)) in entries.iter().copied().enumerate() {
                let policy = if i % 2 == 0 { WritePolicy::WriteBack } else { WritePolicy::WriteThrough };
                let dirty = if policy == WritePolicy::WriteThrough { false } else { dirty_flag };
                lines.push(CacheLine { data, dirty, policy });
                if dirty {
                    memory.push(data.wrapping_add(1));
                } else {
                    memory.push(data);
                }
            }
            let (new_lines, new_memory) = flush_all(&lines, &memory);
            prop_assert!(new_lines.iter().all(|line| !line.dirty));
            for i in 0..new_lines.len() {
                prop_assert_eq!(new_memory[i], new_lines[i].data);
            }
        }
    }
}
