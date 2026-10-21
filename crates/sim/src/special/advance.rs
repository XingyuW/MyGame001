//! Specialist advance mechanics, moved unchanged from special.rs.
use super::*;

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
    // Only active interactions need a prop membership lookup. Building an ordered set of
    // every scenery ID each tick is equivalent but scales with the whole map.
    let expired_interactions: Vec<_> = world
        .resource::<SpecialState>()
        .interactions
        .iter()
        .filter_map(|(&actor, target)| {
            (!index.contains_key(&actor)
                || (!index.contains_key(target)
                    && world.resource::<crate::Props>().get(*target).is_none()))
            .then_some(actor)
        })
        .collect();
    {
        let mut state = world.resource_mut::<SpecialState>();
        state
            .protected
            .retain(|id, t| index.contains_key(id) && *t > tick);
        state
            .disabled
            .retain(|id, t| index.contains_key(id) && *t > tick);
        state.charge.retain(|id, _| index.contains_key(id));
        let garrisons: BTreeSet<_> = state.garrisons.keys().copied().collect();
        // A carrier can itself be aboard another carrier. Retain descendants of actor and
        // garrison roots, including dead actors until Cleanup handles their payload loss.
        let mut cargo_hosts: BTreeSet<_> = index.keys().copied().collect();
        cargo_hosts.extend(garrisons);
        let roots: Vec<_> = cargo_hosts.iter().copied().collect();
        for root in roots {
            cargo_hosts.extend(state.cargo_descendants(root).iter().map(|p| p.id));
        }
        state.cargo.retain(|id, _| cargo_hosts.contains(id));
        state.rearming.retain(|id, _| index.contains_key(id));
        for actor in expired_interactions {
            state.interactions.remove(&actor);
        }
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
        if a.alive()
            && a.owner != crate::NEUTRAL
            && meta.role.superweapon()
            && charges(world, &defs, a)
            && !world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&a.id)
        {
            let mut s = world.resource_mut::<SpecialState>();
            let c = s.charge.entry(a.id).or_default();
            let ready = *c < meta.cooldown && c.saturating_add(1) >= meta.cooldown;
            *c = c.saturating_add(1).min(meta.cooldown);
            if ready && meta.strategic {
                world
                    .resource_mut::<Messages<SimEvent>>()
                    .write(SimEvent::SuperweaponReady {
                        player: a.owner,
                        role: meta.role,
                    });
            }
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
        if defs.rules().is_none()
            && meta.role == Role::Chrono
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
        let aura =
            (meta.role == Role::Repair && defs.rules().is_none()) || (repair_ifv && !meta.gunner);
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
    auto_defuse::advance(world, &defs, &index);
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
    // Hospitals heal only their patients, using the reference IRepairStep and IRepairRate.
    if let Some(rules) = defs.rules()
        && tick.is_multiple_of(rules.economy.infantry_repair_interval.max(1) as u64)
    {
        for a in index.values().filter(|a| {
            a.alive()
                && a.owner != crate::NEUTRAL
                && defs.metadata(a.item).is_some_and(|m| m.hospital)
        }) {
            if let Some(patients) = world.resource_mut::<SpecialState>().cargo.get_mut(&a.id) {
                for patient in patients {
                    if let Some(terms) = defs.unit_combat(patient.kind) {
                        patient.hp = patient
                            .hp
                            .saturating_add(rules.economy.infantry_repair_step)
                            .min(terms.hit_points);
                    }
                }
            }
            let _ = unload_ready(world, a, &defs, |p| {
                defs.unit_combat(p.kind)
                    .is_some_and(|terms| p.hp >= terms.hit_points)
            });
        }
    }
    crate::garrison::advance(world, &defs, &index);
    repair_docked(world, &defs, &index, tick);
    crate::flight::advance(world, &defs, &index, tick);
    infantry::advance(world, &defs);
    vehicles::advance(world, &defs);
    superweapons::advance(world, &defs);
}
