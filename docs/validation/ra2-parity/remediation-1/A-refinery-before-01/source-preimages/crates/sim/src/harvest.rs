//! The harvester cycle (PLAN.md sections 5 and 7, M3, equation 4; docs/DECISIONS.md D26).
//!
//! Since RA5 (docs/RA2_PARITY_PLAN.md section 4.10, docs/DECISIONS.md R58) the ore unit is the
//! bail, a cell holds bails of ore or of gems ([`Grid::is_gems`]), and a harvester counts the
//! bails of gems in its load.
//!
//! A harvester alternates between ore and a refinery of its owner:
//!
//! 1. Standing at the centre of a cell with ore and not full, it works; after every
//!    `harvest_interval` ticks of work (1: every tick) it removes `harvest_rate` bails, or less
//!    if the cell holds less or the harvester has less room left, so a cell's ore falls exactly
//!    to zero and a load never exceeds the capacity.
//! 2. Otherwise, if it is not full, it moves to the nearest cell with ore that is reachable over
//!    passable terrain and neither held by another unit nor the target of another harvester.
//! 3. When it is full, or carries ore and no ore is left, it moves to the nearest free cell
//!    around a refinery of its owner (the refinery's ring, `Footprint::ring`) and, standing
//!    there, unloads: the refinery pays `credits_per_ore` for every bail of ore and
//!    `credits_per_gem` for every bail of gems, at once.
//!
//! A harvester whose refinery disappears (sold, or from M4 on destroyed) while it carries ore to
//! it turns to the nearest other refinery of its owner, or stops (`HarvestIdle::NoRefinery`). A
//! harvester with no reachable ore and nothing to unload stops (`HarvestIdle::NoOre`), and one
//! that sets out [`MAX_HARVEST_ATTEMPTS`] times without harvesting or unloading in between stops
//! (`HarvestIdle::Blocked`). A stopped harvester waits for a `Harvest` command; `Move` and `Stop`
//! also stop the cycle. Movement uses ordinary move orders (`movement`), so harvesters obey
//! cell reservation (invariant I6).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{Query, Res, ResMut};

use crate::building::{Building, Footprint};
use crate::command::PlayerId;
use crate::definitions::Definitions;
use crate::economy::Players;
use crate::event::{HarvestIdle, SimEvent};
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid};
use crate::pathfinding::nearest_cell;
use crate::schedule::SimTick;
use crate::unit::{Mobile, MoveOrder, Orders, Owner, Unit};

/// Times a harvester may set out (for ore or for a refinery) without harvesting or unloading in
/// between before it stops with `HarvestIdle::Blocked`.
pub const MAX_HARVEST_ATTEMPTS: u8 = 8;

/// What a harvester is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestState {
    /// Stopped; waits for a `Harvest` command.
    Idle,
    /// Decides at its next cell centre: harvest, look for ore, or go to a refinery.
    Seeking,
    /// Going to, or harvesting at, this ore cell.
    ToOre(CellPos),
    /// Taking its load to `refinery`, to unload standing at `dock` (a cell of its ring).
    ToRefinery { refinery: SimId, dock: CellPos },
}

/// A harvester unit's cycle (only units whose type is listed in the economy file's
/// `harvesters`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harvester {
    /// Explicitly assigned field, retained across deliveries.
    pub field: Option<CellPos>,
    /// Bails carried, of ore and of gems together, from 0 to the capacity.
    pub load: u32,
    pub state: HarvestState,
    /// Times it set out since it last harvested or unloaded.
    pub attempts: u8,
    /// How many of the bails carried are gems (RA5), from 0 to `load`.
    pub gems: u32,
    /// Ticks worked on the cell it stands on since it arrived or last removed ore (RA5), from
    /// 0 to its type's `harvest_interval` less one.
    pub work: u32,
}

impl Harvester {
    /// An empty harvester that starts its cycle.
    pub(crate) fn new() -> Self {
        Self {
            field: None,
            load: 0,
            state: HarvestState::Seeking,
            attempts: 0,
            gems: 0,
            work: 0,
        }
    }

    /// Stops the cycle (a `Move` or `Stop` command); the load is kept.
    pub(crate) fn stop(&mut self) {
        self.field = None;
        self.state = HarvestState::Idle;
        self.attempts = 0;
        self.work = 0;
    }

