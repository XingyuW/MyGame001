//! Civilian occupation: commands, approach, cargo preservation and deterministic eviction.
use crate::special::{Actor, Passenger};
use crate::{
    CellPos, Definitions, Grid, PlayerId, Players, ProductionItem, Props, RejectReason, SimId,
    SimTick, SpecialState,
};
use bevy_ecs::world::World;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Garrison {
    pub owner: PlayerId,
    pub cell: CellPos,
    pub cooldown: u32,
    pub evicting: bool,
}

pub(crate) fn item(defs: &Definitions, kind: crate::PropKind) -> Option<ProductionItem> {
    let name = match kind {
        crate::PropKind::House => "civilian_house",
        crate::PropKind::CityBlock => "civilian_city",
        crate::PropKind::Tower => "civilian_tower",
        _ => return None,
    };
    defs.building_kind(name).map(ProductionItem::Building)
}
fn error(message: &'static str) -> RejectReason {
    crate::special::unavailable(message)
}
fn red(defs: &Definitions, prop: &crate::Prop) -> bool {
    let percent = defs.rules().map_or(25, |r| r.combat.condition_red_percent);
    let full = item(defs, prop.kind)
        .and_then(|i| match i {
            ProductionItem::Building(k) => defs.building_combat(k),
            _ => None,
        })
        .map_or(prop.kind.hit_points(), |t| t.hit_points);
    i64::from(prop.hit_points) * 100 <= i64::from(full) * i64::from(percent)
}
pub(crate) fn validate(
    world: &World,
    defs: &Definitions,
    a: &Actor,
    target: SimId,
) -> Result<CellPos, RejectReason> {
    let (cell, prop) = world
        .resource::<Props>()
        .get(target)
        .filter(|(_, p)| p.hit_points > 0)
        .ok_or(RejectReason::UnknownTarget(target))?;
    let state = world.resource::<SpecialState>();
    let garrison = state.garrisons.get(&target);
    if state.hidden.contains(&(a.owner, target)) && !garrison.is_some_and(|g| g.owner == a.owner) {
        return Err(RejectReason::UnknownTarget(target));
    }
    if !world
        .resource::<Players>()
        .get(a.owner)
        .is_some_and(|p| p.vision().is_visible(cell))
        && !garrison.is_some_and(|g| g.owner == a.owner)
    {
        return Err(RejectReason::UnknownTarget(target));
    }
    if !a.alive()
        || !defs
            .metadata(a.item)
            .is_some_and(|m| m.occupier && m.infantry)
        || a.foot.is_some()
    {
        return Err(error("Only Occupier infantry may garrison"));
    }
    let entry = item(defs, prop.kind)
        .and_then(|i| defs.metadata(i))
        .filter(|m| m.can_be_occupied && m.max_occupants > 0)
        .ok_or(error("This scenery cannot be occupied"))?;
    if red(defs, prop) || garrison.is_some_and(|g| g.evicting) {
        return Err(error("A red-health building cannot be occupied"));
    }
    if garrison.is_some_and(|g| g.owner != a.owner) {
        return Err(error("Building is occupied by another player"));
    }
    if state.cargo.get(&target).map_or(0, Vec::len) >= entry.max_occupants as usize {
        return Err(error("Garrison is full"));
    }
    Ok(cell)
}
pub(crate) fn order(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    target: SimId,
) -> Result<(), RejectReason> {
    let cell = validate(world, defs, a, target)?;
    crate::special::clear_orders(world, a.entity);
    if a.cell.x.abs_diff(cell.x).max(a.cell.y.abs_diff(cell.y)) <= 1 {
        enter(world, a, target, cell)?;
    } else {
        world
            .resource_mut::<SpecialState>()
            .interactions
            .insert(a.id, target);
    }
    Ok(())
}
fn enter(world: &mut World, a: &Actor, target: SimId, cell: CellPos) -> Result<(), RejectReason> {
    let ProductionItem::Unit(kind) = a.item else {
        return Err(error("Only infantry may garrison"));
    };
    let passenger = Passenger {
        id: a.id,
        kind,
        hp: a.hp,
        harvester: None,
        experience: world.get::<crate::Experience>(a.entity).copied(),
        bay: None,
    };
    crate::special::release(world, a);
    world.despawn(a.entity);
    let mut state = world.resource_mut::<SpecialState>();
    state.interactions.remove(&a.id);
    state.cargo.entry(target).or_default().push(passenger);
    state.garrisons.entry(target).or_insert(Garrison {
        owner: a.owner,
        cell,
        cooldown: 0,
        evicting: false,
    });
    Ok(())
}
pub(crate) fn advance(world: &mut World, defs: &Definitions, actors: &BTreeMap<SimId, Actor>) {
    let pending = world.resource::<SpecialState>().interactions.clone();
    for (unit, target) in pending {
        if world.resource::<Props>().get(target).is_none() {
            continue;
        }
        let Some(a) = actors.get(&unit) else { continue };
        let Ok(cell) = validate(world, defs, a, target) else {
            world
                .resource_mut::<SpecialState>()
                .interactions
                .remove(&unit);
            continue;
        };
        if a.cell.x.abs_diff(cell.x).max(a.cell.y.abs_diff(cell.y)) <= 1 {
            let _ = enter(world, a, target, cell);
        } else if world
            .get::<crate::Orders>(a.entity)
            .is_some_and(|o| o.current.is_none())
        {
            let dest = {
                let grid = world.resource::<Grid>();
                crate::pathfinding::nearest_cell(grid, a.cell, |c| {
                    grid.has_room(c, true) && c.x.abs_diff(cell.x).max(c.y.abs_diff(cell.y)) == 1
                })
            };
            if let Some(dest) = dest {
                let tick = world.resource::<SimTick>().0;
                if let Some(mut orders) = world.get_mut::<crate::Orders>(a.entity) {
                    orders.current = Some(crate::unit::MoveOrder::new(dest, tick));
                }
            } else {
                world
                    .resource_mut::<SpecialState>()
                    .interactions
                    .remove(&unit);
            }
        }
    }
}
pub(crate) fn deploy(world: &mut World, owner: PlayerId, id: SimId) -> Result<(), RejectReason> {
    let state = world.resource::<SpecialState>();
    let g = state
        .garrisons
        .get(&id)
        .ok_or(RejectReason::UnknownUnit(id))?;
    if g.owner != owner
        || world
            .resource::<Props>()
            .get(id)
            .is_none_or(|(_, p)| p.hit_points <= 0)
    {
        return Err(RejectReason::UnknownUnit(id));
    }
    if let Some(g) = world.resource_mut::<SpecialState>().garrisons.get_mut(&id) {
        g.evicting = true;
    }
    cleanup(world);
    Ok(())
}
pub(crate) fn cleanup(world: &mut World) {
    if world.resource::<SpecialState>().garrisons.is_empty() {
        return;
    }
    let defs = world.resource::<Definitions>().clone();
    let occupied = world.resource::<SpecialState>().garrisons.clone();
    for (id, mut g) in occupied {
        if world
            .resource::<Props>()
            .get(id)
            .is_none_or(|(_, prop)| red(&defs, prop))
        {
            g.evicting = true;
        }
        if !g.evicting {
            continue;
        }
        if let Some(saved) = world.resource_mut::<SpecialState>().garrisons.get_mut(&id) {
            saved.evicting = true;
        }
        // The surrogate describes only the already-authorized host. Unload preserves each
        // passenger's ID, health and veterancy and uses Grid::register/claim/standing_point.
        let kind = world
            .resource::<Props>()
            .get(id)
            .map(|(_, p)| p.kind)
            .unwrap_or(crate::PropKind::House);
        let Some(item) = item(&defs, kind) else {
            continue;
        };
        let actor = Actor {
            entity: bevy_ecs::entity::Entity::PLACEHOLDER,
            id,
            owner: g.owner,
            item,
            cell: g.cell,
            foot: Some(crate::Footprint {
                origin: g.cell,
                width: 1,
                height: 1,
            }),
            hp: 1,
        };
        let _ = crate::special::unload(world, &actor, &defs);
        if world
            .resource::<SpecialState>()
            .cargo
            .get(&id)
            .is_none_or(Vec::is_empty)
        {
            let mut state = world.resource_mut::<SpecialState>();
            state.cargo.remove(&id);
            state.garrisons.remove(&id);
        }
    }
}

