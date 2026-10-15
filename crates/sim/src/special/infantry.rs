//! Reference-driven infantry abilities. Legacy worlds keep the M9 paths in the sibling modules.
use super::*;
use crate::Side;

#[derive(Clone, Debug)]
pub struct Charge {
    pub due: u64,
    pub damage: i32,
    pub source: SimId,
    pub c4: bool,
}
#[derive(Clone, Debug)]
pub struct Erasure {
    pub remaining: u64,
    pub beams: BTreeMap<SimId, u32>,
}

pub(crate) fn hash(s: &SpecialState, h: &mut Fnv1a64) {
    if s.deployed.is_empty()
        && s.disguises.is_empty()
        && s.erasures.is_empty()
        && s.charges.is_empty()
        && s.radiation_sites.is_empty()
        && s.coil_charge.is_empty()
        && s.temporal_removed.is_empty()
        && s.death_detonated.is_empty()
        && s.eruption_next.is_empty()
    {
        return;
    }
    h.write(b"infantry6");
    h.write_u64(s.death_detonated.len() as u64);
    for id in &s.death_detonated {
        h.write_u32(id.0);
    }
    h.write_u64(s.eruption_next.len() as u64);
    for (id, next) in &s.eruption_next {
        h.write_u32(id.0);
        h.write_u64(*next);
    }
    h.write_u64(s.deployed.len() as u64);
    for (id, until) in &s.deployed {
        h.write_u32(id.0);
        h.write_u64(*until);
    }
    h.write_u64(s.disguises.len() as u64);
    for (id, kind) in &s.disguises {
        h.write_u32(id.0);
        h.write(&kind.0.to_le_bytes());
    }
    h.write_u64(s.erasures.len() as u64);
    for (id, e) in &s.erasures {
        h.write_u32(id.0);
        h.write_u64(e.remaining);
        h.write_u64(e.beams.len() as u64);
        for (source, damage) in &e.beams {
            h.write_u32(source.0);
            h.write_u32(*damage);
        }
    }
    h.write_u64(s.charges.len() as u64);
    for (id, c) in &s.charges {
        h.write_u32(id.0);
        h.write_u64(c.due);
        h.write(&c.damage.to_le_bytes());
        h.write_u32(c.source.0);
        h.write(&[u8::from(c.c4)]);
    }
    h.write_u64(s.radiation_sites.len() as u64);
    for site in &s.radiation_sites {
        h.write(&site.cell.x.to_le_bytes());
        h.write(&site.cell.y.to_le_bytes());
        h.write_u32(site.spread);
        h.write_u32(site.level);
        h.write_u64(site.born);
        h.write_u64(site.duration);
        h.write(&[u8::from(site.source.is_some())]);
        h.write_u32(site.source.map_or(0, |s| s.0));
    }
    h.write_u64(s.coil_charge.len() as u64);
    for (id, c) in &s.coil_charge {
        h.write_u32(id.0);
        h.write_u32(*c);
    }
    h.write_u64(s.temporal_removed.len() as u64);
    for id in &s.temporal_removed {
        h.write_u32(id.0);
    }
}

pub(crate) fn refresh_limits(world: &mut World) {
    if !world
        .resource::<Definitions>()
        .arsenal()
        .0
        .iter()
        .any(|(item, m)| matches!(item, ProductionItem::Unit(_)) && m.build_limit > 0)
    {
        return;
    }
    let index = living(world);
    let mut counts = BTreeMap::<PlayerId, BTreeMap<UnitKind, u32>>::new();
    let mut add = |owner, kind| {
        if world
            .resource::<Definitions>()
            .metadata(ProductionItem::Unit(kind))
            .is_some_and(|m| m.build_limit > 0)
        {
            *counts.entry(owner).or_default().entry(kind).or_default() += 1;
        }
    };
    for a in index.values() {
        if let ProductionItem::Unit(k) = a.item {
            add(a.owner, k);
        }
        if let Some(cargo) = world.resource::<SpecialState>().cargo.get(&a.id) {
            for p in cargo {
                add(a.owner, p.kind);
            }
        }
        if let Some(b) = world.get::<Building>(a.entity) {
            for &k in &b.pending_units {
                add(a.owner, k);
            }
        }
    }
    for (id, g) in &world.resource::<SpecialState>().garrisons {
        if let Some(cargo) = world.resource::<SpecialState>().cargo.get(id) {
            for p in cargo {
                add(g.owner, p.kind);
            }
        }
    }
    let players: Vec<_> = world.resource::<Players>().iter().map(|(p, _)| p).collect();
    for p in players {
        if let Some(s) = world.resource_mut::<Players>().get_mut(p) {
            s.limited_units = counts.remove(&p).unwrap_or_default();
        }
    }
}

