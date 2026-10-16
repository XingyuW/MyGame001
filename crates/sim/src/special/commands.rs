//! Specialist commands mechanics, moved unchanged from special.rs.
use super::*;

/// Engine-only conditional bridge job. Actor/target authorization precedes readiness;
/// a hidden ship can never turn a bad actor or unknown hut into an accepted goal.
pub(crate) fn bridge_goal_ready(
    world: &mut World,
    issued: &IssuedCommand,
) -> Result<bool, RejectReason> {
    if world.resource::<Players>().get(issued.player).is_none() {
        return Err(RejectReason::UnknownPlayer(issued.player));
    }
    let Command::Interact { unit, target } = issued.command else {
        return Err(unavailable("Choose an Engineer and bridge repair hut"));
    };
    let index = living(world);
    let defs = world.resource::<Definitions>();
    let a = own(world, defs, &index, issued.player, unit)?;
    commandable(world, unit)?;
    let b = index
        .get(&target)
        .ok_or(RejectReason::UnknownTarget(target))?;
    if b.owner != a.owner
        && (!world.resource::<Players>().get(a.owner).is_some_and(|p| {
            b.foot.map_or(p.vision().is_visible(b.cell), |f| {
                p.vision().sees_footprint(f)
            })
        }) || world
            .resource::<SpecialState>()
            .hidden
            .contains(&(a.owner, b.id)))
    {
        return Err(RejectReason::UnknownTarget(target));
    }
    if a.id == b.id
        || role(defs, a) != Role::Engineer
        || !defs.metadata(b.item).is_some_and(|m| m.bridge_repair_hut)
    {
        return Err(unavailable("Choose an Engineer and bridge repair hut"));
    }
    crate::bridges::repair_ready(world, target)
}

