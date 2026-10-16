//! Aircraft ammunition and docks, and the missiles and aircraft that other units launch (RA4,
//! docs/RA2_PARITY_PLAN.md sections 4.6 and 4.8; docs/DECISIONS.md R49, R50).
//!
//! Only with a rules file, and only for types with the arsenal's traits; the state is in
//! [`SpecialState`]. [`advance`] runs in the `Economy` stage, [`fired`] for every shot of the
//! `Combat` stage.
//!
//! **Ammunition.** An aircraft of a type with `ammo` n fires n volleys (a volley is the whole
//! burst of its weapon) and then cannot fire: its attack ends.
//!
//! **Docks.** A building with `docks` n holds n `airport_bound` aircraft of its owner, each
//! above a cell of its footprint (the corners first). Such an aircraft has one dock: the one it
//! was produced for, or the nearest free one of its owner if that building is lost, sold or
//! captured. Whenever it has no order it flies there, and while it stands on its dock it
//! regains one volley every `reload_interval` ticks of the rules. Without any dock it is lost
//! as soon as it has no order (the ini's `AirportBound`): it is destroyed. An aircraft that is
//! not `airport_bound` needs no dock. To the rest of the simulation a docked aircraft is in the
//! air like every aircraft (an assumption recorded in R49).
//!
//! **Spawners.** A unit of a type with `spawns` carries `spawns_number` units of that type in
//! its [`Bay`]. Its launcher weapon fires only while one of them is ready, and each shot
//! launches those that are ready then, one every [`SPAWN_LAUNCH_GAP`] ticks
//! ([`SimEvent::UnitLaunched`]), while the spawner keeps the target; the shot itself deals
//! nothing. A launched unit is a unit of the air layer like any other (it
//! holds its cell, is seen, and anti air fire shoots it down), but it takes no commands and
//! what it destroys counts for the unit that launched it.
//!
//! * A **missile** (`missile`) flies to where its target was when it was launched, at its
//!   type's speed along the cells of its path (the pitch and turn of the original's flight are
//!   not simulated), and goes off there with the combat file's effect of its kind, the elite
//!   one if its launcher was elite, multiplied by its launcher's firepower
//!   ([`SimEvent::MissileDetonated`]). If its way ends elsewhere it goes off where it is. Shot
//!   down, it deals nothing. Its place in the bay is filled again `spawn_regen` ticks after it
//!   went off or was shot down. A missile in flight outlives its launcher.
//! * Any other spawned unit is an **aircraft** with weapons and ammunition of its own: it
//!   attacks the target, and when its ammunition is spent or the target is gone it flies back
//!   and is taken aboard within one cell of its spawner ([`SimEvent::UnitRecovered`]); it is
//!   ready again `spawn_reload` ticks later, whole. One that is shot down is replaced
//!   `spawn_regen` ticks later. It has the rank of its spawner when it is launched, and it is
//!   destroyed with its spawner.

use std::collections::BTreeMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;

use crate::arsenal::{Domain, Launcher};
use crate::building::Footprint;
use crate::combat::{AttackOrder, Blast, Health, Weapon, detonate};
use crate::combat_defs::WeaponKind;
use crate::command::PlayerId;
use crate::definitions::Definitions;
use crate::economy::Players;
use crate::economy_defs::ProductionItem;
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid, SubPos};
use crate::special::{Actor, SpecialState, clear_orders, nearest_cell_of, release};
use crate::unit::{Mobile, MoveOrder, Orders, Position};

/// Ticks between two launches of one spawner (the 20 frames the original engine waits; it is
/// in no ini, docs/DECISIONS.md R50).
pub const SPAWN_LAUNCH_GAP: u32 = 27;

/// One of the units a spawner carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Aboard and ready to be launched.
    Ready,
    /// Launched: the unit with this id.
    Out(SimId),
    /// Aboard again, ready in this many ticks.
    Reloading(u32),
    /// Lost: replaced in this many ticks.
    Lost(u32),
}

