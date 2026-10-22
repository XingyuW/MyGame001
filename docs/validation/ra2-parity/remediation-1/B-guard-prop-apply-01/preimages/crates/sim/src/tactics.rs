//! RA7 persistent area guard, follow and bounded movement waypoints. Commands validate before
//! writing these ordered maps; the schedule consumes them before pathfinding. No client state.
use crate::combat::{AttackOrder, Body};
use crate::hash::Fnv1a64;
use crate::unit::MoveOrder;
use crate::{
    CellPos, Definitions, Grid, Health, Mobile, Orders, Owner, Players, ProductionItem, SimId,
    SimTick, SpecialState, Weapon,
};
use bevy_ecs::{resource::Resource, world::World};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_WAYPOINTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TacticalOrder {
    Guard { anchor: CellPos },
    Follow { target: SimId },
}
#[derive(Resource, Default, Debug)]
pub struct TacticalState {
    pub orders: BTreeMap<SimId, TacticalOrder>,
    pub waypoints: BTreeMap<SimId, VecDeque<CellPos>>,
}
impl TacticalState {
    pub(crate) fn clear(&mut self, id: SimId) {
        self.orders.remove(&id);
        self.waypoints.remove(&id);
    }
    pub(crate) fn hash_into(&self, h: &mut Fnv1a64) {
        if self.orders.is_empty() && self.waypoints.is_empty() {
            return;
        }
        h.write(b"tactics");
        h.write_u64(self.orders.len() as u64);
        for (id, order) in &self.orders {
            h.write_u32(id.0);
            match order {
                TacticalOrder::Guard { anchor } => {
                    h.write(&[0]);
                    cell(h, *anchor);
                }
                TacticalOrder::Follow { target } => {
                    h.write(&[1]);
                    h.write_u32(target.0);
                }
            }
        }
        h.write_u64(self.waypoints.len() as u64);
        for (id, cells) in &self.waypoints {
            h.write_u32(id.0);
            h.write_u64(cells.len() as u64);
            for c in cells {
                cell(h, *c);
            }
        }
    }
}
fn cell(h: &mut Fnv1a64, c: CellPos) {
    h.write(&c.x.to_le_bytes());
    h.write(&c.y.to_le_bytes());
}

pub(crate) fn advance(world: &mut World) {
    if world.resource::<TacticalState>().orders.is_empty()
        && world.resource::<TacticalState>().waypoints.is_empty()
    {
        return;
    }
    let mut actors = BTreeMap::new();
    for (e, id, owner, mobile, building, h) in world
        .query::<(
            bevy_ecs::entity::Entity,
            &SimId,
            &Owner,
            Option<&Mobile>,
            Option<&crate::Building>,
            &Health,
        )>()
        .iter(world)
    {
        if h.hit_points > 0
            && let Some(body) = Body::of(mobile, building)
        {
            actors.insert(
                *id,
                (
                    e,
                    owner.0,
                    mobile
                        .map(|m| m.cell)
                        .or_else(|| building.map(|b| b.footprint.origin))
                        .unwrap_or(CellPos::new(0, 0)),
                    body,
                ),
            );
        }
    }
    let mut state = world.remove_resource::<TacticalState>().unwrap_or_default();
    state.orders.retain(|id, _| actors.contains_key(id));
    state.waypoints.retain(|id, _| actors.contains_key(id));
    let tick = world.resource::<SimTick>().0;
    let mut forgotten = Vec::new();
    for (&id, &order) in &state.orders {
        let Some(&(e, owner, at, _)) = actors.get(&id) else {
            continue;
        };
        if world.resource::<SpecialState>().erasures.contains_key(&id) {
            continue;
        }
        match order {
            TacticalOrder::Follow { target } => {
                let followed = actors.get(&target).filter(|(_, o, _, body)| {
                    *o == owner
                        || (world
                            .resource::<Players>()
                            .get(owner)
                            .is_some_and(|p| body.seen_by(p.vision()))
                            && !world
                                .resource::<SpecialState>()
                                .hidden
                                .contains(&(owner, target)))
                });
                if let Some((_, _, cell, _)) = followed {
                    if at.x.abs_diff(cell.x).max(at.y.abs_diff(cell.y)) > 1 {
                        let domain = world.resource::<Grid>().domain_of(id);
                        let dest = crate::destination_cells(
                            &world.resource::<Grid>().navigation_for(domain),
                            *cell,
                            1,
                            |c| !world.resource::<Grid>().room_for(c, id),
                        )
                        .first()
                        .copied();
                        if let Some(dest) = dest
                            && let Some(mut o) = world.get_mut::<Orders>(e)
                            && o.current.as_ref().is_none_or(|o| o.destination != dest)
                        {
                            o.current = Some(MoveOrder::new(dest, tick));
                        }
                    } else if let Some(mut o) = world.get_mut::<Orders>(e) {
                        o.current = None;
                    }
                } else {
                    forgotten.push(id);
                    if let Some(mut o) = world.get_mut::<Orders>(e) {
                        o.current = None;
                    }
                }
            }
            TacticalOrder::Guard { anchor } => {
                // Explicit GuardRange, or reference weapon Range when absent, bounds
                // area acquisition and pursuit (R85).
                let explicit = world
                    .get::<crate::Unit>(e)
                    .and_then(|u| {
                        world
                            .resource::<Definitions>()
                            .metadata(ProductionItem::Unit(u.kind))
                    })
                    .map_or(0, |m| m.guard_range);
                let range = if explicit > 0 {
                    explicit.saturating_mul(256)
                } else {
                    world
                        .get::<Weapon>(e)
                        .and_then(|w| world.resource::<Definitions>().weapon(w.kind))
                        .map_or(0, |w| w.range)
                };
                let chosen = actors
                    .iter()
                    .filter(|(target, (target_entity, o, c, body))| {
                        world.resource::<Players>().hostile(owner, *o)
                            && !world
                                .resource::<SpecialState>()
                                .hidden
                                .contains(&(owner, **target))
                            && world
                                .resource::<Players>()
                                .get(owner)
                                .is_some_and(|p| body.seen_by(p.vision()))
                            && crate::combat::in_range(
                                (i64::from(c.x) - i64::from(anchor.x)) * 256,
                                (i64::from(c.y) - i64::from(anchor.y)) * 256,
                                range,
                            )
                            && crate::combat::guard_can_acquire(
                                world,
                                e,
                                id,
                                *target_entity,
                                **target,
                                body,
                            )
                    })
                    .map(|(id, (_, _, c, _))| {
                        (at.x.abs_diff(c.x).saturating_add(at.y.abs_diff(c.y)), *id)
                    })
                    .min()
                    .map(|(_, id)| id);
                if let Some(mut w) = world.get_mut::<Weapon>(e) {
                    w.order = chosen.map(|target| AttackOrder::Attack { target, chases: 0 });
                    w.target = chosen;
                }
                if chosen.is_none()
                    && at != anchor
                    && let Some(mut o) = world.get_mut::<Orders>(e)
                    && o.current.is_none()
                {
                    o.current = Some(MoveOrder::new(anchor, tick));
                }
            }
        }
    }
    for id in forgotten {
        state.orders.remove(&id);
    }
    for (&id, queue) in &mut state.waypoints {
        let Some(&(e, _, _, _)) = actors.get(&id) else {
            continue;
        };
        if world.get::<Orders>(e).is_some_and(|o| o.current.is_none())
            && let Some(target) = queue.pop_front()
        {
            let crusher = world
                .get::<crate::Unit>(e)
                .and_then(|u| {
                    world
                        .resource::<Definitions>()
                        .metadata(ProductionItem::Unit(u.kind))
                })
                .is_some_and(|m| m.crusher);
            if let Some(mut o) = world.get_mut::<Orders>(e) {
                o.current = Some(MoveOrder::commanded(target, tick, crusher));
            }
        }
    }
    state.waypoints.retain(|_, q| !q.is_empty());
    world.insert_resource(state);
}

