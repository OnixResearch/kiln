//! Exact rectangular grid indexing over bounded `u8` dimensions.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RectGrid {
    pub width: u8,
    pub height: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RectGridCoord {
    pub x: u8,
    pub y: u8,
}

// r[depends domain-primitives.rect-grid-index]
pub open spec fn rect_grid_valid_spec(width: u8, height: u8) -> bool {
    width > 0 && height > 0
}

// r[depends domain-primitives.rect-grid-index]
pub open spec fn rect_grid_accepted_spec(grid: RectGrid) -> bool {
    rect_grid_valid_spec(grid.width, grid.height)
}

// r[depends domain-primitives.rect-grid-index]
pub open spec fn rect_grid_coord_valid_spec(grid: RectGrid, coord: RectGridCoord) -> bool {
    rect_grid_accepted_spec(grid) && coord.x < grid.width && coord.y < grid.height
}

// r[depends domain-primitives.rect-grid-index]
pub open spec fn rect_grid_area_spec(grid: RectGrid) -> nat {
    (grid.width as nat) * (grid.height as nat)
}

// r[depends domain-primitives.rect-grid-index]
pub open spec fn rect_grid_flatten_spec(grid: RectGrid, coord: RectGridCoord) -> nat {
    (coord.y as nat) * (grid.width as nat) + (coord.x as nat)
}

// r[impl domain-primitives.rect-grid-index]
pub fn make_rect_grid(width: u8, height: u8) -> (result: Option<RectGrid>)
    ensures result.is_some() <==> rect_grid_valid_spec(width, height)
{
    if width > 0 && height > 0 {
        Some(RectGrid { width, height })
    } else {
        None
    }
}

// r[impl domain-primitives.rect-grid-index]
pub fn make_rect_grid_coord(grid: RectGrid, x: u8, y: u8) -> (result: Option<RectGridCoord>)
    requires rect_grid_accepted_spec(grid)
    ensures result.is_some() <==> (x < grid.width && y < grid.height),
            result.is_some() ==> rect_grid_coord_valid_spec(grid, result.unwrap())
{
    if x < grid.width && y < grid.height {
        Some(RectGridCoord { x, y })
    } else {
        None
    }
}

// r[impl domain-primitives.rect-grid-index]
pub fn rect_grid_area(grid: RectGrid) -> (result: u32)
    requires rect_grid_accepted_spec(grid)
    ensures result as nat == rect_grid_area_spec(grid)
{
    assert((grid.width as int) <= 255);
    assert((grid.height as int) <= 255);
    assert((grid.width as int) * (grid.height as int) <= u32::MAX as int) by(nonlinear_arith);
    (grid.width as u32) * (grid.height as u32)
}

// r[impl domain-primitives.rect-grid-index]
pub fn rect_grid_flatten(grid: RectGrid, coord: RectGridCoord) -> (result: u32)
    requires rect_grid_coord_valid_spec(grid, coord)
    ensures result as nat == rect_grid_flatten_spec(grid, coord)
{
    assert((coord.y as int) <= 255);
    assert((grid.width as int) <= 255);
    assert((coord.x as int) <= 255);
    assert((coord.y as int) * (grid.width as int) <= 65025) by(nonlinear_arith);
    assert((coord.y as int) * (grid.width as int) + (coord.x as int) <= u32::MAX as int) by(nonlinear_arith);
    let flat = (coord.y as u32) * (grid.width as u32) + (coord.x as u32);
    assert((coord.y as nat) < (grid.height as nat));
    assert((coord.x as nat) < (grid.width as nat));
    flat
}

// r[verify domain-primitives.rect-grid-index]
pub proof fn rect_grid_valid_coordinate_components(grid: RectGrid, coord: RectGridCoord)
    requires rect_grid_coord_valid_spec(grid, coord)
    ensures coord.x < grid.width, coord.y < grid.height
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_coordinate_flattens_inside_grid() {
        let grid = make_rect_grid(4, 3).unwrap();
        let coord = make_rect_grid_coord(grid, 2, 1).unwrap();
        assert_eq!(rect_grid_flatten(grid, coord), 6);
    }

    #[test]
    fn out_of_range_coordinate_is_rejected() {
        let grid = make_rect_grid(4, 3).unwrap();
        assert!(make_rect_grid_coord(grid, 4, 1).is_none());
    }
}