    /// Load, then a state tag (0 idle, 1 seeking, 2 to ore with the cell, 3 to a refinery with
    /// its id and the dock cell), then the attempts, the bails of gems and the ticks worked.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        match self.field {
            Some(cell) => {
                hasher.write(&[1]);
                hasher.write(&cell.x.to_le_bytes());
                hasher.write(&cell.y.to_le_bytes());
            }
            None => hasher.write(&[0]),
        }
        hasher.write_u32(self.load);
        match self.state {
            HarvestState::Idle => hasher.write(&[0]),
            HarvestState::Seeking => hasher.write(&[1]),
            HarvestState::ToOre(cell) => {
                hasher.write(&[2]);
                hasher.write(&cell.x.to_le_bytes());
                hasher.write(&cell.y.to_le_bytes());
            }
            HarvestState::ToRefinery { refinery, dock } => {
                hasher.write(&[3]);
                hasher.write_u32(refinery.0);
                hasher.write(&dock.x.to_le_bytes());
                hasher.write(&dock.y.to_le_bytes());
            }
        }
        hasher.write(&[self.attempts]);
        hasher.write_u32(self.gems);
        hasher.write_u32(self.work);
    }
}

/// A refinery: its owner, footprint, and payment per bail of ore and of gems.
#[derive(Clone, Copy)]
struct Refinery {
    owner: PlayerId,
    footprint: Footprint,
    credits_per_ore: i64,
    credits_per_gem: i64,
}

/// The cell with ore nearest to `start` (by path cost over passable terrain; `start` itself
/// counts) that is not in `claimed` and has room for `unit` (it is held by no other unit).
pub(crate) fn nearest_ore(
    grid: &Grid,
    start: CellPos,
    unit: SimId,
    claimed: &BTreeSet<CellPos>,
) -> Option<CellPos> {
    nearest_cell(grid, start, |cell| {
        grid.cell(cell).is_some_and(|terrain| terrain.ore > 0)
            && grid.is_passable(cell)
            && !claimed.contains(&cell)
            && grid.room_for(cell, unit)
    })
}

/// The cells claimed as ore targets by harvesters, other than `except`.
pub(crate) fn claimed_ore<'a>(
    harvesters: impl Iterator<Item = (SimId, &'a Harvester)>,
    except: Option<SimId>,
) -> BTreeSet<CellPos> {
    harvesters
        .filter(|(id, _)| Some(*id) != except)
        .filter_map(|(_, harvester)| match harvester.state {
            HarvestState::ToOre(cell) => Some(cell),
            _ => None,
        })
        .collect()
}

type HarvesterQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static Unit,
        &'static Mobile,
        &'static mut Orders,
        &'static mut Harvester,
    ),
>;

/// The tick's context for every harvester.
struct Context<'a> {
    grid: &'a mut Grid,
    players: &'a mut Players,
    events: &'a mut Messages<SimEvent>,
    refineries: &'a BTreeMap<SimId, Refinery>,
    claimed: &'a mut BTreeSet<CellPos>,
    tick: u64,
}

/// One harvester's state for this tick.
struct Worker<'a> {
    id: SimId,
    owner: PlayerId,
    cell: CellPos,
    capacity: u32,
    rate: u32,
    interval: u32,
    orders: &'a mut Orders,
    harvester: &'a mut Harvester,
}

