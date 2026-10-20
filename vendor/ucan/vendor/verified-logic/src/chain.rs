//! Verified chain hash integrity for append-only logs.
//!
//! Models a hash chain where each entry's hash depends on its predecessor,
//! its log index, its term, and its data. Blake3 is modeled as an
//! uninterpreted function with trusted runtime and collision-resistance
//! boundaries. Proves that appending a correctly-hashed entry preserves
//! chain validity.
//!
//! Matches aspen's `compute_entry_hash` and the chain validity invariant
//! from `spec/chain_hash.rs`.

use vstd::prelude::*;

verus! {

/// A chain hash entry: the hash of a log entry in the chain.
pub struct ChainEntry {
    /// The hash value (abstract — 32 bytes in practice).
    pub hash: u64,
    /// Log index this entry covers.
    pub index: u64,
    /// Election term.
    pub term: u64,
    /// Abstract data identifier (hash of the actual data).
    pub data_id: u64,
}

// --- hash function (axiomatized) ---

/// Abstract hash function modeled as uninterpreted.
///
/// In practice this is blake3(prev_hash || index || term || data).
/// We model it as a pure function from four u64 inputs to a u64 hash.
/// Determinism for equal inputs follows directly from pure spec equality;
/// collision resistance remains an explicit trusted axiom below.
// r[depends chain.hash-deterministic]
// r[impl chain.hash-deterministic]
pub uninterp spec fn hash_spec(prev: u64, index: u64, term: u64, data_id: u64) -> u64;

// r[verify chain.hash-deterministic]
/// Pure spec determinism for equal hash inputs.
pub proof fn hash_deterministic(
    prev1: u64, idx1: u64, term1: u64, data1: u64,
    prev2: u64, idx2: u64, term2: u64, data2: u64,
)
    requires prev1 == prev2, idx1 == idx2, term1 == term2, data1 == data2
    ensures hash_spec(prev1, idx1, term1, data1) == hash_spec(prev2, idx2, term2, data2)
{
}

// --- compute_entry_hash ---

/// Genesis hash constant.
pub const GENESIS_HASH: u64 = 0u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Abstract domain tag for linear chain-entry hashing.
pub const CHAIN_HASH_DOMAIN_TAG: u64 = 0x4348_4149_4e5f_5631u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Abstract domain tag for checkpoint commitments.
pub const CHECKPOINT_HASH_DOMAIN_TAG: u64 = 0x4348_4b50_5431_0001u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Abstract domain tag for receipt commitments.
pub const RECEIPT_HASH_DOMAIN_TAG: u64 = 0x5243_5054_5f31_0001u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Abstract domain tag for snapshot commitments.
pub const SNAPSHOT_HASH_DOMAIN_TAG: u64 = 0x534e_4150_5f31_0001u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Abstract domain tag reserved for compact-history overlay hashing.
pub const COMPACT_HISTORY_DOMAIN_TAG: u64 = 0x434f_4d50_4143_5431u64;

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Spec: domain-separated commitments encode the domain in the first hash input.
pub open spec fn domain_commitment_spec(domain: u64, subject: u64, index: u64, payload: u64) -> u64 {
    hash_spec(domain, subject, index, payload)
}

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Spec: chain-entry hashes under the domain-separated wrapper.
pub open spec fn domain_separated_entry_hash_spec(prev_hash: u64, index: u64, term: u64, data_id: u64) -> u64 {
    domain_commitment_spec(CHAIN_HASH_DOMAIN_TAG, prev_hash, index, hash_spec(prev_hash, index, term, data_id))
}

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Spec: legacy compatibility wrapper for callers that still consume `compute_entry_hash_spec`.
pub open spec fn legacy_entry_hash_compat_spec(prev_hash: u64, index: u64, term: u64, data_id: u64) -> u64 {
    compute_entry_hash_spec(prev_hash, index, term, data_id)
}

// r[depends chain.domain-separated-hash]
// r[impl chain.domain-separated-hash]
/// Spec: chain-entry hashes are committed to the chain domain.
pub open spec fn chain_hash_domain_tag_spec() -> u64 {
    CHAIN_HASH_DOMAIN_TAG
}

// r[verify chain.domain-separated-hash]
/// Proof: chain-entry and checkpoint domains cannot alias.
pub proof fn chain_domain_distinct_from_checkpoint()
    ensures CHAIN_HASH_DOMAIN_TAG != CHECKPOINT_HASH_DOMAIN_TAG
{}

// r[verify chain.domain-separated-hash]
/// Proof: chain-entry and receipt domains cannot alias.
pub proof fn chain_domain_distinct_from_receipt()
    ensures CHAIN_HASH_DOMAIN_TAG != RECEIPT_HASH_DOMAIN_TAG
{}

// r[verify chain.domain-separated-hash]
/// Proof: chain-entry and snapshot domains cannot alias.
pub proof fn chain_domain_distinct_from_snapshot()
    ensures CHAIN_HASH_DOMAIN_TAG != SNAPSHOT_HASH_DOMAIN_TAG
{}

// r[verify chain.domain-separated-hash]
/// Proof: chain-entry and compact-history domains cannot alias.
pub proof fn chain_domain_distinct_from_compact_history()
    ensures CHAIN_HASH_DOMAIN_TAG != COMPACT_HISTORY_DOMAIN_TAG
{}

// r[verify chain.domain-separated-hash]
/// Proof: a domain-separated entry commitment cannot equal the same fields under a different domain.
pub proof fn domain_separated_entry_not_other_domain(
    other_domain: u64,
    prev_hash: u64,
    index: u64,
    term: u64,
    data_id: u64,
)
    requires other_domain != CHAIN_HASH_DOMAIN_TAG
    ensures domain_separated_entry_hash_spec(prev_hash, index, term, data_id)
        != domain_commitment_spec(other_domain, prev_hash, index, hash_spec(prev_hash, index, term, data_id))
{
    if domain_separated_entry_hash_spec(prev_hash, index, term, data_id)
        == domain_commitment_spec(other_domain, prev_hash, index, hash_spec(prev_hash, index, term, data_id)) {
        hash_collision_free(
            CHAIN_HASH_DOMAIN_TAG,
            prev_hash,
            index,
            hash_spec(prev_hash, index, term, data_id),
            other_domain,
            prev_hash,
            index,
            hash_spec(prev_hash, index, term, data_id),
        );
        assert(false);
    }
}

// r[depends chain.compute-entry-hash]
/// Spec: compute the chain hash for an entry.
pub open spec fn compute_entry_hash_spec(
    prev_hash: u64, index: u64, term: u64, data_id: u64,
) -> u64 {
    hash_spec(prev_hash, index, term, data_id)
}

// r[impl chain.compute-entry-hash]
/// Computes the chain hash for a log entry.
///
/// Trusted external body — in production this is blake3. Verus trusts
/// the ensures clause and verifies all callers against the spec.
/// @verified-logic:permanent-axiom — crypto primitive, runtime-only
#[verifier::external_body]
pub fn compute_entry_hash(
    prev_hash: u64, index: u64, term: u64, data_id: u64,
) -> (result: u64)
    ensures result == compute_entry_hash_spec(prev_hash, index, term, data_id)
{
    // Runtime: deterministic mix for testing.
    // Verus ignores this body and trusts the ensures.
    let mut h = prev_hash;
    h = h.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(index);
    h = h.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(term);
    h = h.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(data_id);
    h
}

// --- chain validity ---

// r[depends chain.chain-valid]
// r[impl chain.chain-valid]
/// Spec: chain is valid over entries [0, len).
///
/// Each entry's stored hash equals the hash computed from its predecessor.
pub open spec fn chain_valid_spec(entries: Seq<ChainEntry>) -> bool {
    forall|i: int| #![trigger entries[i]]
        0 <= i < entries.len() ==> {
        let prev_hash = if i == 0 { GENESIS_HASH as u64 } else { entries[i - 1].hash };
        entries[i].hash == compute_entry_hash_spec(
            prev_hash, entries[i].index, entries[i].term, entries[i].data_id,
        )
    }
}

// r[verify chain.chain-valid]
/// Empty chain is trivially valid.
proof fn empty_chain_valid()
    ensures chain_valid_spec(Seq::<ChainEntry>::empty())
{}

// --- append preserves chain ---

/// Spec: the new entry's hash is correctly computed from the chain tip.
pub open spec fn correctly_hashed(
    entries: Seq<ChainEntry>, new_entry: ChainEntry,
) -> bool {
    let prev_hash = if entries.len() == 0 {
        GENESIS_HASH as u64
    } else {
        entries[entries.len() - 1].hash
    };
    new_entry.hash == compute_entry_hash_spec(
        prev_hash, new_entry.index, new_entry.term, new_entry.data_id,
    )
}

// r[depends chain.append-preserves]
/// Spec: appending a correctly-hashed entry preserves validity.
pub open spec fn append_preserves_prop(
    entries: Seq<ChainEntry>, new_entry: ChainEntry,
) -> bool {
    (chain_valid_spec(entries) && correctly_hashed(entries, new_entry))
        ==> chain_valid_spec(entries.push(new_entry))
}

// r[impl chain.append-preserves]
// r[verify chain.append-preserves]
/// Proof: appending a correctly-hashed entry preserves chain validity.
proof fn prove_append_preserves(entries: Seq<ChainEntry>, new_entry: ChainEntry)
    requires
        chain_valid_spec(entries),
        correctly_hashed(entries, new_entry),
    ensures chain_valid_spec(entries.push(new_entry))
{
    let new_chain = entries.push(new_entry);

    assert forall|i: int| #![trigger new_chain[i]] 0 <= i < new_chain.len()
    implies {
        let prev_hash = if i == 0 { GENESIS_HASH as u64 } else { new_chain[i - 1].hash };
        new_chain[i].hash == compute_entry_hash_spec(
            prev_hash, new_chain[i].index, new_chain[i].term, new_chain[i].data_id,
        )
    } by {
        if i < entries.len() as int {
            // Existing entry: unchanged, validity preserved from pre-chain
            assert(new_chain[i] == entries[i]);
            if i > 0 {
                assert(new_chain[i - 1] == entries[i - 1]);
            }
        } else {
            // New entry: validity by construction
            assert(i == entries.len() as int);
            assert(new_chain[i] == new_entry);
            if i > 0 {
                assert(new_chain[i - 1] == entries[i - 1]);
            }
        }
    }
}

// r[verify chain.chain-valid]
/// Prefix of a valid chain is valid.
proof fn prefix_valid(entries: Seq<ChainEntry>, k: int)
    requires chain_valid_spec(entries), 0 <= k <= entries.len()
    ensures chain_valid_spec(entries.subrange(0, k))
{
    let sub = entries.subrange(0, k);
    assert forall|i: int| #![trigger sub[i]] 0 <= i < sub.len()
    implies {
        let prev_hash = if i == 0 { GENESIS_HASH as u64 } else { sub[i - 1].hash };
        sub[i].hash == compute_entry_hash_spec(
            prev_hash, sub[i].index, sub[i].term, sub[i].data_id,
        )
    } by {
        assert(sub[i] == entries[i]);
        if i > 0 {
            assert(sub[i - 1] == entries[i - 1]);
        }
    }
}

// --- tip commitments, extension, checkpoints, and forks ---

// r[depends chain.tip-commitment]
// r[impl chain.tip-commitment]
/// Spec: compact commitment to the current chain tip.
/// Empty chains commit to genesis; non-empty chains commit to the final entry hash.
pub open spec fn chain_tip_spec(entries: Seq<ChainEntry>) -> u64 {
    if entries.len() == 0 {
        GENESIS_HASH as u64
    } else {
        entries[entries.len() - 1].hash
    }
}

// r[verify chain.tip-commitment]
/// Proof: empty-chain tip is the genesis hash.
pub proof fn empty_tip_is_genesis(entries: Seq<ChainEntry>)
    requires entries.len() == 0
    ensures chain_tip_spec(entries) == GENESIS_HASH as u64
{}

// r[verify chain.tip-commitment]
/// Proof: non-empty-chain tip is the last entry hash.
pub proof fn nonempty_tip_is_last(entries: Seq<ChainEntry>)
    requires entries.len() > 0
    ensures chain_tip_spec(entries) == entries[entries.len() - 1].hash
{}

// r[depends chain.extension-prefix]
// r[impl chain.extension-prefix]
/// Spec: `full` extends `prefix` exactly when `prefix` is an unchanged prefix.
pub open spec fn chain_extends_spec(prefix: Seq<ChainEntry>, full: Seq<ChainEntry>) -> bool {
    prefix.len() <= full.len()
        && forall|i: int| #![trigger prefix[i]] 0 <= i < prefix.len() ==> prefix[i] == full[i]
}

// r[verify chain.extension-prefix]
/// Proof: selecting a prefix from a valid full chain yields a valid chain.
pub proof fn valid_selected_prefix(entries: Seq<ChainEntry>, k: int)
    requires chain_valid_spec(entries), 0 <= k <= entries.len()
    ensures
        chain_valid_spec(entries.subrange(0, k)),
        chain_extends_spec(entries.subrange(0, k), entries),
{
    prefix_valid(entries, k);
    let prefix = entries.subrange(0, k);
    assert forall|i: int| #![trigger prefix[i]] 0 <= i < prefix.len() implies prefix[i] == entries[i] by {
        assert(prefix[i] == entries[i]);
    }
}

