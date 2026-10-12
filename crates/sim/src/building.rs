//! Buildings: footprints, placement rules, components, and spawning (PLAN.md section 7, M3;
//! docs/DECISIONS.md D24).
//!
//! A building covers a rectangle of cells, its footprint. The cells it covers are impassable for
//! units (`Grid::is_passable`), so no unit can stand in or enter them (invariant I6). A footprint
//! is valid when it lies inside the map, every cell is buildable terrain, no building covers any
//! of its cells, and no unit holds any of them; a building placed during a match must also lie
//! within the economy file's placement distance of a building of its owner.

use std::collections::BTreeSet;
use std::fmt;

use bevy_ecs::component::Component;
use bevy_ecs::world::World;

use crate::combat::combat_components;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy_defs::BuildingKind;
use crate::hash::Fnv1a64;
use crate::id::{SimId, SimIdAllocator};
use crate::map::{CellPos, Grid};
use crate::pathfinding::octile_distance;
use crate::unit::Owner;
use crate::visibility::Vision;

/// The rectangle of cells a building covers: `width` cells along x and `height` cells along y
/// from `origin`, its cell with the smallest coordinates (the top corner of the diamond on
/// screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footprint {
    pub origin: CellPos,
    pub width: u8,
    pub height: u8,
}

impl Footprint {
    /// Smallest and largest x and y, in `i64` so that no origin a command can carry overflows
    /// (invariant I12).
    fn bounds(&self) -> (i64, i64, i64, i64) {
        let x0 = i64::from(self.origin.x);
        let y0 = i64::from(self.origin.y);
        (
            x0,
            y0,
            x0 + i64::from(self.width.max(1)) - 1,
            y0 + i64::from(self.height.max(1)) - 1,
        )
    }

    /// Whether every cell lies inside `grid`.
    pub fn inside(&self, grid: &Grid) -> bool {
        let (x0, y0, x1, y1) = self.bounds();
        x0 >= 0 && y0 >= 0 && x1 < i64::from(grid.width()) && y1 < i64::from(grid.height())
    }

    pub fn contains(&self, cell: CellPos) -> bool {
        let (x0, y0, x1, y1) = self.bounds();
        (x0..=x1).contains(&i64::from(cell.x)) && (y0..=y1).contains(&i64::from(cell.y))
    }

    /// The covered cells in row-major order (cells whose coordinates do not fit in `i32` are
    /// left out; they lie outside every map).
    pub fn cells(&self) -> impl Iterator<Item = CellPos> + use<> {
        let (x0, y0, x1, y1) = self.bounds();
        (y0..=y1).flat_map(move |y| (x0..=x1).filter_map(move |x| cell(x, y)))
    }

    /// The cells around the footprint, at Chebyshev distance 1, in row-major order: where units
    /// leave a building and where harvesters unload at a refinery.
    pub fn ring(&self) -> impl Iterator<Item = CellPos> + use<> {
        let (x0, y0, x1, y1) = self.bounds();
        (y0 - 1..=y1 + 1).flat_map(move |y| {
            (x0 - 1..=x1 + 1)
                .filter(move |&x| y == y0 - 1 || y == y1 + 1 || x == x0 - 1 || x == x1 + 1)
                .filter_map(move |x| cell(x, y))
        })
    }

    /// Chebyshev distance in cells between the nearest cells of the two footprints: 0 if they
    /// overlap, 1 if they touch (also at a corner), and so on.
    pub fn distance(&self, other: &Footprint) -> u32 {
        let (ax0, ay0, ax1, ay1) = self.bounds();
        let (bx0, by0, bx1, by1) = other.bounds();
        let dx = (bx0 - ax1).max(ax0 - bx1).max(0);
        let dy = (by0 - ay1).max(ay0 - by1).max(0);
        u32::try_from(dx.max(dy)).unwrap_or(u32::MAX)
    }

    /// Chebyshev distance from the footprint to `cell` (0 inside it, 1 in its ring).
    pub fn distance_to_cell(&self, cell: CellPos) -> u32 {
        self.distance(&Footprint {
            origin: cell,
            width: 1,
            height: 1,
        })
    }
}

