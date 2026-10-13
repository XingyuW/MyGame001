//! Deterministic specialist actions layered onto the common command, movement and combat paths.
//!
//! Area damage (RA3, docs/RA2_PARITY_PLAN.md section 4.3, docs/DECISIONS.md R35): the nuclear
//! strike, the bolts of the lightning storm, the bomb of a demolition specialist, the charge of
//! a commando and the pulses of a radiation zone are impacts of the warhead and base damage the
//! combat file's `effects` name for them (`area`), and a suicide unit's blast is the impact
//! of its own weapon at its own position, so all of them go through the armour table
//! (`crate::combat::detonate`). An effect the combat file does not list, and a suicide unit in a
//! world without a rules file, keep the rule of M9 (`blast`: damage that falls off with the
//! distance in cells and ignores armour), which is what the fixtures of M9 assert.
//!
//! Cells (RA3, invariant I6', docs/DECISIONS.md R6): every action here that puts a unit into a
//! cell (a teleport, an unloaded passenger, an undeployed base unit, a launched aircraft) asks
//! the grid whether the cell has room for that unit ([`Grid::has_room`], [`Grid::room_for`]),
//! so infantry on foot share a cell three to a cell and stand at their spots, and nothing else
//! shares one.
use crate::{
    Building, CellPos, Definitions, Footprint, Grid, ProductionItem, SimId, SimTick, UnitKind,
    arsenal::*,
    building::{building_bundle, check_footprint},
    combat::{Blast, Body, Condition, DamageState, Health, Weapon, combat_components, detonate},
    combat_defs::Effect,
    command::{Command, IssuedCommand, PlayerId, RejectReason},
    economy::Players,
    event::SimEvent,
    hash::Fnv1a64,
    unit::{Facing, Mobile, MoveOrder, Orders, Owner, Position, Unit, unit_bundle},
};
use bevy_ecs::{entity::Entity, message::Messages, resource::Resource, world::World};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Passenger {
    pub id: SimId,
    pub kind: UnitKind,
    pub hp: i32,
    pub harvester: Option<crate::Harvester>,
    /// The passenger's experience (RA4): it keeps its rank aboard.
    pub experience: Option<crate::Experience>,
    /// The bay of a passenger that spawns units (RA4), as it went aboard: what it had launched
    /// counts as lost, and the timers of its places stand still until it is unloaded.
    pub bay: Option<crate::flight::Bay>,
}
#[derive(Clone, Debug)]
pub struct Zone {
    pub role: Role,
    pub cell: CellPos,
    pub radius: u32,
    pub damage: i32,
    pub remaining: u32,
}
#[derive(Resource, Clone, Debug, Default)]
pub struct SpecialState {
    pub charge: BTreeMap<SimId, u32>,
    pub protected: BTreeMap<SimId, u64>,
    pub disabled: BTreeMap<SimId, u64>,
    pub cargo: BTreeMap<SimId, Vec<Passenger>>,
    pub controlled: BTreeMap<SimId, (SimId, PlayerId)>,
    pub bombs: BTreeMap<SimId, (u64, i32, u32)>,
    pub zones: Vec<Zone>,
    pub rearming: BTreeMap<SimId, u32>,
    pub carrier_flights: BTreeMap<SimId, SimId>,
    pub interactions: BTreeMap<SimId, SimId>,
    pub hidden: BTreeSet<(PlayerId, SimId)>,
    pub outages: BTreeMap<PlayerId, u64>,
    /// Vehicles docked at a building that repairs units, by vehicle (RA2: `unit_repair`, the
    /// Service Depot). A docked vehicle stands beside the building and is repaired in steps
    /// for credits until it is whole, is ordered elsewhere, or the building is lost.
    pub docked: BTreeMap<SimId, SimId>,
    /// RA4 (`crate::flight`): the volleys an aircraft with ammunition has left, for every one
    /// that has fewer than its type's `ammo` (0: it must reload before it fires again).
    pub ammo: BTreeMap<SimId, u32>,
    /// The ticks an aircraft has stood on its dock toward its next round.
    pub reloading: BTreeMap<SimId, u32>,
    /// The dock of an airport bound aircraft: the building, and which of its docks.
    pub docks: BTreeMap<SimId, (SimId, u8)>,
    /// The units a spawner carries and what it launched them at, by spawner.
    pub bays: BTreeMap<SimId, crate::flight::Bay>,
    /// The unit that launched a missile or aircraft, by the launched unit.
    pub spawned: BTreeMap<SimId, SimId>,
    /// The flight of a launched missile.
    pub missiles: BTreeMap<SimId, crate::flight::MissileFlight>,
    /// The tick until which a cloakable unit that fired is seen like any other unit.
    pub surfaced: BTreeMap<SimId, u64>,
}
impl SpecialState {
    pub fn invulnerable(&self, id: SimId, tick: u64) -> bool {
        self.protected.get(&id).is_some_and(|&until| until > tick)
    }
    /// Whether the spawner `id`, which carries `number` units, has one ready to launch (RA4).
    pub fn launch_ready(&self, id: SimId, number: u32) -> bool {
        match self.bays.get(&id) {
            None => number > 0,
            Some(bay) => bay.ready(),
        }
    }
    /// The volleys the aircraft `id` of a type with `ammo` volleys has left (RA4).
    pub fn ammo_of(&self, id: SimId, ammo: u32) -> u32 {
        self.ammo.get(&id).copied().unwrap_or(ammo).min(ammo)
    }
    pub(crate) fn hash_into(&self, h: &mut Fnv1a64) {
        // Stable ordered data, including off-map passengers and pending effects.
        for map in [&self.protected, &self.disabled] {
            h.write_u64(map.len() as u64);
            for (id, t) in map {
                h.write_u32(id.0);
                h.write_u64(*t);
            }
        }
        for map in [&self.charge, &self.rearming] {
            h.write_u64(map.len() as u64);
            for (id, t) in map {
                h.write_u32(id.0);
                h.write_u32(*t);
            }
        }
        h.write_u64(self.carrier_flights.len() as u64);
        for (fighter, carrier) in &self.carrier_flights {
            h.write_u32(fighter.0);
            h.write_u32(carrier.0);
        }
        h.write_u64(self.cargo.len() as u64);
        for (id, ps) in &self.cargo {
            h.write_u32(id.0);
            h.write_u64(ps.len() as u64);
            for p in ps {
                h.write_u32(p.id.0);
                h.write(&p.kind.0.to_le_bytes());
                h.write(&p.hp.to_le_bytes());
                if let Some(worker) = &p.harvester {
                    h.write(&[1]);
                    worker.hash_into(h);
                } else {
                    h.write(&[0]);
                }
                if let Some(experience) = &p.experience {
                    h.write(b"xp");
                    experience.hash_into(h);
                }
                if let Some(bay) = &p.bay {
                    h.write(b"by");
                    bay.hash_into(h);
                }
            }
        }
        h.write_u64(self.controlled.len() as u64);
        for (id, (c, o)) in &self.controlled {
            h.write_u32(id.0);
            h.write_u32(c.0);
            h.write(&[o.0]);
        }
        h.write_u64(self.bombs.len() as u64);
        for (id, (t, d, r)) in &self.bombs {
            h.write_u32(id.0);
            h.write_u64(*t);
            h.write(&d.to_le_bytes());
            h.write_u32(*r);
        }
        h.write_u64(self.zones.len() as u64);
        for z in &self.zones {
            h.write(&[z.role as u8]);
            h.write(&z.cell.x.to_le_bytes());
            h.write(&z.cell.y.to_le_bytes());
            h.write_u32(z.radius);
            h.write(&z.damage.to_le_bytes());
            h.write_u32(z.remaining);
        }
        h.write_u64(self.interactions.len() as u64);
        for (id, t) in &self.interactions {
            h.write_u32(id.0);
            h.write_u32(t.0);
        }
        h.write_u64(self.hidden.len() as u64);
        for (p, id) in &self.hidden {
            h.write(&[p.0]);
            h.write_u32(id.0);
        }
        h.write_u64(self.outages.len() as u64);
        for (p, t) in &self.outages {
            h.write(&[p.0]);
            h.write_u64(*t);
        }
        h.write_u64(self.docked.len() as u64);
        for (unit, building) in &self.docked {
            h.write_u32(unit.0);
            h.write_u32(building.0);
        }
        // RA4: each part is written only if it holds something, under its own tag, so that a
        // world without aircraft docks, spawners and cloaking hashes as before.
        for (tag, map) in [(b"am", &self.ammo), (b"rl", &self.reloading)] {
            if !map.is_empty() {
                h.write(tag);
                h.write_u64(map.len() as u64);
                for (id, value) in map {
                    h.write_u32(id.0);
                    h.write_u32(*value);
                }
            }
        }
        if !self.docks.is_empty() {
            h.write(b"dk");
            h.write_u64(self.docks.len() as u64);
            for (unit, (building, dock)) in &self.docks {
                h.write_u32(unit.0);
                h.write_u32(building.0);
                h.write(&[*dock]);
            }
        }
        if !self.bays.is_empty() {
            h.write(b"by");
            h.write_u64(self.bays.len() as u64);
            for (unit, bay) in &self.bays {
                h.write_u32(unit.0);
                bay.hash_into(h);
            }
        }
        if !self.spawned.is_empty() {
            h.write(b"sp");
            h.write_u64(self.spawned.len() as u64);
            for (unit, parent) in &self.spawned {
                h.write_u32(unit.0);
                h.write_u32(parent.0);
            }
        }
        if !self.missiles.is_empty() {
            h.write(b"ms");
            h.write_u64(self.missiles.len() as u64);
            for (unit, flight) in &self.missiles {
                h.write_u32(unit.0);
                flight.hash_into(h);
            }
        }
        if !self.surfaced.is_empty() {
            h.write(b"sf");
            h.write_u64(self.surfaced.len() as u64);
            for (unit, until) in &self.surfaced {
                h.write_u32(unit.0);
                h.write_u64(*until);
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct Actor {
    pub(crate) entity: Entity,
    pub(crate) id: SimId,
    pub(crate) owner: PlayerId,
    pub(crate) item: ProductionItem,
    pub(crate) cell: CellPos,
    pub(crate) foot: Option<Footprint>,
    pub(crate) hp: i32,
}
impl Actor {
    /// Whether it has hit points left (one without is removed in the tick's `Cleanup`).
    pub(crate) fn alive(&self) -> bool {
        self.hp > 0
    }
}
fn actors(world: &mut World) -> BTreeMap<SimId, Actor> {
    world
        .query::<(
            Entity,
            &SimId,
            &Owner,
            Option<&Unit>,
            Option<&Mobile>,
            Option<&Building>,
            &Health,
        )>()
        .iter(world)
        .filter_map(|(entity, id, o, u, m, b, h)| {
            let (item, cell, foot) = match (u, m, b) {
                (Some(u), Some(m), _) => (ProductionItem::Unit(u.kind), m.cell, None),
                (_, _, Some(b)) => (
                    ProductionItem::Building(b.kind),
                    b.footprint.origin,
                    Some(b.footprint),
                ),
                _ => return None,
            };
            Some((
                *id,
                Actor {
                    entity,
                    id: *id,
                    owner: o.0,
                    item,
                    cell,
                    foot,
                    hp: h.hit_points,
                },
            ))
        })
        .collect()
}
/// The units and buildings a command may act on, or act for: those with hit points left. A
/// command can take the last hit points of something (a nuclear strike lands in the `Commands`
/// stage), and what it destroyed is removed only in `Cleanup`; for every command after it, and
/// for an order waiting to be carried out, it is gone already (docs/DECISIONS.md R56).
fn living(world: &mut World) -> BTreeMap<SimId, Actor> {
    let mut index = actors(world);
    index.retain(|_, a| a.alive());
    index
}
fn near(a: CellPos, b: CellPos, r: u32) -> bool {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    crate::combat::in_range(dx, dy, r)
}
fn distance(a: &Actor, b: &Actor) -> u32 {
    b.foot.map_or(
        a.cell.x.abs_diff(b.cell.x).max(a.cell.y.abs_diff(b.cell.y)),
        |f| f.distance_to_cell(a.cell),
    )
}
fn role(defs: &Definitions, a: &Actor) -> Role {
    defs.metadata(a.item).map_or(Role::Combat, |e| e.role)
}
/// Whether power lets `a` work. A building: [`PlayerState::powers`](crate::PlayerState::powers)
/// (RA2: unless its owner is low on power and it is `powered` and consumes power; without a
/// rules file: while its owner is not low on power). A unit: always with a rules file, and as a
/// building without one (the rule of M9).
fn powered(world: &World, defs: &Definitions, a: &Actor) -> bool {
    let Some(state) = world.resource::<Players>().get(a.owner) else {
        return false;
    };
    match a.item {
        ProductionItem::Building(kind) => state.powers(defs, kind),
        ProductionItem::Unit(_) => defs.rules().is_some() || !state.low_power(),
    }
}
/// Whether the superweapon of building `a` charges and may be used as far as power goes: with a
/// rules file unless its owner is low on power and the arsenal marks it `is_powered` (RA2, the
/// ini's `IsPowered`); without one while its owner is not low on power.
fn charges(world: &World, defs: &Definitions, a: &Actor) -> bool {
    let Some(state) = world.resource::<Players>().get(a.owner) else {
        return false;
    };
    !state.low_power()
        || (defs.rules().is_some() && defs.metadata(a.item).is_none_or(|m| !m.is_powered))
}
/// Whether `b` is a building that repairs the vehicles sent to it (RA2: the arsenal's
/// `unit_repair`, read with a rules file).
fn repairs_units(defs: &Definitions, b: &Actor) -> bool {
    defs.rules().is_some()
        && b.foot.is_some()
        && defs.metadata(b.item).is_some_and(|m| m.unit_repair)
}
/// The checks of sending unit `a` to the repair building `b` (RA2): `b` is the player's own, `a`
/// is a vehicle of the building's element (a ship at a shipyard, a ground vehicle elsewhere; no
/// aircraft), and `a` is damaged.
fn validate_docking(a: &Actor, b: &Actor, defs: &Definitions) -> Result<(), RejectReason> {
    if a.owner != b.owner {
        return Err(unavailable("Choose your own repair building"));
    }
    let ProductionItem::Unit(kind) = a.item else {
        return Err(unavailable("Only vehicles are repaired here"));
    };
    if defs.category(a.item) != Category::Vehicles {
        return Err(unavailable("Only vehicles are repaired here"));
    }
    let shipyard = role(defs, b) == Role::Shipyard;
    let fits = match defs.domain(kind) {
        Domain::Air => false,
        Domain::Amphibious => true,
        Domain::Naval => shipyard,
        Domain::Land => !shipyard,
    };
    if !fits {
        return Err(unavailable("This unit is not repaired here"));
    }
    if defs
        .unit_combat(kind)
        .is_none_or(|terms| a.hp >= terms.hit_points)
    {
        return Err(unavailable("This unit is not damaged"));
    }
    Ok(())
}
/// Frees the cells the unit `a` holds.
fn vacate(world: &mut World, a: &Actor) {
    let mobile = world.get::<Mobile>(a.entity).copied();
    if let Some(m) = mobile {
        let mut g = world.resource_mut::<Grid>();
        g.release(m.cell, a.id);
        if let Some(s) = m.step {
            g.release(s.to, a.id);
        }
    }
}
/// Frees the cells the unit `a` holds and forgets it: it leaves the map.
pub(crate) fn release(world: &mut World, a: &Actor) {
    vacate(world, a);
    if world.get::<Mobile>(a.entity).is_some() {
        world.resource_mut::<Grid>().unregister(a.id);
    }
}
fn teleport(world: &mut World, a: &Actor, cell: CellPos) -> bool {
    let domain = world.resource::<Grid>().domain_of(a.id);
    if !world.resource::<Grid>().passable_for(cell, domain)
        || !world.resource::<Grid>().room_for(cell, a.id)
    {
        return false;
    }
    let held = world.get::<Mobile>(a.entity).copied();
    vacate(world, a);
    let point = {
        let mut g = world.resource_mut::<Grid>();
        if !g.claim(cell, a.id) {
            // The cell refuses the unit after all (the checks above are the grid's own, so
            // this is not expected): it keeps the cells it held and stays.
            if let Some(m) = held {
                g.claim(m.cell, a.id);
                if let Some(s) = m.step {
                    g.claim(s.to, a.id);
                }
            }
            return false;
        }
        g.standing_point(cell, a.id)
    };
    if let Some(mut m) = world.get_mut::<Mobile>(a.entity) {
        m.cell = cell;
        m.step = None;
    }
    if let Some(mut p) = world.get_mut::<Position>(a.entity) {
        p.0 = point;
    }
    if let Some(mut o) = world.get_mut::<Orders>(a.entity) {
        o.current = None;
    }
    true
}
fn hurt(world: &mut World, a: &Actor, damage: i32) {
    let tick = world.resource::<SimTick>().0;
    if !world.resource::<SpecialState>().invulnerable(a.id, tick)
        && let Some(mut h) = world.get_mut::<Health>(a.entity)
    {
        h.hit_points = h.hit_points.saturating_sub(damage);
    }
}
/// The area effect `effect` at `point`: an impact of the warhead and base damage the combat
/// file names for it, on the ground (and `direct`, if given, as if hit at the point itself).
/// Returns whether the combat file lists the effect; if not, nothing happened and the caller
/// applies the rule of M9.
fn area(world: &mut World, effect: Effect, point: crate::SubPos, direct: Option<SimId>) -> bool {
    let Some(def) = world.resource::<Definitions>().effect(effect) else {
        return false;
    };
    detonate(
        world,
        Blast {
            point,
            air: false,
            warhead: def.warhead,
            damage: def.damage,
            direct,
            firepower_pm: 1000,
            source: None,
        },
        // A radiation zone pulses on everything in it once a second: no event for each.
        effect != Effect::Radiation,
    );
    true
}
/// The area damage of M9, for effects the combat file does not list: `damage` at `cell`, less
/// with every cell of distance, nothing beyond `radius` cells, whatever the armour.
fn blast(world: &mut World, cell: CellPos, radius: u32, damage: i32) {
    // Scenery in the area (`crate::props`) is hurt like everything else.
    let hit: Vec<(CellPos, i64)> = world
        .resource::<crate::Props>()
        .iter()
        .filter_map(|(at, _)| {
            let d = at.x.abs_diff(cell.x).max(at.y.abs_diff(cell.y));
            (d <= radius).then(|| {
                (
                    at,
                    i64::from(damage.saturating_mul((radius + 1 - d) as i32) / (radius + 1) as i32),
                )
            })
        })
        .collect();
    let mut props = world.resource_mut::<crate::Props>();
    for (at, damage) in hit {
        props.hurt(at, damage);
    }
    for a in actors(world).values() {
        let d = a.foot.map_or(
            a.cell.x.abs_diff(cell.x).max(a.cell.y.abs_diff(cell.y)),
            |f| f.distance_to_cell(cell),
        );
        if d <= radius {
            hurt(
                world,
                a,
                damage.saturating_mul((radius + 1 - d) as i32) / (radius + 1) as i32,
            );
        }
    }
}
/// Where the unit `a` is (a building: the centre of its footprint's origin).
fn position_of(world: &World, a: &Actor) -> crate::SubPos {
    world
        .get::<Position>(a.entity)
        .map_or(crate::SubPos::centre_of(a.cell), |position| position.0)
}
/// The cell of `foot` nearest to `from`.
pub(crate) fn nearest_cell_of(foot: Footprint, from: CellPos) -> CellPos {
    let clamp = |at: i32, origin: i32, side: u8| {
        let last = i64::from(origin) + i64::from(side.max(1)) - 1;
        i32::try_from(i64::from(at).clamp(i64::from(origin), last)).unwrap_or(origin)
    };
    CellPos::new(
        clamp(from.x, foot.origin.x, foot.width),
        clamp(from.y, foot.origin.y, foot.height),
    )
}
pub(crate) fn clear_orders(world: &mut World, e: Entity) {
    if let Some(mut o) = world.get_mut::<Orders>(e) {
        o.current = None;
    }
    if let Some(mut w) = world.get_mut::<Weapon>(e) {
        w.clear();
    }
}
/// The unit or building `id` of player `p`. One of another player that `p` does not see, or
/// that is hidden from `p`, is reported like one that does not exist, so that a rejection
/// reveals nothing about unseen enemies (docs/DECISIONS.md D30, D33, R56).
fn own<'a>(
    world: &World,
    defs: &Definitions,
    index: &'a BTreeMap<SimId, Actor>,
    p: PlayerId,
    id: SimId,
) -> Result<&'a Actor, RejectReason> {
    let a = index.get(&id).ok_or(RejectReason::UnknownUnit(id))?;
    if a.owner != p {
        return Err(if known_to(world, defs, p, a) {
            RejectReason::NotOwned(id)
        } else {
            RejectReason::UnknownUnit(id)
        });
    }
    Ok(a)
}
/// Whether player `p` may know of `a`, a unit or building of another player: always without
/// fog of war (no combat file); with it while `p` sees a cell of its body and it is not hidden
/// from `p` (a submerged unit, RA4).
fn known_to(world: &World, defs: &Definitions, p: PlayerId, a: &Actor) -> bool {
    if !defs.has_combat() {
        return true;
    }
    let body = match a.foot {
        Some(foot) => Some(Body::Building(foot)),
        None => Body::of(world.get::<Mobile>(a.entity), None),
    };
    world
        .resource::<Players>()
        .get(p)
        .is_some_and(|state| body.is_some_and(|body| body.seen_by(state.vision())))
        && !world.resource::<SpecialState>().hidden.contains(&(p, a.id))
}
/// A launched missile or carried aircraft takes no commands (RA4).
fn commandable(world: &World, id: SimId) -> Result<(), RejectReason> {
    if world.resource::<SpecialState>().spawned.contains_key(&id) {
        return Err(RejectReason::NotCommandable(id));
    }
    Ok(())
}
fn unavailable(s: &'static str) -> RejectReason {
    RejectReason::AbilityUnavailable(s)
}
/// `hp` of `from` hit points as the same share of `to` hit points, at least 1 and at most `to`
/// (deploying and undeploying keep the share, rounded down).
fn share_of(hp: i32, from: i32, to: i32) -> i32 {
    let share = i64::from(hp.max(0)).saturating_mul(i64::from(to)) / i64::from(from.max(1));
    i32::try_from(share).unwrap_or(to).clamp(1, to.max(1))
}

pub(crate) fn apply(world: &mut World, issued: &IssuedCommand) -> Result<(), RejectReason> {
    let index = living(world);
    let defs = world.resource::<Definitions>().clone();
    let tick = world.resource::<SimTick>().0;
    if world.resource::<Players>().get(issued.player).is_none() {
        return Err(RejectReason::UnknownPlayer(issued.player));
    }
    match &issued.command {
        Command::Deploy { unit } => {
            let a = own(world, &defs, &index, issued.player, *unit)?;
            commandable(world, *unit)?;
            let meta = defs
                .metadata(a.item)
                .ok_or(unavailable("This unit has no deploy action"))?;
            match meta.role {
                Role::Mcv => {
                    // With a rules file (RA1) the base unit of a side deploys into that side's
                    // yard; without one, into the only yard of the M9 arsenal.
                    let kind = match (defs.rules(), a.item) {
                        (Some(rules), ProductionItem::Unit(unit)) => rules
                            .side_of_base_unit(unit)
                            .map(|side| side.construction_yard),
                        (Some(_), ProductionItem::Building(_)) => None,
                        (None, _) => defs.building_kind("construction_yard"),
                    }
                    .ok_or(unavailable("Construction Yard unavailable"))?;
                    let def = defs
                        .building(kind)
                        .ok_or(unavailable("Construction Yard unavailable"))?;
                    let foot = Footprint {
                        origin: a.cell,
                        width: def.width,
                        height: def.height,
                    };
                    let mut grid = world.resource::<Grid>().clone();
                    grid.release(a.cell, a.id);
                    if let Some(m) = world.get::<Mobile>(a.entity)
                        && let Some(s) = m.step
                    {
                        grid.release(s.to, a.id);
                    }
                    // Whether the player sees the footprint comes first, as for a placement
                    // (`economy_commands::check_placement`, docs/DECISIONS.md D33): the
                    // footprint's own errors name the building or unit in a cell. A footprint
                    // outside the map is left to them (`OutsideMap` reveals nothing).
                    if foot.inside(&grid)
                        && !world
                            .resource::<Players>()
                            .get(a.owner)
                            .is_some_and(|p| foot.cells().all(|c| p.vision().is_visible(c)))
                    {
                        return Err(unavailable("Explore the deployment footprint first"));
                    }
                    check_footprint(&grid, foot).map_err(RejectReason::Placement)?;
                    release(world, a);
                    for c in foot.cells() {
                        world.resource_mut::<Grid>().cover(c, a.id);
                    }
                    let (_, owner, b, producer) =
                        building_bundle(&defs, a.id, a.owner, kind, foot, vec![]);
                    let mut e = world.entity_mut(a.entity);
                    e.remove::<(Unit, Mobile, Position, Facing, Orders, Weapon)>();
                    e.insert((owner, b));
                    if let Some(p) = producer {
                        e.insert(p);
                    }
                    if let Some(t) = defs.building_combat(kind) {
                        let (c, w) = combat_components(t);
                        e.insert(c);
                        if let Some(w) = w {
                            e.insert(w);
                        }
                        // With a rules file the yard keeps the share of hit points the unit
                        // had, so that deploying and undeploying repairs nothing (R12).
                        if defs.rules().is_some()
                            && let ProductionItem::Unit(unit) = a.item
                            && let Some(from) = defs.unit_combat(unit)
                        {
                            e.insert(Health {
                                hit_points: share_of(a.hp, from.hit_points, t.hit_points),
                            });
                        }
                    }
                    world
                        .resource_mut::<Messages<SimEvent>>()
                        .write(SimEvent::BuildingPlaced {
                            player: a.owner,
                            building: a.id,
                            kind,
                        });
                }
                Role::Transport | Role::Ifv => unload(world, a, &defs)?,
                Role::Radiation => {
                    if world
                        .resource::<SpecialState>()
                        .disabled
                        .get(&a.id)
                        .is_some_and(|&t| t > tick)
                    {
                        return Err(unavailable("Radiation deployment is recharging"));
                    }
                    world.resource_mut::<SpecialState>().zones.push(Zone {
                        role: Role::Radiation,
                        cell: a.cell,
                        radius: meta.radius,
                        damage: meta.strength,
                        remaining: meta.cooldown,
                    });
                    world
                        .resource_mut::<SpecialState>()
                        .disabled
                        .insert(a.id, tick.saturating_add(u64::from(meta.cooldown)));
                    clear_orders(world, a.entity);
                }
                _ => return Err(unavailable("This unit has no deploy action")),
            }
        }
        Command::Undeploy { building } => {
            let a = index
                .get(building)
                .filter(|a| a.foot.is_some())
                .ok_or(RejectReason::UnknownBuilding(*building))?;
            let (ProductionItem::Building(kind), Some(foot)) = (a.item, a.foot) else {
                return Err(RejectReason::UnknownBuilding(*building));
            };
            if a.owner != issued.player {
                // Another player's building the player does not see is reported like one that
                // does not exist (docs/DECISIONS.md D30, D33).
                let seen = !defs.has_combat()
                    || world
                        .resource::<Players>()
                        .get(issued.player)
                        .is_some_and(|p| p.vision().sees_footprint(foot));
                return Err(if seen {
                    RejectReason::NotOwned(*building)
                } else {
                    RejectReason::UnknownBuilding(*building)
                });
            }
            if !world.resource::<crate::MatchState>().rules().mcv_repacks {
                return Err(RejectReason::DisabledByOptions);
            }
            let unit = defs
                .rules()
                .and_then(|rules| rules.side_of_construction_yard(kind))
                .map(|side| side.base_unit)
                .ok_or(unavailable("This building does not undeploy"))?;
            let speed = defs
                .unit(unit)
                .map(|def| def.speed)
                .ok_or(unavailable("This building does not undeploy"))?;
            let domain = defs.domain(unit);
            let on_foot = defs.on_foot(unit);
            // The unit stands where the footprint's origin was: once the building is gone the
            // cell must be ground the unit may stand on, with nothing of its layer in it.
            let mut grid = world.resource::<Grid>().clone();
            for c in foot.cells() {
                grid.uncover(c, a.id);
            }
            if !grid.passable_for(foot.origin, domain)
                || grid.occupant_for(foot.origin, domain).is_some()
            {
                return Err(unavailable("No room to undeploy"));
            }
            let point = {
                let mut grid = world.resource_mut::<Grid>();
                for c in foot.cells() {
                    grid.uncover(c, a.id);
                }
                grid.register(a.id, domain, on_foot);
                if !grid.claim(foot.origin, a.id) {
                    // The cell refuses the unit after all (not expected after the check
                    // above): the yard stays as it was.
                    grid.unregister(a.id);
                    for c in foot.cells() {
                        grid.cover(c, a.id);
                    }
                    return Err(unavailable("No room to undeploy"));
                }
                grid.standing_point(foot.origin, a.id)
            };
            if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                p.repairing.remove(&a.id);
            }
            world.resource_mut::<SpecialState>().charge.remove(&a.id);
            let mut e = world.entity_mut(a.entity);
            e.remove::<(Building, crate::Producer, Weapon)>();
            e.insert(unit_bundle(a.id, a.owner, unit, speed, foot.origin, point));
            if let Some(t) = defs.unit_combat(unit) {
                let (c, w) = combat_components(t);
                e.insert(c);
                if let Some(w) = w {
                    e.insert(w);
                }
                // The unit keeps the share of hit points the yard had (R12).
                if let Some(from) = defs.building_combat(kind) {
                    e.insert(Health {
                        hit_points: share_of(a.hp, from.hit_points, t.hit_points),
                    });
                }
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::BuildingUndeployed {
                    player: a.owner,
                    building: a.id,
                    kind,
                    unit: a.id,
                    into: unit,
                });
        }
        Command::Interact { unit, target } => {
            let a = own(world, &defs, &index, issued.player, *unit)?;
            commandable(world, *unit)?;
            let b = index
                .get(target)
                .ok_or(RejectReason::UnknownTarget(*target))?;
            if b.owner != a.owner
                && (!world.resource::<Players>().get(a.owner).is_some_and(|p| {
                    b.foot.map_or(p.vision().is_visible(b.cell), |f| {
                        p.vision().sees_footprint(f)
                    })
                }) || world
                    .resource::<SpecialState>()
                    .hidden
                    .contains(&(a.owner, b.id)))
            {
                return Err(RejectReason::UnknownTarget(*target));
            }
            validate_interaction(world, a, b, &defs)?;
            // A new order ends a repair at a dock.
            world.resource_mut::<SpecialState>().docked.remove(&a.id);
            if distance(a, b) <= 1 {
                interact(world, a, b, &defs)?;
            } else {
                world
                    .resource_mut::<SpecialState>()
                    .interactions
                    .insert(a.id, b.id);
                clear_orders(world, a.entity);
            }
        }
        Command::Superweapon {
            building,
            target,
            units,
        } => {
            let a = own(world, &defs, &index, issued.player, *building)?;
            let meta = defs
                .metadata(a.item)
                .ok_or(unavailable("No strategic ability"))?;
            if !meta.role.superweapon() {
                return Err(unavailable("No strategic ability"));
            }
            let state = world
                .resource::<Players>()
                .get(a.owner)
                .ok_or(RejectReason::UnknownPlayer(a.owner))?;
            if !world.resource::<Grid>().contains(*target) {
                return Err(RejectReason::OutsideMap(*target));
            }
            if !state.vision().is_visible(*target) {
                return Err(unavailable("Strategic target must be visible"));
            }
            if !charges(world, &defs, a)
                || world
                    .resource::<SpecialState>()
                    .charge
                    .get(building)
                    .copied()
                    .unwrap_or(0)
                    < meta.cooldown
            {
                return Err(unavailable("Ability requires power and a full charge"));
            }
            if meta.role == Role::Chronosphere {
                if units.is_empty() || units.len() > crate::command::MAX_UNITS_PER_COMMAND {
                    return Err(unavailable("Select eligible mechanical units first"));
                }
                let mut taken = BTreeSet::new();
                let mut plan = Vec::new();
                let mut ids = BTreeSet::new();
                for id in units {
                    if !ids.insert(*id) {
                        return Err(RejectReason::DuplicateUnit(*id));
                    }
                    let u = own(world, &defs, &index, a.owner, *id)?;
                    commandable(world, *id)?;
                    if defs.category(u.item) != Category::Vehicles
                        || matches!(u.item, ProductionItem::Building(_))
                    {
                        return Err(unavailable("Chronosphere requires mechanical units"));
                    }
                    if !near(
                        u.cell,
                        index.get(&units[0]).map_or(u.cell, |a| a.cell),
                        meta.radius,
                    ) {
                        return Err(unavailable("Selected units must be grouped together"));
                    }
                    let g = world.resource::<Grid>();
                    let view = g.navigation_for(g.domain_of(*id));
                    let dest = crate::pathfinding::destination_cells(&view, *target, 1, |c| {
                        taken.contains(&c)
                            || view.occupant(c).is_some()
                            || !state.vision().is_visible(c)
                    })
                    .first()
                    .copied()
                    .filter(|c| near(*c, *target, meta.radius))
                    .ok_or(unavailable("No clear teleport destination"))?;
                    taken.insert(dest);
                    plan.push((u.clone(), dest));
                }
                for (u, c) in plan {
                    teleport(world, &u, c);
                    clear_orders(world, u.entity);
                }
            } else if meta.role == Role::Curtain {
                let eligible: Vec<_> = index
                    .values()
                    .filter(|u| {
                        u.owner == a.owner
                            && defs.category(u.item) == Category::Vehicles
                            && near(u.cell, *target, meta.radius)
                    })
                    .map(|u| u.id)
                    .collect();
                if eligible.is_empty() {
                    return Err(unavailable("No friendly mechanical units in target area"));
                }
                for id in eligible {
                    world
                        .resource_mut::<SpecialState>()
                        .protected
                        .insert(id, tick.saturating_add(meta.strength as u64));
                }
            } else if meta.role == Role::Storm {
                world.resource_mut::<SpecialState>().zones.push(Zone {
                    role: Role::Storm,
                    cell: *target,
                    radius: meta.radius,
                    damage: meta.strength / 10,
                    remaining: 200,
                });
            } else if !area(world, Effect::Nuke, crate::SubPos::centre_of(*target), None) {
                blast(world, *target, meta.radius, meta.strength);
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::StrategicActivated {
                    player: a.owner,
                    building: *building,
                    target: *target,
                    role: meta.role,
                    radius: meta.radius,
                });
            world
                .resource_mut::<SpecialState>()
                .charge
                .insert(*building, 0);
        }
        _ => return Err(unavailable("Not a specialist action")),
    }
    Ok(())
}
fn validate_interaction(
    world: &World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
) -> Result<(), RejectReason> {
    if a.id == b.id {
        return Err(unavailable("Choose another target"));
    }
    match role(defs, a) {
        Role::Engineer if b.foot.is_some() => Ok(()),
        Role::Spy if b.foot.is_some() && b.owner != a.owner => Ok(()),
        _ if repairs_units(defs, b) => validate_docking(a, b, defs),
        _ => {
            if world
                .resource::<SpecialState>()
                .controlled
                .contains_key(&a.id)
            {
                return Err(unavailable("Controlled units cannot embark"));
            }
            let m = defs
                .metadata(b.item)
                .ok_or(unavailable("Target cannot carry passengers"))?;
            if a.owner != b.owner
                || m.capacity == 0
                || !matches!(m.role, Role::Transport | Role::Ifv)
                || defs
                    .metadata(a.item)
                    .is_some_and(|m| m.domain == Domain::Air)
                || a.foot.is_some()
                || world
                    .resource::<SpecialState>()
                    .cargo
                    .get(&a.id)
                    .is_some_and(|v| !v.is_empty())
            {
                return Err(unavailable("Choose a friendly transport"));
            }
            if m.domain != Domain::Amphibious && defs.category(a.item) != Category::Infantry {
                return Err(unavailable("This transport carries infantry only"));
            }
            if world
                .resource::<SpecialState>()
                .cargo
                .get(&b.id)
                .map_or(0, Vec::len)
                >= m.capacity
            {
                return Err(unavailable("Transport is full"));
            }
            Ok(())
        }
    }
}
fn interact(
    world: &mut World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
) -> Result<(), RejectReason> {
    validate_interaction(world, a, b, defs)?;
    match role(defs, a) {
        Role::Engineer if b.foot.is_some() => {
            world.entity_mut(b.entity).insert(Owner(a.owner));
            if let ProductionItem::Building(k) = b.item
                && let Some(t) = defs.building_combat(k)
            {
                world.entity_mut(b.entity).insert(Health {
                    hit_points: t.hit_points,
                });
            }
            if let Some(mut w) = world.get_mut::<Weapon>(b.entity) {
                w.clear();
            }
            world.resource_mut::<SpecialState>().charge.remove(&b.id);
            release(world, a);
            world.despawn(a.entity);
        }
        Role::Spy if b.foot.is_some() => {
            let tick = world.resource::<SimTick>().0;
            if let ProductionItem::Building(k) = b.item
                && let Some(def) = defs.building(k)
            {
                if def.credits_per_ore.is_some() {
                    let amount = world
                        .resource::<Players>()
                        .get(b.owner)
                        .map_or(0, |p| p.credits() / 2);
                    let mut ps = world.resource_mut::<Players>();
                    if let Some(p) = ps.get_mut(b.owner) {
                        p.credits -= amount;
                    }
                    if let Some(p) = ps.get_mut(a.owner) {
                        p.credits = p.credits.saturating_add(amount);
                    }
                } else if def.power_produced > 0 {
                    world
                        .resource_mut::<SpecialState>()
                        .outages
                        .insert(b.owner, tick.saturating_add(600));
                } else if defs.metadata(b.item).is_some_and(|m| m.role.superweapon()) {
                    world.resource_mut::<SpecialState>().charge.insert(b.id, 0);
                } else {
                    world
                        .resource_mut::<SpecialState>()
                        .disabled
                        .insert(b.id, tick.saturating_add(600));
                }
            }
            release(world, a);
            world.despawn(a.entity);
        }
        _ if repairs_units(defs, b) => {
            // The vehicle docks: it stays beside the building and `advance` repairs it.
            clear_orders(world, a.entity);
            if let Some(mut harvester) = world.get_mut::<crate::Harvester>(a.entity) {
                harvester.stop();
            }
            world
                .resource_mut::<SpecialState>()
                .docked
                .insert(a.id, b.id);
        }
        _ => {
            let ProductionItem::Unit(kind) = a.item else {
                return Err(unavailable("Only units can embark"));
            };
            let harvester = world.get::<crate::Harvester>(a.entity).cloned();
            let experience = world.get::<crate::Experience>(a.entity).copied();
            let bay = crate::flight::stow(world, defs, a);
            release(world, a);
            world.despawn(a.entity);
            world
                .resource_mut::<SpecialState>()
                .cargo
                .entry(b.id)
                .or_default()
                .push(Passenger {
                    id: a.id,
                    kind,
                    hp: a.hp,
                    harvester,
                    experience,
                    bay,
                });
            if role(defs, b) == Role::Ifv && role(defs, a) == Role::Engineer {
                world.entity_mut(b.entity).remove::<Weapon>();
            }
            if role(defs, b) == Role::Ifv
                && let Some(terms) = defs.unit_combat(kind)
                && let Some(w) = terms.weapon
            {
                world.entity_mut(b.entity).insert(Weapon::new(w));
            }
        }
    }
    Ok(())
}
fn unload(world: &mut World, a: &Actor, defs: &Definitions) -> Result<(), RejectReason> {
    let ps = world
        .resource::<SpecialState>()
        .cargo
        .get(&a.id)
        .cloned()
        .unwrap_or_default();
    if ps.is_empty() {
        return Err(unavailable("Transport is empty"));
    }
    let mut left = Vec::new();
    for p in ps {
        let g = world.resource::<Grid>();
        let view = g.navigation_for(defs.domain(p.kind));
        let on_foot = defs.on_foot(p.kind);
        let dest = Footprint {
            origin: a.cell,
            width: 1,
            height: 1,
        }
        .ring()
        .find(|&c| view.is_passable(c) && view.has_room(c, on_foot));
        let Some(c) = dest else {
            left.push(p);
            continue;
        };
        let Some(u) = defs.unit(p.kind) else {
            left.push(p);
            continue;
        };
        let point = {
            let mut g = world.resource_mut::<Grid>();
            g.register(p.id, defs.domain(p.kind), on_foot);
            if !g.claim(c, p.id) {
                // The cell refuses the passenger after all (not expected after the search
                // above): it stays aboard.
                g.unregister(p.id);
                left.push(p);
                continue;
            }
            g.standing_point(c, p.id)
        };
        let mut e = world.spawn(unit_bundle(p.id, a.owner, p.kind, u.speed, c, point));
        if let Some(t) = defs.unit_combat(p.kind) {
            let (c, w) = combat_components(t);
            e.insert(c);
            e.insert(Health { hit_points: p.hp });
            if let Some(w) = w {
                e.insert(w);
            }
        }
        if let Some(mut worker) = p.harvester {
            worker.state = crate::HarvestState::Seeking;
            e.insert(worker);
        }
        if let Some(experience) = p.experience {
            // It keeps its rank, and at elite rank its elite weapons (RA4).
            e.insert(experience);
            let entity = e.id();
            crate::veterancy::rearm(world, entity, p.kind);
        }
        if let Some(bay) = p.bay {
            // A spawner has the bay it went aboard with (RA4).
            world.resource_mut::<SpecialState>().bays.insert(p.id, bay);
        }
    }
    world
        .resource_mut::<SpecialState>()
        .cargo
        .insert(a.id, left);
    if role(defs, a) == Role::Ifv
        && world
            .resource::<SpecialState>()
            .cargo
            .get(&a.id)
            .is_none_or(Vec::is_empty)
        && let ProductionItem::Unit(k) = a.item
        && let Some(t) = defs.unit_combat(k)
        && let Some(w) = t.weapon
    {
        world.entity_mut(a.entity).insert(Weapon::new(w));
    }
    Ok(())
}

/// Pre-movement effects: charging, recovery, transport approach, chrono movement and support.
pub(crate) fn advance(world: &mut World) {
    if world.resource::<Definitions>().arsenal().0.is_empty() {
        return;
    }
    let defs = world.resource::<Definitions>().clone();
    let tick = world.resource::<SimTick>().0;
    // RA4: every trainable unit has its experience from its first tick on.
    crate::veterancy::enlist(world);
    let index = actors(world);
    {
        let mut state = world.resource_mut::<SpecialState>();
        state
            .protected
            .retain(|id, t| index.contains_key(id) && *t > tick);
        state
            .disabled
            .retain(|id, t| index.contains_key(id) && *t > tick);
        state.charge.retain(|id, _| index.contains_key(id));
        state.cargo.retain(|id, _| index.contains_key(id));
        state.rearming.retain(|id, _| index.contains_key(id));
        state
            .interactions
            .retain(|a, b| index.contains_key(a) && index.contains_key(b));
        state.outages.retain(|_, t| *t > tick);
        state
            .docked
            .retain(|a, b| index.contains_key(a) && index.contains_key(b));
    }
    let flights = world.resource::<SpecialState>().carrier_flights.clone();
    for (fighter, carrier) in flights {
        let Some(a) = index.get(&fighter) else {
            world
                .resource_mut::<SpecialState>()
                .carrier_flights
                .remove(&fighter);
            continue;
        };
        if !index.contains_key(&carrier) {
            if let Some(mut health) = world.get_mut::<Health>(a.entity) {
                health.hit_points = 0;
            }
        } else if world
            .get::<Weapon>(a.entity)
            .is_none_or(|w| w.order.is_none())
            && !world
                .resource::<SpecialState>()
                .rearming
                .contains_key(&fighter)
        {
            clear_orders(world, a.entity);
            world
                .resource_mut::<SpecialState>()
                .rearming
                .insert(fighter, 1);
        }
    }
    let controlled = world.resource::<SpecialState>().controlled.clone();
    for (id, (caster, owner)) in controlled {
        if !index.contains_key(&caster) {
            if let Some(a) = index.get(&id) {
                world.entity_mut(a.entity).insert(Owner(owner));
                clear_orders(world, a.entity);
            }
            world.resource_mut::<SpecialState>().controlled.remove(&id);
        } else if !index.contains_key(&id) {
            world.resource_mut::<SpecialState>().controlled.remove(&id);
        }
    }
    for a in index.values() {
        let Some(meta) = defs.metadata(a.item) else {
            continue;
        };
        let works = powered(world, &defs, a);
        if meta.role.superweapon()
            && charges(world, &defs, a)
            && !world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&a.id)
        {
            let mut s = world.resource_mut::<SpecialState>();
            let c = s.charge.entry(a.id).or_default();
            *c = c.saturating_add(1).min(meta.cooldown);
        }
        if let ProductionItem::Unit(k) = a.item
            && let Some(u) = defs.unit(k)
        {
            let disabled = world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&a.id);
            // With a rules file a vehicle or ship that is no longer green moves at three
            // quarters of its speed (RA3, docs/RA2_PARITY_PLAN.md section 4.1).
            let damaged = defs.rules().is_some()
                && meta.category == Category::Vehicles
                && meta.domain != Domain::Air
                && world
                    .get::<Condition>(a.entity)
                    .is_some_and(|c| c.0 != DamageState::Green);
            // RA4: the speed multiplier of the unit's rank and bonus, before the slowing.
            let speed = match world.get::<crate::Experience>(a.entity) {
                Some(experience) if experience.speed_pm() != 1000 => {
                    crate::veterancy::scaled_speed(u.speed, experience.speed_pm())
                }
                _ => u.speed,
            };
            if let Some(mut m) = world.get_mut::<Mobile>(a.entity) {
                m.speed = if disabled {
                    0
                } else if damaged {
                    speed - speed / 4
                } else {
                    speed
                };
            }
            // RA4: a unit that heals itself (by its type, or by an ability of its rank)
            // regains a hit point every repair interval of the rules, up to its type's.
            if let Some(rules) = defs.rules()
                && tick.is_multiple_of(u64::from(
                    rules.economy.repair_interval.max(1).unsigned_abs(),
                ))
                && (meta.self_healing
                    || world
                        .get::<crate::Experience>(a.entity)
                        .is_some_and(|experience| experience.self_heal))
                && let Some(terms) = defs.unit_combat(k)
                && let Some(mut health) = world.get_mut::<Health>(a.entity)
                && health.hit_points > 0
                && health.hit_points < terms.hit_points
            {
                health.hit_points += 1;
            }
        }
        if meta.role == Role::Chrono
            && !world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&a.id)
        {
            let dest = world
                .get::<Orders>(a.entity)
                .and_then(|o| o.current.as_ref().map(|o| o.destination));
            if let Some(dest) = dest
                && teleport(world, a, dest)
            {
                world
                    .resource_mut::<SpecialState>()
                    .disabled
                    .insert(a.id, tick.saturating_add(u64::from(meta.cooldown)));
            }
        }
        if meta.role == Role::ChronoMiner {
            let dock = world
                .get::<crate::Harvester>(a.entity)
                .and_then(|h| match h.state {
                    crate::HarvestState::ToRefinery { dock, .. } => Some(dock),
                    _ => None,
                });
            if let Some(dock) = dock {
                teleport(world, a, dock);
            }
        }
        let repair_ifv = meta.role == Role::Ifv
            && world
                .resource::<SpecialState>()
                .cargo
                .get(&a.id)
                .is_some_and(|ps| {
                    ps.iter().any(|p| {
                        defs.metadata(ProductionItem::Unit(p.kind))
                            .is_some_and(|m| m.role == Role::Engineer)
                    })
                });
        // The free repair of everything around a Service Depot (M9) is the rule of a world
        // without a rules file; with one a vehicle docks and pays (RA2, `repair_docked`).
        let aura = (meta.role == Role::Repair && defs.rules().is_none()) || repair_ifv;
        if aura && works && tick.is_multiple_of(20) {
            for u in index.values().filter(|u| {
                u.owner == a.owner
                    && u.alive()
                    && defs.category(u.item) == Category::Vehicles
                    && near(a.cell, u.cell, meta.radius)
            }) {
                if let ProductionItem::Unit(k) = u.item
                    && let Some(t) = defs.unit_combat(k)
                    && let Some(mut h) = world.get_mut::<Health>(u.entity)
                {
                    h.hit_points = h.hit_points.saturating_add(meta.strength).min(t.hit_points);
                }
            }
        }
        if world
            .resource::<SpecialState>()
            .rearming
            .contains_key(&a.id)
        {
            let carrier = world
                .resource::<SpecialState>()
                .carrier_flights
                .get(&a.id)
                .copied();
            let base = carrier.and_then(|id| index.get(&id)).or_else(|| index.values().filter(|b|b.owner==a.owner&&matches!(b.item,ProductionItem::Building(k) if defs.production(a.item).is_some_and(|p|defs.same_factory(k,p.producer)))).min_by_key(|b|a.cell.x.abs_diff(b.cell.x)+a.cell.y.abs_diff(b.cell.y)));
            if let Some(base) = base {
                if distance(a, base) <= 2 {
                    if carrier.is_some() {
                        release(world, a);
                        world.despawn(a.entity);
                        let mut state = world.resource_mut::<SpecialState>();
                        state.carrier_flights.remove(&a.id);
                        state.rearming.remove(&a.id);
                        continue;
                    }
                    let mut s = world.resource_mut::<SpecialState>();
                    if let Some(t) = s.rearming.get_mut(&a.id) {
                        *t = t.saturating_sub(1);
                        if *t == 0 {
                            s.rearming.remove(&a.id);
                        }
                    }
                } else if world
                    .get::<Orders>(a.entity)
                    .is_some_and(|o| o.current.is_none())
                {
                    let dest = {
                        let g = world.resource::<Grid>();
                        let view = g.navigation_for(Domain::Air);
                        crate::building::exit_cell(
                            &view,
                            base.foot.unwrap_or(Footprint {
                                origin: base.cell,
                                width: 1,
                                height: 1,
                            }),
                            None,
                            false,
                        )
                    };
                    if let Some(c) = dest
                        && let Some(mut o) = world.get_mut::<Orders>(a.entity)
                    {
                        o.current = Some(MoveOrder::new(c, tick));
                    }
                }
            }
        }
    }
    let pending = world.resource::<SpecialState>().interactions.clone();
    for (id, target) in pending {
        let (Some(a), Some(b)) = (index.get(&id), index.get(&target)) else {
            continue;
        };
        // Neither acts nor is acted on once its hit points are gone (a strike of this tick's
        // commands): it is removed in `Cleanup`, and the order with it.
        if !a.alive() || !b.alive() || validate_interaction(world, a, b, &defs).is_err() {
            world
                .resource_mut::<SpecialState>()
                .interactions
                .remove(&id);
            continue;
        }
        if distance(a, b) <= 1 {
            let _ = interact(world, a, b, &defs);
            world
                .resource_mut::<SpecialState>()
                .interactions
                .remove(&id);
        } else if world
            .get::<Orders>(a.entity)
            .is_some_and(|o| o.current.is_none())
        {
            let dest = {
                let g = world.resource::<Grid>();
                let view = g.navigation_for(g.domain_of(id));
                let on_foot = g.is_on_foot(id);
                crate::pathfinding::nearest_cell(&view, a.cell, |c| {
                    view.has_room(c, on_foot)
                        && b.foot
                            .map_or(near(c, b.cell, 1), |f| f.distance_to_cell(c) == 1)
                })
            };
            if let Some(c) = dest
                && let Some(mut o) = world.get_mut::<Orders>(a.entity)
            {
                o.current = Some(MoveOrder::new(c, tick));
            } else {
                world
                    .resource_mut::<SpecialState>()
                    .interactions
                    .remove(&id);
            }
        }
    }
    repair_docked(world, &defs, &index, tick);
    crate::flight::advance(world, &defs, &index, tick);
}

