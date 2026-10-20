//! Actual scheduled attacker lifecycles for the observed building-job history.
use super::*;
use bevy_app::App;
use bevy_ecs::{entity::Entity, message::Messages};

pub(super) fn app() -> App {
    app_with_power(true)
}

pub(super) fn app_with_power(power: bool) -> App {
    app_country(power, "germany")
}

pub(super) fn app_with_country(country: &str) -> App {
    app_country(true, country)
}

fn app_country(power: bool, country: &str) -> App {
    let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let definitions = crate::load_ruleset(&data).unwrap();
    let mut skirmish = crate::load_skirmish(data.join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(38, 38)];
    skirmish.starting_credits = 0;
    let ai = crate::load_ai(data.join("ai.ron"), &definitions).unwrap();
    let setups = [country, "russia"].map(|country| crate::PlayerSetup {
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
    if power {
        building(&mut app, 0, "nuclear_reactor", CellPos::new(2, 2));
    }
    building(&mut app, 0, "spy_satellite", CellPos::new(2, 8));
    app
}

pub(super) fn building(app: &mut App, owner: u8, kind: &str, cell: CellPos) -> SimId {
    crate::spawn_building(app.world_mut(), PlayerId(owner), kind, cell).unwrap()
}
pub(super) fn unit(app: &mut App, owner: u8, kind: &str, cell: CellPos) -> SimId {
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
pub(super) fn step(app: &mut App) -> Vec<crate::SimEvent> {
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
pub(super) fn issue(app: &mut App, owner: u8, command: Command) {
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

#[test]
fn ra8_failed_real_sorties_do_not_blacklist_a_naturally_charged_strategic_strike() {
    let mut app = app();
    // Declared isolated enemy base: the real plant and captured income below.
    let reserve = {
        let world = app.world_mut();
        world
            .query::<(Entity, &Owner, &Building)>()
            .iter(world)
            .find(|(_, owner, _)| owner.0 == PlayerId(1))
            .unwrap()
            .0
    };
    app.world_mut()
        .get_mut::<crate::Health>(reserve)
        .unwrap()
        .hit_points = 0;
    building(&mut app, 0, "airforce_command", CellPos::new(12, 4));
    let plane = unit(&mut app, 0, "harrier", CellPos::new(12, 8));
    let target = building(&mut app, 1, "power_plant", CellPos::new(40, 8));
    let oil = crate::spawn_building(
        app.world_mut(),
        crate::NEUTRAL,
        "oil_derrick",
        CellPos::new(44, 28),
    )
    .unwrap();
    let engineer = unit(&mut app, 1, "engineer", CellPos::new(43, 28));
    unit(&mut app, 1, "gi", CellPos::new(43, 40));
    step(&mut app);
    issue(
        &mut app,
        1,
        Command::Interact {
            unit: engineer,
            target: oil,
        },
    );
    let maximum = app
        .world()
        .resource::<Definitions>()
        .building_combat(
            app.world()
                .resource::<Definitions>()
                .building_kind("power_plant")
                .unwrap(),
        )
        .unwrap()
        .hit_points;
    let mut paid_heals = 0;
    let mut shots = 0;
    let mut exhausted = false;
    for _ in 0..18_000 {
        let target_entity = entity(&mut app, target).unwrap();
        let before = app
            .world()
            .get::<crate::Health>(target_entity)
            .unwrap()
            .hit_points;
        let credits = app
            .world()
            .resource::<Players>()
            .get(PlayerId(1))
            .unwrap()
            .credits();
        for event in step(&mut app) {
            shots += usize::from(
                matches!(event, crate::SimEvent::WeaponFired { attacker, target: found, .. } if attacker == plane && found == target),
            );
        }
        let e = entity(&mut app, target).unwrap();
        let hp = app.world().get::<crate::Health>(e).unwrap().hit_points;
        let state = app.world().resource::<Players>().get(PlayerId(1)).unwrap();
        paid_heals += usize::from(hp > before && state.credits() < credits);
        let repair =
            hp < maximum && hp > 0 && state.credits() > 0 && !state.repairing().contains(&target);
        if repair {
            issue(&mut app, 1, Command::RepairBuilding { building: target });
        }
        exhausted = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .building_jobs
            .get(&target)
            .is_some_and(|job| job.failures == MAX_TARGET_FAILURES && !job.approaching);
        if exhausted {
            break;
        }
    }
    assert!(
        exhausted && shots > 2 && paid_heals > 0,
        "real sorties/paid repair did not exhaust a building job: shots{shots}, heals{paid_heals}"
    );
    let old = progress(&app, target).clone();
    assert!(old.actors.is_empty() && old.founders.is_empty());
    // The source is added only after real failed attempts. No charge, target HP,
    // ammo, failure history or readiness is injected; normal recharge is measured.
    let source = building(&mut app, 0, "weather_control", CellPos::new(2, 20));
    step(&mut app);
    let definitions = app.world().resource::<Definitions>();
    let cooldown = definitions
        .metadata(ProductionItem::Building(
            definitions.building_kind("weather_control").unwrap(),
        ))
        .unwrap()
        .cooldown;
    assert_eq!(
        app.world()
            .resource::<crate::SpecialState>()
            .charge
            .get(&source),
        Some(&0)
    );
    let mut saw_ready = false;
    let mut launched = false;
    let mut damaged = false;
    for _ in 0..cooldown.saturating_add(2400) {
        let special = app.world().resource::<crate::SpecialState>();
        if special
            .charge
            .get(&source)
            .is_some_and(|charge| *charge >= cooldown)
        {
            saw_ready = true;
            assert!(
                !app.world()
                    .resource::<Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .low_power()
            );
            assert!(
                !special.disabled.contains_key(&source) && !special.erasures.contains_key(&source)
            );
        }
        if !launched {
            assert_eq!(
                progress(&app, target).failures,
                old.failures,
                "ordinary recharge must not clear the ground job"
            );
            assert!(!app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Attack { target: found, .. } if *found == target)), "failed ground job was silently reinstalled");
        }
        let e = entity(&mut app, target);
        let before = e
            .and_then(|e| app.world().get::<crate::Health>(e))
            .map_or(0, |hp| hp.hit_points);
        for event in step(&mut app) {
            launched |= matches!(event, crate::SimEvent::StrategicActivated { building, target: found, role: crate::Role::Storm, .. } if building == source && found == CellPos::new(40, 8));
        }
        let after = entity(&mut app, target)
            .and_then(|e| app.world().get::<crate::Health>(e))
            .map_or(0, |hp| hp.hit_points);
        damaged |= launched && after < before;
        if launched && damaged {
            break;
        }
    }
    eprintln!(
        "failed-sortie strategic: shots{shots}, paid_heals{paid_heals}, old{old:?}, naturally_ready{saw_ready}, launched{launched}, damage{damaged}"
    );
    assert!(
        saw_ready && launched && damaged,
        "ground-cohort failure suppressed an independently legal naturally charged strategic strike"
    );
}

#[test]
fn ra8_an_expired_aa_job_cannot_starve_a_fresh_observed_barrier() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let first = building(&mut app, 1, "patriot", CellPos::new(34, 12));
    building(&mut app, 1, "nuclear_reactor", CellPos::new(42, 2));
    building(&mut app, 1, "soviet_barracks", CellPos::new(30, 20));
    for _ in 0..6000 {
        step(&mut app);
    }
    let old_ground = unit(&mut app, 0, "gi", CellPos::new(30, 12));
    let air = unit(&mut app, 0, "kirov", CellPos::new(12, 14));
    step(&mut app);
    app.world_mut().insert_resource(controllers);
    let mut installed = false;
    for _ in 0..3000 {
        step(&mut app);
        installed = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .air_screens
            .contains_key(&first);
        if installed {
            break;
        }
    }
    assert!(installed, "first actual clearance job must be selected");
    // Ordinary external orders move the original team away. Its real approach/HP
    // minima remain observed history; no timer or controller field is seeded.
    let mut expired = None;
    for _ in 0..6000 {
        issue(
            &mut app,
            0,
            Command::Move {
                units: vec![old_ground, air],
                target: CellPos::new(3, 40),
            },
        );
        step(&mut app);
        let c = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        let progress = &c.air_screens[&first];
        if app
            .world()
            .resource::<SimTick>()
            .0
            .saturating_sub(progress.last_progress)
            >= u64::from(c.difficulty.wave_patience)
        {
            expired = Some(progress.clone());
            break;
        }
    }
    let expired = expired.expect("actual source patience expires after observed progress stops");
    let second = building(&mut app, 1, "patriot", CellPos::new(34, 28));
    let recruits = [
        CellPos::new(30, 28),
        CellPos::new(30, 29),
        CellPos::new(30, 30),
        CellPos::new(31, 30),
    ]
    .map(|at| unit(&mut app, 0, "gi", at));
    assert!(
        !app.world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .air_screens
            .contains_key(&second)
    );
    let mut selected = false;
    let mut assigned = false;
    let mut fired = false;
    for _ in 0..3000 {
        issue(
            &mut app,
            0,
            Command::Move {
                units: vec![old_ground, air],
                target: CellPos::new(3, 40),
            },
        );
        for event in step(&mut app) {
            fired |= assigned
                && matches!(event, crate::SimEvent::WeaponFired { attacker, target, .. } if recruits.contains(&attacker) && target == second);
        }
        let c = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        selected |= c.air_screens.contains_key(&second);
        assigned |=
            app.world()
                .resource::<CommandQueue>()
                .iter()
                .any(|issued| match &issued.command {
                    Command::Attack { units, target } => {
                        *target == second && units.iter().any(|id| recruits.contains(id))
                    }
                    Command::AttackMove { units, target } => {
                        *target == CellPos::new(34, 28)
                            && units.iter().any(|id| recruits.contains(id))
                    }
                    _ => false,
                });
        if c.air_screens[&first] != expired || (selected && assigned && fired) {
            break;
        }
    }
    let c = app
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap();
    eprintln!(
        "two AA jobs: selected{selected} assigned{assigned} fired{fired} first{:?} expired{expired:?} all{:?}",
        c.air_screens.get(&first),
        c.air_screens
    );
    assert!(
        selected && assigned && fired,
        "the expired minimum candidate starved a fresh observed AA clearance job"
    );
    assert_eq!(
        c.air_screens[&first], expired,
        "the selected fresh job must leave the unchanged expired first job alone"
    );
    // Genuine observed damage still reopens the original job. Ordinary source GI
    // fire, not a seeded HP/history change, supplies the new opportunity.
    let shooter = unit(&mut app, 0, "gi", CellPos::new(30, 12));
    issue(
        &mut app,
        0,
        Command::Attack {
            units: vec![shooter],
            target: first,
        },
    );
    let mut hit = false;
    let mut reopened = false;
    for _ in 0..300 {
        for event in step(&mut app) {
            hit |= matches!(event, crate::SimEvent::WeaponFired { attacker, target, .. } if attacker == shooter && target == first);
        }
        let progress = &app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .air_screens[&first];
        reopened |= progress.hp < expired.hp && progress.last_progress > expired.last_progress;
        if hit && reopened {
            break;
        }
    }
    assert!(
        hit && reopened,
        "actual observed HP progress must reopen the old job"
    );
}

#[test]
fn ra8_a_current_observed_building_mission_is_not_replaced_by_a_repaired_home_nearer_target() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let distraction = building(&mut app, 1, "soviet_service_depot", CellPos::new(28, 12));
    let objective = building(&mut app, 1, "tesla_reactor", CellPos::new(34, 12));
    let oil = building(&mut app, 255, "oil_derrick", CellPos::new(44, 2));
    let engineer = unit(&mut app, 1, "engineer", CellPos::new(43, 2));
    step(&mut app);
    issue(
        &mut app,
        1,
        Command::Interact {
            unit: engineer,
            target: oil,
        },
    );
    for _ in 0..6000 {
        step(&mut app);
    }
    assert!(
        entity(&mut app, engineer).is_none(),
        "ordinary engineer capture must activate the income source"
    );
    assert!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(1))
            .unwrap()
            .credits()
            > 0
    );
    let attacker = unit(&mut app, 0, "flak_track", CellPos::new(34, 17));
    issue(
        &mut app,
        0,
        Command::Attack {
            units: vec![attacker],
            target: distraction,
        },
    );
    let definitions = app.world().resource::<Definitions>();
    let maximum = definitions
        .building_combat(definitions.building_kind("soviet_service_depot").unwrap())
        .unwrap()
        .hit_points;
    let mut hit = false;
    let mut damaged = false;
    for _ in 0..200 {
        hit |= step(&mut app).iter().any(|e| matches!(e, crate::SimEvent::WeaponFired { attacker: found, target, .. } if *found == attacker && *target == distraction));
        let e = entity(&mut app, distraction).unwrap();
        damaged = app.world().get::<crate::Health>(e).unwrap().hit_points < maximum;
        if hit && damaged {
            break;
        }
    }
    assert!(
        hit && damaged,
        "the distraction must be genuinely damaged after the native projectile impact"
    );
    issue(
        &mut app,
        1,
        Command::RepairBuilding {
            building: distraction,
        },
    );
    let before = app
        .world()
        .resource::<Players>()
        .get(PlayerId(1))
        .unwrap()
        .credits();
    let mut healed = false;
    for _ in 0..100 {
        let e = entity(&mut app, distraction).unwrap();
        let hp = app.world().get::<crate::Health>(e).unwrap().hit_points;
        step(&mut app);
        let e = entity(&mut app, distraction).unwrap();
        healed |= app.world().get::<crate::Health>(e).unwrap().hit_points > hp;
    }
    assert!(
        healed
            && app
                .world()
                .resource::<Players>()
                .get(PlayerId(1))
                .unwrap()
                .credits()
                < before.saturating_add(20),
        "ordinary income-funded repairs must really compete with native damage: healed{healed}, before{before}, after{}",
        app.world()
            .resource::<Players>()
            .get(PlayerId(1))
            .unwrap()
            .credits()
    );
    let at = CellPos::new(38, 15);
    issue(
        &mut app,
        0,
        Command::Move {
            units: vec![attacker],
            target: at,
        },
    );
    let mut arrived = false;
    for _ in 0..300 {
        step(&mut app);
        let e = entity(&mut app, attacker).unwrap();
        let mobile = app.world().get::<Mobile>(e).unwrap();
        arrived = mobile.cell == at && mobile.step.is_none();
        if arrived {
            break;
        }
    }
    assert!(
        arrived,
        "ordinary movement must establish the current front before AI admission"
    );
    app.world_mut().insert_resource(controllers);
    let mut actual_objective = false;
    for _ in 0..20 {
        step(&mut app);
        let c = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        actual_objective |= c.target == Some(WaveTarget::Building(objective));
    }
    assert!(
        actual_objective,
        "ordinary core selection must choose the nearer current-front objective"
    );
    let c = app
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap();
    assert!(c.known.contains_key(&objective) && c.known.contains_key(&distraction));
    assert!(strategy::attack_job_eligible(c, objective));
    assert!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(34, 12))
    );
    let mut objective_shot = false;
    for _ in 0..1000 {
        objective_shot |= step(&mut app).iter().any(|e| matches!(e, crate::SimEvent::WeaponFired {attacker: found,target,..} if *found == attacker && *target == objective));
        if objective_shot {
            break;
        }
    }
    assert!(
        objective_shot,
        "strategy must preserve the legal current mission through actual alternative-building fire"
    );
    // Ordinary native reinforcement destroys the selected objective; a stale
    // current ID must not prevent the next legal observed mission from firing.
    let reinforcement = unit(&mut app, 0, "rhino", CellPos::new(36, 16));
    issue(
        &mut app,
        0,
        Command::Attack {
            units: vec![reinforcement],
            target: objective,
        },
    );
    let mut gone = false;
    let mut fallback_shot = false;
    for _ in 0..4000 {
        for event in step(&mut app) {
            gone |= matches!(event, crate::SimEvent::BuildingDestroyed { building, .. } if building == objective);
            fallback_shot |= gone
                && matches!(event, crate::SimEvent::WeaponFired { attacker: found, target, .. } if found == attacker && target == distraction);
        }
        if fallback_shot {
            break;
        }
    }
    assert!(
        gone && fallback_shot,
        "native objective destruction must permit actual fire at the remaining legal distraction"
    );
    assert!(
        !app.world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .known
            .contains_key(&objective)
    );
}