fn cell(x: i64, y: i64) -> Option<CellPos> {
    Some(CellPos::new(i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

/// A building of type `kind`. `pending_units` are units waiting to leave it (the free units of
/// a building placed during the match, D24); they leave, oldest first, when a cell of its ring
/// is free.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Building {
    pub kind: BuildingKind,
    pub footprint: Footprint,
    pub pending_units: Vec<UnitKind>,
}

/// A building whose type produces units. Units it produces move to its rally point, if set.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Producer {
    pub rally: Option<CellPos>,
}

/// Why a footprint is not a valid place for a building.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// Some cell of the footprint lies outside the map.
    OutsideMap(Footprint),
    /// The cell's terrain is not buildable.
    NotBuildable(CellPos),
    /// The cell is covered by another building.
    Overlap { cell: CellPos, building: SimId },
    /// A unit stands in or is entering the cell.
    Occupied { cell: CellPos, unit: SimId },
    /// Under fog of war, the issuing player does not see the cell. Checked before the terrain,
    /// buildings, and units of the footprint, so that a rejection names nothing the player does
    /// not see (docs/DECISIONS.md D33).
    NotVisible(CellPos),
    /// The nearest building of the player is `distance` cells away (`None`: the player owns
    /// none), more than the economy file's `placement_distance` = `max`.
    TooFar { distance: Option<u32>, max: u32 },
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideMap(footprint) => write!(
                f,
                "the {}x{} footprint at ({}, {}) does not lie inside the map",
                footprint.width, footprint.height, footprint.origin.x, footprint.origin.y
            ),
            Self::NotBuildable(cell) => {
                write!(f, "cell ({}, {}) is not buildable", cell.x, cell.y)
            }
            Self::Overlap { cell, building } => write!(
                f,
                "cell ({}, {}) is covered by building {}",
                cell.x, cell.y, building.0
            ),
            Self::Occupied { cell, unit } => {
                write!(
                    f,
                    "cell ({}, {}) is held by unit {}",
                    cell.x, cell.y, unit.0
                )
            }
            Self::NotVisible(cell) => {
                write!(
                    f,
                    "cell ({}, {}) is not visible to the player",
                    cell.x, cell.y
                )
            }
            Self::TooFar {
                distance: Some(distance),
                max,
            } => write!(
                f,
                "the nearest own building is {distance} cells away, at most {max} are allowed"
            ),
            Self::TooFar {
                distance: None,
                max,
            } => write!(
                f,
                "the player owns no building, and a building must lie within {max} cells of one"
            ),
        }
    }
}

/// Checks that `footprint` lies inside the map and that each of its cells, in row-major order,
/// is buildable, not covered by a building, and not held by a unit; the first violation is the
/// error.
pub(crate) fn check_footprint(grid: &Grid, footprint: Footprint) -> Result<(), PlacementError> {
    if !footprint.inside(grid) {
        return Err(PlacementError::OutsideMap(footprint));
    }
    for cell in footprint.cells() {
        if !grid.cell(cell).is_some_and(|terrain| terrain.buildable) {
            return Err(PlacementError::NotBuildable(cell));
        }
        if let Some(building) = grid.building(cell) {
            return Err(PlacementError::Overlap { cell, building });
        }
        if let Some(unit) = grid.occupant(cell) {
            return Err(PlacementError::Occupied { cell, unit });
        }
    }
    Ok(())
}

/// Shipyards occupy water and must touch accessible shore; ordinary structures use land.
pub(crate) fn check_building_footprint(
    grid: &Grid,
    footprint: Footprint,
    shipyard: bool,
) -> Result<(), PlacementError> {
    if !shipyard {
        return check_footprint(grid, footprint);
    }
    if !footprint.inside(grid) {
        return Err(PlacementError::OutsideMap(footprint));
    }
    for cell in footprint.cells() {
        if !grid.cell(cell).is_some_and(|c| c.water) {
            return Err(PlacementError::NotBuildable(cell));
        }
        if let Some(building) = grid.building(cell) {
            return Err(PlacementError::Overlap { cell, building });
        }
        if let Some(unit) = grid.occupant(cell) {
            return Err(PlacementError::Occupied { cell, unit });
        }
    }
    if !footprint.ring().any(|c| grid.is_passable(c))
        || !footprint
            .ring()
            .any(|c| grid.passable_for(c, crate::Domain::Naval))
    {
        return Err(PlacementError::NotBuildable(footprint.origin));
    }
    Ok(())
}

/// Under fog of war: every cell of a footprint inside the map must be visible to the player
/// (`vision`); the first unseen cell in row-major order is the error. A footprint outside the
/// map is left to [`check_footprint`] (`OutsideMap`, which reveals nothing). Checked before
/// [`check_footprint`], whose errors name the building or unit in a cell, so that a placement
/// reveals nothing about cells the player does not see (docs/DECISIONS.md D33).
pub(crate) fn check_visible(
    grid: &Grid,
    footprint: Footprint,
    vision: &Vision,
) -> Result<(), PlacementError> {
    if !footprint.inside(grid) {
        return Ok(());
    }
    match footprint.cells().find(|&cell| !vision.is_visible(cell)) {
        Some(cell) => Err(PlacementError::NotVisible(cell)),
        None => Ok(()),
    }
}

