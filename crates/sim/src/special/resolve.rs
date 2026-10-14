//! Specialist resolve mechanics, moved unchanged from special.rs.
use super::*;

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
        {
            // Every powered purifier of the player adds the rules' share of the load's value
            // (RA5, section 4.10); a quarter without a rules file, the rule of M9.
            let purifiers = index
                .values()
                .filter(|b| {
                    b.owner == player
                        && role(&defs, b) == Role::Purifier
                        && powered(world, &defs, b)
                })
                .count();
            let percent = defs.rules().map_or(25, |rules| {
                i64::from(rules.economy.purifier_bonus_percent.max(0))
            });
            let bonus = credits
                .max(0)
                .saturating_mul(percent)
                .saturating_mul(i64::try_from(purifiers).unwrap_or(0))
                / 100;
            if bonus > 0
                && let Some(p) = world.resource_mut::<Players>().get_mut(player)
            {
                p.credits = p.credits.saturating_add(bonus);
            }
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