#[test]
fn ra8_a_remote_armed_unit_does_not_pin_a_surface_mission_on_a_repaired_distraction() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let oil = building(&mut app, 255, "oil_derrick", CellPos::new(44, 2));
    let engineer = unit(&mut app, 1, "engineer", CellPos::new(43, 2));
    step(&mut app);
    issue(
        &mut app,
        1,
        Command::Interact {
            unit: engineer,
            target: oil,
        },
    );
    for _ in 0..6000 {
        step(&mut app);
    }
    assert!(entity(&mut app, engineer).is_none());
    let objective = building(&mut app, 1, "soviet_refinery", CellPos::new(15, 25));
    let attacker = unit(&mut app, 0, "mirage_tank", CellPos::new(28, 24));
    app.world_mut().insert_resource(controllers);
    for _ in 0..20 {
        step(&mut app);
    }
    assert_eq!(
        app.world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .target,
        Some(WaveTarget::Building(objective)),
        "the actual core must first select the refinery"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let distraction = building(&mut app, 1, "tesla_coil", CellPos::new(27, 29));
    let remote = unit(&mut app, 1, "gi", CellPos::new(44, 44));
    issue(
        &mut app,
        0,
        Command::AttackMove {
            units: vec![attacker],
            target: CellPos::new(15, 24),
        },
    );
    let mut distraction_shots = 0;
    for _ in 0..100 {
        distraction_shots += step(&mut app).iter().filter(|e| matches!(e, crate::SimEvent::WeaponFired { attacker: a, target, .. } if *a == attacker && *target == distraction)).count();
    }
    assert!(
        distraction_shots > 0,
        "ordinary opportunistic acquisition must actually fire at the distraction"
    );
    assert!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(1))
            .unwrap()
            .low_power(),
        "the native unpowered Coil isolates mission selection without damage immunity"
    );
    issue(
        &mut app,
        1,
        Command::RepairBuilding {
            building: distraction,
        },
    );
    let mut healed = false;
    for _ in 0..100 {
        let e = entity(&mut app, distraction).unwrap();
        let before = app.world().get::<crate::Health>(e).unwrap().hit_points;
        step(&mut app);
        let e = entity(&mut app, distraction).unwrap();
        healed |= app.world().get::<crate::Health>(e).unwrap().hit_points > before;
    }
    assert!(healed, "income must pay for actual native repair steps");
    let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
    assert!(
        state.vision().is_visible(CellPos::new(44, 44))
            && state.vision().is_visible(CellPos::new(15, 25))
    );
    assert!(entity(&mut app, remote).is_some());
    app.world_mut().insert_resource(controllers);
    let mut objective_shot = false;
    for _ in 0..1000 {
        objective_shot |= step(&mut app).iter().any(|e| matches!(e, crate::SimEvent::WeaponFired {attacker: a, target, ..} if *a == attacker && *target == objective));
        if objective_shot {
            break;
        }
    }
    assert!(
        objective_shot,
        "a visible remote armed unit must not prevent the native wave from resuming and firing at its selected building mission"
    );
    for _ in 0..40 {
        step(&mut app);
        assert!(entity(&mut app, objective).is_some());
        assert!(!app.world().resource::<crate::CommandQueue>().iter().any(|issued| matches!(&issued.command,
            Command::Move { units, .. } | Command::AttackMove { units, .. } | Command::Attack { units, .. } if units.contains(&attacker))),
            "an already matching healthy surface focus must not be reset by another decision");
    }
}

