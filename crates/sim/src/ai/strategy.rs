//! RA8 capabilities use the same snapshots, honest memory and ordinary commands as the core AI.
use super::*;
use std::collections::VecDeque;

pub(super) fn difficulty<'a>(
    view: &'a View<'_>,
    controller: &AiController,
) -> Option<&'a crate::DifficultyRules> {
    view.ai
        .rules_difficulty
        .then(|| {
            view.definitions
                .rules()?
                .difficulties
                .iter()
                .find(|r| r.id == controller.difficulty.id)
        })
        .flatten()
}

pub(super) fn only_aa_building(defs: &Definitions, kind: BuildingKind) -> bool {
    defs.building_combat(kind).is_some_and(|t| {
        t.weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|k| defs.weapon(k))
            .all(|w| !w.anti_ground)
    })
}

pub(super) fn recruit(view: &View<'_>, controller: &AiController, kind: UnitKind) -> bool {
    if !view.ai.rules_difficulty {
        return true;
    }
    let meta = view.definitions.metadata(ProductionItem::Unit(kind));
    if view.definitions.domain(kind) == Domain::Naval {
        let Some(goal) = controller.naval_goal else {
            return false;
        };
        if !naval_targets(view, controller)
            .into_iter()
            .any(|(footprint, target)| {
                naval_reach(view, kind, &target)
                    .is_some_and(|reach| within_reach(footprint, goal, reach))
            })
        {
            return false;
        }
    }
    if meta.is_some_and(|m| matches!(m.role, Role::Ifv | Role::Flak)) && !controller.seen_air {
        return false;
    }
    // Aircraft are additionally checked by normal dock-aware QueueProduction validation.
    true
}

fn stable(view: &View<'_>, cell: CellPos) -> bool {
    view.vision.is_visible(cell) && (!view.vision.uses_fog() || view.stays_visible(cell))
}

fn role(view: &View<'_>, kind: UnitKind) -> Role {
    view.definitions
        .metadata(ProductionItem::Unit(kind))
        .map_or(Role::Combat, |m| m.role)
}

