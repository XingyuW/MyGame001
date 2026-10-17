//! Actual scheduled attacker lifecycles for the observed building-job history.
use super::*;
use bevy_app::App;
use bevy_ecs::{entity::Entity, message::Messages};

fn app() -> App {
    let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let definitions = crate::load_ruleset(&data).unwrap();
    let mut skirmish = crate::load_skirmish(data.join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(38, 38)];
    skirmish.starting_credits = 0;
    let ai = crate::load_ai(data.join("ai.ron"), &definitions).unwrap();
    let setups = ["germany", "russia"].map(|country| crate::PlayerSetup {
        player: PlayerId(u8::from(country == "russia")),
        faction: skirmish
            .factions
            .iter()
            .position(|f| f.id == country)
            .unwrap(),
        start: usize::from(country == "russia"),
        controller: if country == "russia" {
            crate::Controller::Human
        } else {
            crate::Controller::Ai {
                difficulty: ai
                    .difficulties
                    .iter()
                    .position(|d| d.id == "brutal")
                    .unwrap(),
            }
        },
    });
    let mut app = App::new();
    app.add_plugins(crate::SimPlugin { seed: 133 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                crate::Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0,
                };
                48 * 48
            ],
        )
        .unwrap(),
    );
    app.world_mut().insert_resource(definitions);
    crate::setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &setups,
        &crate::MatchOptions {
            mcv_start: false,
            crates: false,
            unit_count: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    building(&mut app, 0, "nuclear_reactor", CellPos::new(2, 2));
    building(&mut app, 0, "spy_satellite", CellPos::new(2, 8));
    app
}

fn building(app: &mut App, owner: u8, kind: &str, cell: CellPos) -> SimId {
    crate::spawn_building(app.world_mut(), PlayerId(owner), kind, cell).unwrap()
}
fn unit(app: &mut App, owner: u8, kind: &str, cell: CellPos) -> SimId {
    crate::spawn_unit(app.world_mut(), PlayerId(owner), kind, cell).unwrap()
}
fn entity(app: &mut App, id: SimId) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(e, _)| e)
}
fn cell(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).unwrap();
    app.world().get::<Mobile>(e).unwrap().cell
}
fn step(app: &mut App) -> Vec<crate::SimEvent> {
    app.world_mut().run_schedule(crate::SimSchedule);
    let events: Vec<_> = app
        .world()
        .resource::<Messages<crate::SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, crate::SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert!(crate::cell_invariant_violations(app.world_mut()).is_empty());
    events
}
fn issue(app: &mut App, owner: u8, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand {
            player: PlayerId(owner),
            command,
        });
}
fn progress(app: &App, target: SimId) -> &BuildingJobProgress {
    &app.world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .building_jobs[&target]
}

fn attacking_tank() -> (App, SimId, SimId) {
    let mut app = app();
    let tank = unit(&mut app, 0, "rhino", CellPos::new(14, 14));
    let target = building(&mut app, 1, "power_plant", CellPos::new(30, 8));
    for _ in 0..5000 {
        step(&mut app);
        let e = entity(&mut app, tank).unwrap();
        let attacking = matches!(app.world().get::<crate::Weapon>(e).unwrap().order,
            Some(crate::AttackOrder::Attack { target: found, .. }) if found == target);
        if attacking
            && app
                .world()
                .resource::<AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .building_jobs
                .contains_key(&target)
        {
            assert!(progress(&app, target).actors.contains_key(&tank));
            return (app, tank, target);
        }
    }
    panic!("actual controller never admitted the capable tank's building attack");
}

#[test]
fn ra8_each_building_job_field_is_value_hashed_in_canonical_order() {
    let mut app = app();
    let mut base = app
        .world_mut()
        .resource_mut::<AiPlayers>()
        .controllers
        .get_mut(&PlayerId(0))
        .unwrap()
        .clone();
    base.building_jobs.insert(
        SimId(10),
        BuildingJobProgress {
            hp: 900,
            failures: 1,
            actors: [(SimId(2), 1), (SimId(3), 0)].into(),
            founders: [SimId(2), SimId(3)].into(),
            approaching: true,
        },
    );
    let hash = |c: &AiController| {
        let mut h = Fnv1a64::new();
        c.hash_into(&mut h);
        h.finish()
    };
    let mut equal = base.clone();
    equal.building_jobs.get_mut(&SimId(10)).unwrap().actors = [(SimId(3), 0), (SimId(2), 1)].into();
    equal.building_jobs.get_mut(&SimId(10)).unwrap().founders = [SimId(3), SimId(2)].into();
    assert_eq!(base, equal);
    assert_eq!(hash(&base), hash(&equal));
    for field in 0..9 {
        let mut changed = base.clone();
        let job = changed.building_jobs.get_mut(&SimId(10)).unwrap();
        match field {
            0 => job.hp -= 1,
            1 => job.failures += 1,
            2 => {
                job.actors.remove(&SimId(2));
            }
            3 => {
                job.actors.insert(SimId(4), 1);
            }
            4 => {
                job.actors.insert(SimId(2), 2);
            }
            5 => {
                job.founders.remove(&SimId(2));
            }
            6 => {
                job.founders.insert(SimId(4));
            }
            7 => job.approaching = false,
            8 => {
                let moved = changed.building_jobs.remove(&SimId(10)).unwrap();
                changed.building_jobs.insert(SimId(11), moved);
            }
            _ => unreachable!(),
        }
        assert_ne!(hash(&base), hash(&changed), "field {field}");
    }
}