#[test]
fn ra8_surface_focus_preserves_a_real_healthy_approach_before_native_building_fire() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    for _ in 0..6000 {
        step(&mut app);
    }
    let objective = building(&mut app, 1, "soviet_refinery", CellPos::new(15, 25));
    let attacker = unit(&mut app, 0, "rhino", CellPos::new(28, 24));
    unit(&mut app, 1, "gi", CellPos::new(44, 44));
    app.world_mut().insert_resource(controllers);
    for _ in 0..20 {
        step(&mut app);
    }
    let controller = app
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap();
    assert_eq!(controller.target, Some(WaveTarget::Building(objective)));
    assert!(controller.wave.contains(&attacker));
    let cadence = u64::from(controller.difficulty().think_interval);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let destination = CellPos::new(15, 24);
    issue(
        &mut app,
        0,
        Command::AttackMove {
            units: vec![attacker],
            target: destination,
        },
    );
    step(&mut app);
    while !app
        .world()
        .resource::<crate::SimTick>()
        .0
        .is_multiple_of(cadence)
    {
        step(&mut app);
    }
    let entity_id = entity(&mut app, attacker).unwrap();
    let mobile = *app.world().get::<Mobile>(entity_id).unwrap();
    assert!(
        mobile.step.is_some(),
        "ordinary native pathfinding must establish real approach progress"
    );
    assert!(
        matches!(app.world().get::<crate::Weapon>(entity_id).unwrap().order,
        Some(crate::AttackOrder::AttackMove { destination: found, moving: true }) if found == destination)
    );
    app.world_mut().insert_resource(controllers);
    // Execute the actual due AI decision and then apply its FIFO intent.
    step(&mut app);
    step(&mut app);
    let entity_id = entity(&mut app, attacker).unwrap();
    eprintln!(
        "healthy surface approach before{mobile:?} after{:?} order{:?}",
        app.world().get::<Mobile>(entity_id),
        app.world().get::<crate::Weapon>(entity_id).unwrap().order
    );
    assert!(
        matches!(app.world().get::<crate::Weapon>(entity_id).unwrap().order,
        Some(crate::AttackOrder::AttackMove { destination: found, moving: true }) if found == destination),
        "a real healthy approach must retain native movement/combat ownership across the due AI application"
    );
    let mut fired = false;
    for _ in 0..1000 {
        fired |= step(&mut app).iter().any(|event| matches!(event,
            crate::SimEvent::WeaponFired { attacker: found, target, .. } if *found == attacker && *target == objective));
        if fired {
            break;
        }
    }
    assert!(
        fired,
        "preserving the native approach must still yield a real building shot"
    );
}