/// What a spawner carries, and what it launches at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bay {
    pub slots: Vec<Slot>,
    /// The target its launcher fired at, while the spawner keeps it.
    pub target: Option<SimId>,
    /// How many units the last shot of its launcher still launches.
    pub pending: u32,
    /// Ticks until it may launch again.
    pub wait: u32,
}

impl Bay {
    pub(crate) fn new(number: u32) -> Self {
        Self {
            slots: vec![Slot::Ready; number as usize],
            target: None,
            pending: 0,
            wait: 0,
        }
    }

    /// Whether a unit is ready to be launched.
    pub fn ready(&self) -> bool {
        self.slots.contains(&Slot::Ready)
    }

    /// The units that are launched.
    pub fn out(&self) -> impl Iterator<Item = SimId> + '_ {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Out(id) => Some(*id),
            _ => None,
        })
    }

    /// The place of the launched unit `unit` becomes `slot`.
    fn settle(&mut self, unit: SimId, slot: Slot) {
        for place in &mut self.slots {
            if *place == Slot::Out(unit) {
                *place = slot;
            }
        }
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.slots.len() as u64);
        for slot in &self.slots {
            let (tag, value) = match *slot {
                Slot::Ready => (0, 0),
                Slot::Out(id) => (1, id.0),
                Slot::Reloading(ticks) => (2, ticks),
                Slot::Lost(ticks) => (3, ticks),
            };
            hasher.write(&[tag]);
            hasher.write_u32(value);
        }
        match self.target {
            None => hasher.write(&[0]),
            Some(target) => {
                hasher.write(&[1]);
                hasher.write_u32(target.0);
            }
        }
        hasher.write_u32(self.pending);
        hasher.write_u32(self.wait);
    }
}

/// The flight of a launched missile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissileFlight {
    /// Where it goes off.
    pub aim: SubPos,
    /// Whether its launcher was elite when it launched it.
    pub elite: bool,
    /// The firepower multiplier of its launcher then, in permille.
    pub firepower_pm: u32,
}

impl MissileFlight {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write(&self.aim.x.to_le_bytes());
        hasher.write(&self.aim.y.to_le_bytes());
        hasher.write(&[u8::from(self.elite)]);
        hasher.write_u32(self.firepower_pm);
    }
}

/// The cell above which the aircraft on dock `dock` of a building with `footprint` stands: the
/// corners of the footprint first, then its other cells row by row.
pub fn dock_cell(footprint: Footprint, dock: u8) -> CellPos {
    let (w, h) = (
        i32::from(footprint.width.max(1)),
        i32::from(footprint.height.max(1)),
    );
    let mut cells: Vec<(i32, i32)> = vec![(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)];
    for y in 0..h {
        for x in 0..w {
            cells.push((x, y));
        }
    }
    let mut order: Vec<(i32, i32)> = Vec::with_capacity(cells.len());
    for cell in cells {
        if !order.contains(&cell) {
            order.push(cell);
        }
    }
    let (dx, dy) = order.get(usize::from(dock)).copied().unwrap_or((0, 0));
    CellPos::new(
        footprint.origin.x.saturating_add(dx),
        footprint.origin.y.saturating_add(dy),
    )
}

/// The docks of `owned` (the types of a player's buildings, one entry per building) together.
pub(crate) fn docks_of(
    definitions: &Definitions,
    owned: impl Iterator<Item = crate::BuildingKind>,
) -> u32 {
    owned
        .filter_map(|kind| definitions.metadata(ProductionItem::Building(kind)))
        .fold(0u32, |total, entry| total.saturating_add(entry.docks))
}

/// Existing aircraft and queued commitments share a dock before production starts.
pub(crate) fn dock_commitments(definitions: &Definitions, state: &crate::PlayerState) -> usize {
    (state.docked_aircraft() as usize).saturating_add(
        state
            .queues()
            .flat_map(|(_, items)| items.iter())
            .filter(|queued| needs_dock(definitions, queued.item))
            .count(),
    )
}

