//! Exact deterministic detection/NMS admission primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionDetection { pub x0:u16, pub y0:u16, pub x1:u16, pub y1:u16, pub score:u16, pub class_id:u16 }

// r[depends domain-primitives.vision-detection-nms]
pub open spec fn vision_detection_valid_spec(d:VisionDetection, max_x:u16, max_y:u16, max_class:u16, min_score:u16)->bool{ d.x0<d.x1 && d.y0<d.y1 && d.x1<=max_x && d.y1<=max_y && d.class_id<max_class && d.score>=min_score }
// r[depends domain-primitives.vision-detection-nms]
pub open spec fn vision_suppresses_spec(a:VisionDetection,b:VisionDetection)->bool{ a.class_id==b.class_id && a.score>=b.score && a.x0<=b.x0 && b.x1<=a.x1 && a.y0<=b.y0 && b.y1<=a.y1 }

// r[impl domain-primitives.vision-detection-nms]
pub fn vision_detection_valid(d:VisionDetection, max_x:u16, max_y:u16, max_class:u16, min_score:u16)->(result:bool) ensures result==vision_detection_valid_spec(d,max_x,max_y,max_class,min_score){ d.x0<d.x1 && d.y0<d.y1 && d.x1<=max_x && d.y1<=max_y && d.class_id<max_class && d.score>=min_score }
// r[impl domain-primitives.vision-detection-nms]
pub fn vision_suppresses(a:VisionDetection,b:VisionDetection)->(result:bool) ensures result==vision_suppresses_spec(a,b){ a.class_id==b.class_id && a.score>=b.score && a.x0<=b.x0 && b.x1<=a.x1 && a.y0<=b.y0 && b.y1<=a.y1 }

// r[verify domain-primitives.vision-detection-nms]
pub proof fn vision_suppression_preserves_class(a:VisionDetection,b:VisionDetection) requires vision_suppresses_spec(a,b) ensures a.class_id==b.class_id {}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detection_admission_and_suppression() {
        let a = VisionDetection {
            x0: 0,
            y0: 0,
            x1: 10,
            y1: 10,
            score: 9,
            class_id: 1,
        };
        let b = VisionDetection {
            x0: 2,
            y0: 2,
            x1: 8,
            y1: 8,
            score: 7,
            class_id: 1,
        };
        assert!(vision_detection_valid(a, 20, 20, 2, 5));
        assert!(vision_suppresses(a, b));
        assert!(!vision_detection_valid(VisionDetection { x1: 0, ..a }, 20, 20, 2, 5));
    }
}