#[test]
fn ra8_real_cargo_and_depot_entry_do_not_fabricate_building_attack_losses() {
    for cargo in [true, false] {
        let (mut app, tank, target) = attacking_tank();
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
        *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
        let at = cell(&mut app, tank);
        let host = if cargo {
            unit(
                &mut app,
                0,
                "amphibious_transport",
                CellPos::new(at.x - 1, at.y),
            )
        } else {
            let tank_entity = entity(&mut app, tank).unwrap();
            app.world_mut()
                .get_mut::<crate::Health>(tank_entity)
                .unwrap()
                .hit_points = 200;
            building(
                &mut app,
                0,
                "soviet_service_depot",
                CellPos::new(at.x - 2, at.y - 1),
            )
        };
        step(&mut app);
        issue(
            &mut app,
            0,
            Command::Interact {
                unit: tank,
                target: host,
            },
        );
        let mut entered = false;
        for _ in 0..100 {
            step(&mut app);
            let special = app.world().resource::<crate::SpecialState>();
            entered = if cargo {
                special
                    .cargo
                    .get(&host)
                    .is_some_and(|p| p.iter().any(|p| p.id == tank))
            } else {
                special.docked.contains_key(&tank)
            };
            if entered {
                break;
            }
        }
        assert!(
            entered,
            "ordinary interaction did not establish cargo={cargo}"
        );
        app.world_mut().insert_resource(controllers);
        for _ in 0..30 {
            step(&mut app);
        }
        assert_eq!(
            progress(&app, target).failures,
            0,
            "temporary cargo={cargo} became a casualty"
        );
        if cargo {
            assert!(progress(&app, target).actors.contains_key(&tank));
        } else {
            // R119 may already have released this unaffordable service job with
            // an ordinary Move. Leaving the old attack is not a casualty either.
            assert!(
                !app.world()
                    .resource::<crate::SpecialState>()
                    .docked
                    .contains_key(&tank)
            );
        }
    }
}

#[test]
fn ra8_actual_chronoshift_and_temporal_disable_keep_attack_attempts_alive() {
    for chrono in [true, false] {
        let (mut app, tank, target) = attacking_tank();
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
        *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
        let at = cell(&mut app, tank);
        if chrono {
            let source = building(&mut app, 0, "chronosphere", CellPos::new(2, 20));
            step(&mut app); // Normal source initialization precedes the isolated readiness fixture.
            let defs = app.world().resource::<Definitions>();
            let cooldown = defs
                .metadata(ProductionItem::Building(
                    defs.building_kind("chronosphere").unwrap(),
                ))
                .unwrap()
                .cooldown;
            app.world_mut()
                .resource_mut::<crate::SpecialState>()
                .charge
                .insert(source, cooldown);
            issue(
                &mut app,
                0,
                Command::Chronoshift {
                    building: source,
                    source: at,
                    target: CellPos::new(at.x + 5, at.y + 5),
                },
            );
            step(&mut app);
            assert!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .disabled
                    .contains_key(&tank)
            );
        } else {
            let attacker = unit(
                &mut app,
                1,
                "chrono_legionnaire",
                CellPos::new(at.x + 2, at.y),
            );
            step(&mut app);
            issue(
                &mut app,
                1,
                Command::Attack {
                    units: vec![attacker],
                    target: tank,
                },
            );
            for _ in 0..50 {
                step(&mut app);
                if app
                    .world()
                    .resource::<crate::SpecialState>()
                    .erasures
                    .contains_key(&tank)
                {
                    break;
                }
            }
            assert!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .erasures
                    .contains_key(&tank)
            );
        }
        app.world_mut().insert_resource(controllers);
        // A due ordinary AI turn occurs before either source's temporary disable ends.
        for _ in 0..20 {
            step(&mut app);
        }
        assert!(entity(&mut app, tank).is_some());
        assert_eq!(progress(&app, target).failures, 0);
        assert!(progress(&app, target).actors.contains_key(&tank));
    }
}

