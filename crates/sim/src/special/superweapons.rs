//! Delayed strategic payloads and public, location-free timer information.
use super::*;
#[derive(Clone, Debug)]
pub struct Missile {
    pub owner: PlayerId,
    pub source: SimId,
    pub target: CellPos,
    pub due: u64,
}
#[derive(Clone, Debug)]
pub struct Storm {
    pub owner: PlayerId,
    pub source: SimId,
    pub target: CellPos,
    pub starts: u64,
    pub ends: u64,
    pub hit: u64,
    pub scatter: u64,
    pub clouds: Vec<(CellPos, u64)>,
}
#[derive(Clone, Debug)]
pub struct Flight {
    pub owner: PlayerId,
    pub target: CellPos,
    pub infantry: UnitKind,
    pub count: u32,
}
#[derive(Clone, Debug, Default)]
pub struct StrategicState {
    pub missiles: Vec<Missile>,
    pub storm: Option<Storm>,
    pub flights: BTreeMap<SimId, Flight>,
    pub announced: BTreeMap<SimId, PlayerId>,
}
impl StrategicState {
    pub(super) fn hash(&self, h: &mut Fnv1a64) {
        fn cell(h: &mut Fnv1a64, c: CellPos) {
            h.write(&c.x.to_le_bytes());
            h.write(&c.y.to_le_bytes());
        }
        h.write_u64(self.missiles.len() as u64);
        for m in &self.missiles {
            h.write(&[m.owner.0]);
            h.write_u32(m.source.0);
            cell(h, m.target);
            h.write_u64(m.due);
        }
        h.write(&[u8::from(self.storm.is_some())]);
        if let Some(s) = &self.storm {
            h.write(&[s.owner.0]);
            h.write_u32(s.source.0);
            cell(h, s.target);
            for t in [s.starts, s.ends, s.hit, s.scatter] {
                h.write_u64(t);
            }
            h.write_u64(s.clouds.len() as u64);
            for (c, t) in &s.clouds {
                cell(h, *c);
                h.write_u64(*t);
            }
        }
        h.write_u64(self.flights.len() as u64);
        for (id, f) in &self.flights {
            h.write_u32(id.0);
            h.write(&[f.owner.0]);
            cell(h, f.target);
            h.write(&f.infantry.0.to_le_bytes());
            h.write_u32(f.count);
        }
        h.write_u64(self.announced.len() as u64);
        for (id, p) in &self.announced {
            h.write_u32(id.0);
            h.write(&[p.0]);
        }
    }
}
fn square(a: CellPos, b: CellPos) -> bool {
    a.x.abs_diff(b.x) <= 1 && a.y.abs_diff(b.y) <= 1
}
fn covered(a: &Actor, c: CellPos) -> bool {
    a.foot
        .map_or(square(a.cell, c), |f| f.cells().any(|p| square(p, c)))
}
fn validate_launch(
    world: &World,
    defs: &Definitions,
    a: &Actor,
    target: CellPos,
    source: Option<CellPos>,
    check_visibility: bool,
    check_power: bool,
) -> Result<(), RejectReason> {
    let player = a.owner;
    let id = a.id;
    let m = defs
        .metadata(a.item)
        .filter(|m| m.strategic)
        .ok_or(unavailable("No strategic ability"))?;
    if m.show_timer && !world.resource::<crate::MatchState>().rules().superweapons {
        return Err(unavailable("Superweapons are disabled"));
    }
    defs.rules().ok_or(unavailable("No strategic rules"))?;
    for c in [Some(target), source].into_iter().flatten() {
        if !world.resource::<Grid>().contains(c) {
            return Err(RejectReason::OutsideMap(c));
        }
        if check_visibility
            && !world
                .resource::<Players>()
                .get(player)
                .is_some_and(|p| p.vision().is_visible(c))
        {
            return Err(unavailable("Strategic target must be visible"));
        }
    }
    if (check_power && !charges(world, defs, a))
        || world
            .resource::<SpecialState>()
            .charge
            .get(&id)
            .copied()
            .unwrap_or(0)
            < m.cooldown
    {
        return Err(unavailable("Ability requires power and a full charge"));
    }
    if m.role == Role::Storm && world.resource::<SpecialState>().strategic.storm.is_some() {
        return Err(unavailable("A lightning storm is already active"));
    }
    if (m.role == Role::Chronosphere) != source.is_some() {
        return Err(unavailable(
            "Choose a Chronosphere source area and destination",
        ));
    }
    if m.role == Role::Paradrop {
        drop_plan(world, defs, a, target)?;
    } else if !matches!(
        m.role,
        Role::Chronosphere | Role::Curtain | Role::Nuke | Role::Storm
    ) {
        return Err(unavailable("No strategic ability"));
    }
    Ok(())
}

