use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::building::PlacementError;
use crate::combat::{AttackOrder, Body, Weapon};
use crate::definitions::Definitions;
use crate::economy_commands::{self, BuildingIndex, EconomyParams};
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::event::{MoveFailure, SimEvent};
use crate::harvest::{HarvestState, Harvester, claimed_ore, nearest_ore};
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::pathfinding::{destination_cells, octile_distance};
use crate::schedule::SimTick;
use crate::unit::{Mobile, MoveOrder, Orders, Owner, Unit};
use crate::visibility::{NO_VISION, Vision};

/// Identifies a player (human or AI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u8);

/// Largest number of units one command may name.
pub const MAX_UNITS_PER_COMMAND: usize = 1024;

/// An order issued by a player (PLAN.md section 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Move the units to `target`. A group receives distinct destination cells around the
    /// target (docs/DECISIONS.md D19). Harvesters among them stop their cycle.
    Move { units: Vec<SimId>, target: CellPos },
    /// Cancel the units' orders; each stops at its next cell centre. Harvesters among them stop
    /// their cycle.
    Stop { units: Vec<SimId> },
    /// Start or resume the harvesting cycle of the harvesters, each at the cell with ore
    /// nearest to `target` that no other harvester has taken (docs/DECISIONS.md D26).
    Harvest { units: Vec<SimId>, target: CellPos },
    /// Append `item` to the player's queue of its producing building type (docs/DECISIONS.md
    /// D25).
    QueueProduction { item: ProductionItem },
    /// Remove the most recently queued `item` from its queue and refund what was charged for it.
    CancelProduction { item: ProductionItem },
    /// Place the player's ready building of type `building` with its footprint's origin (the
    /// cell with the smallest coordinates) at `origin` (docs/DECISIONS.md D24).
    PlaceBuilding {
        building: BuildingKind,
        origin: CellPos,
    },
    /// Sell the building: remove it, free its cells, and refund its sell value.
    SellBuilding { building: SimId },
    /// Units produced by the building move to `cell` when they leave it.
    SetRallyPoint { building: SimId, cell: CellPos },
    /// The armed units attack `target`, a unit or building of another player that the issuing
    /// player sees, moving into range if needed, until it is destroyed or no longer visible
    /// (docs/DECISIONS.md D29). Harvesters among them stop their cycle.
    Attack { units: Vec<SimId>, target: SimId },
    /// Move the units to `target` as `Move` does; the armed ones stop to fight visible enemies in
    /// range on the way and then move on (docs/DECISIONS.md D29). Harvesters among them stop
    /// their cycle.
    AttackMove { units: Vec<SimId>, target: CellPos },
}