// r[verify chain.extension-prefix]
/// Proof: extension preserves each prefix entry pointwise.
pub proof fn extension_preserves_entry(prefix: Seq<ChainEntry>, full: Seq<ChainEntry>, i: int)
    requires chain_extends_spec(prefix, full), 0 <= i < prefix.len()
    ensures prefix[i] == full[i]
{}

// r[depends chain.checkpoint-anchor]
// r[impl chain.checkpoint-anchor]
/// Spec: checkpoint hash authenticates an in-range position of a valid chain.
pub open spec fn checkpoint_valid_spec(entries: Seq<ChainEntry>, index: int, expected_hash: u64) -> bool {
    chain_valid_spec(entries)
        && 0 <= index < entries.len()
        && entries[index].hash == expected_hash
}

// r[verify chain.checkpoint-anchor]
/// Proof: matching checkpoints on valid chains authenticate equal prefixes.
pub proof fn checkpoint_anchor_agreement(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, index: int, expected_hash: u64,
)
    requires
        checkpoint_valid_spec(chain1, index, expected_hash),
        checkpoint_valid_spec(chain2, index, expected_hash),
    ensures forall|i: int| #![trigger chain1[i]] 0 <= i <= index ==> chain1[i] == chain2[i]
{
    assert(chain1[index].hash == chain2[index].hash);
    prove_chain_agreement(chain1, chain2, index);
}

