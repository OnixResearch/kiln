//! Exact connected-component label admission primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionComponentMerge { pub a:u16, pub b:u16, pub components:u16 }

// r[depends domain-primitives.vision-connected-components]
pub open spec fn vision_component_label_valid_spec(label:u16, components:u16)->bool{ components>0 && label<=components }
// r[depends domain-primitives.vision-connected-components]
pub open spec fn vision_component_merge_valid_spec(m:VisionComponentMerge)->bool{ m.components>0 && 1<=m.a && m.a<=m.components && 1<=m.b && m.b<=m.components }

// r[impl domain-primitives.vision-connected-components]
pub fn vision_component_label_valid(label:u16, components:u16)->(result:bool) ensures result==vision_component_label_valid_spec(label,components){ components>0 && label<=components }
// r[impl domain-primitives.vision-connected-components]
pub fn vision_component_merge_valid(m:VisionComponentMerge)->(result:bool) ensures result==vision_component_merge_valid_spec(m){ m.components>0 && 1<=m.a && m.a<=m.components && 1<=m.b && m.b<=m.components }

// r[verify domain-primitives.vision-connected-components]
pub proof fn vision_component_merge_labels_in_range(m:VisionComponentMerge) requires vision_component_merge_valid_spec(m) ensures vision_component_label_valid_spec(m.a,m.components) && vision_component_label_valid_spec(m.b,m.components) {}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_labels_and_merges_are_bounded() {
        assert!(vision_component_label_valid(0, 3));
        assert!(vision_component_merge_valid(VisionComponentMerge {
            a: 1,
            b: 3,
            components: 3
        }));
        assert!(!vision_component_merge_valid(VisionComponentMerge {
            a: 4,
            b: 1,
            components: 3
        }));
    }
}
