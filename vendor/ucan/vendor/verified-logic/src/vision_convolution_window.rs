//! Exact finite convolution-window admission primitives.
use vstd::prelude::*;

use crate::vision_image_buffer::VisionImageBuffer;
use crate::vision_image_buffer::vision_image_buffer_valid;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionKernel { pub width: u8, pub height: u8, pub anchor_x: u8, pub anchor_y: u8 }

// r[depends domain-primitives.vision-convolution-window]
pub open spec fn vision_kernel_valid_spec(k: VisionKernel) -> bool { k.width > 0 && k.height > 0 && k.anchor_x < k.width && k.anchor_y < k.height }

// r[depends domain-primitives.vision-convolution-window]
pub open spec fn vision_window_inside_spec(img: VisionImageBuffer, k: VisionKernel, x: u8, y: u8) -> bool {
    img.width > 0 && img.height > 0 && img.channels > 0 && (img.width as nat) * (img.channels as nat) <= img.stride as nat && (img.stride as nat) * (img.height as nat) <= img.len as nat && vision_kernel_valid_spec(k)
    && k.anchor_x <= x && k.anchor_y <= y
    && (x as nat) + ((k.width - k.anchor_x) as nat) <= img.width as nat
    && (y as nat) + ((k.height - k.anchor_y) as nat) <= img.height as nat
}

// r[impl domain-primitives.vision-convolution-window]
pub fn vision_kernel_valid(k: VisionKernel) -> (result: bool)
    ensures result == vision_kernel_valid_spec(k)
{ k.width > 0 && k.height > 0 && k.anchor_x < k.width && k.anchor_y < k.height }

// r[impl domain-primitives.vision-convolution-window]
pub fn vision_window_inside(img: VisionImageBuffer, k: VisionKernel, x: u8, y: u8) -> (result: bool)
    ensures result == vision_window_inside_spec(img, k, x, y)
{ vision_image_buffer_valid(img) && vision_kernel_valid(k) && k.anchor_x <= x && k.anchor_y <= y && (x as u16) + ((k.width - k.anchor_x) as u16) <= img.width as u16 && (y as u16) + ((k.height - k.anchor_y) as u16) <= img.height as u16 }

// r[verify domain-primitives.vision-convolution-window]
pub proof fn vision_window_inside_has_valid_kernel(img: VisionImageBuffer, k: VisionKernel, x: u8, y: u8)
    requires vision_window_inside_spec(img, k, x, y)
    ensures vision_kernel_valid_spec(k)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision_image_buffer::VisionImageBuffer;
    #[test]
    fn window_rejects_borders() {
        let img = VisionImageBuffer {
            width: 5,
            height: 5,
            channels: 1,
            stride: 5,
            len: 25,
        };
        let k = VisionKernel {
            width: 3,
            height: 3,
            anchor_x: 1,
            anchor_y: 1,
        };
        assert!(vision_window_inside(img, k, 2, 2));
        assert!(!vision_window_inside(img, k, 0, 2));
    }
}
