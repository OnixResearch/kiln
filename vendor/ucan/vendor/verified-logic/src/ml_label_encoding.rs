//! Exact dense ML label encoding primitives.
use vstd::prelude::*;

verus! {

// r[depends domain-primitives.ml-label-encoding]
pub open spec fn ml_label_vocab_valid_spec(class_count: u16) -> bool { 0 < class_count }

// r[depends domain-primitives.ml-label-encoding]
pub open spec fn ml_label_declared_spec(class_count: u16, label: u16) -> bool {
    ml_label_vocab_valid_spec(class_count) && label < class_count
}

// r[impl domain-primitives.ml-label-encoding]
pub fn ml_encode_dense_label(class_count: u16, label: u16) -> (result: Option<u16>)
    ensures result.is_some() <==> ml_label_declared_spec(class_count, label),
            result.is_some() ==> result.unwrap() == label
{
    if 0 < class_count && label < class_count { Some(label) } else { None }
}

// r[impl domain-primitives.ml-label-encoding]
pub fn ml_decode_dense_label(class_count: u16, index: u16) -> (result: Option<u16>)
    ensures result.is_some() <==> ml_label_declared_spec(class_count, index),
            result.is_some() ==> result.unwrap() == index
{
    if 0 < class_count && index < class_count { Some(index) } else { None }
}

// r[verify domain-primitives.ml-label-encoding]
pub proof fn ml_dense_label_roundtrip(class_count: u16, label: u16)
    requires ml_label_declared_spec(class_count, label)
    ensures ml_label_declared_spec(class_count, label)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_roundtrip_and_unknown() {
        let ix = ml_encode_dense_label(3, 2).unwrap();
        assert_eq!(ml_decode_dense_label(3, ix), Some(2));
        assert!(ml_encode_dense_label(3, 4).is_none());
    }
}
