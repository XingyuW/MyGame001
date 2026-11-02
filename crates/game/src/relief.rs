//! Raised ground on screen (docs/DECISIONS.md, "Maps with relief"): every cell of a map with
//! relief is drawn `STEP_PX` higher per step of its elevation (`sim::Grid::elevation`), and so
//! is everything standing on it. Presentation only: it reads the map's terrain (`MapTerrain`)
//! and never the simulation's state.
//!
//! Between neighbouring cells whose heights differ by at most one step (a slope units walk) the
//! ground's height is interpolated between the cells' centres, so units glide up and down; a
//! higher difference is a cliff, and there each cell keeps its own height.

use bevy::math::Vec2;
use sim::{CellPos, Footprint, Grid};

use crate::map::MapTerrain;
use crate::projection::{TileSize, to_world, world_to_cell};

/// Screen pixels a cell rises per step of elevation.
pub const STEP_PX: f32 = 8.0;

/// The height of `cell` in screen pixels (0 outside the map and on a flat map).
pub fn cell_lift(grid: &Grid, cell: CellPos) -> f32 {
    f32::from(grid.elevation(cell)) * STEP_PX
}

/// How high the ground lies at continuous cell coordinates (`u`, `v`), in screen pixels:
/// interpolated between the centres of the four cells around the point, each of which counts
/// with the height of the cell the point lies in where the two are more than a step apart.
pub fn ground_lift(grid: &Grid, u: f32, v: f32) -> f32 {
    if grid.elevations().is_empty() || !u.is_finite() || !v.is_finite() {
        return 0.0;
    }
    let own = CellPos::new(u.floor() as i32, v.floor() as i32);
    let home = i32::from(grid.elevation(own));
    let (u0, v0) = ((u - 0.5).floor(), (v - 0.5).floor());
    let (fu, fv) = (u - 0.5 - u0, v - 0.5 - v0);
    let height = |dx: i32, dy: i32| {
        let cell = CellPos::new(u0 as i32 + dx, v0 as i32 + dy);
        let h = if grid.contains(cell) {
            i32::from(grid.elevation(cell))
        } else {
            home
        };
        (if (h - home).abs() > 1 { home } else { h }) as f32
    };
    let top = height(0, 0) * (1.0 - fu) + height(1, 0) * fu;
    let bottom = height(0, 1) * (1.0 - fu) + height(1, 1) * fu;
    (top * (1.0 - fv) + bottom * fv) * STEP_PX
}

/// The world position of the ground at continuous cell coordinates (`u`, `v`) of the match's
/// grid (`SimHost::grid`), which holds the same relief as the map.
pub fn grid_ground(grid: Option<&Grid>, u: f32, v: f32, tile: TileSize) -> Vec2 {
    to_world(u, v, tile) + Vec2::Y * grid.map_or(0.0, |grid| ground_lift(grid, u, v))
}

/// [`ground_lift`] where the map's terrain is known; 0 otherwise.
pub fn lift(terrain: Option<&MapTerrain>, u: f32, v: f32) -> f32 {
    terrain.map_or(0.0, |terrain| ground_lift(&terrain.0, u, v))
}

/// The world position of the ground at continuous cell coordinates (`u`, `v`).
pub fn ground(terrain: Option<&MapTerrain>, u: f32, v: f32, tile: TileSize) -> Vec2 {
    to_world(u, v, tile) + Vec2::Y * lift(terrain, u, v)
}

/// The world position of the centre of `cell` on its raised ground.
pub fn cell_point(terrain: Option<&MapTerrain>, cell: CellPos, tile: TileSize) -> Vec2 {
    ground(terrain, cell.x as f32 + 0.5, cell.y as f32 + 0.5, tile)
}

/// The corners of `cell` (top, right, bottom, left) on its raised ground.
pub fn cell_corners(terrain: Option<&MapTerrain>, cell: CellPos, tile: TileSize) -> [Vec2; 4] {
    let lift = terrain.map_or(0.0, |terrain| cell_lift(&terrain.0, cell));
    crate::projection::cell_corners(cell, tile).map(|corner| corner + Vec2::Y * lift)
}