/// Read-only complete validation for an engine-created strategic intent. Unsupported
/// legacy commands are never certified. No result is fed back into the AI's target choice.
pub(crate) fn strategic_intent_valid(
    world: &mut World,
    issued: &IssuedCommand,
    check_visibility: bool,
) -> bool {
    check_strategic_intent(world, issued, check_visibility, false, false)
}

/// Revalidate every current requirement except a newly erased source actor. This
/// read-only counterfactual is used only after a fully valid decision certificate.
pub(crate) fn strategic_intent_erasure_only(world: &mut World, issued: &IssuedCommand) -> bool {
    check_strategic_intent(world, issued, true, true, false)
}

/// Complete current validation except an actual powered-source outage. Charge,
/// actor, target, type, arguments and global strategic conflicts remain required.
pub(crate) fn strategic_intent_power_only(world: &mut World, issued: &IssuedCommand) -> bool {
    check_strategic_intent(world, issued, true, false, true)
}

fn check_strategic_intent(
    world: &mut World,
    issued: &IssuedCommand,
    check_visibility: bool,
    erasure_only: bool,
    power_only: bool,
) -> bool {
    let (id, target, source) = match &issued.command {
        Command::Superweapon {
            building,
            target,
            units,
        } if units.is_empty() => (*building, *target, None),
        Command::Chronoshift {
            building,
            target,
            source,
        } => (*building, *target, Some(*source)),
        _ => return false,
    };
    if world.resource::<Players>().get(issued.player).is_none() {
        return false;
    }
    let index = living(world);
    let defs = world.resource::<Definitions>();
    if defs.rules().is_none() {
        return false;
    }
    own(world, defs, &index, issued.player, id)
        .and_then(|a| {
            if power_only
                && (!world
                    .resource::<Players>()
                    .get(a.owner)
                    .is_some_and(|p| p.low_power())
                    || !defs.metadata(a.item).is_some_and(|m| m.is_powered))
            {
                return Err(unavailable("Source has no power outage"));
            }
            if erasure_only {
                if !world.resource::<SpecialState>().erasures.contains_key(&id) {
                    return Err(unavailable("Target is not being erased"));
                }
                commandable_except_erasure(world, id)?;
            } else {
                commandable(world, id)?;
            }
            validate_launch(
                world,
                defs,
                a,
                target,
                source,
                check_visibility,
                !power_only,
            )
        })
        .is_ok()
}

