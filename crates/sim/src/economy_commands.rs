//! Validation and application of the economy commands: `QueueProduction`, `CancelProduction`,
//! `PlaceBuilding`, `SellBuilding`, and `SetRallyPoint` (PLAN.md section 5; docs/DECISIONS.md
//! D25).
//!
//! Every check runs before any state changes, so a rejected command changes no state. Each
//! command is checked in this order, and the first failure is the reason:
//!
//! * all: the issuing player is a player of the match (`UnknownPlayer`);
//! * `QueueProduction`: the item is defined and producible (`UnknownItem`), the player owns a
//!   building of each prerequisite type (`MissingPrerequisite`, in the order the economy file
//!   lists them) and of the producing type (`NoProducer`), and the queue is not full
//!   (`QueueFull`);
//! * `CancelProduction`: the item is defined and producible, and queued (`NotQueued`);
//! * `PlaceBuilding`: the building type is defined, a complete building of that type is at the
//!   head of its queue (`NotReady`), the footprint is valid (`Placement`: inside the map,
//!   buildable, no building, no unit), and it lies within the placement distance of a building
//!   of the player (`Placement(TooFar)`);
//! * `SellBuilding`: the building exists (`UnknownBuilding`) and belongs to the player
//!   (`NotOwned`);
//! * `SetRallyPoint`: the building exists and belongs to the player, its type produces units
//!   (`NotAProducer`), and the cell is inside the map (`OutsideMap`).

use std::collections::BTreeMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{Commands, Query, Res, ResMut, SystemParam};

use crate::building::{
    Building, Footprint, Producer, building_bundle, check_distance, check_footprint,
};
use crate::command::{Command, IssuedCommand, PlayerId, RejectReason};
use crate::definitions::Definitions;
use crate::economy::{MAX_QUEUE_LENGTH, Players, QueueItem};
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::event::SimEvent;
use crate::id::{SimId, SimIdAllocator};
use crate::map::Grid;
use crate::unit::Owner;

/// The system parameters the economy commands need besides the grid and the event buffer.
#[derive(SystemParam)]
pub(crate) struct EconomyParams<'w, 's> {
    commands: Commands<'w, 's>,
    players: ResMut<'w, Players>,
    definitions: Res<'w, Definitions>,
    ids: ResMut<'w, SimIdAllocator>,
    buildings: Query<'w, 's, (Entity, &'static SimId, &'static Owner, &'static Building)>,
}

/// A building as the commands of one tick see it.
#[derive(Clone, Copy)]
struct Indexed {
    entity: Entity,
    owner: PlayerId,
    kind: BuildingKind,
    footprint: Footprint,
}

/// The buildings, kept up to date as the commands of a tick place and sell them (the entity
/// changes themselves are deferred to the end of the stage).
pub(crate) struct BuildingIndex {
    by_id: BTreeMap<SimId, Indexed>,
}

impl BuildingIndex {
    fn owns(&self, player: PlayerId, kind: BuildingKind) -> bool {
        self.by_id
            .values()
            .any(|building| building.owner == player && building.kind == kind)
    }

    fn footprints_of(&self, player: PlayerId) -> impl Iterator<Item = Footprint> + '_ {
        self.by_id
            .values()
            .filter(move |building| building.owner == player)
            .map(|building| building.footprint)
    }

    /// The player's building `id`.
    fn own(&self, player: PlayerId, id: SimId) -> Result<Indexed, RejectReason> {
        let building = *self
            .by_id
            .get(&id)
            .ok_or(RejectReason::UnknownBuilding(id))?;
        if building.owner != player {
            return Err(RejectReason::NotOwned(id));
        }
        Ok(building)
    }
}

impl EconomyParams<'_, '_> {
    pub(crate) fn definitions(&self) -> &Definitions {
        &self.definitions
    }

