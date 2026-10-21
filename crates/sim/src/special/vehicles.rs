//! Reference vehicle specialists. All placements use the shared occupancy allocator.
use super::*;

#[derive(Clone, Debug)]
pub struct Parasite {
    pub passenger: Passenger,
    pub owner: PlayerId,
    pub weapon: crate::WeaponKind,
    pub due: u64,
    pub squid: bool,
    pub escape: Option<CellPos>,
}

pub(super) fn ifv_weapon(world: &mut World, defs: &Definitions, a: &Actor) {
    let Some(meta) = defs.metadata(a.item).filter(|m| m.gunner) else {
        return;
    };
    let mode = world
        .resource::<SpecialState>()
        .cargo
        .get(&a.id)
        .and_then(|ps| ps.first())
        .and_then(|p| defs.metadata(ProductionItem::Unit(p.kind)))
        .map_or(0, |m| m.ifv_mode);
    let weapon = if mode == 0
        && world
            .get::<crate::Experience>(a.entity)
            .is_some_and(|e| e.rank >= 2)
    {
        match a.item {
            ProductionItem::Unit(k) => defs.unit_combat(k).and_then(|t| t.elite[0]),
            _ => None,
        }
    } else {
        meta.ifv_weapons.get(mode as usize).copied()
    };
    if let Some(w) = weapon {
        let old = world.get::<Weapon>(a.entity).map_or(0, |w| w.cooldown);
        let mut next = Weapon::new(w);
        next.cooldown = old;
        world.entity_mut(a.entity).insert(next);
    }
}