#[test]
fn ra8_a_unit_controller_keeps_its_native_approach_instead_of_attacking_an_immune_building() {
    let mut app = app_with_country("iraq");
    let objective = building(&mut app, 1, "soviet_service_depot", CellPos::new(34, 12));
    let recruit = unit(&mut app, 1, "rhino", CellPos::new(36, 17));
    let controller_unit = unit(&mut app, 0, "yuri", CellPos::new(20, 20));
    let definitions = app.world().resource::<Definitions>();
    let role = definitions
        .metadata(crate::ProductionItem::Unit(
            definitions.unit_kind("yuri").unwrap(),
        ))
        .unwrap()
        .role;
    assert_eq!(role, Role::MindControl);
    let targets: BTreeSet<_> = app
        .world_mut()
        .query::<(&SimId, &Owner, &Building)>()
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == PlayerId(1))
        .map(|(id, _, _)| *id)
        .collect();
    let mut known = false;
    let mut in_wave = false;
    let mut wrong_building_orders = 0;
    let mut native_endings = 0;
    let mut taken = false;
    let mut contributed = false;
    for _ in 0..10000 {
        let events = step(&mut app);
        for event in events {
            native_endings += usize::from(
                matches!(event, crate::SimEvent::AttackEnded { unit, .. } if unit == controller_unit),
            );
            contributed |= matches!(event, crate::SimEvent::WeaponFired { attacker, target, .. } if attacker == recruit && targets.contains(&target));
        }
        let ai = app
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        known |= ai.known.contains_key(&objective);
        in_wave |= ai.wave.contains(&controller_unit);
        wrong_building_orders += app.world().resource::<crate::CommandQueue>().iter().filter(|issued| matches!(&issued.command,
            Command::Attack { units, target } if units.contains(&controller_unit) && *target == objective)).count();
        let e = entity(&mut app, recruit).unwrap();
        taken |= app.world().get::<Owner>(e).unwrap().0 == PlayerId(0);
        if taken && contributed {
            break;
        }
    }
    assert!(
        known && in_wave,
        "the actual controller must admit the living source unit and learn the real building"
    );
    assert!(
        taken && contributed,
        "a unit-control actor must preserve native movement, actually take the controllable tank and use its gun; wrong_building_orders{wrong_building_orders}, native_endings{native_endings}, taken{taken}, contributed{contributed}"
    );
    assert_eq!(
        wrong_building_orders, 0,
        "an immune building must not receive the unit controller's focus orders"
    );
}

