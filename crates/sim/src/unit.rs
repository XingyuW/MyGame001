//! Units: their components, spawning, and their part of `state_hash` (PLAN.md sections 5 and 7,
//! M2).
//!
//! A unit always stands in one cell (`Mobile::cell`). While it moves to an adjacent cell it has
//! reserved that cell as well (`Mobile::step`), so the grid's occupant of both cells is the unit
//! (invariant I6). It releases the cell it left when it reaches the centre of the new one.
//!
//! Since RA3 (invariant I6', docs/DECISIONS.md R6) a unit on foot, which is an infantry unit
//! that does not fly ([`Definitions::on_foot`]), holds one of the three spots of its cell
//! instead of the cell, and of the cell it steps into; the grid records which
//! ([`Grid::spot`]). Its [`Position`] is that spot, or while it steps a point on the line from
//! its spot to the spot it has reserved; a step takes as many ticks as from centre to centre.

use std::collections::BTreeMap;
use std::fmt;

use bevy_ecs::component::Component;
use bevy_ecs::world::World;

use crate::combat::combat_components;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::harvest::Harvester;
use crate::hash::Fnv1a64;
use crate::id::{SimId, SimIdAllocator};
use crate::map::{CellPos, Grid, SUBCELLS_PER_CELL, SubPos};

/// Length of an orthogonal step between two cell centres, in subcells.
pub const ORTHOGONAL_STEP: u16 = 256;

/// Length of a diagonal step between two cell centres, in subcells: 256 × √2 rounded to the
/// nearest integer (362.04 → 362), so a diagonal step takes √2 times as long as an orthogonal one
/// to within 0.02 %.
pub const DIAGONAL_STEP: u16 = 362;

/// The player who owns an entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner(pub PlayerId);

/// The unit type of a unit.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
}

/// The unit's fine position: the centre of `Mobile::cell`, or a point on the straight line
/// from it to the centre of the step's target cell. For a unit on foot: the spot it holds in
/// `Mobile::cell`, or a point on the straight line from it to the spot it holds in the step's
/// target cell.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position(pub SubPos);

/// One of the eight directions between neighbouring cells, named by the cell axes: +x is east
/// and +y is south. On screen, east is down-right, south is down-left, and south-east is
/// straight down (PLAN.md section 5, equation 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
}

impl Direction {
    /// All directions in index order.
    pub const ALL: [Direction; 8] = [
        Direction::East,
        Direction::SouthEast,
        Direction::South,
        Direction::SouthWest,
        Direction::West,
        Direction::NorthWest,
        Direction::North,
        Direction::NorthEast,
    ];

    /// The cell offset (dx, dy) of a step in this direction.
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::East => (1, 0),
            Direction::SouthEast => (1, 1),
            Direction::South => (0, 1),
            Direction::SouthWest => (-1, 1),
            Direction::West => (-1, 0),
            Direction::NorthWest => (-1, -1),
            Direction::North => (0, -1),
            Direction::NorthEast => (1, -1),
        }
    }

    /// The direction of a step by (dx, dy), each component in {-1, 0, 1}; `None` for (0, 0) or
    /// anything that is not a step to a neighbour.
    pub fn from_delta(dx: i32, dy: i32) -> Option<Direction> {
        Direction::ALL
            .into_iter()
            .find(|direction| direction.delta() == (dx, dy))
    }

    /// Position of this direction in [`Direction::ALL`]; also the sprite facing index.
    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn is_diagonal(self) -> bool {
        let (dx, dy) = self.delta();
        dx != 0 && dy != 0
    }
}

/// The direction a unit faces: the direction of its last step (eight sprite facings).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facing(pub Direction);

/// A step in progress from `Mobile::cell` to the adjacent cell `to`, which the unit has
/// reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub to: CellPos,
    /// Subcells travelled, from 0 to `length`.
    pub progress: u16,
    /// [`ORTHOGONAL_STEP`] or [`DIAGONAL_STEP`].
    pub length: u16,
}