pub(super) fn fired(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    b: &Actor,
    weapon: crate::WeaponKind,
) -> bool {
    let Some(w) = defs.weapon(weapon) else {
        return false;
    };
    let Some(h) = defs.warhead(w.warhead) else {
        return false;
    };
    if w.suicide
        && defs
            .metadata(a.item)
            .is_some_and(|m| m.death_weapon.is_some() || m.gunner)
    {
        world
            .resource_mut::<SpecialState>()
            .death_detonated
            .insert(a.id);
        payload(world, defs, a, weapon);
        if let Some(mut hp) = world.get_mut::<Health>(a.entity) {
            hp.hit_points = 0;
        }
        return true;
    }
    if w.sonic {
        free(world, defs, b.id, false);
    }
    if !h.parasite || !matches!(role(defs, a), Role::Drone | Role::Squid) {
        return false;
    }
    let tick = world.resource::<SimTick>().0;
    if world.resource::<SpecialState>().invulnerable(b.id, tick) {
        return true;
    }
    let Some(meta) = defs.metadata(b.item) else {
        return true;
    };
    if meta.infantry && role(defs, a) == Role::Drone {
        if let Some(mut hp) = world.get_mut::<Health>(b.entity) {
            hp.hit_points = 0;
        }
        world.resource_mut::<crate::veterancy::Kills>().record(
            a.id,
            b.owner,
            crate::veterancy::worth(defs, b.item),
        );
        return true;
    }
    let squid = h.paralyzes > 0;
    if b.foot.is_some()
        || meta.infantry
        || meta.domain == Domain::Air
        || (squid
            && !world
                .resource::<Grid>()
                .cell(b.cell)
                .is_some_and(|c| c.water))
        || world
            .resource::<SpecialState>()
            .parasites
            .contains_key(&b.id)
    {
        return true;
    }
    let ProductionItem::Unit(kind) = a.item else {
        return true;
    };
    let passenger = Passenger {
        id: a.id,
        kind,
        hp: a.hp,
        harvester: None,
        experience: world.get::<crate::Experience>(a.entity).copied(),
        bay: None,
    };
    release(world, a);
    let reload = crate::veterancy::reload_ticks(
        w.reload,
        crate::combat::ai_reload(
            world.resource::<Players>(),
            a.owner,
            passenger
                .experience
                .as_ref()
                .map_or(1000, crate::Experience::reload_pm),
        ),
    );
    world.despawn(a.entity);
    world.resource_mut::<SpecialState>().parasites.insert(
        b.id,
        Parasite {
            passenger,
            owner: a.owner,
            weapon,
            due: tick.saturating_add(u64::from(reload)),
            squid,
            escape: None,
        },
    );
    if squid {
        clear_movement(world, b);
    }
    true
}
fn clear_movement(world: &mut World, a: &Actor) {
    if let Some(mut orders) = world.get_mut::<Orders>(a.entity) {
        orders.current = None;
    }
    // Finish at the currently held cell rather than leave a reserved destination behind.
    let step = world.get::<Mobile>(a.entity).and_then(|m| m.step);
    if let Some(step) = step {
        world.resource_mut::<Grid>().release(step.to, a.id);
    }
    let point = world.resource::<Grid>().standing_point(a.cell, a.id);
    if let Some(mut m) = world.get_mut::<Mobile>(a.entity) {
        m.step = None;
        m.speed = 0;
    }
    if let Some(mut p) = world.get_mut::<Position>(a.entity) {
        p.0 = point;
    }
}
/// A repair removes a drone; a sonic hit releases a squid. A successful parasite kill also
/// releases it. If no legal ring spot exists it stays pending until the next economy stage.
pub(super) fn free(world: &mut World, defs: &Definitions, host: SimId, repair: bool) {
    let Some(p) = world
        .resource::<SpecialState>()
        .parasites
        .get(&host)
        .cloned()
    else {
        return;
    };
    if repair && !p.squid {
        world.resource_mut::<SpecialState>().parasites.remove(&host);
        return;
    }
    if repair || !p.squid {
        return;
    }
    if let Some(a) = actors(world).get(&host).filter(|a| a.alive()).cloned() {
        if let Some(p) = world
            .resource_mut::<SpecialState>()
            .parasites
            .get_mut(&host)
        {
            p.escape = Some(a.cell);
        }
        release_parasite(world, defs, host, a.cell, p);
    }
}
fn release_parasite(
    world: &mut World,
    defs: &Definitions,
    host: SimId,
    cell: CellPos,
    p: Parasite,
) {
    let foot = Footprint {
        origin: cell,
        width: 1,
        height: 1,
    };
    let domain = defs.domain(p.passenger.kind);
    let dest = foot.ring().chain(std::iter::once(cell)).find(|&c| {
        let g = world.resource::<Grid>();
        g.passable_for(c, domain) && g.navigation_for(domain).has_room(c, false)
    });
    let Some(cell) = dest else {
        return;
    };
    let Some(u) = defs.unit(p.passenger.kind) else {
        return;
    };
    let point = {
        let mut g = world.resource_mut::<Grid>();
        g.register(p.passenger.id, domain, false);
        if !g.claim(cell, p.passenger.id) {
            g.unregister(p.passenger.id);
            return;
        }
        g.standing_point(cell, p.passenger.id)
    };
    let mut e = world.spawn(unit_bundle(
        p.passenger.id,
        p.owner,
        p.passenger.kind,
        u.speed,
        cell,
        point,
    ));
    if let Some(t) = defs.unit_combat(p.passenger.kind) {
        let (c, w) = combat_components(t);
        e.insert(c);
        e.insert(Health {
            hit_points: p.passenger.hp,
        });
        if let Some(w) = w {
            e.insert(w);
        }
    }
    if let Some(xp) = p.passenger.experience {
        e.insert(xp);
        let entity = e.id();
        crate::veterancy::rearm(world, entity, p.passenger.kind);
    }
    world.resource_mut::<SpecialState>().parasites.remove(&host);
}
pub(super) fn advance(world: &mut World, defs: &Definitions) {
    if defs.rules().is_none() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let index = actors(world);
    let parasites = world.resource::<SpecialState>().parasites.clone();
    for (host, p) in parasites {
        if let Some(cell) = p.escape {
            release_parasite(world, defs, host, cell, p);
            continue;
        }
        let Some(a) = index.get(&host).filter(|a| a.alive()) else {
            world.resource_mut::<SpecialState>().parasites.remove(&host);
            continue;
        };
        if p.squid {
            clear_movement(world, a);
        }
        if p.due > tick {
            continue;
        }
        let Some(w) = defs.weapon(p.weapon) else {
            continue;
        };
        let elite = p.passenger.experience.is_some_and(|e| e.rank >= 2);
        if world.resource::<SpecialState>().invulnerable(host, tick) {
            continue;
        }
        crate::combat::detonate_one(
            world,
            Blast {
                point: position_of(world, a),
                air: false,
                warhead: w.warhead,
                damage: w.damage,
                direct: Some(host),
                firepower_pm: 1000,
                source: Some(p.passenger.id),
                identity: Some(crate::combat::DamageIdentity {
                    owner: p.owner,
                    item: ProductionItem::Unit(p.passenger.kind),
                }),
            },
            host,
        );
        let hp = world.get::<Health>(a.entity).map_or(0, |h| h.hit_points);
        let cull = hp > 0
            && defs.warhead(w.warhead).is_some_and(|h| h.culling)
            && defs.rules().is_some_and(|r| {
                let max = match a.item {
                    ProductionItem::Unit(k) => defs.unit_combat(k).map_or(a.hp, |t| t.hit_points),
                    _ => a.hp,
                };
                i64::from(hp) * 100
                    <= i64::from(max)
                        * i64::from(if elite {
                            r.combat.condition_yellow_percent
                        } else {
                            r.combat.condition_red_percent
                        })
            });
        if cull {
            if let Some(mut hp) = world.get_mut::<Health>(a.entity) {
                hp.hit_points = 0;
            }
            world.resource_mut::<crate::veterancy::Kills>().record(
                p.passenger.id,
                a.owner,
                crate::veterancy::worth(defs, a.item),
            );
        }
        let reload = crate::veterancy::reload_ticks(
            w.reload,
            crate::combat::ai_reload(
                world.resource::<Players>(),
                p.owner,
                p.passenger
                    .experience
                    .as_ref()
                    .map_or(1000, crate::Experience::reload_pm),
            ),
        );
        if world
            .get::<Health>(a.entity)
            .is_some_and(|h| h.hit_points <= 0)
        {
            if let Some(p) = world
                .resource_mut::<SpecialState>()
                .parasites
                .get_mut(&host)
            {
                p.escape = Some(a.cell);
            }
            release_parasite(world, defs, host, a.cell, p);
        } else if let Some(p) = world
            .resource_mut::<SpecialState>()
            .parasites
            .get_mut(&host)
        {
            p.due = tick.saturating_add(u64::from(reload));
        }
    }
    // IFV repair is a weapon-rate action; an Engineer passenger has the negative RepairBullet.
    for a in index.values().filter(|a| a.alive()) {
        if world
            .resource::<SpecialState>()
            .erasures
            .contains_key(&a.id)
            || world
                .resource::<SpecialState>()
                .disabled
                .get(&a.id)
                .is_some_and(|until| *until > tick)
        {
            continue;
        }
        let Some(w) = world
            .get::<Weapon>(a.entity)
            .and_then(|w| defs.weapon(w.kind))
            .filter(|w| w.repair)
        else {
            continue;
        };
        if world
            .get::<Weapon>(a.entity)
            .is_some_and(|w| w.cooldown > 0)
        {
            continue;
        }
        let b = index
            .values()
            .filter(|b| {
                b.alive()
                    && b.owner == a.owner
                    && b.id != a.id
                    && !world
                        .resource::<SpecialState>()
                        .erasures
                        .contains_key(&b.id)
                    && b.foot.is_none()
                    && defs.category(b.item) == Category::Vehicles
                    && defs
                        .metadata(b.item)
                        .is_some_and(|m| m.domain != Domain::Air)
            })
            .filter(|b| {
                let x = position_of(world, a);
                let y = position_of(world, b);
                let dx = i64::from(x.x) - i64::from(y.x);
                let dy = i64::from(x.y) - i64::from(y.y);
                dx * dx + dy * dy <= i64::from(w.range) * i64::from(w.range)
            })
            .find(|b| match b.item {
                ProductionItem::Unit(k) => {
                    defs.unit_combat(k).is_some_and(|t| b.hp < t.hit_points)
                        || world
                            .resource::<SpecialState>()
                            .parasites
                            .contains_key(&b.id)
                }
                _ => false,
            });
        if let Some(b) = b {
            let max = match b.item {
                ProductionItem::Unit(k) => defs.unit_combat(k).map_or(b.hp, |t| t.hit_points),
                _ => b.hp,
            };
            if let Some(mut hp) = world.get_mut::<Health>(b.entity) {
                hp.hit_points = hp.hit_points.saturating_sub(w.damage).min(max);
            }
            free(world, defs, b.id, true);
            let reload = weapon_reload(world, a, w.reload);
            if let Some(mut gun) = world.get_mut::<Weapon>(a.entity) {
                gun.cooldown = reload;
            }
        }
    }
}

