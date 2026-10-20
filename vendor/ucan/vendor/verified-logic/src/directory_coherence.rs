//! Verified single-line directory-based cache coherence.
//!
//! The model keeps one directory entry for one cache line. The directory tracks
//! either a shared sharer bit-vector or a single exclusive owner and produces
//! targeted invalidation messages for write requests.

use vstd::prelude::*;

use crate::mesi;

verus! {

/// Directory state: no cache holds the line.
pub const DIR_UNCACHED: u8 = 0u8;
/// Directory state: one or more caches hold shared copies.
pub const DIR_SHARED: u8 = 1u8;
/// Directory state: exactly one cache holds an exclusive or modified copy.
pub const DIR_EXCLUSIVE: u8 = 2u8;

#[derive(Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub owner: Option<usize>,
    pub sharers: Vec<bool>,
    pub state: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReadRequest {
    pub cache: usize,
    pub line: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WriteRequest {
    pub cache: usize,
    pub line: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InvalidateMessage {
    pub cache: usize,
    pub line: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InvalidateAck {
    pub cache: usize,
    pub line: u64,
}

// r[impl directory.consistency]
// r[impl directory.invalidation]
// r[impl directory.single-writer]
// r[impl directory.eviction-notify]
// r[impl directory.serialization]
// r[impl directory.mesi-embedding]
pub closed spec fn valid_directory_state_spec(state: u8) -> bool {
    state == DIR_UNCACHED || state == DIR_SHARED || state == DIR_EXCLUSIVE
}

fn valid_directory_state(state: u8) -> (result: bool)
    ensures result == valid_directory_state_spec(state),
{
    state == DIR_UNCACHED || state == DIR_SHARED || state == DIR_EXCLUSIVE
}

closed spec fn clear_bits_spec(cache_count: usize) -> Seq<bool> {
    Seq::new(cache_count as nat, |i: int| false)
}

closed spec fn all_false_bits_spec(bits: Seq<bool>) -> bool {
    forall|i: int| #![trigger bits[i]] 0 <= i < bits.len() ==> !bits[i]
}

closed spec fn has_any_true_bit_spec(bits: Seq<bool>) -> bool {
    exists|i: int| 0 <= i < bits.len() && bits[i]
}

closed spec fn invalid_cache_vector_spec(cache_count: usize) -> Seq<u8> {
    Seq::new(cache_count as nat, |i: int| mesi::STATE_INVALID)
}

closed spec fn shared_cache_vector_spec(sharers: Seq<bool>) -> Seq<u8> {
    Seq::new(sharers.len() as nat, |i: int|
        if sharers[i] {
            mesi::STATE_SHARED
        } else {
            mesi::STATE_INVALID
        }
    )
}

closed spec fn exclusive_cache_vector_spec(cache_count: usize, owner: usize, modified: bool) -> Seq<u8>
    recommends owner < cache_count
{
    Seq::new(cache_count as nat, |i: int|
        if i == owner as int {
            if modified { mesi::STATE_MODIFIED } else { mesi::STATE_EXCLUSIVE }
        } else {
            mesi::STATE_INVALID
        }
    )
}

closed spec fn two_sharer_bits_spec(cache_count: usize, first: usize, second: usize) -> Seq<bool>
    recommends first < cache_count, second < cache_count
{
    Seq::new(cache_count as nat, |i: int| i == first as int || i == second as int)
}

closed spec fn exclusive_owner_cache_matches_spec(caches: Seq<u8>, owner: usize) -> bool
    recommends owner < caches.len()
{
    &&& (caches[owner as int] == mesi::STATE_EXCLUSIVE || caches[owner as int] == mesi::STATE_MODIFIED)
    &&& forall|i: int| #![trigger caches[i]] 0 <= i < caches.len() && i != owner as int ==> caches[i] == mesi::STATE_INVALID
}

// r[impl directory.consistency]
pub closed spec fn valid_directory_entry_spec(entry: DirectoryEntry, cache_count: usize) -> bool {
    &&& valid_directory_state_spec(entry.state)
    &&& entry.sharers@.len() == cache_count as int
    &&& match entry.owner {
        Some(owner) => owner < cache_count,
        None => true,
    }
    &&& match entry.state {
        DIR_UNCACHED => {
            &&& entry.owner.is_none()
            &&& all_false_bits_spec(entry.sharers@)
        }
        DIR_SHARED => {
            &&& entry.owner.is_none()
            &&& has_any_true_bit_spec(entry.sharers@)
        }
        DIR_EXCLUSIVE => {
            &&& entry.owner.is_some()
            &&& all_false_bits_spec(entry.sharers@)
        }
        _ => false,
    }
}

// r[impl directory.mesi-embedding]
pub closed spec fn directory_to_mesi_spec(entry: DirectoryEntry, cache: usize) -> u8
    recommends cache < entry.sharers@.len()
{
    if entry.state == DIR_SHARED {
        if entry.sharers@[cache as int] {
            mesi::STATE_SHARED
        } else {
            mesi::STATE_INVALID
        }
    } else if entry.state == DIR_EXCLUSIVE {
        if entry.owner.is_some() && entry.owner.unwrap() == cache {
            mesi::STATE_EXCLUSIVE
        } else {
            mesi::STATE_INVALID
        }
    } else {
        mesi::STATE_INVALID
    }
}

// r[impl directory.consistency]
pub closed spec fn directory_cache_consistent_spec(entry: DirectoryEntry, caches: Seq<u8>) -> bool {
    &&& valid_directory_entry_spec(entry, caches.len() as usize)
    &&& forall|i: int| #![trigger caches[i]] 0 <= i < caches.len() ==> mesi::is_valid_state_spec(caches[i])
    &&& match entry.state {
        DIR_UNCACHED => caches =~= invalid_cache_vector_spec(caches.len() as usize),
        DIR_SHARED => caches =~= shared_cache_vector_spec(entry.sharers@),
        DIR_EXCLUSIVE => {
            &&& entry.owner.is_some()
            &&& exclusive_owner_cache_matches_spec(caches, entry.owner.unwrap())
        }
        _ => false,
    }
}

closed spec fn collect_invalidations_prefix_spec(
    sharers: Seq<bool>,
    requester: usize,
    line: u64,
    upto: int,
) -> Seq<InvalidateMessage>
    recommends 0 <= upto <= sharers.len()
    decreases upto
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = upto - 1;
        let prefix = collect_invalidations_prefix_spec(sharers, requester, line, prev);
        if sharers[prev] && prev != requester as int {
            prefix.push(InvalidateMessage { cache: prev as usize, line })
        } else {
            prefix
        }
    }
}

closed spec fn collect_invalidations_spec(sharers: Seq<bool>, requester: usize, line: u64) -> Seq<InvalidateMessage> {
    collect_invalidations_prefix_spec(sharers, requester, line, sharers.len() as int)
}

closed spec fn write_invalidations_spec(entry: DirectoryEntry, requester: usize, line: u64) -> Seq<InvalidateMessage> {
    if entry.state == DIR_SHARED {
        collect_invalidations_spec(entry.sharers@, requester, line)
    } else if entry.state == DIR_EXCLUSIVE && entry.owner.is_some() && entry.owner.unwrap() != requester {
        seq![InvalidateMessage { cache: entry.owner.unwrap(), line }]
    } else {
        seq![]
    }
}

// r[impl directory.consistency]
pub closed spec fn process_read_result_spec(
    entry: DirectoryEntry,
    caches: Seq<u8>,
    requester: usize,
) -> (Option<usize>, Seq<bool>, u8, Seq<u8>)
    recommends requester < caches.len(), caches.len() == entry.sharers@.len()
{
    if entry.state == DIR_UNCACHED {
        (
            Some(requester),
            clear_bits_spec(caches.len() as usize),
            DIR_EXCLUSIVE,
            exclusive_cache_vector_spec(caches.len() as usize, requester, false),
        )
    } else if entry.state == DIR_SHARED {
        let next_sharers = entry.sharers@.update(requester as int, true);
        (
            None,
            next_sharers,
            DIR_SHARED,
            shared_cache_vector_spec(next_sharers),
        )
    } else if entry.owner.is_some() && entry.owner.unwrap() == requester {
        (
            entry.owner,
            entry.sharers@,
            entry.state,
            caches,
        )
    } else {
        let owner = entry.owner.unwrap();
        let next_sharers = two_sharer_bits_spec(caches.len() as usize, owner, requester);
        (
            None,
            next_sharers,
            DIR_SHARED,
            shared_cache_vector_spec(next_sharers),
        )
    }
}

// r[impl directory.invalidation]
// r[impl directory.single-writer]
// r[impl directory.serialization]
pub closed spec fn process_write_result_spec(
    entry: DirectoryEntry,
    caches: Seq<u8>,
    requester: usize,
    line: u64,
) -> (Option<usize>, Seq<bool>, u8, Seq<u8>, Seq<InvalidateMessage>)
    recommends requester < caches.len(), caches.len() == entry.sharers@.len()
{
    (
        Some(requester),
        clear_bits_spec(caches.len() as usize),
        DIR_EXCLUSIVE,
        exclusive_cache_vector_spec(caches.len() as usize, requester, true),
        write_invalidations_spec(entry, requester, line),
    )
}

// r[impl directory.eviction-notify]
pub closed spec fn process_eviction_result_spec(
    entry: DirectoryEntry,
    caches: Seq<u8>,
    cache: usize,
) -> (Option<usize>, Seq<bool>, u8, Seq<u8>, bool)
    recommends cache < caches.len(), caches.len() == entry.sharers@.len()
{
    if entry.state == DIR_SHARED && entry.sharers@[cache as int] {
        let next_sharers = entry.sharers@.update(cache as int, false);
        let next_state = if has_any_true_bit_spec(next_sharers) { DIR_SHARED } else { DIR_UNCACHED };
        (
            None,
            next_sharers,
            next_state,
            shared_cache_vector_spec(next_sharers),
            false,
        )
    } else if entry.state == DIR_EXCLUSIVE && entry.owner.is_some() && entry.owner.unwrap() == cache {
        (
            None,
            clear_bits_spec(caches.len() as usize),
            DIR_UNCACHED,
            invalid_cache_vector_spec(caches.len() as usize),
            caches[cache as int] == mesi::STATE_MODIFIED,
        )
    } else {
        (
            entry.owner,
            entry.sharers@,
            entry.state,
            caches,
            false,
        )
    }
}

fn clone_bool_vec(values: &[bool]) -> (result: Vec<bool>)
    ensures result@ == values@,
{
    let mut result: Vec<bool> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ == values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn clone_u8_vec(values: &[u8]) -> (result: Vec<u8>)
    ensures result@ == values@,
{
    let mut result: Vec<u8> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ == values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn clear_bits(cache_count: usize) -> (result: Vec<bool>)
    ensures result@ == clear_bits_spec(cache_count),
{
    let mut result: Vec<bool> = Vec::new();
    let mut index: usize = 0;
    while index < cache_count
        invariant
            0 <= index <= cache_count,
            result@ == clear_bits_spec(index),
        decreases cache_count - index,
    {
        result.push(false);
        index = index + 1;
    }
    result
}

fn bool_vec_all_false(values: &[bool]) -> (result: bool)
    ensures result == all_false_bits_spec(values@),
{
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            forall|i: int| 0 <= i < index as int ==> !values@[i],
        decreases values@.len() - index,
    {
        if values[index] {
            return false;
        }
        index = index + 1;
    }
    true
}

fn bool_vec_any_true(values: &[bool]) -> (result: bool)
    ensures result == has_any_true_bit_spec(values@),
{
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            forall|i: int| 0 <= i < index as int ==> !values@[i],
        decreases values@.len() - index,
    {
        if values[index] {
            return true;
        }
        index = index + 1;
    }
    false
}

fn update_bool_vec(values: &[bool], index: usize, value: bool) -> (result: Vec<bool>)
    requires index < values@.len()
    ensures result@ == values@.update(index as int, value),
{
    let mut result = clone_bool_vec(values);
    result.set(index, value);
    result
}

fn shared_cache_vector(sharers: &[bool]) -> (result: Vec<u8>)
    ensures result@ == shared_cache_vector_spec(sharers@),
{
    let mut result: Vec<u8> = Vec::new();
    let mut index: usize = 0;
    while index < sharers.len()
        invariant
            0 <= index <= sharers@.len(),
            result@ == shared_cache_vector_spec(sharers@).subrange(0, index as int),
        decreases sharers@.len() - index,
    {
        if sharers[index] {
            result.push(mesi::STATE_SHARED);
        } else {
            result.push(mesi::STATE_INVALID);
        }
        index = index + 1;
    }
    result
}

fn exclusive_cache_vector(cache_count: usize, owner: usize, modified: bool) -> (result: Vec<u8>)
    requires owner < cache_count
    ensures result@ =~= exclusive_cache_vector_spec(cache_count, owner, modified),
{
    let empty_bits = clear_bits(cache_count);
    let mut result = shared_cache_vector(&empty_bits);
    let owner_state: u8 = if modified { mesi::STATE_MODIFIED } else { mesi::STATE_EXCLUSIVE };
    result.set(owner, owner_state);
    assert(result@ =~= invalid_cache_vector_spec(cache_count).update(owner as int, owner_state));
    assert(result@ =~= exclusive_cache_vector_spec(cache_count, owner, modified));
    result
}

fn two_sharer_bits(cache_count: usize, first: usize, second: usize) -> (result: Vec<bool>)
    requires first < cache_count, second < cache_count
    ensures result@ =~= two_sharer_bits_spec(cache_count, first, second),
{
    let mut result = clear_bits(cache_count);
    result.set(first, true);
    result.set(second, true);
    assert(result@ =~= clear_bits_spec(cache_count).update(first as int, true).update(second as int, true));
    assert(result@ =~= two_sharer_bits_spec(cache_count, first, second));
    result
}

fn collect_invalidations(sharers: &[bool], requester: usize, line: u64) -> (result: Vec<InvalidateMessage>)
    ensures result@ =~= collect_invalidations_spec(sharers@, requester, line),
{
    let mut result: Vec<InvalidateMessage> = Vec::new();
    let mut index: usize = 0;
    while index < sharers.len()
        invariant
            0 <= index <= sharers@.len(),
            result@ == collect_invalidations_prefix_spec(sharers@, requester, line, index as int),
        decreases sharers@.len() - index,
    {
        if sharers[index] && index != requester {
            result.push(InvalidateMessage { cache: index, line });
        }
        index = index + 1;
    }
    result
}

fn u8_vec_equal(left: &[u8], right: &[u8]) -> (result: bool)
    ensures result == (left@ =~= right@),
{
    if left.len() != right.len() {
        return false;
    }
    let mut index: usize = 0;
    while index < left.len()
        invariant
            0 <= index <= left@.len(),
            left@.len() == right@.len(),
            forall|i: int| 0 <= i < index as int ==> left@[i] == right@[i],
        decreases left@.len() - index,
    {
        if left[index] != right[index] {
            return false;
        }
        index = index + 1;
    }
    assert(left@ =~= right@);
    true
}

fn exclusive_owner_cache_matches(caches: &[u8], owner: usize) -> (result: bool)
    requires owner < caches@.len()
    ensures result == exclusive_owner_cache_matches_spec(caches@, owner),
{
    let owner_state = caches[owner];
    if owner_state != mesi::STATE_EXCLUSIVE && owner_state != mesi::STATE_MODIFIED {
        return false;
    }
    let mut index: usize = 0;
    while index < caches.len()
        invariant
            0 <= index <= caches@.len(),
            owner < caches@.len(),
            owner_state == caches@[owner as int],
            owner_state == mesi::STATE_EXCLUSIVE || owner_state == mesi::STATE_MODIFIED,
            forall|i: int| 0 <= i < index as int && i != owner as int ==> caches@[i] == mesi::STATE_INVALID,
        decreases caches@.len() - index,
    {
        if index != owner && caches[index] != mesi::STATE_INVALID {
            return false;
        }
        index = index + 1;
    }
    true
}

// r[impl directory.consistency]
#[allow(clippy::collapsible_match, clippy::single_match)]
pub fn valid_directory_entry(entry: &DirectoryEntry, cache_count: usize) -> (result: bool)
    ensures result == valid_directory_entry_spec(*entry, cache_count),
{
    if !valid_directory_state(entry.state) {
        return false;
    }
    if entry.sharers.len() != cache_count {
        return false;
    }
    match entry.owner {
        Some(owner) => {
            if owner >= cache_count {
                return false;
            }
        }
        None => {}
    }
    if entry.state == DIR_UNCACHED {
        entry.owner.is_none() && bool_vec_all_false(&entry.sharers)
    } else if entry.state == DIR_SHARED {
        entry.owner.is_none() && bool_vec_any_true(&entry.sharers)
    } else {
        entry.owner.is_some() && bool_vec_all_false(&entry.sharers)
    }
}

// r[impl directory.mesi-embedding]
// r[impl directory.mesi-embedding.scenario-valid-mapping]
pub fn directory_to_mesi(entry: &DirectoryEntry, cache: usize) -> (result: u8)
    requires cache < entry.sharers@.len()
    ensures result == directory_to_mesi_spec(*entry, cache),
{
    if entry.state == DIR_SHARED {
        if entry.sharers[cache] {
            mesi::STATE_SHARED
        } else {
            mesi::STATE_INVALID
        }
    } else if entry.state == DIR_EXCLUSIVE {
        if entry.owner.is_some() && entry.owner.unwrap() == cache {
            mesi::STATE_EXCLUSIVE
        } else {
            mesi::STATE_INVALID
        }
    } else {
        mesi::STATE_INVALID
    }
}

// r[impl directory.consistency]
// r[impl directory.consistency.scenario-stale-record-rejected]
// r[impl directory.mesi-embedding.scenario-exclusive-owner-compatible]
pub fn check_consistency(entry: &DirectoryEntry, caches: &[u8]) -> (result: bool)
    ensures result == directory_cache_consistent_spec(*entry, caches@),
{
    let entry_valid = valid_directory_entry(entry, caches.len());
    if !entry_valid {
        return false;
    }
    assert(entry_valid);
    assert(valid_directory_entry_spec(*entry, caches.len()));
    if entry.state == DIR_UNCACHED {
        let expected_bits = clear_bits(caches.len());
        let expected = shared_cache_vector(&expected_bits);
        let same = u8_vec_equal(&expected, caches);
        assert(same == (expected@ =~= caches@));
        same
    } else if entry.state == DIR_SHARED {
        let expected = shared_cache_vector(&entry.sharers);
        let same = u8_vec_equal(&expected, caches);
        assert(same == (expected@ =~= caches@));
        same
    } else {
        let same = exclusive_owner_cache_matches(caches, entry.owner.unwrap());
        assert(same == exclusive_owner_cache_matches_spec(caches@, entry.owner.unwrap()));
        same
    }
}

// r[impl directory.consistency]
// r[impl directory.consistency.scenario-read-uncached-exclusive]
// r[impl directory.consistency.scenario-read-exclusive-to-shared]
pub fn process_read_request(
    entry: &DirectoryEntry,
    caches: &[u8],
    request: ReadRequest,
) -> (result: (DirectoryEntry, Vec<u8>))
    requires
        request.cache < caches@.len(),
        caches@.len() == entry.sharers@.len(),
        directory_cache_consistent_spec(*entry, caches@),
    ensures
        result.0.owner == process_read_result_spec(*entry, caches@, request.cache).0,
        result.0.sharers@ =~= process_read_result_spec(*entry, caches@, request.cache).1,
        result.0.state == process_read_result_spec(*entry, caches@, request.cache).2,
        result.1@ =~= process_read_result_spec(*entry, caches@, request.cache).3,
{
    let _ = request.line;
    if entry.state == DIR_UNCACHED {
        let next_sharers = clear_bits(caches.len());
        let next_caches = exclusive_cache_vector(caches.len(), request.cache, false);
        (
            DirectoryEntry { owner: Some(request.cache), sharers: next_sharers, state: DIR_EXCLUSIVE },
            next_caches,
        )
    } else if entry.state == DIR_SHARED {
        let next_sharers = update_bool_vec(&entry.sharers, request.cache, true);
        let next_caches = shared_cache_vector(&next_sharers);
        (
            DirectoryEntry { owner: None, sharers: next_sharers, state: DIR_SHARED },
            next_caches,
        )
    } else if entry.owner.unwrap() == request.cache {
        (
            DirectoryEntry {
                owner: entry.owner,
                sharers: clone_bool_vec(&entry.sharers),
                state: entry.state,
            },
            clone_u8_vec(caches),
        )
    } else {
        let owner = entry.owner.unwrap();
        let next_sharers = two_sharer_bits(caches.len(), owner, request.cache);
        let next_caches = shared_cache_vector(&next_sharers);
        (
            DirectoryEntry { owner: None, sharers: next_sharers, state: DIR_SHARED },
            next_caches,
        )
    }
}

// r[impl directory.invalidation]
// r[impl directory.invalidation.scenario-shared-readers-invalidated]
// r[impl directory.invalidation.scenario-exclusive-owner-invalidated]
// r[impl directory.single-writer]
// r[impl directory.single-writer.scenario-one-modified-owner]
// r[impl directory.serialization]
// r[impl directory.serialization.scenario-same-line-write-order]
// r[impl directory.serialization.scenario-different-lines-independent]
#[allow(clippy::vec_init_then_push)]
pub fn process_write_request(
    entry: &DirectoryEntry,
    caches: &[u8],
    request: WriteRequest,
) -> (result: (DirectoryEntry, Vec<u8>, Vec<InvalidateMessage>))
    requires
        request.cache < caches@.len(),
        caches@.len() == entry.sharers@.len(),
        directory_cache_consistent_spec(*entry, caches@),
    ensures
        result.0.owner == process_write_result_spec(*entry, caches@, request.cache, request.line).0,
        result.0.sharers@ =~= process_write_result_spec(*entry, caches@, request.cache, request.line).1,
        result.0.state == process_write_result_spec(*entry, caches@, request.cache, request.line).2,
        result.1@ =~= process_write_result_spec(*entry, caches@, request.cache, request.line).3,
        result.2@ =~= process_write_result_spec(*entry, caches@, request.cache, request.line).4,
{
    let next_sharers = clear_bits(caches.len());
    let next_caches = exclusive_cache_vector(caches.len(), request.cache, true);
    let messages = if entry.state == DIR_SHARED {
        collect_invalidations(&entry.sharers, request.cache, request.line)
    } else if entry.state == DIR_EXCLUSIVE && entry.owner.unwrap() != request.cache {
        let mut result: Vec<InvalidateMessage> = Vec::new();
        result.push(InvalidateMessage { cache: entry.owner.unwrap(), line: request.line });
        result
    } else {
        Vec::new()
    };
    (
        DirectoryEntry { owner: Some(request.cache), sharers: next_sharers, state: DIR_EXCLUSIVE },
        next_caches,
        messages,
    )
}

// r[impl directory.eviction-notify]
// r[impl directory.eviction-notify.scenario-shared-eviction-clears-bit]
// r[impl directory.eviction-notify.scenario-modified-owner-writeback]
pub fn process_eviction(
    entry: &DirectoryEntry,
    caches: &[u8],
    cache: usize,
) -> (result: (DirectoryEntry, Vec<u8>, bool))
    requires
        cache < caches@.len(),
        caches@.len() == entry.sharers@.len(),
        directory_cache_consistent_spec(*entry, caches@),
    ensures
        result.0.owner == process_eviction_result_spec(*entry, caches@, cache).0,
        result.0.sharers@ =~= process_eviction_result_spec(*entry, caches@, cache).1,
        result.0.state == process_eviction_result_spec(*entry, caches@, cache).2,
        result.1@ =~= process_eviction_result_spec(*entry, caches@, cache).3,
        result.2 == process_eviction_result_spec(*entry, caches@, cache).4,
{
    if entry.state == DIR_SHARED && entry.sharers[cache] {
        let next_sharers = update_bool_vec(&entry.sharers, cache, false);
        let next_state = if bool_vec_any_true(&next_sharers) { DIR_SHARED } else { DIR_UNCACHED };
        let next_caches = shared_cache_vector(&next_sharers);
        (
            DirectoryEntry { owner: None, sharers: next_sharers, state: next_state },
            next_caches,
            false,
        )
    } else if entry.state == DIR_EXCLUSIVE && entry.owner.unwrap() == cache {
        let next_sharers = clear_bits(caches.len());
        let next_caches = shared_cache_vector(&next_sharers);
        (
            DirectoryEntry { owner: None, sharers: next_sharers, state: DIR_UNCACHED },
            next_caches,
            caches[cache] == mesi::STATE_MODIFIED,
        )
    } else {
        (
            DirectoryEntry {
                owner: entry.owner,
                sharers: clone_bool_vec(&entry.sharers),
                state: entry.state,
            },
            clone_u8_vec(caches),
            false,
        )
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const TEST_CACHE_COUNT: usize = 4usize;
    const TEST_LINE: u64 = 7u64;
    const RANDOM_CACHE_MIN: usize = 2usize;
    const RANDOM_CACHE_MAX: usize = 6usize;
    const OP_READ: u8 = 0u8;
    const OP_WRITE: u8 = 1u8;
    const OP_EVICT: u8 = 2u8;

    fn uncached_entry(cache_count: usize) -> DirectoryEntry {
        DirectoryEntry {
            owner: None,
            sharers: vec![false; cache_count],
            state: DIR_UNCACHED,
        }
    }

    fn uncached_caches(cache_count: usize) -> Vec<u8> {
        vec![mesi::STATE_INVALID; cache_count]
    }

    fn shared_entry(sharers: Vec<bool>) -> DirectoryEntry {
        DirectoryEntry {
            owner: None,
            sharers,
            state: DIR_SHARED,
        }
    }

    fn exclusive_entry(cache_count: usize, owner: usize) -> DirectoryEntry {
        DirectoryEntry {
            owner: Some(owner),
            sharers: vec![false; cache_count],
            state: DIR_EXCLUSIVE,
        }
    }

    fn shared_caches(sharers: &[bool]) -> Vec<u8> {
        sharers
            .iter()
            .map(|present| {
                if *present {
                    mesi::STATE_SHARED
                } else {
                    mesi::STATE_INVALID
                }
            })
            .collect()
    }

    fn exclusive_caches(cache_count: usize, owner: usize, modified: bool) -> Vec<u8> {
        let owner_state = if modified {
            mesi::STATE_MODIFIED
        } else {
            mesi::STATE_EXCLUSIVE
        };
        (0usize..cache_count)
            .map(|index| {
                if index == owner {
                    owner_state
                } else {
                    mesi::STATE_INVALID
                }
            })
            .collect()
    }

    // r[verify directory.consistency]
    // r[verify directory.consistency.scenario-read-uncached-exclusive]
    #[test]
    fn test_read_from_uncached_grants_exclusive() {
        let entry = uncached_entry(TEST_CACHE_COUNT);
        let caches = uncached_caches(TEST_CACHE_COUNT);
        let request = ReadRequest {
            cache: 1usize,
            line: TEST_LINE,
        };
        let result = process_read_request(&entry, &caches, request);
        assert_eq!(result.0.owner, Some(1usize));
        assert_eq!(result.0.state, DIR_EXCLUSIVE);
        assert_eq!(result.1, exclusive_caches(TEST_CACHE_COUNT, 1usize, false));
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.consistency]
    // r[verify directory.consistency.scenario-read-exclusive-to-shared]
    #[test]
    fn test_read_from_exclusive_downgrades_to_shared() {
        let entry = exclusive_entry(TEST_CACHE_COUNT, 0usize);
        let caches = exclusive_caches(TEST_CACHE_COUNT, 0usize, true);
        let request = ReadRequest {
            cache: 2usize,
            line: TEST_LINE,
        };
        let result = process_read_request(&entry, &caches, request);
        assert_eq!(result.0.owner, None);
        assert_eq!(result.0.state, DIR_SHARED);
        assert_eq!(result.0.sharers, vec![true, false, true, false]);
        assert_eq!(result.1, shared_caches(&result.0.sharers));
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.invalidation]
    // r[verify directory.invalidation.scenario-shared-readers-invalidated]
    // r[verify directory.single-writer]
    // r[verify directory.single-writer.scenario-one-modified-owner]
    #[test]
    fn test_write_from_shared_invalidates_other_sharers() {
        let entry = shared_entry(vec![true, true, false, true]);
        let caches = shared_caches(&entry.sharers);
        let request = WriteRequest {
            cache: 1usize,
            line: TEST_LINE,
        };
        let result = process_write_request(&entry, &caches, request);
        assert_eq!(result.0.owner, Some(1usize));
        assert_eq!(result.0.state, DIR_EXCLUSIVE);
        assert_eq!(result.1, exclusive_caches(TEST_CACHE_COUNT, 1usize, true));
        assert_eq!(result.2.len(), 2usize);
        assert_eq!(result.2[0], InvalidateMessage {
            cache: 0usize,
            line: TEST_LINE
        });
        assert_eq!(result.2[1], InvalidateMessage {
            cache: 3usize,
            line: TEST_LINE
        });
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.invalidation]
    // r[verify directory.invalidation.scenario-exclusive-owner-invalidated]
    // r[verify directory.single-writer]
    #[test]
    fn test_second_exclusive_request_invalidates_previous_owner() {
        let entry = exclusive_entry(TEST_CACHE_COUNT, 0usize);
        let caches = exclusive_caches(TEST_CACHE_COUNT, 0usize, true);
        let request = WriteRequest {
            cache: 3usize,
            line: TEST_LINE,
        };
        let result = process_write_request(&entry, &caches, request);
        assert_eq!(result.2, vec![InvalidateMessage {
            cache: 0usize,
            line: TEST_LINE
        }]);
        assert_eq!(result.1, exclusive_caches(TEST_CACHE_COUNT, 3usize, true));
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.eviction-notify]
    // r[verify directory.eviction-notify.scenario-shared-eviction-clears-bit]
    #[test]
    fn test_shared_eviction_clears_presence_bit() {
        let entry = shared_entry(vec![true, true, false, false]);
        let caches = shared_caches(&entry.sharers);
        let result = process_eviction(&entry, &caches, 0usize);
        assert_eq!(result.0.state, DIR_SHARED);
        assert_eq!(result.0.sharers, vec![false, true, false, false]);
        assert_eq!(result.1, shared_caches(&result.0.sharers));
        assert!(!result.2);
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.eviction-notify]
    // r[verify directory.eviction-notify.scenario-modified-owner-writeback]
    #[test]
    fn test_modified_eviction_requests_writeback() {
        let entry = exclusive_entry(TEST_CACHE_COUNT, 2usize);
        let caches = exclusive_caches(TEST_CACHE_COUNT, 2usize, true);
        let result = process_eviction(&entry, &caches, 2usize);
        assert_eq!(result.0.state, DIR_UNCACHED);
        assert_eq!(result.1, uncached_caches(TEST_CACHE_COUNT));
        assert!(result.2);
        assert!(check_consistency(&result.0, &result.1));
    }

    // r[verify directory.serialization]
    // r[verify directory.serialization.scenario-different-lines-independent]
    #[test]
    fn test_different_lines_are_independent() {
        let left_entry = uncached_entry(TEST_CACHE_COUNT);
        let left_caches = uncached_caches(TEST_CACHE_COUNT);
        let right_entry = shared_entry(vec![true, false, true, false]);
        let right_caches = shared_caches(&right_entry.sharers);
        let left_result = process_write_request(&left_entry, &left_caches, WriteRequest {
            cache: 0usize,
            line: TEST_LINE,
        });
        assert_eq!(right_entry, shared_entry(vec![true, false, true, false]));
        assert_eq!(right_caches, shared_caches(&[true, false, true, false]));
        assert!(check_consistency(&left_result.0, &left_result.1));
        assert!(check_consistency(&right_entry, &right_caches));
    }

    // r[verify directory.mesi-embedding]
    // r[verify directory.mesi-embedding.scenario-valid-mapping]
    // r[verify directory.mesi-embedding.scenario-exclusive-owner-compatible]
    #[test]
    fn test_directory_mapping_returns_valid_mesi_states() {
        let shared = shared_entry(vec![true, false, true, false]);
        for cache in 0usize..shared.sharers.len() {
            assert!(mesi::is_valid_state(directory_to_mesi(&shared, cache)));
        }
        let exclusive = exclusive_entry(TEST_CACHE_COUNT, 1usize);
        for cache in 0usize..exclusive.sharers.len() {
            assert!(mesi::is_valid_state(directory_to_mesi(&exclusive, cache)));
        }
    }

    // r[verify directory.consistency]
    #[test]
    fn test_invalid_entry_with_owner_and_sharers_is_rejected() {
        let entry = DirectoryEntry {
            owner: Some(0usize),
            sharers: vec![true, false, false, false],
            state: DIR_EXCLUSIVE,
        };
        assert!(!valid_directory_entry(&entry, TEST_CACHE_COUNT));
    }

    // r[verify directory.consistency]
    // r[verify directory.consistency.scenario-stale-record-rejected]
    #[test]
    fn test_stale_shared_entry_is_inconsistent() {
        let entry = shared_entry(vec![true, false, false, false]);
        let caches = vec![
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
        ];
        assert!(!check_consistency(&entry, &caches));
    }

    // r[verify directory.consistency]
    #[test]
    fn test_exclusive_entry_with_extra_holder_is_inconsistent() {
        let entry = exclusive_entry(TEST_CACHE_COUNT, 1usize);
        let caches = vec![
            mesi::STATE_SHARED,
            mesi::STATE_MODIFIED,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
        ];
        assert!(!check_consistency(&entry, &caches));
    }

    fn op_sequences() -> impl Strategy<Value = (Vec<u8>, Vec<usize>)> {
        (
            prop::collection::vec(0u8..=OP_EVICT, 1usize..=16usize),
            prop::collection::vec(RANDOM_CACHE_MIN..RANDOM_CACHE_MAX, 1usize..=16usize),
        )
    }

    // r[verify directory.consistency]
    // r[verify directory.single-writer]
    // r[verify directory.serialization.scenario-same-line-write-order]
    proptest! {
        #[test]
        fn prop_random_operations_preserve_consistency_and_single_writer((ops, raw_indices) in op_sequences()) {
            let cache_count = RANDOM_CACHE_MIN + (raw_indices.len() % (RANDOM_CACHE_MAX - RANDOM_CACHE_MIN + 1usize));
            let mut entry = uncached_entry(cache_count);
            let mut caches = uncached_caches(cache_count);
            for (op, raw_index) in ops.iter().zip(raw_indices.iter()) {
                let cache = raw_index % cache_count;
                match *op {
                    OP_READ => {
                        let result = process_read_request(&entry, &caches, ReadRequest { cache, line: TEST_LINE });
                        entry = result.0;
                        caches = result.1;
                    }
                    OP_WRITE => {
                        let result = process_write_request(&entry, &caches, WriteRequest { cache, line: TEST_LINE });
                        entry = result.0;
                        caches = result.1;
                    }
                    _ => {
                        let result = process_eviction(&entry, &caches, cache);
                        entry = result.0;
                        caches = result.1;
                    }
                }
                prop_assert!(check_consistency(&entry, &caches));
                let modified_count = caches
                    .iter()
                    .fold(0usize, |count, state| count + usize::from(*state == mesi::STATE_MODIFIED));
                prop_assert!(modified_count <= 1usize);
            }
        }
    }

    // r[verify directory.mesi-embedding]
    proptest! {
        #[test]
        fn prop_directory_mapping_is_always_valid_after_random_operations((ops, raw_indices) in op_sequences()) {
            let cache_count = RANDOM_CACHE_MIN + (raw_indices.len() % (RANDOM_CACHE_MAX - RANDOM_CACHE_MIN + 1usize));
            let mut entry = uncached_entry(cache_count);
            let mut caches = uncached_caches(cache_count);
            for (op, raw_index) in ops.iter().zip(raw_indices.iter()) {
                let cache = raw_index % cache_count;
                match *op {
                    OP_READ => {
                        let result = process_read_request(&entry, &caches, ReadRequest { cache, line: TEST_LINE });
                        entry = result.0;
                        caches = result.1;
                    }
                    OP_WRITE => {
                        let result = process_write_request(&entry, &caches, WriteRequest { cache, line: TEST_LINE });
                        entry = result.0;
                        caches = result.1;
                    }
                    _ => {
                        let result = process_eviction(&entry, &caches, cache);
                        entry = result.0;
                        caches = result.1;
                    }
                }
                prop_assert!(check_consistency(&entry, &caches));
                for index in 0usize..cache_count {
                    prop_assert!(mesi::is_valid_state(directory_to_mesi(&entry, index)));
                }
            }
        }
    }
}
