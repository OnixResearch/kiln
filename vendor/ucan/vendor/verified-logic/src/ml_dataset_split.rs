//! Deterministic finite ML dataset split primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlDatasetSplit { pub train: u16, pub validation: u16, pub test: u16 }

// r[depends domain-primitives.ml-dataset-split]
pub open spec fn ml_split_sum_spec(split: MlDatasetSplit) -> nat {
    (split.train as nat) + (split.validation as nat) + (split.test as nat)
}

// r[depends domain-primitives.ml-dataset-split]
pub open spec fn ml_dataset_split_valid_spec(total: u16, split: MlDatasetSplit) -> bool {
    ml_split_sum_spec(split) <= total as nat
}

// r[depends domain-primitives.ml-dataset-split]
pub open spec fn ml_split_partition_spec(split: MlDatasetSplit, index: u16) -> u8 {
    if (index as nat) < split.train as nat { 0 }
    else if (index as nat) < (split.train as nat) + (split.validation as nat) { 1 }
    else if (index as nat) < ml_split_sum_spec(split) { 2 }
    else { 3 }
}

// r[impl domain-primitives.ml-dataset-split]
pub fn ml_make_dataset_split(total: u16, train: u16, validation: u16, test: u16) -> (result: Option<MlDatasetSplit>)
    ensures result.is_some() <==> ml_dataset_split_valid_spec(total, MlDatasetSplit { train, validation, test }),
            result.is_some() ==> ml_dataset_split_valid_spec(total, result.unwrap())
{
    let sum = (train as u32) + (validation as u32) + (test as u32);
    if sum <= total as u32 { Some(MlDatasetSplit { train, validation, test }) } else { None }
}

// r[impl domain-primitives.ml-dataset-split]
pub fn ml_dataset_split_partition(split: MlDatasetSplit, index: u16) -> (result: u8)
    ensures result == ml_split_partition_spec(split, index)
{
    if index < split.train { 0 }
    else if (index as u32) < (split.train as u32) + (split.validation as u32) { 1 }
    else if (index as u32) < (split.train as u32) + (split.validation as u32) + (split.test as u32) { 2 }
    else { 3 }
}

// r[verify domain-primitives.ml-dataset-split]
pub proof fn ml_dataset_split_partitions_ordered(split: MlDatasetSplit, i: u16)
    ensures ml_split_partition_spec(split, i) <= 3
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_rejects_overflow_and_partitions() {
        let s = ml_make_dataset_split(10, 6, 2, 1).unwrap();
        assert_eq!(ml_dataset_split_partition(s, 5), 0);
        assert_eq!(ml_dataset_split_partition(s, 7), 1);
        assert_eq!(ml_dataset_split_partition(s, 8), 2);
        assert_eq!(ml_dataset_split_partition(s, 9), 3);
        assert!(ml_make_dataset_split(5, 3, 3, 0).is_none());
    }
}
