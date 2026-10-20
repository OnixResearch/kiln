//! Exact bounded region-of-interest primitives.
use vstd::prelude::*;

use crate::vision_image_buffer::VisionImageBuffer;
use crate::vision_image_buffer::vision_image_buffer_valid;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionRoi { pub x: u8, pub y: u8, pub width: u8, pub height: u8 }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionRoiCoord { pub x: u8, pub y: u8 }

// r[depends domain-primitives.vision-roi]
pub open spec fn vision_roi_valid_spec(img: VisionImageBuffer, roi: VisionRoi) -> bool {
    img.width > 0 && img.height > 0 && img.channels > 0 && (img.width as nat) * (img.channels as nat) <= img.stride as nat && (img.stride as nat) * (img.height as nat) <= img.len as nat && roi.width > 0 && roi.height > 0
    && (roi.x as nat) + (roi.width as nat) <= img.width as nat
    && (roi.y as nat) + (roi.height as nat) <= img.height as nat
}

// r[depends domain-primitives.vision-roi]
pub open spec fn vision_roi_coord_valid_spec(roi: VisionRoi, p: VisionRoiCoord) -> bool { p.x < roi.width && p.y < roi.height }

// r[impl domain-primitives.vision-roi]
pub fn vision_roi_valid(img: VisionImageBuffer, roi: VisionRoi) -> (result: bool)
    ensures result == vision_roi_valid_spec(img, roi)
{ vision_image_buffer_valid(img) && roi.width > 0 && roi.height > 0 && (roi.x as u16) + (roi.width as u16) <= img.width as u16 && (roi.y as u16) + (roi.height as u16) <= img.height as u16 }

// r[impl domain-primitives.vision-roi]
pub fn vision_roi_map(roi: VisionRoi, p: VisionRoiCoord) -> (result: Option<(u16,u16)>)
    ensures result.is_some() == vision_roi_coord_valid_spec(roi, p)
{ if p.x < roi.width && p.y < roi.height { Some(((roi.x as u16) + (p.x as u16), (roi.y as u16) + (p.y as u16))) } else { None } }

// r[verify domain-primitives.vision-roi]
pub proof fn vision_roi_valid_has_nonzero_area(img: VisionImageBuffer, roi: VisionRoi)
    requires vision_roi_valid_spec(img, roi)
    ensures roi.width > 0 && roi.height > 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision_image_buffer::VisionImageBuffer;
    #[test]
    fn roi_checks_bounds() {
        let img = VisionImageBuffer {
            width: 10,
            height: 8,
            channels: 1,
            stride: 10,
            len: 80,
        };
        let roi = VisionRoi {
            x: 2,
            y: 1,
            width: 4,
            height: 3,
        };
        assert!(vision_roi_valid(img, roi));
        assert_eq!(vision_roi_map(roi, VisionRoiCoord { x: 1, y: 2 }), Some((3, 3)));
        assert!(!vision_roi_valid(img, VisionRoi { x: 9, ..roi }));
    }
}