/// Whether units of the type of `item` need a dock (the arsenal's `airport_bound`, with a
/// rules file).
pub(crate) fn needs_dock(definitions: &Definitions, item: ProductionItem) -> bool {
    definitions.rules().is_some()
        && matches!(item, ProductionItem::Unit(_))
        && definitions
            .metadata(item)
            .is_some_and(|entry| entry.airport_bound)
}

fn entity_of(world: &mut World, unit: SimId) -> Option<Entity> {
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, id)| **id == unit)
        .map(|(entity, _)| entity)
}

fn chebyshev(a: CellPos, b: CellPos) -> u32 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

/// The `Economy` stage of the aircraft and the spawners (see the module documentation).
/// `index` holds the units and buildings as they were at the start of the stage's specialist
/// step; what this function needs fresh it reads from the world.
pub(crate) fn advance(
    world: &mut World,
    defs: &Definitions,
    index: &BTreeMap<SimId, Actor>,
    tick: u64,
) {
    if defs.rules().is_none() {
        return;
    }
    {
        let mut state = world.resource_mut::<SpecialState>();
        state.ammo.retain(|id, _| index.contains_key(id));
        state.reloading.retain(|id, _| index.contains_key(id));
        state
            .surfaced
            .retain(|id, until| index.contains_key(id) && *until > tick);
    }
    docks(world, defs, index, tick);
    spawned_units(world, defs, index, tick);
    bays(world, defs, index, tick);
}

