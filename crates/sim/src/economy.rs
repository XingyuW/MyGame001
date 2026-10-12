//! Players, credits, power, and production (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md
//! D25).
//!
//! Each player has credits, the power its buildings produce and consume, and one production
//! queue per producing building type, or per factory group where the arsenal defines one (RA1,
//! `Definitions::queue_key`). Only the item at the head of a queue progresses, and only while the
//! player owns a building that produces it (`economy_commands::produces`) and its prerequisites. On each tick in which it progresses, an
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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};

use crate::building::{Building, Footprint, Producer, exit_cell};
use crate::combat::combat_components;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy_commands::{missing_prerequisite, produces};
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
    pub(crate) super_me: bool,
    /// Match option "unlock all" (human players only, like Super Me): building prerequisites
    /// are waived. Producers, credits, power, placement and exits still apply.
    pub(crate) unlock_all: bool,
    /// The player's team; 0 for none. Players of the same team (other than 0) are allies: their
    /// units and defences do not fire at each other ([`Players::hostile`]).
    pub(crate) team: u8,
    /// Match option (human players with Super Me only): how much faster than their type's speed
    /// the player's units move, in percent over 100.
    pub(crate) unit_speed_bonus: u16,
    pub(crate) credits: i64,
    pub(crate) power_produced: i32,
    pub(crate) power_consumed: i32,
    /// One queue per producing building type; queues that become empty are removed.
    pub(crate) queues: BTreeMap<BuildingKind, Vec<QueueItem>>,
    /// What the player sees of the grid (M4, `crate::visibility`).
    pub(crate) vision: Vision,
    /// The items the player's faction may produce (M7, docs/DECISIONS.md D48); `None` for a
    /// player added without a faction (the scenario setups of the M3 to M6 tests), who may
    /// produce every item with production terms.
    pub(crate) roster: Option<BTreeSet<ProductionItem>>,
    /// The player's buildings being repaired (`RepairBuilding`, [`repair_buildings`]).
    pub(crate) repairing: BTreeSet<SimId>,
    /// The player's country (RA1, an index into the countries of the rules file); `None` for a
    /// player of a faction without a side and for a player added without a faction.
    pub(crate) country: Option<crate::rules::CountryId>,
}

impl PlayerState {
    /// The player's buildings being repaired.
    pub fn repairing(&self) -> &BTreeSet<SimId> {
        &self.repairing
    }

    pub fn super_me(&self) -> bool {
        self.super_me
    }
    /// The player's country, an index into the countries of the rules file (RA1).
    pub fn country(&self) -> Option<crate::rules::CountryId> {
        self.country
    }
    /// Whether building prerequisites are waived for this player (match option).
    pub fn unlock_all(&self) -> bool {
        self.unlock_all
    }
    /// The player's team, 0 for none.
    pub fn team(&self) -> u8 {
        self.team
    }
    /// The speed of the player's units in percent of their type's speed (100: unchanged).
    pub fn unit_speed_percent(&self) -> u32 {
        100 + u32::from(self.unit_speed_bonus)
    }
    pub fn production_cost(&self, base: i64) -> i64 {
        if self.super_me { 1 } else { base }
    }
    pub fn production_speed(&self) -> u32 {
        if self.super_me { 10 } else { 1 }
    }

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

    /// The queue kept under building type `producer`, head first. A queue is kept under the
    /// producer of its items, or under the first building type of the producer's factory group
    /// where the arsenal defines groups ([`Definitions::queue_key`]).
    pub fn queue(&self, producer: BuildingKind) -> &[QueueItem] {
        self.queues.get(&producer).map_or(&[], Vec::as_slice)
    }

    /// The player's visibility grid as of the last `Visibility` stage (shroud and fog,
    /// docs/DECISIONS.md D30).
    pub fn vision(&self) -> &Vision {
        &self.vision
    }

    /// Whether the player's faction may produce `item` (every item for a player without a
    /// faction).
    pub fn offers(&self, item: ProductionItem) -> bool {
        self.roster
            .as_ref()
            .is_none_or(|roster| roster.contains(&item))
    }

    /// The roster of the player's faction, or `None` for a player added without one.
    pub fn roster(&self) -> Option<&BTreeSet<ProductionItem>> {
        self.roster.as_ref()
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

/// The players of the match, their economic state, their visibility grids, and the rosters of their
/// factions (PLAN.md section 5).
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
        self.insert(player, credits, None)
    }

    /// Adds `player` with `credits` starting credits and the roster of its faction: it may queue
    /// only the items of `roster` (M7). Setup only, like [`Players::add`].
    pub fn add_with_roster(
        &mut self,
        player: PlayerId,
        credits: i64,
        roster: BTreeSet<ProductionItem>,
    ) -> Result<(), PlayerSetupError> {
        self.insert(player, credits, Some(roster))
    }