    pub(crate) fn building_index(&self) -> BuildingIndex {
        BuildingIndex {
            by_id: self
                .buildings
                .iter()
                .map(|(entity, id, owner, building)| {
                    (
                        *id,
                        Indexed {
                            entity,
                            owner: owner.0,
                            kind: building.kind,
                            footprint: building.footprint,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// Validates and applies an economy command. Unit commands are handled by `command.rs`.
pub(crate) fn apply(
    issued: &IssuedCommand,
    params: &mut EconomyParams,
    index: &mut BuildingIndex,
    grid: &mut Grid,
    events: &mut Messages<SimEvent>,
) -> Result<(), RejectReason> {
    let player = issued.player;
    let definitions = &*params.definitions;
    let state = params
        .players
        .get_mut(player)
        .ok_or(RejectReason::UnknownPlayer(player))?;
    match &issued.command {
        &Command::QueueProduction { item } => {
            let terms = definitions
                .production(item)
                .ok_or(RejectReason::UnknownItem(item))?;
            if let Some(&prerequisite) = terms
                .prerequisites
                .iter()
                .find(|&&prerequisite| !index.owns(player, prerequisite))
            {
                return Err(RejectReason::MissingPrerequisite { item, prerequisite });
            }
            if !index.owns(player, terms.producer) {
                return Err(RejectReason::NoProducer {
                    item,
                    producer: terms.producer,
                });
            }
            if state.queue(terms.producer).len() >= MAX_QUEUE_LENGTH {
                return Err(RejectReason::QueueFull {
                    producer: terms.producer,
                    max: MAX_QUEUE_LENGTH,
                });
            }
            state
                .queues
                .entry(terms.producer)
                .or_default()
                .push(QueueItem {
                    item,
                    progress: 0,
                    charged: 0,
                });
        }
        &Command::CancelProduction { item } => {
            let terms = definitions
                .production(item)
                .ok_or(RejectReason::UnknownItem(item))?;
            let queue = state
                .queues
                .get_mut(&terms.producer)
                .ok_or(RejectReason::NotQueued(item))?;
            let position = queue
                .iter()
                .rposition(|queued| queued.item == item)
                .ok_or(RejectReason::NotQueued(item))?;
            let removed = queue.remove(position);
            if queue.is_empty() {
                state.queues.remove(&terms.producer);
            }
            state.credits = state.credits.saturating_add(removed.charged);
            events.write(SimEvent::ProductionCancelled {
                player,
                item,
                refund: removed.charged,
            });
        }
        &Command::PlaceBuilding { building, origin } => {
            let item = ProductionItem::Building(building);
            let def = definitions
                .building(building)
                .ok_or(RejectReason::UnknownItem(item))?;
            let producer = def.production.producer;
            let ready = state.queue(producer).first().is_some_and(|head| {
                head.item == item && head.progress >= def.production.build_time
            });
            if !ready {
                return Err(RejectReason::NotReady(building));
            }
            let footprint = Footprint {
                origin,
                width: def.width,
                height: def.height,
            };
            check_footprint(grid, footprint).map_err(RejectReason::Placement)?;
            check_distance(
                footprint,
                index.footprints_of(player),
                definitions.placement_distance(),
            )
            .map_err(RejectReason::Placement)?;
            let id = params.ids.allocate().ok_or(RejectReason::IdsExhausted)?;

            if let Some(queue) = state.queues.get_mut(&producer) {
                queue.remove(0);
                if queue.is_empty() {
                    state.queues.remove(&producer);
                }
            }
            for cell in footprint.cells() {
                grid.cover(cell, id);
            }
            let (id, owner, component, rally) = building_bundle(
                definitions,
                id,
                player,
                building,
                footprint,
                def.free_units.clone(),
            );
            let mut entity = params.commands.spawn((id, owner, component));
            if let Some(rally) = rally {
                entity.insert(rally);
            }
            let entity = entity.id();
            index.by_id.insert(
                id,
                Indexed {
                    entity,
                    owner: player,
                    kind: building,
                    footprint,
                },
            );
            events.write(SimEvent::BuildingPlaced {
                player,
                building: id,
                kind: building,
            });
        }
        &Command::SellBuilding { building } => {
            let target = index.own(player, building)?;
            // Every indexed building's kind comes from the definitions, which do not change
            // during a match, so the lookup always succeeds.
            let refund = definitions
                .building(target.kind)
                .map_or(0, |def| def.sell_value);
            for cell in target.footprint.cells() {
                grid.uncover(cell, building);
            }
            params.commands.entity(target.entity).despawn();
            index.by_id.remove(&building);
            state.credits = state.credits.saturating_add(refund);
            events.write(SimEvent::BuildingSold {
                player,
                building,
                refund,
            });
        }
        &Command::SetRallyPoint { building, cell } => {
            let target = index.own(player, building)?;
            if !definitions.produces_units(target.kind) {
                return Err(RejectReason::NotAProducer(building));
            }
            if !grid.contains(cell) {
                return Err(RejectReason::OutsideMap(cell));
            }
            params
                .commands
                .entity(target.entity)
                .insert(Producer { rally: Some(cell) });
        }
        Command::Move { .. } | Command::Stop { .. } | Command::Harvest { .. } => {}
    }
    Ok(())
}
