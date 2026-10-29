//! Players, credits, power, and production (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md
//! D25).
//!
//! Each player has credits, the power its buildings produce and consume, and one production
//! queue per producing building type, or per factory group where the arsenal defines one (RA1,
//! `Definitions::queue_key`). Only the item at the head of a queue progresses, and only while the
//! player owns a building that produces it (`economy_commands::produces`) and its
//! prerequisites. On each tick in which it progresses, an
//! item of cost `C` and build time `T` is charged `C / T` credits, and on its last tick
//! `C / T + C mod T`, so the total charged is exactly `C` (equation 3, invariant I7). An item
//! does not progress on a tick in which the player's credits are below that tick's charge, so
//! credits never fall below zero, and under low power (consumed > produced) it progresses only on
//! even ticks. Cancelling refunds exactly what was charged.
//!
//! With a rules file (RA2, docs/RA2_PARITY_PLAN.md section 4.2; docs/DECISIONS.md R22) the head
//! of a queue advances by a rate instead of one unit per tick: 1000 permille of a unit per tick,
//! times 1000 / `multiple_factory_pm` for every further working factory of the item's group up
//! to [`MAX_COUNTED_FACTORIES`] ([`factory_rate_pm`]), times the low power factor
//! ([`power_rate_pm`]: between `min_low_power_speed_pm` and `max_low_power_speed_pm` while the
//! player is short of power), or with Super Me [`SUPER_ME_PRODUCTION_FACTOR`] times the factory
//! rate whatever the power (docs/DECISIONS.md R211). The rate is added to the item's
//! [`QueueItem::fraction`] every tick; each whole unit is one unit of progress, charged as
//! above, so equation 3 and invariant I7 hold with every rate; a unit the player cannot pay
//! ends the tick's progress, and the fraction below one unit is kept. The Construction Yard has
//! two lanes there, buildings and defences ([`Lane`]), which progress side by side and hold one
//! item each; a queue of units holds one item in production and `max_queued_objects` behind it
//! (`Definitions::queue_capacity`). Without a rules file a queue holds [`MAX_QUEUE_LENGTH`]
//! items, a producer has one queue, and low power halves production by the even tick rule.
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
use crate::economy_commands::{missing_prerequisite_for, produces_for};
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

/// Largest number of items in one production queue of a world without a rules file (with one:
/// `Definitions::queue_capacity`).
pub const MAX_QUEUE_LENGTH: usize = 16;

/// Largest number of factories of one group that speed its production up (RA2). The ini knows
/// no cap; the game counts at most this many so that the rate is bounded (assumption,
/// docs/RA2_PARITY_PLAN.md section 4.2).
pub const MAX_COUNTED_FACTORIES: usize = 10;

/// Largest production rate in permille of a progress unit per tick, whatever the rules file
/// holds (invariant I12).
pub const MAX_PRODUCTION_RATE_PM: u64 = 1_000_000_000;

/// How many times faster than normal a Super Me player produces (docs/DECISIONS.md R211). Low
/// power does not slow a Super Me player down.
pub const SUPER_ME_PRODUCTION_FACTOR: u32 = 30;

/// The lane of a queue (RA2, docs/RA2_PARITY_PLAN.md section 5, RA2 item 3). With a rules file
/// the buildings of the Construction Yard are produced in two lanes that progress side by side:
/// the defences (the arsenal's category `armory`, the ini's `BuildCat=Combat`) and everything
/// else. Units, and every item of a world without a rules file, are in the main lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lane {
    Main,
    Defences,
    /// The placed Super Me units (docs/DECISIONS.md R218): a complete one waits here for
    /// `PlaceUnit` without holding up the buildings of its producer.
    Placed,
}

/// What a player's queue is kept under: the building type of its producer, or the first
/// building type of the producer's factory group ([`Definitions::queue_key`]), and the lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueueKey {
    pub producer: BuildingKind,
    pub lane: Lane,
}

/// An item in a production queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueItem {
    pub item: ProductionItem,
    /// Ticks of progress made, from 0 to the item's build time.
    pub progress: u32,
    /// Credits charged so far, from 0 to the item's cost.
    pub charged: i64,
    /// Permille of a unit of progress carried over to the next tick, from 0 to 999 (RA2: the
    /// production rate is a permille per tick). Always 0 in a world without a rules file.
    pub fraction: u32,
}

impl QueueItem {
    /// `item` as it enters a queue: no progress, nothing charged.
    pub fn new(item: ProductionItem) -> Self {
        Self {
            item,
            progress: 0,
            charged: 0,
            fraction: 0,
        }
    }
}

