//! Mapped House/CityBlock/Tower specialist actions without fabricated ECS actors.
use super::*;

/// A physical mapped Prop. No placeholder Entity or fabricated Actor is permitted here.
#[derive(Clone, Copy)]
pub(super) struct Target {
    pub id: SimId,
    pub item: ProductionItem,
    pub owner: PlayerId,
    pub cell: CellPos,
    pub hp: i32,
}
pub(super) fn target(world: &World, defs: &Definitions, id: SimId) -> Option<Target> {
    let (cell, prop) = world.resource::<crate::Props>().get(id)?;
    let item = crate::garrison::item(defs, prop.kind)?;
    let owner = world
        .resource::<SpecialState>()
        .garrisons
        .get(&id)
        .map_or(crate::NEUTRAL, |g| g.owner);
    Some(Target {
        id,
        item,
        owner,
        cell,
        hp: prop.hit_points,
    })
}
pub(super) fn known(world: &World, viewer: PlayerId, b: Target) -> bool {
    b.owner == viewer
        || (world
            .resource::<Players>()
            .get(viewer)
            .is_some_and(|p| p.vision().is_visible(b.cell))
            && !world
                .resource::<SpecialState>()
                .hidden
                .contains(&(viewer, b.id)))
}
fn handles(defs: &Definitions, a: &Actor) -> bool {
    defs.rules().is_some()
        && defs
            .metadata(a.item)
            .is_some_and(|m| m.c4 || matches!(m.role, Role::Demolition | Role::Engineer))
}
#[derive(Clone, Copy)]
enum Action {
    C4,
    Ivan,
    Defuse,
}
fn validate(
    world: &World,
    defs: &Definitions,
    a: &Actor,
    id: SimId,
) -> Result<(Target, Action), RejectReason> {
    if !a.alive()
        || world
            .get::<Health>(a.entity)
            .is_none_or(|h| h.hit_points <= 0)
        || world.get::<Owner>(a.entity).is_none_or(|o| o.0 != a.owner)
    {
        return Err(RejectReason::UnknownUnit(a.id));
    }
    let b = target(world, defs, id)
        .filter(|b| b.hp > 0 && known(world, a.owner, *b))
        .ok_or(RejectReason::UnknownTarget(id))?;
    let m = defs
        .metadata(a.item)
        .ok_or(unavailable("No specialist action"))?;
    let n = defs
        .metadata(b.item)
        .ok_or(unavailable("No specialist target"))?;
    if m.role == Role::Engineer {
        if world
            .resource::<SpecialState>()
            .charges
            .get(&id)
            .is_some_and(|c| !c.c4)
        {
            return Ok((b, Action::Defuse));
        }
        return Err(unavailable("There is no bomb to disarm"));
    }
    if m.c4 {
        return if n.can_c4 && !n.immune && a.owner != b.owner {
            Ok((b, Action::C4))
        } else {
            Err(unavailable("Target cannot receive C4"))
        };
    }
    if m.role == Role::Demolition {
        let ivan = match a.item {
            ProductionItem::Unit(k) => defs.unit_combat(k).and_then(|t| t.weapon),
            _ => None,
        }
        .and_then(|w| defs.weapon(w))
        .and_then(|w| defs.warhead(w.warhead))
        .is_some_and(|h| h.ivan_bomb);
        return if ivan && n.bombable && !n.immune {
            Ok((b, Action::Ivan))
        } else {
            Err(unavailable("Target cannot receive a bomb"))
        };
    }
    Err(unavailable("No specialist action"))
}
pub(super) fn label(
    world: &World,
    defs: &Definitions,
    a: &Actor,
    id: SimId,
) -> Option<Result<&'static str, RejectReason>> {
    handles(defs, a).then(|| {
        validate(world, defs, a, id).map(|(_, action)| match action {
            Action::C4 => "C4",
            Action::Ivan => "BOMB",
            Action::Defuse => "DEFUSE",
        })
    })
}
fn apply(world: &mut World, defs: &Definitions, a: &Actor, b: Target, action: Action) {
    match action {
        Action::Defuse => {
            world.resource_mut::<SpecialState>().charges.remove(&b.id);
        }
        Action::C4 => infantry::plant_target(world, a, b.id, defs, true, None),
        Action::Ivan => infantry::plant_target(world, a, b.id, defs, false, None),
    }
    clear_orders(world, a.entity);
    world
        .resource_mut::<SpecialState>()
        .interactions
        .remove(&a.id);
}
pub(super) fn order(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    id: SimId,
) -> Option<Result<(), RejectReason>> {
    if !handles(defs, a) {
        return None;
    }
    Some((|| {
        let (b, action) = validate(world, defs, a, id)?;
        world.resource_mut::<SpecialState>().docked.remove(&a.id);
        clear_orders(world, a.entity);
        if a.cell.x.abs_diff(b.cell.x).max(a.cell.y.abs_diff(b.cell.y)) <= 1 {
            apply(world, defs, a, b, action);
        } else {
            world
                .resource_mut::<SpecialState>()
                .interactions
                .insert(a.id, id);
        }
        Ok(())
    })())
}
/// Called by garrison::advance before its Occupier-only path. True consumes this pending job.
pub(crate) fn advance_interaction(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    id: SimId,
) -> bool {
    if !handles(defs, a) {
        return false;
    }
    let Some((b, action)) = a
        .alive()
        .then(|| validate(world, defs, a, id).ok())
        .flatten()
    else {
        world
            .resource_mut::<SpecialState>()
            .interactions
            .remove(&a.id);
        clear_orders(world, a.entity);
        return true;
    };
    if a.cell.x.abs_diff(b.cell.x).max(a.cell.y.abs_diff(b.cell.y)) <= 1 {
        apply(world, defs, a, b, action);
    } else if world
        .get::<Orders>(a.entity)
        .is_some_and(|o| o.current.is_none())
    {
        let dest = {
            let g = world.resource::<Grid>();
            let view = g.navigation_for(g.domain_of(a.id));
            let on_foot = g.is_on_foot(a.id);
            crate::pathfinding::nearest_cell(&view, a.cell, |c| {
                view.has_room(c, on_foot) && c.x.abs_diff(b.cell.x).max(c.y.abs_diff(b.cell.y)) == 1
            })
        };
        if let Some(dest) = dest {
            let tick = world.resource::<SimTick>().0;
            if let Some(mut orders) = world.get_mut::<Orders>(a.entity) {
                orders.current = Some(MoveOrder::new(dest, tick));
            }
        } else {
            world
                .resource_mut::<SpecialState>()
                .interactions
                .remove(&a.id);
        }
    }
    true
}
pub(super) fn fired(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    id: SimId,
    weapon: crate::WeaponKind,
) -> bool {
    let Some(b) = target(world, defs, id).filter(|b| b.hp > 0 && known(world, a.owner, *b)) else {
        return false;
    };
    let Some(w) = defs.weapon(weapon) else {
        return false;
    };
    let Some(h) = defs.warhead(w.warhead) else {
        return false;
    };
    if h.ivan_bomb {
        infantry::plant_target(world, a, id, defs, false, Some(weapon));
        return true;
    }
    if h.temporal {
        let full = match b.item {
            ProductionItem::Building(k) => defs.building_combat(k).map(|t| t.hit_points),
            _ => None,
        }
        .unwrap_or(b.hp)
        .max(1) as u64;
        world
            .resource_mut::<SpecialState>()
            .erasures
            .entry(id)
            .or_insert(infantry::Erasure {
                remaining: full.saturating_mul(10),
                beams: BTreeMap::new(),
            })
            .beams
            .insert(a.id, w.damage.max(0) as u32);
        return true;
    }
    if h.bomb_disarm {
        world.resource_mut::<SpecialState>().charges.remove(&id);
        return true;
    }
    // FakeC4/Sapper remain ordinary combat impacts; no role-based replacement here.
    false
}
pub(super) fn beam_extent(
    world: &World,
    defs: &Definitions,
    viewer: PlayerId,
    id: SimId,
) -> Option<crate::combat::Extent> {
    target(world, defs, id)
        .filter(|b| b.hp > 0 && known(world, viewer, *b))
        .map(|b| {
            crate::combat::Extent::footprint(Footprint {
                origin: b.cell,
                width: 1,
                height: 1,
            })
        })
}
pub(super) fn erase(world: &mut World, defs: &Definitions, id: SimId) -> bool {
    let Some(b) = target(world, defs, id) else {
        return false;
    };
    world
        .resource_mut::<SpecialState>()
        .temporal_removed
        .insert(id);
    world.resource_mut::<crate::Props>().hurt(b.cell, i64::MAX);
    true
}
pub(super) fn resolve_charge(
    world: &mut World,
    defs: &Definitions,
    id: SimId,
    charge: &infantry::Charge,
    tick: u64,
) -> bool {
    let Some(b) = target(world, defs, id) else {
        return false;
    };
    if b.hp > 0 && charge.due > tick {
        return true;
    }
    if b.hp > 0
        && let Some(effect) = defs.effect(if charge.c4 {
            Effect::C4
        } else {
            Effect::IvanBomb
        })
    {
        let blast = Blast {
            point: crate::SubPos::centre_of(b.cell),
            air: false,
            warhead: effect.warhead,
            damage: charge.damage,
            direct: Some(id),
            firepower_pm: 1000,
            source: Some(charge.source),
            identity: Some(charge.identity),
        };
        if charge.c4 {
            crate::combat::detonate_one(world, blast, id);
        } else {
            detonate(world, blast, true);
        }
    }
    world.resource_mut::<SpecialState>().charges.remove(&id);
    true
}