pub(super) fn decide(
    view: &View<'_>,
    controller: &mut AiController,
    special: &crate::SpecialState,
    props: &crate::Props,
) -> Vec<Command> {
    controller.reserved.clear();
    if !view.ai.rules_difficulty {
        return Vec::new();
    }
    let mut commands = Vec::new();
    naval(view, controller);
    // One paid building repair per decision; the ordinary repair system retains the toggle.
    if let Some(building) = view.own_buildings.iter().find(|b| {
        view.definitions
            .building_combat(b.kind)
            .is_some_and(|t| b.hp < t.hit_points)
            && view
                .definitions
                .metadata(ProductionItem::Building(b.kind))
                .is_some_and(|m| m.repairable)
            && !view.state.repairing().contains(&b.id)
            && view.state.credits() > 0
    }) {
        commands.push(Command::RepairBuilding {
            building: building.id,
        });
    }
    // A single engineer is enough for a nearby observed capture. Bridge recovery has priority.
    if controller.broken_bridges.is_empty() {
        let target = view
            .seen_tech
            .iter()
            .min_by_key(|b| (b.footprint.distance_to_cell(controller.start), b.id));
        if let Some(target) = target {
            if let Some(engineer) = view
                .own_units
                .iter()
                .find(|u| role(view, u.kind) == Role::Engineer && u.idle())
            {
                controller.reserved.insert(engineer.id);
                if stable(view, target.footprint.origin) {
                    commands.push(Command::Interact {
                        unit: engineer.id,
                        target: target.id,
                    });
                }
            } else if !view
                .own_units
                .iter()
                .any(|u| role(view, u.kind) == Role::Engineer)
            {
                queue_support(view, Role::Engineer, &mut commands);
            }
        }
    }
    if controller.unseen_loss
        && !view
            .own_units
            .iter()
            .any(|u| role(view, u.kind) == Role::Dog)
    {
        queue_support(view, Role::Dog, &mut commands);
    }
    // Damaged vehicles near home use actual service bays. Their interaction removes them
    // from subsequent army candidates until repaired/released.
    for unit in view
        .own_units
        .iter()
        .filter(|u| u.idle())
        .take(crate::MAX_AI_ARMY as usize)
    {
        let Some(meta) = view.definitions.metadata(ProductionItem::Unit(unit.kind)) else {
            continue;
        };
        let Some(health) = view.definitions.unit_combat(unit.kind) else {
            continue;
        };
        if meta.infantry
            || meta.domain == Domain::Air
            || unit.hp >= health.hit_points
            || chebyshev(unit.mobile.cell, controller.start) > view.ai.policy.depot_radius
        {
            continue;
        }
        if let Some(depot) = view
            .own_buildings
            .iter()
            .filter(|b| {
                view.definitions
                    .metadata(ProductionItem::Building(b.kind))
                    .is_some_and(|m| {
                        m.unit_repair
                            && (m.role == Role::Shipyard) == (meta.domain == Domain::Naval)
                            && (m.vehicle_docks == 0
                                || special
                                    .docked
                                    .iter()
                                    .chain(&special.interactions)
                                    .filter(|(_, host)| **host == b.id)
                                    .map(|(unit, _)| *unit)
                                    .collect::<BTreeSet<_>>()
                                    .len()
                                    < m.vehicle_docks as usize)
                    })
            })
            .min_by_key(|b| (b.footprint.distance_to_cell(unit.mobile.cell), b.id))
        {
            controller.reserved.insert(unit.id);
            commands.push(Command::Interact {
                unit: unit.id,
                target: depot.id,
            });
            break;
        }
    }
    // Keep a small near-base garrison without consuming every infantry attack wave.
    let occupied: usize = special
        .garrisons
        .iter()
        .filter(|(_, g)| g.owner == view.player)
        .map(|(id, _)| special.cargo.get(id).map_or(0, Vec::len))
        .sum::<usize>()
        + special
            .interactions
            .iter()
            .filter(|(unit, host)| {
                view.living_owned_ids.contains(unit)
                    && props.get(**host).is_some_and(|(_, p)| {
                        crate::garrison::item(view.definitions, p.kind)
                            .and_then(|item| view.definitions.metadata(item))
                            .is_some_and(|meta| meta.can_be_occupied)
                    })
            })
            .count();
    if occupied < view.ai.policy.garrison_quota as usize && view.state.credits() > 0 {
        let host = props
            .iter()
            .filter(|(cell, _)| {
                view.vision.is_visible(*cell)
                    && chebyshev(*cell, controller.start) <= view.ai.policy.garrison_radius
            })
            .filter_map(|(cell, p)| {
                let id = props.id_at(cell)?;
                let item = crate::garrison::item(view.definitions, p.kind)?;
                let meta = view.definitions.metadata(item)?;
                let incoming = special
                    .interactions
                    .iter()
                    .filter(|(unit, host)| **host == id && view.living_owned_ids.contains(unit))
                    .count();
                let full = special
                    .cargo
                    .get(&id)
                    .map_or(0, Vec::len)
                    .saturating_add(incoming)
                    >= meta.max_occupants as usize;
                let maximum = match item {
                    ProductionItem::Building(k) => view
                        .definitions
                        .building_combat(k)
                        .map_or(p.kind.hit_points(), |t| t.hit_points),
                    _ => p.kind.hit_points(),
                };
                let red = view
                    .definitions
                    .rules()
                    .map_or(25, |r| r.combat.condition_red_percent);
                (i64::from(p.hit_points) * 100 > i64::from(maximum) * i64::from(red)
                    && meta.can_be_occupied
                    && !full
                    && special
                        .garrisons
                        .get(&id)
                        .is_none_or(|g| g.owner == view.player))
                .then_some((cell, id))
            })
            .min_by_key(|(cell, id)| (chebyshev(*cell, controller.start), *id));
        if let Some((cell, host)) = host
            && stable(view, cell)
            && let Some(unit) = view.own_units.iter().find(|u| {
                u.idle()
                    && !controller.wave.contains(&u.id)
                    && view
                        .definitions
                        .metadata(ProductionItem::Unit(u.kind))
                        .is_some_and(|m| m.occupier)
            })
        {
            controller.reserved.insert(unit.id);
            commands.push(Command::Interact {
                unit: unit.id,
                target: host,
            });
        }
    }
    let ground_wave: Vec<_> = view
        .own_units
        .iter()
        .filter(|u| {
            controller.wave.contains(&u.id)
                && view.definitions.domain(u.kind) != Domain::Naval
                && !controller.reserved.contains(&u.id)
        })
        .copied()
        .collect();
    if let Some(target) = controller
        .known
        .iter()
        .filter(|(_, b)| stable(view, b.footprint.origin))
        .min_by_key(|(id, b)| (b.footprint.distance_to_cell(controller.start), **id))
    {
        // Unarmed miners must not distract a wave indefinitely from the actual base.
        if !view
            .enemy_units
            .iter()
            .any(|u| u.armed && u.harvest.is_none())
        {
            let units: Vec<_> = ground_wave
                .iter()
                .filter(|u| view.fights(u))
                .map(|u| u.id)
                .collect();
            if !units.is_empty() {
                commands.push(Command::Attack {
                    units,
                    target: *target.0,
                });
            }
        } else {
            // Long range weapons sit behind the foremost tank, acquiring through normal combat.
            let front = ground_wave
                .iter()
                .filter(|u| {
                    view.definitions.domain(u.kind) == Domain::Land
                        && !view.definitions.on_foot(u.kind)
                        && range(view.definitions, u.kind)
                            < view.ai.policy.artillery_min_range * crate::SUBCELLS_PER_CELL as u32
                })
                .min_by_key(|u| chebyshev(u.mobile.cell, target.1.footprint.origin));
            if let Some(front) = front {
                let goal = target.1.footprint.origin;
                let distance = view.ai.policy.artillery_standoff as i32;
                let cell = CellPos::new(
                    front
                        .mobile
                        .cell
                        .x
                        .saturating_sub((goal.x - front.mobile.cell.x).signum() * distance),
                    front
                        .mobile
                        .cell
                        .y
                        .saturating_sub((goal.y - front.mobile.cell.y).signum() * distance),
                );
                let units: Vec<_> = ground_wave
                    .iter()
                    .filter(|u| {
                        u.idle()
                            && view.definitions.domain(u.kind) == Domain::Land
                            && range(view.definitions, u.kind)
                                >= view.ai.policy.artillery_min_range
                                    * crate::SUBCELLS_PER_CELL as u32
                    })
                    .map(|u| u.id)
                    .collect();
                if !units.is_empty() && view.grid.contains(cell) {
                    commands.push(Command::AttackMove {
                        units,
                        target: cell,
                    });
                }
            }
        }
    }
    if let Some(goal) = controller.naval_goal {
        let units: Vec<_> = view
            .own_units
            .iter()
            .filter(|u| {
                view.definitions.domain(u.kind) == Domain::Naval
                    && u.idle()
                    && !controller.reserved.contains(&u.id)
            })
            .map(|u| u.id)
            .collect();
        if !units.is_empty() {
            commands.push(Command::AttackMove {
                units,
                target: goal,
            });
        }
    }
    if let Some(command) = strategic(view, controller, special, &ground_wave) {
        commands.push(command);
    }
    commands
}