impl Context<'_> {
    fn idle(&mut self, worker: &mut Worker<'_>, reason: HarvestIdle) {
        worker.harvester.state = HarvestState::Idle;
        worker.harvester.attempts = 0;
        worker.harvester.work = 0;
        worker.orders.current = None;
        self.events.write(SimEvent::HarvesterIdle {
            unit: worker.id,
            reason,
        });
    }

    /// Counts one more attempt and orders the move to `cell`; stops the harvester instead if it
    /// has used up its attempts.
    fn set_out(&mut self, worker: &mut Worker<'_>, cell: CellPos, state: HarvestState) {
        if worker.harvester.attempts >= MAX_HARVEST_ATTEMPTS {
            self.idle(worker, HarvestIdle::Blocked);
            return;
        }
        worker.harvester.attempts += 1;
        worker.harvester.work = 0;
        worker.harvester.state = state;
        if let HarvestState::ToOre(target) = state {
            self.claimed.insert(target);
        }
        worker.orders.current = (cell != worker.cell).then(|| MoveOrder::new(cell, self.tick));
    }

    /// The free ring cell of a refinery of the worker's owner nearest to it by path cost (a
    /// ring cell held by another unit if no ring cell is free), and that refinery (the one with
    /// the lowest id if the cell touches several).
    fn nearest_dock(&self, worker: &Worker<'_>) -> Option<(SimId, CellPos)> {
        let own: Vec<(SimId, Footprint)> = self
            .refineries
            .iter()
            .filter(|(_, refinery)| refinery.owner == worker.owner)
            .map(|(&id, refinery)| (id, refinery.footprint))
            .collect();
        if own.is_empty() || self.players.get(worker.owner).is_none() {
            return None;
        }
        let refinery_at = |cell: CellPos| {
            own.iter()
                .find(|(_, footprint)| footprint.distance_to_cell(cell) == 1)
                .map(|&(id, _)| id)
        };
        let grid = &*self.grid;
        let free = |cell: CellPos| grid.room_for(cell, worker.id);
        let dock = nearest_cell(grid, worker.cell, |cell| {
            refinery_at(cell).is_some() && free(cell)
        })
        .or_else(|| nearest_cell(grid, worker.cell, |cell| refinery_at(cell).is_some()))?;
        Some((refinery_at(dock)?, dock))
    }

    fn go_to_refinery(&mut self, worker: &mut Worker<'_>) {
        match self.nearest_dock(worker) {
            Some((refinery, dock)) => {
                self.set_out(worker, dock, HarvestState::ToRefinery { refinery, dock });
            }
            None => self.idle(worker, HarvestIdle::NoRefinery),
        }
    }

    /// Harvests where the worker stands, or looks for ore, or goes to a refinery.
    fn work(&mut self, worker: &mut Worker<'_>) {
        let load = worker.harvester.load;
        let room = worker.capacity.saturating_sub(load);
        let ore_here = self.grid.cell(worker.cell).map_or(0, |terrain| terrain.ore);
        if room > 0 && ore_here > 0 {
            worker.harvester.attempts = 0;
            worker.harvester.state = HarvestState::ToOre(worker.cell);
            self.claimed.insert(worker.cell);
            // One more tick of work on this cell; the ore comes off when the interval is full.
            worker.harvester.work = worker.harvester.work.saturating_add(1);
            if worker.harvester.work < worker.interval {
                return;
            }
            worker.harvester.work = 0;
            let gems = self.grid.is_gems(worker.cell);
            let taken = self.grid.take_ore(worker.cell, worker.rate.min(room));
            worker.harvester.load = load.saturating_add(taken);
            if gems {
                worker.harvester.gems = worker.harvester.gems.saturating_add(taken);
            }
            if worker.harvester.load >= worker.capacity {
                self.go_to_refinery(worker);
            }
            return;
        }
        if room == 0 {
            self.go_to_refinery(worker);
            return;
        }
        match nearest_ore(
            self.grid,
            worker.harvester.field.unwrap_or(worker.cell),
            worker.id,
            self.claimed,
        ) {
            Some(cell) => self.set_out(worker, cell, HarvestState::ToOre(cell)),
            None if load > 0 => self.go_to_refinery(worker),
            None => self.idle(worker, HarvestIdle::NoOre),
        }
    }

    /// Unloads at `refinery` if the worker stands in its ring; otherwise sets out for a
    /// refinery again.
    fn unload(&mut self, worker: &mut Worker<'_>, refinery: SimId) {
        let Some(&target) = self.refineries.get(&refinery) else {
            self.go_to_refinery(worker);
            return;
        };
        if target.owner != worker.owner || target.footprint.distance_to_cell(worker.cell) != 1 {
            self.go_to_refinery(worker);
            return;
        }
        let Some(player) = self.players.get_mut(worker.owner) else {
            self.idle(worker, HarvestIdle::NoRefinery);
            return;
        };
        let ore = worker.harvester.load;
        let gems = worker.harvester.gems.min(ore);
        // At most MAX_ORE_CAPACITY × MAX_CREDITS_PER_ORE = 10^8; saturating for invariant I12.
        let credits = i64::from(ore - gems)
            .saturating_mul(target.credits_per_ore)
            .saturating_add(i64::from(gems).saturating_mul(target.credits_per_gem));
        player.credits = player.credits.saturating_add(credits);
        worker.harvester.load = 0;
        worker.harvester.gems = 0;
        worker.harvester.work = 0;
        worker.harvester.attempts = 0;
        worker.harvester.state = HarvestState::Seeking;
        self.events.write(SimEvent::OreDelivered {
            player: worker.owner,
            harvester: worker.id,
            refinery,
            ore,
            gems,
            credits,
        });
    }
}

