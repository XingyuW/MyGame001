use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::event::{MoveFailure, SimEvent};
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::pathfinding::{destination_cells, octile_distance};
use crate::schedule::SimTick;
use crate::unit::{Mobile, MoveOrder, Orders, Owner, Unit};

/// Identifies a player (human or AI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u8);

/// Largest number of units one command may name.
pub const MAX_UNITS_PER_COMMAND: usize = 1024;

/// An order issued by a player (PLAN.md section 5). The remaining variants are added by the
/// milestones that implement them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Move the units to `target`. A group receives distinct destination cells around the
    /// target (docs/DECISIONS.md D19).
    Move { units: Vec<SimId>, target: CellPos },
    /// Cancel the units' orders; each stops at its next cell centre.
    Stop { units: Vec<SimId> },
}

impl Command {
    fn units(&self) -> &[SimId] {
        match self {
            Command::Move { units, .. } | Command::Stop { units } => units,
        }
    }
}

/// Why a command was rejected. A rejected command changes no state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectReason {
    NoUnits,
    TooManyUnits {
        count: usize,
        max: usize,
    },
    /// The unit is named more than once.
    DuplicateUnit(SimId),
    /// No unit with this id exists.
    UnknownUnit(SimId),
    /// The unit belongs to another player.
    NotOwned(SimId),
    /// The target cell is outside the map.
    OutsideMap(CellPos),
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUnits => write!(f, "the command names no unit"),
            Self::TooManyUnits { count, max } => {
                write!(
                    f,
                    "the command names {count} units, at most {max} are allowed"
                )
            }
            Self::DuplicateUnit(id) => write!(f, "unit {} is named more than once", id.0),
            Self::UnknownUnit(id) => write!(f, "unit {} does not exist", id.0),
            Self::NotOwned(id) => write!(f, "unit {} belongs to another player", id.0),
            Self::OutsideMap(cell) => {
                write!(f, "cell ({}, {}) is outside the map", cell.x, cell.y)
            }
        }
    }
}

/// A [`Command`] together with the player who issued it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedCommand {
    pub player: PlayerId,
    pub command: Command,
}

/// Pending commands, consumed in push order at the start of the next tick.
///
/// This is the only way for the interface, the AI, and tests to change simulation state
/// (invariant I5).
#[derive(Resource, Debug, Default)]
pub struct CommandQueue {
    pending: Vec<IssuedCommand>,
}

impl CommandQueue {
    pub fn push(&mut self, issued: IssuedCommand) {
        self.pending.push(issued);
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

type UnitQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static Mobile,
        &'static mut Orders,
    ),
    With<Unit>,
>;

/// Drains the queue in push order, validating and applying each command. An invalid command
/// produces [`SimEvent::CommandRejected`] and changes no state.
pub(crate) fn process_commands(
    mut queue: ResMut<CommandQueue>,
    grid: Res<Grid>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    mut units: UnitQuery,
) {
    if queue.pending.is_empty() {
        return;
    }
    let index: BTreeMap<SimId, Entity> =
        units.iter().map(|(entity, id, ..)| (*id, entity)).collect();
    for issued in queue.pending.drain(..) {
        if let Err(reason) = validate(&issued, &grid, &index, &units) {
            events.write(SimEvent::CommandRejected {
                player: issued.player,
                command: issued.command,
                reason,
            });
            continue;
        }
        match issued.command {
            Command::Move { units: ids, target } => {
                apply_move(&ids, target, tick.0, &grid, &index, &mut units, &mut events);
            }
            Command::Stop { units: ids } => {
                for id in ids {
                    if let Some(&entity) = index.get(&id)
                        && let Ok((.., mut orders)) = units.get_mut(entity)
                    {
                        orders.current = None;
                    }
                }
            }
        }
    }
}

/// Ownership, existence, and bounds. Visibility is not checked yet: there is no fog of war
/// before M4, so every cell is visible to every player (docs/DECISIONS.md D19).
fn validate(
    issued: &IssuedCommand,
    grid: &Grid,
    index: &BTreeMap<SimId, Entity>,
    units: &UnitQuery,
) -> Result<(), RejectReason> {
    let ids = issued.command.units();
    if ids.is_empty() {
        return Err(RejectReason::NoUnits);
    }
    if ids.len() > MAX_UNITS_PER_COMMAND {
        return Err(RejectReason::TooManyUnits {
            count: ids.len(),
            max: MAX_UNITS_PER_COMMAND,
        });
    }
    let mut seen = BTreeSet::new();
    for &id in ids {
        if !seen.insert(id) {
            return Err(RejectReason::DuplicateUnit(id));
        }
        let owner = index
            .get(&id)
            .and_then(|&entity| units.get(entity).ok())
            .map(|(_, _, owner, ..)| owner.0)
            .ok_or(RejectReason::UnknownUnit(id))?;
        if owner != issued.player {
            return Err(RejectReason::NotOwned(id));
        }
    }
    if let Command::Move { target, .. } = issued.command
        && !grid.contains(target)
    {
        return Err(RejectReason::OutsideMap(target));
    }
    Ok(())
}

/// Gives every unit of a valid move command its own destination cell (docs/DECISIONS.md D19).
///
/// The cells are the free cells nearest to the target by path cost over passable terrain
/// ([`destination_cells`]); cells held by idle units outside the group are not handed out. The
/// units, ordered by their distance to the target (then by id), receive the cells in that
/// order, so the units nearest to the target take the cells nearest to it. A unit for which no
/// cell remains stops, with [`MoveFailure::NoDestination`]. A unit that later finds its cell
/// closed off by units that arrived first takes the nearest cell it can reach instead
/// (`pathfinding::find_paths`).
fn apply_move(
    ids: &[SimId],
    target: CellPos,
    tick: u64,
    grid: &Grid,
    index: &BTreeMap<SimId, Entity>,
    units: &mut UnitQuery,
    events: &mut Messages<SimEvent>,
) {
    let group: BTreeSet<SimId> = ids.iter().copied().collect();
    let idle_outside_group: BTreeSet<SimId> = units
        .iter()
        .filter(|(_, id, _, mobile, orders)| {
            !group.contains(id) && mobile.step.is_none() && orders.current.is_none()
        })
        .map(|(_, id, ..)| *id)
        .collect();
    let mut order: Vec<(u32, SimId, Entity)> = ids
        .iter()
        .filter_map(|id| {
            let entity = *index.get(id)?;
            let (_, _, _, mobile, _) = units.get(entity).ok()?;
            Some((octile_distance(mobile.next_centre(), target), *id, entity))
        })
        .collect();
    order.sort_unstable();
    let destinations = destination_cells(grid, target, order.len(), |cell| {
        grid.occupant(cell)
            .is_some_and(|holder| idle_outside_group.contains(&holder))
    });
    for (position, (_, id, entity)) in order.into_iter().enumerate() {
        let Ok((.., mut orders)) = units.get_mut(entity) else {
            continue;
        };
        match destinations.get(position) {
            Some(&destination) => orders.current = Some(MoveOrder::new(destination, tick)),
            None => {
                orders.current = None;
                events.write(SimEvent::MoveFailed {
                    unit: id,
                    reason: MoveFailure::NoDestination,
                });
            }
        }
    }
}