/// The docks, the way home and the reloading of the airport bound aircraft, in id order.
fn docks(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>, tick: u64) {
    let Some(rules) = defs.rules() else {
        return;
    };
    let interval = u32::try_from(rules.economy.reload_interval.max(1)).unwrap_or(1);
    // The buildings with docks: id, owner, footprint, docks.
    let pads: Vec<(SimId, PlayerId, Footprint, u32)> = index
        .values()
        .filter_map(|b| {
            let docks = defs.metadata(b.item).map_or(0, |entry| entry.docks);
            let footprint = b.foot?;
            (docks > 0 && b.hp > 0).then_some((b.id, b.owner, footprint, docks))
        })
        .collect();
    // A dock is kept while its aircraft and its building exist and have one owner.
    world
        .resource_mut::<SpecialState>()
        .docks
        .retain(|unit, (building, dock)| {
            index.get(unit).is_some_and(|a| {
                pads.iter().any(|&(id, owner, _, docks)| {
                    id == *building && owner == a.owner && u32::from(*dock) < docks
                })
            })
        });
    let mut owned: BTreeMap<PlayerId, u32> = BTreeMap::new();
    for a in index.values() {
        let Some(entry) = defs.metadata(a.item).filter(|entry| entry.airport_bound) else {
            continue;
        };
        if a.hp <= 0 {
            continue;
        }
        let Some(mobile) = world.get::<Mobile>(a.entity).copied() else {
            continue;
        };
        *owned.entry(a.owner).or_default() += 1;
        let ammo = world.resource::<SpecialState>().ammo_of(a.id, entry.ammo);
        // Its dock, or the nearest free one of its owner (ties by the building's id).
        let dock = world.resource::<SpecialState>().docks.get(&a.id).copied();
        let dock = dock.or_else(|| {
            let state = world.resource::<SpecialState>();
            let free = |building: SimId, docks: u32| {
                (0..docks).map(|dock| dock as u8).find(|dock| {
                    !state
                        .docks
                        .values()
                        .any(|&taken| taken == (building, *dock))
                })
            };
            pads.iter()
                .filter(|&&(_, owner, ..)| owner == a.owner)
                .filter_map(|&(building, _, footprint, docks)| {
                    free(building, docks)
                        .map(|dock| (footprint.distance_to_cell(mobile.cell), building, dock))
                })
                .min()
                .map(|(_, building, dock)| (building, dock))
        });
        if let Some(found) = dock {
            world
                .resource_mut::<SpecialState>()
                .docks
                .insert(a.id, found);
        }
        let mut fighting = world
            .get::<Weapon>(a.entity)
            .is_some_and(|weapon| weapon.order.is_some());
        let in_volley = world
            .get::<Weapon>(a.entity)
            .is_some_and(|weapon| weapon.burst_left > 0);
        if fighting && ammo == 0 && !in_volley {
            // Out of ammunition, and the volley it paid for is over: so is the attack.
            clear_orders(world, a.entity);
            fighting = false;
        }
        let idle = !fighting
            && world
                .get::<Orders>(a.entity)
                .is_some_and(|orders| orders.current.is_none());
        let cell = dock.and_then(|(building, dock)| {
            pads.iter()
                .find(|&&(id, ..)| id == building)
                .map(|&(_, _, footprint, _)| dock_cell(footprint, dock))
        });
        let Some(cell) = cell else {
            // No dock is left for it: once it has nothing to do it is lost.
            if idle && let Some(mut health) = world.get_mut::<Health>(a.entity) {
                health.hit_points = 0;
            }
            world.resource_mut::<SpecialState>().reloading.remove(&a.id);
            continue;
        };
        let landed = idle && mobile.cell == cell && mobile.step.is_none();
        if idle
            && !landed
            && let Some(mut orders) = world.get_mut::<Orders>(a.entity)
        {
            orders.current = Some(MoveOrder::new(cell, tick));
        }
        let mut state = world.resource_mut::<SpecialState>();
        if landed && ammo < entry.ammo {
            let waited = state.reloading.entry(a.id).or_default();
            *waited = waited.saturating_add(1);
            if *waited >= interval {
                state.reloading.remove(&a.id);
                if ammo + 1 >= entry.ammo {
                    state.ammo.remove(&a.id);
                } else {
                    state.ammo.insert(a.id, ammo + 1);
                }
            }
        } else {
            state.reloading.remove(&a.id);
        }
    }
    // How many aircraft that need a dock every player owns, and how many docks its buildings
    // have, for the production's check.
    for (player, state) in world.resource_mut::<Players>().iter_mut() {
        state.aircraft = owned.get(&player).copied().unwrap_or(0);
        state.docks = pads
            .iter()
            .filter(|&&(_, owner, ..)| owner == player)
            .fold(0u32, |total, &(.., docks)| total.saturating_add(docks));
    }
}

