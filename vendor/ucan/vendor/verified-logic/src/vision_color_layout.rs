//! Exact color-channel layout admission primitives.
use vstd::prelude::*;

verus! {

pub const VISION_CH_R:u8=1; pub const VISION_CH_G:u8=2; pub const VISION_CH_B:u8=3; pub const VISION_CH_A:u8=4;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionColorLayout { pub c0:u8, pub c1:u8, pub c2:u8, pub c3:u8, pub channels:u8, pub pixel_stride:u8 }

// r[depends domain-primitives.vision-color-layout]
pub open spec fn vision_channel_known_spec(c:u8)->bool{ c==VISION_CH_R || c==VISION_CH_G || c==VISION_CH_B || c==VISION_CH_A }
// r[depends domain-primitives.vision-color-layout]
pub open spec fn vision_color_layout_valid_spec(l:VisionColorLayout)->bool{ 1<=l.channels && l.channels<=4 && l.pixel_stride>=l.channels && vision_channel_known_spec(l.c0) && (l.channels<2 || (vision_channel_known_spec(l.c1)&&l.c1!=l.c0)) && (l.channels<3 || (vision_channel_known_spec(l.c2)&&l.c2!=l.c0&&l.c2!=l.c1)) && (l.channels<4 || (vision_channel_known_spec(l.c3)&&l.c3!=l.c0&&l.c3!=l.c1&&l.c3!=l.c2)) }

// r[impl domain-primitives.vision-color-layout]
pub fn vision_channel_known(c:u8)->(result:bool) ensures result==vision_channel_known_spec(c){ c==VISION_CH_R || c==VISION_CH_G || c==VISION_CH_B || c==VISION_CH_A }
// r[impl domain-primitives.vision-color-layout]
pub fn vision_color_layout_valid(l:VisionColorLayout)->(result:bool) ensures result==vision_color_layout_valid_spec(l){ 1<=l.channels && l.channels<=4 && l.pixel_stride>=l.channels && vision_channel_known(l.c0) && (l.channels<2 || (vision_channel_known(l.c1)&&l.c1!=l.c0)) && (l.channels<3 || (vision_channel_known(l.c2)&&l.c2!=l.c0&&l.c2!=l.c1)) && (l.channels<4 || (vision_channel_known(l.c3)&&l.c3!=l.c0&&l.c3!=l.c1&&l.c3!=l.c2)) }

// r[verify domain-primitives.vision-color-layout]
pub proof fn vision_color_layout_valid_bounds(l:VisionColorLayout) requires vision_color_layout_valid_spec(l) ensures 1<=l.channels && l.channels<=4 && l.pixel_stride>=l.channels {}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_layout_checks_duplicates() {
        let l = VisionColorLayout {
            c0: VISION_CH_R,
            c1: VISION_CH_G,
            c2: VISION_CH_B,
            c3: VISION_CH_A,
            channels: 3,
            pixel_stride: 3,
        };
        assert!(vision_color_layout_valid(l));
        assert!(!vision_color_layout_valid(VisionColorLayout { c1: VISION_CH_R, ..l }));
    }
}
