//! Verified fixed-capacity store buffer with store-to-load forwarding.
//!
//! The model keeps the buffer as a FIFO sequence whose head is the oldest
//! store and whose tail is the youngest. Loads scan from tail to head and may
//! forward only from the youngest store that fully covers the load byte range.
//! A store fence drains the whole buffer before any subsequent store enters.

use vstd::prelude::*;

verus! {

/// One-byte store/load width.
pub const STORE_SIZE_BYTE: u8 = 1u8;
/// Two-byte store/load width.
pub const STORE_SIZE_HALFWORD: u8 = 2u8;
/// Four-byte store/load width.
pub const STORE_SIZE_WORD: u8 = 4u8;
/// Eight-byte store/load width.
pub const STORE_SIZE_DOUBLEWORD: u8 = 8u8;

/// Fixed model capacity for the store buffer.
pub const STORE_BUFFER_CAPACITY: usize = 8usize;

/// A load is served from the store buffer.
pub const LOAD_SOURCE_BUFFER: u8 = 0u8;
/// A load is served from the cache.
pub const LOAD_SOURCE_CACHE: u8 = 1u8;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StoreEntry {
    pub addr: u64,
    pub size: u8,
    pub data: u64,
    pub seq: u64,
}

// r[depends store-buffer.full-coverage]
// r[depends store-buffer.capacity]
pub open spec fn valid_store_size_spec(size: u8) -> bool {
    size == STORE_SIZE_BYTE
        || size == STORE_SIZE_HALFWORD
        || size == STORE_SIZE_WORD
        || size == STORE_SIZE_DOUBLEWORD
}

// r[impl store-buffer.full-coverage]
pub fn valid_store_size(size: u8) -> (result: bool)
    ensures result == valid_store_size_spec(size),
{
    size == STORE_SIZE_BYTE
        || size == STORE_SIZE_HALFWORD
        || size == STORE_SIZE_WORD
        || size == STORE_SIZE_DOUBLEWORD
}

pub open spec fn entry_valid_spec(entry: StoreEntry) -> bool {
    valid_store_size_spec(entry.size)
}

pub open spec fn strictly_increasing_seq_spec(buffer: Seq<StoreEntry>) -> bool {
    forall|i: int, j: int|
        #![trigger buffer[i], buffer[j]]
        0 <= i < j < buffer.len() ==> buffer[i].seq < buffer[j].seq
}

pub open spec fn buffer_valid_spec(buffer: Seq<StoreEntry>) -> bool {
    &&& buffer.len() <= STORE_BUFFER_CAPACITY as int
    &&& forall|i: int| #![trigger buffer[i]] 0 <= i < buffer.len() ==> entry_valid_spec(buffer[i])
    &&& strictly_increasing_seq_spec(buffer)
}

pub open spec fn fresh_entry_spec(buffer: Seq<StoreEntry>, entry: StoreEntry) -> bool {
    &&& entry_valid_spec(entry)
    &&& if buffer.len() == 0 {
        true
    } else {
        buffer[buffer.len() - 1].seq < entry.seq
    }
}

// r[depends store-buffer.full-coverage]
pub open spec fn can_forward_spec(entry: StoreEntry, load_addr: u64, load_size: u8) -> bool {
    &&& entry_valid_spec(entry)
    &&& valid_store_size_spec(load_size)
    &&& entry.addr <= load_addr
    &&& (load_size as int) <= (entry.size as int)
    &&& (load_addr as int) - (entry.addr as int) <= (entry.size as int) - (load_size as int)
}

pub open spec fn has_forward_match_spec(buffer: Seq<StoreEntry>, load_addr: u64, load_size: u8) -> bool {
    exists|i: int| #![trigger buffer[i]] 0 <= i < buffer.len() && can_forward_spec(buffer[i], load_addr, load_size)
}

pub open spec fn no_forward_match_in_suffix_spec(
    buffer: Seq<StoreEntry>,
    load_addr: u64,
    load_size: u8,
    from: int,
) -> bool {
    forall|i: int| #![trigger buffer[i]] from <= i < buffer.len() ==> !can_forward_spec(buffer[i], load_addr, load_size)
}

pub open spec fn load_source_spec(buffer: Seq<StoreEntry>, load_addr: u64, load_size: u8) -> u8 {
    if has_forward_match_spec(buffer, load_addr, load_size) {
        LOAD_SOURCE_BUFFER
    } else {
        LOAD_SOURCE_CACHE
    }
}

// r[impl store-buffer.fence]
pub open spec fn is_fence_satisfied_spec(buffer: Seq<StoreEntry>) -> bool {
    buffer.len() == 0
}

proof fn push_preserves_buffer_validity(buffer: Seq<StoreEntry>, entry: StoreEntry)
    requires
        buffer_valid_spec(buffer),
        buffer.len() < STORE_BUFFER_CAPACITY as int,
        fresh_entry_spec(buffer, entry),
    ensures
        buffer_valid_spec(buffer.push(entry)),
{
    let extended = buffer.push(entry);
    assert(extended.len() == buffer.len() + 1);
    assert(extended.len() <= STORE_BUFFER_CAPACITY as int);
    assert forall|i: int| #![trigger extended[i]] 0 <= i < extended.len() implies entry_valid_spec(extended[i]) by {
        if i < buffer.len() {
            assert(extended[i] == buffer[i]);
        } else {
            assert(i == buffer.len());
            assert(extended[i] == entry);
        }
    };
    assert forall|i: int, j: int| #![trigger extended[i], extended[j]] 0 <= i < j < extended.len()
        implies extended[i].seq < extended[j].seq by {
        if j < buffer.len() {
            assert(extended[i] == buffer[i]);
            assert(extended[j] == buffer[j]);
        } else {
            assert(j == buffer.len());
            assert(extended[j] == entry);
            assert(extended[i] == buffer[i]);
            if buffer.len() > 0 {
                let last = buffer.len() - 1;
                assert(i <= last);
                if i < last {
                    assert(buffer[i].seq < buffer[last].seq);
                }
                assert(buffer[last].seq < entry.seq);
            }
        }
    };
}

