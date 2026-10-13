//! Visibility: shroud and fog of war (PLAN.md section 5, equation 6, and section 7, M4;
//! docs/DECISIONS.md D30).
//!
//! Each player of the match ([`Players`]) has a [`Vision`] of the grid. In the `Visibility` stage
//! of every tick it is recomputed from the player's units and buildings: a cell is visible when
//! some owned unit or building with [`Sight`] `s` satisfies `dx² + dy² ≤ s²`, with `dx` and `dy`
//! in cells between the cell and the unit's cell, or the nearest cell of the building's
//! footprint. A cell that has ever been visible stays explored. Unexplored cells are shroud;
//! explored cells that are not visible are fog.
//!
//! A unit or building of another player is visible to a player when one of its cells is visible
//! ([`Vision::sees_unit`], [`Vision::sees_footprint`]). The simulation uses this to hide unseen enemies from target
//! acquisition and from command validation; the game uses it to hide them from rendering.
//! Commands and the `Combat` stage of a tick see the vision computed at the end of the previous
//! tick, so before the first tick nothing is visible.

use bevy_ecs::system::{Query, Res, ResMut};

use crate::building::{Building, Footprint};
use crate::combat::Sight;
use crate::economy::Players;
use crate::hash::Fnv1a64;
use crate::map::{CellPos, Grid};
use crate::unit::{Mobile, Owner};

/// Bit of a cell that is visible now.
const VISIBLE: u8 = 1;
/// Bit of a cell that has been visible.
const EXPLORED: u8 = 2;

/// The vision of a player who is not a player of the match: nothing is visible.
pub(crate) static NO_VISION: Vision = Vision {
    width: 0,
    height: 0,
    cells: Vec::new(),
};

/// A player's view of the grid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vision {
    width: u32,
    height: u32,
    /// Row-major, as the grid: [`VISIBLE`] and [`EXPLORED`] bits.
    cells: Vec<u8>,
}

impl Vision {
    pub(crate) fn conceal(&mut self, centre: CellPos, radius: u32) {
        for y in centre.y.saturating_sub(radius as i32)..=centre.y.saturating_add(radius as i32) {
            for x in centre.x.saturating_sub(radius as i32)..=centre.x.saturating_add(radius as i32)
            {
                if let Some(i) = self.index(CellPos::new(x, y)) {
                    self.cells[i] &= !VISIBLE;
                }
            }
        }
    }

    fn index(&self, cell: CellPos) -> Option<usize> {
        let x = u32::try_from(cell.x).ok()?;
        let y = u32::try_from(cell.y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    fn bits(&self, cell: CellPos) -> u8 {
        self.index(cell)
            .and_then(|index| self.cells.get(index))
            .copied()
            .unwrap_or(0)
    }

    /// Whether `cell` is visible now (`false` outside the map).
    pub fn is_visible(&self, cell: CellPos) -> bool {
        self.bits(cell) & VISIBLE != 0
    }

    /// Whether `cell` has ever been visible: not shroud (`false` outside the map).
    pub fn is_explored(&self, cell: CellPos) -> bool {
        self.bits(cell) & EXPLORED != 0
    }

    /// Whether a unit that moves as `mobile` is visible: the cell it stands in or the cell it is
    /// stepping into is (docs/DECISIONS.md D30).
    pub fn sees_unit(&self, mobile: &Mobile) -> bool {
        self.is_visible(mobile.cell) || mobile.step.is_some_and(|step| self.is_visible(step.to))
    }

    /// Whether a building with this footprint is visible: some cell of it is.
    pub fn sees_footprint(&self, footprint: Footprint) -> bool {
        footprint.cells().any(|cell| self.is_visible(cell))
    }

    /// The number of cells visible now.
    pub fn visible_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|&&bits| bits & VISIBLE != 0)
            .count()
    }