fn queue_support(view: &View<'_>, needed: Role, commands: &mut Vec<Command>) {
    if let Some(&kind) = view.ai.support.iter().find(|&&k| {
        role(view, k) == needed
            && view.queued(ProductionItem::Unit(k)) == 0
            && view.queueable(ProductionItem::Unit(k))
    }) {
        commands.push(Command::QueueProduction {
            item: ProductionItem::Unit(kind),
        });
    }
}

pub(super) fn range(defs: &Definitions, kind: UnitKind) -> u32 {
    defs.unit_combat(kind).map_or(0, |t| {
        t.weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|k| defs.weapon(k))
            .map(|w| w.range)
            .max()
            .unwrap_or(0)
    })
}

fn within_reach(footprint: Footprint, cell: CellPos, reach: u32) -> bool {
    let x0 = i64::from(footprint.origin.x);
    let y0 = i64::from(footprint.origin.y);
    let x1 = x0 + i64::from(footprint.width.max(1)) - 1;
    let y1 = y0 + i64::from(footprint.height.max(1)) - 1;
    let x = i64::from(cell.x);
    let y = i64::from(cell.y);
    let dx = x - x.clamp(x0, x1);
    let dy = y - y.clamp(y0, y1);
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
        <= i64::from(reach).saturating_mul(i64::from(reach))
}

