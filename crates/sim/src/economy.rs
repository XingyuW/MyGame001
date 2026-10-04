//! Players, credits, power, and production (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md
//! D25).
//!
//! Each player has credits, the power its buildings produce and consume, and one production
//! queue per producing building type. Only the item at the head of a queue progresses, and only
//! while the player owns a building of the queue's type. On each tick in which it progresses, an
//! item of cost `C` and build time `T` is charged `C / T` credits, and on its last tick
//! `C / T + C mod T`, so the total charged is exactly `C` (equation 3, invariant I7). An item
//! does not progress on a tick in which the player's credits are below that tick's charge, so
//! credits never fall below zero, and under low power (consumed > produced) it progresses only on
//! even ticks. Cancelling refunds exactly what was charged.
//!
//! A complete building waits at the head of its queue until it is placed with `PlaceBuilding`.
//! A complete unit leaves a producing building of its owner as soon as a cell around the
//! building is free (`building::exit_cell`), and moves to the building's rally point if one is
//! set; a harvester starts its harvesting cycle instead.

use std::collections::BTreeMap;
use std::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};

use crate::building::{Building, Footprint, Producer, exit_cell};
use crate::combat::combat_components;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::event::SimEvent;
use crate::harvest::Harvester;
use crate::hash::Fnv1a64;
use crate::id::{SimId, SimIdAllocator};
use crate::map::{CellPos, Grid};
use crate::pathfinding::destination_cells;
use crate::schedule::SimTick;
use crate::unit::{MoveOrder, Owner, unit_bundle};
use crate::visibility::Vision;

/// Largest number of items in one production queue.
pub const MAX_QUEUE_LENGTH: usize = 16;

/// An item in a production queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueItem {
    pub item: ProductionItem,
    /// Ticks of progress made, from 0 to the item's build time.
    pub progress: u32,
    /// Credits charged so far, from 0 to the item's cost.
    pub charged: i64,
}

/// The economic state of one player.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerState {
    pub(crate) credits: i64,
    pub(crate) power_produced: i32,
    pub(crate) power_consumed: i32,
    /// One queue per producing building type; queues that become empty are removed.
    pub(crate) queues: BTreeMap<BuildingKind, Vec<QueueItem>>,
    /// What the player sees of the grid (M4, `crate::visibility`).
    pub(crate) vision: Vision,
}

impl PlayerState {
    /// Credits, never negative (invariant I7).
    pub fn credits(&self) -> i64 {
        self.credits
    }

    /// Power produced by the player's buildings, as of the last `Economy` stage.
    pub fn power_produced(&self) -> i32 {
        self.power_produced
    }

    /// Power consumed by the player's buildings, as of the last `Economy` stage.
    pub fn power_consumed(&self) -> i32 {
        self.power_consumed
    }

    /// Low power: more power consumed than produced (PLAN.md section 5).
    pub fn low_power(&self) -> bool {
        self.power_consumed > self.power_produced
    }

    /// The queue of items produced by buildings of type `producer`, head first.
    pub fn queue(&self, producer: BuildingKind) -> &[QueueItem] {
        self.queues.get(&producer).map_or(&[], Vec::as_slice)
    }

    /// The player's visibility grid as of the last `Visibility` stage (shroud and fog,
    /// docs/DECISIONS.md D30).
    pub fn vision(&self) -> &Vision {
        &self.vision
    }

    /// Every non-empty queue, by producing building type.
    pub fn queues(&self) -> impl Iterator<Item = (BuildingKind, &[QueueItem])> {
        self.queues
            .iter()
            .map(|(&producer, items)| (producer, items.as_slice()))
    }
}

/// Why a player could not be added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerSetupError {
    NegativeCredits(i64),
    AlreadyAdded(PlayerId),
}

impl fmt::Display for PlayerSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeCredits(credits) => {
                write!(f, "starting credits {credits} are negative")
            }
            Self::AlreadyAdded(player) => write!(f, "player {} was already added", player.0),
        }
    }
}

impl std::error::Error for PlayerSetupError {}

/// The players of the match, their economic state, and their visibility grids (PLAN.md section 5).
/// Factions arrive in M7.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Players {
    players: BTreeMap<PlayerId, PlayerState>,
}

impl Players {
    /// Adds `player` with `credits` starting credits.
    ///
    /// Scenario and match setup only, like installing the grid (docs/DECISIONS.md D11): during
    /// a match credits change only through production, refining, selling, and cancelling.
    pub fn add(&mut self, player: PlayerId, credits: i64) -> Result<(), PlayerSetupError> {
        if credits < 0 {
            return Err(PlayerSetupError::NegativeCredits(credits));
        }
        if self.players.contains_key(&player) {
            return Err(PlayerSetupError::AlreadyAdded(player));
        }
        self.players.insert(
            player,
            PlayerState {
                credits,
                ..PlayerState::default()
            },
        );
        Ok(())
    }

