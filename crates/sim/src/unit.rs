//! Units: their components, spawning, and their part of `state_hash` (PLAN.md sections 5 and 7,
//! M2).
//!
//! A unit always stands in one cell (`Mobile::cell`). While it moves to an adjacent cell it has
//! reserved that cell as well (`Mobile::step`), so the grid's occupant of both cells is the unit
//! (invariant I6). It releases the cell it left when it reaches the centre of the new one.

use std::fmt;

use bevy_ecs::component::Component;
use bevy_ecs::world::World;

use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
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
/// from it to the centre of the step's target cell.
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
    /// centres; exact at both ends).
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
/// south-east (towards the viewer).
///
/// Scenario and match setup only (tests, and the game before the first tick); during a match
/// units are created by production (M3). The world must have been set up by
/// [`SimPlugin`](crate::SimPlugin), with the grid and definitions of the match installed.
pub fn spawn_unit(
    world: &mut World,
    owner: PlayerId,
    unit_type: &str,
    cell: CellPos,
) -> Result<SimId, SpawnError> {
    let (kind, speed) = {
        let definitions = world.resource::<Definitions>();
        let kind = definitions
            .unit_kind(unit_type)
            .ok_or_else(|| SpawnError::UnknownUnitType(unit_type.to_owned()))?;
        let speed = definitions.unit(kind).map_or(0, |def| def.speed);
        (kind, speed)
    };
    {
        let grid = world.resource::<Grid>();
        if !grid.contains(cell) {
            return Err(SpawnError::OutsideMap(cell));
        }
        if !grid.is_passable(cell) {
            return Err(SpawnError::Impassable(cell));
        }
        if let Some(by) = grid.occupant(cell) {
            return Err(SpawnError::Occupied { cell, by });
        }
    }
    let id = world
        .resource_mut::<SimIdAllocator>()
        .allocate()
        .ok_or(SpawnError::IdsExhausted)?;
    world.resource_mut::<Grid>().claim(cell, id);
    world.spawn((
        id,
        Owner(owner),
        Unit { kind },
        Mobile {
            speed,
            cell,
            step: None,
        },
        Position(SubPos::centre_of(cell)),
        Facing(Direction::SouthEast),
        Orders::default(),
    ));
    Ok(id)
}

fn write_cell(hasher: &mut Fnv1a64, cell: CellPos) {
    hasher.write(&cell.x.to_le_bytes());
    hasher.write(&cell.y.to_le_bytes());
}

/// Appends a unit's components to the hash, in a fixed layout (docs/DECISIONS.md D4, extended in
/// M2): owner, kind, speed, cell, step (flag, target, progress, length), position, facing, and
/// the order (flag, destination, state (with its flags and tick), path length and cells, blocked
/// ticks, retargets, remaining cost (flag and value), best cost, paths since progress, stalled
/// ticks, repaths).
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
            hasher.write(&[1]);
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
