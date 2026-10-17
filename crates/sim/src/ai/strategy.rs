//! RA8 capabilities use the same snapshots, honest memory and ordinary commands as the core AI.
use super::*;
use std::collections::VecDeque;

pub(super) fn attack_job_eligible(controller: &AiController, target: SimId) -> bool {
    controller
        .building_jobs
        .get(&target)
        .is_none_or(|job| job.failures < MAX_TARGET_FAILURES || job.approaching)
}

fn stowed_own_attacker(view: &View<'_>, special: &crate::SpecialState, id: SimId) -> bool {
    view.own_buildings.iter().any(|b| b.id == id)
        || special.cargo.iter().any(|(host, passengers)| {
            passengers.iter().any(|p| p.id == id)
                && (view.owned_units.iter().any(|u| u.id == *host)
                    || view.own_buildings.iter().any(|b| b.id == *host)
                    || special
                        .garrisons
                        .get(host)
                        .is_some_and(|g| g.owner == view.player))
        })
        || special
            .parasites
            .values()
            .any(|p| p.owner == view.player && p.passenger.id == id)
}

/// Complete only actual own loss/capture or a source-ammunition cycle, never slow travel
/// or temporary removal. Repairs cannot turn the same repeated HP low into new progress.
pub(super) fn observe_attack_jobs(
    view: &View<'_>,
    controller: &mut AiController,
    special: &crate::SpecialState,
) {
    if !view.ai.rules_difficulty {
        return;
    }
    controller
        .building_jobs
        .retain(|id, _| controller.known.contains_key(id));
    for (&id, job) in &mut controller.building_jobs {
        let Some(target) = view.enemy_buildings.iter().find(|b| b.id == id) else {
            continue;
        };
        let mut completed = false;
        job.actors.retain(|actor, old_ammo| {
            let Some(unit) = view.owned_units.iter().find(|u| u.id == *actor) else {
                if stowed_own_attacker(view, special, *actor) { return true; }
                completed = true;
                return false;
            };
            let ammo = view.definitions.metadata(ProductionItem::Unit(unit.kind)).map_or(0, |m| m.ammo);
            if *old_ammo > 0 && special.ammo_of(*actor, ammo) == 0 {
                completed = true;
                return false;
            }
            if !view.own_units.iter().any(|active| active.id == *actor) { return true; }
            matches!(unit.attack_order, Some(crate::AttackOrder::Attack { target, .. }) if target == id)
        });
        job.founders.retain(|actor| job.actors.contains_key(actor));
        if target.hp < job.hp {
            job.hp = target.hp;
            job.failures = 0;
            job.founders = job.actors.keys().copied().collect();
        } else if completed {
            job.failures = job.failures.saturating_add(1).min(MAX_TARGET_FAILURES);
        }
        // The original cohort's healthy path/freeze can finish. Replacing casualties
        // alone never installs a fresh exemption for an already failed job.
        job.approaching = job.founders.iter().any(|actor| {
            view.frozen_army.contains(actor) || view.own_units.iter().any(|u| {
                u.id == *actor && u.has_order
                    && matches!(u.attack_order, Some(crate::AttackOrder::Attack { target, .. }) if target == id)
            })
        });
    }
}