    pub fn get(&self, player: PlayerId) -> Option<&PlayerState> {
        self.players.get(&player)
    }

    pub(crate) fn get_mut(&mut self, player: PlayerId) -> Option<&mut PlayerState> {
        self.players.get_mut(&player)
    }

    /// The players in ascending id order.
    pub fn iter(&self) -> impl Iterator<Item = (PlayerId, &PlayerState)> {
        self.players.iter().map(|(&player, state)| (player, state))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (PlayerId, &mut PlayerState)> {
        self.players
            .iter_mut()
            .map(|(&player, state)| (player, state))
    }

    /// Every player in id order: id, credits, power produced and consumed, then each queue
    /// (producing building type, item count, and per item its kind tag and index, progress,
    /// and credits charged), then the vision (from M4).
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.players.len() as u64);
        for (player, state) in &self.players {
            hasher.write(&[player.0]);
            hasher.write(&state.credits.to_le_bytes());
            hasher.write(&state.power_produced.to_le_bytes());
            hasher.write(&state.power_consumed.to_le_bytes());
            hasher.write_u64(state.queues.len() as u64);
            for (producer, items) in &state.queues {
                hasher.write(&producer.0.to_le_bytes());
                hasher.write_u64(items.len() as u64);
                for item in items {
                    let (tag, index) = match item.item {
                        ProductionItem::Building(kind) => (0, kind.0),
                        ProductionItem::Unit(kind) => (1, kind.0),
                    };
                    hasher.write(&[tag]);
                    hasher.write(&index.to_le_bytes());
                    hasher.write_u32(item.progress);
                    hasher.write(&item.charged.to_le_bytes());
                }
            }
            state.vision.hash_into(hasher);
        }
    }
}

/// The credits charged on the tick that takes an item of cost `cost` and build time
/// `build_time` from `progress` to `progress + 1` (equation 3): `cost / build_time`, plus
/// `cost mod build_time` on the last tick. `build_time` is at least 1 for every defined item; a
/// value of 0 is treated as 1.
pub(crate) fn charge_for_tick(cost: i64, build_time: u32, progress: u32) -> i64 {
    let time = i64::from(build_time.max(1));
    let per_tick = cost / time;
    if progress.saturating_add(1) >= build_time {
        per_tick + cost % time
    } else {
        per_tick
    }
}

/// Recomputes every player's power from the buildings it owns (first system of the `Economy`
/// stage, so buildings placed or sold by this tick's commands count).
pub(crate) fn update_power(
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    buildings: Query<(&Owner, &Building)>,
) {
    let mut totals: BTreeMap<PlayerId, (i32, i32)> = BTreeMap::new();
    for (owner, building) in &buildings {
        let Some(def) = definitions.building(building.kind) else {
            continue;
        };
        let total = totals.entry(owner.0).or_default();
        // Bounded far below `i32::MAX` (economy_defs::MAX_POWER); saturating for invariant I12.
        total.0 = total.0.saturating_add(def.power_produced);
        total.1 = total.1.saturating_add(def.power_consumed);
    }
    for (player, state) in &mut players.players {
        let (produced, consumed) = totals.get(player).copied().unwrap_or_default();
        state.power_produced = produced;
        state.power_consumed = consumed;
    }
}

/// A building of a player that units leave: its id, footprint, and rally point.
#[derive(Clone, Copy)]
struct Exit {
    building: SimId,
    footprint: Footprint,
    rally: Option<CellPos>,
}

/// Everything a unit needs to leave a building.
struct Spawner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    grid: &'a mut Grid,
    ids: &'a mut SimIdAllocator,
    definitions: &'a Definitions,
    events: &'a mut Messages<SimEvent>,
    tick: u64,
}

impl Spawner<'_, '_, '_> {
    /// Spawns a unit of `kind` for `player` at a free ring cell of `exit`, if there is one, and
    /// returns its id. A harvester starts harvesting; any other unit moves to the free cell
    /// nearest to the rally point, if one is set.
    fn leave(&mut self, player: PlayerId, kind: UnitKind, exit: Exit) -> Option<SimId> {
        let speed = self.definitions.unit(kind)?.speed;
        let cell = exit_cell(self.grid, exit.footprint, exit.rally)?;
        let id = self.ids.allocate()?;
        if !self.grid.claim(cell, id) {
            return None;
        }
        let mut bundle = unit_bundle(id, player, kind, speed, cell);
        let harvester = self.definitions.harvester(kind).is_some();
        if !harvester && let Some(rally) = exit.rally {
            let destination = destination_cells(self.grid, rally, 1, |other| {
                self.grid.occupant(other).is_some_and(|holder| holder != id)
            });
            if let Some(&destination) = destination.first() {
                bundle.6.current = Some(MoveOrder::new(destination, self.tick));
            }
        }
        let mut entity = self.commands.spawn(bundle);
        if harvester {
            entity.insert(Harvester::new());
        }
        if let Some(terms) = self.definitions.unit_combat(kind) {
            let (components, weapon) = combat_components(terms);
            entity.insert(components);
            if let Some(weapon) = weapon {
                entity.insert(weapon);
            }
        }
        self.events.write(SimEvent::UnitSpawned {
            player,
            unit: id,
            building: exit.building,
        });
        Some(id)
    }
}

type BuildingQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static mut Building,
        Option<&'static Producer>,
    ),
>;

/// Advances the head of every production queue by one tick (equation 3), lets complete units
/// leave their producing buildings, and lets the free units of newly placed buildings leave
/// them. Players are processed in id order and queues in order of their building type, so the
/// result is deterministic (invariant I4).
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_production(
    mut commands: Commands,
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    tick: Res<SimTick>,
    mut grid: ResMut<Grid>,
    mut ids: ResMut<SimIdAllocator>,
    mut events: ResMut<Messages<SimEvent>>,
    mut buildings: BuildingQuery,
) {
    // The buildings each player owns of each type, in id order.
    let mut exits: BTreeMap<(PlayerId, BuildingKind), Vec<Exit>> = BTreeMap::new();
    let mut sorted: Vec<(SimId, Entity, PlayerId, BuildingKind, Exit)> = buildings
        .iter()
        .map(|(entity, id, owner, building, producer)| {
            (
                *id,
                entity,
                owner.0,
                building.kind,
                Exit {
                    building: *id,
                    footprint: building.footprint,
                    rally: producer.and_then(|producer| producer.rally),
                },
            )
        })
        .collect();
    sorted.sort_unstable_by_key(|(id, ..)| *id);
    for &(_, _, owner, kind, exit) in &sorted {
        exits.entry((owner, kind)).or_default().push(exit);
    }

    let even_tick = tick.0.is_multiple_of(2);
    let mut spawner = Spawner {
        commands: &mut commands,
        grid: &mut grid,
        ids: &mut ids,
        definitions: &definitions,
        events: &mut events,
        tick: tick.0,
    };
    for (&player, state) in &mut players.players {
        let low_power = state.low_power();
        let producers: Vec<BuildingKind> = state.queues.keys().copied().collect();
        for producer in producers {
            let owned = exits
                .get(&(player, producer))
                .map_or(&[][..], Vec::as_slice);
            let credits = &mut state.credits;
            let Some(queue) = state.queues.get_mut(&producer) else {
                continue;
            };
            let Some(head) = queue.first_mut() else {
                continue;
            };
            // Every queued item was checked against the definitions, which do not change
            // during a match.
            let Some(terms) = definitions.production(head.item) else {
                continue;
            };
            if head.progress < terms.build_time {
                if owned.is_empty() || (low_power && !even_tick) {
                    continue;
                }
                let charge = charge_for_tick(terms.cost, terms.build_time, head.progress);
                if *credits < charge {
                    continue;
                }
                *credits -= charge;
                head.charged = head.charged.saturating_add(charge);
                head.progress += 1;
                if head.progress < terms.build_time {
                    continue;
                }
                spawner.events.write(SimEvent::ProductionComplete {
                    player,
                    item: head.item,
                });
            }
            // Complete: a building waits for `PlaceBuilding`; a unit leaves the first of the
            // player's producing buildings (in id order) that has a free ring cell.
            if let ProductionItem::Unit(kind) = head.item
                && owned
                    .iter()
                    .any(|&exit| spawner.leave(player, kind, exit).is_some())
            {
                queue.remove(0);
                if queue.is_empty() {
                    state.queues.remove(&producer);
                }
            }
        }
    }

    // Free units of buildings placed during the match leave them, oldest first.
    for (_, entity, owner, _, exit) in sorted {
        let Ok((_, _, _, mut building, _)) = buildings.get_mut(entity) else {
            continue;
        };
        while let Some(&unit) = building.pending_units.first() {
            if spawner.leave(owner, unit, exit).is_none() {
                break;
            }
            building.pending_units.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy_defs::{MAX_BUILD_TIME, MAX_COST};

    /// The charges of every tick of an item, summed, are its cost (equation 3), at the limits
    /// the economy file admits.
    #[test]
    fn the_charges_of_an_item_sum_to_its_cost_at_the_limits() {
        for (cost, time) in [
            (0, 1),
            (0, MAX_BUILD_TIME),
            (1, 1),
            (1, MAX_BUILD_TIME),
            (MAX_COST, 1),
            (MAX_COST, MAX_BUILD_TIME),
            (MAX_COST - 1, MAX_BUILD_TIME - 1),
            (105, 10),
        ] {
            let charges: Vec<i64> = (0..time)
                .map(|progress| charge_for_tick(cost, time, progress))
                .collect();
            assert_eq!(charges.iter().sum::<i64>(), cost, "{cost} over {time}");
            assert!(charges.iter().all(|&charge| charge >= 0));
            let per_tick = cost / i64::from(time);
            assert!(charges[..charges.len() - 1].iter().all(|&c| c == per_tick));
        }
    }
}