/// Movement state of a ground unit.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mobile {
    /// Subcells per tick.
    pub speed: u16,
    /// The cell the unit stands in; while stepping, the cell it is leaving.
    pub cell: CellPos,
    pub step: Option<Step>,
}

impl Mobile {
    /// The cell whose centre the unit reaches next: the step's target, or its own cell.
    pub fn next_centre(&self) -> CellPos {
        self.step.map_or(self.cell, |step| step.to)
    }

    /// The fine position for the current step progress (integer interpolation between the two
    /// centres; exact at both ends) of a unit that is not on foot.
    pub fn sub_position(&self) -> SubPos {
        let from = SubPos::centre_of(self.cell);
        let Some(step) = self.step else {
            return from;
        };
        let length = i32::from(step.length.max(1));
        let progress = i32::from(step.progress.min(step.length));
        // At most 256 × 362, far below i32::MAX. The step's target is a neighbour of the cell,
        // so each axis moves by at most one cell; saturation only guards against values that
        // the simulation never creates (invariant I12).
        let offset = SUBCELLS_PER_CELL.saturating_mul(progress) / length;
        let axis = |from: i32, to: i32| to.saturating_sub(from).signum().saturating_mul(offset);
        SubPos::new(
            from.x.saturating_add(axis(self.cell.x, step.to.x)),
            from.y.saturating_add(axis(self.cell.y, step.to.y)),
        )
    }

    /// The fine position of the unit `id` with this movement state: of a unit on foot the spot
    /// it holds in its cell, or while it steps the point on the line from there to the spot it
    /// holds in the step's target cell that has the step's share of the way behind it (integer
    /// interpolation, rounded toward the start; exact at both ends); of any other unit
    /// [`Mobile::sub_position`].
    pub fn position(&self, grid: &Grid, id: SimId) -> SubPos {
        let Some(spot) = grid.spot(self.cell, id) else {
            return self.sub_position();
        };
        let from = SubPos::spot_of(self.cell, spot);
        let Some(step) = self.step else {
            return from;
        };
        let to = grid.standing_point(step.to, id);
        let length = i32::from(step.length.max(1));
        let progress = i32::from(step.progress.min(step.length));
        // Two spots of neighbouring cells are at most 256 + 144 subcells apart on an axis, and
        // a step is at most 362 long: far below i32::MAX (invariant I12).
        let axis = |from: i32, to: i32| {
            from.saturating_add(to.saturating_sub(from).saturating_mul(progress) / length)
        };
        SubPos::new(axis(from.x, to.x), axis(from.y, to.y))
    }
}

/// The state of a move order's path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathState {
    /// Waiting for the pathfinder, which serves requests in order of `since` (the tick of the
    /// request), then of the unit's id. With `avoid_units`, cells held by units that are not
    /// advancing count as blocked. `queued` is set when the unit asked while it waited behind
    /// units that are moving on (docs/DECISIONS.md D22): a path that only confirms the way it
    /// already follows is then not a new path.
    Requested {
        since: u64,
        avoid_units: bool,
        queued: bool,
    },
    /// Following `MoveOrder::path`.
    Following,
}