pub(super) fn record_attack_job(
    view: &View<'_>,
    controller: &mut AiController,
    special: &crate::SpecialState,
    command: &Command,
) {
    if !view.ai.rules_difficulty {
        return;
    }
    let Command::Attack { units, target } = command else {
        return;
    };
    let Some(building) = view.enemy_buildings.iter().find(|b| b.id == *target) else {
        return;
    };
    let Some(terms) = view.definitions.building_combat(building.kind) else {
        return;
    };
    let mut traits = crate::combat::traits_of(
        view.grid,
        building.id,
        terms.armour,
        &crate::combat::Body::Building(building.footprint),
        Some(&terms),
    );
    if let Some(meta) = view
        .definitions
        .metadata(ProductionItem::Building(building.kind))
    {
        traits.bombable = meta.bombable;
        traits.immune_psionics |= meta.immune_psionics;
        traits.immune_radiation = meta.immune_radiation;
    }
    let admitted: Vec<_> = view
        .own_units
        .iter()
        .filter(|u| units.contains(&u.id))
        .filter_map(|u| {
            let terms = view.definitions.unit_combat(u.kind)?;
            let (weapon, percent) =
                crate::weapon_against(view.definitions, &terms, terms.weapons(false), &traits)?;
            if percent <= 1
                || view
                    .definitions
                    .weapon(weapon)
                    .is_none_or(|w| w.damage <= 0)
            {
                return None;
            }
            let ammo = view
                .definitions
                .metadata(ProductionItem::Unit(u.kind))
                .map_or(0, |m| m.ammo);
            let remaining = special.ammo_of(u.id, ammo);
            (ammo == 0 || remaining > 0).then_some((u.id, remaining))
        })
        .collect();
    if admitted.is_empty() {
        return;
    }
    let job = controller
        .building_jobs
        .entry(*target)
        .or_insert_with(|| BuildingJobProgress {
            hp: building.hp,
            failures: 0,
            actors: BTreeMap::new(),
            founders: BTreeSet::new(),
            approaching: false,
        });
    for (id, ammo) in admitted {
        job.actors.entry(id).or_insert(ammo);
        if job.failures == 0 {
            job.founders.insert(id);
        }
    }
}

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
    if !service_income(view) {
        for unit in &view.recoverable_service {
            if service_cost(view, unit).is_some_and(|cost| view.state.credits() < cost) {
                commands.push(Command::Move {
                    units: vec![unit.id],
                    target: if view.definitions.domain(unit.kind) == Domain::Naval {
                        unit.mobile.cell
                    } else {
                        controller.gather
                    },
                });
            }
        }
    }
    // One ordinary repair toggle per decision. Futile paid repairs must not keep
    // changing a helpless player's credits and restarting its last-stand patience.
    let repair_recovery = repair_recovery(view, controller);
    if !repair_recovery {
        if let Some(building) = view
            .own_buildings
            .iter()
            .find(|b| view.state.repairing().contains(&b.id))
        {
            commands.push(Command::RepairBuilding {
                building: building.id,
            });
        }
    } else if let Some(building) = view.own_buildings.iter().find(|b| {
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
        if (!service_income(view)
            && service_cost(view, unit).is_some_and(|cost| view.state.credits() < cost))
            || meta.infantry
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
    let mut ground_wave: Vec<_> = view
        .own_units
        .iter()
        .filter(|u| {
            controller.wave.contains(&u.id)
                && view.definitions.domain(u.kind) != Domain::Naval
                && !controller.reserved.contains(&u.id)
        })
        .copied()
        .collect();
    // Service/specialist reservations above exclude unavailable actors. An active
    // combat clearance job below reserves against conflicting tactical orders only;
    // its actual wave remains eligible for ordinary Curtain/Chronosphere support.
    let combat_support = strategic(view, controller, special, &ground_wave);
    screen_strike_aircraft(view, controller, special, &ground_wave, &mut commands);
    ground_wave.retain(|unit| !controller.reserved.contains(&unit.id));
    if let Some(target) = controller
        .known
        .iter()
        .filter(|(id, _)| attack_job_eligible(controller, **id))
        .filter(|(_, b)| stable(view, b.footprint.origin))
        .min_by_key(|(id, b)| (b.footprint.distance_to_cell(controller.start), **id))
    {
        // Dock-bound strike aircraft keep their job even in an escorted wave.
        // The other actors retain their original combat/standoff policy below.
        let matching_attack = |unit: &UnitSnap| {
            matches!(unit.attack_order,
            Some(crate::AttackOrder::Attack { target: current, .. }) if current == *target.0)
        };
        let strike_wave: Vec<_> = ground_wave
            .iter()
            .copied()
            .filter(|unit| {
                view.definitions
                    .metadata(ProductionItem::Unit(unit.kind))
                    .is_some_and(|m| m.airport_bound)
            })
            .collect();
        let interrupted = strike_wave.iter().any(|unit| local_wave_threat(view, unit));
        if !interrupted {
            // This job also owns its preserved healthy Attack: suppress contradictory
            // core scouting/gather orders without resetting the existing chase.
            controller
                .reserved
                .extend(strike_wave.iter().filter(|u| view.fights(u)).map(|u| u.id));
            let units: Vec<_> = strike_wave
                .iter()
                .filter(|u| view.fights(u) && !matching_attack(u))
                .map(|u| u.id)
                .collect();
            if !units.is_empty() {
                commands.push(Command::Attack {
                    units,
                    target: *target.0,
                });
            }
        } else {
            // If a base-focused approach meets a real threat, ordinary AttackMove
            // resumes en-route acquisition. Other healthy orders keep their history.
            controller.reserved.extend(strike_wave.iter().filter(|unit| {
                    matching_attack(unit) || matches!(unit.attack_order,
                        Some(crate::AttackOrder::AttackMove { destination, .. }) if destination == target.1.footprint.origin)
                }).map(|unit| unit.id));
            let units: Vec<_> = strike_wave
                .iter()
                .filter(|unit| matching_attack(unit))
                .map(|unit| unit.id)
                .collect();
            if !units.is_empty() {
                commands.push(Command::AttackMove {
                    units,
                    target: target.1.footprint.origin,
                });
            }
        }
        // Keep the original policy for the surface/undocked-air part of a mixed
        // wave; a Dog keeps its own mission and artillery keeps its tank line.
        if !view
            .enemy_units
            .iter()
            .any(|u| u.armed && u.harvest.is_none())
        {
            let units: Vec<_> = ground_wave
                .iter()
                .filter(|u| {
                    view.fights(u)
                        && !view
                            .definitions
                            .metadata(ProductionItem::Unit(u.kind))
                            .is_some_and(|m| m.airport_bound)
                })
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
    if let Some(command) = combat_support {
        commands.push(command);
    }
    commands
}

/// A dock-bound strike plane cannot trade through an observed longer-range AA screen
/// indefinitely. Let eligible ground wave members clear that screen first; the existing
/// target failure/arrival policy still handles unreachable jobs and ordinary movement.
fn screen_strike_aircraft(
    view: &View<'_>,
    controller: &mut AiController,
    special: &crate::SpecialState,
    wave: &[&UnitSnap],
    commands: &mut Vec<Command>,
) {
    let Some(objective) = controller.known.values().min_by_key(|b| {
        (
            b.footprint.distance_to_cell(controller.start),
            b.footprint.origin,
        )
    }) else {
        return;
    };
    let aircraft: Vec<_> = wave
        .iter()
        .copied()
        .filter(|u| {
            view.definitions
                .metadata(ProductionItem::Unit(u.kind))
                .is_some_and(|m| m.airport_bound)
        })
        .collect();
    if aircraft.is_empty() {
        return;
    }
    let barrier = view
        .enemy_buildings
        .iter()
        .filter(|b| {
            stable(view, b.footprint.origin)
                && only_aa_building(view.definitions, b.kind)
                && controller.failures.get(&b.id).copied().unwrap_or(0) < MAX_TARGET_FAILURES
        })
        .filter_map(|b| {
            let terms = view.definitions.building_combat(b.kind)?;
            let traits = crate::TargetTraits {
                armour: terms.armour,
                building: true,
                immune_psionics: true,
                wall: terms.wall,
                bombable: view
                    .definitions
                    .metadata(ProductionItem::Building(b.kind))
                    .is_some_and(|m| m.bombable),
                ..crate::TargetTraits::default()
            };
            let threatened: Vec<_> = aircraft
                .iter()
                .copied()
                .filter(|aircraft| {
                    let Some(air) = view.definitions.unit_combat(aircraft.kind) else {
                        return false;
                    };
                    let air_traits = crate::TargetTraits {
                        armour: air.armour,
                        airborne: true,
                        immune_psionics: true,
                        organic: air.organic,
                        ..crate::TargetTraits::default()
                    };
                    let Some((anti_air, percent)) = crate::weapon_against(
                        view.definitions,
                        &terms,
                        terms.weapons(false),
                        &air_traits,
                    ) else {
                        return false;
                    };
                    let Some(anti_air) = view
                        .definitions
                        .weapon(anti_air)
                        .filter(|w| w.damage > 0 && percent > 1)
                    else {
                        return false;
                    };
                    let Some((strike, percent)) =
                        crate::weapon_against(view.definitions, &air, air.weapons(false), &traits)
                    else {
                        return false;
                    };
                    let Some(strike) = view
                        .definitions
                        .weapon(strike)
                        .filter(|w| w.damage > 0 && percent > 1)
                    else {
                        return false;
                    };
                    let reach = anti_air.range.saturating_add(terms.air_range_bonus);
                    reach > strike.range
                        && within_reach(
                            b.footprint,
                            objective.footprint.origin,
                            reach.div_ceil(crate::SUBCELLS_PER_CELL as u32),
                        )
                        && !within_reach(
                            b.footprint,
                            controller.gather,
                            reach.div_ceil(crate::SUBCELLS_PER_CELL as u32),
                        )
                })
                .collect();
            if threatened.is_empty() {
                return None;
            }
            let ground: Vec<_> = wave
                .iter()
                .copied()
                .filter(|u| {
                    view.definitions.domain(u.kind) == Domain::Land
                        && view.fights(u)
                        && view.definitions.unit_combat(u.kind).is_some_and(|t| {
                            crate::weapon_against(view.definitions, &t, t.weapons(false), &traits)
                                .is_some_and(|(k, p)| {
                                    p > 1
                                        && view.definitions.weapon(k).is_some_and(|w| w.damage > 0)
                                })
                        })
                })
                .map(|u| u.id)
                .collect();
            (!ground.is_empty()).then_some((b, threatened, ground))
        })
        .min_by_key(|(b, _, _)| (b.footprint.distance_to_cell(controller.start), b.id));
    let Some((barrier, aircraft, ground)) = barrier else {
        return;
    };
    let Some(barrier_terms) = view.definitions.building_combat(barrier.kind) else {
        return;
    };
    let traits = crate::TargetTraits {
        armour: barrier_terms.armour,
        building: true,
        immune_psionics: true,
        wall: barrier_terms.wall,
        bombable: view
            .definitions
            .metadata(ProductionItem::Building(barrier.kind))
            .is_some_and(|m| m.bombable),
        ..crate::TargetTraits::default()
    };
    let target_extent = crate::combat::Extent::footprint(barrier.footprint);
    let mut remaining = u32::MAX;
    let mut distance = u32::MAX;
    for unit in wave.iter().copied().filter(|u| ground.contains(&u.id)) {
        distance = distance.min(barrier.footprint.distance_to_cell(unit.mobile.cell));
        let Some(terms) = view.definitions.unit_combat(unit.kind) else {
            continue;
        };
        let Some((weapon, _)) =
            crate::weapon_against(view.definitions, &terms, terms.weapons(false), &traits)
        else {
            continue;
        };
        let Some(weapon) = view.definitions.weapon(weapon) else {
            continue;
        };
        let covers = |position| {
            let (dx, dy) = crate::combat::Extent::point(position).gap(&target_extent);
            crate::in_range(dx, dy, weapon.range)
                && !crate::inside_min_range(dx, dy, weapon.min_range)
        };
        if covers(unit.mobile.position(view.grid, unit.id)) {
            remaining = 0;
        } else if let Some((goal, best)) = unit.move_progress
            && covers(crate::SubPos::centre_of(goal))
        {
            remaining = remaining.min(best);
        }
    }
    let progress = controller
        .air_screens
        .entry(barrier.id)
        .or_insert(AirScreenProgress {
            remaining,
            distance,
            hp: barrier.hp,
            last_progress: view.tick,
        });
    progress.advance(remaining, distance, barrier.hp, view.tick);
    // The deadline belongs to this observed target, not to the current recruits.
    // A failed job stays released until actual closer approach or damage improves it.
    if view.tick.saturating_sub(progress.last_progress)
        >= u64::from(controller.difficulty.wave_patience)
    {
        return;
    }
    let mut focus = Vec::new();
    let mut approach = Vec::new();
    let mut intercepts: BTreeMap<SimId, Vec<SimId>> = BTreeMap::new();
    let mut ground_stalled = wave
        .iter()
        .filter(|u| ground.contains(&u.id))
        .all(|u| u.idle() && u.combat_target.is_none());
    for unit in wave.iter().copied().filter(|u| ground.contains(&u.id)) {
        let Some(terms) = view.definitions.unit_combat(unit.kind) else {
            continue;
        };
        let own_traits = screen_unit_traits(view, unit, &terms);
        let from = crate::combat::Extent::point(unit.mobile.position(view.grid, unit.id));
        let threat = view
            .enemy_units
            .iter()
            .filter_map(|enemy| {
                let enemy_terms = view.definitions.unit_combat(enemy.kind)?;
                let (hostile_weapon, percent) = crate::weapon_against(
                    view.definitions,
                    &enemy_terms,
                    enemy_terms.weapons(false),
                    &own_traits,
                )?;
                if percent <= 1 || view.definitions.weapon(hostile_weapon)?.damage <= 0 {
                    return None;
                }
                let enemy_traits = screen_unit_traits(view, enemy, &enemy_terms);
                let (weapon, percent) = crate::weapon_against(
                    view.definitions,
                    &terms,
                    terms.weapons(false),
                    &enemy_traits,
                )?;
                let weapon = view.definitions.weapon(weapon)?;
                let (dx, dy) = from.gap(&crate::combat::Extent::point(
                    enemy.mobile.position(view.grid, enemy.id),
                ));
                let reach =
                    weapon
                        .range
                        .saturating_add(if enemy_traits.airborne && weapon.anti_air {
                            terms.air_range_bonus
                        } else {
                            0
                        });
                (percent > 1
                    && weapon.damage > 0
                    && crate::in_range(dx, dy, reach)
                    && !crate::inside_min_range(dx, dy, weapon.min_range))
                .then_some((
                    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)),
                    enemy.id,
                ))
            })
            .min();
        if let Some((_, target)) = threat {
            ground_stalled = false;
            let matching = matches!(unit.attack_order, Some(crate::AttackOrder::Attack { target: current, .. }) if current == target)
                || (matches!(
                    unit.attack_order,
                    Some(crate::AttackOrder::AttackMove { .. })
                ) && unit.combat_target == Some(target));
            if !matching {
                intercepts.entry(target).or_default().push(unit.id);
            }
            continue;
        }
        if matches!(unit.attack_order, Some(crate::AttackOrder::Attack { target, .. }) if target == barrier.id)
        {
            // Reissuing Attack resets the chase/path failure history every decision.
            continue;
        }
        let Some((weapon, _)) =
            crate::weapon_against(view.definitions, &terms, terms.weapons(false), &traits)
        else {
            continue;
        };
        let Some(weapon) = view.definitions.weapon(weapon) else {
            continue;
        };
        let covers = |position| {
            let (dx, dy) = crate::combat::Extent::point(position).gap(&target_extent);
            crate::in_range(dx, dy, weapon.range)
                && !crate::inside_min_range(dx, dy, weapon.min_range)
        };
        ground_stalled &= !covers(unit.mobile.position(view.grid, unit.id));
        let incidental_safe_building = unit.combat_target.is_some_and(|target| {
            view.enemy_buildings.iter().any(|building| {
                building.id == target
                    && building.economy
                    && only_aa_building(view.definitions, building.kind)
            })
        });
        if covers(unit.mobile.position(view.grid, unit.id)) || incidental_safe_building {
            // An ordinary approach may stop for an unarmed economy building before
            // reaching the AA. Keep the clearance objective, while ordinary combat
            // against intervening ground threats is allowed to finish.
            focus.push(unit.id);
        } else if !matches!(unit.attack_order, Some(crate::AttackOrder::AttackMove { destination, .. })
            if covers(crate::SubPos::centre_of(destination)))
        {
            // Travel through ordinary combat rather than walking past observed enemies.
            // A matching approach keeps its movement progress and bounded failure state.
            approach.push(unit.id);
        }
    }
    // A returning/holding plane is not progress by the ground clearance team. The
    // core already counts failed all-idle waves; count the same bounded failure when
    // only those planes keep it active. Own movement/combat outcomes supply the fact,
    // without probing unseen blockers or delaying legitimate movement/firing.
    if ground_stalled && wave.iter().any(|u| !u.idle()) {
        let failures = controller.failures.entry(barrier.id).or_default();
        *failures = failures.saturating_add(1);
        if *failures >= MAX_TARGET_FAILURES {
            return;
        }
    }
    controller.target = Some(WaveTarget::Building(barrier.id));
    controller.reserved.extend(ground.iter().copied());
    commands.extend(
        intercepts
            .into_iter()
            .map(|(target, units)| Command::Attack { units, target }),
    );
    if !focus.is_empty() {
        commands.push(Command::Attack {
            units: focus,
            target: barrier.id,
        });
    }
    if !approach.is_empty() {
        commands.push(Command::AttackMove {
            units: approach,
            target: barrier.footprint.origin,
        });
    }
    let mut waiting = BTreeSet::new();
    for unit in aircraft {
        controller.reserved.insert(unit.id);
        let ammo = view
            .definitions
            .metadata(ProductionItem::Unit(unit.kind))
            .map_or(0, |m| m.ammo);
        // Empty planes keep the actual dock return/reload cycle. Loaded aircraft wait
        // outside known source-AA reach near this job, rather than crossing the map home.
        if special.ammo_of(unit.id, ammo) == 0 {
            continue;
        }
        let Some(target) = air_wait_cell(view, controller, unit, barrier, &waiting) else {
            continue;
        };
        waiting.insert(target);
        if unit.attacking
            || (unit.move_progress.map(|(goal, _)| goal) != Some(target)
                && (unit.mobile.cell != target || unit.mobile.step.is_some()))
        {
            commands.push(Command::Move {
                units: vec![unit.id],
                target,
            });
        }
    }
}

fn air_wait_cell(
    view: &View<'_>,
    controller: &AiController,
    unit: &UnitSnap,
    barrier: &BuildingSnap,
    waiting: &BTreeSet<CellPos>,
) -> Option<CellPos> {
    let terms = view.definitions.unit_combat(unit.kind)?;
    let traits = screen_unit_traits(view, unit, &terms);
    let reach = |enemy: &crate::CombatTerms| {
        let (kind, percent) =
            crate::weapon_against(view.definitions, enemy, enemy.weapons(false), &traits)?;
        let weapon = view.definitions.weapon(kind)?;
        (percent > 1 && weapon.damage > 0).then_some((
            weapon.range.saturating_add(enemy.air_range_bonus),
            weapon.min_range,
        ))
    };
    let (range, _) = reach(&view.definitions.building_combat(barrier.kind)?)?;
    // One cell beyond the rounded-up source envelope. This is geometry, not a policy radius.
    let radius = i64::from(range.div_ceil(crate::SUBCELLS_PER_CELL as u32)) + 1;
    let x0 = i64::from(barrier.footprint.origin.x) - radius;
    let x1 =
        i64::from(barrier.footprint.origin.x) + i64::from(barrier.footprint.width) - 1 + radius;
    let y0 = i64::from(barrier.footprint.origin.y) - radius;
    let y1 =
        i64::from(barrier.footprint.origin.y) + i64::from(barrier.footprint.height) - 1 + radius;
    let safe = |cell: CellPos| {
        let point = crate::combat::Extent::point(crate::SubPos::centre_of(cell));
        let threatened = |extent: crate::combat::Extent, enemy: &crate::CombatTerms| {
            reach(enemy).is_some_and(|(range, minimum)| {
                let (dx, dy) = extent.gap(&point);
                crate::in_range(dx, dy, range) && !crate::inside_min_range(dx, dy, minimum)
            })
        };
        !controller.known.values().any(|b| {
            view.definitions
                .building_combat(b.kind)
                .is_some_and(|terms| {
                    threatened(crate::combat::Extent::footprint(b.footprint), &terms)
                })
        }) && !view.enemy_units.iter().any(|u| {
            view.definitions.unit_combat(u.kind).is_some_and(|terms| {
                threatened(
                    crate::combat::Extent::point(u.mobile.position(view.grid, u.id)),
                    &terms,
                )
            })
        })
    };
    let available = |cell| {
        view.vision.is_explored(cell)
            && view.grid.passable_for(cell, Domain::Air)
            && !waiting.contains(&cell)
            && view.grid.room_in(cell, Domain::Air, false, |id| {
                id != unit.id && view.known_occupants.contains(&id)
            }) > 0
            && safe(cell)
    };
    let perimeter = |cell: CellPos| {
        let (x, y) = (i64::from(cell.x), i64::from(cell.y));
        (x == x0 || x == x1) && (y0..=y1).contains(&y)
            || (y == y0 || y == y1) && (x0..=x1).contains(&x)
    };
    if let Some((goal, _)) = unit.move_progress
        && perimeter(goal)
        && available(goal)
    {
        return Some(goal);
    }
    if perimeter(unit.mobile.cell) && available(unit.mobile.cell) {
        return Some(unit.mobile.cell);
    }
    let mut cells = BTreeSet::new();
    for y in y0.max(0)..=y1.min(i64::from(view.grid.height()) - 1) {
        for x in [x0, x1] {
            if x >= 0 && x < i64::from(view.grid.width()) {
                cells.insert(CellPos::new(x as i32, y as i32));
            }
        }
    }
    for x in x0.max(0)..=x1.min(i64::from(view.grid.width()) - 1) {
        for y in [y0, y1] {
            if y >= 0 && y < i64::from(view.grid.height()) {
                cells.insert(CellPos::new(x as i32, y as i32));
            }
        }
    }
    cells
        .into_iter()
        .filter(|&cell| available(cell))
        .min_by_key(|&cell| (octile_distance(unit.mobile.cell, cell), cell.y, cell.x))
        .or_else(|| available(unit.mobile.cell).then_some(unit.mobile.cell))
        .or_else(|| available(controller.gather).then_some(controller.gather))
}

fn service_income(view: &View<'_>) -> bool {
    let refinery = view.own_kinds.iter().any(|&kind| {
        view.definitions
            .building(kind)
            .is_some_and(|b| b.credits_per_ore.is_some())
    });
    (refinery
        && view
            .own_units
            .iter()
            .any(|u| u.harvest.is_some_and(|h| h != HarvestState::Idle)))
        || view.own_buildings.iter().any(|b| {
            view.definitions
                .metadata(ProductionItem::Building(b.kind))
                .is_some_and(|m| m.cash_amount > 0)
        })
}

fn repair_recovery(view: &View<'_>, controller: &AiController) -> bool {
    service_income(view)
        || view.owned_units.iter().any(|unit| {
            can_finish(view, controller, unit)
                || role(view, unit.kind) == Role::Mcv
                || (role(view, unit.kind) == Role::Engineer && !view.seen_tech.is_empty())
        })
        || !view.recoverable_garrisons.is_empty()
        || view.own_buildings.iter().any(|building| {
            view.definitions
                .metadata(ProductionItem::Building(building.kind))
                .is_some_and(|meta| meta.role.superweapon())
        })
        // These are the ordinary prerequisite/producer/owner checks, independent
        // of cash and temporary queue capacity. A recoverable base keeps repairs.
        || view.definitions.buildings().iter().enumerate().any(|(i, _)| {
            u16::try_from(i)
                .ok()
                .is_some_and(|i| view.buildable(ProductionItem::Building(BuildingKind(i))))
        })
        || view
            .ai
            .army
            .iter()
            .any(|&kind| view.buildable(ProductionItem::Unit(kind)))
}

fn service_cost(view: &View<'_>, unit: &UnitSnap) -> Option<i64> {
    let rules = &view.definitions.rules()?.economy;
    let cost = view
        .definitions
        .production(ProductionItem::Unit(unit.kind))?
        .cost;
    let maximum = view.definitions.unit_combat(unit.kind)?.hit_points;
    Some(crate::repair_step_cost(
        rules,
        cost,
        maximum,
        view.state.super_me,
    ))
}

fn screen_unit_traits(
    view: &View<'_>,
    unit: &UnitSnap,
    terms: &crate::CombatTerms,
) -> crate::TargetTraits {
    let mut traits = crate::combat::traits_of(
        view.grid,
        unit.id,
        terms.armour,
        &crate::combat::Body::Unit {
            cell: unit.mobile.cell,
            next: unit.mobile.step.map(|step| step.to),
        },
        Some(terms),
    );
    if let Some(meta) = view.definitions.metadata(ProductionItem::Unit(unit.kind)) {
        traits.immune_psionics |= meta.immune_psionics || meta.role == Role::MindControl;
    }
    traits
}

fn local_wave_threat(view: &View<'_>, unit: &UnitSnap) -> bool {
    let Some(terms) = view.definitions.unit_combat(unit.kind) else {
        return false;
    };
    let unit_traits = |unit: &UnitSnap, terms: &crate::CombatTerms| {
        let mut traits = screen_unit_traits(view, unit, terms);
        if let Some(meta) = view.definitions.metadata(ProductionItem::Unit(unit.kind)) {
            traits.bombable = meta.bombable;
            traits.immune_radiation = meta.immune_radiation;
        }
        traits
    };
    let traits = unit_traits(unit, &terms);
    let from = crate::combat::Extent::point(unit.mobile.position(view.grid, unit.id));
    view.enemy_units.iter().any(|enemy| {
        let Some(hostile) = view.definitions.unit_combat(enemy.kind) else {
            return false;
        };
        let Some((weapon, percent)) =
            crate::weapon_against(view.definitions, &hostile, hostile.weapons(false), &traits)
        else {
            return false;
        };
        let Some(weapon) = view
            .definitions
            .weapon(weapon)
            .filter(|w| percent > 1 && w.damage > 0)
        else {
            return false;
        };
        let (dx, dy) = from.gap(&crate::combat::Extent::point(
            enemy.mobile.position(view.grid, enemy.id),
        ));
        let hostile_range = weapon
            .range
            .saturating_add(if traits.airborne && weapon.anti_air {
                hostile.air_range_bonus
            } else {
                0
            });
        if crate::in_range(dx, dy, hostile_range)
            && !crate::inside_min_range(dx, dy, weapon.min_range)
        {
            return true;
        }
        let enemy_traits = unit_traits(enemy, &hostile);
        crate::weapon_against(
            view.definitions,
            &terms,
            terms.weapons(false),
            &enemy_traits,
        )
        .and_then(|(kind, percent)| {
            view.definitions
                .weapon(kind)
                .filter(|w| percent > 1 && w.damage > 0)
        })
        .is_some_and(|weapon| {
            let reach = weapon
                .range
                .saturating_add(if enemy_traits.airborne && weapon.anti_air {
                    terms.air_range_bonus
                } else {
                    0
                });
            crate::in_range(dx, dy, reach) && !crate::inside_min_range(dx, dy, weapon.min_range)
        })
    })
}

/// An observed base threat may reserve a defender only when its actual weapon can
/// reach a firing position through public terrain. Occupancy is deliberately absent:
/// ordinary Attack handles traffic and the physical, bounded chase at application.
pub(super) fn defence_reachable(view: &View<'_>, unit: &UnitSnap, target: SimId) -> bool {
    let Some(terms) = view.definitions.unit_combat(unit.kind) else {
        return false;
    };
    let target = if let Some(enemy) = view.enemy_units.iter().find(|u| u.id == target) {
        let Some(enemy_terms) = view.definitions.unit_combat(enemy.kind) else {
            return false;
        };
        let mut traits = screen_unit_traits(view, enemy, &enemy_terms);
        if let Some(meta) = view.definitions.metadata(ProductionItem::Unit(enemy.kind)) {
            traits.bombable = meta.bombable;
            traits.immune_radiation = meta.immune_radiation;
        }
        (
            traits,
            crate::combat::Extent::point(enemy.mobile.position(view.grid, enemy.id)),
        )
    } else if let Some(enemy) = view.enemy_buildings.iter().find(|b| b.id == target) {
        let Some(enemy_terms) = view.definitions.building_combat(enemy.kind) else {
            return false;
        };
        let mut traits = crate::combat::traits_of(
            view.grid,
            enemy.id,
            enemy_terms.armour,
            &crate::combat::Body::Building(enemy.footprint),
            Some(&enemy_terms),
        );
        if let Some(meta) = view
            .definitions
            .metadata(ProductionItem::Building(enemy.kind))
        {
            traits.bombable = meta.bombable;
            traits.immune_psionics |= meta.immune_psionics;
            traits.immune_radiation = meta.immune_radiation;
        }
        (traits, crate::combat::Extent::footprint(enemy.footprint))
    } else {
        return false;
    };
    let Some((weapon, percent)) =
        crate::weapon_against(view.definitions, &terms, terms.weapons(false), &target.0)
    else {
        return false;
    };
    let Some(weapon) = view
        .definitions
        .weapon(weapon)
        .filter(|w| percent > 1 && w.damage > 0)
    else {
        return false;
    };
    let fires_from = |point, range| {
        let (dx, dy) = crate::combat::Extent::point(point).gap(&target.1);
        crate::in_range(dx, dy, range) && !crate::inside_min_range(dx, dy, weapon.min_range)
    };
    // Actual current-position shots include AirRangeBonus. Ordinary Attack deliberately
    // seeks a plain-range chase destination, which is still the search below.
    let shot_range = weapon
        .range
        .saturating_add(if target.0.airborne && weapon.anti_air {
            terms.air_range_bonus
        } else {
            0
        });
    if fires_from(unit.mobile.position(view.grid, unit.id), shot_range) {
        return true;
    }
    let covers = |point| fires_from(point, weapon.range);
    let on_foot = view.grid.is_on_foot(unit.id);
    let mut seen = BTreeSet::from([unit.mobile.cell]);
    let mut open = VecDeque::from([unit.mobile.cell]);
    while let Some(cell) = open.pop_front() {
        if if on_foot {
            (0..crate::SPOTS_PER_CELL as u8).all(|spot| covers(crate::SubPos::spot_of(cell, spot)))
        } else {
            covers(crate::SubPos::centre_of(cell))
        } {
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
        .iter()
        .filter(|(id, b)| attack_job_eligible(controller, **id) && stable(view, b.footprint.origin))
        .map(|(_, b)| (b.footprint, weight(view, b.kind, weights)))
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

// The source freeze prevents an arriving vehicle from immediately defending itself.
// Plan against remembered building and currently observed unit fire only. Public
// Chronoshift retains its ordinary permissive geometry and ownership validation.
fn chrono_landing_safe(
    view: &View<'_>,
    controller: &AiController,
    unit: &UnitSnap,
    cell: CellPos,
) -> bool {
    let Some(terms) = view.definitions.unit_combat(unit.kind) else {
        return false;
    };
    let mut traits = crate::combat::traits_of(
        view.grid,
        unit.id,
        terms.armour,
        &crate::combat::Body::Unit { cell, next: None },
        Some(&terms),
    );
    if let Some(meta) = view.definitions.metadata(ProductionItem::Unit(unit.kind)) {
        traits.immune_psionics |= meta.immune_psionics || meta.role == Role::MindControl;
    }
    let to = crate::combat::Extent::point(crate::SubPos::centre_of(cell));
    let threatens = |enemy: &crate::CombatTerms, from: crate::combat::Extent| {
        let Some((weapon, percent)) =
            crate::weapon_against(view.definitions, enemy, enemy.weapons(false), &traits)
        else {
            return false;
        };
        let Some(weapon) = view.definitions.weapon(weapon) else {
            return false;
        };
        let (dx, dy) = from.gap(&to);
        percent > 1
            && weapon.damage > 0
            && crate::in_range(
                dx,
                dy,
                weapon.range.saturating_add(if traits.airborne {
                    enemy.air_range_bonus
                } else {
                    0
                }),
            )
            && !crate::inside_min_range(dx, dy, weapon.min_range)
    };
    !controller.known.values().any(|b| {
        view.definitions
            .building_combat(b.kind)
            .is_some_and(|t| threatens(&t, crate::combat::Extent::footprint(b.footprint)))
    }) && !view.enemy_units.iter().any(|u| {
        view.definitions.unit_combat(u.kind).is_some_and(|t| {
            threatens(
                &t,
                crate::combat::Extent::point(u.mobile.position(view.grid, u.id)),
            )
        })
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
            || special.erasures.contains_key(&building.id)
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
                let group: Vec<_> = view
                    .owned_units
                    .iter()
                    .copied()
                    .filter(|u| chebyshev(u.mobile.cell, source) <= 1)
                    .collect();
                if !stable(view, source)
                    || group.iter().any(|u| {
                        view.definitions.on_foot(u.kind)
                            || (meta.role == Role::Chronosphere
                                && (view.definitions.domain(u.kind) == Domain::Air
                                    || !view.own_units.iter().any(|active| active.id == u.id)))
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
                        let legal = group.iter().all(|u| {
                            let cell = CellPos::new(
                                dest.x.saturating_add(u.mobile.cell.x - source.x),
                                dest.y.saturating_add(u.mobile.cell.y - source.y),
                            );
                            stable(view, cell)
                                && view
                                    .grid
                                    .passable_for(cell, view.definitions.domain(u.kind))
                                && view.grid.building(cell).is_none()
                                && chrono_landing_safe(view, controller, u, cell)
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