/// The economic state of one player.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerState {
    /// Zero is the neutral scenario default; actual AI houses install ported rules at setup.
    pub(crate) ai_rof_pm: u32,
    pub(crate) ai_build_time_pm: u32,
    pub(crate) ai_virtual_purifiers: u32,
    /// Persistent infiltration benefits: infantry, vehicles, Allied tech, Soviet tech.
    pub infiltration: u8,
    pub(crate) limited_units: BTreeMap<UnitKind, u32>,
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
    /// One queue per producing building type and lane; queues that become empty are removed.
    pub(crate) queues: BTreeMap<QueueKey, Vec<QueueItem>>,
    /// Whether the player has radar, as of the last `Economy` stage ([`PlayerState::radar`]).
    pub(crate) radar: bool,
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
    /// The aircraft the player owns that need a dock, as of the last `Economy` stage (RA4,
    /// `crate::flight`).
    pub(crate) aircraft: u32,
    /// The docks of the player's buildings, as of the last `Economy` stage.
    pub(crate) docks: u32,
    /// Whether a Super Me unit with `unlimited_power` of the player stands, as of the last
    /// `Economy` stage (docs/DECISIONS.md R218); the player is then never low on power.
    pub(crate) unlimited_power: bool,
    /// The cells of the player's Super Me units with a build radius and that radius, as of the
    /// last `Economy` stage: buildings may be placed within it ([`check_placement`]).
    ///
    /// [`check_placement`]: crate::check_placement
    pub(crate) build_anchors: Vec<(CellPos, u32)>,
}

impl PlayerState {
    pub fn ai_reload_pm(&self) -> u32 {
        if self.ai_rof_pm == 0 {
            1000
        } else {
            self.ai_rof_pm
        }
    }
    pub fn ai_build_time_pm(&self) -> u32 {
        if self.ai_build_time_pm == 0 {
            1000
        } else {
            self.ai_build_time_pm
        }
    }
    pub fn ai_virtual_purifiers(&self) -> u32 {
        self.ai_virtual_purifiers
    }
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
    /// The aircraft the player owns that need a dock, as of the last `Economy` stage (RA4).
    pub fn docked_aircraft(&self) -> u32 {
        self.aircraft
    }
    /// The docks of the player's buildings, as of the last `Economy` stage (RA4).
    pub fn docks(&self) -> u32 {
        self.docks
    }
    /// Whether a Super Me unit gives the player unlimited power, as of the last `Economy` stage
    /// (docs/DECISIONS.md R218).
    pub fn unlimited_power(&self) -> bool {
        self.unlimited_power
    }
    /// The cells of the player's Super Me units near which buildings may be placed, each with
    /// its reach in cells, as of the last `Economy` stage (docs/DECISIONS.md R218).
    pub fn build_anchors(&self) -> &[(CellPos, u32)] {
        &self.build_anchors
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
        if self.super_me {
            SUPER_ME_PRODUCTION_FACTOR
        } else {
            1
        }
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

    /// Whether the player has radar (RA2, the minimap of the game): with a rules file, while it
    /// owns a working radar building (the arsenal's `radar`), is not low on power and suffers no
    /// power outage from a spy; always in a world without a rules file. As of the last
    /// `Economy` stage ([`update_power`]).
    pub fn radar(&self) -> bool {
        self.radar
    }

    /// Whether power lets a building of type `kind` of this player work. Without a rules file:
    /// while the player is not low on power (the rule of M3 to M9). With one (RA2): always,
    /// unless the player is low on power and the building is `powered` and consumes power
    /// (docs/RA2_PARITY_PLAN.md section 4.2: a Pillbox and a Sentry Gun keep firing).
    pub fn powers(&self, definitions: &Definitions, kind: BuildingKind) -> bool {
        if !self.low_power() {
            return true;
        }
        if definitions.rules().is_none() {
            return false;
        }
        let drains = definitions
            .building(kind)
            .is_some_and(|def| def.power_consumed > 0);
        let powered = definitions
            .metadata(ProductionItem::Building(kind))
            .is_some_and(|entry| entry.powered);
        !(powered && drains)
    }

    /// The main lane of the queue kept under building type `producer`, head first. A queue is
    /// kept under the producer of its items, or under the first building type of the producer's
    /// factory group where the arsenal defines groups ([`Definitions::queue_key`]), and under
    /// the lane of its items ([`PlayerState::lane`], [`Definitions::queue_of`]).
    pub fn queue(&self, producer: BuildingKind) -> &[QueueItem] {
        self.lane(QueueKey {
            producer,
            lane: Lane::Main,
        })
    }

    /// The queue kept under `key`, head first.
    pub fn lane(&self, key: QueueKey) -> &[QueueItem] {
        self.queues.get(&key).map_or(&[], Vec::as_slice)
    }

    /// The player's visibility grid as of the last `Visibility` stage (shroud and fog,
    /// docs/DECISIONS.md D30).
    pub fn vision(&self) -> &Vision {
        &self.vision
    }

    /// Whether the player's faction may produce `item` (every item for a player without a
    /// faction). A Super Me player is offered every unit of both sides (docs/DECISIONS.md R213).
    pub fn offers(&self, item: ProductionItem) -> bool {
        (self.super_me && matches!(item, ProductionItem::Unit(_)))
            || self
                .roster
                .as_ref()
                .is_none_or(|roster| roster.contains(&item))
    }

    /// The roster of the player's faction, or `None` for a player added without one.
    pub fn roster(&self) -> Option<&BTreeSet<ProductionItem>> {
        self.roster.as_ref()
    }

    /// Every non-empty queue, by producing building type and lane.
    pub fn queues(&self) -> impl Iterator<Item = (QueueKey, &[QueueItem])> {
        self.queues
            .iter()
            .map(|(&key, items)| (key, items.as_slice()))
    }
}

/// Why a player could not be added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerSetupError {
    NegativeCredits(i64),
    ReservedNeutral,
    AlreadyAdded(PlayerId),
}