impl Command {
    /// The units a unit command names; `None` for the economy commands.
    fn units(&self) -> Option<&[SimId]> {
        match self {
            Command::Move { units, .. }
            | Command::Stop { units }
            | Command::Harvest { units, .. }
            | Command::Attack { units, .. }
            | Command::AttackMove { units, .. } => Some(units),
            Command::QueueProduction { .. }
            | Command::CancelProduction { .. }
            | Command::PlaceBuilding { .. }
            | Command::SellBuilding { .. }
            | Command::SetRallyPoint { .. } => None,
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
    /// The unit is not a harvester (`Harvest`).
    NotAHarvester(SimId),
    /// The issuing player is not a player of the match (economy commands).
    UnknownPlayer(PlayerId),
    /// The item is not defined, or the economy file gives it no production terms.
    UnknownItem(ProductionItem),
    /// The roster of the player's faction does not offer `item` (M7).
    NotInRoster(ProductionItem),
    /// The player owns no building of type `prerequisite`, which `item` requires.
    MissingPrerequisite {
        item: ProductionItem,
        prerequisite: BuildingKind,
    },
    /// The player owns no building of type `producer`, which produces `item`.
    NoProducer {
        item: ProductionItem,
        producer: BuildingKind,
    },
    /// The queue of `producer` already holds `max` items.
    QueueFull {
        producer: BuildingKind,
        max: usize,
    },
    /// No such item is in the player's queues (`CancelProduction`).
    NotQueued(ProductionItem),
    /// The player has no complete building of this type at the head of its queue
    /// (`PlaceBuilding`).
    NotReady(BuildingKind),
    /// The building may not be placed there.
    Placement(PlacementError),
    /// No simulation id is left for the new building.
    IdsExhausted,
    /// No building with this id exists.
    UnknownBuilding(SimId),
    /// The building's type produces no units, so it has no rally point.
    NotAProducer(SimId),
    /// The unit has no weapon (`Attack`).
    NotArmed(SimId),
    /// The target belongs to the issuing player (`Attack`).
    OwnTarget(SimId),
    /// No unit or building with this id is visible to the issuing player (`Attack`): it does not
    /// exist (any more), or it is an enemy the player does not see. The two cases are reported
    /// alike, so that commands reveal nothing about unseen enemies (docs/DECISIONS.md D30).
    UnknownTarget(SimId),
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
            Self::NotAHarvester(id) => write!(f, "unit {} is not a harvester", id.0),
            Self::UnknownPlayer(player) => {
                write!(f, "player {} is not a player of the match", player.0)
            }
            Self::UnknownItem(item) => write!(f, "{item:?} cannot be produced"),
            Self::NotInRoster(item) => {
                write!(f, "{item:?} is not in the roster of the player's faction")
            }
            Self::MissingPrerequisite { item, prerequisite } => {
                write!(f, "{item:?} requires a building of type {}", prerequisite.0)
            }
            Self::NoProducer { item, producer } => write!(
                f,
                "{item:?} is produced by buildings of type {}, and the player owns none",
                producer.0
            ),
            Self::QueueFull { producer, max } => write!(
                f,
                "the queue of building type {} already holds {max} items",
                producer.0
            ),
            Self::NotQueued(item) => write!(f, "{item:?} is not queued"),
            Self::NotReady(kind) => write!(f, "no building of type {} is ready", kind.0),
            Self::Placement(error) => error.fmt(f),
            Self::IdsExhausted => write!(f, "no simulation id is left"),
            Self::UnknownBuilding(id) => write!(f, "building {} does not exist", id.0),
            Self::NotAProducer(id) => write!(f, "building {} produces no units", id.0),
            Self::NotArmed(id) => write!(f, "unit {} has no weapon", id.0),
            Self::OwnTarget(id) => write!(f, "{} belongs to the player", id.0),
            Self::UnknownTarget(id) => {
                write!(f, "no unit or building {} is visible to the player", id.0)
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

    /// The pending commands in push order. After a tick these are the commands the computer
    /// players pushed in its `Ai` stage (M6), since the `Commands` stage drained the queue first.
    pub fn iter(&self) -> impl Iterator<Item = &IssuedCommand> {
        self.pending.iter()
    }
}

type UnitQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static Unit,
        &'static Mobile,
        &'static mut Orders,
        Option<&'static mut Harvester>,
        Option<&'static mut Weapon>,
    ),
    With<Unit>,
>;

/// Drains the queue in push order, validating and applying each command. An invalid command
/// produces [`SimEvent::CommandRejected`] and changes no state.
///
/// Buildings placed or sold by a command are spawned or despawned when the stage ends (Bevy
/// applies the deferred entity changes at the sync point before the `Economy` stage), but the
/// grid, credits, and queues change at once, and later commands of the same tick see the
/// change (docs/DECISIONS.md D25).
pub(crate) fn process_commands(
    mut queue: ResMut<CommandQueue>,
    mut grid: ResMut<Grid>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    mut units: UnitQuery,
    mut economy: EconomyParams,
) {
    if queue.pending.is_empty() {
        return;
    }
    let index: BTreeMap<SimId, Entity> =
        units.iter().map(|(entity, id, ..)| (*id, entity)).collect();
    let mut buildings = None;
    for issued in queue.pending.drain(..) {
        let result = if issued.command.units().is_some() {
            let buildings = buildings.get_or_insert_with(|| economy.building_index());
            validate_player(&issued, &economy)
                .and_then(|()| validate(&issued, &grid, &economy, &index, &units))
                .and_then(|()| validate_combat(&issued, &economy, buildings, &index, &units))
                .map(|()| {
                    apply_unit_command(
                        &issued.command,
                        tick.0,
                        &grid,
                        economy.definitions(),
                        &index,
                        &mut units,
                        &mut events,
                    );
                })
        } else {
            let buildings = buildings.get_or_insert_with(|| economy.building_index());
            economy_commands::apply(&issued, &mut economy, buildings, &mut grid, &mut events)
        };
        if let Err(reason) = result {
            events.write(SimEvent::CommandRejected {
                player: issued.player,
                command: issued.command,
                reason,
            });
        }
    }
}

/// Applies a valid unit command.
fn apply_unit_command(
    command: &Command,
    tick: u64,
    grid: &Grid,
    definitions: &Definitions,
    index: &BTreeMap<SimId, Entity>,
    units: &mut UnitQuery,
    events: &mut Messages<SimEvent>,
) {
    match command {
        Command::Move { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            apply_move(ids, *target, tick, grid, index, units, events);
        }
        Command::Stop { units: ids } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for id in ids {
                if let Some(&entity) = index.get(id)
                    && let Ok((.., mut orders, _, _)) = units.get_mut(entity)
                {
                    orders.current = None;
                }
            }
        }
        Command::Harvest { units: ids, target } => {
            stop_fighting(ids, index, units);
            apply_harvest(ids, *target, tick, grid, definitions, index, units);
        }
        Command::Attack { units: ids, target } => {
            stop_harvesting(ids, index, units);
            for id in ids {
                if let Some(&entity) = index.get(id)
                    && let Ok((.., mut orders, _, Some(mut weapon))) = units.get_mut(entity)
                {
                    // The `Combat` stage of this tick moves the unit into range or fires.
                    orders.current = None;
                    weapon.target = Some(*target);
                    weapon.order = Some(AttackOrder::Attack {
                        target: *target,
                        chases: 0,
                    });
                }
            }
        }
        Command::AttackMove { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for (entity, destination) in apply_move(ids, *target, tick, grid, index, units, events)
            {
                if let Ok((.., Some(mut weapon))) = units.get_mut(entity) {
                    weapon.order = Some(AttackOrder::AttackMove {
                        destination,
                        moving: true,
                    });
                }
            }
        }
        Command::QueueProduction { .. }
        | Command::CancelProduction { .. }
        | Command::PlaceBuilding { .. }
        | Command::SellBuilding { .. }
        | Command::SetRallyPoint { .. } => {}
    }
}

/// `Move`, `Stop`, `Attack`, and `AttackMove` end the harvesting cycle of the harvesters they
/// name.
fn stop_harvesting(ids: &[SimId], index: &BTreeMap<SimId, Entity>, units: &mut UnitQuery) {
    for id in ids {
        if let Some(&entity) = index.get(id)
            && let Ok((.., Some(mut harvester), _)) = units.get_mut(entity)
        {
            harvester.stop();
        }
    }
}

/// `Move`, `Stop`, `Harvest`, and `AttackMove` end the combat orders of the units they name, and
/// the units forget their targets.
fn stop_fighting(ids: &[SimId], index: &BTreeMap<SimId, Entity>, units: &mut UnitQuery) {
    for id in ids {
        if let Some(&entity) = index.get(id)
            && let Ok((.., Some(mut weapon))) = units.get_mut(entity)
        {
            weapon.clear();
        }
    }
}

/// Starts the cycle of each harvester, in id order, at the cell with ore nearest to `target`
/// that is not taken by another harvester (docs/DECISIONS.md D26). A harvester for which no
/// such cell is reachable from the target starts looking from where it stands, and a full one
/// goes to a refinery first.
fn apply_harvest(
    ids: &[SimId],
    target: CellPos,
    tick: u64,
    grid: &Grid,
    definitions: &Definitions,
    index: &BTreeMap<SimId, Entity>,
    units: &mut UnitQuery,
) {
    let mut sorted: Vec<SimId> = ids.to_vec();
    sorted.sort_unstable();
    let group: BTreeSet<SimId> = sorted.iter().copied().collect();
    let mut claimed = claimed_ore(
        units.iter().filter_map(|(_, id, .., harvester, _)| {
            harvester
                .filter(|_| !group.contains(id))
                .map(|harvester| (*id, harvester))
        }),
        None,
    );
    for id in sorted {
        let Some(&entity) = index.get(&id) else {
            continue;
        };
        let Ok((_, _, _, unit, mobile, mut orders, Some(mut harvester), _)) = units.get_mut(entity)
        else {
            continue;
        };
        harvester.attempts = 0;
        let full = definitions
            .harvester(unit.kind)
            .is_some_and(|terms| harvester.load >= terms.capacity);
        if full {
            harvester.state = HarvestState::Seeking;
            orders.current = None;
            continue;
        }
        match nearest_ore(grid, target, id, &claimed) {
            Some(cell) => {
                claimed.insert(cell);
                harvester.state = HarvestState::ToOre(cell);
                orders.current = (cell != mobile.next_centre()).then(|| MoveOrder::new(cell, tick));
            }
            None => {
                harvester.state = HarvestState::Seeking;
                orders.current = None;
            }
        }
    }
}

/// Ownership, existence, and bounds of a unit command. Under fog of war a named unit of another
/// player that the issuing player does not see is `UnknownUnit` rather than `NotOwned`. The
/// target of `Attack` is checked by [`validate_combat`] (docs/DECISIONS.md D30).
fn validate(
    issued: &IssuedCommand,
    grid: &Grid,
    economy: &EconomyParams,
    index: &BTreeMap<SimId, Entity>,
    units: &UnitQuery,
) -> Result<(), RejectReason> {
    let Some(ids) = issued.command.units() else {
        return Ok(());
    };
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
        let (owner, mobile, harvester) = index
            .get(&id)
            .and_then(|&entity| units.get(entity).ok())
            .map(|(_, _, owner, _, mobile, _, harvester, _)| {
                (owner.0, *mobile, harvester.is_some())
            })
            .ok_or(RejectReason::UnknownUnit(id))?;
        if owner != issued.player {
            // Under fog of war, another player's unit the player does not see is reported like
            // one that does not exist (docs/DECISIONS.md D30).
            let hidden = player_fog(issued.player, economy).is_some_and(|vision| {
                !Body::of(Some(&mobile), None).is_some_and(|body| body.seen_by(vision))
            });
            return Err(if hidden {
                RejectReason::UnknownUnit(id)
            } else {
                RejectReason::NotOwned(id)
            });
        }
        if matches!(issued.command, Command::Harvest { .. }) && !harvester {
            return Err(RejectReason::NotAHarvester(id));
        }
    }
    if let Command::Move { target, .. }
    | Command::Harvest { target, .. }
    | Command::AttackMove { target, .. } = issued.command
        && !grid.contains(target)
    {
        return Err(RejectReason::OutsideMap(target));
    }
    Ok(())
}