fn naval_reach(view: &View<'_>, kind: UnitKind, target: &crate::TargetTraits) -> Option<u32> {
    let unit = view.definitions.unit_combat(kind)?;
    let (weapon, percent) =
        crate::weapon_against(view.definitions, &unit, unit.weapons(false), target)?;
    (percent > 1)
        .then(|| view.definitions.weapon(weapon).map(|w| w.range / 256))
        .flatten()
}

fn naval_targets(
    view: &View<'_>,
    controller: &AiController,
) -> Vec<(Footprint, crate::TargetTraits)> {
    controller
        .known
        .values()
        .filter_map(|b| {
            let t = view.definitions.building_combat(b.kind)?;
            Some((
                b.footprint,
                crate::TargetTraits {
                    armour: t.armour,
                    wall: t.wall,
                    building: true,
                    immune_psionics: true,
                    on_water: view.grid.cell(b.footprint.origin).is_some_and(|c| c.water),
                    ..crate::TargetTraits::default()
                },
            ))
        })
        .chain(
            view.enemy_units
                .iter()
                .filter(|u| view.definitions.domain(u.kind) == Domain::Naval)
                .filter_map(|u| {
                    let t = view.definitions.unit_combat(u.kind)?;
                    Some((
                        cell_footprint(u.mobile.cell),
                        crate::TargetTraits {
                            armour: t.armour,
                            on_water: true,
                            underwater: t.underwater,
                            organic: t.organic,
                            ..crate::TargetTraits::default()
                        },
                    ))
                }),
        )
        .collect()
}

fn damages_buildings(view: &View<'_>, kind: UnitKind) -> bool {
    let Some(terms) = view.definitions.unit_combat(kind) else {
        return false;
    };
    view.definitions
        .buildings()
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            let kind = BuildingKind(u16::try_from(index).ok()?);
            if view
                .definitions
                .metadata(ProductionItem::Building(kind))
                .is_some_and(|m| m.immune || m.insignificant)
            {
                return None;
            }
            view.definitions.building_combat(kind)
        })
        .any(|target| {
            let target = crate::TargetTraits {
                armour: target.armour,
                wall: target.wall,
                building: true,
                immune_psionics: true,
                ..crate::TargetTraits::default()
            };
            crate::weapon_against(view.definitions, &terms, terms.weapons(false), &target)
                .is_some_and(|(weapon, percent)| {
                    percent > 1
                        && view
                            .definitions
                            .weapon(weapon)
                            .is_some_and(|w| w.damage > 0)
                })
        })
}

// Only observed hostile recruits with a useful weapon and an eligible control shot count.
// Static terrain can prove isolation; temporary traffic/buildings cannot prove permanent loss.
fn useful_control_opportunity(
    view: &View<'_>,
    unit: &UnitSnap,
    terms: &crate::CombatTerms,
) -> bool {
    let goals: Vec<_> = view
        .enemy_units
        .iter()
        .filter_map(|enemy| {
            if !damages_buildings(view, enemy.kind) {
                return None;
            }
            let target = view.definitions.unit_combat(enemy.kind)?;
            let meta = view
                .definitions
                .metadata(ProductionItem::Unit(enemy.kind))?;
            let mut traits = crate::combat::traits_of(
                view.grid,
                enemy.id,
                target.armour,
                &crate::combat::Body::Unit {
                    cell: enemy.mobile.cell,
                    next: enemy.mobile.step.map(|step| step.to),
                },
                Some(&target),
            );
            traits.immune_psionics |= meta.immune_psionics || meta.role == Role::MindControl;
            let (weapon, percent) =
                crate::weapon_against(view.definitions, terms, terms.weapons(false), &traits)?;
            let weapon = view.definitions.weapon(weapon)?;
            (percent > 1
                && view
                    .definitions
                    .warhead(weapon.warhead)
                    .is_some_and(|h| h.mind_control))
            .then_some((
                cell_footprint(enemy.mobile.cell),
                weapon.range / crate::SUBCELLS_PER_CELL as u32,
            ))
        })
        .collect();
    if goals.is_empty() {
        return false;
    }
    let mut seen = BTreeSet::from([unit.mobile.cell]);
    let mut open = VecDeque::from([unit.mobile.cell]);
    while let Some(cell) = open.pop_front() {
        if goals
            .iter()
            .any(|(target, range)| within_reach(*target, cell, *range))
        {
            return true;
        }
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
            let terrain =
                view.grid
                    .cell(next)
                    .is_some_and(|cell| match view.definitions.domain(unit.kind) {
                        Domain::Land => cell.passable,
                        Domain::Naval => cell.water,
                        Domain::Amphibious => cell.passable || cell.water,
                        Domain::Air => true,
                    });
            if terrain && seen.insert(next) {
                open.push_back(next);
            }
        }
    }
    false
}