proof fn suffix_preserves_buffer_validity(buffer: Seq<StoreEntry>, from: int)
    requires
        buffer_valid_spec(buffer),
        0 <= from <= buffer.len(),
    ensures
        buffer_valid_spec(buffer.subrange(from, buffer.len() as int)),
{
    let suffix = buffer.subrange(from, buffer.len() as int);
    assert(suffix.len() <= STORE_BUFFER_CAPACITY as int);
    assert forall|i: int| #![trigger suffix[i]] 0 <= i < suffix.len() implies entry_valid_spec(suffix[i]) by {
        assert(suffix[i] == buffer[from + i]);
    };
    assert forall|i: int, j: int| #![trigger suffix[i], suffix[j]] 0 <= i < j < suffix.len()
        implies suffix[i].seq < suffix[j].seq by {
        assert(suffix[i] == buffer[from + i]);
        assert(suffix[j] == buffer[from + j]);
    };
}

// r[verify store-buffer.youngest-match]
proof fn youngest_match_has_greatest_seq(buffer: Seq<StoreEntry>, load_addr: u64, load_size: u8, index: int)
    requires
        buffer_valid_spec(buffer),
        0 <= index < buffer.len(),
        can_forward_spec(buffer[index], load_addr, load_size),
        forall|j: int| #![trigger buffer[j]] index < j < buffer.len() ==> !can_forward_spec(buffer[j], load_addr, load_size),
    ensures
        forall|j: int| #![trigger buffer[j]] 0 <= j < buffer.len() && can_forward_spec(buffer[j], load_addr, load_size)
            ==> buffer[j].seq <= buffer[index].seq,
{
}

// r[verify store-buffer.cache-fallback]
proof fn no_match_uses_cache(buffer: Seq<StoreEntry>, load_addr: u64, load_size: u8)
    requires
        !has_forward_match_spec(buffer, load_addr, load_size),
    ensures
        load_source_spec(buffer, load_addr, load_size) == LOAD_SOURCE_CACHE,
{
}

// r[verify store-buffer.fence]
proof fn empty_buffer_satisfies_fence()
    ensures
        is_fence_satisfied_spec(seq![]),
{
}