/// Repairs the vehicles docked at a building that repairs units (RA2, docs/RA2_PARITY_PLAN.md
/// section 4.6), in id order: every `unit_repair_interval` ticks a docked vehicle regains
/// `repair_step` hit points (at most what it lacks) for the step cost of its own price and hit
/// points ([`repair_step_cost`](crate::repair_step_cost)). A step its owner cannot pay for is
/// skipped (invariant I7), as is every step while a spy has switched the building off. The
/// vehicle leaves the dock when it is whole (it then moves a cell away, to make room), when it
/// is no longer beside the building or has an order to move, and when the vehicle or the
/// building is lost or changes owner.
fn repair_docked(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>, tick: u64) {
    let Some(rules) = defs.rules().map(|rules| rules.economy) else {
        return;
    };
    let docked = world.resource::<SpecialState>().docked.clone();
    let step_now = tick.is_multiple_of(u64::from(rules.unit_repair_interval.max(1).unsigned_abs()));
    for (id, building) in docked {
        let stays = (|| {
            let (a, b) = (index.get(&id)?, index.get(&building)?);
            let ProductionItem::Unit(kind) = a.item else {
                return None;
            };
            let max = defs.unit_combat(kind)?.hit_points;
            let moving = world
                .get::<Orders>(a.entity)
                .is_none_or(|o| o.current.is_some());
            if a.owner != b.owner
                || !repairs_units(defs, b)
                || distance(a, b) > 1
                || moving
                || a.hp <= 0
            {
                return None;
            }
            if a.hp >= max {
                // Whole: it makes room for the next vehicle.
                let dest = {
                    let g = world.resource::<Grid>();
                    let view = g.navigation_for(g.domain_of(id));
                    let on_foot = g.is_on_foot(id);
                    crate::pathfinding::nearest_cell(&view, a.cell, |c| {
                        view.has_room(c, on_foot)
                            && b.foot.is_some_and(|f| f.distance_to_cell(c) >= 2)
                    })
                };
                if let Some(c) = dest
                    && let Some(mut o) = world.get_mut::<Orders>(a.entity)
                {
                    o.current = Some(MoveOrder::new(c, tick));
                }
                return None;
            }
            let off = world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&building);
            if step_now && !off {
                let price = defs.production(a.item).map_or(0, |terms| terms.cost);
                let mut players = world.resource_mut::<Players>();
                let state = players.get_mut(a.owner)?;
                let cost = crate::repair_step_cost(&rules, price, max, state.super_me);
                if state.credits >= cost {
                    state.credits -= cost;
                    let heal = rules.repair_step.max(1).min(max.saturating_sub(a.hp));
                    if let Some(mut h) = world.get_mut::<Health>(a.entity) {
                        h.hit_points = h.hit_points.saturating_add(heal).min(max);
                    }
                }
            }
            Some(())
        })();
        if stays.is_none() {
            world.resource_mut::<SpecialState>().docked.remove(&id);
        }
    }
}