pub(super) fn launch(
    world: &mut World,
    player: PlayerId,
    id: SimId,
    target: CellPos,
    source: Option<CellPos>,
) -> Result<(), RejectReason> {
    let defs = world.resource::<Definitions>().clone();
    let index = living(world);
    let a = own(world, &defs, &index, player, id)?;
    commandable(world, id)?;
    validate_launch(world, &defs, a, target, source, true, true)?;
    let m = defs
        .metadata(a.item)
        .ok_or(unavailable("No strategic ability"))?;
    let rules = defs
        .rules()
        .ok_or(unavailable("No strategic rules"))?
        .superweapons;
    let tick = world.resource::<SimTick>().0;
    match m.role {
        Role::Chronosphere => {
            let src = source.ok_or(unavailable("Choose source area"))?;
            let selected: Vec<_> = index
                .values()
                .filter(|u| {
                    u.foot.is_none()
                        && square(u.cell, src)
                        && !world
                            .resource::<SpecialState>()
                            .erasures
                            .contains_key(&u.id)
                        && !world.resource::<SpecialState>().spawned.contains_key(&u.id)
                        && !world
                            .resource::<SpecialState>()
                            .strategic
                            .flights
                            .contains_key(&u.id)
                })
                .cloned()
                .collect();
            // All members vacate together, so overlapping source/destination areas preserve offsets.
            for u in &selected {
                vacate(world, u);
            }
            for u in selected {
                let Some(meta) = defs.metadata(u.item) else {
                    continue;
                };
                let dest = CellPos::new(
                    target.x.saturating_add(u.cell.x.saturating_sub(src.x)),
                    target.y.saturating_add(u.cell.y.saturating_sub(src.y)),
                );
                if (meta.infantry && !meta.teleporter)
                    || !world.resource::<Grid>().passable_for(dest, meta.domain)
                    || !world.resource::<Grid>().room_for(dest, u.id)
                {
                    if let Some(mut h) = world.get_mut::<Health>(u.entity) {
                        h.hit_points = 0;
                    }
                    continue;
                }
                teleport(world, &u, dest);
                clear_orders(world, u.entity);
                world
                    .resource_mut::<SpecialState>()
                    .disabled
                    .insert(u.id, tick.saturating_add(rules.chrono_freeze as u64));
            }
        }
        Role::Curtain => {
            for u in index
                .values()
                .filter(|u| covered(u, target))
                .cloned()
                .collect::<Vec<_>>()
            {
                if world
                    .resource::<SpecialState>()
                    .erasures
                    .contains_key(&u.id)
                {
                    continue;
                }
                if defs.metadata(u.item).is_some_and(|m| m.infantry) {
                    if let Some(mut h) = world.get_mut::<Health>(u.entity) {
                        h.hit_points = 0;
                    }
                } else {
                    world.resource_mut::<SpecialState>().protected.insert(
                        u.id,
                        tick.saturating_add(rules.iron_curtain_duration as u64),
                    );
                }
            }
        }
        Role::Nuke => world
            .resource_mut::<SpecialState>()
            .strategic
            .missiles
            .push(Missile {
                owner: player,
                source: id,
                target,
                due: tick.saturating_add(rules.nuclear_flight as u64),
            }),
        Role::Storm => {
            let starts = tick.saturating_add(rules.lightning_deferment as u64);
            world.resource_mut::<SpecialState>().strategic.storm = Some(Storm {
                owner: player,
                source: id,
                target,
                starts,
                ends: starts.saturating_add(rules.lightning_duration as u64),
                hit: starts,
                scatter: starts,
                clouds: vec![],
            });
        }
        Role::Paradrop => drop_plane(world, &defs, a, target)?,
        _ => return Err(unavailable("No strategic ability")),
    }
    world.resource_mut::<SpecialState>().charge.insert(id, 0);
    world
        .resource_mut::<Messages<SimEvent>>()
        .write(SimEvent::SuperweaponLaunched {
            player,
            role: m.role,
        });
    // Location-bearing animation stays visibility-filtered; public announcements carry no location.
    world
        .resource_mut::<Messages<SimEvent>>()
        .write(SimEvent::StrategicActivated {
            player,
            building: id,
            target,
            role: m.role,
            radius: 1,
        });
    Ok(())
}
fn drop_plan(
    world: &World,
    defs: &Definitions,
    a: &Actor,
    target: CellPos,
) -> Result<(CellPos, UnitKind, u32), RejectReason> {
    let (width, height) = {
        let g = world.resource::<Grid>();
        (g.width(), g.height())
    };
    let _plane = defs
        .unit_kind("paradrop_plane")
        .ok_or(unavailable("Paradrop plane unavailable"))?;
    let entry = (0..width)
        .map(|x| CellPos::new(x as i32, 0))
        .chain((0..height).map(|y| CellPos::new(0, y as i32)))
        .filter(|&c| {
            let g = world.resource::<Grid>();
            g.passable_for(c, Domain::Air) && g.navigation_for(Domain::Air).has_room(c, false)
        })
        .min_by_key(|c| c.x.abs_diff(target.x) + c.y.abs_diff(target.y))
        .ok_or(unavailable("No clear plane entry"))?;
    let rules = defs.rules().ok_or(unavailable("No paradrop rules"))?;
    let country = world
        .resource::<Players>()
        .get(a.owner)
        .and_then(|p| p.country());
    let side = country
        .and_then(|c| rules.country(c))
        .and_then(|c| rules.side(c.side))
        .ok_or(unavailable("No paradrop side"))?;
    let american = matches!(a.item,ProductionItem::Building(k) if Some(k)==defs.building_kind("american_airforce_command"));
    let infantry = if american {
        defs.unit_kind("gi").unwrap_or(side.paradrop_infantry)
    } else {
        side.paradrop_infantry
    };
    let count = if american {
        rules.superweapons.american_paradrop_count as u32
    } else {
        side.paradrop_count as u32
    };
    Ok((entry, infantry, count))
}
fn drop_plane(
    world: &mut World,
    defs: &Definitions,
    a: &Actor,
    target: CellPos,
) -> Result<(), RejectReason> {
    let (entry, infantry, count) = drop_plan(world, defs, a, target)?;
    let id = crate::spawn_unit(world, a.owner, "paradrop_plane", entry)
        .map_err(|_| unavailable("No clear plane entry"))?;
    if let Some(e) = actors(world).get(&id).map(|a| a.entity) {
        let tick = world.resource::<SimTick>().0;
        if let Some(mut o) = world.get_mut::<Orders>(e) {
            o.current = Some(MoveOrder::new(target, tick));
        }
    }
    world
        .resource_mut::<SpecialState>()
        .strategic
        .flights
        .insert(
            id,
            Flight {
                owner: a.owner,
                target,
                infantry,
                count,
            },
        );
    Ok(())
}
pub(super) fn advance(world: &mut World, defs: &Definitions) {
    let Some(rules) = defs.rules().map(|r| r.superweapons) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let index = living(world);
    for a in index.values().filter(|a| a.owner != crate::NEUTRAL) {
        let Some(m) = defs.metadata(a.item).filter(|m| m.strategic) else {
            continue;
        };
        let old = world
            .resource::<SpecialState>()
            .strategic
            .announced
            .get(&a.id)
            .copied();
        if old != Some(a.owner) {
            world
                .resource_mut::<SpecialState>()
                .strategic
                .announced
                .insert(a.id, a.owner);
            world.resource_mut::<SpecialState>().charge.insert(a.id, 0);
            if m.show_timer {
                world
                    .resource_mut::<Messages<SimEvent>>()
                    .write(SimEvent::SuperweaponBuilt {
                        player: a.owner,
                        role: m.role,
                    });
            }
        }
        if world.resource::<SpecialState>().charge.get(&a.id) == Some(&m.cooldown) {
            // The ready event is edge-triggered by the charging stage, not repeated here.
        }
    }
    let missiles = std::mem::take(&mut world.resource_mut::<SpecialState>().strategic.missiles);
    for m in missiles {
        if m.due > tick {
            world
                .resource_mut::<SpecialState>()
                .strategic
                .missiles
                .push(m);
            continue;
        }
        if let Some(w) = defs.weapon_kind("nuke_payload") {
            let a = Actor {
                entity: Entity::PLACEHOLDER,
                id: m.source,
                owner: m.owner,
                item: ProductionItem::Building(crate::BuildingKind(0)),
                cell: m.target,
                foot: None,
                hp: 0,
            };
            vehicles::payload(world, defs, &a, w);
        }
    }
    if let Some(mut storm) = world.resource_mut::<SpecialState>().strategic.storm.take()
        && tick < storm.ends
    {
        if tick >= storm.starts {
            let enemies: Vec<_> = world
                .resource::<Players>()
                .iter()
                .map(|(p, _)| p)
                .filter(|&p| world.resource::<Players>().hostile(p, storm.owner))
                .collect();
            for p in enemies {
                if let Some(p) = world.resource_mut::<Players>().get_mut(p) {
                    p.radar = false;
                }
            }
            storm.clouds.retain(|(_, until)| *until > tick);
            if tick >= storm.hit {
                bolt(world, defs, &storm, storm.target);
                storm.clouds.push((
                    storm.target,
                    tick.saturating_add(rules.lightning_hit_delay as u64),
                ));
                storm.hit = tick.saturating_add(rules.lightning_hit_delay as u64);
            }
            if tick >= storm.scatter {
                let spread = rules.lightning_cell_spread.max(1) as u32;
                let rng = &mut *world.resource_mut::<crate::SimRng>();
                let c = CellPos::new(
                    storm.target.x + (rng.next_u32() % spread) as i32 - spread as i32 / 2,
                    storm.target.y + (rng.next_u32() % spread) as i32 - spread as i32 / 2,
                );
                if world.resource::<Grid>().contains(c)
                    && storm.clouds.iter().all(|(b, _)| {
                        b.x.abs_diff(c.x) + b.y.abs_diff(c.y) >= rules.lightning_separation as u32
                    })
                {
                    bolt(world, defs, &storm, c);
                    storm
                        .clouds
                        .push((c, tick.saturating_add(rules.lightning_hit_delay as u64)));
                }
                storm.scatter = tick.saturating_add(rules.lightning_scatter_delay as u64);
            }
        }
        world.resource_mut::<SpecialState>().strategic.storm = Some(storm);
    }
    let flights = world.resource::<SpecialState>().strategic.flights.clone();
    for (id, f) in flights {
        let Some(a) = index.get(&id) else {
            world
                .resource_mut::<SpecialState>()
                .strategic
                .flights
                .remove(&id);
            continue;
        };
        if near(a.cell, f.target, (rules.paradrop_radius as u32) / 256) {
            let name = defs.unit(f.infantry).map(|u| u.id.clone());
            let mut count = f.count;
            if let Some(name) = name {
                let cells: Vec<_> = (0..=4)
                    .flat_map(|r| {
                        (-r..=r).flat_map(move |y| {
                            (-r..=r).map(move |x| CellPos::new(f.target.x + x, f.target.y + y))
                        })
                    })
                    .collect();
                for c in cells.into_iter().filter(|c| near(*c, f.target, 4)) {
                    while count > 0 && crate::spawn_unit(world, f.owner, &name, c).is_ok() {
                        count -= 1;
                    }
                    if count == 0 {
                        break;
                    }
                }
            }
            if count == 0 {
                release(world, a);
                world.despawn(a.entity);
                world
                    .resource_mut::<SpecialState>()
                    .strategic
                    .flights
                    .remove(&id);
            } else if let Some(f) = world
                .resource_mut::<SpecialState>()
                .strategic
                .flights
                .get_mut(&id)
            {
                f.count = count;
            }
        }
    }
}
fn bolt(world: &mut World, defs: &Definitions, s: &Storm, c: CellPos) {
    if let Some(e) = defs.effect(Effect::Lightning) {
        detonate(
            world,
            Blast {
                point: crate::SubPos::centre_of(c),
                air: false,
                warhead: e.warhead,
                damage: e.damage,
                direct: None,
                firepower_pm: 1000,
                source: Some(s.source),
                identity: None,
            },
            true,
        );
    }
}