    /// The number of explored cells.
    pub fn explored_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|&&bits| bits & EXPLORED != 0)
            .count()
    }

    /// Starts a recomputation: sized to `grid` (a grid of another size, which only match setup
    /// installs, starts a new vision with nothing explored), with no cell visible.
    fn begin(&mut self, grid: &Grid) {
        if self.width != grid.width() || self.height != grid.height() {
            let cells =
                usize::try_from(u64::from(grid.width()) * u64::from(grid.height())).unwrap_or(0);
            *self = Vision {
                width: grid.width(),
                height: grid.height(),
                cells: vec![0; cells],
            };
            return;
        }
        for bits in &mut self.cells {
            *bits &= !VISIBLE;
        }
    }

    /// A vision of `grid` in which the cells within `sight` of `cell` are visible (for unit
    /// tests of what a player may know).
    #[cfg(test)]
    pub(crate) fn reveal_for_test(&mut self, grid: &Grid, cell: CellPos, sight: u32) {
        self.begin(grid);
        let (x, y) = (i64::from(cell.x), i64::from(cell.y));
        self.reveal((x, y, x, y), sight);
    }

    /// Reveals the cells within `sight` of the rectangle of cells `x0..=x1`, `y0..=y1`
    /// (equation 6, measured to the rectangle's nearest cell). Computed in `i64`, so no source
    /// position overflows (invariant I12).
    pub(crate) fn reveal(&mut self, (x0, y0, x1, y1): (i64, i64, i64, i64), sight: u32) {
        let s = i64::from(sight);
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        // Only cells inside the map and within `sight` of the rectangle are visited, so every
        // offset below is at most `sight` (≤ MAX_SIGHT); the operations saturate all the same
        // (invariant I12).
        let (left, right) = (
            x0.saturating_sub(s).max(0),
            x1.saturating_add(s).min(width - 1),
        );
        let (top, bottom) = (
            y0.saturating_sub(s).max(0),
            y1.saturating_add(s).min(height - 1),
        );
        let limit = s.saturating_mul(s);
        for y in top..=bottom {
            let dy = y0.saturating_sub(y).max(y.saturating_sub(y1)).max(0);
            for x in left..=right {
                let dx = x0.saturating_sub(x).max(x.saturating_sub(x1)).max(0);
                if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) > limit {
                    continue;
                }
                // Inside the map by the clamping above.
                if let Ok(index) = usize::try_from(y * width + x)
                    && let Some(bits) = self.cells.get_mut(index)
                {
                    *bits |= VISIBLE | EXPLORED;
                }
            }
        }
    }

    /// Width, height, and every cell's bits.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u32(self.width);
        hasher.write_u32(self.height);
        hasher.write(&self.cells);
    }
}

/// Recomputes every player's vision from its units and buildings (the `Visibility` stage). The
/// order of the sources does not matter: revealing is a union (invariant I4).
pub(crate) fn update_visibility(
    grid: Res<Grid>,
    mut players: ResMut<Players>,
    sources: Query<(&Owner, &Sight, Option<&Mobile>, Option<&Building>)>,
) {
    for (_, state) in players.iter_mut() {
        state.vision.begin(&grid);
    }
    for (owner, sight, mobile, building) in &sources {
        let Some(state) = players.get_mut(owner.0) else {
            continue;
        };
        let area = match (mobile, building) {
            (Some(mobile), _) => {
                let (x, y) = (i64::from(mobile.cell.x), i64::from(mobile.cell.y));
                (x, y, x, y)
            }
            (None, Some(building)) => {
                let origin = building.footprint.origin;
                let (x, y) = (i64::from(origin.x), i64::from(origin.y));
                (
                    x,
                    y,
                    x + i64::from(building.footprint.width.max(1)) - 1,
                    y + i64::from(building.footprint.height.max(1)) - 1,
                )
            }
            (None, None) => continue,
        };
        state.vision.reveal(area, sight.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vision(width: u32, height: u32) -> Vision {
        Vision {
            width,
            height,
            cells: vec![0; (width * height) as usize],
        }
    }

    #[test]
    fn reveal_follows_equation_6_and_stays_inside_the_map() {
        let mut view = vision(20, 20);
        view.reveal((10, 10, 10, 10), 5);
        assert!(view.is_visible(CellPos::new(15, 10)), "dx = sight");
        assert!(!view.is_visible(CellPos::new(16, 10)), "dx = sight + 1");
        assert!(view.is_visible(CellPos::new(13, 14)), "3² + 4² = 5²");
        assert!(!view.is_visible(CellPos::new(14, 14)), "4² + 4² > 5²");
        // At the map's corner the circle is clipped.
        let mut corner = vision(4, 4);
        corner.reveal((0, 0, 0, 0), 16);
        assert_eq!(corner.visible_cells(), 16);
        // Sight 0: only the source's own cells.
        let mut own = vision(8, 8);
        own.reveal((2, 3, 3, 4), 0);
        assert_eq!(own.visible_cells(), 4);
        // Far outside the map and at the extremes of i32: nothing, and no overflow.
        let mut far = vision(8, 8);
        far.reveal((i64::from(i32::MIN), 0, i64::from(i32::MIN), 0), 16);
        far.reveal(
            (
                i64::from(i32::MAX),
                i64::from(i32::MAX),
                i64::from(i32::MAX),
                i64::from(i32::MAX),
            ),
            16,
        );
        assert_eq!(far.visible_cells(), 0);
    }

    #[test]
    fn the_hash_covers_every_cell_of_a_vision() {
        let digest = |view: &Vision| {
            let mut hasher = Fnv1a64::new();
            view.hash_into(&mut hasher);
            hasher.finish()
        };
        let base = vision(4, 4);
        let mut seen = base.clone();
        seen.reveal((3, 3, 3, 3), 0);
        assert_ne!(digest(&base), digest(&seen));
        // Explored but no longer visible differs from both.
        let mut fog = seen.clone();
        let grid_like = fog.clone();
        fog.cells.iter_mut().for_each(|bits| *bits &= !VISIBLE);
        assert_ne!(digest(&fog), digest(&seen));
        assert_ne!(digest(&fog), digest(&base));
        assert_eq!(digest(&grid_like), digest(&seen));
        assert_ne!(digest(&vision(4, 5)), digest(&vision(5, 4)));
    }
}