#[test]
fn ra8_target_qualified_surface_focus_keeps_native_temporal_and_ivan_effects() {
    for (country, kind) in [("germany", "chrono_legionnaire"), ("iraq", "crazy_ivan")] {
        let mut app = app_with_country(country);
        let objective = building(&mut app, 1, "power_plant", CellPos::new(34, 12));
        // Let the real initial source-wave deadline pass without home travel,
        // then admit a native actor inside both Combat's footprint-edge range
        // and Temporal's current nearest-cell-centre range. R170 records the
        // independent edge mismatch; complete native effects remain required.
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
        let c = controllers.controller(PlayerId(0)).unwrap();
        let ready = c
            .difficulty
            .first_wave
            .saturating_add(c.difficulty.wave_patience)
            .saturating_add(1);
        for _ in 0..ready {
            step(&mut app);
        }
        assert!(
            app.world()
                .resource::<crate::MatchState>()
                .outcome()
                .is_none()
        );
        let actor = unit(&mut app, 0, kind, CellPos::new(30, 15));
        app.world_mut().insert_resource(controllers);
        let remote = unit(&mut app, 1, "gi", CellPos::new(44, 44));
        let e = entity(&mut app, objective).unwrap();
        let initial = app.world().get::<crate::Health>(e).unwrap().hit_points;
        let mut focused = false;
        let mut fired = false;
        let mut effect = false;
        let mut first_remaining = None;
        let mut progressed = false;
        let mut planted = false;
        for _ in 0..10000 {
            fired |= step(&mut app).iter().any(|event| matches!(event,
                crate::SimEvent::WeaponFired { attacker, target, .. } if *attacker == actor && *target == objective));
            focused |= app
                .world()
                .resource::<crate::CommandQueue>()
                .iter()
                .any(|issued| {
                    matches!(&issued.command,
                Command::Attack { units, target } if units.contains(&actor) && *target == objective)
                });
            effect |= entity(&mut app, objective)
                .is_none_or(|e| app.world().get::<crate::Health>(e).unwrap().hit_points < initial);
            let special = app.world().resource::<crate::SpecialState>();
            if let Some(beam) = special.erasures.get(&objective) {
                let first = first_remaining.get_or_insert(beam.remaining);
                progressed |= beam.remaining < *first;
            }
            planted |= special.charges.contains_key(&objective);
            if focused && fired && effect {
                break;
            }
        }
        assert!(
            entity(&mut app, remote).is_some(),
            "the remote armed-unit branch must remain active"
        );
        eprintln!(
            "native ability {kind}: focused{focused}, fired{fired}, effect{effect}, first_remaining{first_remaining:?}, progressed{progressed}, planted{planted}"
        );
        assert!(
            if kind == "chrono_legionnaire" {
                progressed
            } else {
                planted
            },
            "the actual Temporal channel must advance, or the actual Ivan charge must exist before its impact"
        );
        assert!(
            focused && fired && effect,
            "{kind} must retain actual AI focus, native weapon fire and real erasure/bomb damage: focused{focused}, fired{fired}, effect{effect}"
        );
    }
}