fn clone_buffer(buffer: &[StoreEntry]) -> (result: Vec<StoreEntry>)
    ensures result@ =~= buffer@,
{
    let mut result: Vec<StoreEntry> = Vec::new();
    let mut index: usize = 0;
    while index < buffer.len()
        invariant
            0 <= index <= buffer@.len(),
            result@ =~= buffer@.subrange(0, index as int),
        decreases buffer@.len() - index,
    {
        result.push(buffer[index]);
        index = index + 1;
    }
    result
}

fn clone_from(buffer: &[StoreEntry], from: usize) -> (result: Vec<StoreEntry>)
    requires from <= buffer@.len(),
    ensures result@ =~= buffer@.subrange(from as int, buffer@.len() as int),
{
    let mut result: Vec<StoreEntry> = Vec::new();
    let mut index: usize = from;
    while index < buffer.len()
        invariant
            from <= index <= buffer@.len(),
            result@ =~= buffer@.subrange(from as int, index as int),
        decreases buffer@.len() - index,
    {
        result.push(buffer[index]);
        index = index + 1;
    }
    result
}

// r[impl store-buffer.capacity]
pub fn enqueue(buffer: &[StoreEntry], entry: StoreEntry) -> (result: Option<Vec<StoreEntry>>)
    requires
        buffer_valid_spec(buffer@),
        fresh_entry_spec(buffer@, entry),
    ensures
        result.is_some() ==> {
            &&& buffer@.len() < STORE_BUFFER_CAPACITY as int
            &&& result.unwrap()@ == buffer@.push(entry)
            &&& buffer_valid_spec(result.unwrap()@)
        },
        result.is_none() ==> buffer@.len() == STORE_BUFFER_CAPACITY as int,
{
    if buffer.len() == STORE_BUFFER_CAPACITY {
        return None;
    }
    let mut next = clone_buffer(buffer);
    next.push(entry);
    proof {
        push_preserves_buffer_validity(buffer@, entry);
    }
    Some(next)
}

// r[impl store-buffer.drain-ordering]
pub fn drain_oldest(buffer: &[StoreEntry]) -> (result: Option<(StoreEntry, Vec<StoreEntry>)>)
    requires buffer_valid_spec(buffer@),
    ensures
        result.is_some() ==> {
            &&& buffer@.len() > 0
            &&& result.unwrap().0 == buffer@[0]
            &&& result.unwrap().1@ == buffer@.subrange(1, buffer@.len() as int)
            &&& buffer_valid_spec(result.unwrap().1@)
        },
        result.is_none() ==> buffer@.len() == 0,
{
    if buffer.len() == 0 {
        return None;
    }
    let head = buffer[0];
    let rest = clone_from(buffer, 1);
    proof {
        suffix_preserves_buffer_validity(buffer@, 1);
    }
    Some((head, rest))
}

// r[impl store-buffer.drain-ordering]
pub fn drain_all(buffer: &[StoreEntry]) -> (result: Vec<StoreEntry>)
    requires buffer_valid_spec(buffer@),
    ensures result@ =~= buffer@,
{
    clone_buffer(buffer)
}

// r[impl store-buffer.full-coverage]
pub fn can_forward(entry: StoreEntry, load_addr: u64, load_size: u8) -> (result: bool)
    requires
        entry_valid_spec(entry),
        valid_store_size_spec(load_size),
    ensures result == can_forward_spec(entry, load_addr, load_size),
{
    let store_size = entry.size as u64;
    let load_size_u64 = load_size as u64;
    entry.addr <= load_addr
        && load_size_u64 <= store_size
        && load_addr - entry.addr <= store_size - load_size_u64
}

