//! Exact finite ML tensor shape primitives.
// r[impl domain-primitives.ml-non-claims]
// r[verify domain-primitives.ml-non-claims]
// This ML family is limited to exact finite metadata/admission primitives and does not
//! claim model quality, training correctness, fairness, stochastic convergence, or
//! floating-point accuracy.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlTensorShape {
    pub rank: u8,
    pub dim0: u16,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlTensorIndex {
    pub i0: u16,
}

// r[depends domain-primitives.ml-tensor-shape]
pub open spec fn ml_tensor_shape_valid_spec(shape: MlTensorShape) -> bool {
    shape.rank == 1 && 0 < shape.dim0
}

// r[depends domain-primitives.ml-tensor-shape]
pub open spec fn ml_tensor_element_count_spec(shape: MlTensorShape) -> nat {
    shape.dim0 as nat
}

// r[depends domain-primitives.ml-tensor-shape]
pub open spec fn ml_tensor_index_valid_spec(shape: MlTensorShape, index: MlTensorIndex) -> bool {
    ml_tensor_shape_valid_spec(shape) && index.i0 < shape.dim0
}

// r[depends domain-primitives.ml-tensor-shape]
pub open spec fn ml_tensor_flatten_spec(_shape: MlTensorShape, index: MlTensorIndex) -> nat {
    index.i0 as nat
}

// r[impl domain-primitives.ml-tensor-shape]
pub fn ml_make_tensor_shape(rank: u8, dim0: u16) -> (result: Option<MlTensorShape>)
    ensures result.is_some() <==> ml_tensor_shape_valid_spec(MlTensorShape { rank, dim0 }),
            result.is_some() ==> ml_tensor_shape_valid_spec(result.unwrap())
{
    let shape = MlTensorShape { rank, dim0 };
    if rank == 1 && 0 < dim0 { Some(shape) } else { None }
}

// r[impl domain-primitives.ml-tensor-shape]
pub fn ml_make_tensor_index(shape: MlTensorShape, i0: u16) -> (result: Option<MlTensorIndex>)
    requires ml_tensor_shape_valid_spec(shape)
    ensures result.is_some() <==> ml_tensor_index_valid_spec(shape, MlTensorIndex { i0 }),
            result.is_some() ==> ml_tensor_index_valid_spec(shape, result.unwrap())
{
    if i0 < shape.dim0 { Some(MlTensorIndex { i0 }) } else { None }
}

// r[impl domain-primitives.ml-tensor-shape]
pub fn ml_tensor_flatten(shape: MlTensorShape, index: MlTensorIndex) -> (result: u16)
    requires ml_tensor_index_valid_spec(shape, index)
    ensures result as nat == ml_tensor_flatten_spec(shape, index),
            (result as nat) < ml_tensor_element_count_spec(shape)
{
    index.i0
}

// r[verify domain-primitives.ml-tensor-shape]
pub proof fn ml_tensor_valid_flatten_in_bounds(shape: MlTensorShape, index: MlTensorIndex)
    requires ml_tensor_index_valid_spec(shape, index)
    ensures ml_tensor_flatten_spec(shape, index) < ml_tensor_element_count_spec(shape)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tensor_shape_accepts_and_flattens() {
        let s = ml_make_tensor_shape(1, 3).unwrap();
        let i = ml_make_tensor_index(s, 2).unwrap();
        assert_eq!(ml_tensor_flatten(s, i), 2);
        assert!(ml_make_tensor_index(s, 3).is_none());
    }
}