/// Army membership includes detectors and parasites. Only actual victory capability
/// can postpone liquidation once the economy has stopped.
pub(super) fn can_finish(view: &View<'_>, controller: &AiController, unit: &UnitSnap) -> bool {
    if !view.fights(unit) {
        return false;
    }
    if !view.ai.rules_difficulty {
        return true;
    }
    let Some(terms) = view.definitions.unit_combat(unit.kind) else {
        return false;
    };
    if !damages_buildings(view, unit.kind) {
        let control = terms
            .weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|k| view.definitions.weapon(k))
            .any(|w| {
                view.definitions
                    .warhead(w.warhead)
                    .is_some_and(|h| h.mind_control)
            });
        return control
            && (useful_control_opportunity(view, unit, &terms)
                || (unit.has_order && controller.waypoints.iter().any(|w| w.seen == 0)));
    }
    if view.definitions.domain(unit.kind) != Domain::Naval {
        return true;
    }
    let targets: Vec<_> = naval_targets(view, controller)
        .into_iter()
        .filter_map(|(f, t)| naval_reach(view, unit.kind, &t).map(|range| (f, range)))
        .collect();
    if targets.is_empty() {
        return unit.mobile.step.is_some();
    }
    let mut seen = BTreeSet::from([unit.mobile.cell]);
    let mut open = VecDeque::from([unit.mobile.cell]);
    while let Some(cell) = open.pop_front() {
        if targets.iter().any(|(f, r)| within_reach(*f, cell, *r)) {
            return true;
        }
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
            // Temporary traffic and buildings cannot prove a permanently stranded fleet.
            if view.grid.cell(next).is_some_and(|c| c.water) && seen.insert(next) {
                open.push_back(next);
            }
        }
    }
    false
}