/// Specialist weapons share target selection and reload with ordinary weapons. Effects are
/// resolved in shot order, before cleanup, then persistent bombs and radiation advance.
pub(crate) fn resolve(world: &mut World) {
    if world.resource::<Definitions>().arsenal().0.is_empty() {
        return;
    }
    let defs = world.resource::<Definitions>().clone();
    let tick = world.resource::<SimTick>().0;
    let index = actors(world);
    let events: Vec<_> = world
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    for event in events {
        if let SimEvent::WeaponFired {
            attacker,
            target,
            weapon,
        } = event
        {
            let Some(a) = index.get(&attacker) else {
                continue;
            };
            // RA4: ammunition, launchers and the surfacing of a cloaked unit, for every shot,
            // whatever it is fired at (scenery is no actor).
            crate::flight::fired(world, &defs, a, target, weapon, tick);
            let Some(b) = index.get(&target) else {
                continue;
            };
            let Some(meta) = defs.metadata(a.item) else {
                continue;
            };
            match meta.role {
                Role::MindControl if b.foot.is_none() => {
                    if !world
                        .resource::<SpecialState>()
                        .controlled
                        .values()
                        .any(|(c, _)| *c == a.id)
                        && role(&defs, b) != Role::MindControl
                    {
                        world
                            .resource_mut::<SpecialState>()
                            .controlled
                            .insert(b.id, (a.id, b.owner));
                        world.entity_mut(b.entity).insert(Owner(a.owner));
                        clear_orders(world, b.entity);
                    }
                }
                Role::Drone | Role::Squid | Role::Chrono => {
                    world
                        .resource_mut::<SpecialState>()
                        .disabled
                        .insert(b.id, tick.saturating_add(u64::from(meta.cooldown)));
                    clear_orders(world, b.entity);
                }
                Role::Demolition => {
                    // With a rules file the bomb goes off after the ini's `IvanTimedDelay`.
                    let delay = defs.rules().map_or(u64::from(meta.cooldown), |rules| {
                        u64::from(rules.combat.ivan_delay.max(1).unsigned_abs())
                    });
                    world
                        .resource_mut::<SpecialState>()
                        .bombs
                        .entry(b.id)
                        .or_insert((tick.saturating_add(delay), meta.strength, meta.radius));
                }
                Role::Suicide => {
                    // With a rules file the blast is the impact of the unit's own weapon where
                    // it stands (`combat` deals no other damage for the shot).
                    let own = defs
                        .rules()
                        .and(defs.weapon(weapon))
                        .map(|def| (def.warhead, def.damage));
                    match own {
                        Some((warhead, damage)) => detonate(
                            world,
                            Blast {
                                point: position_of(world, a),
                                air: false,
                                warhead,
                                damage,
                                direct: None,
                                firepower_pm: 1000,
                                source: Some(a.id),
                            },
                            true,
                        ),
                        None => blast(world, a.cell, meta.radius, meta.strength),
                    }
                    if let Some(mut h) = world.get_mut::<Health>(a.entity) {
                        h.hit_points = 0;
                    }
                }
                // Without a rules file the shot of a splash unit also blasts the cells around
                // its target; with one, the spread of its warhead does (`combat`).
                Role::Splash if defs.rules().is_none() => {
                    blast(world, b.cell, meta.radius, meta.strength);
                }
                Role::Commando if b.foot.is_some() => {
                    let point = b.foot.map_or(crate::SubPos::centre_of(b.cell), |foot| {
                        crate::SubPos::centre_of(nearest_cell_of(foot, a.cell))
                    });
                    if !area(world, Effect::C4, point, Some(b.id)) {
                        hurt(world, b, meta.strength.saturating_mul(10));
                    }
                }
                Role::Carrier => launch_fighters(world, a, b, &defs, meta.capacity),
                // A strike aircraft turns for home after the last shot of its volley.
                Role::Strike
                    if world
                        .get::<Weapon>(a.entity)
                        .is_none_or(|w| w.burst_left == 0) =>
                {
                    clear_orders(world, a.entity);
                    world
                        .resource_mut::<SpecialState>()
                        .rearming
                        .insert(a.id, meta.cooldown);
                }
                _ => {}
            }
        }
        if let SimEvent::UnitSpawned {
            player,
            unit,
            building: source,
        } = event
            && let Some(a) = index.get(&unit)
            && defs.category(a.item) == Category::Infantry
            && index
                .get(&source)
                .is_none_or(|b| role(&defs, b) != Role::Cloner)
        {
            let cloner = index.values().find(|b| {
                b.owner == player && role(&defs, b) == Role::Cloner && powered(world, &defs, b)
            });
            if let Some(b) = cloner
                && let ProductionItem::Unit(k) = a.item
                && let Some(mut building) = world.get_mut::<Building>(b.entity)
                && building.pending_units.len() < crate::economy::MAX_QUEUE_LENGTH
            {
                building.pending_units.push(k);
            }
        }
        if let SimEvent::OreDelivered {
            player, credits, ..
        } = event
            && index.values().any(|b| {
                b.owner == player && role(&defs, b) == Role::Purifier && powered(world, &defs, b)
            })
            && let Some(p) = world.resource_mut::<Players>().get_mut(player)
        {
            p.credits = p.credits.saturating_add(credits / 4);
        }
    }
    let bombs = world.resource::<SpecialState>().bombs.clone();
    for (id, (due, damage, radius)) in bombs {
        if due <= tick || !index.contains_key(&id) {
            if let Some(a) = index.get(&id)
                && !area(world, Effect::IvanBomb, position_of(world, a), Some(a.id))
            {
                blast(world, a.cell, radius, damage);
            }
            world.resource_mut::<SpecialState>().bombs.remove(&id);
        }
    }
    let zones = world.resource::<SpecialState>().zones.clone();
    if tick.is_multiple_of(20) {
        for z in &zones {
            if z.role == Role::Radiation {
                for a in actors(world).values() {
                    if role(&defs, a) != Role::Radiation
                        && !matches!(a.item,ProductionItem::Unit(k) if defs.domain(k)==Domain::Air)
                        && near(a.cell, z.cell, z.radius)
                    {
                        // With the effect listed, a pulse is an impact of the radiation
                        // warhead on everything in the zone, each where it is.
                        let point = a.foot.map_or(position_of(world, a), |foot| {
                            crate::SubPos::centre_of(nearest_cell_of(foot, z.cell))
                        });
                        if !area(world, Effect::Radiation, point, Some(a.id)) {
                            hurt(world, a, z.damage);
                        }
                    }
                }
            } else if !area(
                world,
                Effect::Lightning,
                crate::SubPos::centre_of(z.cell),
                None,
            ) {
                blast(world, z.cell, z.radius, z.damage);
            }
        }
    }
    let mut s = world.resource_mut::<SpecialState>();
    for z in &mut s.zones {
        z.remaining = z.remaining.saturating_sub(1);
    }
    s.zones.retain(|z| z.remaining > 0);
}