pub(crate) fn apply(world: &mut World, issued: &IssuedCommand) -> Result<(), RejectReason> {
    let index = living(world);
    let defs = world.resource::<Definitions>().clone();
    let tick = world.resource::<SimTick>().0;
    if world.resource::<Players>().get(issued.player).is_none() {
        return Err(RejectReason::UnknownPlayer(issued.player));
    }
    match &issued.command {
        Command::Deploy { unit } => {
            if world
                .resource::<SpecialState>()
                .garrisons
                .contains_key(unit)
            {
                return crate::garrison::deploy(world, issued.player, *unit);
            }
            let a = own(world, &defs, &index, issued.player, *unit)?;
            commandable(world, *unit)?;
            let meta = defs
                .metadata(a.item)
                .ok_or(unavailable("This unit has no deploy action"))?;
            if defs.rules().is_some() && meta.deployer {
                return infantry::deploy(world, a, &defs);
            }
            match meta.role {
                Role::Mcv => {
                    // With a rules file (RA1) the base unit of a side deploys into that side's
                    // yard; without one, into the only yard of the M9 arsenal.
                    let kind = match (defs.rules(), a.item) {
                        (Some(rules), ProductionItem::Unit(unit)) => rules
                            .side_of_base_unit(unit)
                            .map(|side| side.construction_yard),
                        (Some(_), ProductionItem::Building(_)) => None,
                        (None, _) => defs.building_kind("construction_yard"),
                    }
                    .ok_or(unavailable("Construction Yard unavailable"))?;
                    let def = defs
                        .building(kind)
                        .ok_or(unavailable("Construction Yard unavailable"))?;
                    let foot = Footprint {
                        origin: a.cell,
                        width: def.width,
                        height: def.height,
                    };
                    let mut grid = world.resource::<Grid>().clone();
                    grid.release(a.cell, a.id);
                    if let Some(m) = world.get::<Mobile>(a.entity)
                        && let Some(s) = m.step
                    {
                        grid.release(s.to, a.id);
                    }
                    // Whether the player sees the footprint comes first, as for a placement
                    // (`economy_commands::check_placement`, docs/DECISIONS.md D33): the
                    // footprint's own errors name the building or unit in a cell. A footprint
                    // outside the map is left to them (`OutsideMap` reveals nothing).
                    if foot.inside(&grid)
                        && !world
                            .resource::<Players>()
                            .get(a.owner)
                            .is_some_and(|p| foot.cells().all(|c| p.vision().is_visible(c)))
                    {
                        return Err(unavailable("Explore the deployment footprint first"));
                    }
                    check_footprint(&grid, foot).map_err(RejectReason::Placement)?;
                    release(world, a);
                    for c in foot.cells() {
                        world.resource_mut::<Grid>().cover(c, a.id);
                    }
                    let (_, owner, b, producer) =
                        building_bundle(&defs, a.id, a.owner, kind, foot, vec![]);
                    let mut e = world.entity_mut(a.entity);
                    e.remove::<(Unit, Mobile, Position, Facing, Orders, Weapon)>();
                    e.insert((owner, b));
                    if let Some(p) = producer {
                        e.insert(p);
                    }
                    if let Some(t) = defs.building_combat(kind) {
                        let (c, w) = combat_components(t);
                        e.insert(c);
                        if let Some(w) = w {
                            e.insert(w);
                        }
                        // With a rules file the yard keeps the share of hit points the unit
                        // had, so that deploying and undeploying repairs nothing (R12).
                        if defs.rules().is_some()
                            && let ProductionItem::Unit(unit) = a.item
                            && let Some(from) = defs.unit_combat(unit)
                        {
                            e.insert(Health {
                                hit_points: share_of(a.hp, from.hit_points, t.hit_points),
                            });
                        }
                    }
                    world
                        .resource_mut::<Messages<SimEvent>>()
                        .write(SimEvent::BuildingPlaced {
                            player: a.owner,
                            building: a.id,
                            kind,
                        });
                }
                Role::Transport | Role::Ifv => unload(world, a, &defs)?,
                Role::Radiation => {
                    if world
                        .resource::<SpecialState>()
                        .disabled
                        .get(&a.id)
                        .is_some_and(|&t| t > tick)
                    {
                        return Err(unavailable("Radiation deployment is recharging"));
                    }
                    world.resource_mut::<SpecialState>().zones.push(Zone {
                        role: Role::Radiation,
                        cell: a.cell,
                        radius: meta.radius,
                        damage: meta.strength,
                        remaining: meta.cooldown,
                    });
                    world
                        .resource_mut::<SpecialState>()
                        .disabled
                        .insert(a.id, tick.saturating_add(u64::from(meta.cooldown)));
                    clear_orders(world, a.entity);
                }
                _ => return Err(unavailable("This unit has no deploy action")),
            }
        }
        Command::Undeploy { building } => {
            let a = index
                .get(building)
                .filter(|a| a.foot.is_some())
                .ok_or(RejectReason::UnknownBuilding(*building))?;
            let (ProductionItem::Building(kind), Some(foot)) = (a.item, a.foot) else {
                return Err(RejectReason::UnknownBuilding(*building));
            };
            if a.owner != issued.player {
                // Another player's building the player does not see is reported like one that
                // does not exist (docs/DECISIONS.md D30, D33).
                let seen = !defs.has_combat()
                    || world
                        .resource::<Players>()
                        .get(issued.player)
                        .is_some_and(|p| p.vision().sees_footprint(foot));
                return Err(if seen {
                    RejectReason::NotOwned(*building)
                } else {
                    RejectReason::UnknownBuilding(*building)
                });
            }
            if !world.resource::<crate::MatchState>().rules().mcv_repacks {
                return Err(RejectReason::DisabledByOptions);
            }
            let unit = defs
                .rules()
                .and_then(|rules| rules.side_of_construction_yard(kind))
                .map(|side| side.base_unit)
                .ok_or(unavailable("This building does not undeploy"))?;
            let speed = defs
                .unit(unit)
                .map(|def| def.speed)
                .ok_or(unavailable("This building does not undeploy"))?;
            let domain = defs.domain(unit);
            let on_foot = defs.on_foot(unit);
            // The unit stands where the footprint's origin was: once the building is gone the
            // cell must be ground the unit may stand on, with nothing of its layer in it.
            let mut grid = world.resource::<Grid>().clone();
            for c in foot.cells() {
                grid.uncover(c, a.id);
            }
            if !grid.passable_for(foot.origin, domain)
                || grid.occupant_for(foot.origin, domain).is_some()
            {
                return Err(unavailable("No room to undeploy"));
            }
            let point = {
                let mut grid = world.resource_mut::<Grid>();
                for c in foot.cells() {
                    grid.uncover(c, a.id);
                }
                grid.register(a.id, domain, on_foot);
                if !grid.claim(foot.origin, a.id) {
                    // The cell refuses the unit after all (not expected after the check
                    // above): the yard stays as it was.
                    grid.unregister(a.id);
                    for c in foot.cells() {
                        grid.cover(c, a.id);
                    }
                    return Err(unavailable("No room to undeploy"));
                }
                grid.standing_point(foot.origin, a.id)
            };
            if let Some(p) = world.resource_mut::<Players>().get_mut(a.owner) {
                p.repairing.remove(&a.id);
            }
            world.resource_mut::<SpecialState>().charge.remove(&a.id);
            let mut e = world.entity_mut(a.entity);
            e.remove::<(Building, crate::Producer, Weapon)>();
            e.insert(unit_bundle(a.id, a.owner, unit, speed, foot.origin, point));
            if let Some(t) = defs.unit_combat(unit) {
                let (c, w) = combat_components(t);
                e.insert(c);
                if let Some(w) = w {
                    e.insert(w);
                }
                // The unit keeps the share of hit points the yard had (R12).
                if let Some(from) = defs.building_combat(kind) {
                    e.insert(Health {
                        hit_points: share_of(a.hp, from.hit_points, t.hit_points),
                    });
                }
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::BuildingUndeployed {
                    player: a.owner,
                    building: a.id,
                    kind,
                    unit: a.id,
                    into: unit,
                });
        }
        Command::Interact { unit, target } => {
            let a = own(world, &defs, &index, issued.player, *unit)?;
            commandable(world, *unit)?;
            if world.resource::<crate::Props>().get(*target).is_some() {
                return crate::garrison::order(world, &defs, a, *target);
            }
            let b = index
                .get(target)
                .ok_or(RejectReason::UnknownTarget(*target))?;
            if b.owner != a.owner
                && (!world.resource::<Players>().get(a.owner).is_some_and(|p| {
                    b.foot.map_or(p.vision().is_visible(b.cell), |f| {
                        p.vision().sees_footprint(f)
                    })
                }) || world
                    .resource::<SpecialState>()
                    .hidden
                    .contains(&(a.owner, b.id)))
            {
                return Err(RejectReason::UnknownTarget(*target));
            }
            validate_interaction(world, a, b, &defs)?;
            // A new order ends a repair at a dock.
            world.resource_mut::<SpecialState>().docked.remove(&a.id);
            if distance(a, b) <= 1 {
                interact(world, a, b, &defs)?;
            } else {
                world
                    .resource_mut::<SpecialState>()
                    .interactions
                    .insert(a.id, b.id);
                clear_orders(world, a.entity);
            }
        }
        Command::Chronoshift {
            building,
            source,
            target,
        } => {
            return superweapons::launch(world, issued.player, *building, *target, Some(*source));
        }
        Command::Superweapon {
            building,
            target,
            units,
        } => {
            if defs.rules().is_some()
                && defs
                    .metadata(index.get(building).map_or(
                        ProductionItem::Building(crate::BuildingKind(u16::MAX)),
                        |a| a.item,
                    ))
                    .is_some_and(|m| m.strategic)
            {
                // Authorize the source before inspecting optional unit arguments: hidden
                // enemy sources must answer exactly as missing sources, regardless of type.
                own(world, &defs, &index, issued.player, *building)?;
                commandable(world, *building)?;
                if index
                    .get(building)
                    .is_some_and(|a| role(&defs, a) == Role::Chronosphere)
                {
                    let mut ids = BTreeSet::new();
                    for id in units {
                        if !ids.insert(*id) {
                            return Err(RejectReason::DuplicateUnit(*id));
                        }
                        own(world, &defs, &index, issued.player, *id)?;
                        commandable(world, *id)?;
                    }
                }
                return superweapons::launch(world, issued.player, *building, *target, None);
            }
            let a = own(world, &defs, &index, issued.player, *building)?;
            let meta = defs
                .metadata(a.item)
                .ok_or(unavailable("No strategic ability"))?;
            if !meta.role.superweapon() {
                return Err(unavailable("No strategic ability"));
            }
            let state = world
                .resource::<Players>()
                .get(a.owner)
                .ok_or(RejectReason::UnknownPlayer(a.owner))?;
            if !world.resource::<Grid>().contains(*target) {
                return Err(RejectReason::OutsideMap(*target));
            }
            if !state.vision().is_visible(*target) {
                return Err(unavailable("Strategic target must be visible"));
            }
            if !charges(world, &defs, a)
                || world
                    .resource::<SpecialState>()
                    .charge
                    .get(building)
                    .copied()
                    .unwrap_or(0)
                    < meta.cooldown
            {
                return Err(unavailable("Ability requires power and a full charge"));
            }
            if meta.role == Role::Chronosphere {
                if units.is_empty() || units.len() > crate::command::MAX_UNITS_PER_COMMAND {
                    return Err(unavailable("Select eligible mechanical units first"));
                }
                let mut taken = BTreeSet::new();
                let mut plan = Vec::new();
                let mut ids = BTreeSet::new();
                for id in units {
                    if !ids.insert(*id) {
                        return Err(RejectReason::DuplicateUnit(*id));
                    }
                    let u = own(world, &defs, &index, a.owner, *id)?;
                    commandable(world, *id)?;
                    if defs.category(u.item) != Category::Vehicles
                        || matches!(u.item, ProductionItem::Building(_))
                    {
                        return Err(unavailable("Chronosphere requires mechanical units"));
                    }
                    if !near(
                        u.cell,
                        index.get(&units[0]).map_or(u.cell, |a| a.cell),
                        meta.radius,
                    ) {
                        return Err(unavailable("Selected units must be grouped together"));
                    }
                    let g = world.resource::<Grid>();
                    let view = g.navigation_for(g.domain_of(*id));
                    let dest = crate::pathfinding::destination_cells(&view, *target, 1, |c| {
                        taken.contains(&c)
                            || view.occupant(c).is_some()
                            || !state.vision().is_visible(c)
                    })
                    .first()
                    .copied()
                    .filter(|c| near(*c, *target, meta.radius))
                    .ok_or(unavailable("No clear teleport destination"))?;
                    taken.insert(dest);
                    plan.push((u.clone(), dest));
                }
                for (u, c) in plan {
                    teleport(world, &u, c);
                    clear_orders(world, u.entity);
                }
            } else if meta.role == Role::Curtain {
                let eligible: Vec<_> = index
                    .values()
                    .filter(|u| {
                        u.owner == a.owner
                            && defs.category(u.item) == Category::Vehicles
                            && near(u.cell, *target, meta.radius)
                    })
                    .map(|u| u.id)
                    .collect();
                if eligible.is_empty() {
                    return Err(unavailable("No friendly mechanical units in target area"));
                }
                for id in eligible {
                    world
                        .resource_mut::<SpecialState>()
                        .protected
                        .insert(id, tick.saturating_add(meta.strength as u64));
                }
            } else if meta.role == Role::Storm {
                world.resource_mut::<SpecialState>().zones.push(Zone {
                    role: Role::Storm,
                    cell: *target,
                    radius: meta.radius,
                    damage: meta.strength / 10,
                    remaining: 200,
                });
            } else if !area(world, Effect::Nuke, crate::SubPos::centre_of(*target), None) {
                blast(world, *target, meta.radius, meta.strength);
            }
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::StrategicActivated {
                    player: a.owner,
                    building: *building,
                    target: *target,
                    role: meta.role,
                    radius: meta.radius,
                });
            world
                .resource_mut::<SpecialState>()
                .charge
                .insert(*building, 0);
        }
        _ => return Err(unavailable("Not a specialist action")),
    }
    Ok(())
}