pub(crate) fn deploy(world: &mut World, a: &Actor, defs: &Definitions) -> Result<(), RejectReason> {
    let m = defs
        .metadata(a.item)
        .ok_or(unavailable("No deploy action"))?;
    if !m.deployer {
        return Err(unavailable("No deploy action"));
    }
    if world
        .resource_mut::<SpecialState>()
        .deployed
        .remove(&a.id)
        .is_some()
    {
        return Ok(());
    }
    if let ProductionItem::Unit(k) = a.item
        && defs
            .unit_combat(k)
            .and_then(|t| t.secondary)
            .and_then(|w| defs.weapon(w))
            .is_some_and(|w| w.area_fire && w.rad_level == 0)
        && world
            .get::<Weapon>(a.entity)
            .is_some_and(|w| w.cooldown > 0)
    {
        return Err(unavailable("Deploy weapon is recharging"));
    }
    let tick = world.resource::<SimTick>().0;
    clear_orders(world, a.entity);
    // Finish a half-step by returning to the held cell, preserving its registered spot.
    teleport(world, a, a.cell);
    world.resource_mut::<SpecialState>().deployed.insert(
        a.id,
        if m.undeploy_delay == 0 {
            0
        } else {
            tick.saturating_add(u64::from(m.undeploy_delay))
        },
    );
    if let Some(mut mobile) = world.get_mut::<Mobile>(a.entity) {
        mobile.speed = 0;
    }
    if let ProductionItem::Unit(k) = a.item {
        let elite = world
            .get::<crate::Experience>(a.entity)
            .is_some_and(|e| e.rank >= crate::ELITE);
        if let Some(terms) = defs.unit_combat(k) {
            let weapon = if elite {
                terms.elite[1].or(terms.secondary)
            } else {
                terms.secondary
            };
            if let Some(w) = weapon.and_then(|w| defs.weapon(w))
                && w.area_fire
                && w.rad_level == 0
            {
                detonate(
                    world,
                    Blast {
                        point: position_of(world, a),
                        air: false,
                        warhead: w.warhead,
                        damage: w.damage,
                        direct: None,
                        firepower_pm: 1000,
                        source: Some(a.id),
                    },
                    true,
                );
                if let Some(mut weapon) = world.get_mut::<Weapon>(a.entity) {
                    weapon.cooldown = w.reload;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn special_interaction(
    world: &World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
) -> Option<Result<(), RejectReason>> {
    defs.rules()?;
    let m = defs.metadata(a.item)?;
    let target = defs.metadata(b.item)?;
    let primary = match a.item {
        ProductionItem::Unit(k) => defs.unit_combat(k).and_then(|t| t.weapon),
        ProductionItem::Building(_) => None,
    }
    .and_then(|w| defs.weapon(w))
    .and_then(|w| defs.warhead(w.warhead));
    // A role alone also exists in legacy fixtures; only reference traits opt in.
    if m.role == Role::Spy && !primary.is_some_and(|h| h.makes_disguise) {
        return None;
    }
    if m.role == Role::Engineer
        && world
            .resource::<SpecialState>()
            .charges
            .get(&b.id)
            .is_some_and(|c| !c.c4)
    {
        return Some(Ok(()));
    }
    if a.owner == b.owner && target.hospital {
        return None;
    }
    // Friendly transport entry takes precedence over copying or planting a charge.
    if a.owner == b.owner
        && target.capacity > 0
        && matches!(target.role, Role::Transport | Role::Ifv)
    {
        return None;
    }
    if m.role == Role::Spy && b.foot.is_none() {
        return Some(if target.infantry {
            Ok(())
        } else {
            Err(unavailable("Disguise requires infantry"))
        });
    }
    if m.c4 && b.foot.is_some() {
        return Some(
            if target.can_c4
                && (a.owner != b.owner || target.bridge_repair_hut)
                && (!target.immune || target.bridge_repair_hut)
            {
                if target.bridge_repair_hut
                    && !world
                        .resource::<crate::MatchState>()
                        .rules()
                        .bridge_destruction
                {
                    Err(RejectReason::DisabledByOptions)
                } else {
                    Ok(())
                }
            } else {
                Err(unavailable("Target cannot receive C4"))
            },
        );
    }
    if m.role == Role::Demolition && primary.is_some_and(|h| h.ivan_bomb) {
        return Some(if target.bombable && !target.immune {
            Ok(())
        } else {
            Err(unavailable("Target cannot receive a bomb"))
        });
    }
    if m.role == Role::Tesla && a.foot.is_none() && target.overpowerable {
        return Some(if a.owner == b.owner {
            Ok(())
        } else {
            Err(unavailable("Charge your own Tesla Coil"))
        });
    }
    if m.role == Role::Spy && b.foot.is_some() {
        return Some(if target.spyable && a.owner != b.owner {
            Ok(())
        } else {
            Err(unavailable("Target cannot be infiltrated"))
        });
    }
    None
}

pub(crate) fn interact(world: &mut World, a: &Actor, b: &Actor, defs: &Definitions) -> bool {
    let Some(Ok(())) = special_interaction(world, a, b, defs) else {
        return false;
    };
    let Some(m) = defs.metadata(a.item) else {
        return false;
    };
    if m.role == Role::Engineer {
        world.resource_mut::<SpecialState>().charges.remove(&b.id);
    } else if m.role == Role::Spy && b.foot.is_none() {
        if let ProductionItem::Unit(k) = b.item {
            world
                .resource_mut::<SpecialState>()
                .disguises
                .insert(a.id, k);
        }
    } else if m.role == Role::Spy {
        infiltrate(world, a, b, defs);
    } else if m.c4 {
        plant(world, a, b, defs, true, None);
    } else if m.role == Role::Demolition {
        plant(world, a, b, defs, false, None);
    } else {
        clear_orders(world, a.entity);
    }
    true
}
fn infiltrate(world: &mut World, a: &Actor, b: &Actor, defs: &Definitions) {
    let (ProductionItem::Building(k), Some(r)) = (b.item, defs.rules()) else {
        return;
    };
    let (Some(d), Some(m)) = (defs.building(k), defs.metadata(b.item)) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    // Exactly one effect, in explicit precedence order. Satellite blocks radar reset.
    if d.credits_per_ore.is_some() {
        let amount = world.resource::<Players>().get(b.owner).map_or(0, |p| {
            p.credits()
                .saturating_mul(i64::from(r.combat.spy_money_steal_percent))
                / 100
        });
        if let Some(p) = world.resource_mut::<Players>().get_mut(b.owner) {
            p.credits -= amount;
        }
        if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
            p.credits = p.credits.saturating_add(amount);
        }
    } else if d.power_produced > 0 {
        world.resource_mut::<SpecialState>().outages.insert(
            b.owner,
            tick.saturating_add(r.combat.spy_power_blackout as u64),
        );
    } else if m.role.superweapon() {
        world.resource_mut::<SpecialState>().charge.insert(b.id, 0);
    } else if m.factory == Some(FactoryGroup::Infantry) {
        if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
            p.infiltration |= 1;
        }
    } else if m.factory == Some(FactoryGroup::Vehicles) {
        if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
            p.infiltration |= 2;
        }
    } else if m.radar {
        let satellite = living(world).values().any(|v| {
            v.owner == b.owner && role(defs, v) == Role::Satellite && powered(world, defs, v)
        });
        if !satellite && let Some(p) = world.resource_mut::<Players>().get_mut(b.owner) {
            p.vision.reset_shroud();
        }
    } else if r
        .prerequisite_groups
        .iter()
        .any(|g| g.id == "tech" && g.buildings.contains(&k))
        && let Some(p) = world.resource_mut::<Players>().get_mut(a.owner)
    {
        p.infiltration |= if m.side == Some(Side::Allied) { 4 } else { 8 };
    }
    release(world, a);
    world.despawn(a.entity);
}
fn plant(
    world: &mut World,
    a: &Actor,
    b: &Actor,
    defs: &Definitions,
    c4: bool,
    weapon: Option<crate::WeaponKind>,
) {
    let Some(r) = defs.rules() else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let weapon = weapon.or_else(|| {
        let ProductionItem::Unit(k) = a.item else {
            return None;
        };
        let t = defs.unit_combat(k)?;
        if world
            .get::<crate::Experience>(a.entity)
            .is_some_and(|e| e.rank >= crate::ELITE)
        {
            t.elite[0].or(t.weapon)
        } else {
            t.weapon
        }
    });
    let damage = if c4 {
        r.combat.max_damage
    } else {
        weapon
            .and_then(|w| defs.weapon(w))
            .map_or(r.combat.ivan_damage, |w| w.damage)
    };
    world
        .resource_mut::<SpecialState>()
        .charges
        .entry(b.id)
        .or_insert(Charge {
            due: tick.saturating_add(if c4 {
                r.combat.c4_delay
            } else {
                r.combat.ivan_delay
            } as u64),
            damage,
            source: a.id,
            c4,
        });
    clear_orders(world, a.entity);
}

pub(crate) fn advance(world: &mut World, defs: &Definitions) {
    if defs.rules().is_none() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let index = living(world);
    {
        let mut s = world.resource_mut::<SpecialState>();
        s.deployed
            .retain(|id, t| index.contains_key(id) && (*t == 0 || *t > tick));
        s.disguises.retain(|id, _| index.contains_key(id));
        s.coil_charge.clear();
        s.temporal_removed.clear();
        s.death_detonated.clear();
        s.eruption_next.retain(|id, _| index.contains_key(id));
    }
    // Release control immediately when its controller dies; ownership is restored before combat.
    let controls = world.resource::<SpecialState>().controlled.clone();
    for (id, (caster, owner)) in controls {
        if !index.contains_key(&caster) || !index.contains_key(&id) {
            if let Some(a) = index.get(&id) {
                world.entity_mut(a.entity).insert(Owner(owner));
                clear_orders(world, a.entity);
            }
            world.resource_mut::<SpecialState>().controlled.remove(&id);
        }
    }
    retain_beams(world, defs, &index);
    for a in index.values() {
        let Some(m) = defs.metadata(a.item) else {
            continue;
        };
        if m.role == Role::Spy
            && !world
                .resource::<SpecialState>()
                .disguises
                .contains_key(&a.id)
        {
            let soviet = world
                .resource::<Players>()
                .get(a.owner)
                .and_then(|p| p.country())
                .and_then(|c| defs.rules()?.country(c))
                .is_some_and(|c| c.side == Side::Soviet);
            if let Some(k) = defs.unit_kind(if soviet { "conscript" } else { "gi" }) {
                world
                    .resource_mut::<SpecialState>()
                    .disguises
                    .insert(a.id, k);
            }
        }
        let frozen = world
            .resource::<SpecialState>()
            .erasures
            .contains_key(&a.id);
        let deployed = world
            .resource::<SpecialState>()
            .deployed
            .contains_key(&a.id);
        if (frozen || deployed)
            && let Some(mut mobile) = world.get_mut::<Mobile>(a.entity)
        {
            mobile.speed = 0;
        }
        if m.teleporter
            && m.role != Role::ChronoMiner
            && !frozen
            && !world
                .resource::<SpecialState>()
                .disabled
                .contains_key(&a.id)
        {
            let dest = world
                .get::<Orders>(a.entity)
                .and_then(|o| o.current.as_ref().map(|o| o.destination));
            if let Some(dest) = dest {
                let p = position_of(world, a);
                let q = crate::SubPos::centre_of(dest);
                let dx = i64::from(p.x) - i64::from(q.x);
                let dy = i64::from(p.y) - i64::from(q.y);
                let distance = ((dx * dx + dy * dy) as u64).isqrt();
                if teleport(world, a, dest)
                    && let Some(r) = defs.rules()
                {
                    let converted =
                        ((distance / r.combat.chrono_distance_factor as u64) * 4 + 1) / 3;
                    let delay = converted.max(r.combat.chrono_minimum_delay as u64);
                    world
                        .resource_mut::<SpecialState>()
                        .disabled
                        .insert(a.id, tick.saturating_add(delay));
                }
            }
        }
        if m.role == Role::Tesla
            && a.foot.is_none()
            && !frozen
            && world
                .get::<Orders>(a.entity)
                .is_none_or(|o| o.current.is_none())
            && world
                .get::<Weapon>(a.entity)
                .is_none_or(|w| w.target.is_none())
        {
            let range = if let ProductionItem::Unit(k) = a.item {
                defs.unit_combat(k)
                    .and_then(|t| t.secondary)
                    .and_then(|w| defs.weapon(w))
                    .map_or(0, |w| w.range)
            } else {
                0
            };
            if let Some(coil) = index
                .values()
                .filter(|b| {
                    b.owner == a.owner && defs.metadata(b.item).is_some_and(|m| m.overpowerable)
                })
                .find(|b| {
                    let p = position_of(world, a);
                    let c = crate::SubPos::centre_of(
                        b.foot.map_or(b.cell, |f| nearest_cell_of(f, a.cell)),
                    );
                    crate::combat::in_range(
                        i64::from(p.x) - i64::from(c.x),
                        i64::from(p.y) - i64::from(c.y),
                        range,
                    )
                })
            {
                *world
                    .resource_mut::<SpecialState>()
                    .coil_charge
                    .entry(coil.id)
                    .or_default() += 1;
            }
        }
        if deployed
            && m.role == Role::Radiation
            && !frozen
            && let ProductionItem::Unit(k) = a.item
            && let Some(w) = defs
                .unit_combat(k)
                .and_then(|t| t.secondary)
                .and_then(|w| defs.weapon(w))
            && world
                .resource::<SpecialState>()
                .eruption_next
                .get(&a.id)
                .is_none_or(|next| tick >= *next)
            && radiation::level(world, a.cell) < w.rad_level / 3
        {
            let spread = defs.warhead(w.warhead).map_or(0, |h| h.cell_spread);
            radiation::add(world, a.cell, spread, w.rad_level, Some(a.id));
            world
                .resource_mut::<SpecialState>()
                .eruption_next
                .insert(a.id, tick.saturating_add(u64::from(w.reload.max(1))));
        }
    }
}
fn retain_beams(world: &mut World, defs: &Definitions, index: &BTreeMap<SimId, Actor>) {
    let old = world.resource::<SpecialState>().erasures.clone();
    for (id, mut erase) in old {
        erase.beams.retain(|source, _| {
            let (Some(a), Some(b)) = (index.get(source), index.get(&id)) else {
                return false;
            };
            let Some(w) = world.get::<Weapon>(a.entity) else {
                return false;
            };
            let p = position_of(world, a);
            let q = b.foot.map_or(position_of(world, b), |f| {
                crate::SubPos::centre_of(nearest_cell_of(f, a.cell))
            });
            w.target == Some(id)
                && !world
                    .resource::<SpecialState>()
                    .erasures
                    .contains_key(source)
                && world
                    .get::<Orders>(a.entity)
                    .is_none_or(|o| o.current.is_none())
                && !world
                    .resource::<SpecialState>()
                    .disabled
                    .contains_key(source)
                && known_to(world, defs, a.owner, b)
                && defs.weapon(w.kind).is_some_and(|d| {
                    crate::combat::in_range(
                        i64::from(p.x) - i64::from(q.x),
                        i64::from(p.y) - i64::from(q.y),
                        d.range,
                    )
                })
        });
        if erase.beams.is_empty() {
            world.resource_mut::<SpecialState>().erasures.remove(&id);
        } else {
            world
                .resource_mut::<SpecialState>()
                .erasures
                .insert(id, erase);
        }
    }
}
/// Returns true when the reference weapon has been fully handled here.
pub(crate) fn fired(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    b: &Actor,
    weapon: crate::WeaponKind,
) -> bool {
    let Some(d) = defs.weapon(weapon) else {
        return false;
    };
    let Some(h) = defs.warhead(d.warhead) else {
        return false;
    };
    if h.parasite && role(defs, a) == Role::Dog && defs.metadata(b.item).is_some_and(|m| m.infantry)
    {
        if !world
            .resource::<SpecialState>()
            .invulnerable(b.id, world.resource::<SimTick>().0)
        {
            if let Some(mut hp) = world.get_mut::<Health>(b.entity) {
                hp.hit_points = 0;
            }
            world.resource_mut::<crate::veterancy::Kills>().record(
                a.id,
                b.owner,
                crate::veterancy::worth(defs, b.item),
            );
        }
        return true;
    }
    if h.temporal {
        let strength = match b.item {
            ProductionItem::Unit(k) => defs.unit_combat(k),
            ProductionItem::Building(k) => defs.building_combat(k),
        }
        .map_or(b.hp, |t| t.hit_points);
        world
            .resource_mut::<SpecialState>()
            .erasures
            .entry(b.id)
            .or_insert(Erasure {
                remaining: (strength.max(1) as u64) * 10,
                beams: BTreeMap::new(),
            })
            .beams
            .insert(a.id, d.damage.max(0) as u32);
        return true;
    }
    if h.mind_control {
        if b.foot.is_some() || defs.metadata(b.item).is_some_and(|m| m.immune_psionics) {
            return true;
        }
        let old = world.resource::<SpecialState>().controlled.clone();
        let original_owner = old.get(&b.id).map_or(b.owner, |(_, owner)| *owner);
        let index = living(world);
        for (id, (caster, owner)) in old {
            if caster == a.id || id == b.id {
                if let Some(v) = index.get(&id) {
                    world.entity_mut(v.entity).insert(Owner(owner));
                    clear_orders(world, v.entity);
                }
                world.resource_mut::<SpecialState>().controlled.remove(&id);
            }
        }
        world
            .resource_mut::<SpecialState>()
            .controlled
            .insert(b.id, (a.id, original_owner));
        world.entity_mut(b.entity).insert(Owner(a.owner));
        clear_orders(world, b.entity);
        return true;
    }
    if h.ivan_bomb {
        plant(world, a, b, defs, false, Some(weapon));
        return true;
    }
    if h.bomb_disarm {
        world.resource_mut::<SpecialState>().charges.remove(&b.id);
        return true;
    }
    if h.makes_disguise || h.electric_assault {
        return true;
    }
    if d.suicide {
        return false;
    }
    // FakeC4/Sapper are ordinary impacts. Legacy role-only commandos keep their M9 charge.
    defs.metadata(a.item).is_some_and(|m| m.c4 || m.teleporter)
}
pub(crate) fn resolve(world: &mut World, defs: &Definitions) {
    if defs.rules().is_none() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let index = living(world);
    retain_beams(world, defs, &index);
    let erasures = world.resource::<SpecialState>().erasures.clone();
    for (id, mut e) in erasures {
        let damage = e
            .beams
            .values()
            .fold(0u64, |s, &d| s.saturating_add(u64::from(d)))
            * 3
            / 4;
        e.remaining = e.remaining.saturating_sub(damage);
        if e.remaining == 0 {
            if let Some(a) = index.get(&id) {
                if let Some(source) = e.beams.keys().next() {
                    world.resource_mut::<crate::veterancy::Kills>().record(
                        *source,
                        a.owner,
                        crate::veterancy::worth(defs, a.item),
                    );
                }
                world
                    .resource_mut::<SpecialState>()
                    .temporal_removed
                    .insert(id);
                if let Some(mut hp) = world.get_mut::<Health>(a.entity) {
                    hp.hit_points = 0;
                }
            }
            world.resource_mut::<SpecialState>().erasures.remove(&id);
        } else {
            world.resource_mut::<SpecialState>().erasures.insert(id, e);
        }
    }
    let charges = world.resource::<SpecialState>().charges.clone();
    for (id, c) in charges {
        if !index.contains_key(&id) {
            world.resource_mut::<SpecialState>().charges.remove(&id);
            continue;
        }
        if c.due > tick {
            continue;
        }
        if let Some(a) = index.get(&id) {
            if c.c4 && defs.metadata(a.item).is_some_and(|m| m.bridge_repair_hut) {
                if world
                    .resource::<crate::MatchState>()
                    .rules()
                    .bridge_destruction
                    && let Some(bridge) = world.resource::<crate::Bridges>().huts.get(&id).copied()
                    && let Some((cell, _)) = world.resource::<crate::Props>().get(bridge)
                {
                    world.resource_mut::<crate::Props>().hurt(cell, i64::MAX);
                }
            } else if let Some(effect) =
                defs.effect(if c.c4 { Effect::C4 } else { Effect::IvanBomb })
            {
                let blast = Blast {
                    point: position_of(world, a),
                    air: false,
                    warhead: effect.warhead,
                    damage: c.damage,
                    direct: Some(id),
                    firepower_pm: 1000,
                    source: Some(c.source),
                };
                if c.c4 {
                    crate::combat::detonate_one(world, blast, id);
                } else {
                    detonate(world, blast, true);
                }
            }
        }
        world.resource_mut::<SpecialState>().charges.remove(&id);
    }
    radiation::advance(world);
}

/// Infantry death-weapon hook; RA6-B extends this dispatcher to vehicle payloads.
/// Every blast uses RA3 and retains the dead source's id for kill credit.
pub(crate) fn finish_deaths(world: &mut World, defs: &Definitions) {
    if defs.rules().is_none() {
        return;
    }
    loop {
        let next = actors(world).into_values().find(|a| {
            !a.alive()
                && defs
                    .metadata(a.item)
                    .is_some_and(|m| m.infantry && m.role == Role::Suicide)
                && match a.item {
                    ProductionItem::Unit(k) => defs
                        .unit_combat(k)
                        .and_then(|t| t.weapon)
                        .and_then(|w| defs.weapon(w))
                        .is_some_and(|w| w.suicide),
                    ProductionItem::Building(_) => false,
                }
                && !world
                    .resource::<SpecialState>()
                    .temporal_removed
                    .contains(&a.id)
                && !world
                    .resource::<SpecialState>()
                    .death_detonated
                    .contains(&a.id)
        });
        let Some(a) = next else {
            break;
        };
        world
            .resource_mut::<SpecialState>()
            .death_detonated
            .insert(a.id);
        let ProductionItem::Unit(k) = a.item else {
            continue;
        };
        if let Some(w) = defs
            .unit_combat(k)
            .and_then(|t| t.weapon)
            .and_then(|w| defs.weapon(w))
        {
            detonate(
                world,
                Blast {
                    point: position_of(world, &a),
                    air: false,
                    warhead: w.warhead,
                    damage: w.damage,
                    direct: None,
                    firepower_pm: 1000,
                    source: Some(a.id),
                },
                true,
            );
        }
    }
    let index = living(world);
    let controls = world.resource::<SpecialState>().controlled.clone();
    for (id, (caster, owner)) in controls {
        if !index.contains_key(&caster) || !index.contains_key(&id) {
            if let Some(a) = index.get(&id) {
                world.entity_mut(a.entity).insert(Owner(owner));
                clear_orders(world, a.entity);
            }
            world.resource_mut::<SpecialState>().controlled.remove(&id);
        }
    }
    refresh_limits(world);
}
