//! GIS tile index primitives over finite quadtree coordinates.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GisTileIndex {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

// r[depends domain-primitives.gis-tile-index]
pub open spec fn gis_tile_zoom_span_spec(zoom: u8) -> nat {
    if zoom == 0 { 1nat }
    else if zoom == 1 { 2nat }
    else if zoom == 2 { 4nat }
    else if zoom == 3 { 8nat }
    else if zoom == 4 { 16nat }
    else if zoom == 5 { 32nat }
    else if zoom == 6 { 64nat }
    else { 128nat }
}

// r[depends domain-primitives.gis-tile-index]
pub open spec fn gis_tile_valid_spec(max_zoom: u8, tile: GisTileIndex) -> bool {
    tile.zoom <= max_zoom && max_zoom < 8 && (tile.x as nat) < gis_tile_zoom_span_spec(tile.zoom) && (tile.y as nat) < gis_tile_zoom_span_spec(tile.zoom)
}

// r[impl domain-primitives.gis-tile-index]
pub fn gis_tile_zoom_span(zoom: u8) -> (result: u32)
    requires zoom < 8
    ensures result as nat == gis_tile_zoom_span_spec(zoom)
{
    if zoom == 0 { 1 }
    else if zoom == 1 { 2 }
    else if zoom == 2 { 4 }
    else if zoom == 3 { 8 }
    else if zoom == 4 { 16 }
    else if zoom == 5 { 32 }
    else if zoom == 6 { 64 }
    else { 128 }
}

// r[impl domain-primitives.gis-tile-index]
pub fn make_gis_tile(max_zoom: u8, zoom: u8, x: u32, y: u32) -> (result: Option<GisTileIndex>)
    ensures result.is_some() ==> gis_tile_valid_spec(max_zoom, result.unwrap())
{
    if max_zoom < 8 && zoom <= max_zoom {
        let span = gis_tile_zoom_span(zoom);
        if x < span && y < span {
            Some(GisTileIndex { zoom, x, y })
        } else {
            None
        }
    } else {
        None
    }
}

// r[impl domain-primitives.gis-tile-index]
pub fn gis_tile_parent(max_zoom: u8, tile: GisTileIndex) -> (result: Option<GisTileIndex>)
    requires gis_tile_valid_spec(max_zoom, tile)
    ensures result.is_some() ==> gis_tile_valid_spec(max_zoom, result.unwrap())
{
    if tile.zoom > 0 {
        Some(GisTileIndex { zoom: tile.zoom - 1, x: tile.x / 2, y: tile.y / 2 })
    } else {
        None
    }
}

// r[depends domain-primitives.gis-tile-index]
pub open spec fn gis_tile_parent_refines_spec(child: GisTileIndex, parent: GisTileIndex) -> bool {
    child.zoom > 0 && parent.zoom + 1 == child.zoom && parent.x == child.x / 2 && parent.y == child.y / 2
}

// r[verify domain-primitives.gis-tile-index]
pub proof fn gis_tile_valid_has_bounded_zoom(max_zoom: u8, tile: GisTileIndex)
    requires gis_tile_valid_spec(max_zoom, tile)
    ensures tile.zoom <= max_zoom
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_tile_refines_to_parent() {
        let t = make_gis_tile(7, 3, 6, 4).unwrap();
        let p = gis_tile_parent(7, t).unwrap();
        assert_eq!(p, GisTileIndex { zoom: 2, x: 3, y: 2 });
    }

    #[test]
    fn tile_coordinates_beyond_zoom_range_are_rejected() {
        assert!(make_gis_tile(7, 3, 8, 0).is_none());
        assert!(make_gis_tile(8, 8, 0, 0).is_none());
    }
}