impl fmt::Display for PlayerSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedNeutral => write!(f, "the neutral owner cannot compete"),
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
        if player == crate::NEUTRAL {
            return Err(PlayerSetupError::ReservedNeutral);
        }
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
        if a == b || a == crate::NEUTRAL || b == crate::NEUTRAL {
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
    /// (producing building type, lane, item count, and per item its kind tag and index, progress,
    /// credits charged and fraction), then the vision (from M4), then the roster (from M7: a flag
    /// byte, and with a roster its length and each item's kind tag and index), the buildings
    /// being repaired, the country (from RA1: its index, or 255 for none), and the radar (RA2).
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
            if state.infiltration != 0 || !state.limited_units.is_empty() {
                hasher.write(b"infiltration");
                hasher.write(&[state.infiltration]);
                hasher.write_u64(state.limited_units.len() as u64);
                for (kind, count) in &state.limited_units {
                    hasher.write(&kind.0.to_le_bytes());
                    hasher.write_u32(*count);
                }
            }
            if state.ai_rof_pm != 0
                || state.ai_build_time_pm != 0
                || state.ai_virtual_purifiers != 0
            {
                hasher.write(b"ai_difficulty");
                hasher.write_u32(state.ai_rof_pm);
                hasher.write_u32(state.ai_build_time_pm);
                hasher.write_u32(state.ai_virtual_purifiers);
            }
            hasher.write(&state.credits.to_le_bytes());
            hasher.write(&state.power_produced.to_le_bytes());
            hasher.write(&state.power_consumed.to_le_bytes());
            hasher.write_u64(state.queues.len() as u64);
            for (key, items) in &state.queues {
                hasher.write(&key.producer.0.to_le_bytes());
                hasher.write(&[key.lane as u8]);
                hasher.write_u64(items.len() as u64);
                for item in items {
                    let (tag, index) = item_key(item.item);
                    hasher.write(&[tag]);
                    hasher.write(&index.to_le_bytes());
                    hasher.write_u32(item.progress);
                    hasher.write(&item.charged.to_le_bytes());
                    hasher.write_u32(item.fraction);
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
            hasher.write(&[
                state.country.map_or(u8::MAX, |country| country.0),
                u8::from(state.radar),
            ]);
            // RA4: written only for a player that has such aircraft or docks, so that a world
            // without them hashes as before.
            if state.aircraft != 0 || state.docks != 0 {
                hasher.write(b"ac");
                hasher.write_u32(state.aircraft);
                hasher.write_u32(state.docks);
            }
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

/// The production rate of `factories` working factories of one group, in permille of a unit of
/// progress per tick (RA2, docs/RA2_PARITY_PLAN.md section 4.2): 1000 with one factory, and for
/// every further one, up to [`MAX_COUNTED_FACTORIES`], times 1000 / `multiple_factory_pm` (the
/// ini's `MultipleFactory=0.8` is a build time factor), rounded down once:
/// `1000 x 1000^(n-1) div multiple_factory_pm^(n-1)`, which is `1000 x 5^(n-1) div 4^(n-1)` for
/// the value of Red Alert 2. At most [`MAX_PRODUCTION_RATE_PM`].
pub fn factory_rate_pm(rules: &crate::rules::ProductionRules, factories: usize) -> u64 {
    let factor = u128::from(rules.multiple_factory_pm.clamp(1, 1000).unsigned_abs());
    let (mut numerator, mut denominator) = (1000_u128, 1_u128);
    // At most 1000^10 and 1000^9, far below `u128::MAX`.
    for _ in 1..factories.clamp(1, MAX_COUNTED_FACTORIES) {
        numerator *= 1000;
        denominator *= factor;
    }
    u64::try_from(numerator / denominator)
        .unwrap_or(MAX_PRODUCTION_RATE_PM)
        .min(MAX_PRODUCTION_RATE_PM)
}

/// The factor of the production rate of a player whose buildings produce `produced` and consume
/// `consumed` power, in permille (RA2, section 4.2): 1000 while the power suffices; otherwise
/// `1000 - low_power_penalty_pm x shortage`, where the shortage is `1 - produced / consumed`
/// (assumption: the ini does not define it), clamped to `min_low_power_speed_pm` and
/// `max_low_power_speed_pm`: with the values of Red Alert 2, 800 for a shortage of up to a
/// fifth, 500 for a half or more.
pub fn power_rate_pm(rules: &crate::rules::ProductionRules, produced: i32, consumed: i32) -> u64 {
    if consumed <= produced {
        return 1000;
    }
    // `consumed` is above `produced`, so above 0 unless `produced` is negative, which no
    // building's power is.
    let ratio = (i64::from(produced.max(0)) * 1000 / i64::from(consumed.max(1))).min(1000);
    let penalty = i64::from(rules.low_power_penalty_pm) * (1000 - ratio) / 1000;
    let (min, max) = (
        i64::from(rules.min_low_power_speed_pm),
        i64::from(rules.max_low_power_speed_pm),
    );
    u64::try_from((1000 - penalty).clamp(min, max.max(min))).unwrap_or(0)
}

/// The production rate of a player in permille of a unit of progress per tick (RA2, section
/// 4.2): [`factory_rate_pm`] times [`power_rate_pm`], rounded down; with Super Me
/// [`factory_rate_pm`] times [`SUPER_ME_PRODUCTION_FACTOR`], whatever the power.
pub fn production_rate_pm(
    rules: &crate::rules::ProductionRules,
    factories: usize,
    produced: i32,
    consumed: i32,
    super_me: bool,
) -> u64 {
    let rate = if super_me {
        factory_rate_pm(rules, factories).saturating_mul(u64::from(SUPER_ME_PRODUCTION_FACTOR))
    } else {
        factory_rate_pm(rules, factories) * power_rate_pm(rules, produced, consumed) / 1000
    };
    rate.min(MAX_PRODUCTION_RATE_PM)
}

/// Advances `head`, an item of cost `cost` and build time `build_time` that is not complete, by
/// one tick at `rate_pm` permille of a unit per tick, paying from `credits` (RA2, section 4.2).
/// The rate is added to the item's fraction; every whole unit is one unit of progress, charged
/// as [`charge_for_tick`] charges it (with Super Me: nothing, and 1 credit for the last unit).
/// Progress ends at the first unit the credits do not pay for; the fraction below one unit is
/// kept either way, and whole units that were not made are not carried over.
pub(crate) fn advance_item(
    head: &mut QueueItem,
    cost: i64,
    build_time: u32,
    rate_pm: u64,
    super_me: bool,
    credits: &mut i64,
) {
    let total = u64::from(head.fraction).saturating_add(rate_pm);
    // Below 1000.
    head.fraction = (total % 1000) as u32;
    let left = build_time.saturating_sub(head.progress);
    let wanted = u32::try_from(total / 1000).unwrap_or(u32::MAX).min(left);
    if wanted == 0 {
        return;
    }
    let time = i64::from(build_time.max(1));
    let (step, last) = if super_me {
        (0, 1)
    } else {
        (cost / time, cost / time + cost % time)
    };
    // Every unit but the last of the item costs `step`; the last one `last`.
    let finishes = wanted == left;
    let plain = wanted - u32::from(finishes);
    let paid = if step <= 0 {
        plain
    } else {
        plain.min(u32::try_from((*credits).max(0) / step).unwrap_or(u32::MAX))
    };
    let charge = step.max(0).saturating_mul(i64::from(paid));
    *credits -= charge;
    head.charged = head.charged.saturating_add(charge);
    head.progress = head.progress.saturating_add(paid);
    if finishes && paid == plain && *credits >= last {
        *credits -= last;
        head.charged = head.charged.saturating_add(last);
        head.progress = head.progress.saturating_add(1);
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

/// The credits one repair step costs with a rules file (RA2, docs/RA2_PARITY_PLAN.md section
/// 4.6), for a building or a vehicle of price `price` and `strength` hit points when whole:
/// `max(1, (price div max(strength div repair_step, 1)) x repair_percent div 100)`, so that a
/// repair from nothing costs about `repair_percent` of the price. Super Me charges at most 1
/// credit, as it does for production.
pub fn repair_step_cost(
    rules: &crate::rules::EconomyRules,
    price: i64,
    strength: i32,
    super_me: bool,
) -> i64 {
    let steps = i64::from((strength / rules.repair_step.max(1)).max(1));
    let cost = (price.max(0) / steps).saturating_mul(i64::from(rules.repair_percent)) / 100;
    let cost = cost.max(1);
    if super_me { cost.min(1) } else { cost }
}

/// Repairs the buildings each player has ordered repaired (`RepairBuilding`), in id order: every
/// [`REPAIR_INTERVAL`] ticks a building regains `max / REPAIR_STEPS` hit points (at least 1, at
/// most what it lacks) for [`repair_cost`] credits. With a rules file (RA2): every
/// `repair_interval` ticks it regains `repair_step` hit points (at most what it lacks) for
/// [`repair_step_cost`] credits. A step the player cannot pay for is skipped and the repair
/// waits for credits. A repair ends when the building is whole, destroyed, sold, or no longer
/// the player's (captured).
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
    let rules = definitions.rules().map(|rules| rules.economy);
    let interval = rules.map_or(REPAIR_INTERVAL, |rules| {
        u64::from(rules.repair_interval.max(1).unsigned_abs())
    });
    let step_now = tick.0.is_multiple_of(interval);
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
            let lacking = max.saturating_sub(health.hit_points);
            let price = definitions
                .building(building.kind)
                .map_or(0, |def| def.production.cost);
            let (heal, cost) = match &rules {
                Some(rules) => (
                    rules.repair_step.max(1).min(lacking),
                    repair_step_cost(rules, price, max, state.super_me),
                ),
                None => {
                    let heal = (max / REPAIR_STEPS).max(1).min(lacking);
                    (heal, repair_cost(price, heal, max, state.super_me))
                }
            };
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
/// stage, so buildings placed or sold by this tick's commands count), and from it the player's
/// radar ([`PlayerState::radar`]): with a rules file a player has radar while it owns a radar
/// building that a spy has not switched off, is not low on power, and suffers no power outage.
pub(crate) fn update_power(
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    buildings: Query<(&SimId, &Owner, &Building, Option<&crate::Health>)>,
    units: Query<(&Owner, &crate::Unit, &crate::Mobile, Option<&crate::Health>)>,
    special: Res<crate::SpecialState>,
    tick: Res<SimTick>,
) {
    let mut totals: BTreeMap<PlayerId, (i32, i32)> = BTreeMap::new();
    let mut radars: BTreeSet<PlayerId> = BTreeSet::new();
    for (id, owner, building, health) in &buildings {
        if definitions.rules().is_some() && health.is_some_and(|h| h.hit_points <= 0) {
            continue;
        }
        let Some(def) = definitions.building(building.kind) else {
            continue;
        };
        if definitions
            .metadata(ProductionItem::Building(building.kind))
            .is_some_and(|entry| entry.radar)
            && !special
                .disabled
                .get(id)
                .is_some_and(|until| *until > tick.0)
        {
            radars.insert(owner.0);
        }
        let total = totals.entry(owner.0).or_default();
        // Bounded far below `i32::MAX` (economy_defs::MAX_POWER); saturating for invariant I12.
        total.0 = total.0.saturating_add(def.power_produced);
        total.1 = total.1.saturating_add(def.power_consumed);
    }
    let storm_suppressed: BTreeSet<_> = special
        .strategic
        .storm
        .as_ref()
        .filter(|storm| tick.0 >= storm.starts && tick.0 < storm.ends)
        .map(|storm| {
            players
                .iter()
                .map(|(p, _)| p)
                .filter(|p| players.hostile(*p, storm.owner))
                .collect()
        })
        .unwrap_or_default();
    // The Super Me units (docs/DECISIONS.md R218) grant their powers only to a Super Me owner.
    let mut unlimited: BTreeSet<PlayerId> = BTreeSet::new();
    let mut anchors: BTreeMap<PlayerId, Vec<(CellPos, u32)>> = BTreeMap::new();
    if !definitions.super_me_units().is_empty() {
        for (owner, unit, mobile, health) in &units {
            if health.is_some_and(|h| h.hit_points <= 0) {
                continue;
            }
            let Some(grants) = definitions.super_me_unit(unit.kind) else {
                continue;
            };
            if !players.get(owner.0).is_some_and(PlayerState::super_me) {
                continue;
            }
            if grants.unlimited_power {
                unlimited.insert(owner.0);
            }
            if grants.build_radius > 0 {
                anchors
                    .entry(owner.0)
                    .or_default()
                    .push((mobile.cell, grants.build_radius));
            }
        }
    }
    for (player, state) in &mut players.players {
        let (produced, consumed) = totals.get(player).copied().unwrap_or_default();
        let outage = special.outages.get(player).is_some_and(|t| *t > tick.0);
        state.unlimited_power = unlimited.contains(player);
        state.build_anchors = anchors.remove(player).unwrap_or_default();
        // Unlimited power covers every consumer, and a spy's outage does not cut it.
        state.power_produced = if state.unlimited_power {
            produced.max(consumed)
        } else if outage {
            0
        } else {
            produced
        };
        state.power_consumed = consumed;
        state.radar = definitions.rules().is_none()
            || (radars.contains(player)
                && (!outage || state.unlimited_power)
                && !state.low_power()
                && !storm_suppressed.contains(player));
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
        let on_foot = self.definitions.on_foot(kind);
        let cell = exit_cell(
            &self.grid.navigation_for(domain),
            exit.footprint,
            exit.rally,
            on_foot,
        )?;
        let id = self.ids.allocate()?;
        self.grid.register(id, domain, on_foot);
        if !self.grid.claim(cell, id) {
            self.grid.unregister(id);
            return None;
        }
        let position = self.grid.standing_point(cell, id);
        let mut bundle = unit_bundle(id, player, kind, speed, cell, position);
        let harvester = self.definitions.harvester(kind).is_some();
        if !harvester && let Some(rally) = exit.rally {
            let destination =
                destination_cells(&self.grid.navigation_for(domain), rally, 1, |other| {
                    !self.grid.room_for(other, id)
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

/// Advances the head of every production queue by one tick (equation 3; with a rules file by
/// the player's production rate, [`production_rate_pm`]), lets complete units leave their
/// producing buildings, and lets the free units of newly placed buildings leave them. Players
/// are processed in id order and queues in order of their building type and lane, so the result
/// is deterministic (invariant I4).
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
    healths: Query<&crate::Health>,
    special: Res<crate::SpecialState>,
) {
    // The buildings each player owns of each type, in id order.
    let mut exits: BTreeMap<(PlayerId, BuildingKind), Vec<Exit>> = BTreeMap::new();
    let mut sorted: Vec<(SimId, Entity, PlayerId, BuildingKind, Exit)> = buildings
        .iter()
        .filter_map(|(entity, id, owner, building, producer)| {
            if healths
                .get(entity)
                .is_ok_and(|health| health.hit_points <= 0)
            {
                return None;
            }
            Some((
                *id,
                entity,
                owner.0,
                building.kind,
                Exit {
                    building: *id,
                    footprint: building.footprint,
                    rally: producer.and_then(|producer| producer.rally),
                },
            ))
        })
        .collect();
    sorted.sort_unstable_by_key(|(id, ..)| *id);
    for &(_, _, owner, kind, exit) in &sorted {
        if !special.disabled.contains_key(&exit.building)
            && !special.erasures.contains_key(&exit.building)
        {
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
    let rates = definitions.rules().map(|rules| rules.production);
    for (&player, state) in &mut players.players {
        let low_power = state.low_power();
        let speed = state.production_speed();
        let super_me = state.super_me;
        let country = state.country;
        let ai_build_time_pm = state.ai_build_time_pm();
        let unlock_all = state.unlock_all;
        let (produced, consumed) = (state.power_produced, state.power_consumed);
        let producers: Vec<QueueKey> = state.queues.keys().copied().collect();
        // The types of the player's buildings that work, one entry per building.
        let kinds: Vec<BuildingKind> = exits
            .iter()
            .filter(|((owner, _), _)| *owner == player)
            .flat_map(|((_, kind), list)| std::iter::repeat_n(*kind, list.len()))
            .collect();
        // Erasure suspends active factories, not a living building's passive
        // prerequisite contribution. Preserve the existing disabled-building policy.
        let prerequisites: Vec<BuildingKind> = sorted
            .iter()
            .filter(|(_, _, owner, _, exit)| {
                *owner == player && !special.disabled.contains_key(&exit.building)
            })
            .map(|(_, _, _, kind, _)| *kind)
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
            // the item (with Super Me, any side for a unit).
            let mut owned: Vec<Exit> = exits
                .iter()
                .filter(|((owner, kind), _)| {
                    *owner == player && produces_for(&definitions, super_me, *kind, head.item)
                })
                .flat_map(|(_, list)| list.iter().copied())
                .collect();
            owned.sort_unstable_by_key(|exit| exit.building);
            if head.progress < terms.build_time {
                if owned.is_empty()
                    || !unlock_all
                        && missing_prerequisite_for(
                            &definitions,
                            super_me,
                            country,
                            &prerequisites,
                            head.item,
                        )
                        .is_some()
                {
                    continue;
                }
                match &rates {
                    // RA2: by the rate of the working factories of the item's group and of the
                    // player's power.
                    Some(rates) => {
                        let factories = kinds
                            .iter()
                            .filter(|&&kind| definitions.same_factory(kind, terms.producer))
                            .count();
                        let rate =
                            production_rate_pm(rates, factories, produced, consumed, super_me)
                                .saturating_mul(1000)
                                / u64::from(ai_build_time_pm);
                        advance_item(head, terms.cost, terms.build_time, rate, super_me, credits);
                    }
                    // Without a rules file: one unit per tick, on even ticks under low power
                    // (Super Me: `speed` units on every tick, whatever the power).
                    None => {
                        if low_power && !even_tick && !super_me {
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
                    }
                }
                if head.progress < terms.build_time {
                    continue;
                }
                spawner.events.write(SimEvent::ProductionComplete {
                    player,
                    item: head.item,
                });
            }
            // RA4: an aircraft that needs a dock waits in its factory while every dock of the
            // player's working buildings is taken (one was lost while it was built).
            if crate::flight::needs_dock(&definitions, head.item)
                && state.aircraft >= crate::flight::docks_of(&definitions, kinds.iter().copied())
            {
                continue;
            }
            // Complete: a building waits for `PlaceBuilding` and a placed Super Me unit for
            // `PlaceUnit` (docs/DECISIONS.md R218); any other unit leaves the first of the
            // player's producing buildings (in id order) that has a free ring cell.
            if let ProductionItem::Unit(kind) = head.item
                && !definitions.placed_on_completion(head.item)
                && let Some(unit) = owned
                    .iter()
                    .find_map(|&exit| spawner.leave(player, kind, exit))
            {
                // Spy training is a production benefit. Free/refinery/crate/paradrop units
                // never enter this branch; the queued command runs after the spawn bundle.
                let eligible = definitions.rules().is_some()
                    && definitions.metadata(head.item).is_some_and(|m| {
                        (state.infiltration & 1 != 0 && m.infantry)
                            || (state.infiltration & 2 != 0
                                && m.category == crate::Category::Vehicles
                                && m.domain != crate::Domain::Naval
                                && definitions
                                    .metadata(ProductionItem::Building(terms.producer))
                                    .is_some_and(|p| {
                                        p.factory == Some(crate::FactoryGroup::Vehicles)
                                    }))
                    });
                if eligible {
                    spawner
                        .commands
                        .queue(move |world: &mut bevy_ecs::world::World| {
                            crate::promote(world, unit, crate::VETERAN);
                        });
                }
                queue.remove(0);
                if queue.is_empty() {
                    state.queues.remove(&producer);
                }
            }
        }
    }

    // Free units of buildings placed during the match leave them, oldest first.
    for (_, entity, owner, _, exit) in sorted {
        if special.erasures.contains_key(&exit.building) {
            continue;
        }
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

    /// A small deterministic generator for the property test below (splitmix64).
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound.max(1)
        }
    }

    /// One tick of an item at a rate, unit by unit: the definition that [`advance_item`]
    /// computes in closed form.
    fn advance_by_units(
        head: &mut QueueItem,
        cost: i64,
        build_time: u32,
        rate_pm: u64,
        super_me: bool,
        credits: &mut i64,
    ) {
        let total = u64::from(head.fraction) + rate_pm;
        head.fraction = (total % 1000) as u32;
        for _ in 0..total / 1000 {
            if head.progress >= build_time {
                break;
            }
            let charge = if super_me {
                i64::from(head.progress + 1 == build_time)
            } else {
                charge_for_tick(cost, build_time, head.progress)
            };
            if *credits < charge {
                break;
            }
            *credits -= charge;
            head.charged += charge;
            head.progress += 1;
        }
    }

    /// Invariant I7 with every rate (RA2): whatever the cost, the build time, the rate and the
    /// credits a player has from tick to tick, the credits never fall below zero, every tick
    /// charges what the units of progress it made cost by equation 3, and a complete item has
    /// been charged exactly its cost (1 credit with Super Me).
    #[test]
    fn an_item_is_charged_unit_by_unit_and_exactly_its_cost_at_every_rate() {
        let item = ProductionItem::Unit(UnitKind(0));
        let mut random = Random(2);
        let rates = [
            0,
            1,
            499,
            500,
            666,
            800,
            999,
            1000,
            1001,
            1250,
            1562,
            7450,
            10_000,
            74_500,
            MAX_PRODUCTION_RATE_PM,
        ];
        for case in 0..4000 {
            let cost = match case % 4 {
                0 => random.below(50) as i64,
                1 => random.below(5000) as i64,
                2 => MAX_COST - random.below(3) as i64,
                _ => random.below(MAX_COST as u64 + 1) as i64,
            };
            let build_time = match case % 3 {
                0 => 1 + random.below(40) as u32,
                1 => 1 + random.below(3000) as u32,
                _ => MAX_BUILD_TIME - random.below(3) as u32,
            };
            let super_me = case % 7 == 0;
            let rate = rates[random.below(rates.len() as u64) as usize];
            let rate = if case % 5 == 0 {
                1 + random.below(20_000)
            } else {
                rate
            };
            let (mut fast, mut slow) = (QueueItem::new(item), QueueItem::new(item));
            // Sometimes plenty of credits, sometimes a trickle.
            let mut credits = if case % 2 == 0 { MAX_COST * 2 } else { 0 };
            let price = if super_me { 1 } else { cost };
            for _ in 0..200 {
                if fast.progress >= build_time {
                    break;
                }
                if case % 2 == 1 {
                    credits += random.below((cost as u64 / 20).max(2)) as i64;
                }
                let (before, mut reference) = (credits, credits);
                let charged = fast.charged;
                advance_item(&mut fast, cost, build_time, rate, super_me, &mut credits);
                advance_by_units(&mut slow, cost, build_time, rate, super_me, &mut reference);
                assert_eq!(
                    fast, slow,
                    "case {case}: {cost} over {build_time} at {rate}"
                );
                assert_eq!(credits, reference, "case {case}");
                assert!(credits >= 0 && credits <= before, "case {case}");
                assert_eq!(fast.charged - charged, before - credits, "case {case}");
                assert!(fast.fraction < 1000 && fast.progress <= build_time);
                assert!(fast.charged <= price, "case {case}");
            }
            if fast.progress >= build_time {
                assert_eq!(fast.charged, price, "case {case}: {cost} over {build_time}");
            }
        }
    }

    /// The new state of RA2 is hashed (invariant I3): the fraction of a queued item, the lane
    /// of a queue, and the radar.
    #[test]
    fn fractions_lanes_and_radar_are_hashed() {
        let hash = |change: &dyn Fn(&mut PlayerState)| {
            let mut players = Players::default();
            players.add(PlayerId(0), 100).expect("a player");
            let state = players.get_mut(PlayerId(0)).expect("the player");
            state.queues.insert(
                QueueKey {
                    producer: BuildingKind(1),
                    lane: Lane::Main,
                },
                vec![QueueItem::new(ProductionItem::Building(BuildingKind(2)))],
            );
            change(state);
            let mut hasher = Fnv1a64::new();
            players.hash_into(&mut hasher);
            hasher.finish()
        };
        let plain = hash(&|_| {});
        assert_eq!(plain, hash(&|_| {}));
        let fraction = hash(&|state| {
            for items in state.queues.values_mut() {
                items[0].fraction = 250;
            }
        });
        let lane = hash(&|state| {
            let items = state.queues.pop_first().expect("a queue").1;
            state.queues.insert(
                QueueKey {
                    producer: BuildingKind(1),
                    lane: Lane::Defences,
                },
                items,
            );
        });
        let radar = hash(&|state| state.radar = true);
        let rof = hash(&|state| state.ai_rof_pm = 800);
        let build = hash(&|state| state.ai_build_time_pm = 800);
        let purifiers = hash(&|state| state.ai_virtual_purifiers = 4);
        let all = [plain, fraction, lane, radar, rof, build, purifiers];
        for (index, a) in all.iter().enumerate() {
            for b in &all[index + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