/// Powered reconnaissance participates in the existing vision grid. Concealment is an
/// additional per-observer target filter, also consumed by the presentation layer.
pub(crate) fn reconnaissance(world: &mut World) {
    if world.resource::<Definitions>().arsenal().0.is_empty() {
        return;
    }
    let defs = world.resource::<Definitions>().clone();
    let index = actors(world);
    let players: Vec<_> = world.resource::<Players>().iter().map(|(p, _)| p).collect();
    let mut hidden = BTreeSet::new();
    for a in index.values() {
        let Some(m) = defs.metadata(a.item) else {
            continue;
        };
        if m.role == Role::Satellite && powered(world, &defs, a) {
            let (gw, gh) = {
                let g = world.resource::<Grid>();
                (g.width(), g.height())
            };
            if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                p.vision.reveal((0, 0, i64::from(gw), i64::from(gh)), 0);
            }
        }
    }
    for a in index.values() {
        let Some(m) = defs.metadata(a.item) else {
            continue;
        };
        if m.role == Role::Gap && powered(world, &defs, a) {
            for &p in &players {
                if p != a.owner
                    && let Some(p) = world.resource_mut::<Players>().get_mut(p)
                {
                    p.vision.conceal(a.cell, m.radius);
                }
            }
        }
        let concealed = m.role == Role::Spy
            || (m.role == Role::Mirage
                && world
                    .get::<Orders>(a.entity)
                    .is_some_and(|o| o.current.is_none())
                && world
                    .get::<Weapon>(a.entity)
                    .is_some_and(|w| w.cooldown == 0));
        if concealed {
            for &p in &players {
                if p == a.owner {
                    continue;
                }
                let detected = index.values().any(|b| {
                    b.owner == p
                        && near(
                            a.cell,
                            b.cell,
                            if role(&defs, b) == Role::Dog
                                || (role(&defs, b) == Role::Sensor && powered(world, &defs, b))
                            {
                                6
                            } else {
                                2
                            },
                        )
                });
                if !detected {
                    hidden.insert((p, a.id));
                }
            }
        }
    }
    // RA4 (docs/RA2_PARITY_PLAN.md section 4.8): a cloakable unit is hidden from every enemy
    // of its owner that has no unit or building with sensors near enough, unless it fired
    // within the rules' cloak delay.
    if defs.rules().is_some() {
        let tick = world.resource::<SimTick>().0;
        let sensors: Vec<(&Actor, u32)> = index
            .values()
            .filter_map(|b| {
                let sight = defs.metadata(b.item).map_or(0, |entry| entry.sensors_sight);
                (sight > 0 && b.hp > 0).then_some((b, sight))
            })
            .collect();
        for a in index.values() {
            if !defs.metadata(a.item).is_some_and(|entry| entry.cloakable) {
                continue;
            }
            if world
                .resource::<SpecialState>()
                .surfaced
                .get(&a.id)
                .is_some_and(|&until| until > tick)
            {
                continue;
            }
            for &p in &players {
                if !world.resource::<Players>().hostile(p, a.owner) {
                    continue;
                }
                let detected = sensors.iter().any(|&(b, sight)| {
                    b.owner == p
                        && near(
                            a.cell,
                            b.foot.map_or(b.cell, |foot| nearest_cell_of(foot, a.cell)),
                            sight,
                        )
                });
                if !detected {
                    hidden.insert((p, a.id));
                }
            }
        }
    }
    // Direct local sight counters gap shrouding; a satellite alone does not.
    if index.values().any(|a| role(&defs, a) == Role::Gap) {
        for a in index.values() {
            if let Some(sight) = world.get::<crate::Sight>(a.entity).copied() {
                let f = a.foot.unwrap_or(Footprint {
                    origin: a.cell,
                    width: 1,
                    height: 1,
                });
                if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                    p.vision.reveal(
                        (
                            i64::from(f.origin.x),
                            i64::from(f.origin.y),
                            i64::from(f.origin.x) + i64::from(f.width) - 1,
                            i64::from(f.origin.y) + i64::from(f.height) - 1,
                        ),
                        sight.0,
                    );
                }
            }
        }
    }
    world.resource_mut::<SpecialState>().hidden = hidden;
}

