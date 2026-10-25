//! Deterministic BombSight selection and DEFUSE-only approach jobs.
use super::*;
use crate::combat::{Extent, in_range, inside_min_range};
#[derive(Clone, Copy)]
struct Host {
    id: SimId,
    extent: Extent,
}
fn extent(world: &World, a: &Actor) -> Extent {
    a.foot.map_or_else(
        || {
            Extent::point(
                world
                    .get::<Position>(a.entity)
                    .map_or(crate::SubPos::centre_of(a.cell), |p| p.0),
            )
        },
        Extent::footprint,
    )
}
fn host(
    world: &World,
    defs: &Definitions,
    index: &BTreeMap<SimId, Actor>,
    a: &Actor,
    id: SimId,
) -> Option<Host> {
    if !world
        .resource::<SpecialState>()
        .charges
        .get(&id)
        .is_some_and(|c| !c.c4)
    {
        return None;
    }
    if let Some(b) = index.get(&id) {
        if !b.alive()
            || world
                .get::<Health>(b.entity)
                .is_none_or(|h| h.hit_points <= 0)
            || world.get::<Owner>(b.entity).is_none_or(|o| o.0 != b.owner)
            || (b.owner != a.owner && !known_to(world, defs, a.owner, b))
        {
            return None;
        }
        return Some(Host {
            id,
            extent: extent(world, b),
        });
    }
    // S's actual mapped House/CityBlock/Tower target; no forged Actor/Entity.
    // The sibling target/known contract leaves Trees/Bridges unmapped.
    let b = scenery::target(world, defs, id)?;
    if b.hp <= 0 || !scenery::known(world, a.owner, b) {
        return None;
    }
    Some(Host {
        id,
        extent: Extent::footprint(Footprint {
            origin: b.cell,
            width: 1,
            height: 1,
        }),
    })
}
fn kit(defs: &Definitions, a: &Actor) -> Option<crate::WeaponDef> {
    let ProductionItem::Unit(k) = a.item else {
        return None;
    };
    let w = defs.weapon(defs.unit_combat(k)?.weapon?)?;
    defs.warhead(w.warhead)
        .filter(|h| h.bomb_disarm)
        .map(|_| w.clone())
}
fn eligible(world: &World, a: &Actor) -> bool {
    let s = world.resource::<SpecialState>();
    let tick = world.resource::<SimTick>().0;
    a.alive()
        && a.foot.is_none()
        && commandable(world, a.id).is_ok()
        && world.get::<Owner>(a.entity).is_some_and(|o| o.0 == a.owner)
        && world
            .get::<Health>(a.entity)
            .is_some_and(|h| h.hit_points > 0)
        && !s.disabled.get(&a.id).is_some_and(|until| *until > tick)
        && !s.interactions.contains_key(&a.id)
        && !s.docked.contains_key(&a.id)
        && world.get::<Mobile>(a.entity).is_some()
}
fn idle(world: &World, a: &Actor) -> bool {
    eligible(world, a)
        && world
            .get::<Orders>(a.entity)
            .is_some_and(|o| o.current.is_none())
        && world
            .get::<Mobile>(a.entity)
            .is_some_and(|m| m.step.is_none())
        && world
            .get::<Weapon>(a.entity)
            .is_none_or(|w| w.order.is_none() && w.target.is_none())
        && !world
            .resource::<crate::TacticalState>()
            .orders
            .contains_key(&a.id)
        && !world
            .resource::<crate::TacticalState>()
            .waypoints
            .contains_key(&a.id)
}
pub(super) fn cancel(world: &mut World, id: SimId) {
    let tick = world.resource::<SimTick>().0;
    let mut s = world.resource_mut::<SpecialState>();
    if s.auto_defuses.remove(&id).is_some() {
        s.auto_defuse_cancelled.insert(id, tick);
    }
}
fn finish(world: &mut World, a: &Actor) {
    clear_orders(world, a.entity);
}
pub(super) fn advance(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>) {
    if defs.rules().is_none() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    world
        .resource_mut::<SpecialState>()
        .auto_defuse_cancelled
        .retain(|id, at| *at == tick && index.get(id).is_some_and(Actor::alive));
    // Actor and charge IDs are ordered. No random draw or visibility write.
    for a in index.values() {
        if !idle(world, a)
            || world
                .resource::<SpecialState>()
                .auto_defuses
                .contains_key(&a.id)
            || world
                .resource::<SpecialState>()
                .auto_defuse_cancelled
                .contains_key(&a.id)
        {
            continue;
        }
        let Some(radius) = defs
            .metadata(a.item)
            .map(|m| m.bomb_sight)
            .filter(|r| *r > 0)
        else {
            continue;
        };
        if kit(defs, a).is_none() {
            continue;
        }
        let from = extent(world, a);
        let picked = world
            .resource::<SpecialState>()
            .charges
            .keys()
            .copied()
            .filter(|id| *id != a.id)
            .filter_map(|id| host(world, defs, index, a, id))
            .filter_map(|b| {
                let (dx, dy) = from.gap(&b.extent);
                in_range(
                    dx,
                    dy,
                    radius.saturating_mul(crate::SUBCELLS_PER_CELL as u32),
                )
                .then_some((
                    i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy),
                    b.id,
                ))
            })
            .min()
            .map(|(_, id)| id);
        if let Some(id) = picked {
            world
                .resource_mut::<SpecialState>()
                .auto_defuses
                .insert(a.id, (id, a.owner));
        }
    }
    let jobs = world.resource::<SpecialState>().auto_defuses.clone();
    for (id, (target, owner)) in jobs {
        let Some(a) = index.get(&id) else {
            world
                .resource_mut::<SpecialState>()
                .auto_defuses
                .remove(&id);
            continue;
        };
        let valid = eligible(world, a) && a.owner == owner;
        let b = valid.then(|| host(world, defs, index, a, target)).flatten();
        let w = kit(defs, a);
        let (Some(b), Some(w)) = (b, w) else {
            finish(world, a);
            continue;
        };
        let (dx, dy) = extent(world, a).gap(&b.extent);
        if in_range(dx, dy, w.range) && !inside_min_range(dx, dy, w.min_range) {
            // Direct DEFUSE only, never generic interact's repair/capture fallback.
            world.resource_mut::<SpecialState>().charges.remove(&target);
            finish(world, a);
        } else if world
            .get::<Orders>(a.entity)
            .is_some_and(|o| o.current.is_none())
        {
            let dest = {
                let g = world.resource::<Grid>();
                let view = g.navigation_for(g.domain_of(id));
                let on_foot = g.is_on_foot(id);
                let reaches = |point: crate::SubPos| {
                    let (dx, dy) = Extent::point(point).gap(&b.extent);
                    in_range(dx, dy, w.range) && !inside_min_range(dx, dy, w.min_range)
                };
                crate::pathfinding::nearest_cell(&view, a.cell, |c| {
                    view.has_room(c, on_foot)
                        && if on_foot {
                            (0..crate::SPOTS_PER_CELL as u8)
                                .all(|spot| reaches(crate::SubPos::spot_of(c, spot)))
                        } else {
                            reaches(crate::SubPos::centre_of(c))
                        }
                })
            };
            if let Some(dest) = dest {
                if let Some(mut o) = world.get_mut::<Orders>(a.entity) {
                    o.current = Some(MoveOrder::new(dest, tick));
                }
            } else {
                finish(world, a);
            }
        }
    }
}
