//! RA7 observer concealment and powered reconnaissance; legacy fixture rules remain opt-in.
use super::*;

/// additional per-observer target filter, also consumed by the presentation layer.
pub(crate) fn reconnaissance(world: &mut World) {
    refresh(world, true);
}

/// Refresh concealment before commands and after production/movement, without changing shroud.
pub(crate) fn refresh_concealment(world: &mut World) {
    refresh(world, false);
}

fn refresh(world: &mut World, reveal: bool) {
    if world.resource::<Definitions>().arsenal().0.is_empty() {
        return;
    }
    world.resource_scope(
        |world, defs: bevy_ecs::change_detection::Mut<Definitions>| {
            refresh_with_definitions(world, &defs, reveal);
        },
    );
}

fn refresh_with_definitions(world: &mut World, defs: &Definitions, reveal: bool) {
    let index = actors(world);
    let players: Vec<_> = world.resource::<Players>().iter().map(|(p, _)| p).collect();
    // Explicit prop/garrison observation filters last until the regular Visibility stage;
    // the early pass refreshes derived mobile concealment without deleting those filters.
    let mut hidden: BTreeSet<_> = if reveal {
        BTreeSet::new()
    } else {
        world
            .resource::<SpecialState>()
            .hidden
            .iter()
            .copied()
            .filter(|(_, id)| world.resource::<crate::Props>().get(*id).is_some())
            .collect()
    };
    world
        .resource_mut::<SpecialState>()
        .mirage_trees
        .retain(|id, _| index.get(id).is_some_and(Actor::alive));
    for a in index.values() {
        let Some(m) = defs.metadata(a.item) else {
            continue;
        };
        if reveal && a.alive() && m.role == Role::Satellite && powered(world, defs, a) {
            let (gw, gh) = {
                let g = world.resource::<Grid>();
                (g.width(), g.height())
            };
            if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                p.vision.reveal((0, 0, i64::from(gw), i64::from(gh)), 0);
            }
        }
    }
    for a in index.values() {
        let Some(m) = defs.metadata(a.item) else {
            continue;
        };
        if reveal && a.alive() && m.role == Role::Gap && powered(world, defs, a) {
            for &p in &players {
                if world.resource::<Players>().hostile(p, a.owner)
                    && let Some(p) = world.resource_mut::<Players>().get_mut(p)
                {
                    if defs.rules().is_some() {
                        p.vision.reshroud(a.cell, m.radius);
                    } else {
                        p.vision.conceal(a.cell, m.radius);
                    }
                }
            }
        }
        if !a.alive() {
            continue;
        }
        let still = m.disguise_when_still
            && world
                .get::<Mobile>(a.entity)
                .is_some_and(|m| m.step.is_none())
            && world
                .get::<Orders>(a.entity)
                .is_some_and(|o| o.current.is_none());
        if still
            && !world
                .resource::<SpecialState>()
                .mirage_trees
                .contains_key(&a.id)
        {
            let variant = (world.resource_mut::<crate::SimRng>().next_u32() % 4) as u8;
            world
                .resource_mut::<SpecialState>()
                .mirage_trees
                .insert(a.id, variant);
        } else if !still {
            world
                .resource_mut::<SpecialState>()
                .mirage_trees
                .remove(&a.id);
        }
        let concealed = m.role == Role::Spy
            || still
            || (!m.disguise_when_still
                && m.role == Role::Mirage
                && world
                    .get::<Orders>(a.entity)
                    .is_some_and(|o| o.current.is_none())
                && world
                    .get::<Weapon>(a.entity)
                    .is_some_and(|w| w.cooldown == 0));
        if concealed {
            for &p in &players {
                if p == a.owner {
                    continue;
                }
                let detected = index.values().any(|b| {
                    b.owner == p
                        && b.alive()
                        && (b.foot.is_none() || powered(world, defs, b))
                        && (defs.rules().is_none()
                            || (m.role != Role::Spy && !m.disguise_when_still)
                            || defs.metadata(b.item).is_some_and(|m| m.detect_disguise))
                        && near(
                            a.cell,
                            b.cell,
                            if defs.rules().is_some()
                                && (m.role == Role::Spy || m.disguise_when_still)
                            {
                                if role(defs, b) == Role::Sensor {
                                    defs.metadata(b.item).map_or(0, |m| m.radius)
                                } else {
                                    world.get::<crate::Sight>(b.entity).map_or(0, |s| s.0)
                                }
                            } else if role(defs, b) == Role::Dog
                                || (role(defs, b) == Role::Sensor && powered(world, defs, b))
                            {
                                6
                            } else {
                                2
                            },
                        )
                });
                if !detected {
                    hidden.insert((p, a.id));
                }
            }
        }
    }
    // RA4 (docs/RA2_PARITY_PLAN.md section 4.8): a cloakable unit is hidden from every enemy
    // of its owner that has no unit or building with sensors near enough, unless it fired
    // within the rules' cloak delay.
    if defs.rules().is_some() {
        let tick = world.resource::<SimTick>().0;
        let sensors: Vec<(&Actor, u32)> = index
            .values()
            .filter_map(|b| {
                let sight = defs.metadata(b.item).map_or(0, |entry| entry.sensors_sight);
                (sight > 0 && b.alive() && !matches!(b.item, ProductionItem::Unit(k) if defs.domain(k) == crate::Domain::Air) && (b.foot.is_none() || powered(world, defs, b))).then_some((b, sight))
            })
            .collect();
        for a in index.values() {
            if !defs.metadata(a.item).is_some_and(|entry| entry.cloakable) {
                continue;
            }
            // (The delay itself ends in `crate::flight::advance`, which drops an entry whose
            // tick has come in the `Economy` stage; an entry met here is still running, and
            // the comparison only says so again.)
            if world
                .resource::<SpecialState>()
                .surfaced
                .get(&a.id)
                .is_some_and(|&until| until > tick)
            {
                continue;
            }
            for &p in &players {
                if !world.resource::<Players>().hostile(p, a.owner) {
                    continue;
                }
                let detected = sensors.iter().any(|&(b, sight)| {
                    b.owner == p
                        && near(
                            a.cell,
                            b.foot.map_or(b.cell, |foot| nearest_cell_of(foot, a.cell)),
                            sight,
                        )
                });
                if !detected {
                    hidden.insert((p, a.id));
                }
            }
        }
    }
    // Direct local sight counters gap shrouding; a satellite alone does not.
    if reveal && index.values().any(|a| role(defs, a) == Role::Gap) {
        for a in index.values().filter(|a| a.alive()) {
            if let Some(sight) = world.get::<crate::Sight>(a.entity).copied() {
                let f = a.foot.unwrap_or(Footprint {
                    origin: a.cell,
                    width: 1,
                    height: 1,
                });
                if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                    p.vision.reveal(
                        (
                            i64::from(f.origin.x),
                            i64::from(f.origin.y),
                            i64::from(f.origin.x) + i64::from(f.width) - 1,
                            i64::from(f.origin.y) + i64::from(f.height) - 1,
                        ),
                        sight.0,
                    );
                }
            }
        }
    }
    world.resource_mut::<SpecialState>().hidden = hidden;
}