#[test]
fn ra8_actual_death_and_mind_control_complete_only_the_lost_owned_attempt() {
    for capture in [false, true] {
        let (mut app, tank, target) = attacking_tank();
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
        *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
        let at = cell(&mut app, tank);
        if !capture {
            let e = entity(&mut app, tank).unwrap();
            app.world_mut()
                .get_mut::<crate::Health>(e)
                .unwrap()
                .hit_points = 1;
        }
        let attacker = unit(
            &mut app,
            1,
            if capture { "yuri" } else { "conscript" },
            CellPos::new(at.x + 1, at.y),
        );
        step(&mut app);
        issue(
            &mut app,
            1,
            Command::Attack {
                units: vec![attacker],
                target: tank,
            },
        );
        let mut changed = false;
        for _ in 0..100 {
            step(&mut app);
            changed = entity(&mut app, tank)
                .is_none_or(|e| app.world().get::<Owner>(e).unwrap().0 != PlayerId(0));
            if changed {
                break;
            }
        }
        assert!(changed, "ordinary capture={capture} never completed");
        app.world_mut().insert_resource(controllers);
        for _ in 0..20 {
            step(&mut app);
        }
        let job = progress(&app, target);
        assert_eq!(
            job.failures, 1,
            "actual capture={capture} must complete one observed batch"
        );
        assert!(!job.actors.contains_key(&tank));
        assert!(!job.founders.contains(&tank));
        assert!(
            app.world()
                .resource::<crate::MatchState>()
                .outcome()
                .is_none()
        );
    }
}

#[test]
fn ra8_real_declining_hp_and_ammunition_reloads_keep_a_healthy_building_job() {
    let mut app = app();
    building(&mut app, 0, "airforce_command", CellPos::new(12, 4));
    let plane = unit(&mut app, 0, "harrier", CellPos::new(12, 8));
    let target = building(&mut app, 1, "power_plant", CellPos::new(40, 8));
    let defs = app.world().resource::<Definitions>();
    let ammo = defs
        .metadata(ProductionItem::Unit(defs.unit_kind("harrier").unwrap()))
        .unwrap()
        .ammo;
    let mut low = i32::MAX;
    let mut improvements = 0;
    let mut empty = false;
    let mut reloads = 0;
    let mut rearming = false;
    let mut shots = 0;
    for _ in 0..12000 {
        let events = step(&mut app);
        shots += events.iter().filter(|e| matches!(e, crate::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == plane && *found == target)).count();
        let special = app.world().resource::<crate::SpecialState>();
        let now_empty = special.ammo_of(plane, ammo) == 0;
        reloads += usize::from(empty && !now_empty);
        empty = now_empty;
        rearming |= special.reloading.contains_key(&plane);
        let Some(job) = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .building_jobs
            .get(&target)
        else {
            continue;
        };
        if job.hp < low {
            if low != i32::MAX {
                improvements += 1;
                assert_eq!(
                    job.failures, 0,
                    "an actually improving HP floor did not renew the job"
                );
            }
            low = job.hp;
        }
        assert!(
            entity(&mut app, plane).is_some(),
            "temporary docking was mistaken for an actual aircraft loss"
        );
        if improvements >= 2 && reloads >= 2 && rearming {
            break;
        }
    }
    assert!(
        improvements >= 2 && reloads >= 2 && rearming && shots > 2,
        "source shots{shots}/HP improvements{improvements}/reloads{reloads}/rearming{rearming}"
    );
}