/// The distance rule: the footprint must lie within `max` cells of one of `owned` (the
/// footprints of the player's buildings).
pub(crate) fn check_distance(
    footprint: Footprint,
    owned: impl Iterator<Item = Footprint>,
    max: u32,
) -> Result<(), PlacementError> {
    let nearest = owned.map(|other| footprint.distance(&other)).min();
    match nearest {
        Some(distance) if distance <= max => Ok(()),
        distance => Err(PlacementError::TooFar { distance, max }),
    }
}

/// How many cells beyond a building's ring [`exit_cell`] looks for a free cell when the ring is
/// full. Enough for well over a thousand units around one factory; the bound keeps the search
/// cheap where a producer is sealed in for good.
pub(crate) const EXIT_SEARCH_DEPTH: u32 = 24;

/// The free cell where a unit leaves the building of `footprint`: inside the map, passable, and
/// with room for the unit ([`Grid::has_room`]: held by no unit, or for a unit on foot,
/// `on_foot`, by no vehicle and by fewer than three infantry). A cell of the footprint's ring if
/// one is free: the one nearest to `toward` (the rally point) if given, otherwise the one lowest
/// on screen (largest x + y); ties are broken by row, then column.
///
/// When every ring cell is held, the unit comes out further away instead of waiting inside, so
/// that a crowd around a factory never stops its production: the search spreads outward from
/// the ring over passable cells (through the units standing there, never across terrain they
/// could not cross), one step at a time up to [`EXIT_SEARCH_DEPTH`], and the nearest step with a
/// free cell supplies it by the same preference as the ring.
pub(crate) fn exit_cell(
    grid: &Grid,
    footprint: Footprint,
    toward: Option<CellPos>,
    on_foot: bool,
) -> Option<CellPos> {
    let preference = |cell: &CellPos| match toward {
        Some(target) => (i64::from(octile_distance(*cell, target)), cell.y, cell.x),
        None => (-(i64::from(cell.x) + i64::from(cell.y)), cell.y, cell.x),
    };
    let mut frontier: Vec<CellPos> = footprint
        .ring()
        .filter(|&cell| grid.is_passable(cell))
        .collect();
    let mut seen: BTreeSet<CellPos> = frontier.iter().copied().collect();
    for _ in 0..=EXIT_SEARCH_DEPTH {
        let free = frontier
            .iter()
            .filter(|&&cell| grid.has_room(cell, on_foot))
            .min_by_key(|&cell| preference(cell));
        if let Some(&cell) = free {
            return Some(cell);
        }
        let mut next = Vec::new();
        for cell in &frontier {
            for (dx, dy) in NEIGHBOURS {
                let (Some(x), Some(y)) = (cell.x.checked_add(dx), cell.y.checked_add(dy)) else {
                    continue;
                };
                let neighbour = CellPos::new(x, y);
                if grid.is_passable(neighbour) && seen.insert(neighbour) {
                    next.push(neighbour);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    None
}

/// The eight cells around a cell.
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Why a building could not be spawned as scenario setup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnBuildingError {
    UnknownBuildingType(String),
    Placement(PlacementError),
    IdsExhausted,
}

impl fmt::Display for SpawnBuildingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBuildingType(id) => write!(f, "building type {id:?} is not defined"),
            Self::Placement(error) => error.fmt(f),
            Self::IdsExhausted => write!(f, "no simulation id is left"),
        }
    }
}

impl std::error::Error for SpawnBuildingError {}

/// The components of a new building.
pub(crate) fn building_bundle(
    definitions: &Definitions,
    id: SimId,
    owner: PlayerId,
    kind: BuildingKind,
    footprint: Footprint,
    pending_units: Vec<UnitKind>,
) -> (SimId, Owner, Building, Option<Producer>) {
    (
        id,
        Owner(owner),
        Building {
            kind,
            footprint,
            pending_units,
        },
        definitions
            .produces_units(kind)
            .then_some(Producer::default()),
    )
}

/// Spawns a building of type `building_type` owned by `owner` with its footprint's origin at
/// `origin`.
///
/// Scenario and match setup only (tests, and the game before the first tick), like
/// [`spawn_unit`](crate::spawn_unit): the footprint must be valid, but the distance rule does
/// not apply and the building delivers no free units. During a match buildings are produced and
/// placed with commands. The world must have been set up by [`SimPlugin`](crate::SimPlugin),
/// with the grid and definitions of the match installed. With a combat file, the building gets
/// its hit points, armour, sight, and weapon (M4).
pub fn spawn_building(
    world: &mut World,
    owner: PlayerId,
    building_type: &str,
    origin: CellPos,
) -> Result<SimId, SpawnBuildingError> {
    let definitions = world.resource::<Definitions>().clone();
    let kind = definitions
        .building_kind(building_type)
        .ok_or_else(|| SpawnBuildingError::UnknownBuildingType(building_type.to_owned()))?;
    let def = definitions
        .building(kind)
        .ok_or_else(|| SpawnBuildingError::UnknownBuildingType(building_type.to_owned()))?;
    let footprint = Footprint {
        origin,
        width: def.width,
        height: def.height,
    };
    check_building_footprint(
        world.resource::<Grid>(),
        footprint,
        definitions
            .metadata(crate::ProductionItem::Building(kind))
            .is_some_and(|e| e.role == crate::Role::Shipyard),
    )
    .map_err(SpawnBuildingError::Placement)?;
    let id = world
        .resource_mut::<SimIdAllocator>()
        .allocate()
        .ok_or(SpawnBuildingError::IdsExhausted)?;
    {
        let mut grid = world.resource_mut::<Grid>();
        for cell in footprint.cells() {
            grid.cover(cell, id);
        }
    }
    let (id, owner, building, producer) =
        building_bundle(&definitions, id, owner, kind, footprint, Vec::new());
    let mut entity = world.spawn((id, owner, building));
    if let Some(producer) = producer {
        entity.insert(producer);
    }
    if let Some(terms) = definitions.building_combat(kind) {
        let (components, weapon) = combat_components(terms);
        entity.insert(components);
        if let Some(weapon) = weapon {
            entity.insert(weapon);
        }
    }
    Ok(id)
}

/// Appends a building's components to the hash: kind, footprint (origin, width, height), the
/// pending units, and the rally point (flag byte 0 for a building that produces no units, 1
/// without and 2 with a rally point, then the cell).
pub(crate) fn hash_building(
    hasher: &mut Fnv1a64,
    building: &Building,
    producer: Option<&Producer>,
) {
    hasher.write(&building.kind.0.to_le_bytes());
    hasher.write(&building.footprint.origin.x.to_le_bytes());
    hasher.write(&building.footprint.origin.y.to_le_bytes());
    hasher.write(&[building.footprint.width, building.footprint.height]);
    hasher.write_u64(building.pending_units.len() as u64);
    for kind in &building.pending_units {
        hasher.write(&kind.0.to_le_bytes());
    }
    match producer {
        None => hasher.write(&[0]),
        Some(Producer { rally: None }) => hasher.write(&[1]),
        Some(Producer { rally: Some(cell) }) => {
            hasher.write(&[2]);
            hasher.write(&cell.x.to_le_bytes());
            hasher.write(&cell.y.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footprint(x: i32, y: i32, width: u8, height: u8) -> Footprint {
        Footprint {
            origin: CellPos::new(x, y),
            width,
            height,
        }
    }

    #[test]
    fn footprint_cells_ring_and_distance() {
        let a = footprint(2, 3, 2, 1);
        assert_eq!(
            a.cells().collect::<Vec<_>>(),
            [CellPos::new(2, 3), CellPos::new(3, 3)]
        );
        let ring: Vec<CellPos> = a.ring().collect();
        assert_eq!(ring.len(), 10);
        assert!(ring.iter().all(|&cell| a.distance_to_cell(cell) == 1));
        assert!(a.contains(CellPos::new(3, 3)) && !a.contains(CellPos::new(4, 3)));
        // Chebyshev distance between the nearest cells.
        assert_eq!(a.distance(&a), 0);
        assert_eq!(a.distance(&footprint(4, 3, 1, 1)), 1);
        assert_eq!(a.distance(&footprint(4, 4, 1, 1)), 1, "a corner");
        assert_eq!(a.distance(&footprint(6, 0, 3, 3)), 3);
        assert_eq!(footprint(6, 0, 3, 3).distance(&a), 3, "symmetric");
        assert_eq!(a.distance(&footprint(-10, 3, 1, 1)), 12);
    }

    #[test]
    fn footprints_at_the_extremes_of_i32_do_not_overflow() {
        let grid = Grid::default();
        for (x, y) in [
            (i32::MAX, i32::MAX),
            (i32::MIN, i32::MIN),
            (i32::MAX, 0),
            (0, i32::MIN),
        ] {
            let far = footprint(x, y, 8, 8);
            assert!(!far.inside(&grid));
            assert!(far.cells().count() <= 64);
            assert!(far.ring().count() <= 36);
            assert!(far.distance(&footprint(0, 0, 1, 1)) > 0);
        }
        assert_eq!(
            footprint(i32::MIN, i32::MIN, 1, 1).distance(&footprint(i32::MAX, i32::MAX, 1, 1)),
            u32::MAX
        );
    }
}
