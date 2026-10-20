//! GIS bounding boxes over exact coordinate envelopes.

use vstd::prelude::*;

use crate::bounded_interval::BoundedInterval;
#[cfg(verus_keep_ghost)]
use crate::bounded_interval::bounded_interval_accepted_spec;
#[cfg(verus_keep_ghost)]
use crate::bounded_interval::bounded_interval_contains_spec;
#[cfg(verus_keep_ghost)]
use crate::bounded_interval::bounded_interval_overlaps_spec;
use crate::gis_coordinate_envelope::GisCoordinate;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GisBoundingBox {
    pub x_bounds: BoundedInterval,
    pub y_bounds: BoundedInterval,
}

// r[depends domain-primitives.gis-bounding-box]
pub open spec fn gis_bounding_box_valid_spec(b: GisBoundingBox) -> bool {
    bounded_interval_accepted_spec(b.x_bounds) && bounded_interval_accepted_spec(b.y_bounds)
}

// r[depends domain-primitives.gis-bounding-box]
pub open spec fn gis_bounding_box_contains_spec(b: GisBoundingBox, p: GisCoordinate) -> bool {
    gis_bounding_box_valid_spec(b)
        && bounded_interval_contains_spec(b.x_bounds, p.x)
        && bounded_interval_contains_spec(b.y_bounds, p.y)
}

// r[depends domain-primitives.gis-bounding-box]
pub open spec fn gis_bounding_box_intersects_spec(a: GisBoundingBox, b: GisBoundingBox) -> bool {
    gis_bounding_box_valid_spec(a) && gis_bounding_box_valid_spec(b)
        && bounded_interval_overlaps_spec(a.x_bounds, b.x_bounds)
        && bounded_interval_overlaps_spec(a.y_bounds, b.y_bounds)
}

// r[impl domain-primitives.gis-bounding-box]
pub fn make_gis_bounding_box(min_x: i64, min_y: i64, max_x: i64, max_y: i64) -> (result: Option<GisBoundingBox>)
    ensures result.is_some() <==> (min_x <= max_x && min_y <= max_y),
            result.is_some() ==> gis_bounding_box_valid_spec(result.unwrap())
{
    if min_x <= max_x && min_y <= max_y {
        Some(GisBoundingBox {
            x_bounds: BoundedInterval { lo: min_x, hi: max_x },
            y_bounds: BoundedInterval { lo: min_y, hi: max_y },
        })
    } else {
        None
    }
}

// r[impl domain-primitives.gis-bounding-box]
pub fn gis_bounding_box_contains(b: GisBoundingBox, p: GisCoordinate) -> (result: bool)
    requires gis_bounding_box_valid_spec(b)
    ensures result == gis_bounding_box_contains_spec(b, p)
{
    b.x_bounds.lo <= p.x && p.x <= b.x_bounds.hi && b.y_bounds.lo <= p.y && p.y <= b.y_bounds.hi
}

// r[impl domain-primitives.gis-bounding-box]
pub fn gis_bounding_box_intersects(a: GisBoundingBox, b: GisBoundingBox) -> (result: bool)
    requires gis_bounding_box_valid_spec(a), gis_bounding_box_valid_spec(b)
    ensures result == gis_bounding_box_intersects_spec(a, b)
{
    a.x_bounds.lo <= b.x_bounds.hi && b.x_bounds.lo <= a.x_bounds.hi
        && a.y_bounds.lo <= b.y_bounds.hi && b.y_bounds.lo <= a.y_bounds.hi
}

// r[verify domain-primitives.gis-bounding-box]
pub proof fn gis_bounding_box_intersection_symmetric(a: GisBoundingBox, b: GisBoundingBox)
    ensures gis_bounding_box_intersects_spec(a, b) == gis_bounding_box_intersects_spec(b, a)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_box_contains_interior_point() {
        let b = make_gis_bounding_box(0, 0, 10, 10).unwrap();
        assert!(gis_bounding_box_contains(b, GisCoordinate { x: 5, y: 5 }));
    }

    #[test]
    fn inverted_box_is_rejected() {
        assert!(make_gis_bounding_box(5, 0, 4, 10).is_none());
    }
}