/// The launched units, in id order: a missile goes off at the end of its flight; an aircraft
/// that no longer attacks flies back and is taken aboard.
fn spawned_units(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>, tick: u64) {
    let launched = world.resource::<SpecialState>().spawned.clone();
    for (unit, parent) in launched {
        let regen = index
            .get(&parent)
            .and_then(|p| defs.metadata(p.item))
            .map_or(0, |entry| entry.spawn_regen);
        let Some(a) = index.get(&unit) else {
            // Shot down: its place is filled again after the time to replace it.
            world
                .resource_mut::<crate::Projectiles>()
                .reassign(unit, parent);
            let mut state = world.resource_mut::<SpecialState>();
            state.spawned.remove(&unit);
            state.missiles.remove(&unit);
            if let Some(bay) = state.bays.get_mut(&parent) {
                bay.settle(unit, Slot::Lost(regen.max(1)));
            }
            continue;
        };
        if a.hp <= 0 {
            continue;
        }
        let Some(mobile) = world.get::<Mobile>(a.entity).copied() else {
            continue;
        };
        let moving = world
            .get::<Orders>(a.entity)
            .is_some_and(|orders| orders.current.is_some());
        let flight = world
            .resource::<SpecialState>()
            .missiles
            .get(&unit)
            .copied();
        if let Some(flight) = flight {
            let arrived = mobile.cell == flight.aim.cell() && mobile.step.is_none();
            if !arrived && (moving || mobile.step.is_some()) {
                continue;
            }
            // The end of its flight: at its aim, or where its way ended.
            let point = if arrived {
                flight.aim
            } else {
                world
                    .get::<Position>(a.entity)
                    .map_or(flight.aim, |position| position.0)
            };
            let effect = defs
                .metadata(a.item)
                .and_then(|entry| entry.missile)
                .and_then(|missile| defs.effect(missile.effect(flight.elite)));
            release(world, a);
            world.despawn(a.entity);
            {
                let mut state = world.resource_mut::<SpecialState>();
                state.spawned.remove(&unit);
                state.missiles.remove(&unit);
                state.ammo.remove(&unit);
                if let Some(bay) = state.bays.get_mut(&parent) {
                    bay.settle(unit, Slot::Lost(regen.max(1)));
                }
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::MissileDetonated {
                    player: a.owner,
                    parent,
                    unit,
                    at: point,
                });
            if let Some(effect) = effect {
                detonate(
                    world,
                    Blast {
                        point,
                        air: false,
                        warhead: effect.warhead,
                        damage: effect.damage,
                        direct: None,
                        firepower_pm: flight.firepower_pm,
                        source: Some(parent),
                    },
                    true,
                );
            }
            continue;
        }
        // An aircraft: it is destroyed with its spawner.
        let Some(p) = index.get(&parent).filter(|p| p.hp > 0) else {
            if let Some(mut health) = world.get_mut::<Health>(a.entity) {
                health.hit_points = 0;
            }
            continue;
        };
        let (ammo, reload) = {
            let own = defs.metadata(a.item).map_or(0, |entry| entry.ammo);
            (
                (own, world.resource::<SpecialState>().ammo_of(unit, own)),
                defs.metadata(p.item).map_or(0, |entry| entry.spawn_reload),
            )
        };
        let attacking = world
            .get::<Weapon>(a.entity)
            .is_some_and(|weapon| weapon.order.is_some())
            && (ammo.0 == 0 || ammo.1 > 0);
        if attacking {
            continue;
        }
        if let Some(mut weapon) = world.get_mut::<Weapon>(a.entity) {
            weapon.clear();
        }
        let home = world.get::<Mobile>(p.entity).map_or(p.cell, |m| m.cell);
        if chebyshev(mobile.cell, home) <= 1 && mobile.step.is_none() {
            release(world, a);
            world.despawn(a.entity);
            world
                .resource_mut::<crate::Projectiles>()
                .reassign(unit, parent);
            let mut state = world.resource_mut::<SpecialState>();
            state.spawned.remove(&unit);
            state.ammo.remove(&unit);
            if let Some(bay) = state.bays.get_mut(&parent) {
                bay.settle(
                    unit,
                    if reload > 0 {
                        Slot::Reloading(reload)
                    } else {
                        Slot::Ready
                    },
                );
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::UnitRecovered {
                    player: a.owner,
                    parent,
                    unit,
                });
        } else if let Some(mut orders) = world.get_mut::<Orders>(a.entity)
            && orders
                .current
                .as_ref()
                .is_none_or(|order| order.destination != home)
        {
            orders.current = Some(MoveOrder::new(home, tick));
        }
    }
}