/// A unit's current move order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveOrder {
    pub destination: CellPos,
    /// The cells still to enter, the next one last.
    pub path: Vec<CellPos>,
    pub state: PathState,
    /// Consecutive ticks the unit has waited at a cell centre because its next cell was held by
    /// another unit.
    pub blocked_ticks: u16,
    /// Times the destination was moved because idle units closed every way to it.
    pub retargets: u32,
    /// Cost of the rest of `path`, in the step costs of A* (10 orthogonal, 14 diagonal), while
    /// `path` leads to `destination`; `None` while the unit waits for a path or steps aside.
    pub remaining: Option<u32>,
    /// The least `remaining` cost with which the unit has started a step during this order
    /// (`u32::MAX` before its first step, and again after terrain changed under its path). A
    /// step that lowers it is real progress toward the destination (docs/DECISIONS.md D21).
    pub best: u32,
    /// Paths the unit has taken since its last real progress (a step that lowered `best`).
    pub paths_since_progress: u16,
    /// Stalled ticks since the unit last started a step along a path to its destination (a step
    /// aside does not count): ticks in which it waited at a cell centre for cells held by units
    /// that are not moving on. Ticks in a step, waiting for the pathfinder, or waiting behind
    /// units that are moving on (in a step, or queued behind one) are not stalled ticks
    /// (docs/DECISIONS.md D22).
    pub stalled_ticks: u16,
    /// Repaths since the order was given (for diagnostics and tests).
    pub repaths: u32,
    /// Whether the unit may drive over hostile infantry on its way (RA3, docs/DECISIONS.md R6):
    /// set for the move of a `Move` or `AttackMove` command to a unit of a type with the
    /// arsenal's `crusher` trait, and for no other order.
    pub crush: bool,
}

impl MoveOrder {
    pub(crate) fn new(destination: CellPos, tick: u64) -> Self {
        Self {
            destination,
            path: Vec::new(),
            state: PathState::Requested {
                since: tick,
                avoid_units: false,
                queued: false,
            },
            blocked_ticks: 0,
            retargets: 0,
            remaining: None,
            best: u32::MAX,
            paths_since_progress: 0,
            stalled_ticks: 0,
            repaths: 0,
            crush: false,
        }
    }

    /// The move of a `Move` or `AttackMove` command: as [`MoveOrder::new`], and the unit may
    /// drive over hostile infantry on its way if its type is a `crusher`.
    pub(crate) fn commanded(destination: CellPos, tick: u64, crusher: bool) -> Self {
        Self {
            crush: crusher,
            ..Self::new(destination, tick)
        }
    }
}

/// The order a unit is executing, if any.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Orders {
    pub current: Option<MoveOrder>,
}