fn naval(view: &View<'_>, controller: &mut AiController) {
    if view.tick < controller.next_naval_scan {
        return;
    }
    controller.next_naval_scan = view
        .tick
        .saturating_add(u64::from(view.ai.policy.naval_rescan));
    controller.naval_goal = None;
    controller.naval_site = None;
    let goals: Vec<_> = naval_targets(view, controller)
        .into_iter()
        .filter_map(|(footprint, target)| {
            let reach = view
                .ai
                .army
                .iter()
                .filter(|&&kind| view.definitions.domain(kind) == Domain::Naval)
                .filter_map(|&kind| naval_reach(view, kind, &target))
                .max()?;
            Some((footprint, reach))
        })
        .collect();
    if goals.is_empty() {
        return;
    }
    let yard = view.ai.build_order.iter().copied().find(|&k| {
        view.definitions
            .metadata(ProductionItem::Building(k))
            .is_some_and(|m| m.role == Role::Shipyard)
    });
    let Some(yard) = yard else {
        return;
    };
    let Some(def) = view.definitions.building(yard) else {
        return;
    };
    let water = |cell| {
        view.grid.cell(cell).is_some_and(|c| c.water)
            && !view
                .own_buildings
                .iter()
                .any(|b| b.footprint.contains(cell))
            && (!view.vision.is_visible(cell) || view.grid.building(cell).is_none())
    };
    // Cache each component once while trying legal sites. Unknown building occupancy
    // cannot change planning; only static water and observed/owned blockers may do so.
    let mut components: BTreeMap<CellPos, Option<CellPos>> = BTreeMap::new();
    let mut chosen = None;
    let mut accepts = |origin| {
        let footprint = Footprint {
            origin,
            width: def.width,
            height: def.height,
        };
        for start in footprint.ring().filter(|&cell| water(cell)) {
            if let Some(&goal) = components.get(&start) {
                if goal.is_some() {
                    chosen = goal;
                    return true;
                }
                continue;
            }
            let mut visited = BTreeSet::from([start]);
            let mut open = VecDeque::from([start]);
            let mut best = None;
            while let Some(cell) = open.pop_front() {
                if let Some(distance) = goals
                    .iter()
                    .filter_map(|(f, reach)| {
                        within_reach(*f, cell, *reach).then_some(f.distance_to_cell(cell))
                    })
                    .min()
                {
                    let key = (distance, cell.y, cell.x);
                    if best.as_ref().is_none_or(|(found, _)| key < *found) {
                        best = Some((key, cell));
                    }
                }
                for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                    let next = CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy));
                    if water(next) && visited.insert(next) {
                        open.push_back(next);
                    }
                }
            }
            let goal = best.map(|(_, cell)| cell);
            for cell in visited {
                components.insert(cell, goal);
            }
            if goal.is_some() {
                chosen = goal;
                return true;
            }
        }
        false
    };
    let existing: Vec<_> = view
        .own_buildings
        .iter()
        .filter(|b| b.kind == yard)
        .map(|b| b.footprint.origin)
        .collect();
    let site = if existing.is_empty() {
        find_placement_where(view, controller, yard, &mut accepts)
    } else {
        existing.into_iter().find(|&origin| accepts(origin))
    };
    controller.naval_goal = chosen;
    controller.naval_site = site;
}

fn weight(view: &View<'_>, kind: BuildingKind, weights: &[u32; 12]) -> u32 {
    let item = ProductionItem::Building(kind);
    let Some(meta) = view.definitions.metadata(item) else {
        return 0;
    };
    let index = if meta.role.major_superweapon() {
        11
    } else if meta.factory == Some(crate::FactoryGroup::Buildings) {
        0
    } else if meta.factory == Some(crate::FactoryGroup::Vehicles) {
        1
    } else if view
        .definitions
        .building(kind)
        .is_some_and(|d| d.power_produced > 0)
    {
        2
    } else if meta.category == crate::Category::Armory {
        8
    } else if meta.factory == Some(crate::FactoryGroup::Aircraft) {
        10
    } else {
        9
    };
    weights[index]
}

fn unit_weight(view: &View<'_>, unit: &UnitSnap, weights: &[u32; 12]) -> u32 {
    let index = if unit.harvest.is_some() {
        Some(5)
    } else {
        match role(view, unit.kind) {
            Role::Engineer => Some(3),
            Role::Spy => Some(4),
            Role::Mcv => Some(6),
            Role::Transport => Some(7),
            _ => None,
        }
    };
    index.map_or(0, |i| weights[i])
}

fn strike_target(
    view: &View<'_>,
    controller: &AiController,
    radius: u32,
    weights: &[u32; 12],
) -> Option<CellPos> {
    let targets: Vec<_> = controller
        .known
        .values()
        .filter(|b| stable(view, b.footprint.origin))
        .map(|b| (b.footprint, weight(view, b.kind, weights)))
        .chain(
            view.enemy_units
                .iter()
                .filter(|u| stable(view, u.mobile.cell))
                .map(|u| (cell_footprint(u.mobile.cell), unit_weight(view, u, weights))),
        )
        .filter(|(_, weight)| *weight > 0)
        .collect();
    targets.iter().map(|(f, _)| f.origin).max_by_key(|&cell| {
        let score: u64 = targets
            .iter()
            .filter(|(f, _)| f.distance_to_cell(cell) <= radius)
            .map(|(_, weight)| u64::from(*weight))
            .sum();
        (score, std::cmp::Reverse(cell))
    })
}