/// How high a building with `footprint` stands: on the highest cell under it.
pub fn footprint_lift(terrain: Option<&MapTerrain>, footprint: Footprint) -> f32 {
    terrain.map_or(0.0, |terrain| {
        footprint
            .cells()
            .map(|cell| cell_lift(&terrain.0, cell))
            .fold(0.0, f32::max)
    })
}

/// The cell of a `width` × `height` map drawn under `world`: of the cells whose raised diamond
/// holds the point, the one in front (lowest on the screen); on a flat map
/// `projection::pick_cell`. A point on a cliff face picks the cell below it.
pub fn pick(
    terrain: Option<&MapTerrain>,
    world: Vec2,
    tile: TileSize,
    width: u32,
    height: u32,
) -> Option<CellPos> {
    let inside = |cell: CellPos| {
        u32::try_from(cell.x).is_ok_and(|x| x < width)
            && u32::try_from(cell.y).is_ok_and(|y| y < height)
    };
    let flat = world_to_cell(world, tile).filter(|&cell| inside(cell));
    let Some(grid) = terrain.map(|terrain| &terrain.0) else {
        return flat;
    };
    if grid.elevations().is_empty() {
        return flat;
    }
    let mut best: Option<CellPos> = None;
    for step in 0..=sim::MAX_ELEVATION {
        let lifted = f32::from(step) * STEP_PX;
        let Some(cell) = world_to_cell(world - Vec2::Y * lifted, tile) else {
            continue;
        };
        if inside(cell)
            && grid.elevation(cell) == step
            && best.is_none_or(|b| cell.x + cell.y >= b.x + b.y)
        {
            best = Some(cell);
        }
    }
    best.or(flat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Cell;

    const TILE: TileSize = TileSize {
        width: 64.0,
        height: 32.0,
    };

    fn hill() -> MapTerrain {
        let open = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        let mut grid = Grid::from_cells(4, 4, vec![open; 16]).expect("a grid");
        // Row-major: a cliff of 5 steps at (3, 0), a slope from 0 to 2 along x on row 1.
        let mut heights = vec![0_u8; 16];
        heights[3] = 5;
        heights[4] = 0;
        heights[5] = 1;
        heights[6] = 2;
        assert!(grid.set_relief(heights, Vec::new()));
        MapTerrain(grid)
    }

    #[test]
    fn slopes_are_interpolated_and_cliffs_are_not() {
        let terrain = hill();
        let lift = |u: f32, v: f32| super::lift(Some(&terrain), u, v);
        assert_eq!(
            lift(1.5, 1.5),
            STEP_PX,
            "a cell's centre lies at its height"
        );
        assert_eq!(lift(2.0, 1.5), 1.5 * STEP_PX, "halfway up the slope");
        assert_eq!(lift(3.5, 0.5), 5.0 * STEP_PX);
        // Beside the cliff, the low cell keeps its own height up to the cliff's edge.
        assert_eq!(lift(2.9, 0.5), 0.0);
        assert_eq!(super::lift(None, 2.0, 1.5), 0.0, "no terrain: flat");
    }

    #[test]
    fn picking_finds_the_raised_cell_in_front() {
        let terrain = hill();
        // The centre of the cliff cell, drawn five steps up.
        let cliff = cell_point(Some(&terrain), CellPos::new(3, 0), TILE);
        assert_eq!(
            pick(Some(&terrain), cliff, TILE, 4, 4),
            Some(CellPos::new(3, 0))
        );
        // Where the cliff cell would lie unraised, the cells there are drawn.
        let low = crate::projection::cell_centre(CellPos::new(2, 0), TILE);
        assert_eq!(
            pick(Some(&terrain), low, TILE, 4, 4),
            Some(CellPos::new(2, 0))
        );
        assert_eq!(
            footprint_lift(
                Some(&terrain),
                Footprint {
                    origin: CellPos::new(2, 0),
                    width: 2,
                    height: 2
                }
            ),
            5.0 * STEP_PX
        );
    }
}
