//! Bounded contiguous ML batch range primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlBatchRange { pub start: u16, pub end: u16 }

// r[depends domain-primitives.ml-batch-iterator]
pub open spec fn ml_batch_request_valid_spec(total: u16, batch_size: u16, batch_index: u16) -> bool {
    0 < batch_size && (batch_index as nat) * (batch_size as nat) < total as nat
}

// r[depends domain-primitives.ml-batch-iterator]
pub open spec fn ml_batch_range_valid_spec(total: u16, range: MlBatchRange) -> bool {
    range.start < range.end && range.end <= total
}

// r[impl domain-primitives.ml-batch-iterator]
#[allow(clippy::cast_possible_truncation)]
pub fn ml_batch_range(total: u16, batch_size: u16, batch_index: u16) -> (result: Option<MlBatchRange>)
    ensures result.is_some() ==> ml_batch_range_valid_spec(total, result.unwrap()),
            result.is_none() ==> !ml_batch_request_valid_spec(total, batch_size, batch_index)
{
    if batch_size == 0 { return None; }
    assert((batch_index as u32) * (batch_size as u32) <= 4294836225u32) by(nonlinear_arith);
    let start32 = (batch_index as u32) * (batch_size as u32);
    if start32 >= total as u32 { return None; }
    let mut end32 = start32 + (batch_size as u32);
    if end32 > total as u32 { end32 = total as u32; }
    assert(start32 < end32);
    assert(end32 <= total as u32);
    Some(MlBatchRange { start: start32 as u16, end: end32 as u16 })
}

// r[impl domain-primitives.ml-batch-iterator]
pub fn ml_index_in_batch(range: MlBatchRange, index: u16) -> (result: bool)
    requires range.start <= range.end
    ensures result == (range.start <= index && index < range.end)
{
    range.start <= index && index < range.end
}

// r[verify domain-primitives.ml-batch-iterator]
pub proof fn ml_batch_range_nonempty(total: u16, range: MlBatchRange)
    requires ml_batch_range_valid_spec(total, range)
    ensures range.start < range.end
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_ranges_are_contiguous() {
        assert_eq!(ml_batch_range(10, 4, 0).unwrap(), MlBatchRange { start: 0, end: 4 });
        assert_eq!(ml_batch_range(10, 4, 2).unwrap(), MlBatchRange { start: 8, end: 10 });
        assert!(ml_batch_range(10, 0, 0).is_none());
    }
}
