//! Exact finite grid-cell/index envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomGridCell { pub x: u8, pub y: u8, pub width: u8, pub height: u8, pub cell_size: u8 }
// r[depends domain-primitives.geometry-foundation.grid-cell]
pub open spec fn geom_grid_cell_valid_spec(g: GeomGridCell) -> bool { g.cell_size > 0 && g.width > 0 && g.height > 0 && g.x < g.width && g.y < g.height }
// r[impl domain-primitives.geometry-foundation.grid-cell]
pub fn geom_grid_cell_valid(g: GeomGridCell) -> (result: bool) ensures result == geom_grid_cell_valid_spec(g) { g.cell_size > 0 && g.width > 0 && g.height > 0 && g.x < g.width && g.y < g.height }
// r[verify domain-primitives.geometry-foundation.grid-cell]
pub proof fn valid_grid_cell_fits_extent(g: GeomGridCell) requires geom_grid_cell_valid_spec(g) ensures g.x < g.width, g.y < g.height {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_cell_fits_extent() {
        assert!(geom_grid_cell_valid(GeomGridCell {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
            cell_size: 1
        }));
        assert!(!geom_grid_cell_valid(GeomGridCell {
            x: 2,
            y: 1,
            width: 2,
            height: 2,
            cell_size: 1
        }));
    }
}
