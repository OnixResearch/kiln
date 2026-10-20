//! Exact bounded image-pyramid dimension primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionPyramidLevel { pub prev_width:u8, pub prev_height:u8, pub width:u8, pub height:u8 }

// r[depends domain-primitives.vision-image-pyramid]
pub open spec fn vision_pyramid_level_valid_spec(l:VisionPyramidLevel)->bool{ l.prev_width>0 && l.prev_height>0 && l.width>0 && l.height>0 && l.width<=l.prev_width && l.height<=l.prev_height }

// r[impl domain-primitives.vision-image-pyramid]
pub fn vision_pyramid_level_valid(l:VisionPyramidLevel)->(result:bool) ensures result==vision_pyramid_level_valid_spec(l){ l.prev_width>0 && l.prev_height>0 && l.width>0 && l.height>0 && l.width<=l.prev_width && l.height<=l.prev_height }

// r[verify domain-primitives.vision-image-pyramid]
pub proof fn vision_pyramid_level_monotone(l:VisionPyramidLevel) requires vision_pyramid_level_valid_spec(l) ensures l.width<=l.prev_width && l.height<=l.prev_height {}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pyramid_level_is_monotone() {
        assert!(vision_pyramid_level_valid(VisionPyramidLevel {
            prev_width: 8,
            prev_height: 8,
            width: 4,
            height: 4
        }));
        assert!(!vision_pyramid_level_valid(VisionPyramidLevel {
            prev_width: 8,
            prev_height: 8,
            width: 9,
            height: 4
        }));
    }
}