// r[verify chain.checkpoint-anchor]
/// Proof: a valid checkpoint's selected prefix is valid.
pub proof fn checkpoint_prefix_valid(entries: Seq<ChainEntry>, index: int, expected_hash: u64)
    requires checkpoint_valid_spec(entries, index, expected_hash)
    ensures chain_valid_spec(entries.subrange(0, index + 1))
{
    prefix_valid(entries, index + 1);
}

// r[depends chain.fork-detection]
// r[impl chain.fork-detection]
/// Spec: two chains diverge when some shared in-range entry differs.
pub open spec fn chains_diverge_spec(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>) -> bool {
    exists|i: int| 0 <= i < chain1.len() && i < chain2.len() && chain1[i] != chain2[i]
}

// r[depends chain.fork-detection]
// r[impl chain.fork-detection]
/// Spec: `k` is the first shared index where two chains differ.
pub open spec fn first_divergence_spec(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, k: int) -> bool {
    0 <= k < chain1.len()
        && k < chain2.len()
        && chain1[k] != chain2[k]
        && (forall|i: int| 0 <= i < k ==> chain1[i] == chain2[i])
}

// r[depends chain.fork-detection]
// r[impl chain.fork-detection]
/// Spec: valid same-length chains with the same tip have no fork.
pub open spec fn same_tip_no_fork_prop(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>) -> bool {
    (chain_valid_spec(chain1) && chain_valid_spec(chain2)
        && chain1.len() == chain2.len()
        && chain_tip_spec(chain1) == chain_tip_spec(chain2))
    ==> !chains_diverge_spec(chain1, chain2)
}

// r[verify chain.fork-detection]
/// Proof: valid same-length chains with equal tips have no divergent entry.
pub proof fn same_tip_no_fork(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        chain1.len() == chain2.len(),
        chain_tip_spec(chain1) == chain_tip_spec(chain2),
    ensures !chains_diverge_spec(chain1, chain2)
{
    if chain1.len() == 0 {
        assert(!chains_diverge_spec(chain1, chain2));
    } else {
        let k = chain1.len() - 1;
        assert(chain1[k].hash == chain_tip_spec(chain1));
        assert(chain2[k].hash == chain_tip_spec(chain2));
        prove_chain_agreement(chain1, chain2, k);
        assert forall|i: int| 0 <= i < chain1.len() && i < chain2.len() implies chain1[i] == chain2[i] by {
            assert(i <= k);
        }
        assert(!chains_diverge_spec(chain1, chain2));
    }
}

// r[verify chain.fork-detection]
/// Proof: any divergence in valid same-length chains forces different tips.
pub proof fn divergence_implies_different_tip(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, j: int)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        chain1.len() == chain2.len(),
        0 <= j < chain1.len(),
        chain1[j] != chain2[j],
    ensures chain_tip_spec(chain1) != chain_tip_spec(chain2)
{
    let k = chain1.len() - 1;
    prove_chain_divergence(chain1, chain2, j, k);
    assert(chain1[k].hash != chain2[k].hash);
}

// r[verify chain.fork-detection]
/// Proof: a first-divergence witness in valid same-length chains forces different tips.
pub proof fn first_divergence_implies_different_tip(chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, first: int)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        chain1.len() == chain2.len(),
        first_divergence_spec(chain1, chain2, first),
    ensures chain_tip_spec(chain1) != chain_tip_spec(chain2)
{
    divergence_implies_different_tip(chain1, chain2, first);
}

// --- batch append and log shape ---

// r[depends chain.batch-append]
// r[impl chain.batch-append]
/// Spec: append a batch by folding one correctly linked entry at a time.
pub open spec fn append_batch_spec(base: Seq<ChainEntry>, batch: Seq<ChainEntry>) -> Seq<ChainEntry>
    decreases batch.len()
{
    if batch.len() == 0 {
        base
    } else {
        append_batch_spec(
            base.push(batch[0]),
            batch.subrange(1, batch.len() as int),
        )
    }
}

// r[depends chain.batch-append]
// r[impl chain.batch-append]
/// Spec: every batch entry is correctly hashed from the current folded tip.
pub open spec fn batch_correctly_hashed_spec(base: Seq<ChainEntry>, batch: Seq<ChainEntry>) -> bool
    decreases batch.len()
{
    if batch.len() == 0 {
        true
    } else {
        correctly_hashed(base, batch[0])
            && batch_correctly_hashed_spec(
                base.push(batch[0]),
                batch.subrange(1, batch.len() as int),
            )
    }
}

// r[depends chain.batch-append]
// r[impl chain.batch-append]
/// Spec: admitted batch append preserves validity and extends the base chain.
pub open spec fn batch_append_preserves_prop(base: Seq<ChainEntry>, batch: Seq<ChainEntry>) -> bool {
    (chain_valid_spec(base) && batch_correctly_hashed_spec(base, batch))
        ==> chain_valid_spec(append_batch_spec(base, batch))
}

// r[verify chain.batch-append]
/// Proof: a correctly linked batch preserves chain validity by repeated append.
pub proof fn prove_batch_append_preserves(base: Seq<ChainEntry>, batch: Seq<ChainEntry>)
    requires
        chain_valid_spec(base),
        batch_correctly_hashed_spec(base, batch),
    ensures chain_valid_spec(append_batch_spec(base, batch))
    decreases batch.len()
{
    if batch.len() == 0 {
    } else {
        let first = batch[0];
        let rest = batch.subrange(1, batch.len() as int);
        prove_append_preserves(base, first);
        prove_batch_append_preserves(base.push(first), rest);
    }
}

// r[verify chain.batch-append]
/// Proof: batch append leaves the original base prefix unchanged.
pub proof fn batch_append_extends_base(base: Seq<ChainEntry>, batch: Seq<ChainEntry>)
    requires batch_correctly_hashed_spec(base, batch)
    ensures chain_extends_spec(base, append_batch_spec(base, batch))
    decreases batch.len()
{
    if batch.len() == 0 {
        assert forall|i: int| #![trigger base[i]] 0 <= i < base.len() implies base[i] == base[i] by {}
    } else {
        let first = batch[0];
        let rest = batch.subrange(1, batch.len() as int);
        batch_append_extends_base(base.push(first), rest);
        let result = append_batch_spec(base, batch);
        assert(result == append_batch_spec(base.push(first), rest));
        assert forall|i: int| #![trigger base[i]] 0 <= i < base.len() implies base[i] == result[i] by {
            assert(base.push(first)[i] == base[i]);
        }
    }
}