/// The bays, in the order of their spawners' ids: the timers of what is aboard, and the next
/// launch.
fn bays(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>, tick: u64) {
    let spawners: Vec<SimId> = world
        .resource::<SpecialState>()
        .bays
        .keys()
        .copied()
        .collect();
    for id in spawners {
        let Some(a) = index.get(&id).filter(|a| a.hp > 0) else {
            // The spawner is gone, and its aircraft with it (`spawned_units`); a missile in
            // flight flies on.
            world.resource_mut::<SpecialState>().bays.remove(&id);
            continue;
        };
        let Some(mut bay) = world.resource::<SpecialState>().bays.get(&id).cloned() else {
            continue;
        };
        for slot in &mut bay.slots {
            *slot = match *slot {
                Slot::Reloading(ticks) | Slot::Lost(ticks) if ticks <= 1 => Slot::Ready,
                Slot::Reloading(ticks) => Slot::Reloading(ticks - 1),
                Slot::Lost(ticks) => Slot::Lost(ticks - 1),
                other => other,
            };
        }
        bay.wait = bay.wait.saturating_sub(1);
        // It launches at a target only while it keeps that target.
        let ground = world
            .get::<Weapon>(a.entity)
            .and_then(|w| w.order)
            .and_then(|o| match o {
                AttackOrder::ForceFire {
                    target: crate::ForceTarget::Cell(c),
                } => Some(c),
                _ => None,
            });
        let keeps = ground.map(|_| SimId(u32::MAX)).or_else(|| {
            world
                .get::<Weapon>(a.entity)
                .and_then(|weapon| weapon.target)
        });
        if bay.target != keeps {
            bay.target = None;
            bay.pending = 0;
        }
        let stopped = world.resource::<SpecialState>().disabled.contains_key(&id);
        if let Some(target) = bay.target
            && bay.pending > 0
            && bay.wait == 0
            && !stopped
            && let Some(place) = bay.slots.iter().position(|slot| *slot == Slot::Ready)
        {
            // What it launches at: a unit or building that stands, or scenery, which is no
            // actor and is aimed at where it stands.
            let aim = match index.get(&target).filter(|target| target.hp > 0) {
                Some(target) => Some(Aim::Actor(target)),
                None => world
                    .resource::<crate::Props>()
                    .get(target)
                    .map(|(cell, _)| Aim::Scenery(target, cell))
                    .or_else(|| ground.map(|c| Aim::Scenery(target, c))),
            };
            match aim {
                None => {
                    bay.target = None;
                    bay.pending = 0;
                }
                Some(aim) => {
                    if let Some(unit) = launch(world, defs, a, aim, tick) {
                        bay.slots[place] = Slot::Out(unit);
                        bay.pending -= 1;
                        bay.wait = SPAWN_LAUNCH_GAP;
                    }
                }
            }
        }
        world.resource_mut::<SpecialState>().bays.insert(id, bay);
    }
}

/// What a spawner launches at.
#[derive(Clone, Copy)]
enum Aim<'a> {
    /// A unit or building.
    Actor(&'a Actor),
    /// Scenery (`crate::props`): its id and the cell it stands in.
    Scenery(SimId, CellPos),
}

impl Aim<'_> {
    fn id(&self) -> SimId {
        match *self {
            Aim::Actor(target) => target.id,
            Aim::Scenery(id, _) => id,
        }
    }

    fn cell(&self) -> CellPos {
        match *self {
            Aim::Actor(target) => target.cell,
            Aim::Scenery(_, cell) => cell,
        }
    }
}

/// The bay of the spawner `a` as it goes aboard a transport, if it has one: it leaves the map
/// with its spawner. What it had launched counts as lost from now on (its aircraft are
/// destroyed with a spawner that is gone, and a missile in flight does not come back), each
/// place to be filled after the time to replace it; the timers stand still aboard.
pub(crate) fn stow(world: &mut World, defs: &Definitions, a: &Actor) -> Option<Bay> {
    let mut bay = world.resource_mut::<SpecialState>().bays.remove(&a.id)?;
    let regen = defs
        .metadata(a.item)
        .map_or(0, |entry| entry.spawn_regen)
        .max(1);
    for slot in &mut bay.slots {
        if matches!(slot, Slot::Out(_)) {
            *slot = Slot::Lost(regen);
        }
    }
    bay.target = None;
    bay.pending = 0;
    Some(bay)
}