/// Occupation weapons use the ordinary impact function and event path. One reload divided by
/// the current occupant count expresses the reference's combined fire rate.
pub(crate) fn fire(world: &mut World) {
    if world.resource::<SpecialState>().garrisons.is_empty() {
        return;
    }
    use bevy_ecs::message::Messages;
    let defs = world.resource::<Definitions>().clone();
    let occupied = world.resource::<SpecialState>().garrisons.clone();
    for (id, g) in occupied {
        if g.evicting
            || world
                .resource::<Props>()
                .get(id)
                .is_none_or(|(_, p)| red(&defs, p))
        {
            continue;
        }
        let crew = world
            .resource::<SpecialState>()
            .cargo
            .get(&id)
            .cloned()
            .unwrap_or_default();
        let Some(first) = crew.first() else { continue };
        if let Some(saved) = world.resource_mut::<SpecialState>().garrisons.get_mut(&id) {
            saved.cooldown = saved.cooldown.saturating_sub(1);
            if saved.cooldown > 0 {
                continue;
            }
        }
        let Some((_, prop)) = world.resource::<Props>().get(id) else {
            continue;
        };
        let Some(ProductionItem::Building(kind)) = item(&defs, prop.kind) else {
            continue;
        };
        let Some(terms) = defs.building_combat(kind) else {
            continue;
        };
        let allied = defs
            .metadata(ProductionItem::Unit(first.kind))
            .is_some_and(|m| m.side == Some(crate::Side::Allied));
        let Some(weapon_kind) = (if allied {
            terms.weapon
        } else {
            terms.secondary
        }) else {
            continue;
        };
        let Some(weapon) = defs.weapon(weapon_kind) else {
            continue;
        };
        let actors = crate::special::actors(world);
        let target = actors
            .values()
            .filter(|a| {
                a.alive()
                    && world.resource::<Players>().hostile(g.owner, a.owner)
                    && world.resource::<Players>().get(g.owner).is_some_and(|p| {
                        a.foot.map_or(p.vision().is_visible(a.cell), |f| {
                            p.vision().sees_footprint(f)
                        })
                    })
                    && !world
                        .resource::<SpecialState>()
                        .hidden
                        .contains(&(g.owner, a.id))
                    && world.resource::<Grid>().domain_of(a.id) != crate::Domain::Air
                    && !crate::world_objects::immune(&defs, Some(a.item))
            })
            .filter_map(|a| {
                let armour = world.get::<crate::Armour>(a.entity)?.0;
                let body = crate::combat::Body::of(
                    world.get::<crate::Mobile>(a.entity),
                    world.get::<crate::Building>(a.entity),
                )?;
                let target_terms = match a.item {
                    ProductionItem::Unit(k) => defs.unit_combat(k),
                    ProductionItem::Building(k) => defs.building_combat(k),
                };
                let traits = crate::combat::traits_of(
                    world.resource::<Grid>(),
                    a.id,
                    armour,
                    &body,
                    target_terms.as_ref(),
                );
                if !crate::weapon_against(&defs, &terms, [Some(weapon_kind), None], &traits)
                    .is_some_and(|(_, versus)| versus > 0)
                {
                    return None;
                }
                let point = a.foot.map_or(a.cell, |f| {
                    CellPos::new(
                        g.cell.x.clamp(
                            f.origin.x,
                            f.origin
                                .x
                                .saturating_add(i32::from(f.width))
                                .saturating_sub(1),
                        ),
                        g.cell.y.clamp(
                            f.origin.y,
                            f.origin
                                .y
                                .saturating_add(i32::from(f.height))
                                .saturating_sub(1),
                        ),
                    )
                });
                let dx = i64::from(point.x) - i64::from(g.cell.x);
                let dy = i64::from(point.y) - i64::from(g.cell.y);
                let distance = (dx * dx + dy * dy) as u64 * 256 * 256;
                (distance <= u64::from(weapon.range).saturating_mul(u64::from(weapon.range)))
                    .then_some((distance, a.id, point))
            })
            .min_by_key(|(distance, id, _)| (*distance, *id));
        // Occupied civilian props have no ECS entity, but are owner-aware combat targets.
        let props = world.resource::<Props>();
        let special = world.resource::<SpecialState>();
        let occupied_targets = special.garrisons.iter().filter_map(|(&other_id, other)| {
            let (cell, prop) = props.get(other_id)?;
            if prop.hit_points <= 0
                || !world.resource::<Players>().hostile(g.owner, other.owner)
                || !world
                    .resource::<Players>()
                    .get(g.owner)
                    .is_some_and(|p| p.vision().is_visible(cell))
                || special.hidden.contains(&(g.owner, other_id))
            {
                return None;
            }
            let traits = crate::combat::traits_of(
                world.resource::<Grid>(),
                other_id,
                props.armour_of(prop.kind),
                &crate::combat::Body::Prop(cell),
                None,
            );
            if !crate::weapon_against(&defs, &terms, [Some(weapon_kind), None], &traits)
                .is_some_and(|(_, versus)| versus > 0)
            {
                return None;
            }
            let dx = i64::from(cell.x) - i64::from(g.cell.x);
            let dy = i64::from(cell.y) - i64::from(g.cell.y);
            let distance = (dx * dx + dy * dy) as u64 * 256 * 256;
            (distance <= u64::from(weapon.range).saturating_mul(u64::from(weapon.range)))
                .then_some((distance, other_id, cell))
        });
        let target = target
            .into_iter()
            .chain(occupied_targets)
            .min_by_key(|(distance, id, _)| (*distance, *id));
        let Some((_, target, cell)) = target else {
            continue;
        };
        world
            .resource_mut::<Messages<crate::SimEvent>>()
            .write(crate::SimEvent::WeaponFired {
                attacker: id,
                target,
                weapon: weapon_kind,
            });
        crate::combat::detonate(
            world,
            crate::combat::Blast {
                point: crate::SubPos::centre_of(cell),
                air: false,
                warhead: weapon.warhead,
                damage: weapon.damage,
                direct: Some(target),
                firepower_pm: 1000,
                source: Some(id),
            },
            false,
        );
        if let Some(saved) = world.resource_mut::<SpecialState>().garrisons.get_mut(&id) {
            saved.cooldown =
                (weapon.reload / u32::try_from(crew.len()).unwrap_or(u32::MAX).max(1)).max(1);
        }
    }
}
