//! Specialist transport mechanics, moved unchanged from special.rs.
use super::*;

pub(crate) fn unload(world: &mut World, a: &Actor, defs: &Definitions) -> Result<(), RejectReason> {
    unload_ready(world, a, defs, |_| true)
}
pub(super) fn unload_ready(
    world: &mut World,
    a: &Actor,
    defs: &Definitions,
    ready: impl Fn(&Passenger) -> bool,
) -> Result<(), RejectReason> {
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
        if !ready(&p) {
            left.push(p);
            continue;
        }
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
    vehicles::ifv_weapon(world, defs, a);
    Ok(())
}