/// Launches one unit of the type `spawner` spawns at `target`; its id, or `None` if there is no
/// room in the air around the spawner.
fn launch(
    world: &mut World,
    defs: &Definitions,
    spawner: &Actor,
    target: Aim,
    tick: u64,
) -> Option<SimId> {
    let kind = defs.metadata(spawner.item)?.spawns?;
    let def = defs.unit(kind)?;
    let entry = defs.metadata(ProductionItem::Unit(kind))?;
    let from = world
        .get::<Mobile>(spawner.entity)
        .map_or(spawner.cell, |mobile| mobile.cell);
    let cell = {
        let grid = world.resource::<Grid>();
        let view = grid.navigation_for(Domain::Air);
        if view.is_passable(from) && view.has_room(from, false) {
            Some(from)
        } else {
            crate::building::exit_cell(
                &view,
                Footprint {
                    origin: from,
                    width: 1,
                    height: 1,
                },
                Some(target.cell()),
                false,
            )
        }
    }?;
    let unit = crate::spawn_unit(world, spawner.owner, &def.id, cell).ok()?;
    let entity = entity_of(world, unit)?;
    let experience = world.get::<crate::Experience>(spawner.entity).copied();
    let rank = experience.map_or(0, |experience| experience.rank);
    world
        .resource_mut::<SpecialState>()
        .spawned
        .insert(unit, spawner.id);
    if entry.missile.is_some() {
        // It flies to where the target is now: a unit's position, the cell of a building
        // nearest to the launcher, or the cell scenery stands in.
        let aim = match target {
            Aim::Actor(target) => match target.foot {
                Some(footprint) => SubPos::centre_of(nearest_cell_of(footprint, from)),
                None => world
                    .get::<Position>(target.entity)
                    .map_or(SubPos::centre_of(target.cell), |position| position.0),
            },
            Aim::Scenery(_, cell) => SubPos::centre_of(cell),
        };
        world.resource_mut::<SpecialState>().missiles.insert(
            unit,
            MissileFlight {
                aim,
                elite: rank >= crate::veterancy::ELITE,
                firepower_pm: experience.map_or(1000, |experience| experience.firepower_pm()),
            },
        );
        if let Some(mut orders) = world.get_mut::<Orders>(entity) {
            orders.current = Some(MoveOrder::new(aim.cell(), tick));
        }
    } else {
        // An aircraft has the rank of its spawner and attacks the target.
        crate::veterancy::inherit(world, entity, kind, rank);
        if let Some(mut weapon) = world.get_mut::<Weapon>(entity) {
            weapon.target = Some(target.id());
            weapon.order = Some(if target.id() == SimId(u32::MAX) {
                AttackOrder::ForceFire {
                    target: crate::ForceTarget::Cell(target.cell()),
                }
            } else {
                AttackOrder::Attack {
                    target: target.id(),
                    chases: 0,
                }
            });
        }
    }
    world
        .resource_mut::<Messages<SimEvent>>()
        .write(SimEvent::UnitLaunched {
            player: spawner.owner,
            parent: spawner.id,
            unit,
        });
    Some(unit)
}