    fn insert(
        &mut self,
        player: PlayerId,
        credits: i64,
        roster: Option<BTreeSet<ProductionItem>>,
    ) -> Result<(), PlayerSetupError> {
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
                roster,
                ..PlayerState::default()
            },
        );
        Ok(())
    }

    pub fn get(&self, player: PlayerId) -> Option<&PlayerState> {
        self.players.get(&player)
    }

    /// Puts `player` in `team` (0: none) and returns whether the player exists. Scenario and
    /// match setup only, like [`Players::add`].
    pub fn set_team(&mut self, player: PlayerId, team: u8) -> bool {
        self.players
            .get_mut(&player)
            .map(|state| state.team = team)
            .is_some()
    }

    /// Gives `player` the country `country` of the rules file (`None`: no country) and returns
    /// whether the player exists. Scenario and match setup only, like [`Players::add`].
    pub fn set_country(
        &mut self,
        player: PlayerId,
        country: Option<crate::rules::CountryId>,
    ) -> bool {
        self.players
            .get_mut(&player)
            .map(|state| state.country = country)
            .is_some()
    }

    /// Whether `a` and `b` are enemies: different players that do not share a team. An owner who
    /// is not a player of the match is an enemy of everyone else, as before teams existed.
    pub fn hostile(&self, a: PlayerId, b: PlayerId) -> bool {
        if a == b {
            return false;
        }
        let team = |player| self.get(player).map_or(0, |state| state.team);
        let (a, b) = (team(a), team(b));
        a == 0 || a != b
    }

    /// The speed of `player`'s units in percent of their type's speed.
    pub fn unit_speed_percent(&self, player: PlayerId) -> u32 {
        self.get(player)
            .map_or(100, PlayerState::unit_speed_percent)
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
    /// and credits charged), then the vision (from M4), then the roster (from M7: a flag byte, and
    /// with a roster its length and each item's kind tag and index).
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.players.len() as u64);
        for (player, state) in &self.players {
            // One byte of match options; unchanged from earlier versions when unlock-all is off
            // and the player has neither a team nor a unit speed bonus.
            let extended = state.team != 0 || state.unit_speed_bonus != 0;
            hasher.write(&[
                player.0,
                u8::from(state.super_me)
                    | (u8::from(state.unlock_all) << 1)
                    | (u8::from(extended) << 2),
            ]);
            if extended {
                hasher.write(&[state.team]);
                hasher.write(&state.unit_speed_bonus.to_le_bytes());
            }
            hasher.write(&state.credits.to_le_bytes());
            hasher.write(&state.power_produced.to_le_bytes());
            hasher.write(&state.power_consumed.to_le_bytes());
            hasher.write_u64(state.queues.len() as u64);
            for (producer, items) in &state.queues {
                hasher.write(&producer.0.to_le_bytes());
                hasher.write_u64(items.len() as u64);
                for item in items {
                    let (tag, index) = item_key(item.item);
                    hasher.write(&[tag]);
                    hasher.write(&index.to_le_bytes());
                    hasher.write_u32(item.progress);
                    hasher.write(&item.charged.to_le_bytes());
                }
            }
            state.vision.hash_into(hasher);
            match &state.roster {
                None => hasher.write(&[0]),
                Some(roster) => {
                    hasher.write(&[1]);
                    hasher.write_u64(roster.len() as u64);
                    for item in roster {
                        let (tag, index) = item_key(*item);
                        hasher.write(&[tag]);
                        hasher.write(&index.to_le_bytes());
                    }
                }
            }
            hasher.write_u64(state.repairing.len() as u64);
            for id in &state.repairing {
                hasher.write_u32(id.0);
            }
            hasher.write(&[state.country.map_or(u8::MAX, |country| country.0)]);
        }
    }
}