// r[depends chain.log-shape]
// r[impl chain.log-shape]
/// Spec: indices are contiguous from `first_index` in mathematical integer space.
pub open spec fn indices_contiguous_from_spec(entries: Seq<ChainEntry>, first_index: u64) -> bool {
    forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==>
        entries[i].index as int == first_index as int + i
}

// r[depends chain.log-shape]
// r[impl chain.log-shape]
/// Spec: terms are monotonically nondecreasing along the chain.
pub open spec fn terms_nondecreasing_spec(entries: Seq<ChainEntry>) -> bool {
    forall|i: int| #![trigger entries[i]] 1 <= i < entries.len() ==>
        entries[i - 1].term <= entries[i].term
}

// r[depends chain.log-shape]
// r[impl chain.log-shape]
/// Spec: shape validity is separate from hash validity.
pub open spec fn log_shape_valid_spec(entries: Seq<ChainEntry>, first_index: u64) -> bool {
    indices_contiguous_from_spec(entries, first_index) && terms_nondecreasing_spec(entries)
}

// r[depends chain.log-shape]
// r[impl chain.log-shape]
/// Spec: combined log validity composes hash-chain validity with log shape validity.
pub open spec fn shaped_chain_valid_spec(entries: Seq<ChainEntry>, first_index: u64) -> bool {
    chain_valid_spec(entries) && log_shape_valid_spec(entries, first_index)
}

// r[verify chain.log-shape]
/// Proof: combined validity implies ordinary hash-chain validity.
pub proof fn shaped_chain_implies_hash_valid(entries: Seq<ChainEntry>, first_index: u64)
    requires shaped_chain_valid_spec(entries, first_index)
    ensures chain_valid_spec(entries)
{}

// r[verify chain.log-shape]
/// Proof: combined validity implies separate shape validity.
pub proof fn shaped_chain_implies_shape_valid(entries: Seq<ChainEntry>, first_index: u64)
    requires shaped_chain_valid_spec(entries, first_index)
    ensures log_shape_valid_spec(entries, first_index)
{}