/// Carrier sorties reuse the ordinary aircraft movement, targeting and damage path.
fn launch_fighters(
    world: &mut World,
    carrier: &Actor,
    target: &Actor,
    defs: &Definitions,
    capacity: usize,
) {
    let Some(kind) = defs.arsenal().0.iter().find_map(|(item, meta)| {
        if meta.role == Role::Strike
            && let ProductionItem::Unit(k) = item
        {
            return Some(*k);
        }
        None
    }) else {
        return;
    };
    let Some(unit) = defs.unit(kind) else {
        return;
    };
    let active = world
        .resource::<SpecialState>()
        .carrier_flights
        .values()
        .filter(|id| **id == carrier.id)
        .count();
    for _ in active..capacity {
        let cell = {
            let grid = world.resource::<Grid>();
            let view = grid.navigation_for(Domain::Air);
            crate::building::exit_cell(
                &view,
                Footprint {
                    origin: carrier.cell,
                    width: 1,
                    height: 1,
                },
                None,
                false,
            )
        };
        let Some(cell) = cell else {
            break;
        };
        let Ok(id) = crate::spawn_unit(world, carrier.owner, &unit.id, cell) else {
            break;
        };
        let entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, i)| **i == id)
            .map(|(e, _)| e)
            .unwrap();
        if let Some(mut weapon) = world.get_mut::<Weapon>(entity) {
            weapon.order = Some(crate::combat::AttackOrder::Attack {
                target: target.id,
                chases: 0,
            });
        }
        world
            .resource_mut::<SpecialState>()
            .carrier_flights
            .insert(id, carrier.id);
    }
}