#[test]
fn ra8_original_approach_survives_but_replacement_ids_cannot_renew_a_failed_job() {
    let (mut app, founder, target) = attacking_tank();
    let replacement = unit(&mut app, 0, "rhino", CellPos::new(12, 18));
    // Declared exhausted history for an already scheduled real approach. The new
    // actor must be admitted by the actual controller, rather than a helper call.
    {
        let mut players = app.world_mut().resource_mut::<AiPlayers>();
        let controller = players.controllers.get_mut(&PlayerId(0)).unwrap();
        let job = controller.building_jobs.get_mut(&target).unwrap();
        job.failures = MAX_TARGET_FAILURES;
        job.approaching = true;
        controller.wave.insert(replacement);
    }
    for _ in 0..10 {
        step(&mut app);
    }
    let job = progress(&app, target);
    assert_eq!(job.failures, MAX_TARGET_FAILURES);
    assert!(
        job.approaching,
        "the original live path must remain eligible"
    );
    assert!(
        job.actors.contains_key(&replacement),
        "actual AI did not admit the ready replacement"
    );
    assert!(job.founders.contains(&founder));
    assert!(
        !job.founders.contains(&replacement),
        "a new ID reset the exhausted job's approach exemption"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    let at = cell(&mut app, founder);
    let e = entity(&mut app, founder).unwrap();
    app.world_mut()
        .get_mut::<crate::Health>(e)
        .unwrap()
        .hit_points = 1;
    let killer = unit(&mut app, 1, "conscript", CellPos::new(at.x, at.y + 1));
    step(&mut app);
    issue(
        &mut app,
        1,
        Command::Attack {
            units: vec![killer],
            target: founder,
        },
    );
    for _ in 0..40 {
        step(&mut app);
        if entity(&mut app, founder).is_none() {
            break;
        }
    }
    assert!(
        entity(&mut app, founder).is_none(),
        "source fire must remove the original founder"
    );
    app.world_mut().insert_resource(controllers);
    for _ in 0..10 {
        step(&mut app);
    }
    let job = progress(&app, target);
    assert_eq!(job.failures, MAX_TARGET_FAILURES);
    assert!(
        !job.approaching,
        "replacement travel alone cannot renew the same failed job"
    );
    assert!(!job.founders.contains(&replacement));
    assert!(entity(&mut app, replacement).is_some());
    assert!(
        app.world()
            .resource::<crate::MatchState>()
            .outcome()
            .is_none()
    );
}

#[test]
fn ra8_exhausted_known_jobs_keep_legal_search_and_do_not_fabricate_a_winner() {
    let (mut app, tank, _) = attacking_tank();
    let exhausted: BTreeSet<_>;
    {
        let mut players = app.world_mut().resource_mut::<AiPlayers>();
        let controller = players.controllers.get_mut(&PlayerId(0)).unwrap();
        exhausted = controller.known.keys().copied().collect();
        for &id in &exhausted {
            controller.building_jobs.insert(
                id,
                BuildingJobProgress {
                    hp: 0,
                    failures: MAX_TARGET_FAILURES,
                    actors: BTreeMap::new(),
                    founders: BTreeSet::new(),
                    approaching: false,
                },
            );
        }
    }
    assert!(exhausted.len() >= 2);
    for _ in 0..100 {
        step(&mut app);
        for issued in app.world().resource::<CommandQueue>().iter() {
            match &issued.command {
                Command::Attack { units, target } => {
                    assert!(!units.is_empty());
                    assert!(
                        !exhausted.contains(target),
                        "exhausted jobs were silently reinstalled"
                    );
                }
                Command::Move { units, .. } | Command::AttackMove { units, .. } => {
                    assert!(!units.is_empty())
                }
                _ => {}
            }
        }
        for id in &exhausted {
            let job = progress(&app, *id);
            assert_eq!(job.failures, MAX_TARGET_FAILURES);
            assert!(
                job.actors.is_empty(),
                "search invented a completed attacker"
            );
        }
        assert!(entity(&mut app, tank).is_some());
        assert!(
            app.world()
                .resource::<crate::MatchState>()
                .outcome()
                .is_none()
        );
        assert!(
            !app.world()
                .resource::<AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .last_stand
        );
    }
}

#[test]
fn ra8_real_support_launch_and_source_recharge_do_not_complete_an_army_attempt() {
    let (mut app, tank, target) = attacking_tank();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    let source = building(&mut app, 0, "iron_curtain", CellPos::new(2, 20));
    step(&mut app); // Actual source announcement initializes its ordinary charge.
    let defs = app.world().resource::<Definitions>();
    let cooldown = defs
        .metadata(ProductionItem::Building(
            defs.building_kind("iron_curtain").unwrap(),
        ))
        .unwrap()
        .cooldown;
    app.world_mut()
        .resource_mut::<crate::SpecialState>()
        .charge
        .insert(source, cooldown);
    app.world_mut().insert_resource(controllers);
    let mut launched = false;
    let mut restarted = false;
    for _ in 0..40 {
        let events = step(&mut app);
        launched |= events.iter().any(|event| matches!(event, crate::SimEvent::StrategicActivated { building, .. } if *building == source));
        let special = app.world().resource::<crate::SpecialState>();
        restarted |= launched
            && special
                .charge
                .get(&source)
                .is_some_and(|charge| *charge > 0 && *charge < cooldown);
        assert_eq!(progress(&app, target).failures, 0);
        assert!(progress(&app, target).actors.contains_key(&tank));
        assert!(!progress(&app, target).actors.contains_key(&source));
        if launched && restarted {
            break;
        }
    }
    assert!(
        launched && restarted,
        "normal real launch/recharge must precede the progress assertions"
    );
}
