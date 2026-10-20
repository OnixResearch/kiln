//! Exact finite machine-vision image buffer primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionImageBuffer { pub width: u8, pub height: u8, pub channels: u8, pub stride: u16, pub len: u32 }

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionPixelCoord { pub x: u8, pub y: u8, pub channel: u8 }

// r[depends domain-primitives.vision-image-buffer]
pub open spec fn vision_image_buffer_valid_spec(img: VisionImageBuffer) -> bool {
    img.width > 0 && img.height > 0 && img.channels > 0
    && (img.width as nat) * (img.channels as nat) <= img.stride as nat
    && (img.stride as nat) * (img.height as nat) <= img.len as nat
}

// r[depends domain-primitives.vision-image-buffer]
pub open spec fn vision_coord_in_bounds_spec(img: VisionImageBuffer, p: VisionPixelCoord) -> bool {
    vision_image_buffer_valid_spec(img) && p.x < img.width && p.y < img.height && p.channel < img.channels
}

// r[depends domain-primitives.vision-image-buffer]
pub open spec fn vision_pixel_offset_spec(img: VisionImageBuffer, p: VisionPixelCoord) -> nat {
    (p.y as nat) * (img.stride as nat) + (p.x as nat) * (img.channels as nat) + p.channel as nat
}

// r[impl domain-primitives.vision-image-buffer]
pub fn vision_image_buffer_valid(img: VisionImageBuffer) -> (result: bool)
    ensures result == vision_image_buffer_valid_spec(img)
{
    assert((img.width as int) * (img.channels as int) <= 255int * 255int) by(nonlinear_arith);
    assert((img.stride as int) * (img.height as int) <= 65535int * 255int) by(nonlinear_arith);
    img.width > 0 && img.height > 0 && img.channels > 0 && (img.width as u32) * (img.channels as u32) <= img.stride as u32 && (img.stride as u32) * (img.height as u32) <= img.len
}

// r[impl domain-primitives.vision-image-buffer]
pub fn vision_pixel_offset(img: VisionImageBuffer, p: VisionPixelCoord) -> (result: Option<u32>)
    ensures result.is_some() == vision_coord_in_bounds_spec(img, p)
{
    if vision_image_buffer_valid(img) && p.x < img.width && p.y < img.height && p.channel < img.channels {
        assert((p.y as int) * (img.stride as int) <= 255int * 65535int) by(nonlinear_arith);
        assert((p.x as int) * (img.channels as int) <= 255int * 255int) by(nonlinear_arith);
        let off = (p.y as u32) * (img.stride as u32) + (p.x as u32) * (img.channels as u32) + (p.channel as u32);
        Some(off)
    } else { None }
}

// r[verify domain-primitives.vision-image-buffer]
pub proof fn vision_valid_buffer_has_positive_channels(img: VisionImageBuffer)
    requires vision_image_buffer_valid_spec(img)
    ensures img.channels > 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_buffer_checks_layout() {
        let img = VisionImageBuffer {
            width: 4,
            height: 3,
            channels: 3,
            stride: 12,
            len: 36,
        };
        assert!(vision_image_buffer_valid(img));
        assert_eq!(vision_pixel_offset(img, VisionPixelCoord { x: 1, y: 1, channel: 2 }), Some(17));
        assert!(!vision_image_buffer_valid(VisionImageBuffer { stride: 10, ..img }));
    }
}