/// The issuing player's vision while fog of war is active (the definitions include a combat
/// file); a player who is not a player of the match then sees nothing. `None` without fog.
fn player_fog<'a>(player: PlayerId, economy: &'a EconomyParams) -> Option<&'a Vision> {
    economy.definitions().has_combat().then(|| {
        economy
            .players()
            .get(player)
            .map_or(&NO_VISION, |state| state.vision())
    })
}

/// `Attack` and `AttackMove` are issued only by a player of the match (`UnknownPlayer`), checked
/// first as for the economy commands: only players have a visibility grid (docs/DECISIONS.md
/// D30). `Move`, `Stop`, and `Harvest` keep the M2 rule that does not require it.
fn validate_player(issued: &IssuedCommand, economy: &EconomyParams) -> Result<(), RejectReason> {
    match issued.command {
        Command::Attack { .. } | Command::AttackMove { .. }
            if economy.players().get(issued.player).is_none() =>
        {
            Err(RejectReason::UnknownPlayer(issued.player))
        }
        _ => Ok(()),
    }
}

/// The checks of `Attack` beyond [`validate`], in this order (docs/DECISIONS.md D29, D30): every
/// named unit has a weapon (`NotArmed`), and the target is not the player's own (`OwnTarget`),
/// and exists and is visible to the player (`UnknownTarget`, the same reason for both, so that an
/// unseen enemy is reported like a missing one). Every unit and building has hit points whenever
/// any unit has a weapon (a combat file gives every type combat terms, docs/DECISIONS.md D28
/// item 4), so no check for them is needed (M4 review 1 P2-2, D32).
fn validate_combat(
    issued: &IssuedCommand,
    economy: &EconomyParams,
    buildings: &BuildingIndex,
    index: &BTreeMap<SimId, Entity>,
    units: &UnitQuery,
) -> Result<(), RejectReason> {
    let Command::Attack { units: ids, target } = &issued.command else {
        return Ok(());
    };
    let target = *target;
    let vision = economy
        .players()
        .get(issued.player)
        .ok_or(RejectReason::UnknownPlayer(issued.player))?
        .vision();
    for &id in ids {
        let armed = index
            .get(&id)
            .and_then(|&entity| units.get(entity).ok())
            .is_some_and(|(.., weapon)| weapon.is_some());
        if !armed {
            return Err(RejectReason::NotArmed(id));
        }
    }
    // The target as the player may know it: owner and body.
    let found = index
        .get(&target)
        .and_then(|&entity| units.get(entity).ok())
        .map(|(_, _, owner, _, mobile, ..)| (owner.0, Body::of(Some(mobile), None)))
        .or_else(|| {
            buildings
                .get(target)
                .map(|(owner, _, footprint)| (owner, Some(Body::Building(footprint))))
        });
    match found {
        Some((owner, _)) if owner == issued.player => Err(RejectReason::OwnTarget(target)),
        Some((_, Some(body))) if body.seen_by(vision) => Ok(()),
        _ => Err(RejectReason::UnknownTarget(target)),
    }
}