/// Enemy order information granted to a powered Psychic Sensor's owner. No IDs, hidden
/// sources, own/allied orders or dead actors are exposed. Coordinates describe detected orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PsychicOrder {
    pub from: CellPos,
    pub target: CellPos,
}
pub fn psychic_orders(world: &World, viewer: crate::PlayerId) -> Vec<PsychicOrder> {
    let Some(defs) = world.get_resource::<Definitions>() else {
        return Vec::new();
    };
    let Some(players) = world.get_resource::<Players>() else {
        return Vec::new();
    };
    let mut sensors = Vec::new();
    let mut actors = BTreeMap::new();
    for e in world.iter_entities() {
        let (Some(id), Some(o), Some(h)) = (e.get::<SimId>(), e.get::<Owner>(), e.get::<Health>())
        else {
            continue;
        };
        if h.hit_points <= 0 {
            continue;
        }
        let c = e
            .get::<Mobile>()
            .map(|m| m.cell)
            .or_else(|| e.get::<crate::Building>().map(|b| b.footprint.origin));
        let Some(c) = c else {
            continue;
        };
        actors.insert(*id, c);
        if o.0 == viewer
            && let Some(b) = e.get::<crate::Building>()
            && let Some(m) = defs.metadata(ProductionItem::Building(b.kind))
            && m.role == crate::Role::Sensor
            && players.get(viewer).is_some_and(|p| p.powers(defs, b.kind))
        {
            sensors.push((c, m.radius));
        }
    }
    if sensors.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for e in world.iter_entities() {
        let (Some(id), Some(o), Some(h), Some(m)) = (
            e.get::<SimId>(),
            e.get::<Owner>(),
            e.get::<Health>(),
            e.get::<Mobile>(),
        ) else {
            continue;
        };
        if h.hit_points <= 0
            || !players.hostile(viewer, o.0)
            || world
                .resource::<SpecialState>()
                .hidden
                .contains(&(viewer, *id))
        {
            continue;
        }
        let target = e
            .get::<Weapon>()
            .and_then(|w| w.order)
            .and_then(|o| match o {
                AttackOrder::Attack { target, .. } => actors.get(&target).copied(),
                AttackOrder::AttackMove { destination, .. } => Some(destination),
                AttackOrder::ForceFire { target } => match target {
                    crate::ForceTarget::Cell(c) => Some(c),
                    crate::ForceTarget::Entity(id) => actors.get(&id).copied(),
                },
            })
            .or_else(|| {
                e.get::<Orders>()
                    .and_then(|o| o.current.as_ref())
                    .map(|o| o.destination)
            });
        if let Some(target) = target
            && sensors.iter().any(|(c, r)| {
                crate::combat::in_range(
                    i64::from(c.x) - i64::from(target.x),
                    i64::from(c.y) - i64::from(target.y),
                    *r,
                )
            })
        {
            lines.push((
                *id,
                PsychicOrder {
                    from: m.cell,
                    target,
                },
            ));
        }
    }
    lines.sort_by_key(|(id, _)| *id);
    lines.into_iter().map(|(_, line)| line).collect()
}
