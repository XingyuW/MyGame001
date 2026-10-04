//! Isometric projection between map cells and world coordinates (PLAN.md section 5, equation 7;
//! docs/DECISIONS.md, D7).
//!
//! World coordinates are Bevy's 2D coordinates (y up). Continuous cell coordinates (u, v) map to
//! `world = ((u − v)·W/2, −(u + v)·H/2)`. Integer coordinates are diamond corners: cell c spans
//! [c, c + 1) on each axis, its centre is at c + ½, and the top corner of cell (0, 0) is the
//! world origin. The map entity is placed at [`map_origin_offset`], which makes
//! `bevy_ecs_tiled` draw every tile exactly where this projection puts it.

use bevy::math::{Rect, Vec2};
use bevy_ecs_tilemap::prelude::TilePos;
use sim::CellPos;

/// Size of one cell's diamond in pixels (world units at zoom 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileSize {
    pub width: f32,
    pub height: f32,
}

impl TileSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
        }
    }
}

/// Equation 7 for continuous cell coordinates.
pub fn to_world(u: f32, v: f32, tile: TileSize) -> Vec2 {
    Vec2::new((u - v) * tile.width / 2.0, -(u + v) * tile.height / 2.0)
}

/// Centre of `cell` in world coordinates.
pub fn cell_centre(cell: CellPos, tile: TileSize) -> Vec2 {
    to_world(cell.x as f32 + 0.5, cell.y as f32 + 0.5, tile)
}

/// Corners of `cell` in world coordinates: top, right, bottom, left.
pub fn cell_corners(cell: CellPos, tile: TileSize) -> [Vec2; 4] {
    let (x, y) = (cell.x as f32, cell.y as f32);
    [
        to_world(x, y, tile),
        to_world(x + 1.0, y, tile),
        to_world(x + 1.0, y + 1.0, tile),
        to_world(x, y + 1.0, tile),
    ]
}

/// The cell containing `world`, by the inverse of equation 7 followed by floor; `None` if
/// `world` is not finite. Points on a shared edge belong to the cell with the larger
/// coordinate.
pub fn world_to_cell(world: Vec2, tile: TileSize) -> Option<CellPos> {
    if !world.is_finite() {
        return None;
    }
    let a = world.x / (tile.width / 2.0);
    let b = -world.y / (tile.height / 2.0);
    // `as` saturates, so points far outside any map stay outside it.
    Some(CellPos::new(
        ((a + b) / 2.0).floor() as i32,
        ((b - a) / 2.0).floor() as i32,
    ))
}

/// The cell of a `width` × `height` map under `world`, or `None` outside the map.
pub fn pick_cell(world: Vec2, tile: TileSize, width: u32, height: u32) -> Option<CellPos> {
    world_to_cell(world, tile).filter(|cell| {
        u32::try_from(cell.x).is_ok_and(|x| x < width)
            && u32::try_from(cell.y).is_ok_and(|y| y < height)
    })
}

/// Axis-aligned bounding rectangle of a `width` × `height` map's diamond.
pub fn map_bounds(width: u32, height: u32, tile: TileSize) -> Rect {
    let (w, h) = (width as f32, height as f32);
    Rect::new(
        to_world(0.0, h, tile).x,
        to_world(w, h, tile).y,
        to_world(w, 0.0, tile).x,
        to_world(0.0, 0.0, tile).y,
    )
}

/// Translation of the map entity (spawned with `TilemapAnchor::None`) under which
/// `bevy_ecs_tiled` draws the centre of every cell at [`cell_centre`]. `bevy_ecs_tiled` places
/// Tiled cell (x, y) at the `bevy_ecs_tilemap` position (x, height − 1 − y), whose diamond
/// centre is at [`cell_centre`] plus `((height − 1)·W/2, height·H/2)` (docs/DECISIONS.md, D7).
pub fn map_origin_offset(height: u32, tile: TileSize) -> Vec2 {
    let h = height as f32;
    -Vec2::new((h - 1.0) * tile.width / 2.0, h * tile.height / 2.0)
}