// r[impl store-buffer.youngest-match]
pub fn forward_lookup(buffer: &[StoreEntry], load_addr: u64, load_size: u8) -> (result: Option<StoreEntry>)
    requires
        buffer_valid_spec(buffer@),
        valid_store_size_spec(load_size),
    ensures
        result.is_some() ==> {
            exists|i: int| #![trigger buffer@[i]]
                0 <= i < buffer@.len()
                    && result.unwrap() == buffer@[i]
                    && can_forward_spec(buffer@[i], load_addr, load_size)
                    && forall|j: int| #![trigger buffer@[j]] i < j < buffer@.len() ==> !can_forward_spec(buffer@[j], load_addr, load_size)
        },
        result.is_none() ==> forall|i: int| #![trigger buffer@[i]] 0 <= i < buffer@.len() ==> !can_forward_spec(buffer@[i], load_addr, load_size),
{
    let mut index: usize = buffer.len();
    while index > 0
        invariant
            buffer_valid_spec(buffer@),
            valid_store_size_spec(load_size),
            0 <= index <= buffer@.len(),
            no_forward_match_in_suffix_spec(buffer@, load_addr, load_size, index as int),
        decreases index,
    {
        let candidate_index = index - 1;
        let candidate = buffer[candidate_index];
        proof {
            let candidate_index_int = candidate_index as int;
            assert(0 <= candidate_index_int);
            assert(candidate_index_int < buffer@.len());
            assert(entry_valid_spec(buffer@[candidate_index_int]));
            assert(candidate == buffer@[candidate_index_int]);
            assert(entry_valid_spec(candidate));
        }
        if can_forward(candidate, load_addr, load_size) {
            proof {
                assert(candidate == buffer@[candidate_index as int]);
                assert(forall|j: int| #![trigger buffer@[j]] (candidate_index as int) < j && j < buffer@.len() ==> !can_forward_spec(buffer@[j], load_addr, load_size));
            }
            return Some(candidate);
        }
        proof {
            assert(!can_forward_spec(buffer@[candidate_index as int], load_addr, load_size));
        }
        index = candidate_index;
    }
    None
}

// r[impl store-buffer.cache-fallback]
// r[impl store-buffer.youngest-match]
pub fn load_source(buffer: &[StoreEntry], load_addr: u64, load_size: u8) -> (result: u8)
    requires
        buffer_valid_spec(buffer@),
        valid_store_size_spec(load_size),
    ensures result == load_source_spec(buffer@, load_addr, load_size),
{
    if forward_lookup(buffer, load_addr, load_size).is_some() {
        LOAD_SOURCE_BUFFER
    } else {
        LOAD_SOURCE_CACHE
    }
}

// r[impl store-buffer.fence]
pub fn is_fence_satisfied(buffer: &[StoreEntry]) -> (result: bool)
    requires buffer_valid_spec(buffer@),
    ensures result == is_fence_satisfied_spec(buffer@),
{
    buffer.len() == 0
}

// r[impl store-buffer.fence]
pub fn store_fence(buffer: &[StoreEntry]) -> (result: Vec<StoreEntry>)
    requires buffer_valid_spec(buffer@),
    ensures
        result@.len() == 0,
        buffer_valid_spec(result@),
        is_fence_satisfied_spec(result@),
{
    let result: Vec<StoreEntry> = Vec::new();
    proof {
        empty_buffer_satisfies_fence();
    }
    result
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const TEST_ADDR_BASE: u64 = 100u64;
    const TEST_ADDR_OFFSET: u64 = 1u64;
    const TEST_ADDR_OTHER: u64 = 200u64;
    const TEST_DATA_OLDER: u64 = 0x11u64;
    const TEST_DATA_YOUNGER: u64 = 0x22u64;
    const TEST_DATA_CACHE: u64 = 0x33u64;
    const TEST_SEQ_OLDER: u64 = 1u64;
    const TEST_SEQ_YOUNGER: u64 = 2u64;
    const TEST_SEQ_THIRD: u64 = 3u64;
    const TEST_SIZE_TAG_COUNT: u8 = 4u8;
    const TEST_MAX_RANDOM_ADDR: u64 = 256u64;
    const TEST_RANDOM_BUFFER_FACTOR: usize = 2usize;

    fn entry(addr: u64, size: u8, data: u64, seq: u64) -> StoreEntry {
        StoreEntry { addr, size, data, seq }
    }

    fn size_from_tag(tag: u8) -> u8 {
        match tag {
            0 => STORE_SIZE_BYTE,
            1 => STORE_SIZE_HALFWORD,
            2 => STORE_SIZE_WORD,
            _ => STORE_SIZE_DOUBLEWORD,
        }
    }

    fn make_random_buffer(specs: &[(u64, u8, u64)]) -> Vec<StoreEntry> {
        let mut buffer = Vec::new();
        let mut index: usize = 0;
        while index < specs.len() && index < STORE_BUFFER_CAPACITY {
            let (addr, size_tag, data) = specs[index];
            buffer.push(entry(
                addr % TEST_MAX_RANDOM_ADDR,
                size_from_tag(size_tag % TEST_SIZE_TAG_COUNT),
                data,
                index as u64,
            ));
            index += 1;
        }
        buffer
    }

    fn oracle_can_forward(entry: StoreEntry, load_addr: u64, load_size: u8) -> bool {
        let store_size = entry.size as u64;
        let load_size_u64 = load_size as u64;
        entry.addr <= load_addr && load_size_u64 <= store_size && load_addr - entry.addr <= store_size - load_size_u64
    }

    fn oracle_forward_lookup(buffer: &[StoreEntry], load_addr: u64, load_size: u8) -> Option<StoreEntry> {
        for candidate in buffer.iter().rev() {
            if oracle_can_forward(*candidate, load_addr, load_size) {
                return Some(*candidate);
            }
        }
        None
    }

    // r[verify store-buffer.capacity]
    #[test]
    fn test_enqueue_adds_entry_when_not_full() {
        let buffer = vec![entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER)];
        let pushed = enqueue(&buffer, entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_YOUNGER, TEST_SEQ_YOUNGER));
        assert!(pushed.is_some());
        let next = pushed.unwrap();
        assert_eq!(next.len(), 2);
        assert_eq!(next[1].addr, TEST_ADDR_OTHER);
        assert_eq!(next[1].seq, TEST_SEQ_YOUNGER);
    }

    // r[verify store-buffer.capacity]
    #[test]
    fn test_enqueue_rejects_full_buffer() {
        let mut buffer = Vec::new();
        let mut index: usize = 0;
        while index < STORE_BUFFER_CAPACITY {
            buffer.push(entry(TEST_ADDR_BASE + index as u64, STORE_SIZE_BYTE, index as u64, index as u64));
            index += 1;
        }
        let rejected =
            enqueue(&buffer, entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_CACHE, STORE_BUFFER_CAPACITY as u64));
        assert_eq!(rejected, None);
        assert_eq!(buffer.len(), STORE_BUFFER_CAPACITY);
    }

    // r[verify store-buffer.drain-ordering]
    #[test]
    fn test_drain_oldest_on_empty_returns_none() {
        let buffer: Vec<StoreEntry> = Vec::new();
        assert_eq!(drain_oldest(&buffer), None);
    }

    // r[verify store-buffer.drain-ordering]
    #[test]
    fn test_drain_oldest_returns_head_and_preserves_tail_order() {
        let oldest = entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER);
        let youngest = entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_YOUNGER, TEST_SEQ_YOUNGER);
        let buffer = vec![oldest, youngest];
        let drained = drain_oldest(&buffer).unwrap();
        assert_eq!(drained.0, oldest);
        assert_eq!(drained.1, vec![youngest]);
    }

    // r[verify store-buffer.full-coverage]
    #[test]
    fn test_can_forward_when_store_covers_load() {
        let covering_store = entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER);
        let load_addr = TEST_ADDR_BASE + TEST_ADDR_OFFSET;
        let load_size = STORE_SIZE_HALFWORD;
        assert!(can_forward(covering_store, load_addr, load_size));
    }

    // r[verify store-buffer.full-coverage]
    #[test]
    fn test_can_forward_rejects_partial_store() {
        let narrow_store = entry(TEST_ADDR_BASE, STORE_SIZE_HALFWORD, TEST_DATA_OLDER, TEST_SEQ_OLDER);
        let load_size = STORE_SIZE_WORD;
        assert!(!can_forward(narrow_store, TEST_ADDR_BASE, load_size));
    }

    // r[verify store-buffer.youngest-match]
    #[test]
    fn test_forward_lookup_selects_youngest_match() {
        let older = entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER);
        let middle = entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_CACHE, TEST_SEQ_YOUNGER);
        let younger = entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_YOUNGER, TEST_SEQ_THIRD);
        let buffer = vec![older, middle, younger];
        let forwarded = forward_lookup(&buffer, TEST_ADDR_BASE, STORE_SIZE_WORD);
        assert_eq!(forwarded, Some(younger));
    }

    // r[verify store-buffer.youngest-match]
    // r[verify store-buffer.cache-fallback]
    #[test]
    fn test_forward_lookup_none_on_miss() {
        let buffer = vec![entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER)];
        assert_eq!(forward_lookup(&buffer, TEST_ADDR_BASE, STORE_SIZE_WORD), None);
        assert_eq!(load_source(&buffer, TEST_ADDR_BASE, STORE_SIZE_WORD), LOAD_SOURCE_CACHE);
    }

    // r[verify store-buffer.fence]
    #[test]
    fn test_store_fence_returns_empty_buffer() {
        let buffer = vec![entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER)];
        let fenced = store_fence(&buffer);
        assert!(fenced.is_empty());
        assert!(is_fence_satisfied(&fenced));
    }

    // r[verify store-buffer.fence]
    #[test]
    fn test_post_fence_enqueue_enters_empty_buffer() {
        let buffer = vec![entry(TEST_ADDR_BASE, STORE_SIZE_WORD, TEST_DATA_OLDER, TEST_SEQ_OLDER)];
        let fenced = store_fence(&buffer);
        let next_entry = entry(TEST_ADDR_OTHER, STORE_SIZE_WORD, TEST_DATA_YOUNGER, TEST_SEQ_OLDER);
        let pushed = enqueue(&fenced, next_entry).unwrap();
        assert_eq!(pushed, vec![next_entry]);
    }

    // r[verify store-buffer.drain-ordering]
    proptest! {
        #[test]
        fn proptest_drain_all_preserves_fifo_order(
            specs in prop::collection::vec(
                (0u64..TEST_MAX_RANDOM_ADDR, 0u8..TEST_SIZE_TAG_COUNT, any::<u64>()),
                0..(STORE_BUFFER_CAPACITY + 1),
            )
        ) {
            let buffer = make_random_buffer(&specs);
            let drained = drain_all(&buffer);
            prop_assert_eq!(drained, buffer);
        }
    }

    // r[verify store-buffer.youngest-match]
    // r[verify store-buffer.full-coverage]
    proptest! {
        #[test]
        fn proptest_forward_lookup_matches_oracle(
            specs in prop::collection::vec(
                (0u64..TEST_MAX_RANDOM_ADDR, 0u8..TEST_SIZE_TAG_COUNT, any::<u64>()),
                0..(STORE_BUFFER_CAPACITY + 1),
            ),
            load_addr in 0u64..TEST_MAX_RANDOM_ADDR,
            load_size_tag in 0u8..TEST_SIZE_TAG_COUNT,
        ) {
            let buffer = make_random_buffer(&specs);
            let load_size = size_from_tag(load_size_tag % TEST_SIZE_TAG_COUNT);
            let expected = oracle_forward_lookup(&buffer, load_addr, load_size);
            let actual = forward_lookup(&buffer, load_addr, load_size);
            prop_assert_eq!(actual, expected);
        }
    }

    // r[verify store-buffer.capacity]
    proptest! {
        #[test]
        fn proptest_sequential_enqueue_never_exceeds_capacity(
            specs in prop::collection::vec(
                (0u64..TEST_MAX_RANDOM_ADDR, 0u8..TEST_SIZE_TAG_COUNT, any::<u64>()),
                0..(STORE_BUFFER_CAPACITY * TEST_RANDOM_BUFFER_FACTOR + 1),
            )
        ) {
            let mut buffer: Vec<StoreEntry> = Vec::new();
            let mut index: usize = 0;
            while index < specs.len() {
                let (addr, size_tag, data) = specs[index];
                let next = entry(
                    addr % TEST_MAX_RANDOM_ADDR,
                    size_from_tag(size_tag % TEST_SIZE_TAG_COUNT),
                    data,
                    index as u64,
                );
                match enqueue(&buffer, next) {
                    Some(pushed) => {
                        buffer = pushed;
                    }
                    None => {
                        prop_assert_eq!(buffer.len(), STORE_BUFFER_CAPACITY);
                    }
                }
                prop_assert!(buffer.len() <= STORE_BUFFER_CAPACITY);
                index += 1;
            }
        }
    }
}
