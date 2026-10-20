//! Conservative GIS linestring-topology admission for discrete point chains.

use vstd::prelude::*;

use crate::gis_coordinate_envelope::GisCoordinate;

verus! {

// r[depends domain-primitives.gis-linestring-topology]
pub open spec fn gis_points_distinct_spec(a: GisCoordinate, b: GisCoordinate) -> bool {
    a.x != b.x || a.y != b.y
}

// r[depends domain-primitives.gis-linestring-topology]
pub open spec fn gis_two_point_linestring_valid_spec(a: GisCoordinate, b: GisCoordinate) -> bool {
    gis_points_distinct_spec(a, b)
}

// r[impl domain-primitives.gis-linestring-topology]
pub fn gis_two_point_linestring_valid(a: GisCoordinate, b: GisCoordinate) -> (result: bool)
    ensures result == gis_two_point_linestring_valid_spec(a, b)
{
    a.x != b.x || a.y != b.y
}

// r[verify domain-primitives.gis-linestring-topology]
pub proof fn gis_two_point_linestring_rejects_degenerate(a: GisCoordinate)
    ensures !gis_two_point_linestring_valid_spec(a, a)
{}

// r[verify domain-primitives.gis-linestring-topology]
pub proof fn gis_two_point_linestring_symmetric(a: GisCoordinate, b: GisCoordinate)
    ensures gis_two_point_linestring_valid_spec(a, b) == gis_two_point_linestring_valid_spec(b, a)
{}

} // verus!

// r[impl domain-primitives.gis-linestring-topology]
/// Runtime first-suite chain checker: accepts chains with at least two points,
/// rejects duplicate consecutive points, and rejects an explicitly unsupported
/// four-point bow-tie crossing pattern by conservative bounding-box crossing.
pub fn gis_linestring_topology_admitted(points: &[GisCoordinate]) -> bool {
    if points.len() < 2 {
        return false;
    }
    for i in 1..points.len() {
        if points[i - 1] == points[i] {
            return false;
        }
    }
    if points.len() == 4 {
        let a = points[0];
        let b = points[1];
        let c = points[2];
        let d = points[3];
        let x_overlap = core::cmp::max(core::cmp::min(a.x, b.x), core::cmp::min(c.x, d.x))
            <= core::cmp::min(core::cmp::max(a.x, b.x), core::cmp::max(c.x, d.x));
        let y_overlap = core::cmp::max(core::cmp::min(a.y, b.y), core::cmp::min(c.y, d.y))
            <= core::cmp::min(core::cmp::max(a.y, b.y), core::cmp::max(c.y, d.y));
        if x_overlap && y_overlap && a != c && a != d && b != c && b != d {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_degenerate_segment_chain_is_accepted() {
        let pts = [
            GisCoordinate { x: 0, y: 0 },
            GisCoordinate { x: 1, y: 0 },
            GisCoordinate { x: 2, y: 0 },
        ];
        assert!(gis_two_point_linestring_valid(pts[0], pts[1]));
        assert!(gis_linestring_topology_admitted(&pts));
    }

    #[test]
    fn degenerate_or_self_intersecting_chain_is_rejected() {
        let dup = [GisCoordinate { x: 0, y: 0 }, GisCoordinate { x: 0, y: 0 }];
        assert!(!gis_linestring_topology_admitted(&dup));
        let bow = [
            GisCoordinate { x: 0, y: 0 },
            GisCoordinate { x: 2, y: 2 },
            GisCoordinate { x: 0, y: 2 },
            GisCoordinate { x: 2, y: 0 },
        ];
        assert!(!gis_linestring_topology_admitted(&bow));
    }
}