/// The kind tag (0 building, 1 unit) and index of an item, as hashed.
fn item_key(item: ProductionItem) -> (u8, u16) {
    match item {
        ProductionItem::Building(kind) => (0, kind.0),
        ProductionItem::Unit(kind) => (1, kind.0),
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

/// Ticks between two repair steps of a building: four steps a second at 20 ticks a second.
pub const REPAIR_INTERVAL: u64 = 5;
/// Repair steps that take a building from no hit points to whole (20 seconds).
pub const REPAIR_STEPS: i32 = 80;
/// A complete repair from no hit points costs `1 / REPAIR_COST_DIVISOR` of the building's price.
pub const REPAIR_COST_DIVISOR: i64 = 4;

/// The credits charged for restoring `heal` of `max` hit points of a building of price `price`:
/// the proportional share of `price / REPAIR_COST_DIVISOR`, rounded up, and at least 1 credit for
/// a building with a price. Super Me charges at most 1 credit, as it does for production.
pub fn repair_cost(price: i64, heal: i32, max: i32, super_me: bool) -> i64 {
    if price <= 0 || heal <= 0 || max <= 0 {
        return 0;
    }
    let denominator = i64::from(max).saturating_mul(REPAIR_COST_DIVISOR);
    let cost = price
        .saturating_mul(i64::from(heal))
        .saturating_add(denominator - 1)
        / denominator;
    let cost = cost.max(1);
    if super_me { cost.min(1) } else { cost }
}

/// Repairs the buildings each player has ordered repaired (`RepairBuilding`), in id order: every
/// [`REPAIR_INTERVAL`] ticks a building regains `max / REPAIR_STEPS` hit points (at least 1, at
/// most what it lacks) for [`repair_cost`] credits. A step the player cannot pay for is skipped
/// and the repair waits for credits. A repair ends when the building is whole, destroyed, sold,
/// or no longer the player's (captured).
pub(crate) fn repair_buildings(
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    tick: Res<SimTick>,
    mut buildings: Query<(
        Entity,
        &SimId,
        &Owner,
        &Building,
        &mut crate::combat::Health,
    )>,
) {
    if players
        .players
        .values()
        .all(|state| state.repairing.is_empty())
    {
        return;
    }
    let step_now = tick.0.is_multiple_of(REPAIR_INTERVAL);
    let index: BTreeMap<SimId, Entity> = buildings
        .iter()
        .map(|(entity, id, ..)| (*id, entity))
        .collect();
    for (&player, state) in &mut players.players {
        let mut ended = Vec::new();
        for &id in &state.repairing {
            let Some(Ok((_, _, owner, building, mut health))) =
                index.get(&id).map(|&entity| buildings.get_mut(entity))
            else {
                ended.push(id);
                continue;
            };
            let max = definitions
                .building_combat(building.kind)
                .map_or(0, |terms| terms.hit_points);
            if owner.0 != player || health.hit_points <= 0 || health.hit_points >= max {
                ended.push(id);
                continue;
            }
            if !step_now {
                continue;
            }
            let heal = (max / REPAIR_STEPS)
                .max(1)
                .min(max.saturating_sub(health.hit_points));
            let price = definitions
                .building(building.kind)
                .map_or(0, |def| def.production.cost);
            let cost = repair_cost(price, heal, max, state.super_me);
            if state.credits < cost {
                continue;
            }
            state.credits -= cost;
            health.hit_points = health.hit_points.saturating_add(heal);
            if health.hit_points >= max {
                ended.push(id);
            }
        }
        for id in ended {
            state.repairing.remove(&id);
        }
    }
}

/// Recomputes every player's power from the buildings it owns (first system of the `Economy`
/// stage, so buildings placed or sold by this tick's commands count).
pub(crate) fn update_power(
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    buildings: Query<(&Owner, &Building)>,
    special: Res<crate::SpecialState>,
    tick: Res<SimTick>,
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
        state.power_produced = if special.outages.get(player).is_some_and(|t| *t > tick.0) {
            0
        } else {
            produced
        };
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
        let domain = self.definitions.domain(kind);
        let cell = exit_cell(
            &self.grid.navigation_for(domain),
            exit.footprint,
            exit.rally,
        )?;
        let id = self.ids.allocate()?;
        self.grid.register_domain(id, domain);
        if !self.grid.claim(cell, id) {
            return None;
        }
        let mut bundle = unit_bundle(id, player, kind, speed, cell);
        let harvester = self.definitions.harvester(kind).is_some();
        if !harvester && let Some(rally) = exit.rally {
            let destination =
                destination_cells(&self.grid.navigation_for(domain), rally, 1, |other| {
                    self.grid
                        .occupant_for(other, domain)
                        .is_some_and(|holder| holder != id)
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
    special: Res<crate::SpecialState>,
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
        if !special.disabled.contains_key(&exit.building) {
            exits.entry((owner, kind)).or_default().push(exit);
        }
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
        let speed = state.production_speed();
        let super_me = state.super_me;
        let unlock_all = state.unlock_all;
        let producers: Vec<BuildingKind> = state.queues.keys().copied().collect();
        // The types of the player's buildings that work, one entry per building.
        let kinds: Vec<BuildingKind> = exits
            .iter()
            .filter(|((owner, _), _)| *owner == player)
            .flat_map(|((_, kind), list)| std::iter::repeat_n(*kind, list.len()))
            .collect();
        for producer in producers {
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
            // The player's buildings that produce the item, in id order: those of the producer's
            // type, or with factory groups (RA1) the factories of its group whose side builds
            // the item.
            let mut owned: Vec<Exit> = exits
                .iter()
                .filter(|((owner, kind), _)| {
                    *owner == player && produces(&definitions, *kind, head.item)
                })
                .flat_map(|(_, list)| list.iter().copied())
                .collect();
            owned.sort_unstable_by_key(|exit| exit.building);
            if head.progress < terms.build_time {
                if owned.is_empty()
                    || !unlock_all
                        && missing_prerequisite(&definitions, &kinds, head.item).is_some()
                    || (low_power && !even_tick)
                {
                    continue;
                }
                let next = head.progress.saturating_add(speed).min(terms.build_time);
                let charge = if super_me {
                    i64::from(next == terms.build_time)
                } else {
                    charge_for_tick(terms.cost, terms.build_time, head.progress)
                };
                if *credits < charge {
                    continue;
                }
                *credits -= charge;
                head.charged = head.charged.saturating_add(charge);
                head.progress = next;
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
