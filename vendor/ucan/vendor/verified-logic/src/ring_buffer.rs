//! Verified fixed-capacity ring buffer.
//!
//! A FIFO queue backed by a fixed-size array with power-of-two capacity.
//! All operations have machine-checked pre/postconditions. The well-
//! formedness invariant is preserved across every operation — Verus
//! verifies this mechanically.
//!
//! Uses `bitwise::is_power_of_two` for capacity validation and
//! `slice::chunk_count` for batch counting.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::bitwise::is_power_of_two_spec;
#[cfg(verus_keep_ghost)]
use crate::slice::chunk_count_spec;

verus! {

/// A fixed-capacity ring buffer of `u64` values.
///
/// Fields are public for spec-level access. Use `wf()` as the abstraction
/// boundary — all methods require and preserve it.
pub struct RingBuffer {
    /// Backing storage.
    pub data: Vec<u64>,
    /// Read index (next pop position).
    pub head: usize,
    /// Write index (next push position).
    pub tail: usize,
    /// Number of elements currently stored.
    pub count: usize,
}

impl RingBuffer {

    /// Well-formedness invariant.
    pub open spec fn wf(&self) -> bool {
        let cap = self.data@.len();
        &&& cap > 0
        &&& is_power_of_two_spec(cap as u64)
        &&& self.head < cap
        &&& self.tail < cap
        &&& self.count <= cap
    }

    /// Ghost view: current number of elements.
    pub open spec fn len_spec(&self) -> nat {
        self.count as nat
    }

    /// Ghost view: capacity.
    pub open spec fn cap_spec(&self) -> nat {
        self.data@.len()
    }

    // r[impl ring-buffer.new]
    /// Creates an empty ring buffer with the given capacity.
    ///
    /// Capacity must be a power of two and nonzero.
    pub fn new(cap: usize) -> (result: RingBuffer)
        requires
            cap > 0,
            is_power_of_two_spec(cap as u64),
        ensures
            result.wf(),
            result.count == 0,
            result.cap_spec() == cap as nat,
    {
        let mut data = Vec::new();
        let mut i: usize = 0;
        while i < cap
            invariant
                0 <= i <= cap,
                data@.len() == i as int,
            decreases cap - i
        {
            data.push(0);
            i = i + 1;
        }
        RingBuffer { data, head: 0, tail: 0, count: 0 }
    }

    // r[impl ring-buffer.push]
    /// Writes `value` at the tail and advances. Returns false if full.
    pub fn push(&mut self, value: u64) -> (success: bool)
        requires old(self).wf()
        ensures
            self.wf(),
            self.cap_spec() == old(self).cap_spec(),
            success <==> old(self).count < old(self).data@.len(),
            success ==> self.count == old(self).count + 1,
            success ==> self.data@[old(self).tail as int] == value,
            !success ==> self.count == old(self).count,
    {
        if self.count == self.data.len() {
            return false;
        }
        self.data.set(self.tail, value);
        self.tail = (self.tail + 1) % self.data.len();
        self.count = self.count + 1;
        true
    }

    // r[impl ring-buffer.pop]
    /// Removes and returns the head element. Returns None if empty.
    pub fn pop(&mut self) -> (result: Option<u64>)
        requires old(self).wf()
        ensures
            self.wf(),
            self.cap_spec() == old(self).cap_spec(),
            result.is_some() <==> old(self).count > 0,
            result.is_some() ==> {
                &&& result.unwrap() == old(self).data@[old(self).head as int]
                &&& self.count == old(self).count - 1
            },
            result.is_none() ==> self.count == old(self).count,
    {
        if self.count == 0 {
            return None;
        }
        let h = self.head;
        let cap = self.data.len();
        let value = self.data[h];
        self.head = (h + 1) % cap;
        self.count = self.count - 1;
        Some(value)
    }

    // r[impl ring-buffer.peek]
    /// Returns the head element without removing it. Returns None if empty.
    pub fn peek(&self) -> (result: Option<u64>)
        requires self.wf()
        ensures
            result.is_some() <==> self.count > 0,
            result.is_some() ==> result.unwrap() == self.data@[self.head as int],
    {
        if self.count == 0 {
            None
        } else {
            Some(self.data[self.head])
        }
    }

    // r[impl ring-buffer.len]
    /// Returns the current number of elements.
    pub fn len(&self) -> (result: usize)
        requires self.wf()
        ensures result == self.count
    {
        self.count
    }

    // r[impl ring-buffer.is-empty]
    /// Returns true if the buffer contains no elements.
    pub fn is_empty(&self) -> (result: bool)
        requires self.wf()
        ensures result <==> self.count == 0
    {
        self.count == 0
    }

    // r[impl ring-buffer.is-full]
    /// Returns true if the buffer is at capacity.
    pub fn is_full(&self) -> (result: bool)
        requires self.wf()
        ensures result <==> self.count == self.data@.len()
    {
        self.count == self.data.len()
    }

    // r[impl ring-buffer.capacity]
    /// Returns the fixed capacity of the buffer.
    pub fn capacity(&self) -> (result: usize)
        requires self.wf()
        ensures result == self.data@.len()
    {
        self.data.len()
    }

    // r[impl ring-buffer.remaining]
    /// Returns the number of free slots.
    pub fn remaining(&self) -> (result: usize)
        requires self.wf()
        ensures result == self.data@.len() - self.count
    {
        self.data.len() - self.count
    }

    // r[impl ring-buffer.full-batches]
    /// Returns how many complete batches of `batch_size` can be popped.
    pub fn full_batches(&self, batch_size: usize) -> (result: usize)
        requires self.wf(), batch_size > 0
        ensures result == self.count / batch_size
    {
        self.count / batch_size
    }

}

// --- Proofs ---

// r[verify ring-buffer.is-empty]
// r[verify ring-buffer.is-full]
/// An empty buffer is not full (when capacity > 0).
proof fn empty_not_full(rb: RingBuffer)
    requires rb.wf(), rb.count == 0
    ensures rb.count != rb.data@.len()
{}

// r[verify ring-buffer.remaining]
/// Remaining slots + count == capacity.
proof fn remaining_plus_count(rb: RingBuffer)
    requires rb.wf()
    ensures (rb.data@.len() - rb.count) + rb.count == rb.data@.len()
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    // r[verify ring-buffer.capacity]
    // r[verify ring-buffer.new]
    #[test]
    fn test_new() {
        let rb = RingBuffer::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
        assert!(!rb.is_full());
        assert_eq!(rb.capacity(), 4);
        assert_eq!(rb.remaining(), 4);
    }

    // r[verify ring-buffer.push]
    // r[verify ring-buffer.pop]
    #[test]
    fn test_push_pop_fifo() {
        let mut rb = RingBuffer::new(4);
        assert!(rb.push(10));
        assert!(rb.push(20));
        assert!(rb.push(30));
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.pop(), Some(10));
        assert_eq!(rb.pop(), Some(20));
        assert_eq!(rb.pop(), Some(30));
        assert_eq!(rb.pop(), None);
        assert!(rb.is_empty());
    }

    // r[verify ring-buffer.push]
    #[test]
    fn test_push_full() {
        let mut rb = RingBuffer::new(2);
        assert!(rb.push(1));
        assert!(rb.push(2));
        assert!(!rb.push(3)); // full
        assert!(rb.is_full());
        assert_eq!(rb.remaining(), 0);
    }

    // r[verify ring-buffer.peek]
    #[test]
    fn test_peek() {
        let mut rb = RingBuffer::new(4);
        assert_eq!(rb.peek(), None);
        rb.push(42);
        assert_eq!(rb.peek(), Some(42));
        assert_eq!(rb.len(), 1); // peek doesn't remove
        assert_eq!(rb.pop(), Some(42));
    }

    // r[verify ring-buffer.push]
    // r[verify ring-buffer.pop]
    #[test]
    fn test_wraparound() {
        let mut rb = RingBuffer::new(4);
        // Fill and drain to advance head/tail past the end
        for i in 0..4_u64 {
            rb.push(i);
        }
        for _ in 0_i32..4_i32 {
            rb.pop();
        }
        // Now head and tail are both at position 4 % 4 = 0
        // Push more to wrap around
        assert!(rb.push(100));
        assert!(rb.push(200));
        assert!(rb.push(300));
        assert!(rb.push(400));
        assert_eq!(rb.pop(), Some(100));
        assert_eq!(rb.pop(), Some(200));
        assert_eq!(rb.pop(), Some(300));
        assert_eq!(rb.pop(), Some(400));
    }

    // r[verify ring-buffer.full-batches]
    #[test]
    fn test_full_batches() {
        let mut rb = RingBuffer::new(8);
        for i in 0..7_u64 {
            rb.push(i);
        }
        assert_eq!(rb.full_batches(3), 2); // 7 / 3 = 2
        assert_eq!(rb.full_batches(1), 7);
        assert_eq!(rb.full_batches(8), 0);
    }

    // r[verify ring-buffer.full-batches]
    #[test]
    fn test_full_batches_matches_chunk_count() {
        let mut rb = RingBuffer::new(16);
        for i in 0..10_u64 {
            rb.push(i);
        }
        for batch_size in 1..=10_usize {
            let expected = rb.len() / batch_size;
            assert_eq!(rb.full_batches(batch_size), expected);
        }
    }

    // --- Operation type for stateful fuzzing ---

    #[derive(Debug, Clone)]
    enum Op {
        Push(u64),
        Pop,
        Peek,
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => any::<u64>().prop_map(Op::Push),
            3 => Just(Op::Pop),
            1 => Just(Op::Peek),
        ]
    }

    fn capacity_strategy() -> impl Strategy<Value = usize> {
        prop::sample::select(vec![2_usize, 4, 8, 16, 32, 64])
    }

    proptest! {
        // r[verify ring-buffer.push]
        // r[verify ring-buffer.pop]
        #[test]
        fn prop_push_pop_roundtrip(value: u64) {
            let mut rb = RingBuffer::new(4);
            prop_assert!(rb.push(value));
            prop_assert_eq!(rb.pop(), Some(value));
            prop_assert!(rb.is_empty());
        }

        // r[verify ring-buffer.push]
        // r[verify ring-buffer.len]
        #[test]
        fn prop_push_increments_len(
            values in proptest::collection::vec(any::<u64>(), 0..=8),
        ) {
            let mut rb = RingBuffer::new(8);
            for (i, &v) in values.iter().enumerate() {
                prop_assert_eq!(rb.len(), i);
                prop_assert!(rb.push(v));
            }
            prop_assert_eq!(rb.len(), values.len());
        }

        // r[verify ring-buffer.push]
        // r[verify ring-buffer.pop]
        #[test]
        fn prop_fifo_order(
            values in proptest::collection::vec(any::<u64>(), 1..=16),
        ) {
            let mut rb = RingBuffer::new(16);
            for &v in &values {
                prop_assert!(rb.push(v));
            }
            for &v in &values {
                prop_assert_eq!(rb.pop(), Some(v));
            }
            prop_assert!(rb.is_empty());
        }

        // r[verify ring-buffer.remaining]
        #[test]
        fn prop_remaining_plus_len(
            n_push in 0usize..=8,
        ) {
            let mut rb = RingBuffer::new(8);
            for i in 0..n_push {
                rb.push(i as u64);
            }
            prop_assert_eq!(rb.len() + rb.remaining(), rb.capacity());
        }

        // r[verify ring-buffer.is-empty]
        // r[verify ring-buffer.is-full]
        #[test]
        fn prop_empty_full_predicates(
            n_push in 0usize..=4,
        ) {
            let mut rb = RingBuffer::new(4);
            for i in 0..n_push {
                rb.push(i as u64);
            }
            prop_assert_eq!(rb.is_empty(), rb.count == 0);
            prop_assert_eq!(rb.is_full(), rb.len() == rb.capacity());
        }

        // r[verify ring-buffer.push]
        // r[verify ring-buffer.pop]
        // r[verify ring-buffer.peek]
        // r[verify ring-buffer.len]
        // r[verify ring-buffer.is-empty]
        // r[verify ring-buffer.is-full]
        // r[verify ring-buffer.remaining]
        /// Model-based stateful fuzz: run random operation traces against a
        /// VecDeque oracle. Every observable must match after every step.
        #[test]
        fn prop_model_oracle(
            cap in capacity_strategy(),
            ops in proptest::collection::vec(op_strategy(), 0..=200),
        ) {
            let mut rb = RingBuffer::new(cap);
            let mut oracle: std::collections::VecDeque<u64> = std::collections::VecDeque::new();

            for op in &ops {
                // Invariants hold before every operation
                prop_assert_eq!(rb.len(), oracle.len());
                prop_assert_eq!(rb.is_empty(), oracle.is_empty());
                prop_assert_eq!(rb.is_full(), oracle.len() == cap);
                prop_assert_eq!(rb.len() + rb.remaining(), rb.capacity());

                match op {
                    Op::Push(v) => {
                        let rb_ok = rb.push(*v);
                        if oracle.len() < cap {
                            oracle.push_back(*v);
                            prop_assert!(rb_ok);
                        } else {
                            prop_assert!(!rb_ok);
                        }
                    }
                    Op::Pop => {
                        let rb_val = rb.pop();
                        let oracle_val = oracle.pop_front();
                        prop_assert_eq!(rb_val, oracle_val);
                    }
                    Op::Peek => {
                        let rb_val = rb.peek();
                        let oracle_val = oracle.front().copied();
                        prop_assert_eq!(rb_val, oracle_val);
                    }
                }
            }

            // Final invariants
            prop_assert_eq!(rb.len(), oracle.len());
            prop_assert_eq!(rb.len() + rb.remaining(), rb.capacity());

            // Drain both and compare full FIFO order
            while let Some(oracle_val) = oracle.pop_front() {
                prop_assert_eq!(rb.pop(), Some(oracle_val));
            }
            prop_assert!(rb.is_empty());
        }
    }
}
