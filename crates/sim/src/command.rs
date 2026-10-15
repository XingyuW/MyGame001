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

/// A forced shot may name a known object (including an ally) or a ground cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForceTarget {
    Cell(CellPos),
    Entity(SimId),
}

/// An order issued by a player (PLAN.md section 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Guard {
        units: Vec<SimId>,
    },
    Scatter {
        units: Vec<SimId>,
    },
    Follow {
        units: Vec<SimId>,
        target: SimId,
    },
    ForceFire {
        units: Vec<SimId>,
        target: ForceTarget,
    },
    ForceMove {
        units: Vec<SimId>,
        target: CellPos,
    },
    QueueMove {
        units: Vec<SimId>,
        target: CellPos,
    },
    Deploy {
        unit: SimId,
    },
    /// Turn the player's Construction Yard back into the base unit of its side, standing in the
    /// cell of the yard's footprint origin (RA1, the lobby's "MCV repacks"; `crate::special`).
    /// Rejected with `DisabledByOptions` in a match that has the option switched off.
    Undeploy {
        building: SimId,
    },
    Interact {
        unit: SimId,
        target: SimId,
    },
    Chronoshift {
        building: SimId,
        source: CellPos,
        target: CellPos,
    },
    Superweapon {
        building: SimId,
        target: CellPos,
        units: Vec<SimId>,
    },
    /// Move the units to `target`. A group receives distinct destination cells around the
    /// target (docs/DECISIONS.md D19). Harvesters among them stop their cycle.
    Move {
        units: Vec<SimId>,
        target: CellPos,
    },
    /// Cancel the units' orders; each stops at its next cell centre. Harvesters among them stop
    /// their cycle.
    Stop {
        units: Vec<SimId>,
    },
    /// Start or resume the harvesting cycle of the harvesters, each at the cell with ore
    /// nearest to `target` that no other harvester has taken (docs/DECISIONS.md D26).
    Harvest {
        units: Vec<SimId>,
        target: CellPos,
    },
    /// Append `item` to the player's queue of its producing building type (docs/DECISIONS.md
    /// D25).
    QueueProduction {
        item: ProductionItem,
    },
    /// Remove the most recently queued `item` from its queue and refund what was charged for it.
    CancelProduction {
        item: ProductionItem,
    },
    /// Place the player's ready building of type `building` with its footprint's origin (the
    /// cell with the smallest coordinates) at `origin` (docs/DECISIONS.md D24).
    PlaceBuilding {
        building: BuildingKind,
        origin: CellPos,
    },
    /// Sell the building: remove it, free its cells, and refund its sell value.
    SellBuilding {
        building: SimId,
    },
    /// Start repairing the player's building, or stop if it is being repaired. A repaired
    /// building regains hit points in steps, each paid for from the player's credits, until it
    /// is whole (`economy::repair_buildings`).
    RepairBuilding {
        building: SimId,
    },
    /// Units produced by the building move to `cell` when they leave it.
    SetRallyPoint {
        building: SimId,
        cell: CellPos,
    },
    /// The armed units attack `target`, a unit or building of another player that the issuing
    /// player sees, moving into range if needed, until it is destroyed or no longer visible
    /// (docs/DECISIONS.md D29). Harvesters among them stop their cycle.
    Attack {
        units: Vec<SimId>,
        target: SimId,
    },
    /// Move the units to `target` as `Move` does; the armed ones stop to fight visible enemies in
    /// range on the way and then move on (docs/DECISIONS.md D29). Harvesters among them stop
    /// their cycle.
    AttackMove {
        units: Vec<SimId>,
        target: CellPos,
    },
}

impl Command {
    /// The units a unit command names; `None` for the economy commands.
    pub(crate) fn units(&self) -> Option<&[SimId]> {
        match self {
            Command::Guard { units }
            | Command::Scatter { units }
            | Command::Follow { units, .. }
            | Command::ForceFire { units, .. }
            | Command::ForceMove { units, .. }
            | Command::QueueMove { units, .. }
            | Command::Move { units, .. }
            | Command::Stop { units }
            | Command::Harvest { units, .. }
            | Command::Attack { units, .. }
            | Command::AttackMove { units, .. } => Some(units),
            Command::Deploy { .. }
            | Command::Undeploy { .. }
            | Command::Interact { .. }
            | Command::Chronoshift { .. }
            | Command::Superweapon { .. }
            | Command::QueueProduction { .. }
            | Command::CancelProduction { .. }
            | Command::PlaceBuilding { .. }
            | Command::SellBuilding { .. }
            | Command::RepairBuilding { .. }
            | Command::SetRallyPoint { .. } => None,
        }
    }
}