pub(super) fn death_weapon(
    world: &World,
    defs: &Definitions,
    a: &Actor,
) -> Option<crate::WeaponKind> {
    let m = defs.metadata(a.item)?;
    if m.gunner {
        let passenger = world.resource::<SpecialState>().cargo.get(&a.id)?.first()?;
        return defs
            .metadata(ProductionItem::Unit(passenger.kind))
            .filter(|m| m.ifv_mode == 7)
            .and_then(|_| m.ifv_weapons.get(7).copied());
    }
    // Kirov is the reference balloon aircraft; its primary is its crash payload.
    if m.explodes {
        return m.death_weapon.or_else(|| match a.item {
            ProductionItem::Unit(k) => defs.unit_combat(k).and_then(|t| t.weapon),
            ProductionItem::Building(k) => defs.building_combat(k).and_then(|t| t.weapon),
        });
    }
    m.death_weapon
}
pub(super) fn payload(world: &mut World, defs: &Definitions, a: &Actor, kind: crate::WeaponKind) {
    let Some(w) = defs.weapon(kind) else {
        return;
    };
    // DeathWeaponDamageModifier belongs to the destroyed type, not to NukePayload itself.
    // Preserve the shared strategic missile's Damage600 and all radius/warhead/radiation data.
    let damage_pm = defs
        .metadata(a.item)
        .map_or(1000, |m| m.death_weapon_damage_pm);
    if damage_pm > 0 {
        detonate(
            world,
            Blast {
                point: position_of(world, a),
                air: false,
                warhead: w.warhead,
                damage: w.damage,
                direct: None,
                firepower_pm: damage_pm,
                source: Some(a.id),
                identity: Some(crate::combat::DamageIdentity {
                    owner: a.owner,
                    item: a.item,
                }),
            },
            true,
        );
    }
    if w.rad_level > 0 {
        radiation::add(
            world,
            a.cell,
            defs.warhead(w.warhead).map_or(0, |h| h.cell_spread),
            w.rad_level,
            Some(a.id),
        );
    }
}
/// Run again after bridge/garrison cleanup: explosions can collapse more bridges and kill
/// more payload carriers. Each id detonates once, so the fixed point is bounded by actors.
pub(crate) fn cleanup(world: &mut World) {
    let defs = world.resource::<Definitions>().clone();
    if defs.rules().is_none() {
        return;
    }
    loop {
        let count = world.resource::<SpecialState>().death_detonated.len();
        crate::bridges::collapse(world);
        crate::garrison::cleanup(world);
        infantry::finish_deaths(world, &defs);
        if world.resource::<SpecialState>().death_detonated.len() == count {
            break;
        }
    }
    let dead: Vec<_> = actors(world)
        .into_values()
        .filter(|a| !a.alive())
        .map(|a| a.id)
        .collect();
    let mut s = world.resource_mut::<SpecialState>();
    for id in dead {
        let descendants: Vec<_> = s.cargo_descendants(id).iter().map(|p| p.id).collect();
        for passenger in descendants {
            s.cargo.remove(&passenger);
        }
        s.cargo.remove(&id);
        if s.parasites.get(&id).is_some_and(|p| p.escape.is_none()) {
            s.parasites.remove(&id);
        }
    }
}