fn strategic(
    view: &View<'_>,
    controller: &AiController,
    special: &crate::SpecialState,
    wave: &[&UnitSnap],
) -> Option<Command> {
    let rules = difficulty(view, controller)?;
    for building in &view.own_buildings {
        let meta = view
            .definitions
            .metadata(ProductionItem::Building(building.kind))?;
        if !meta.strategic
            || (!view.superweapons && meta.role.major_superweapon())
            || (meta.is_powered && view.state.low_power())
            || special.disabled.contains_key(&building.id)
            || special.charge.get(&building.id).copied().unwrap_or(0) < meta.cooldown
        {
            continue;
        }
        let best = controller
            .known
            .values()
            .filter(|b| stable(view, b.footprint.origin))
            .max_by_key(|b| {
                let score: u64 = controller
                    .known
                    .values()
                    .filter(|other| other.footprint.distance(&b.footprint) <= meta.radius)
                    .map(|other| u64::from(weight(view, other.kind, &rules.target_weights)))
                    .sum();
                (score, std::cmp::Reverse(b.footprint.origin))
            });
        match meta.role {
            Role::Nuke | Role::Storm => {
                if meta.role == Role::Storm && special.strategic.storm.is_some() {
                    continue;
                }
                if let Some(target) =
                    strike_target(view, controller, meta.radius, &rules.target_weights)
                {
                    return Some(Command::Superweapon {
                        building: building.id,
                        target,
                        units: Vec::new(),
                    });
                }
            }
            Role::Paradrop => {
                if let Some(target) = best {
                    let base = target.footprint.origin;
                    let offset = view.ai.policy.paradrop_offset as i32;
                    let behind = CellPos::new(
                        base.x
                            .saturating_add((base.x - controller.start.x).signum() * offset),
                        base.y
                            .saturating_add((base.y - controller.start.y).signum() * offset),
                    );
                    if view.grid.contains(behind)
                        && stable(view, behind)
                        && view.grid.passable_for(behind, Domain::Land)
                        && view
                            .grid
                            .room(behind, true, |id| view.known_occupants.contains(&id))
                            > 0
                    {
                        return Some(Command::Superweapon {
                            building: building.id,
                            target: behind,
                            units: Vec::new(),
                        });
                    }
                }
            }
            Role::Curtain | Role::Chronosphere => {
                let source = wave
                    .iter()
                    .filter(|u| {
                        !view.definitions.on_foot(u.kind)
                            && view.definitions.domain(u.kind) != Domain::Air
                    })
                    .max_by_key(|u| {
                        wave.iter()
                            .filter(|v| chebyshev(v.mobile.cell, u.mobile.cell) <= 1)
                            .count()
                    })
                    .map(|u| u.mobile.cell);
                let Some(source) = source else {
                    continue;
                };
                if !stable(view, source)
                    || view.own_units.iter().any(|u| {
                        view.definitions.on_foot(u.kind) && chebyshev(u.mobile.cell, source) <= 1
                    })
                {
                    continue;
                }
                if meta.role == Role::Curtain {
                    return Some(Command::Superweapon {
                        building: building.id,
                        target: source,
                        units: Vec::new(),
                    });
                }
                if let Some(target) = best {
                    let base = target.footprint.origin;
                    for radius in view.ai.policy.chrono_min_offset as i32
                        ..=view.ai.policy.chrono_max_offset as i32
                    {
                        let dest = CellPos::new(
                            base.x
                                .saturating_add((base.x - controller.start.x).signum() * radius),
                            base.y
                                .saturating_add((base.y - controller.start.y).signum() * radius),
                        );
                        let legal = view
                            .own_units
                            .iter()
                            .filter(|u| chebyshev(u.mobile.cell, source) <= 1)
                            .all(|u| {
                                let cell = CellPos::new(
                                    dest.x.saturating_add(u.mobile.cell.x - source.x),
                                    dest.y.saturating_add(u.mobile.cell.y - source.y),
                                );
                                stable(view, cell)
                                    && view
                                        .grid
                                        .passable_for(cell, view.definitions.domain(u.kind))
                                    && view.grid.building(cell).is_none()
                                    && view.grid.room_in(
                                        cell,
                                        view.definitions.domain(u.kind),
                                        view.definitions.on_foot(u.kind),
                                        |id| view.known_occupants.contains(&id),
                                    ) > 0
                            });
                        if legal {
                            return Some(Command::Chronoshift {
                                building: building.id,
                                source,
                                target: dest,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}