/// Why a command was rejected. A rejected command changes no state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectReason {
    AbilityUnavailable(&'static str),
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
    /// The player owns factories of the kind that produces `item`, but none of a side that
    /// builds it (RA1: an Allied Barracks does not train Conscripts).
    NoFactoryOfSide(ProductionItem),
    /// Only certain countries build `item`, and the player's is not one of them (RA1).
    CountryRequired(ProductionItem),
    /// The player's country may not build `item` (RA1).
    CountryForbidden(ProductionItem),
    /// The options of the match switch this off (RA1: a superweapon building with superweapons
    /// off, `Undeploy` with "MCV repacks" off).
    DisabledByOptions,
    /// The queue of `producer` already holds `max` items (with a rules file, RA2: the lane of
    /// the item's producer, which holds one building, or one unit and those queued behind it).
    QueueFull {
        producer: BuildingKind,
        max: usize,
    },
    /// The player already owns or has queued `limit` buildings of this type, the most its type
    /// allows (RA2, the ini's `BuildLimit`).
    BuildLimitReached {
        item: ProductionItem,
        limit: u32,
    },
    /// The building's type cannot be sold (RA2: a wall).
    NotSellable(SimId),
    /// The building's type cannot be repaired (RA2: a wall).
    NotRepairable(SimId),
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
    /// The aircraft needs a dock, and every dock of the player's buildings is taken by an
    /// aircraft it owns or has queued (`QueueProduction`; RA4).
    NoFreeDock(ProductionItem),
    /// The unit is a missile or an aircraft that another unit launched: it takes no commands
    /// (RA4).
    NotCommandable(SimId),
    /// No unit or building with this id is visible to the issuing player (`Attack`): it does not
    /// exist (any more), or it is an enemy the player does not see. The two cases are reported
    /// alike, so that commands reveal nothing about unseen enemies (docs/DECISIONS.md D30).
    UnknownTarget(SimId),
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbilityUnavailable(reason) => write!(f, "{reason}"),
            Self::NoFreeDock(item) => {
                write!(f, "{item:?} needs a dock, and every dock is taken")
            }
            Self::NotCommandable(id) => write!(f, "unit {} takes no commands", id.0),
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
            Self::NoFactoryOfSide(item) => write!(
                f,
                "{item:?} is built by factories of another side, and the player owns none"
            ),
            Self::CountryRequired(item) => {
                write!(f, "{item:?} is built only by other countries")
            }
            Self::CountryForbidden(item) => {
                write!(f, "{item:?} may not be built by the player's country")
            }
            Self::DisabledByOptions => write!(f, "the options of the match switch this off"),
            Self::QueueFull { producer, max } => write!(
                f,
                "the queue of building type {} already holds {max} items",
                producer.0
            ),
            Self::BuildLimitReached { item, limit } => write!(
                f,
                "the player may have {limit} of {item:?}, and has or has queued as many"
            ),
            Self::NotSellable(id) => write!(f, "building {} cannot be sold", id.0),
            Self::NotRepairable(id) => write!(f, "building {} cannot be repaired", id.0),
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn process_regular_commands(
    mut queue: ResMut<CommandQueue>,
    mut grid: ResMut<Grid>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    mut units: UnitQuery,
    mut economy: EconomyParams,
    mut special: ResMut<crate::SpecialState>,
    mut tactics: ResMut<crate::TacticalState>,
) {
    if queue.pending.is_empty() {
        return;
    }
    // The units a command may name: those with hit points left. One that an earlier command of
    // the tick destroyed is removed in `Cleanup` and is gone for the commands after it
    // (docs/DECISIONS.md R56); so is such a building (`EconomyParams::building_index`).
    let index: BTreeMap<SimId, Entity> = units
        .iter()
        .filter(|(entity, ..)| economy.alive(*entity))
        .map(|(entity, id, ..)| (*id, entity))
        .collect();
    let mut buildings = None;
    for issued in queue.pending.drain(..) {
        let result = if issued.command.units().is_some() {
            let buildings = buildings.get_or_insert_with(|| economy.building_index());
            validate_player(&issued, &economy)
                .and_then(|()| validate(&issued, &grid, &economy, &index, &units, &special.hidden))
                .and_then(|()| {
                    // A launched missile or carried aircraft takes no commands (RA4).
                    match issued.command.units().and_then(|ids| {
                        ids.iter().find(|id| {
                            special.spawned.contains_key(id)
                                || special.strategic.flights.contains_key(id)
                                || special.erasures.contains_key(id)
                                || (special.deployed.contains_key(id)
                                    && matches!(
                                        issued.command,
                                        Command::Move { .. }
                                            | Command::ForceMove { .. }
                                            | Command::QueueMove { .. }
                                            | Command::Follow { .. }
                                            | Command::Scatter { .. }
                                            | Command::AttackMove { .. }
                                            | Command::Harvest { .. }
                                    ))
                        })
                    }) {
                        Some(&id) => Err(RejectReason::NotCommandable(id)),
                        None => Ok(()),
                    }
                })
                .and_then(|()| {
                    validate_combat(&issued, &economy, buildings, &index, &units, &special)
                })
                .and_then(|()| {
                    if let Command::QueueMove { units: ids, .. } = &issued.command {
                        for id in ids {
                            if tactics
                                .waypoints
                                .get(id)
                                .is_some_and(|q| q.len() >= crate::MAX_WAYPOINTS)
                            {
                                return Err(RejectReason::AbilityUnavailable(
                                    "Waypoint queue is full",
                                ));
                            }
                        }
                    }
                    Ok(())
                })
                .map(|()| {
                    if let Some(ids) = issued.command.units() {
                        for id in ids {
                            if !matches!(issued.command, Command::QueueMove { .. }) {
                                tactics.clear(*id);
                            }
                        }
                    }
                    if let Some(ids) = issued.command.units() {
                        for id in ids {
                            special.interactions.remove(id);
                            special.docked.remove(id);
                        }
                    }
                    apply_unit_command(
                        &issued.command,
                        tick.0,
                        &grid,
                        economy.definitions(),
                        &index,
                        &mut units,
                        &mut events,
                        &mut tactics,
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
#[allow(clippy::too_many_arguments)]
fn apply_unit_command(
    command: &Command,
    tick: u64,
    grid: &Grid,
    definitions: &Definitions,
    index: &BTreeMap<SimId, Entity>,
    units: &mut UnitQuery,
    events: &mut Messages<SimEvent>,
    tactics: &mut crate::TacticalState,
) {
    match command {
        Command::Guard { units: ids } | Command::Scatter { units: ids } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            for id in sorted {
                if let Some(&e) = index.get(&id)
                    && let Ok((_, _, _, _, m, mut o, ..)) = units.get_mut(e)
                {
                    o.current = None;
                    if matches!(command, Command::Guard { .. }) {
                        tactics
                            .orders
                            .insert(id, crate::TacticalOrder::Guard { anchor: m.cell });
                    } else {
                        // Ordered eight-direction scatter uses free navigation destinations.
                        let directions = [
                            (-2, -2),
                            (0, -2),
                            (2, -2),
                            (2, 0),
                            (2, 2),
                            (0, 2),
                            (-2, 2),
                            (-2, 0),
                        ];
                        let d = directions[id.0 as usize % directions.len()];
                        let target = CellPos::new(
                            m.cell.x.saturating_add(d.0),
                            m.cell.y.saturating_add(d.1),
                        );
                        let dest = destination_cells(
                            &grid.navigation_for(grid.domain_of(id)),
                            target,
                            1,
                            |c| !grid.room_for(c, id),
                        )
                        .first()
                        .copied();
                        o.current = dest.map(|c| MoveOrder::new(c, tick));
                    }
                }
            }
        }
        Command::Follow { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for id in ids {
                tactics
                    .orders
                    .insert(*id, crate::TacticalOrder::Follow { target: *target });
            }
        }
        Command::ForceFire { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for id in ids {
                if let Some(&e) = index.get(id)
                    && let Ok((.., mut o, _, Some(mut w))) = units.get_mut(e)
                {
                    o.current = None;
                    w.order = Some(AttackOrder::ForceFire { target: *target });
                }
            }
        }
        Command::QueueMove { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for id in ids {
                tactics.orders.remove(id);
                tactics.waypoints.entry(*id).or_default().push_back(*target);
            }
        }
        Command::Move { units: ids, target } | Command::ForceMove { units: ids, target } => {
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            apply_move(ids, *target, tick, grid, definitions, index, units, events);
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
            let miners: Vec<_> = ids
                .iter()
                .copied()
                .filter(|id| {
                    grid.cell(*target).is_some_and(|c| c.ore > 0)
                        && index
                            .get(id)
                            .and_then(|e| units.get(*e).ok())
                            .is_some_and(|u| u.6.is_some())
                })
                .collect();
            let fighters: Vec<_> = ids
                .iter()
                .copied()
                .filter(|id| !miners.contains(id))
                .collect();
            stop_fighting(&miners, index, units);
            apply_harvest(&miners, *target, tick, grid, definitions, index, units);
            let ids = &fighters;
            stop_harvesting(ids, index, units);
            stop_fighting(ids, index, units);
            for (entity, destination) in
                apply_move(ids, *target, tick, grid, definitions, index, units, events)
            {
                if let Ok((.., Some(mut weapon))) = units.get_mut(entity) {
                    weapon.order = Some(AttackOrder::AttackMove {
                        destination,
                        moving: true,
                    });
                }
            }
        }
        Command::Deploy { .. }
        | Command::Undeploy { .. }
        | Command::Interact { .. }
        | Command::Chronoshift { .. }
        | Command::Superweapon { .. }
        | Command::QueueProduction { .. }
        | Command::CancelProduction { .. }
        | Command::PlaceBuilding { .. }
        | Command::SellBuilding { .. }
        | Command::RepairBuilding { .. }
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
        harvester.field = grid
            .cell(target)
            .filter(|c| c.passable && c.ore > 0)
            .map(|_| target);
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
                harvester.field = Some(cell);
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
/// player that the issuing player does not see, or that is hidden from it (`hidden`: a submerged
/// unit, RA4), is `UnknownUnit` rather than `NotOwned`. The target of `Attack` is checked by
/// [`validate_combat`] (docs/DECISIONS.md D30).
fn validate(
    issued: &IssuedCommand,
    grid: &Grid,
    economy: &EconomyParams,
    index: &BTreeMap<SimId, Entity>,
    units: &UnitQuery,
    hidden: &BTreeSet<(PlayerId, SimId)>,
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
            let unseen = player_fog(issued.player, economy).is_some_and(|vision| {
                !Body::of(Some(&mobile), None).is_some_and(|body| body.seen_by(vision))
            }) || hidden.contains(&(issued.player, id));
            return Err(if unseen {
                RejectReason::UnknownUnit(id)
            } else {
                RejectReason::NotOwned(id)
            });
        }
        if matches!(issued.command, Command::Harvest { .. }) && !harvester {
            return Err(RejectReason::NotAHarvester(id));
        }
    }
    if let Command::ForceMove { target, .. }
    | Command::QueueMove { target, .. }
    | Command::ForceFire {
        target: ForceTarget::Cell(target),
        ..
    }
    | Command::Move { target, .. }
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
/// and exists and is visible to the player and not hidden from it (`UnknownTarget`, the same
/// reason for all three, so that an unseen or hidden enemy is reported like a missing one, and
/// only after the checks that a missing one meets: docs/DECISIONS.md R56). Every unit and building has hit points whenever
/// any unit has a weapon (a combat file gives every type combat terms, docs/DECISIONS.md D28
/// item 4), so no check for them is needed (M4 review 1 P2-2, D32).
fn validate_combat(
    issued: &IssuedCommand,
    economy: &EconomyParams,
    buildings: &BuildingIndex,
    index: &BTreeMap<SimId, Entity>,
    units: &UnitQuery,
    special: &crate::SpecialState,
) -> Result<(), RejectReason> {
    let (ids, target, forced, follow) = match &issued.command {
        Command::Attack { units, target } => (units, Some(*target), false, false),
        Command::Follow { units, target } => (units, Some(*target), true, true),
        Command::ForceFire { units, target } => (
            units,
            match target {
                ForceTarget::Entity(id) => Some(*id),
                ForceTarget::Cell(_) => None,
            },
            true,
            false,
        ),
        _ => return Ok(()),
    };
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
        if !follow && !armed {
            return Err(RejectReason::NotArmed(id));
        }
    }
    let Some(target) = target else {
        return Ok(());
    };
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
    if let Some(g) = special.garrisons.get(&target) {
        return if !forced && g.owner == issued.player {
            Err(RejectReason::OwnTarget(target))
        } else if !vision.is_visible(g.cell) || special.hidden.contains(&(issued.player, target)) {
            Err(RejectReason::UnknownTarget(target))
        } else {
            Ok(())
        };
    }
    // Scenery (`crate::props`) may be attacked once the player has explored its cell.
    if found.is_none()
        && let Some((cell, _)) = economy.props().get(target)
    {
        return if Body::Prop(cell).seen_by(vision) {
            Ok(())
        } else {
            Err(RejectReason::UnknownTarget(target))
        };
    }
    match found {
        Some((owner, _)) if !forced && owner == issued.player => {
            Err(RejectReason::OwnTarget(target))
        }
        Some((owner, Some(body)))
            if (owner == issued.player || body.seen_by(vision))
                && !special.hidden.contains(&(issued.player, target)) =>
        {
            Ok(())
        }
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
///
/// Since RA3 (invariant I6', docs/DECISIONS.md R6) a cell is handed to one vehicle or to as many
/// units on foot as it has spots left beside the idle infantry outside the group that stands in
/// it, at most three; the vehicles of a domain get their cells first, then its infantry. The
/// move of a crusher may crush ([`MoveOrder::commanded`]).
#[allow(clippy::too_many_arguments)]
fn apply_move(
    ids: &[SimId],
    target: CellPos,
    tick: u64,
    grid: &Grid,
    definitions: &Definitions,
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
    let mut assigned = Vec::with_capacity(order.len());
    // The room of each cell that is promised to units of the group, in spots, per layer.
    let mut reserved: BTreeMap<(bool, CellPos), u8> = BTreeMap::new();
    // The units of the group by what they need of a cell: their domain, and whether they are on
    // foot (the vehicles of a domain come first).
    let class = |id: SimId| (grid.domain_of(id), grid.is_on_foot(id));
    let mut counts: BTreeMap<(crate::Domain, bool), usize> = BTreeMap::new();
    for (_, id, _) in &order {
        *counts.entry(class(*id)).or_default() += 1;
    }
    let mut destinations: BTreeMap<(crate::Domain, bool), std::collections::VecDeque<CellPos>> =
        BTreeMap::new();
    for ((domain, on_foot), count) in counts {
        // Aircraft and surface units hold cells in separate layers (`Grid::occupant`), so a
        // cell handed to one may still be handed to the other.
        let air = domain == crate::Domain::Air;
        let view = grid.navigation_for(domain);
        let needed = Grid::room_needed(on_foot);
        // The room a cell has left for units of this class: what the idle units outside the
        // group leave of it, less what is promised to the group.
        let room = |reserved: &BTreeMap<(bool, CellPos), u8>, cell: CellPos| {
            view.room(cell, on_foot, |holder| idle_outside_group.contains(&holder))
                .saturating_sub(reserved.get(&(air, cell)).copied().unwrap_or(0))
        };
        let cells = destination_cells(&view, target, count, |cell| room(&reserved, cell) < needed);
        let mut handed = std::collections::VecDeque::with_capacity(count);
        for cell in cells {
            // A cell is handed out as often as it has room: once to a vehicle, up to three
            // times to units on foot.
            let places = usize::from(room(&reserved, cell) / needed).min(count - handed.len());
            if places == 0 {
                break;
            }
            handed.extend(std::iter::repeat_n(cell, places));
            let promised = u8::try_from(places)
                .unwrap_or(u8::MAX)
                .saturating_mul(needed);
            let total = reserved.entry((air, cell)).or_default();
            *total = total.saturating_add(promised);
        }
        destinations.insert((domain, on_foot), handed);
    }
    for (_, id, entity) in order {
        let destination = destinations
            .get_mut(&class(id))
            .and_then(|cells| cells.pop_front());
        let Ok((_, _, _, unit, _, mut orders, _, _)) = units.get_mut(entity) else {
            continue;
        };
        match destination {
            Some(destination) => {
                let crusher = definitions.crusher(unit.kind);
                orders.current = Some(MoveOrder::commanded(destination, tick, crusher));
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

/// Flush each contiguous run of ordinary commands before an ability. This keeps ownership,
/// resource and collision checks atomic even for mixed commands within one tick.
pub(crate) fn process_commands(world: &mut bevy_ecs::world::World) {
    if world.resource::<CommandQueue>().is_empty() {
        // Raw scenario spawns may precede an AI turn without any intervening command.
        // Its View must use the same living/powered concealment as command validation.
        if world
            .resource::<crate::AiPlayers>()
            .thinks_at(world.resource::<SimTick>().0)
        {
            refresh_command_vision(world);
        }
        return;
    }
    refresh_command_vision(world);
    let mut mirages = concealment_movers(world);
    let pending = std::mem::take(&mut world.resource_mut::<CommandQueue>().pending);
    for issued in pending {
        if matches!(
            issued.command,
            Command::Deploy { .. }
                | Command::Undeploy { .. }
                | Command::Interact { .. }
                | Command::Chronoshift { .. }
                | Command::Superweapon { .. }
        ) {
            if !world.resource::<CommandQueue>().pending.is_empty() {
                crate::special::infantry::refresh_limits(world);
                world
                    .run_system_cached(process_regular_commands)
                    .expect("registered command system");
            }
            let ability_actor = match &issued.command {
                Command::Deploy { unit } | Command::Interact { unit, .. } => Some(*unit),
                Command::Undeploy { building } => Some(*building),
                _ => None,
            };
            let result = crate::special::apply(world, &issued);
            let accepted = result.is_ok();
            if let Err(reason) = result {
                world
                    .resource_mut::<Messages<SimEvent>>()
                    .write(SimEvent::CommandRejected {
                        player: issued.player,
                        command: issued.command,
                        reason,
                    });
            }
            if accepted && let Some(id) = ability_actor {
                world.resource_mut::<crate::TacticalState>().clear(id);
            }
            refresh_command_vision(world);
            mirages = concealment_movers(world);
        } else {
            let boundary = matches!(
                issued.command,
                Command::SellBuilding { .. } | Command::PlaceBuilding { .. }
            ) || issued
                .command
                .units()
                .is_some_and(|ids| ids.iter().any(|id| mirages.contains(id)));
            world.resource_mut::<CommandQueue>().pending.push(issued);
            if boundary {
                crate::special::infantry::refresh_limits(world);
                world
                    .run_system_cached(process_regular_commands)
                    .expect("registered command system");
                refresh_command_vision(world);
            }
        }
    }
    if !world.resource::<CommandQueue>().pending.is_empty() {
        crate::special::infantry::refresh_limits(world);
        world
            .run_system_cached(process_regular_commands)
            .expect("registered command system");
    }
}

/// Only sales/placement and actual Mirage orders split regular batches. Normal army orders
/// retain contiguous processing, with no per-command Definitions clone or full unit scan.
fn concealment_movers(world: &bevy_ecs::world::World) -> BTreeSet<SimId> {
    let defs = world.resource::<Definitions>();
    world
        .iter_entities()
        .filter_map(|e| {
            let unit = e.get::<Unit>()?;
            defs.metadata(crate::ProductionItem::Unit(unit.kind))
                .is_some_and(|m| m.disguise_when_still || m.role == crate::Role::Mirage)
                .then(|| e.get::<SimId>().copied())
                .flatten()
        })
        .collect()
}
fn refresh_command_vision(world: &mut bevy_ecs::world::World) {
    world
        .run_system_cached(crate::economy::update_power)
        .expect("registered power system");
    crate::special::refresh_concealment(world);
}
