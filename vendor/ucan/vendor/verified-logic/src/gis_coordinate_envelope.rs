//! GIS coordinate envelopes over exact scaled integer coordinates.

use vstd::prelude::*;

use crate::bounded_interval::BoundedInterval;
#[cfg(verus_keep_ghost)]
use crate::bounded_interval::bounded_interval_accepted_spec;
#[cfg(verus_keep_ghost)]
use crate::bounded_interval::bounded_interval_contains_spec;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GisCoordinateEnvelope {
    pub x_bounds: BoundedInterval,
    pub y_bounds: BoundedInterval,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GisCoordinate {
    pub x: i64,
    pub y: i64,
}

// r[depends domain-primitives.gis-coordinate-envelope]
pub open spec fn gis_coordinate_envelope_valid_spec(env: GisCoordinateEnvelope) -> bool {
    bounded_interval_accepted_spec(env.x_bounds) && bounded_interval_accepted_spec(env.y_bounds)
}

// r[depends domain-primitives.gis-coordinate-envelope]
pub open spec fn gis_coordinate_valid_spec(env: GisCoordinateEnvelope, coord: GisCoordinate) -> bool {
    gis_coordinate_envelope_valid_spec(env)
        && bounded_interval_contains_spec(env.x_bounds, coord.x)
        && bounded_interval_contains_spec(env.y_bounds, coord.y)
}

// r[impl domain-primitives.gis-coordinate-envelope]
pub fn make_gis_coordinate_envelope(x_bounds: BoundedInterval, y_bounds: BoundedInterval) -> (result: Option<GisCoordinateEnvelope>)
    ensures result.is_some() <==> (bounded_interval_accepted_spec(x_bounds) && bounded_interval_accepted_spec(y_bounds)),
            result.is_some() ==> gis_coordinate_envelope_valid_spec(result.unwrap())
{
    if x_bounds.lo <= x_bounds.hi && y_bounds.lo <= y_bounds.hi {
        Some(GisCoordinateEnvelope { x_bounds, y_bounds })
    } else {
        None
    }
}

// r[impl domain-primitives.gis-coordinate-envelope]
pub fn make_gis_coordinate(env: GisCoordinateEnvelope, x: i64, y: i64) -> (result: Option<GisCoordinate>)
    requires gis_coordinate_envelope_valid_spec(env)
    ensures result.is_some() <==> (bounded_interval_contains_spec(env.x_bounds, x) && bounded_interval_contains_spec(env.y_bounds, y)),
            result.is_some() ==> gis_coordinate_valid_spec(env, result.unwrap())
{
    if env.x_bounds.lo <= x && x <= env.x_bounds.hi && env.y_bounds.lo <= y && y <= env.y_bounds.hi {
        Some(GisCoordinate { x, y })
    } else {
        None
    }
}

// r[verify domain-primitives.gis-coordinate-envelope]
pub proof fn gis_coordinate_components_in_bounds(env: GisCoordinateEnvelope, coord: GisCoordinate)
    requires gis_coordinate_valid_spec(env, coord)
    ensures bounded_interval_contains_spec(env.x_bounds, coord.x),
            bounded_interval_contains_spec(env.y_bounds, coord.y)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_interval::make_bounded_interval;

    #[test]
    fn valid_coordinate_is_admitted() {
        let x = make_bounded_interval(-180, 180).unwrap();
        let y = make_bounded_interval(-90, 90).unwrap();
        let env = make_gis_coordinate_envelope(x, y).unwrap();
        assert_eq!(make_gis_coordinate(env, 10, -20).unwrap(), GisCoordinate { x: 10, y: -20 });
    }

    #[test]
    fn out_of_range_coordinate_is_rejected() {
        let x = make_bounded_interval(-180, 180).unwrap();
        let y = make_bounded_interval(-90, 90).unwrap();
        let env = make_gis_coordinate_envelope(x, y).unwrap();
        assert!(make_gis_coordinate(env, 181, 0).is_none());
        assert!(make_gis_coordinate(env, 0, -91).is_none());
    }
}