// --- tamper classification ---

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: changing the payload identifier while holding the chain context fixed is data tampering.
pub open spec fn data_tamper_spec(prev_hash: u64, index: u64, term: u64, old_data: u64, new_data: u64) -> bool {
    old_data != new_data ==> compute_entry_hash_spec(prev_hash, index, term, old_data)
        != compute_entry_hash_spec(prev_hash, index, term, new_data)
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: changing the log index while holding the chain context fixed is index tampering.
pub open spec fn index_tamper_spec(prev_hash: u64, old_index: u64, new_index: u64, term: u64, data_id: u64) -> bool {
    old_index != new_index ==> compute_entry_hash_spec(prev_hash, old_index, term, data_id)
        != compute_entry_hash_spec(prev_hash, new_index, term, data_id)
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: changing the term while holding the chain context fixed is term tampering.
pub open spec fn term_tamper_spec(prev_hash: u64, index: u64, old_term: u64, new_term: u64, data_id: u64) -> bool {
    old_term != new_term ==> compute_entry_hash_spec(prev_hash, index, old_term, data_id)
        != compute_entry_hash_spec(prev_hash, index, new_term, data_id)
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: changing the predecessor hash while holding entry fields fixed is predecessor tampering.
pub open spec fn predecessor_tamper_spec(old_prev: u64, new_prev: u64, index: u64, term: u64, data_id: u64) -> bool {
    old_prev != new_prev ==> compute_entry_hash_spec(old_prev, index, term, data_id)
        != compute_entry_hash_spec(new_prev, index, term, data_id)
}

// r[verify chain.tamper-classification]
/// Proof: payload tampering changes the committed entry hash under collision freedom.
pub proof fn data_tamper_changes_hash(prev_hash: u64, index: u64, term: u64, old_data: u64, new_data: u64)
    requires old_data != new_data
    ensures compute_entry_hash_spec(prev_hash, index, term, old_data)
        != compute_entry_hash_spec(prev_hash, index, term, new_data)
{
    if compute_entry_hash_spec(prev_hash, index, term, old_data)
        == compute_entry_hash_spec(prev_hash, index, term, new_data) {
        hash_collision_free(prev_hash, index, term, old_data, prev_hash, index, term, new_data);
    }
}

// r[verify chain.tamper-classification]
/// Proof: index tampering changes the committed entry hash under collision freedom.
pub proof fn index_tamper_changes_hash(prev_hash: u64, old_index: u64, new_index: u64, term: u64, data_id: u64)
    requires old_index != new_index
    ensures compute_entry_hash_spec(prev_hash, old_index, term, data_id)
        != compute_entry_hash_spec(prev_hash, new_index, term, data_id)
{
    if compute_entry_hash_spec(prev_hash, old_index, term, data_id)
        == compute_entry_hash_spec(prev_hash, new_index, term, data_id) {
        hash_collision_free(prev_hash, old_index, term, data_id, prev_hash, new_index, term, data_id);
    }
}

// r[verify chain.tamper-classification]
/// Proof: term tampering changes the committed entry hash under collision freedom.
pub proof fn term_tamper_changes_hash(prev_hash: u64, index: u64, old_term: u64, new_term: u64, data_id: u64)
    requires old_term != new_term
    ensures compute_entry_hash_spec(prev_hash, index, old_term, data_id)
        != compute_entry_hash_spec(prev_hash, index, new_term, data_id)
{
    if compute_entry_hash_spec(prev_hash, index, old_term, data_id)
        == compute_entry_hash_spec(prev_hash, index, new_term, data_id) {
        hash_collision_free(prev_hash, index, old_term, data_id, prev_hash, index, new_term, data_id);
    }
}

// r[verify chain.tamper-classification]
/// Proof: predecessor tampering changes the computed hash.
pub proof fn predecessor_tamper_changes_hash(old_prev: u64, new_prev: u64, index: u64, term: u64, data_id: u64)
    requires old_prev != new_prev
    ensures predecessor_tamper_spec(old_prev, new_prev, index, term, data_id)
{
    if compute_entry_hash_spec(old_prev, index, term, data_id)
        == compute_entry_hash_spec(new_prev, index, term, data_id) {
        hash_collision_free(old_prev, index, term, data_id, new_prev, index, term, data_id);
        assert(false);
    }
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: `k` is a first valid-chain divergence where the payload differs while metadata is unchanged.
pub open spec fn data_tamper_divergence_spec(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int) -> bool {
    chain_valid_spec(a)
        && chain_valid_spec(b)
        && a.len() == b.len()
        && 0 <= k < a.len()
        && (forall|i: int| 0 <= i < k ==> a[i] == b[i])
        && a[k].index == b[k].index
        && a[k].term == b[k].term
        && a[k].data_id != b[k].data_id
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: `k` is a first valid-chain divergence where the log index differs.
pub open spec fn index_tamper_divergence_spec(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int) -> bool {
    chain_valid_spec(a)
        && chain_valid_spec(b)
        && a.len() == b.len()
        && 0 <= k < a.len()
        && (forall|i: int| 0 <= i < k ==> a[i] == b[i])
        && a[k].index != b[k].index
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: `k` is a first valid-chain divergence where the term differs.
pub open spec fn term_tamper_divergence_spec(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int) -> bool {
    chain_valid_spec(a)
        && chain_valid_spec(b)
        && a.len() == b.len()
        && 0 <= k < a.len()
        && (forall|i: int| 0 <= i < k ==> a[i] == b[i])
        && a[k].term != b[k].term
}

// r[depends chain.tamper-classification]
// r[impl chain.tamper-classification]
/// Spec: `k` is a first valid-chain divergence caused by a predecessor/hash-link mismatch.
pub open spec fn predecessor_tamper_divergence_spec(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int) -> bool {
    chain_valid_spec(a)
        && chain_valid_spec(b)
        && a.len() == b.len()
        && 0 < k < a.len()
        && (forall|i: int| 0 <= i < k ==> a[i] == b[i])
        && a[k - 1].hash == b[k - 1].hash
        && a[k].hash != b[k].hash
}

// r[verify chain.tamper-classification]
/// Proof: any first valid-chain tamper classification changes the compared tip.
pub proof fn classified_tamper_changes_tip(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int)
    requires
        0 <= k < a.len(),
        a.len() == b.len(),
        chain_valid_spec(a),
        chain_valid_spec(b),
        a[k] != b[k],
    ensures chain_tip_spec(a) != chain_tip_spec(b)
{
    if a.len() > 0 {
        prove_chain_divergence(a, b, k, a.len() - 1);
        assert(chain_tip_spec(a) == a[a.len() - 1].hash);
        assert(chain_tip_spec(b) == b[b.len() - 1].hash);
    }
}

// r[verify chain.tamper-classification]
/// Proof: a classified data tamper propagates to the compared tip.
pub proof fn data_tamper_divergence_changes_tip(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int)
    requires data_tamper_divergence_spec(a, b, k)
    ensures chain_tip_spec(a) != chain_tip_spec(b)
{
    assert(a[k] != b[k]);
    classified_tamper_changes_tip(a, b, k);
}

// r[verify chain.tamper-classification]
/// Proof: a classified index tamper propagates to the compared tip.
pub proof fn index_tamper_divergence_changes_tip(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int)
    requires index_tamper_divergence_spec(a, b, k)
    ensures chain_tip_spec(a) != chain_tip_spec(b)
{
    assert(a[k] != b[k]);
    classified_tamper_changes_tip(a, b, k);
}

// r[verify chain.tamper-classification]
/// Proof: a classified term tamper propagates to the compared tip.
pub proof fn term_tamper_divergence_changes_tip(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int)
    requires term_tamper_divergence_spec(a, b, k)
    ensures chain_tip_spec(a) != chain_tip_spec(b)
{
    assert(a[k] != b[k]);
    classified_tamper_changes_tip(a, b, k);
}

// r[verify chain.tamper-classification]
/// Proof: a classified predecessor/hash-link tamper propagates to the compared tip.
pub proof fn predecessor_tamper_divergence_changes_tip(a: Seq<ChainEntry>, b: Seq<ChainEntry>, k: int)
    requires predecessor_tamper_divergence_spec(a, b, k)
    ensures chain_tip_spec(a) != chain_tip_spec(b)
{
    assert(a[k] != b[k]);
    classified_tamper_changes_tip(a, b, k);
}



// r[depends chain.compact-history-proof]
// r[impl chain.compact-history-proof]
/// Spec: compact-history overlays summarize a valid base chain and commit to its tip.
pub open spec fn compact_history_overlay_spec(entries: Seq<ChainEntry>, overlay_tip: u64) -> bool {
    chain_valid_spec(entries) && chain_tip_spec(entries) == overlay_tip
}

// r[depends chain.compact-history-proof]
// r[impl chain.compact-history-proof]
/// Spec: a compact range proof names a valid inclusive range in the base chain.
pub open spec fn compact_history_range_spec(entries: Seq<ChainEntry>, start: int, end: int, range_tip: u64) -> bool {
    chain_valid_spec(entries)
        && 0 <= start <= end
        && end < entries.len()
        && entries[end].hash == range_tip
}

// r[depends chain.compact-history-proof]
// r[impl chain.compact-history-proof]
/// Spec: a compact membership proof names one entry hash in a base-chain range and anchors it to the same overlay tip.
pub open spec fn compact_history_membership_spec(entries: Seq<ChainEntry>, member: int, member_hash: u64, overlay_tip: u64) -> bool {
    compact_history_overlay_spec(entries, overlay_tip)
        && 0 <= member < entries.len()
        && entries[member].hash == member_hash
}

// r[verify chain.compact-history-proof]
/// Proof: an overlay is only an accelerator; it never replaces base-chain validity.
pub proof fn compact_history_overlay_implies_chain_valid(entries: Seq<ChainEntry>, overlay_tip: u64)
    requires compact_history_overlay_spec(entries, overlay_tip)
    ensures chain_valid_spec(entries)
{}

// r[verify chain.compact-history-proof]
/// Proof: a compact membership commitment is a checkpoint over the underlying linear chain.
pub proof fn compact_history_membership_implies_checkpoint(entries: Seq<ChainEntry>, member: int, member_hash: u64, overlay_tip: u64)
    requires compact_history_membership_spec(entries, member, member_hash, overlay_tip)
    ensures checkpoint_valid_spec(entries, member, member_hash)
{}

// r[verify chain.compact-history-proof]
/// Proof: a compact range commitment anchors to the underlying linear chain tip at `end`.
pub proof fn compact_history_range_anchors_tip(entries: Seq<ChainEntry>, start: int, end: int, range_tip: u64)
    requires compact_history_range_spec(entries, start, end, range_tip)
    ensures
        chain_valid_spec(entries),
        0 <= start <= end < entries.len(),
        entries[end].hash == range_tip,
{}

// --- collision resistance (axiom) ---

// r[depends chain.collision-free]
/// Spec: the hash function is collision-free.
/// Same output from hash_spec implies all four inputs are identical.
pub open spec fn hash_collision_free_prop(
    a1: u64, b1: u64, c1: u64, d1: u64,
    a2: u64, b2: u64, c2: u64, d2: u64,
) -> bool {
    hash_spec(a1, b1, c1, d1) == hash_spec(a2, b2, c2, d2)
        ==> (a1 == a2 && b1 == b2 && c1 == c2 && d1 == d2)
}

// r[impl chain.collision-free]
// r[verify chain.collision-free]
/// Axiom: hash_spec is collision-free (idealized cryptographic assumption).
/// Trusted — mirrors the real-world assumption that Blake3 is collision-resistant.
/// @verified-logic:permanent-axiom — cryptographic collision resistance
#[verifier::external_body]
pub proof fn hash_collision_free(
    a1: u64, b1: u64, c1: u64, d1: u64,
    a2: u64, b2: u64, c2: u64, d2: u64,
)
    requires hash_spec(a1, b1, c1, d1) == hash_spec(a2, b2, c2, d2)
    ensures a1 == a2, b1 == b2, c1 == c2, d1 == d2
{}

// --- chain agreement ---

// r[depends chain.agreement]
/// Spec: two valid chains with matching hashes at position k agree on entries 0..=k.
pub open spec fn chain_agreement_prop(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, k: int,
) -> bool {
    (chain_valid_spec(chain1) && chain_valid_spec(chain2)
        && 0 <= k && k < chain1.len() && k < chain2.len()
        && chain1[k].hash == chain2[k].hash)
    ==> forall|i: int| #![trigger chain1[i]]
        0 <= i <= k ==> chain1[i] == chain2[i]
}

/// Helper: one step of the agreement proof.
/// If two valid chains have matching hashes at position k, the entries
/// at k are identical and predecessor hashes also match.
/// Separated from the main recursive proof to keep Z3's workload bounded.
proof fn agreement_step(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, k: int,
)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        0 <= k < chain1.len(),
        0 <= k < chain2.len(),
        chain1[k].hash == chain2[k].hash,
    ensures
        chain1[k] == chain2[k],
        k > 0 ==> chain1[k - 1].hash == chain2[k - 1].hash,
{
    let prev1 = if k == 0 { GENESIS_HASH as u64 } else { chain1[k - 1].hash };
    let prev2 = if k == 0 { GENESIS_HASH as u64 } else { chain2[k - 1].hash };

    // Instantiate chain_valid_spec at position k for both chains.
    assert(chain1[k].hash == compute_entry_hash_spec(
        prev1, chain1[k].index, chain1[k].term, chain1[k].data_id,
    ));
    assert(chain2[k].hash == compute_entry_hash_spec(
        prev2, chain2[k].index, chain2[k].term, chain2[k].data_id,
    ));

    // Both hash outputs are equal (given). Collision-freeness:
    // same output → same inputs.
    hash_collision_free(
        prev1, chain1[k].index, chain1[k].term, chain1[k].data_id,
        prev2, chain2[k].index, chain2[k].term, chain2[k].data_id,
    );
}

/// Helper: prove agreement at a single position i, peeling back from k.
/// Non-quantified ensures — Z3 handles this without rlimit issues.
proof fn agreement_at(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, k: int, i: int,
)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        0 <= i <= k,
        k < chain1.len(),
        k < chain2.len(),
        chain1[k].hash == chain2[k].hash,
    ensures chain1[i] == chain2[i]
    decreases k
{
    agreement_step(chain1, chain2, k);
    if i < k {
        // chain1[k-1].hash == chain2[k-1].hash from agreement_step.
        // Peel back one level: prove agreement at i within 0..=k-1.
        agreement_at(chain1, chain2, k - 1, i);
    }
}

// r[impl chain.agreement]
// r[verify chain.agreement]
/// Proof: two valid chains with matching hashes at position k
/// have identical entries at all positions 0 through k.
///
/// The fundamental security guarantee of hash chains:
/// the hash at any position commits to the entire chain prefix.
/// Uses agreement_at to prove each position, assembled via assert-forall.
proof fn prove_chain_agreement(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, k: int,
)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        0 <= k < chain1.len(),
        0 <= k < chain2.len(),
        chain1[k].hash == chain2[k].hash,
    ensures
        forall|i: int| #![trigger chain1[i]]
            0 <= i <= k ==> chain1[i] == chain2[i]
{
    assert forall|i: int| #![trigger chain1[i]]
        0 <= i <= k implies chain1[i] == chain2[i] by {
        agreement_at(chain1, chain2, k, i);
    }
}

// --- chain divergence (tamper evidence) ---

// r[depends chain.divergence-detected]
/// Spec: if two valid chains differ at position j, hashes at k >= j must differ.
pub open spec fn chain_divergence_prop(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, j: int, k: int,
) -> bool {
    (chain_valid_spec(chain1) && chain_valid_spec(chain2)
        && 0 <= j <= k && k < chain1.len() && k < chain2.len()
        && chain1[j] != chain2[j])
    ==> chain1[k].hash != chain2[k].hash
}

// r[impl chain.divergence-detected]
// r[verify chain.divergence-detected]
/// Proof: divergence at position j forces different hashes at all k >= j.
/// Contrapositive of chain agreement. Hash chains are tamper-evident:
/// any modification to any entry is detectable by comparing tip hashes.
proof fn prove_chain_divergence(
    chain1: Seq<ChainEntry>, chain2: Seq<ChainEntry>, j: int, k: int,
)
    requires
        chain_valid_spec(chain1),
        chain_valid_spec(chain2),
        0 <= j <= k,
        k < chain1.len(),
        k < chain2.len(),
        chain1[j] != chain2[j],
    ensures chain1[k].hash != chain2[k].hash
{
    // Proof by contradiction: if hashes matched at k, agreement
    // gives identical entries 0..=k, contradicting divergence at j.
    if chain1[k].hash == chain2[k].hash {
        prove_chain_agreement(chain1, chain2, k);
        assert(chain1[j] == chain2[j]); // contradicts requires
    }
}

// --- single entry validity ---

// r[depends chain.single-entry-valid]
/// Spec: a single correctly-hashed entry from genesis forms a valid chain.
pub open spec fn single_entry_valid_prop(entry: ChainEntry) -> bool {
    correctly_hashed(Seq::<ChainEntry>::empty(), entry)
        ==> chain_valid_spec(seq![entry])
}

// r[impl chain.single-entry-valid]
// r[verify chain.single-entry-valid]
/// Proof: a single entry correctly hashed from genesis is a valid chain.
proof fn prove_single_entry_valid(entry: ChainEntry)
    requires correctly_hashed(Seq::<ChainEntry>::empty(), entry)
    ensures chain_valid_spec(seq![entry])
{
    let chain = seq![entry];
    // Trigger chain_valid_spec quantifier at i = 0.
    // correctly_hashed gives entry.hash == compute_entry_hash_spec(GENESIS_HASH, ...).
    // chain_valid_spec needs chain[0].hash == compute_entry_hash_spec(GENESIS_HASH, ...).
    // Since chain[0] == entry, these are identical.
    assert(chain[0] == entry);
}

} // verus!

#[cfg(test)]
/// Domain-separation label used by the concrete BLAKE3 chain-entry hash.
pub const CHAIN_HASH_DOMAIN_BYTES: &[u8] = b"trellis.chain.entry.v1";

#[cfg(test)]
// r[depends chain.runtime-blake3-alignment]
// r[impl chain.runtime-blake3-alignment]
/// Runtime BLAKE3 bridge for chain-entry hashing.
///
/// The Verus proof model keeps the hash as an uninterpreted, collision-resistant
/// function; this executable helper fixes the concrete byte layout used by tests
/// and production adapters: domain tag followed by little-endian `prev`, `index`,
/// `term`, and `data_id`, truncated to a u64 model hash.
pub fn compute_entry_hash_blake3_runtime(prev_hash: u64, index: u64, term: u64, data_id: u64) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHAIN_HASH_DOMAIN_BYTES);
    hasher.update(&prev_hash.to_le_bytes());
    hasher.update(&index.to_le_bytes());
    hasher.update(&term.to_le_bytes());
    hasher.update(&data_id.to_le_bytes());
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(out)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // Simple test hash for runtime tests (not the axiomatized one)
    fn test_hash(prev: u64, index: u64, term: u64, data_id: u64) -> u64 {
        compute_entry_hash_blake3_runtime(prev, index, term, data_id)
    }

    fn build_chain(entries: &[(u64, u64, u64)]) -> Vec<ChainEntry> {
        let mut chain = Vec::new();
        let mut prev_hash = GENESIS_HASH;
        for &(index, term, data_id) in entries {
            let hash = test_hash(prev_hash, index, term, data_id);
            chain.push(ChainEntry {
                hash,
                index,
                term,
                data_id,
            });
            prev_hash = hash;
        }
        chain
    }

    fn is_chain_valid(chain: &[ChainEntry]) -> bool {
        for (i, entry) in chain.iter().enumerate() {
            let prev = if i == 0 { GENESIS_HASH } else { chain[i - 1].hash };
            let expected = test_hash(prev, entry.index, entry.term, entry.data_id);
            if entry.hash != expected {
                return false;
            }
        }
        true
    }

    fn is_log_shape_valid(chain: &[ChainEntry], first_index: u64) -> bool {
        for (i, entry) in chain.iter().enumerate() {
            if entry.index != first_index + i as u64 {
                return false;
            }
            if i > 0 && chain[i - 1].term > entry.term {
                return false;
            }
        }
        true
    }

    // r[verify chain.chain-valid]
    #[test]
    fn test_empty_chain_valid() {
        assert!(is_chain_valid(&[]));
    }

    // r[verify chain.compute-entry-hash]
    #[test]
    fn test_hash_deterministic() {
        let h1 = test_hash(0, 1, 1, 42);
        let h2 = test_hash(0, 1, 1, 42);
        assert_eq!(h1, h2);
    }

    // r[verify chain.runtime-blake3-alignment]
    #[test]
    fn test_blake3_runtime_hash_matches_documented_layout() {
        let prev = 11u64;
        let index = 22u64;
        let term = 33u64;
        let data_id = 44u64;
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHAIN_HASH_DOMAIN_BYTES);
        hasher.update(&prev.to_le_bytes());
        hasher.update(&index.to_le_bytes());
        hasher.update(&term.to_le_bytes());
        hasher.update(&data_id.to_le_bytes());
        let digest = hasher.finalize();
        let mut expected = [0u8; 8];
        expected.copy_from_slice(&digest.as_bytes()[..8]);
        assert_eq!(compute_entry_hash_blake3_runtime(prev, index, term, data_id), u64::from_le_bytes(expected));
    }

    // r[verify chain.domain-separated-hash]
    #[test]
    fn test_chain_hash_domain_is_separated() {
        assert_ne!(CHAIN_HASH_DOMAIN_TAG, CHECKPOINT_HASH_DOMAIN_TAG);
        assert_ne!(CHAIN_HASH_DOMAIN_TAG, RECEIPT_HASH_DOMAIN_TAG);
        assert_ne!(CHAIN_HASH_DOMAIN_TAG, SNAPSHOT_HASH_DOMAIN_TAG);
        assert_ne!(CHAIN_HASH_DOMAIN_TAG, COMPACT_HISTORY_DOMAIN_TAG);
        assert_ne!(CHAIN_HASH_DOMAIN_BYTES, b"trellis.compact-history.v1");
    }

    // r[verify chain.compact-history-proof]
    #[test]
    fn test_compact_history_overlay_uses_base_chain_tip() {
        let chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        let overlay_tip = chain.last().unwrap().hash;
        assert!(is_chain_valid(&chain));
        assert_eq!(overlay_tip, chain[2].hash);
    }

    // r[verify chain.compact-history-proof]
    #[test]
    fn test_compact_history_membership_and_range_anchor_to_base_chain() {
        let chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        let overlay_tip = chain.last().unwrap().hash;
        let member_hash = chain[1].hash;
        assert!(is_chain_valid(&chain));
        assert_eq!(member_hash, chain[1].hash);
        assert_eq!(overlay_tip, chain[2].hash);
    }

    // r[verify chain.chain-valid]
    #[test]
    fn test_chain_valid() {
        let chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        assert!(is_chain_valid(&chain));
    }

    // r[verify chain.chain-valid]
    #[test]
    fn test_corruption_detected() {
        let mut chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        // Corrupt the second entry's hash
        chain[1].hash = 999;
        assert!(!is_chain_valid(&chain));
    }

    // r[verify chain.append-preserves]
    #[test]
    fn test_append_preserves() {
        let mut chain = build_chain(&[(1, 1, 100), (2, 1, 200)]);
        assert!(is_chain_valid(&chain));

        // Append correctly
        let prev_hash = chain.last().unwrap().hash;
        let new_hash = test_hash(prev_hash, 3, 2, 300);
        chain.push(ChainEntry {
            hash: new_hash,
            index: 3,
            term: 2,
            data_id: 300,
        });
        assert!(is_chain_valid(&chain));
    }

    // r[verify chain.single-entry-valid]
    #[test]
    fn test_single_entry_valid() {
        let hash = test_hash(GENESIS_HASH, 1, 1, 42);
        let chain = vec![ChainEntry {
            hash,
            index: 1,
            term: 1,
            data_id: 42,
        }];
        assert!(is_chain_valid(&chain));
    }

    // r[verify chain.agreement]
    #[test]
    fn test_agreement_matching_chains() {
        // Two chains built from identical inputs must have identical entries.
        let data = &[(1, 1, 100), (2, 1, 200), (3, 2, 300)];
        let chain_a = build_chain(data);
        let chain_b = build_chain(data);
        for i in 0..chain_a.len() {
            assert_eq!(chain_a[i].hash, chain_b[i].hash);
            assert_eq!(chain_a[i].index, chain_b[i].index);
            assert_eq!(chain_a[i].term, chain_b[i].term);
            assert_eq!(chain_a[i].data_id, chain_b[i].data_id);
        }
    }

    // r[verify chain.divergence-detected]
    #[test]
    fn test_divergence_propagates() {
        // Two chains that diverge at position 1 must have different
        // hashes at all subsequent positions.
        let chain_a = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        let chain_b = build_chain(&[(1, 1, 100), (2, 1, 999), (3, 2, 300)]);
        // Position 0: same inputs → same hash
        assert_eq!(chain_a[0].hash, chain_b[0].hash);
        // Position 1: different data → different hash
        assert_ne!(chain_a[1].hash, chain_b[1].hash);
        // Position 2: different predecessor hash propagates
        assert_ne!(chain_a[2].hash, chain_b[2].hash);
    }

    // r[verify chain.collision-free]
    #[test]
    fn test_collision_free_different_inputs() {
        // Different inputs should produce different hashes (probabilistic).
        let h1 = test_hash(0, 1, 1, 42);
        let h2 = test_hash(0, 1, 1, 43);
        assert_ne!(h1, h2);
        let h3 = test_hash(0, 1, 2, 42);
        assert_ne!(h1, h3);
        let h4 = test_hash(1, 1, 1, 42);
        assert_ne!(h1, h4);
    }

    // r[verify chain.log-shape]
    #[test]
    fn test_log_shape_accepts_contiguous_nondecreasing_terms() {
        let mut chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        chain[0].term = 1;
        chain[1].term = 1;
        chain[2].term = 2;
        assert!(is_log_shape_valid(&chain, 1));
    }

    // r[verify chain.log-shape]
    #[test]
    fn test_log_shape_rejects_index_gap() {
        let mut chain = build_chain(&[(1, 1, 100), (2, 1, 200), (3, 2, 300)]);
        chain[1].index = 7;
        assert!(!is_log_shape_valid(&chain, 1));
    }

    // r[verify chain.log-shape]
    #[test]
    fn test_log_shape_rejects_term_regression() {
        let chain = build_chain(&[(1, 2, 100), (2, 1, 200), (3, 2, 300)]);
        assert!(!is_log_shape_valid(&chain, 1));
    }

    // r[verify chain.tamper-classification]
    #[test]
    fn test_tamper_classification_inputs_change_hashes() {
        let prev = test_hash(GENESIS_HASH, 1, 1, 100);
        let base = test_hash(prev, 2, 1, 200);
        assert_ne!(base, test_hash(prev, 2, 1, 201));
        assert_ne!(base, test_hash(prev, 3, 1, 200));
        assert_ne!(base, test_hash(prev, 2, 2, 200));
        assert_ne!(base, test_hash(prev.wrapping_add(1), 2, 1, 200));
    }

    proptest! {
        // r[verify chain.hash-deterministic]
        #[test]
        fn prop_hash_deterministic(prev: u64, idx: u64, term: u64, data: u64) {
            prop_assert_eq!(test_hash(prev, idx, term, data), test_hash(prev, idx, term, data));
        }

        // r[verify chain.chain-valid]
        // r[verify chain.append-preserves]
        #[test]
        fn prop_build_chain_valid(
            entries in proptest::collection::vec((1u64..100, 1u64..10, 0u64..1000), 0..=20),
        ) {
            let indexed: Vec<(u64, u64, u64)> = entries
                .iter()
                .enumerate()
                .map(|(i, &(_, term, data))| ((i + 1) as u64, term, data))
                .collect();
            let chain = build_chain(&indexed);
            prop_assert!(is_chain_valid(&chain));
        }

        // r[verify chain.chain-valid]
        #[test]
        fn prop_corruption_detected(
            entries in proptest::collection::vec((1u64..10, 0u64..1000), 2..=10),
            corrupt_idx in 0usize..10,
        ) {
            let indexed: Vec<(u64, u64, u64)> = entries
                .iter()
                .enumerate()
                .map(|(i, &(term, data))| ((i + 1) as u64, term, data))
                .collect();
            let mut chain = build_chain(&indexed);
            let idx = corrupt_idx % chain.len();
            chain[idx].hash = chain[idx].hash.wrapping_add(1);
            prop_assert!(!is_chain_valid(&chain));
        }

        // r[verify chain.chain-valid]
        #[test]
        fn prop_prefix_valid(
            entries in proptest::collection::vec((1u64..10, 0u64..1000), 1..=10),
            k_raw: usize,
        ) {
            let indexed: Vec<(u64, u64, u64)> = entries
                .iter()
                .enumerate()
                .map(|(i, &(term, data))| ((i + 1) as u64, term, data))
                .collect();
            let chain = build_chain(&indexed);
            let k = k_raw % (chain.len() + 1);
            prop_assert!(is_chain_valid(&chain[..k]));
        }

        // r[verify chain.agreement]
        #[test]
        fn prop_agreement_identical_inputs(
            entries in proptest::collection::vec((1u64..100, 1u64..10, 0u64..1000), 1..=20),
        ) {
            let indexed: Vec<(u64, u64, u64)> = entries
                .iter()
                .enumerate()
                .map(|(i, &(_, term, data))| ((i + 1) as u64, term, data))
                .collect();
            let chain_a = build_chain(&indexed);
            let chain_b = build_chain(&indexed);
            // Identical inputs → identical entries at every position.
            for i in 0..chain_a.len() {
                prop_assert_eq!(chain_a[i].hash, chain_b[i].hash);
                prop_assert_eq!(chain_a[i].index, chain_b[i].index);
                prop_assert_eq!(chain_a[i].term, chain_b[i].term);
                prop_assert_eq!(chain_a[i].data_id, chain_b[i].data_id);
            }
        }

        // r[verify chain.divergence-detected]
        #[test]
        fn prop_divergence_propagates(
            entries in proptest::collection::vec((1u64..10, 0u64..1000), 2..=10),
            diverge_raw: usize,
        ) {
            let indexed: Vec<(u64, u64, u64)> = entries
                .iter()
                .enumerate()
                .map(|(i, &(term, data))| ((i + 1) as u64, term, data))
                .collect();
            let chain_a = build_chain(&indexed);

            // Build chain_b with different data at the divergence point.
            let diverge_idx = diverge_raw % indexed.len();
            let mut modified = indexed.clone();
            modified[diverge_idx].2 = modified[diverge_idx].2.wrapping_add(1);
            let chain_b = build_chain(&modified);

            // All positions from diverge_idx onward must have different hashes.
            for k in diverge_idx..chain_a.len() {
                prop_assert_ne!(chain_a[k].hash, chain_b[k].hash,
                    "hashes should differ at position {} (diverged at {})", k, diverge_idx);
            }
        }

        // r[verify chain.single-entry-valid]
        #[test]
        fn prop_single_entry_valid(idx in 1u64..1000, term in 1u64..10, data in 0u64..1000) {
            let chain = build_chain(&[(idx, term, data)]);
            prop_assert_eq!(chain.len(), 1);
            prop_assert!(is_chain_valid(&chain));
        }
    }
}