/// The map cell drawn by the tile at `tile` of the map's tilemap: `bevy_ecs_tiled` places Tiled
/// cell (x, y) at `TilePos { x, y: height − 1 − y }` (docs/DECISIONS.md D7), so this is the
/// inverse; `None` for a tile outside a map `height` cells tall.
pub fn tile_cell(tile: TilePos, height: u32) -> Option<CellPos> {
    let y = height.checked_sub(1)?.checked_sub(tile.y)?;
    Some(CellPos::new(
        i32::try_from(tile.x).ok()?,
        i32::try_from(y).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use bevy_ecs_tilemap::prelude::{
        IsoCoordSystem, TilePos, TilemapAnchor, TilemapGridSize, TilemapSize, TilemapTileSize,
        TilemapType,
    };

    use super::*;

    /// The tile size of the demo map. With it, every coordinate used below is a multiple of
    /// 1/64 cell and is computed exactly in `f32`, so picking is tested for zero error.
    const TILE: TileSize = TileSize {
        width: 64.0,
        height: 32.0,
    };

    /// Map sizes required by PLAN.md section 7, M1: 1×1, 1×N, N×1, and 64×64.
    const MAP_SIZES: [(u32, u32); 4] = [(1, 1), (1, 37), (37, 1), (64, 64)];

    /// Offsets inside a cell, in cells: the near corner, just inside, centre, just inside the
    /// far edges.
    const INSIDE: [f32; 4] = [0.0, 1.0 / 64.0, 0.5, 63.0 / 64.0];

    #[test]
    fn the_tested_tile_size_is_the_only_one_maps_may_use() {
        // Exact picking is established for `TILE` only, so `sim::load_map` rejects every other
        // tile size (docs/DECISIONS.md, D13).
        assert_eq!(TILE, TileSize::new(sim::TILE_WIDTH_PX, sim::TILE_HEIGHT_PX));
    }

    fn cells(width: u32, height: u32) -> impl Iterator<Item = CellPos> {
        (0..height as i32).flat_map(move |y| (0..width as i32).map(move |x| CellPos::new(x, y)))
    }

    #[test]
    fn cell_world_cell_round_trip_for_every_cell() {
        for (width, height) in MAP_SIZES {
            for cell in cells(width, height) {
                let centre = cell_centre(cell, TILE);
                assert_eq!(
                    pick_cell(centre, TILE, width, height),
                    Some(cell),
                    "{width}x{height}"
                );
                for corner in cell_corners(cell, TILE) {
                    // Halfway between the centre and each corner is inside the cell.
                    let inner = (centre + corner) / 2.0;
                    assert_eq!(pick_cell(inner, TILE, width, height), Some(cell));
                }
            }
        }
    }

    #[test]
    fn picking_at_centres_edges_and_corners() {
        for (width, height) in MAP_SIZES {
            for cell in cells(width, height) {
                let (x, y) = (cell.x as f32, cell.y as f32);
                for du in INSIDE {
                    for dv in INSIDE {
                        let world = to_world(x + du, y + dv, TILE);
                        assert_eq!(
                            pick_cell(world, TILE, width, height),
                            Some(cell),
                            "{width}x{height}, offset ({du}, {dv})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn shared_edges_and_corners_belong_to_the_larger_coordinate() {
        let (width, height) = (3, 3);
        let pick = |u: f32, v: f32| pick_cell(to_world(u, v, TILE), TILE, width, height);
        // Edge between (0, 1) and (1, 1), and between (1, 0) and (1, 1).
        assert_eq!(pick(1.0, 1.5), Some(CellPos::new(1, 1)));
        assert_eq!(pick(1.5, 1.0), Some(CellPos::new(1, 1)));
        // Corner shared by (0, 0), (1, 0), (0, 1), and (1, 1).
        assert_eq!(pick(1.0, 1.0), Some(CellPos::new(1, 1)));
        // The map's own corners: the top corner is inside, the other three are on the far
        // edges and therefore outside.
        assert_eq!(pick(0.0, 0.0), Some(CellPos::new(0, 0)));
        assert_eq!(pick(3.0, 0.0), None);
        assert_eq!(pick(0.0, 3.0), None);
        assert_eq!(pick(3.0, 3.0), None);
        // Just inside those three corners.
        let e = 1.0 / 64.0;
        assert_eq!(pick(3.0 - e, 0.0), Some(CellPos::new(2, 0)));
        assert_eq!(pick(0.0, 3.0 - e), Some(CellPos::new(0, 2)));
        assert_eq!(pick(3.0 - e, 3.0 - e), Some(CellPos::new(2, 2)));
    }

    #[test]
    fn picking_outside_the_map_returns_none() {
        let e = 1.0 / 64.0;
        for (width, height) in MAP_SIZES {
            let (w, h) = (width as f32, height as f32);
            let pick = |u: f32, v: f32| pick_cell(to_world(u, v, TILE), TILE, width, height);
            // Just beyond each of the four edges of the diamond, along its whole length.
            for i in 0..4 * width.max(height) {
                let t = i as f32 / 4.0;
                if t < w {
                    assert_eq!(pick(t, -e), None, "{width}x{height}, top-right edge");
                    assert_eq!(pick(t, h), None, "{width}x{height}, bottom-left edge");
                }
                if t < h {
                    assert_eq!(pick(-e, t), None, "{width}x{height}, top-left edge");
                    assert_eq!(pick(w, t), None, "{width}x{height}, bottom-right edge");
                }
            }
            // Far away, and points that are not finite.
            for world in [
                Vec2::new(1e9, 0.0),
                Vec2::new(-1e9, -1e9),
                Vec2::new(0.0, 1e30),
                Vec2::NAN,
                Vec2::new(f32::INFINITY, 0.0),
                Vec2::new(0.0, f32::NEG_INFINITY),
            ] {
                assert_eq!(pick_cell(world, TILE, width, height), None, "{world}");
            }
        }
    }

    #[test]
    fn map_bounds_enclose_the_diamond_exactly() {
        for (width, height) in MAP_SIZES {
            let bounds = map_bounds(width, height, TILE);
            let (w, h) = (width as f32, height as f32);
            // The four corners of the diamond lie on the four sides of the rectangle.
            assert_eq!(to_world(0.0, 0.0, TILE).y, bounds.max.y);
            assert_eq!(to_world(w, 0.0, TILE).x, bounds.max.x);
            assert_eq!(to_world(w, h, TILE).y, bounds.min.y);
            assert_eq!(to_world(0.0, h, TILE).x, bounds.min.x);
            for cell in cells(width, height) {
                for corner in cell_corners(cell, TILE) {
                    assert!(bounds.contains(corner), "{width}x{height}, {cell:?}");
                }
            }
        }
    }

    #[test]
    fn map_origin_offset_aligns_bevy_ecs_tilemap_with_the_projection() {
        // `bevy_ecs_tiled` stores Tiled cell (x, y) at TilePos (x, height − 1 − y) of a diamond
        // tilemap. Under the map entity's translation, the tilemap's tile centres and its own
        // picking must agree with this module for every cell.
        let grid = TilemapGridSize {
            x: TILE.width,
            y: TILE.height,
        };
        let tile = TilemapTileSize {
            x: TILE.width,
            y: TILE.height,
        };
        let map_type = TilemapType::Isometric(IsoCoordSystem::Diamond);
        for (width, height) in MAP_SIZES.into_iter().chain([(16, 16), (3, 50)]) {
            let size = TilemapSize {
                x: width,
                y: height,
            };
            let offset = map_origin_offset(height, TILE);
            for cell in cells(width, height) {
                let tile_pos = TilePos {
                    x: cell.x as u32,
                    y: height - 1 - cell.y as u32,
                };
                let drawn = offset
                    + tile_pos.center_in_world(
                        &size,
                        &grid,
                        &tile,
                        &map_type,
                        &TilemapAnchor::None,
                    );
                let ctx = format!("{width}x{height}, {cell:?}");
                assert!(
                    (drawn - cell_centre(cell, TILE)).abs().max_element() < 1e-3,
                    "{ctx}"
                );
                let picked_by_tilemap = TilePos::from_world_pos(
                    &(cell_centre(cell, TILE) - offset),
                    &size,
                    &grid,
                    &tile,
                    &map_type,
                    &TilemapAnchor::None,
                );
                assert_eq!(picked_by_tilemap, Some(tile_pos), "{ctx}");
            }
        }
    }

    #[test]
    fn tile_cell_inverts_the_tilemap_placement_of_every_cell() {
        // The tile under each cell centre, as the tilemap itself picks it, maps back to the cell
        // (M4: the fog tints tiles by the cell they draw).
        let grid = TilemapGridSize {
            x: TILE.width,
            y: TILE.height,
        };
        let tile = TilemapTileSize {
            x: TILE.width,
            y: TILE.height,
        };
        let map_type = TilemapType::Isometric(IsoCoordSystem::Diamond);
        for (width, height) in MAP_SIZES {
            let size = TilemapSize {
                x: width,
                y: height,
            };
            let offset = map_origin_offset(height, TILE);
            for cell in cells(width, height) {
                let picked = TilePos::from_world_pos(
                    &(cell_centre(cell, TILE) - offset),
                    &size,
                    &grid,
                    &tile,
                    &map_type,
                    &TilemapAnchor::None,
                )
                .expect("inside");
                assert_eq!(tile_cell(picked, height), Some(cell), "{width}x{height}");
            }
            assert_eq!(tile_cell(TilePos::new(0, height), height), None);
        }
        assert_eq!(tile_cell(TilePos::new(0, 0), 0), None);
        assert_eq!(tile_cell(TilePos::new(u32::MAX, 0), 1), None);
    }
}