/// A shot of the `Combat` stage: `a` fired `weapon` at `target`. A cloakable unit is seen for
/// the rules' `cloak_delay`; a launcher weapon gives its spawner's bay the target; and a unit
/// with ammunition has one volley less when the volley is over, which ends the attack of an
/// airport bound aircraft that has none left.
pub(crate) fn fired(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    target: SimId,
    weapon: WeaponKind,
    tick: u64,
) {
    let (Some(rules), Some(entry)) = (defs.rules(), defs.metadata(a.item)) else {
        return;
    };
    if entry.cloakable {
        let delay = u64::try_from(rules.combat.cloak_delay.max(0)).unwrap_or(0);
        world
            .resource_mut::<SpecialState>()
            .surfaced
            .insert(a.id, tick.saturating_add(delay));
    }
    let carried = world.get::<Weapon>(a.entity).copied();
    if entry.spawns.is_some()
        && let Some(carried) = carried
    {
        let launcher = match entry.launcher {
            Some(Launcher::Primary) => Some(carried.kind),
            Some(Launcher::Secondary) => carried.secondary,
            None => None,
        };
        if launcher == Some(weapon) {
            let mut state = world.resource_mut::<SpecialState>();
            let bay = state
                .bays
                .entry(a.id)
                .or_insert_with(|| Bay::new(entry.spawns_number));
            bay.target = Some(target);
            bay.pending = bay
                .slots
                .iter()
                .filter(|slot| **slot == Slot::Ready)
                .count() as u32;
        }
    }
    if entry.ammo > 0 {
        // A round is a volley, and it is spent when the volley begins: with its first shot,
        // after which the weapon has all the shots of its burst but one left. A volley that
        // is cut short (its target is destroyed, or nothing is left to fire at when its next
        // shot is due) has cost its round all the same (docs/DECISIONS.md R56).
        let burst = defs.weapon(weapon).map_or(1, |def| def.burst.max(1));
        let left_in_volley = carried.map_or(0, |carried| carried.burst_left);
        let left = {
            let mut state = world.resource_mut::<SpecialState>();
            let mut left = state.ammo_of(a.id, entry.ammo);
            if left_in_volley == burst - 1 {
                left = left.saturating_sub(1);
                state.ammo.insert(a.id, left);
            }
            left
        };
        // With the last shot of its last volley an airport bound aircraft turns for home.
        if left == 0 && left_in_volley == 0 && entry.airport_bound {
            clear_orders(world, a.entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_docks_of_a_building_are_its_corners_first_and_all_different() {
        let footprint = |width, height| Footprint {
            origin: CellPos::new(10, 20),
            width,
            height,
        };
        let cells = |width, height, docks: u8| -> Vec<(i32, i32)> {
            (0..docks)
                .map(|dock| {
                    let cell = dock_cell(footprint(width, height), dock);
                    (cell.x - 10, cell.y - 20)
                })
                .collect()
        };
        assert_eq!(cells(3, 3, 4), [(0, 0), (2, 0), (0, 2), (2, 2)]);
        assert_eq!(cells(2, 2, 4), [(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert_eq!(cells(1, 1, 1), [(0, 0)]);
        let all = cells(3, 2, 6);
        let mut unique = all.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 6, "{all:?}");
        // A dock the building does not have is its first cell, never a cell outside it.
        assert_eq!(dock_cell(footprint(2, 2), 9), CellPos::new(10, 20));
    }

    #[test]
    fn a_bay_is_hashed_with_every_slot_its_target_and_its_wait() {
        let hash = |bay: &Bay| {
            let mut hasher = Fnv1a64::new();
            bay.hash_into(&mut hasher);
            hasher.finish()
        };
        let base = Bay::new(2);
        assert!(base.ready());
        let mut seen = std::collections::BTreeSet::from([hash(&base)]);
        for slot in [
            Slot::Out(SimId(7)),
            Slot::Out(SimId(8)),
            Slot::Reloading(7),
            Slot::Lost(7),
        ] {
            let mut bay = base.clone();
            bay.slots[1] = slot;
            assert!(seen.insert(hash(&bay)), "{slot:?}");
        }
        let mut bay = base.clone();
        bay.target = Some(SimId(3));
        assert!(seen.insert(hash(&bay)));
        bay.wait = 5;
        assert!(seen.insert(hash(&bay)));
        bay.pending = 2;
        assert!(seen.insert(hash(&bay)));
        let mut empty = Bay::new(1);
        empty.slots[0] = Slot::Lost(3);
        assert!(!empty.ready());
        empty.settle(SimId(1), Slot::Ready);
        assert!(!empty.ready(), "only the place of that unit changes");
    }
}
