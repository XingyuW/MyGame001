//! Specialist interaction mechanics, moved unchanged from special.rs.
use super::*;

pub(super) fn validate_interaction(
    world: &World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
) -> Result<(), RejectReason> {
    if a.id == b.id {
        return Err(unavailable("Choose another target"));
    }
    match role(defs, a) {
        Role::Engineer if b.foot.is_some() => {
            if defs.metadata(b.item).is_some_and(|m| m.bridge_repair_hut) {
                return crate::bridges::validate_repair(world, b.id);
            }
            if defs
                .metadata(b.item)
                .is_some_and(|m| m.immune || (m.needs_engineer && !m.capturable))
            {
                return Err(unavailable("Target cannot be captured"));
            }
            Ok(())
        }
        Role::Spy if b.foot.is_some() && b.owner != a.owner => Ok(()),
        _ if defs.metadata(b.item).is_some_and(|m| m.hospital) => {
            if a.owner != b.owner
                || !defs
                    .metadata(a.item)
                    .is_some_and(|m| m.infantry && m.domain == Domain::Land)
            {
                return Err(unavailable("Choose an owned hospital for infantry"));
            }
            Ok(())
        }
        _ if repairs_units(defs, b) => {
            validate_docking(a, b, defs)?;
            let capacity = defs.metadata(b.item).map_or(0, |m| m.vehicle_docks);
            if capacity > 0
                && world
                    .resource::<SpecialState>()
                    .docked
                    .iter()
                    .filter(|(unit, host)| **host == b.id && **unit != a.id)
                    .count()
                    >= capacity as usize
            {
                return Err(unavailable("Repair dock is occupied"));
            }
            Ok(())
        }
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
pub(super) fn interact(
    world: &mut World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
) -> Result<(), RejectReason> {
    validate_interaction(world, a, b, defs)?;
    if role(defs, a) == Role::Engineer && defs.metadata(b.item).is_some_and(|m| m.bridge_repair_hut)
    {
        return crate::bridges::repair(world, a, b.id);
    }
    match role(defs, a) {
        Role::Engineer if b.foot.is_some() => {
            if b.owner != a.owner && defs.metadata(b.item).is_some_and(|m| m.hospital) {
                world.resource_mut::<SpecialState>().cargo.remove(&b.id);
            }
            crate::world_objects::captured(world, defs, b.id, b.item, b.owner, a.owner);
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
                    // The rules' share of the victim's credits (RA5); half without a rules
                    // file, the rule of M9.
                    let percent = defs.rules().map_or(50, |rules| {
                        i64::from(rules.combat.spy_money_steal_percent.clamp(0, 100))
                    });
                    let amount = world
                        .resource::<Players>()
                        .get(b.owner)
                        .map_or(0, |p| p.credits().max(0).saturating_mul(percent) / 100);
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