/// Public rule information only: no structure id, cell, or target is returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicTimer {
    pub owner: PlayerId,
    pub role: Role,
    pub remaining: u32,
}
#[allow(deprecated)]
pub fn public_timers(world: &World) -> Vec<PublicTimer> {
    let Some(defs) = world.get_resource::<Definitions>() else {
        return vec![];
    };
    let Some(state) = world.get_resource::<SpecialState>() else {
        return vec![];
    };
    let mut result = Vec::new();
    for e in world.iter_entities() {
        if let (Some(id), Some(owner), Some(b), Some(h)) = (
            e.get::<SimId>(),
            e.get::<Owner>(),
            e.get::<Building>(),
            e.get::<Health>(),
        ) && h.hit_points > 0
            && owner.0 != crate::NEUTRAL
            && let Some(m) = defs
                .metadata(ProductionItem::Building(b.kind))
                .filter(|m| m.strategic && m.show_timer)
        {
            result.push(PublicTimer {
                owner: owner.0,
                role: m.role,
                remaining: m
                    .cooldown
                    .saturating_sub(state.charge.get(id).copied().unwrap_or(0)),
            });
        }
    }
    result.sort_by_key(|t| (t.owner, t.role as u8, t.remaining));
    result
}
/// Incoming nuclear targets are known to their owner or to a powered Psychic Sensor in range.
/// This deliberately does not return the source building's identity or coordinates.
#[allow(deprecated)]
pub fn nuclear_marks(world: &World, viewer: PlayerId) -> Vec<CellPos> {
    let Some(defs) = world.get_resource::<Definitions>() else {
        return vec![];
    };
    let Some(state) = world.get_resource::<SpecialState>() else {
        return vec![];
    };
    let Some(players) = world.get_resource::<Players>() else {
        return vec![];
    };
    state
        .strategic
        .missiles
        .iter()
        .filter(|m| {
            m.owner == viewer
                || world.iter_entities().any(|e| {
                    let (Some(owner), Some(b), Some(h)) =
                        (e.get::<Owner>(), e.get::<Building>(), e.get::<Health>())
                    else {
                        return false;
                    };
                    owner.0 == viewer
                        && h.hit_points > 0
                        && players.get(viewer).is_some_and(|p| p.powers(defs, b.kind))
                        && defs
                            .metadata(ProductionItem::Building(b.kind))
                            .is_some_and(|meta| {
                                meta.role == Role::Sensor
                                    && near(b.footprint.origin, m.target, meta.radius)
                            })
                })
        })
        .map(|m| m.target)
        .collect()
}