#[test]
fn ra8_original_surface_approach_keeps_its_native_temporal_building_channel() {
    let mut app = app_with_country("germany");
    let objective = building(&mut app, 1, "power_plant", CellPos::new(34, 12));
    let actor = unit(&mut app, 0, "chrono_legionnaire", CellPos::new(20, 20));
    let remote = unit(&mut app, 1, "gi", CellPos::new(44, 44));
    let mut focused = false;
    let mut fired = false;
    let mut first_remaining = None;
    let mut progressed = false;
    let mut first_point = None;
    for _ in 0..10000 {
        let events = step(&mut app);
        if events.iter().any(|event| matches!(event,
            crate::SimEvent::WeaponFired { attacker, target, .. } if *attacker == actor && *target == objective)) {
            fired = true;
            let e = entity(&mut app, actor).unwrap();
            first_point.get_or_insert(app.world().get::<crate::Position>(e).unwrap().0);
        }
        focused |= app
            .world()
            .resource::<crate::CommandQueue>()
            .iter()
            .any(|issued| {
                matches!(&issued.command,
            Command::Attack { units, target } if units.contains(&actor) && *target == objective)
            });
        if let Some(beam) = app
            .world()
            .resource::<crate::SpecialState>()
            .erasures
            .get(&objective)
        {
            let first = first_remaining.get_or_insert(beam.remaining);
            progressed |= beam.remaining < *first;
        }
        if entity(&mut app, objective).is_none() {
            break;
        }
    }
    assert!(entity(&mut app, actor).is_some() && entity(&mut app, remote).is_some());
    assert!(
        focused && fired && progressed && entity(&mut app, objective).is_none(),
        "original native approach must actually focus/fire/progress/erase: focused{focused} fired{fired} progressed{progressed} first_remaining{first_remaining:?} first_point{first_point:?}"
    );
}