/// Advances every harvester's cycle by one tick, in ascending id order (invariant I4).
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters keep the independent live-health query explicit"
)]
pub(crate) fn harvest(
    mut grid: ResMut<Grid>,
    mut players: ResMut<Players>,
    definitions: Res<Definitions>,
    tick: Res<SimTick>,
    mut events: ResMut<Messages<SimEvent>>,
    mut harvesters: HarvesterQuery,
    buildings: Query<(Entity, &SimId, &Owner, &Building)>,
    healths: Query<&crate::Health>,
    special: Res<crate::SpecialState>,
) {
    let alive = |entity| {
        !healths
            .get(entity)
            .is_ok_and(|health| health.hit_points <= 0)
    };
    let refineries: BTreeMap<SimId, Refinery> = buildings
        .iter()
        .filter_map(|(entity, id, owner, building)| {
            if !alive(entity) {
                return None;
            }
            let def = definitions.building(building.kind)?;
            let credits_per_ore = def.credits_per_ore?;
            Some((
                *id,
                Refinery {
                    owner: owner.0,
                    footprint: building.footprint,
                    credits_per_ore,
                    credits_per_gem: def.credits_per_gem.unwrap_or(credits_per_ore),
                },
            ))
        })
        .collect();
    let mut order: Vec<(SimId, Entity)> = harvesters
        .iter()
        .filter(|(entity, ..)| alive(*entity))
        .map(|(entity, id, ..)| (*id, entity))
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_unstable();
    let mut claimed = claimed_ore(
        harvesters
            .iter()
            .filter(|(entity, ..)| alive(*entity))
            .map(|(_, id, .., harvester)| (*id, harvester)),
        None,
    );
    let mut context = Context {
        grid: &mut grid,
        players: &mut players,
        events: &mut events,
        refineries: &refineries,
        claimed: &mut claimed,
        tick: tick.0,
    };
    for (id, entity) in order {
        // A temporarily frozen worker keeps its load, job and partial work unchanged.
        if special.erasures.contains_key(&id) {
            continue;
        }
        let Ok((_, _, owner, unit, mobile, mut orders, mut harvester)) = harvesters.get_mut(entity)
        else {
            continue;
        };
        // Only harvester unit types carry the component, and definitions do not change during
        // a match.
        let Some(terms) = definitions.harvester(unit.kind) else {
            continue;
        };
        let mut worker = Worker {
            id,
            owner: owner.0,
            cell: mobile.cell,
            capacity: terms.capacity,
            rate: terms.harvest_rate,
            interval: terms.harvest_interval,
            orders: &mut orders,
            harvester: &mut harvester,
        };
        // A refinery that disappeared while the harvester carried ore to it: turn to another.
        if let HarvestState::ToRefinery { refinery, .. } = worker.harvester.state
            && !refineries
                .get(&refinery)
                .is_some_and(|target| target.owner == worker.owner)
        {
            worker.harvester.attempts = 0;
            // A new order replaces the one to the lost refinery; a unit between two cells
            // completes its step first (D19 item 3).
            worker.cell = mobile.next_centre();
            context.go_to_refinery(&mut worker);
            continue;
        }
        if mobile.step.is_some() || worker.orders.current.is_some() {
            continue;
        }
        match worker.harvester.state {
            HarvestState::Idle => {}
            HarvestState::ToRefinery { refinery, .. } => context.unload(&mut worker, refinery),
            HarvestState::Seeking | HarvestState::ToOre(_) => {
                if let HarvestState::ToOre(cell) = worker.harvester.state {
                    context.claimed.remove(&cell);
                    if cell != worker.cell {
                        worker.harvester.field = None;
                    }
                }
                context.work(&mut worker);
            }
        }
    }
}
