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
pub(crate) fn actors(world: &mut World) -> BTreeMap<SimId, Actor> {
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

fn weapon_reload(world: &World, actor: &Actor, ticks: u32) -> u32 {
    let rank = world
        .get::<crate::Experience>(actor.entity)
        .map_or(1000, crate::Experience::reload_pm);
    crate::veterancy::reload_ticks(
        ticks,
        crate::combat::ai_reload(world.resource::<Players>(), actor.owner, rank),
    )
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
fn validate_docking(
    world: &World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
    check_damage: bool,
) -> Result<(), RejectReason> {
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
    if check_damage
        && defs
            .unit_combat(kind)
            .is_none_or(|terms| a.hp >= terms.hit_points)
        && !world
            .resource::<SpecialState>()
            .parasites
            .contains_key(&a.id)
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
    if let Some(id) = world.get::<SimId>(e).copied() {
        world.resource_mut::<crate::TacticalState>().clear(id);
    }
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
    if world.resource::<SpecialState>().erasures.contains_key(&id) {
        return Err(unavailable("Target is being erased"));
    }
    commandable_except_erasure(world, id)
}

fn commandable_except_erasure(world: &World, id: SimId) -> Result<(), RejectReason> {
    if world.resource::<SpecialState>().spawned.contains_key(&id)
        || world
            .resource::<SpecialState>()
            .strategic
            .flights
            .contains_key(&id)
    {
        return Err(RejectReason::NotCommandable(id));
    }
    Ok(())
}
pub(crate) fn unavailable(s: &'static str) -> RejectReason {
    RejectReason::AbilityUnavailable(s)
}
/// `hp` of `from` hit points as the same share of `to` hit points, at least 1 and at most `to`
/// (deploying and undeploying keep the share, rounded down).
fn share_of(hp: i32, from: i32, to: i32) -> i32 {
    let share = i64::from(hp.max(0)).saturating_mul(i64::from(to)) / i64::from(from.max(1));
    i32::try_from(share).unwrap_or(to).clamp(1, to.max(1))
}

mod advance;
mod carrier;
mod commands;
pub(crate) use commands::{bridge_goal_ready, service_intent_valid};
mod interaction;
mod reconnaissance;
mod repair;
mod resolve;
mod state;
pub(crate) mod superweapons;
pub(crate) use superweapons::{
    strategic_intent_erasure_only, strategic_intent_power_only, strategic_intent_valid,
};
mod transport;
pub(crate) mod vehicles;

pub(crate) use advance::advance;
use carrier::launch_fighters;
pub(crate) use commands::apply;
use interaction::{interact, validate_interaction};
pub(crate) use reconnaissance::{reconnaissance, refresh_concealment};
use repair::repair_docked;
pub(crate) use resolve::resolve;
pub use state::{Passenger, SpecialState, Zone};
pub(crate) use transport::unload;
use transport::unload_ready;

pub(crate) mod infantry;
pub(crate) mod radiation;

/// Context action the player's unit may perform on a known target. Runs the same read-only
/// validator as Interact, so cursor hints cannot disclose hidden targets or promise invalid jobs.
pub fn interaction_label(
    world: &World,
    player: PlayerId,
    unit: SimId,
    target: SimId,
) -> Option<&'static str> {
    fn actor(e: bevy_ecs::world::EntityRef<'_>) -> Option<Actor> {
        let id = *e.get::<SimId>()?;
        let owner = e.get::<Owner>()?.0;
        let hp = e.get::<Health>()?.hit_points;
        let (item, cell, foot) = if let Some(u) = e.get::<Unit>() {
            (ProductionItem::Unit(u.kind), e.get::<Mobile>()?.cell, None)
        } else {
            let b = e.get::<Building>()?;
            (
                ProductionItem::Building(b.kind),
                b.footprint.origin,
                Some(b.footprint),
            )
        };
        (hp > 0).then_some(Actor {
            entity: e.id(),
            id,
            owner,
            item,
            cell,
            foot,
            hp,
        })
    }
    let defs = world.get_resource::<Definitions>()?;
    let a = world
        .iter_entities()
        .find(|e| e.get::<SimId>() == Some(&unit))
        .and_then(actor)?;
    if a.owner != player || commandable(world, unit).is_err() {
        return None;
    }
    if let Some((cell, _)) = world.resource::<crate::Props>().get(target) {
        if !world
            .resource::<Players>()
            .get(player)?
            .vision()
            .is_visible(cell)
        {
            return None;
        }
        return crate::garrison::validate(world, defs, &a, target)
            .ok()
            .map(|_| "GARRISON");
    }
    let b = world
        .iter_entities()
        .find(|e| e.get::<SimId>() == Some(&target))
        .and_then(actor)?;
    if (b.owner != player && !known_to(world, defs, player, &b))
        || validate_interaction(world, &a, &b, defs).is_err()
    {
        return None;
    }
    let m = defs.metadata(a.item)?;
    let n = defs.metadata(b.item)?;
    Some(if n.role == Role::Transport || n.role == Role::Ifv {
        "BOARD"
    } else if n.hospital {
        "HEAL"
    } else if n.unit_repair {
        "REPAIR"
    } else if m.role == Role::Engineer {
        if a.owner == b.owner || n.bridge_repair_hut {
            "REPAIR"
        } else {
            "CAPTURE"
        }
    } else if m.role == Role::Spy {
        if b.foot.is_some() {
            "INFILTRATE"
        } else {
            "DISGUISE"
        }
    } else if m.c4 || m.role == Role::Commando {
        "C4"
    } else {
        "INTERACT"
    })
}