/// Gives every unit of a valid move command its own destination cell (docs/DECISIONS.md D19).
///
/// The cells are the free cells nearest to the target by path cost over passable terrain
/// ([`destination_cells`]); cells held by idle units outside the group are not handed out. The
/// units, ordered by their distance to the target (then by id), receive the cells in that
/// order, so the units nearest to the target take the cells nearest to it. A unit for which no
/// cell remains stops, with [`MoveFailure::NoDestination`]. A unit that later finds its cell
/// closed off by units that arrived first takes the nearest cell it can reach instead
/// (`pathfinding::find_paths`). Returns each unit that got a destination, with its cell.
fn apply_move(
    ids: &[SimId],
    target: CellPos,
    tick: u64,
    grid: &Grid,
    index: &BTreeMap<SimId, Entity>,
    units: &mut UnitQuery,
    events: &mut Messages<SimEvent>,
) -> Vec<(Entity, CellPos)> {
    let group: BTreeSet<SimId> = ids.iter().copied().collect();
    let idle_outside_group: BTreeSet<SimId> = units
        .iter()
        .filter(|(_, id, _, _, mobile, orders, ..)| {
            !group.contains(id) && mobile.step.is_none() && orders.current.is_none()
        })
        .map(|(_, id, ..)| *id)
        .collect();
    let mut order: Vec<(u32, SimId, Entity)> = ids
        .iter()
        .filter_map(|id| {
            let entity = *index.get(id)?;
            let (_, _, _, _, mobile, ..) = units.get(entity).ok()?;
            Some((octile_distance(mobile.next_centre(), target), *id, entity))
        })
        .collect();
    order.sort_unstable();
    let destinations = destination_cells(grid, target, order.len(), |cell| {
        grid.occupant(cell)
            .is_some_and(|holder| idle_outside_group.contains(&holder))
    });
    let mut assigned = Vec::with_capacity(order.len());
    for (position, (_, id, entity)) in order.into_iter().enumerate() {
        let Ok((.., mut orders, _, _)) = units.get_mut(entity) else {
            continue;
        };
        match destinations.get(position) {
            Some(&destination) => {
                orders.current = Some(MoveOrder::new(destination, tick));
                assigned.push((entity, destination));
            }
            None => {
                orders.current = None;
                events.write(SimEvent::MoveFailed {
                    unit: id,
                    reason: MoveFailure::NoDestination,
                });
            }
        }
    }
    assigned
}