/// Why a unit could not be spawned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnError {
    UnknownUnitType(String),
    OutsideMap(CellPos),
    Impassable(CellPos),
    Occupied { cell: CellPos, by: SimId },
    IdsExhausted,
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnitType(id) => write!(f, "unit type {id:?} is not defined"),
            Self::OutsideMap(cell) => write!(f, "cell ({}, {}) is outside the map", cell.x, cell.y),
            Self::Impassable(cell) => write!(f, "cell ({}, {}) is impassable", cell.x, cell.y),
            Self::Occupied { cell, by } => write!(
                f,
                "cell ({}, {}) is occupied by unit {}",
                cell.x, cell.y, by.0
            ),
            Self::IdsExhausted => write!(f, "no simulation id is left"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Spawns a unit of type `unit_type` owned by `owner`, standing idle in `cell` and facing
/// south-east (towards the viewer). A harvester starts its harvesting cycle at once
/// (`HarvestState::Seeking`). With a combat file, the unit gets its hit points, armour, sight,
/// and weapon (M4).
///
/// Used by scenario/match setup and carrier sortie launch. Purchased units are created by
/// the production queue (M3). The world must have been set up by
/// [`SimPlugin`](crate::SimPlugin), with the grid and definitions of the match installed.
pub fn spawn_unit(
    world: &mut World,
    owner: PlayerId,
    unit_type: &str,
    cell: CellPos,
) -> Result<SimId, SpawnError> {
    let (kind, speed, harvester, combat) = {
        let definitions = world.resource::<Definitions>();
        let kind = definitions
            .unit_kind(unit_type)
            .ok_or_else(|| SpawnError::UnknownUnitType(unit_type.to_owned()))?;
        let speed = definitions
            .unit(kind)
            .map(|def| def.speed)
            .ok_or_else(|| SpawnError::UnknownUnitType(unit_type.to_owned()))?;
        (
            kind,
            speed,
            definitions.harvester(kind).is_some(),
            definitions.unit_combat(kind),
        )
    };
    let domain = world.resource::<Definitions>().domain(kind);
    let on_foot = world.resource::<Definitions>().on_foot(kind);
    {
        let grid = world.resource::<Grid>();
        if !grid.contains(cell) {
            return Err(SpawnError::OutsideMap(cell));
        }
        if !grid.passable_for(cell, domain) {
            return Err(SpawnError::Impassable(cell));
        }
        // A unit on foot shares a cell with up to two others on foot (invariant I6').
        if grid.room_in(cell, domain, on_foot, |_| true) < Grid::room_needed(on_foot)
            && let Some(by) = grid.occupant_for(cell, domain)
        {
            return Err(SpawnError::Occupied { cell, by });
        }
    }
    let id = world
        .resource_mut::<SimIdAllocator>()
        .allocate()
        .ok_or(SpawnError::IdsExhausted)?;
    let position = {
        let mut grid = world.resource_mut::<Grid>();
        grid.register(id, domain, on_foot);
        grid.claim(cell, id);
        grid.standing_point(cell, id)
    };
    let mut entity = world.spawn(unit_bundle(id, owner, kind, speed, cell, position));
    if harvester {
        entity.insert(Harvester::new());
    }
    if let Some(terms) = combat {
        let (components, weapon) = combat_components(terms);
        entity.insert(components);
        if let Some(weapon) = weapon {
            entity.insert(weapon);
        }
    }
    Ok(id)
}

/// The components of a new unit standing idle in `cell` at `position` (where the grid has it
/// stand once it holds the cell, [`Grid::standing_point`]), facing south-east. A harvester also
/// gets a [`Harvester`] component.
pub(crate) fn unit_bundle(
    id: SimId,
    owner: PlayerId,
    kind: UnitKind,
    speed: u16,
    cell: CellPos,
    position: SubPos,
) -> (SimId, Owner, Unit, Mobile, Position, Facing, Orders) {
    (
        id,
        Owner(owner),
        Unit { kind },
        Mobile {
            speed,
            cell,
            step: None,
        },
        Position(position),
        Facing(Direction::SouthEast),
        Orders::default(),
    )
}

/// What the units of a world hold of a cell, by their own state: the units that are not on foot,
/// and the units on foot.
#[derive(Default)]
struct Held {
    whole: Vec<SimId>,
    foot: Vec<SimId>,
}

/// Invariant I6' (docs/DECISIONS.md R6) on the state of `world`, taken from the units' own
/// state and compared with the grid; the violations found, in words (none: it holds).
///
/// A unit holds the cell it stands in and, while it steps, the cell it enters. Checked:
///
/// * a surface cell is held by at most one unit that is not on foot, or by at most
///   [`SPOTS_PER_CELL`](crate::SPOTS_PER_CELL) units on foot, and never by both; a cell of the
///   air layer by at most one aircraft;
/// * no unit of the surface holds a cell covered by a building;
/// * the grid records every unit as a holder of the cells it holds, in the layer of its domain,
///   a unit on foot at a spot of its own, and knows whether it is on foot;
/// * every unit is where its cells put it: at the centre of its cell, a unit on foot at its
///   spot, or while it steps at the step's share of the way to the next.
///
/// [`stray_claims`] is the other half: the holders the grid records that hold nothing. The
/// soak (`examples/m7_soak.rs`) and the tests check both; they are no part of a tick.
pub fn cell_invariant_violations(world: &mut World) -> Vec<String> {
    let mut query = world.query::<(&SimId, &Unit, &Mobile, &Position)>();
    let mut units: Vec<(SimId, UnitKind, Mobile, SubPos)> = query
        .iter(world)
        .map(|(id, unit, mobile, position)| (*id, unit.kind, *mobile, position.0))
        .collect();
    units.sort_unstable_by_key(|(id, ..)| *id);
    let definitions = world.resource::<Definitions>();
    let grid = world.resource::<Grid>();
    let mut problems = Vec::new();
    let mut surface: BTreeMap<CellPos, Held> = BTreeMap::new();
    let mut air: BTreeMap<CellPos, Vec<SimId>> = BTreeMap::new();
    for &(id, kind, mobile, position) in &units {
        let domain = definitions.domain(kind);
        let flies = domain == crate::Domain::Air;
        let on_foot = definitions.on_foot(kind);
        if grid.is_on_foot(id) != on_foot || grid.domain_of(id) != domain {
            problems.push(format!(
                "I6': the grid takes unit {} for {:?} and on foot {}, its type is {domain:?} and \
                 on foot {on_foot}",
                id.0,
                grid.domain_of(id),
                grid.is_on_foot(id)
            ));
        }
        let entering = mobile
            .step
            .map(|step| step.to)
            .filter(|to| *to != mobile.cell);
        for cell in std::iter::once(mobile.cell).chain(entering) {
            if flies {
                air.entry(cell).or_default().push(id);
            } else {
                let held = surface.entry(cell).or_default();
                if on_foot {
                    held.foot.push(id);
                } else {
                    held.whole.push(id);
                }
                if grid.building(cell).is_some() {
                    problems.push(format!(
                        "I6': unit {} holds {cell:?}, which a building covers",
                        id.0
                    ));
                }
            }
            if !grid.holders_for(cell, domain).contains(&Some(id)) {
                problems.push(format!(
                    "I6': unit {} holds {cell:?}, but the grid does not record it there",
                    id.0
                ));
            }
            if (on_foot && !flies) != grid.spot(cell, id).is_some() {
                problems.push(format!(
                    "I6': unit {} (on foot: {on_foot}) and the spots of {cell:?} disagree",
                    id.0
                ));
            }
        }
        let expected = mobile.position(grid, id);
        if position != expected {
            problems.push(format!(
                "I6': unit {} is at {position:?}, its cells put it at {expected:?}",
                id.0
            ));
        }
    }
    let ids = |units: &[SimId]| {
        units
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (cell, held) in &surface {
        if held.whole.len() > 1 {
            problems.push(format!(
                "I6': {cell:?} is held by the vehicles {}",
                ids(&held.whole)
            ));
        }
        if held.foot.len() > crate::SPOTS_PER_CELL {
            problems.push(format!(
                "I6': {cell:?} is held by {} infantry: {}",
                held.foot.len(),
                ids(&held.foot)
            ));
        }
        if !held.whole.is_empty() && !held.foot.is_empty() {
            problems.push(format!(
                "I6': {cell:?} is held by the vehicle {} and the infantry {}",
                ids(&held.whole),
                ids(&held.foot)
            ));
        }
        let mut spots: Vec<u8> = held
            .foot
            .iter()
            .filter_map(|&id| grid.spot(*cell, id))
            .collect();
        spots.sort_unstable();
        spots.dedup();
        if spots.len() != held.foot.len() {
            problems.push(format!(
                "I6': the infantry {} do not each hold a spot of {cell:?}",
                ids(&held.foot)
            ));
        }
    }
    for (cell, aircraft) in &air {
        if aircraft.len() > 1 {
            problems.push(format!(
                "I6': the air above {cell:?} is held by the aircraft {}",
                ids(aircraft)
            ));
        }
    }
    problems
}

/// The holders the grid of `world` records that hold nothing by their own state: a unit that
/// does not exist, or one that neither stands in the cell nor enters it, in the surface layer
/// or the air layer. None while every claim is released when its unit moves on or is removed
/// (the other half of [`cell_invariant_violations`]; it looks at every cell of the map).
pub fn stray_claims(world: &mut World) -> Vec<String> {
    let mut query = world.query::<(&SimId, &Mobile)>();
    let units: BTreeMap<SimId, Mobile> = query
        .iter(world)
        .map(|(id, mobile)| (*id, *mobile))
        .collect();
    let grid = world.resource::<Grid>();
    let mut problems = Vec::new();
    for y in 0..i32::try_from(grid.height()).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(grid.width()).unwrap_or(i32::MAX) {
            let cell = CellPos::new(x, y);
            for layer in [crate::Domain::Land, crate::Domain::Air] {
                for holder in grid.holders_for(cell, layer).into_iter().flatten() {
                    let holds = units.get(&holder).is_some_and(|mobile| {
                        (grid.domain_of(holder) == crate::Domain::Air)
                            == (layer == crate::Domain::Air)
                            && (mobile.cell == cell
                                || mobile.step.is_some_and(|step| step.to == cell))
                    });
                    if !holds {
                        problems.push(format!(
                            "I6': the grid records unit {} in {cell:?} ({layer:?} layer), which it \
                             does not hold",
                            holder.0
                        ));
                    }
                }
            }
        }
    }
    problems
}

fn write_cell(hasher: &mut Fnv1a64, cell: CellPos) {
    hasher.write(&cell.x.to_le_bytes());
    hasher.write(&cell.y.to_le_bytes());
}

/// Appends a unit's components to the hash, in a fixed layout (docs/DECISIONS.md D4, extended in
/// M2): owner, kind, speed, cell, step (flag, target, progress, length), position, facing, and
/// the order (flag, destination, state (with its flags and tick), path length and cells, blocked
/// ticks, retargets, remaining cost (flag and value), best cost, paths since progress, stalled
/// ticks, repaths). Since RA3 the order's flag is 2 instead of 1 for an order that may crush
/// (`MoveOrder::crush`), so the hash of every other order is what it was; the spots of a unit
/// on foot are hashed with the grid, and its position here.
pub(crate) fn hash_unit(
    hasher: &mut Fnv1a64,
    (owner, unit, mobile, position, facing, orders): (
        &Owner,
        &Unit,
        &Mobile,
        &Position,
        &Facing,
        &Orders,
    ),
) {
    hasher.write(&[owner.0.0]);
    hasher.write(&unit.kind.0.to_le_bytes());
    hasher.write(&mobile.speed.to_le_bytes());
    write_cell(hasher, mobile.cell);
    match mobile.step {
        None => hasher.write(&[0]),
        Some(step) => {
            hasher.write(&[1]);
            write_cell(hasher, step.to);
            hasher.write(&step.progress.to_le_bytes());
            hasher.write(&step.length.to_le_bytes());
        }
    }
    hasher.write(&position.0.x.to_le_bytes());
    hasher.write(&position.0.y.to_le_bytes());
    hasher.write(&[facing.0.index()]);
    match &orders.current {
        None => hasher.write(&[0]),
        Some(order) => {
            hasher.write(&[if order.crush { 2 } else { 1 }]);
            write_cell(hasher, order.destination);
            match order.state {
                PathState::Requested {
                    since,
                    avoid_units,
                    queued,
                } => {
                    hasher.write(&[1, u8::from(avoid_units), u8::from(queued)]);
                    hasher.write_u64(since);
                }
                PathState::Following => hasher.write(&[2]),
            }
            hasher.write_u64(order.path.len() as u64);
            for &cell in &order.path {
                write_cell(hasher, cell);
            }
            hasher.write(&order.blocked_ticks.to_le_bytes());
            hasher.write_u32(order.retargets);
            match order.remaining {
                None => hasher.write(&[0]),
                Some(remaining) => {
                    hasher.write(&[1]);
                    hasher.write_u32(remaining);
                }
            }
            hasher.write_u32(order.best);
            hasher.write(&order.paths_since_progress.to_le_bytes());
            hasher.write(&order.stalled_ticks.to_le_bytes());
            hasher.write_u32(order.repaths);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Cell;

    /// RA3: a unit on foot is at its spot, and while it steps on the straight line to the spot
    /// it has reserved, exact at both ends; any other unit moves between centres as before.
    #[test]
    fn a_unit_on_foot_is_placed_between_its_spots() {
        let cell = Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 0,
        };
        let mut grid = Grid::from_cells(4, 4, vec![cell; 16]).expect("a grid");
        let (soldier, other, truck) = (SimId(1), SimId(2), SimId(3));
        grid.register(soldier, crate::Domain::Land, true);
        grid.register(other, crate::Domain::Land, true);
        grid.register(truck, crate::Domain::Land, false);
        let (from, to) = (CellPos::new(1, 1), CellPos::new(2, 2));
        assert!(grid.claim(to, other), "spot 0 of the next cell is taken");
        assert!(grid.claim(from, soldier));
        assert!(grid.claim_spot(to, soldier, Some(0)));
        assert_eq!(
            (grid.spot(from, soldier), grid.spot(to, soldier)),
            (Some(0), Some(1))
        );
        let mobile = |progress: u16| Mobile {
            speed: 16,
            cell: from,
            step: Some(Step {
                to,
                progress,
                length: DIAGONAL_STEP,
            }),
        };
        let standing = Mobile {
            speed: 16,
            cell: from,
            step: None,
        };
        let (a, b) = (SubPos::spot_of(from, 0), SubPos::spot_of(to, 1));
        assert_eq!(standing.position(&grid, soldier), a);
        assert_eq!(mobile(0).position(&grid, soldier), a);
        assert_eq!(mobile(DIAGONAL_STEP).position(&grid, soldier), b);
        // Half way, rounded toward the start: from (320, 320) by (400, 304) to (720, 624).
        assert_eq!((a, b), (SubPos::new(320, 320), SubPos::new(720, 624)));
        assert_eq!(mobile(181).position(&grid, soldier), SubPos::new(520, 472));
        assert_eq!(mobile(1).position(&grid, soldier), SubPos::new(321, 320));
        assert_eq!(mobile(361).position(&grid, soldier), SubPos::new(718, 623));
        // Beyond the step's length it stays at the end (invariant I12).
        assert_eq!(mobile(u16::MAX).position(&grid, soldier), b);
        // A unit that is not on foot: between the centres.
        assert_eq!(standing.position(&grid, truck), SubPos::centre_of(from));
        assert_eq!(
            mobile(181).position(&grid, truck),
            mobile(181).sub_position()
        );
        assert_eq!(mobile(181).sub_position(), SubPos::new(512, 512));
    }

    /// RA3: the hash of a unit covers whether its order may crush, and an order that may not
    /// is hashed as before (invariant I3; docs/DECISIONS.md R6).
    #[test]
    fn the_hash_covers_whether_an_order_may_crush() {
        let hash = |order: Option<MoveOrder>| {
            let mut hasher = Fnv1a64::new();
            hash_unit(
                &mut hasher,
                (
                    &Owner(PlayerId(0)),
                    &Unit { kind: UnitKind(3) },
                    &Mobile {
                        speed: 16,
                        cell: CellPos::new(1, 1),
                        step: None,
                    },
                    &Position(SubPos::centre_of(CellPos::new(1, 1))),
                    &Facing(Direction::East),
                    &Orders { current: order },
                ),
            );
            hasher.finish()
        };
        let plain = MoveOrder::new(CellPos::new(5, 5), 7);
        let commanded = MoveOrder::commanded(CellPos::new(5, 5), 7, false);
        let crushing = MoveOrder::commanded(CellPos::new(5, 5), 7, true);
        assert_eq!(plain, commanded);
        assert!(crushing.crush && !plain.crush);
        assert_eq!(hash(Some(plain.clone())), hash(Some(commanded)));
        assert_ne!(hash(Some(plain.clone())), hash(Some(crushing)));
        assert_ne!(hash(Some(plain)), hash(None));
    }
}
