//! RA8 shipped controller acceptance, through match setup, commands and SimSchedule.
use std::path::PathBuf;

use bevy_app::App;
use sim::{
    Cell, CellPos, Command, CommandQueue, Controller, Grid, MatchOptions, PlayerId, PlayerSetup,
    SimPlugin, SimSchedule, SimTick, load_ai, load_ruleset, load_skirmish, setup_match_configured,
    spawn_unit,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

fn start(country: &str, difficulty: &str) -> App {
    scenario(country, difficulty, 10_000, 10, true)
}

fn scenario(country: &str, difficulty: &str, credits: i64, units: u32, superweapons: bool) -> App {
    scenario_mode(
        country,
        difficulty,
        credits,
        units,
        superweapons,
        ScenarioMode::default(),
    )
}

#[derive(Default)]
struct ScenarioMode {
    human: bool,
    second_ai: bool,
    cadences: Option<[u32; 2]>,
}

fn scenario_mode(
    country: &str,
    difficulty: &str,
    credits: i64,
    units: u32,
    superweapons: bool,
    mode: ScenarioMode,
) -> App {
    let definitions = load_ruleset(data()).unwrap();
    let mut skirmish = load_skirmish(data().join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(38, 38)];
    skirmish.starting_credits = credits;
    let mut ai = load_ai(data().join("ai.ron"), &definitions).unwrap();
    let difficulty = ai
        .difficulties
        .iter()
        .position(|d| d.id == difficulty)
        .unwrap();
    let other = (difficulty + 1) % ai.difficulties.len();
    if let Some([a, b]) = mode.cadences {
        ai.difficulties[difficulty].think_interval = a;
        ai.difficulties[other].think_interval = b;
    }
    let setups = [
        PlayerSetup {
            player: PlayerId(0),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == country)
                .unwrap(),
            start: 0,
            controller: if mode.human {
                Controller::Human
            } else {
                Controller::Ai { difficulty }
            },
        },
        PlayerSetup {
            player: PlayerId(1),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == "russia")
                .unwrap(),
            start: 1,
            controller: if mode.second_ai {
                Controller::Ai { difficulty: other }
            } else {
                Controller::Human
            },
        },
    ];
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 8 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0
                };
                48 * 48
            ],
        )
        .unwrap(),
    );
    app.world_mut().insert_resource(definitions);
    setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &setups,
        &MatchOptions {
            mcv_start: false,
            crates: false,
            unit_count: Some(units),
            superweapons,
            ..MatchOptions::default()
        },
    )
    .unwrap();
    app
}

fn step(app: &mut App) -> Vec<sim::SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    let events: Vec<_> = app
        .world()
        .resource::<bevy_ecs::message::Messages<sim::SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, sim::SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    events
}
fn issue(app: &mut App, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(0),
            command,
        });
}
fn put(app: &mut App, id: &str, x: i32, y: i32) -> sim::SimId {
    sim::spawn_building(app.world_mut(), PlayerId(0), id, CellPos::new(x, y)).unwrap()
}
fn quiet(app: &mut App) {
    // A 3x3 enemy island has no free landing cell for a Chrono Legionnaire.
    // Aircraft and long-range ships can still hurt its ordinary, high-HP yard.
    // Extra HP alone cannot extend a Temporal target's data-strength erasure timer.
    let mut grid = app.world_mut().resource_mut::<Grid>();
    for y in 32..48 {
        for x in 32..48 {
            if !(38..=40).contains(&x) || !(38..=40).contains(&y) {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        buildable: false,
                        water: false,
                        ore: 0,
                    },
                );
            }
        }
    }
    let world = app.world_mut();
    let ids: Vec<_> = world
        .query::<(bevy_ecs::entity::Entity, &sim::Owner)>()
        .iter(world)
        .filter(|(_, o)| o.0 == PlayerId(1))
        .map(|(e, _)| e)
        .collect();
    for e in ids {
        world.entity_mut(e).remove::<sim::Weapon>();
        if let Some(mut hp) = world.get_mut::<sim::Health>(e) {
            hp.hit_points = 1_000_000;
        }
    }
}

fn unit_entity(app: &mut App, id: sim::SimId) -> bevy_ecs::entity::Entity {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &sim::SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .unwrap()
        .0
}

fn isolated(difficulty: &str) -> App {
    let mut app = scenario("great_britain", difficulty, 10_000, 0, true);
    app.world_mut().insert_resource(sim::AiPlayers::default());
    app
}

#[test]
fn ra8_incapable_specialist_and_stranded_ship_survivors_do_not_block_a_real_winner() {
    for kind in ["attack_dog", "terror_drone", "giant_squid", "destroyer"] {
        let mut app = scenario("great_britain", "medium", 0, 0, false);
        let cell = CellPos::new(20, 20);
        if matches!(kind, "giant_squid" | "destroyer") {
            let mut grid = app.world_mut().resource_mut::<Grid>();
            for y in 18..23 {
                for x in 18..23 {
                    grid.set_terrain(
                        CellPos::new(x, y),
                        Cell {
                            passable: false,
                            buildable: false,
                            water: true,
                            ore: 0,
                        },
                    );
                }
            }
        }
        spawn_unit(app.world_mut(), PlayerId(0), kind, cell).unwrap();
        for _ in 0..9000 {
            step(&mut app);
            if let Some(outcome) = app.world().resource::<sim::MatchState>().outcome() {
                assert!(
                    matches!(
                        outcome,
                        sim::MatchOutcome::Won {
                            winner: PlayerId(1),
                            ..
                        }
                    ),
                    "{kind}: {outcome:?}"
                );
                break;
            }
        }
        assert!(
            matches!(
                app.world().resource::<sim::MatchState>().outcome(),
                Some(sim::MatchOutcome::Won {
                    winner: PlayerId(1),
                    ..
                })
            ),
            "{kind} prevented a winner"
        );
    }
}

#[test]
fn ra8_mixed_slow_last_stand_cadences_produce_a_win_instead_of_a_simultaneous_draw() {
    for cadences in [[512, 1024], [1024, 512], [1200, 40]] {
        let mut app = scenario_mode(
            "great_britain",
            "medium",
            0,
            0,
            false,
            ScenarioMode {
                second_ai: true,
                cadences: Some(cadences),
                ..ScenarioMode::default()
            },
        );
        for _ in 0..15000 {
            step(&mut app);
            if app
                .world()
                .resource::<sim::MatchState>()
                .outcome()
                .is_some()
            {
                break;
            }
        }
        assert!(
            matches!(
                app.world().resource::<sim::MatchState>().outcome(),
                Some(sim::MatchOutcome::Won { .. })
            ),
            "{cadences:?}"
        );
    }
}

#[test]
fn ra8_rejected_empty_guard_cannot_reorder_mirage_appearance_and_crate_rng() {
    let make = || {
        let mut app = scenario_mode(
            "great_britain",
            "medium",
            10000,
            0,
            true,
            ScenarioMode {
                human: true,
                ..ScenarioMode::default()
            },
        );
        let mut rules = app.world().resource::<sim::MatchState>().rules();
        rules.crates = true;
        app.world_mut()
            .resource_mut::<sim::MatchState>()
            .set_rules(rules);
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "mirage_tank",
            CellPos::new(20, 20),
        )
        .unwrap();
        app
    };
    let mut actual = make();
    let mut control = make();
    assert!(
        actual
            .world()
            .resource::<sim::AiPlayers>()
            .players()
            .next()
            .is_none()
    );
    issue(&mut actual, Command::Guard { units: Vec::new() });
    actual.world_mut().run_schedule(SimSchedule);
    let events: Vec<_> = actual
        .world()
        .resource::<bevy_ecs::message::Messages<sim::SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. }))
    );
    control.world_mut().run_schedule(SimSchedule);
    assert_eq!(
        actual.world().resource::<sim::Crates>(),
        control.world().resource::<sim::Crates>()
    );
    assert_eq!(
        sim::state_hash(actual.world_mut()),
        sim::state_hash(control.world_mut())
    );
    assert_eq!(
        actual.world().resource::<sim::SimRng>().clone().next_u64(),
        control.world().resource::<sim::SimRng>().clone().next_u64()
    );
}

fn civilian(app: &mut App, cell: CellPos) -> sim::SimId {
    app.world_mut().resource_mut::<Grid>().set_terrain(
        cell,
        Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        },
    );
    let mut looks = vec![0; 48 * 48];
    looks[(cell.y * 48 + cell.x) as usize] = 5 << 4;
    let defs = app.world().resource::<sim::Definitions>();
    let armour = defs
        .building_combat(defs.building_kind("civilian_house").unwrap())
        .unwrap()
        .armour;
    let props = sim::Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
    let id = props.id_at(cell).unwrap();
    app.world_mut().insert_resource(props);
    id
}

#[test]
fn ra8_control_holding_specialists_do_not_pin_a_capable_ai_wave_to_one_waypoint() {
    let mut app = scenario("russia", "easy", 0, 0, false);
    quiet(&mut app);
    // A real armed aircraft beyond the ground reserve keeps the ordinary building-only
    // attack override out of this waypoint probe. Its bomb cannot reach the shore army.
    spawn_unit(app.world_mut(), PlayerId(1), "kirov", CellPos::new(38, 38)).unwrap();
    let yuris: Vec<_> = [10, 12]
        .into_iter()
        .map(|y| spawn_unit(app.world_mut(), PlayerId(0), "yuri", CellPos::new(12, y)).unwrap())
        .collect();
    let victims: Vec<_> = [10, 12]
        .into_iter()
        .map(|y| spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(14, y)).unwrap())
        .collect();
    for y in [14, 16] {
        spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(10, y)).unwrap();
    }
    let mut held = false;
    let mut targets = std::collections::BTreeSet::new();
    for tick in 0..14000 {
        step(&mut app);
        held |= victims.iter().all(|victim| {
            app.world()
                .resource::<sim::SpecialState>()
                .controlled
                .get(victim)
                .is_some_and(|(caster, _)| yuris.contains(caster))
        });
        if tick > 10000
            && let Some(target) = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .target()
        {
            targets.insert(format!("{target:?}"));
        }
    }
    assert!(
        held,
        "the actual wave must contain control-holding specialists"
    );
    for yuri in &yuris {
        let entity = unit_entity(&mut app, *yuri);
        eprintln!(
            "late Yuri {yuri:?}: {:?}, {:?}, wave={:?}, held={:?}",
            app.world().get::<sim::Health>(entity),
            app.world().get::<sim::Weapon>(entity),
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave(),
            app.world().resource::<sim::SpecialState>().controlled
        );
        assert!(
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(yuri)
        );
    }
    eprintln!("late target transitions: {targets:?}");
    assert!(
        targets.len() >= 2,
        "capable wave remained pinned by held-control markers: {targets:?}"
    );
}

#[test]
fn ra8_control_holding_yuri_completes_attack_move_and_preserves_explicit_retargeting() {
    let mut app = isolated("medium");
    let yuri = spawn_unit(app.world_mut(), PlayerId(0), "yuri", CellPos::new(20, 20)).unwrap();
    let victim = spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(22, 20)).unwrap();
    for _ in 0..20 {
        step(&mut app);
    }
    assert_eq!(
        app.world().resource::<sim::SpecialState>().controlled[&victim].0,
        yuri
    );
    issue(
        &mut app,
        Command::AttackMove {
            units: vec![yuri],
            target: CellPos::new(30, 20),
        },
    );
    for _ in 0..1000 {
        step(&mut app);
    }
    let entity = unit_entity(&mut app, yuri);
    assert_eq!(
        app.world().get::<sim::Mobile>(entity).unwrap().cell,
        CellPos::new(30, 20)
    );
    assert!(
        app.world()
            .get::<sim::Orders>(entity)
            .unwrap()
            .current
            .is_none()
    );
    assert!(
        app.world()
            .get::<sim::Weapon>(entity)
            .unwrap()
            .order
            .is_none(),
        "held control left an immortal AttackMove marker: {:?}",
        app.world().get::<sim::Weapon>(entity)
    );
    assert_eq!(
        app.world().resource::<sim::SpecialState>().controlled[&victim].0,
        yuri
    );
    let second = spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(32, 20)).unwrap();
    step(&mut app);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .controlled
            .contains_key(&second),
        "holding control must not automatically retarget"
    );
    issue(
        &mut app,
        Command::Attack {
            units: vec![yuri],
            target: second,
        },
    );
    for _ in 0..400 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::SpecialState>()
            .controlled
            .contains_key(&second)
        {
            break;
        }
    }
    assert_eq!(
        app.world().resource::<sim::SpecialState>().controlled[&second].0,
        yuri
    );
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .controlled
            .contains_key(&victim)
    );
    let entity = unit_entity(&mut app, victim);
    assert_eq!(
        app.world().get::<sim::Owner>(entity).unwrap().0,
        PlayerId(1)
    );
}

#[test]
fn ra8_yuri_cannot_acquire_a_hostile_occupied_building_as_a_control_target() {
    let mut app = isolated("medium");
    let host = civilian(&mut app, CellPos::new(13, 8));
    let occupier = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "conscript",
        CellPos::new(14, 8),
    )
    .unwrap();
    step(&mut app);
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Interact {
                unit: occupier,
                target: host,
            },
        });
    step(&mut app);
    assert_eq!(
        app.world().resource::<sim::SpecialState>().garrisons[&host].owner,
        PlayerId(1)
    );
    let yuri = spawn_unit(app.world_mut(), PlayerId(0), "yuri", CellPos::new(10, 11)).unwrap();
    step(&mut app);
    issue(
        &mut app,
        Command::Attack {
            units: vec![yuri],
            target: host,
        },
    );
    for _ in 0..8 {
        let events = step(&mut app);
        let entity = unit_entity(&mut app, yuri);
        let weapon = app.world().get::<sim::Weapon>(entity).unwrap();
        assert_ne!(
            weapon.target,
            Some(host),
            "synthetic building acquired: {weapon:?}; host {host:?}"
        );
        assert!(!events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == yuri && *target == host)), "Yuri {yuri:?} fired an impossible control shot at occupied host {host:?}: {events:?}");
        assert!(
            !app.world()
                .resource::<sim::SpecialState>()
                .controlled
                .contains_key(&host)
        );
    }
}

#[test]
fn ra8_damaged_own_garrison_is_no_economy_building_and_depleted_base_recovers_its_infantry() {
    for credits in [0, 100000] {
        let mut app = scenario("great_britain", "medium", credits, 0, false);
        let host = civilian(&mut app, CellPos::new(13, 8));
        let gi = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(12, 8)).unwrap();
        step(&mut app);
        issue(
            &mut app,
            Command::Interact {
                unit: gi,
                target: host,
            },
        );
        step(&mut app);
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .cargo
                .contains_key(&host)
        );
        if credits > 0 {
            let attacker =
                spawn_unit(app.world_mut(), PlayerId(1), "grizzly", CellPos::new(17, 8)).unwrap();
            step(&mut app); // Establish the new observer's sight of the occupied prop.
            app.world_mut()
                .resource_mut::<CommandQueue>()
                .push(sim::IssuedCommand {
                    player: PlayerId(1),
                    command: Command::Attack {
                        units: vec![attacker],
                        target: host,
                    },
                });
            let mut damaged = false;
            for _ in 0..100 {
                step(&mut app);
                damaged |= app
                    .world()
                    .resource::<sim::Props>()
                    .get(host)
                    .is_some_and(|(_, p)| p.hit_points < 300);
            }
            assert!(damaged);
        } else {
            let mut recovered = false;
            for _ in 0..2000 {
                let events = step(&mut app);
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e, sim::SimEvent::BuildingSold { .. })),
                    "sold before recovering the garrison"
                );
                let world = app.world_mut();
                recovered = world.query::<&sim::SimId>().iter(world).any(|id| *id == gi);
                if recovered {
                    break;
                }
            }
            assert!(
                recovered,
                "garrison infantry were ignored during last stand"
            );
        }
    }
}

#[test]
fn ra8_endgame_preserves_temporal_mind_control_and_active_naval_travel() {
    let mut temporal = scenario("great_britain", "medium", 0, 0, false);
    spawn_unit(
        temporal.world_mut(),
        PlayerId(0),
        "chrono_legionnaire",
        CellPos::new(20, 20),
    )
    .unwrap();
    for _ in 0..2000 {
        assert!(
            !step(&mut temporal)
                .iter()
                .any(|e| matches!(e, sim::SimEvent::BuildingSold { .. }))
        );
    }
    let mut control = scenario("great_britain", "medium", 0, 0, false);
    let yuri = spawn_unit(
        control.world_mut(),
        PlayerId(0),
        "yuri",
        CellPos::new(15, 8),
    )
    .unwrap();
    let victim = spawn_unit(
        control.world_mut(),
        PlayerId(1),
        "rhino",
        CellPos::new(18, 8),
    )
    .unwrap();
    let e = unit_entity(&mut control, victim);
    control.world_mut().entity_mut(e).remove::<sim::Weapon>();
    let mut captured = false;
    for _ in 0..100 {
        step(&mut control);
        captured |= control
            .world()
            .resource::<sim::SpecialState>()
            .controlled
            .get(&victim)
            .is_some_and(|(caster, _)| *caster == yuri);
    }
    assert!(captured, "legitimate mind control opportunity was lost");
    let mut travel = scenario("great_britain", "medium", 0, 0, false);
    for y in 2..48 {
        for x in 18..23 {
            travel.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0,
                },
            );
        }
    }
    let ship = spawn_unit(
        travel.world_mut(),
        PlayerId(0),
        "destroyer",
        CellPos::new(20, 3),
    )
    .unwrap();
    for _ in 0..3800 {
        step(&mut travel);
    }
    issue(
        &mut travel,
        Command::Move {
            units: vec![ship],
            target: CellPos::new(20, 46),
        },
    );
    for _ in 0..500 {
        assert!(
            !step(&mut travel)
                .iter()
                .any(|e| matches!(e, sim::SimEvent::BuildingSold { .. })),
            "liquidated during active naval travel"
        );
    }
}

#[test]
fn ra8_lone_yuri_preserves_only_seen_reachable_controllable_building_damaging_recruits() {
    for (victim_kind, isolated, useful) in [
        ("conscript", false, true),
        ("gi", false, true),
        ("tanya", false, false),
        ("attack_dog", false, false),
        ("conscript", true, false),
    ] {
        let mut app = scenario("russia", "easy", 0, 0, false);
        if isolated {
            for y in 0..48 {
                for x in 17..=22 {
                    app.world_mut().resource_mut::<Grid>().set_terrain(
                        CellPos::new(x, y),
                        Cell {
                            passable: false,
                            water: true,
                            buildable: false,
                            ore: 0,
                        },
                    );
                }
            }
        }
        let yuri = spawn_unit(app.world_mut(), PlayerId(0), "yuri", CellPos::new(15, 8)).unwrap();
        let victim = spawn_unit(
            app.world_mut(),
            PlayerId(1),
            victim_kind,
            CellPos::new(24, 8),
        )
        .unwrap();
        step(&mut app);
        assert!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(24, 8))
        );
        let mut sold = false;
        let mut captured = false;
        for _ in 0..4200 {
            let events = step(&mut app);
            sold |= events.iter().any(|e| {
                matches!(
                    e,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    }
                )
            });
            captured |= app
                .world()
                .resource::<sim::SpecialState>()
                .controlled
                .get(&victim)
                .is_some_and(|(caster, _)| *caster == yuri);
            if useful {
                assert!(
                    !sold,
                    "liquidated before a real {victim_kind} capture opportunity"
                );
            } else if sold {
                break;
            }
        }
        if useful {
            // Prove the retained opportunity through normal commands without imposing a
            // new target-selection policy or a deadline on the AI's waypoint exploration.
            let building = sim::spawn_building(
                app.world_mut(),
                PlayerId(1),
                "tesla_reactor",
                CellPos::new(26, 11),
            )
            .unwrap();
            issue(
                &mut app,
                Command::Attack {
                    units: vec![yuri],
                    target: victim,
                },
            );
            for _ in 0..2000 {
                step(&mut app);
                captured |= app
                    .world()
                    .resource::<sim::SpecialState>()
                    .controlled
                    .get(&victim)
                    .is_some_and(|(caster, _)| *caster == yuri);
                if captured {
                    break;
                }
            }
            assert!(captured, "ordinary control must capture {victim_kind}");
            let building_entity = unit_entity(&mut app, building);
            let initial_hp = app
                .world()
                .get::<sim::Health>(building_entity)
                .unwrap()
                .hit_points;
            issue(
                &mut app,
                Command::Attack {
                    units: vec![victim],
                    target: building,
                },
            );
            let mut fired = false;
            for _ in 0..100 {
                fired |= step(&mut app).iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == victim && *target == building));
            }
            assert!(
                fired,
                "controlled {victim_kind} must help attack a real building"
            );
            assert!(
                app.world()
                    .get::<sim::Health>(building_entity)
                    .is_none_or(|hp| hp.hit_points < initial_hp)
            );
        }
        assert_eq!(captured, useful, "{victim_kind}, isolated={isolated}");
        assert_eq!(sold, !useful, "{victim_kind}, isolated={isolated}");
    }
}

#[test]
fn ra8_artillery_uses_the_surface_tank_line_when_aircraft_share_its_wave() {
    let mut app = scenario("russia", "medium", 0, 0, false);
    quiet(&mut app);
    let tank = spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(8, 18)).unwrap();
    let artillery = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "v3_launcher",
        CellPos::new(8, 20),
    )
    .unwrap();
    let aircraft = spawn_unit(app.world_mut(), PlayerId(0), "kirov", CellPos::new(8, 22)).unwrap();
    spawn_unit(app.world_mut(), PlayerId(0), "spy", CellPos::new(30, 8)).unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "rocketeer",
        CellPos::new(24, 14),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(34, 8),
    )
    .unwrap();
    for _ in 0..10_000 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&artillery)
        {
            break;
        }
    }
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    for id in [tank, artillery, aircraft] {
        assert!(
            controllers
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&id)
        );
    }
    // Pause decisions only while ordinary commands arrange the controlled geometry. All
    // controller memory/wave state is retained, then the real scheduled AI turn resumes.
    issue(
        &mut app,
        Command::Stop {
            units: vec![tank, artillery, aircraft],
        },
    );
    for (id, target) in [
        (tank, CellPos::new(14, 8)),
        (artillery, CellPos::new(12, 8)),
        (aircraft, CellPos::new(22, 8)),
    ] {
        issue(
            &mut app,
            Command::Move {
                units: vec![id],
                target,
            },
        );
    }
    for _ in 0..2000 {
        step(&mut app);
        if [tank, artillery, aircraft].iter().all(|id| {
            let entity = unit_entity(&mut app, *id);
            app.world()
                .get::<sim::Orders>(entity)
                .unwrap()
                .current
                .is_none()
        }) {
            break;
        }
    }
    for (id, target) in [
        (tank, CellPos::new(14, 8)),
        (artillery, CellPos::new(12, 8)),
        (aircraft, CellPos::new(22, 8)),
    ] {
        let entity = unit_entity(&mut app, id);
        assert_eq!(app.world().get::<sim::Mobile>(entity).unwrap().cell, target);
    }
    while !app.world().resource::<SimTick>().0.is_multiple_of(20) {
        step(&mut app);
    }
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(24, 14))
    );
    app.world_mut().insert_resource(controllers);
    step(&mut app);
    let command = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .filter_map(|issued| match &issued.command {
            Command::AttackMove { units, target } if units.contains(&artillery) => Some(*target),
            _ => None,
        })
        .last();
    assert_eq!(
        command,
        Some(CellPos::new(11, 8)),
        "V3 must stand behind the Rhino, not the nearer Kirov"
    );
}

#[test]
fn ra8_special_weapon_rof_scales_psi_eruption_parasite_repair_and_occupation_only() {
    for (difficulty, pm) in [("easy", 1200u32), ("medium", 1000), ("brutal", 800)] {
        for (id, x) in [("yuri", 20), ("desolator", 24)] {
            let mut app = isolated(difficulty);
            let unit = spawn_unit(app.world_mut(), PlayerId(0), id, CellPos::new(x, 20)).unwrap();
            let defs = app.world().resource::<sim::Definitions>().clone();
            let terms = defs.unit_combat(defs.unit_kind(id).unwrap()).unwrap();
            let reload = defs.weapon(terms.secondary.unwrap()).unwrap().reload * pm / 1000;
            issue(&mut app, Command::Deploy { unit });
            step(&mut app);
            let entity = unit_entity(&mut app, unit);
            if id == "yuri" {
                assert_eq!(
                    app.world().get::<sim::Weapon>(entity).unwrap().cooldown + 1,
                    reload,
                    "{difficulty} psi"
                );
                // Deploy animation is a global delay, independent of the weapon ROF.
                assert_eq!(
                    app.world().resource::<sim::SpecialState>().deployed[&unit]
                        - app.world().resource::<SimTick>().0,
                    199
                );
            } else {
                assert_eq!(
                    app.world().resource::<sim::SpecialState>().eruption_next[&unit] + 1
                        - app.world().resource::<SimTick>().0,
                    u64::from(reload),
                    "{difficulty} eruption"
                );
            }
        }
        let mut app = isolated(difficulty);
        let drone = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "terror_drone",
            CellPos::new(20, 20),
        )
        .unwrap();
        let host = spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(21, 20)).unwrap();
        let e = unit_entity(&mut app, host);
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        app.world_mut()
            .get_mut::<sim::Health>(e)
            .unwrap()
            .hit_points = 10000;
        step(&mut app);
        issue(
            &mut app,
            Command::Attack {
                units: vec![drone],
                target: host,
            },
        );
        step(&mut app);
        let first = app.world().resource::<sim::SpecialState>().parasites[&host].due;
        let defs = app.world().resource::<sim::Definitions>();
        let reload = defs
            .weapon(
                defs.unit_combat(defs.unit_kind("terror_drone").unwrap())
                    .unwrap()
                    .weapon
                    .unwrap(),
            )
            .unwrap()
            .reload
            * pm
            / 1000;
        assert_eq!(
            first + 1 - app.world().resource::<SimTick>().0,
            u64::from(reload),
            "{difficulty} attachment"
        );
        while app.world().resource::<SimTick>().0 <= first {
            step(&mut app);
        }
        assert_eq!(
            app.world().resource::<sim::SpecialState>().parasites[&host].due - first,
            u64::from(reload),
            "{difficulty} parasite repeat"
        );

        let mut app = isolated(difficulty);
        let ifv = spawn_unit(app.world_mut(), PlayerId(0), "ifv", CellPos::new(20, 20)).unwrap();
        let engineer = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(19, 20),
        )
        .unwrap();
        let tank = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "grizzly",
            CellPos::new(21, 21),
        )
        .unwrap();
        step(&mut app);
        issue(
            &mut app,
            Command::Interact {
                unit: engineer,
                target: ifv,
            },
        );
        step(&mut app);
        let e = unit_entity(&mut app, tank);
        app.world_mut()
            .get_mut::<sim::Health>(e)
            .unwrap()
            .hit_points = 10;
        step(&mut app);
        let e = unit_entity(&mut app, ifv);
        let weapon = app.world().get::<sim::Weapon>(e).unwrap();
        let reload = app
            .world()
            .resource::<sim::Definitions>()
            .weapon(weapon.kind)
            .unwrap()
            .reload
            * pm
            / 1000;
        assert_eq!(weapon.cooldown + 1, reload, "{difficulty} repair");

        let mut app = isolated(difficulty);
        let cell = CellPos::new(20, 20);
        app.world_mut().resource_mut::<Grid>().set_terrain(
            cell,
            Cell {
                passable: false,
                buildable: false,
                water: false,
                ore: 0,
            },
        );
        let mut looks = vec![0; 48 * 48];
        looks[20 * 48 + 20] = 5 << 4;
        let armour = app
            .world()
            .resource::<sim::Definitions>()
            .building_combat(
                app.world()
                    .resource::<sim::Definitions>()
                    .building_kind("civilian_house")
                    .unwrap(),
            )
            .unwrap()
            .armour;
        let props = sim::Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
        app.world_mut().insert_resource(props);
        let host = app.world().resource::<sim::Props>().id_at(cell).unwrap();
        let gi = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(19, 20)).unwrap();
        let victim =
            spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(23, 20)).unwrap();
        let e = unit_entity(&mut app, victim);
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        step(&mut app);
        issue(
            &mut app,
            Command::Interact {
                unit: gi,
                target: host,
            },
        );
        let mut fired = step(&mut app)
            .iter()
            .any(|e| matches!(e,sim::SimEvent::WeaponFired {attacker,..} if *attacker==host));
        for _ in 0..40 {
            if fired {
                break;
            }
            fired = step(&mut app)
                .iter()
                .any(|e| matches!(e,sim::SimEvent::WeaponFired {attacker,..} if *attacker==host));
        }
        assert!(fired, "{difficulty} occupation did not fire");
        let defs = app.world().resource::<sim::Definitions>();
        let reload = defs
            .weapon(defs.weapon_kind("allied_occupy").unwrap())
            .unwrap()
            .reload
            * pm
            / 1000;
        assert_eq!(
            app.world().resource::<sim::SpecialState>().garrisons[&host].cooldown,
            reload.max(1),
            "{difficulty} occupation"
        );
    }
}

#[test]
fn ra8_every_country_has_obtainable_full_tech_national_and_conditional_lists() {
    let defs = load_ruleset(data()).unwrap();
    let skirmish = load_skirmish(data().join("skirmish.ron"), &defs).unwrap();
    let ai = load_ai(data().join("ai.ron"), &defs).unwrap();
    assert!(ai.rules_difficulty);
    assert_eq!(ai.plans.len(), 11);
    for faction in &skirmish.factions {
        let plan = ai.plan_for(faction).unwrap();
        let tree = sim::country_tree(&defs, faction.country.unwrap());
        assert!(
            plan.build_order
                .iter()
                .all(|&k| tree.contains(&sim::ProductionItem::Building(k)))
        );
        assert!(
            plan.army
                .iter()
                .chain(plan.support.iter())
                .all(|&k| tree.contains(&sim::ProductionItem::Unit(k)))
        );
        assert!(plan.build_order.iter().any(|&k| {
            defs.metadata(sim::ProductionItem::Building(k))
                .unwrap()
                .radar
        }));
        assert_eq!(
            plan.build_order
                .iter()
                .filter(|&&k| defs.building(k).unwrap().credits_per_ore.is_some())
                .count(),
            2
        );
        let special = match faction.id.as_str() {
            "america" => "american_airforce_command",
            "france" => "grand_cannon",
            "germany" => "tank_destroyer",
            "great_britain" => "sniper",
            "korea" => "black_eagle",
            "russia" => "tesla_tank",
            "iraq" => "desolator",
            "cuba" => "terrorist",
            "libya" => "demolition_truck",
            _ => panic!("country"),
        };
        let contains = defs
            .unit_kind(special)
            .is_some_and(|k| plan.army.contains(&k))
            || defs
                .building_kind(special)
                .is_some_and(|k| plan.build_order.contains(&k) || plan.defences.contains(&k));
        assert!(contains, "{}: {special}", faction.id);
    }
}

#[test]
fn ra8_medium_country_defence_counts_national_and_later_army_tiers_are_produced() {
    for (country, wanted) in [("france", 20usize), ("russia", 22usize)] {
        let mut app = scenario(country, "medium", 100000, 0, false);
        quiet(&mut app);
        let air = spawn_unit(app.world_mut(), PlayerId(1), "kirov", CellPos::new(15, 8)).unwrap();
        let e = unit_entity(&mut app, air);
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        for _ in 0..21 {
            step(&mut app);
        }
        app.world_mut()
            .get_mut::<sim::Health>(e)
            .unwrap()
            .hit_points = 0;
        let defs = app.world().resource::<sim::Definitions>().clone();
        let plan = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .plan()
            .clone();
        let mut observed_aircraft = false;
        let mut advanced = false;
        let mut national = country != "france";
        let mut count = 0;
        for tick in 0..45000 {
            step(&mut app);
            if tick % 100 != 0 {
                continue;
            }
            let world = app.world_mut();
            for (o, u) in world
                .query::<(&sim::Owner, &sim::Unit)>()
                .iter(world)
                .filter(|(o, _)| o.0 == PlayerId(0))
            {
                let _ = o;
                observed_aircraft |= defs.domain(u.kind) == sim::Domain::Air;
                advanced |= defs.domain(u.kind) == sim::Domain::Land
                    && defs
                        .metadata(sim::ProductionItem::Unit(u.kind))
                        .is_some_and(|m| m.tech_level >= 6);
            }
            let owned: Vec<_> = world
                .query::<(&sim::Owner, &sim::Building)>()
                .iter(world)
                .filter(|(o, _)| o.0 == PlayerId(0))
                .map(|(_, b)| b.kind)
                .collect();
            count = owned.iter().filter(|k| plan.defences.contains(k)).count();
            national |= owned
                .iter()
                .any(|&k| defs.building(k).unwrap().id == "grand_cannon");
            let player = world.resource::<sim::Players>().get(PlayerId(0)).unwrap();
            assert!(player.docked_aircraft() <= player.docks(), "dock capacity");
            if count == wanted && national && observed_aircraft && advanced {
                break;
            }
        }
        assert_eq!(count, wanted, "{country}");
        assert!(
            national && observed_aircraft && advanced,
            "{country}: national={national}, aircraft={observed_aircraft}, advanced={advanced}"
        );
    }
}

#[test]
fn ra8_strategic_strikes_weight_known_clusters_and_do_not_target_unseen_bases() {
    for reveal in [false, true] {
        let mut app = scenario("iraq", "brutal", 100000, 0, true);
        quiet(&mut app);
        put(&mut app, "nuclear_reactor", 2, 2);
        let source = put(&mut app, "nuclear_silo", 2, 5);
        if reveal {
            put(&mut app, "spy_satellite", 2, 12);
        }
        for (id, x, y) in [
            ("power_plant", 25, 10),
            ("power_plant", 27, 10),
            ("war_factory", 25, 30),
        ] {
            sim::spawn_building(app.world_mut(), PlayerId(1), id, CellPos::new(x, y)).unwrap();
        }
        let defs = app.world().resource::<sim::Definitions>();
        let charge = defs
            .metadata(sim::ProductionItem::Building(
                defs.building_kind("nuclear_silo").unwrap(),
            ))
            .unwrap()
            .cooldown;
        step(&mut app); // The real built/owner announcement initializes charge to zero.
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .charge
            .insert(source, charge);
        let mut target = None;
        for _ in 0..60 {
            for event in step(&mut app) {
                if let sim::SimEvent::StrategicActivated {
                    building,
                    target: cell,
                    ..
                } = event
                    && building == source
                {
                    target = Some(cell);
                }
            }
        }
        if reveal {
            assert!(
                matches!(target, Some(CellPos { x: 25 | 27, y: 10 })),
                "{target:?}"
            );
        } else {
            assert_eq!(target, None);
        }
    }
}

#[test]
fn ra8_curtain_and_explicit_chronoshift_support_actual_tank_attack_waves() {
    for (country, ability, tank) in [
        ("russia", "iron_curtain", "rhino"),
        ("great_britain", "chronosphere", "grizzly"),
    ] {
        let mut app = scenario(country, "brutal", 0, 0, true);
        let world = app.world_mut();
        for (owner, mut hp) in world
            .query::<(&sim::Owner, &mut sim::Health)>()
            .iter_mut(world)
        {
            if owner.0 == PlayerId(1) {
                hp.hit_points = 1_000_000;
            }
        }
        put(&mut app, "nuclear_reactor", 2, 2);
        put(&mut app, "spy_satellite", 2, 12);
        let source = put(&mut app, ability, 2, 5);
        let mut tanks = Vec::new();
        for (x, y) in [(14, 14), (15, 14), (14, 15), (15, 15)] {
            tanks.push(spawn_unit(app.world_mut(), PlayerId(0), tank, CellPos::new(x, y)).unwrap());
        }
        step(&mut app);
        let defs = app.world().resource::<sim::Definitions>();
        let charge = defs
            .metadata(sim::ProductionItem::Building(
                defs.building_kind(ability).unwrap(),
            ))
            .unwrap()
            .cooldown;
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .charge
            .insert(source, charge);
        let mut activated = false;
        let mut explicit = false;
        for _ in 0..6000 {
            let events = step(&mut app);
            explicit|=app.world().resource::<CommandQueue>().iter().any(|issued|matches!(issued.command,Command::Chronoshift {building,..} if building==source));
            if events.iter().any(|event|matches!(event,sim::SimEvent::StrategicActivated {building,..} if *building==source)) {activated=true;break;}
        }
        assert!(activated, "{ability} did not support its actual wave");
        if ability == "chronosphere" {
            assert!(
                explicit,
                "automation did not use the explicit source/target command"
            );
        } else {
            assert!(tanks.iter().any(|id| {
                app.world()
                    .resource::<sim::SpecialState>()
                    .protected
                    .contains_key(id)
            }));
        }
    }
}

#[test]
fn ra8_difficulty_start_credit_build_and_reload_use_real_house_rules() {
    let mut first_hits = Vec::new();
    for (difficulty, credits, rof, build, virtuals) in [
        ("easy", 10000, 1200, 1000, 0),
        ("medium", 10000, 1000, 1000, 2),
        ("brutal", 50000, 800, 800, 4),
    ] {
        let mut app = scenario("great_britain", difficulty, 10000, 0, true);
        let p = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        assert_eq!(
            (
                p.credits(),
                p.ai_reload_pm(),
                p.ai_build_time_pm(),
                p.ai_virtual_purifiers()
            ),
            (credits, rof, build, virtuals)
        );
        assert_eq!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(1))
                .unwrap()
                .credits(),
            10000
        );
        app.world_mut().insert_resource(sim::AiPlayers::default());
        put(&mut app, "power_plant", 2, 2);
        let defs = app.world().resource::<sim::Definitions>().clone();
        let power = defs.building_kind("power_plant").unwrap();
        issue(
            &mut app,
            Command::QueueProduction {
                item: sim::ProductionItem::Building(power),
            },
        );
        for _ in 0..100 {
            step(&mut app);
        }
        let progress = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .next()
            .unwrap()
            .1[0]
            .progress;
        assert_eq!(progress, 100000 / build);
        let attacker = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "grizzly",
            CellPos::new(20, 20),
        )
        .unwrap();
        let victim =
            spawn_unit(app.world_mut(), PlayerId(1), "rhino", CellPos::new(22, 20)).unwrap();
        let world = app.world_mut();
        let e = world
            .query::<(bevy_ecs::entity::Entity, &sim::SimId)>()
            .iter(world)
            .find(|(_, id)| **id == victim)
            .unwrap()
            .0;
        world.entity_mut(e).remove::<sim::Weapon>();
        world.get_mut::<sim::Health>(e).unwrap().hit_points = 100000;
        step(&mut app);
        issue(
            &mut app,
            Command::Attack {
                units: vec![attacker],
                target: victim,
            },
        );
        let mut shots = Vec::new();
        let mut first_hit = None;
        for tick in 0..300 {
            if step(&mut app).iter().any(|e|matches!(e,sim::SimEvent::WeaponFired { attacker:found,.. } if *found==attacker)) {shots.push(tick);}
            let hp = app.world().get::<sim::Health>(e).unwrap().hit_points;
            if hp < 100000 && first_hit.is_none() {
                first_hit = Some(100000 - hp);
            }
        }
        first_hits.push(first_hit.expect("the shot landed"));
        let reload = defs
            .unit_combat(defs.unit_kind("grizzly").unwrap())
            .unwrap()
            .weapon
            .and_then(|k| defs.weapon(k))
            .unwrap()
            .reload;
        assert!(shots.len() >= 2, "{difficulty}: {shots:?}");
        assert!(
            shots.windows(2).all(|w| w[1] - w[0] == reload * rof / 1000),
            "{difficulty}: {shots:?}"
        );
    }
    assert!(
        first_hits.iter().all(|hit| *hit == first_hits[0]),
        "difficulty may change ROF, never firepower or armour: {first_hits:?}"
    );
}

#[test]
fn ra8_virtual_and_physical_purifiers_add_independent_shares_of_base_delivery() {
    for (difficulty, virtuals) in [("easy", 0), ("medium", 2), ("brutal", 4)] {
        for physical in [false, true] {
            let mut app = scenario("great_britain", difficulty, 10000, 0, true);
            app.world_mut().insert_resource(sim::AiPlayers::default());
            put(&mut app, "nuclear_reactor", 2, 2);
            put(&mut app, "refinery", 10, 25);
            if physical {
                put(&mut app, "ore_purifier", 20, 25);
            }
            app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(15, 25),
                Cell {
                    passable: true,
                    buildable: false,
                    water: false,
                    ore: 20,
                },
            );
            let miner = spawn_unit(
                app.world_mut(),
                PlayerId(0),
                "chrono_miner",
                CellPos::new(15, 25),
            )
            .unwrap();
            step(&mut app);
            issue(
                &mut app,
                Command::Harvest {
                    units: vec![miner],
                    target: CellPos::new(15, 25),
                },
            );
            let before = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .credits();
            let mut delivered = None;
            for _ in 0..1500 {
                let events = step(&mut app);
                if let Some(value) = events.iter().find_map(|e| match e {
                    sim::SimEvent::OreDelivered {
                        harvester, credits, ..
                    } if *harvester == miner => Some(*credits),
                    _ => None,
                }) {
                    delivered = Some(value);
                    break;
                }
            }
            assert_eq!(delivered, Some(500));
            assert_eq!(
                app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits()
                    - before,
                500 + 500 * 25 * (virtuals + i64::from(physical)) / 100
            );
        }
    }
}

#[test]
fn ra8_engineer_is_produced_and_captures_seen_tech_through_ordinary_commands() {
    let mut app = scenario("great_britain", "medium", 100000, 0, true);
    quiet(&mut app);
    put(&mut app, "barracks", 2, 14);
    let derrick = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "oil_derrick",
        CellPos::new(13, 8),
    )
    .unwrap();
    let mut trained = false;
    let engineer = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("engineer")
        .unwrap();
    for _ in 0..3000 {
        let events = step(&mut app);
        let spawned: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                sim::SimEvent::UnitSpawned { unit, .. } => Some(*unit),
                _ => None,
            })
            .collect();
        let world = app.world_mut();
        trained |= world
            .query::<(&sim::SimId, &sim::Unit)>()
            .iter(world)
            .any(|(id, unit)| spawned.contains(id) && unit.kind == engineer);
        if world
            .query::<(&sim::SimId, &sim::Owner)>()
            .iter(world)
            .any(|(id, owner)| *id == derrick && owner.0 == PlayerId(0))
        {
            assert!(trained);
            return;
        }
    }
    panic!("Engineer did not capture the visible derrick");
}

#[test]
fn ra8_building_and_depot_repairs_use_paid_production_paths() {
    let mut app = scenario("great_britain", "medium", 100000, 0, true);
    quiet(&mut app);
    let building = put(&mut app, "power_plant", 2, 2);
    put(&mut app, "service_depot", 2, 20);
    let vehicle = spawn_unit(app.world_mut(), PlayerId(0), "grizzly", CellPos::new(5, 20)).unwrap();
    let world = app.world_mut();
    for (id, mut hp) in world
        .query::<(&sim::SimId, &mut sim::Health)>()
        .iter_mut(world)
    {
        if *id == building || *id == vehicle {
            hp.hit_points = 10;
        }
    }
    let mut docked = false;
    for _ in 0..800 {
        step(&mut app);
        docked |= app
            .world()
            .resource::<sim::SpecialState>()
            .docked
            .contains_key(&vehicle);
    }
    let world = app.world_mut();
    let health: std::collections::BTreeMap<_, _> = world
        .query::<(&sim::SimId, &sim::Health)>()
        .iter(world)
        .map(|(id, h)| (*id, h.hit_points))
        .collect();
    assert!(health[&building] > 10);
    assert!(
        docked && health[&vehicle] > 10,
        "docked={docked}, health={health:?}"
    );
}

#[test]
fn ra8_full_tree_builds_and_fires_strategic_weapons_and_off_keeps_american_paradrop() {
    for (country, on) in [("great_britain", true), ("iraq", true), ("america", false)] {
        let mut app = scenario(country, "brutal", 100000, 0, on);
        quiet(&mut app);
        if country == "america" {
            // A second real enemy base has open terrain behind it for a legal paradrop.
            let reserve = sim::spawn_building(
                app.world_mut(),
                PlayerId(1),
                "soviet_construction_yard",
                CellPos::new(8, 38),
            )
            .unwrap();
            let e = unit_entity(&mut app, reserve);
            app.world_mut()
                .get_mut::<sim::Health>(e)
                .unwrap()
                .hit_points = 1_000_000;
        }
        let defs = app.world().resource::<sim::Definitions>().clone();
        let mut major = false;
        let mut para = false;
        let mut types = std::collections::BTreeMap::new();
        for tick in 0..42000 {
            for e in step(&mut app) {
                if let sim::SimEvent::StrategicActivated { player, role, .. } = e
                    && player == PlayerId(0)
                {
                    major |= role.major_superweapon();
                    para |= role == sim::Role::Paradrop;
                }
            }
            if tick % 100 == 0 {
                types.clear();
                let world = app.world_mut();
                for (owner, b) in world
                    .query::<(&sim::Owner, &sim::Building)>()
                    .iter(world)
                    .filter(|(o, _)| o.0 == PlayerId(0))
                {
                    let _ = owner;
                    *types
                        .entry(defs.building(b.kind).unwrap().id.clone())
                        .or_insert(0usize) += 1;
                }
                let required = if country == "iraq" {
                    [
                        "radar_tower",
                        "soviet_battle_lab",
                        "soviet_service_depot",
                        "cloning_vats",
                    ]
                } else {
                    [
                        if country == "america" {
                            "american_airforce_command"
                        } else {
                            "airforce_command"
                        },
                        "tech_center",
                        "service_depot",
                        "ore_purifier",
                    ]
                };
                if required.iter().all(|id| types.contains_key(*id)) && (major || (!on && para)) {
                    break;
                }
            }
        }
        eprintln!("{country}, SW={on}: {types:?}, major={major}, para={para}");
        assert_eq!(major, on, "{country}: {types:?}");
        if country == "america" {
            assert!(para);
            assert!(!types.keys().any(|id| {
                defs.building_kind(id)
                    .and_then(|k| defs.metadata(sim::ProductionItem::Building(k)))
                    .unwrap()
                    .role
                    .major_superweapon()
            }));
        }
        let refinery = if country == "iraq" {
            "soviet_refinery"
        } else {
            "refinery"
        };
        assert!(types.get(refinery).copied().unwrap_or(0) >= 2, "{types:?}");
        let lab = if country == "iraq" {
            "soviet_battle_lab"
        } else {
            "tech_center"
        };
        assert!(types.contains_key(lab));
    }
}

#[test]
fn ra8_near_base_occupier_enters_a_real_civilian_garrison() {
    let mut app = scenario("great_britain", "medium", 100000, 0, false);
    quiet(&mut app);
    let cell = CellPos::new(13, 8);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        cell,
        Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        },
    );
    let mut looks = vec![0; 48 * 48];
    looks[8 * 48 + 13] = 5 << 4;
    let defs = app.world().resource::<sim::Definitions>();
    let armour = defs
        .building_combat(defs.building_kind("civilian_house").unwrap())
        .unwrap()
        .armour;
    let props = sim::Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
    let host = props.id_at(cell).unwrap();
    app.world_mut().insert_resource(props);
    let infantry = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(12, 8)).unwrap();
    for _ in 0..300 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::SpecialState>()
            .cargo
            .get(&host)
            .is_some_and(|c| c.iter().any(|p| p.id == infantry))
        {
            return;
        }
    }
    panic!("near-base GI did not garrison");
}

#[test]
fn ra8_air_observation_reserves_then_trains_an_anti_air_unit() {
    let mut app = scenario("great_britain", "medium", 100000, 0, false);
    quiet(&mut app);
    let defs = app.world().resource::<sim::Definitions>().clone();
    let ifv = defs.unit_kind("ifv").unwrap();
    for _ in 0..2500 {
        step(&mut app);
    }
    assert!(
        !app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .observed_aircraft()
    );
    assert!(
        !app.world_mut()
            .query::<&sim::Unit>()
            .iter(app.world())
            .any(|u| u.kind == ifv)
    );
    spawn_unit(app.world_mut(), PlayerId(1), "kirov", CellPos::new(13, 10)).unwrap();
    quiet(&mut app);
    for _ in 0..4000 {
        step(&mut app);
        if app
            .world_mut()
            .query::<(&sim::Owner, &sim::Unit)>()
            .iter(app.world())
            .any(|(o, u)| o.0 == PlayerId(0) && u.kind == ifv)
        {
            assert!(
                app.world()
                    .resource::<sim::AiPlayers>()
                    .controller(PlayerId(0))
                    .unwrap()
                    .observed_aircraft()
            );
            return;
        }
    }
    panic!("observed air did not produce anti-air");
}

#[test]
fn ra8_an_actual_loss_to_an_unseen_mirage_requests_a_detector() {
    let mut app = scenario("great_britain", "medium", 100000, 0, false);
    quiet(&mut app);
    put(&mut app, "barracks", 2, 14);
    let victim = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(13, 8)).unwrap();
    let world = app.world_mut();
    for (id, mut hp) in world
        .query::<(&sim::SimId, &mut sim::Health)>()
        .iter_mut(world)
    {
        if *id == victim {
            hp.hit_points = 1;
        }
    }
    // Establish owned-unit memory before introducing the concealed attacker.
    step(&mut app);
    spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "mirage_tank",
        CellPos::new(15, 8),
    )
    .unwrap();
    let dog = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("attack_dog")
        .unwrap();
    for _ in 0..2500 {
        step(&mut app);
        let world = app.world_mut();
        if world
            .query::<(&sim::Owner, &sim::Unit)>()
            .iter(world)
            .any(|(o, u)| o.0 == PlayerId(0) && u.kind == dog)
        {
            assert!(
                world
                    .resource::<sim::AiPlayers>()
                    .controller(PlayerId(0))
                    .unwrap()
                    .needs_detector()
            );
            return;
        }
    }
    panic!("unseen lethal attack did not produce a detector");
}

#[test]
fn ra8_navy_requires_reachable_placement_water_and_an_enemy_hurtable_from_that_sea() {
    for water in [0, 1, 2, 3] {
        let mut app = scenario("great_britain", "brutal", 100000, 0, false);
        quiet(&mut app);
        if water > 0 {
            let mut grid = app.world_mut().resource_mut::<Grid>();
            for y in 0..if water == 2 { 12 } else { 48 } {
                for x in 16..=24 {
                    grid.set_terrain(
                        CellPos::new(x, y),
                        Cell {
                            passable: false,
                            buildable: false,
                            water: true,
                            ore: 0,
                        },
                    );
                }
            }
            if water == 3 {
                for y in 11..14 {
                    for x in 2..5 {
                        grid.set_terrain(
                            CellPos::new(x, y),
                            Cell {
                                passable: false,
                                buildable: false,
                                water: true,
                                ore: 0,
                            },
                        );
                    }
                }
            }
        }
        let defs = app.world().resource::<sim::Definitions>().clone();
        let yard = defs.building_kind("naval_shipyard").unwrap();
        let mut ships = false;
        let mut built = false;
        let mut fired = false;
        for tick in 0..24000 {
            let events = step(&mut app);
            let world = app.world_mut();
            let naval_ids: std::collections::BTreeSet<_> = world
                .query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
                .iter(world)
                .filter(|(_, owner, unit)| {
                    owner.0 == PlayerId(0) && defs.domain(unit.kind) == sim::Domain::Naval
                })
                .map(|(id, _, _)| *id)
                .collect();
            fired|=events.iter().any(|event|matches!(event,sim::SimEvent::WeaponFired {attacker,..} if naval_ids.contains(attacker)));
            if tick % 100 == 0 {
                let world = app.world_mut();
                built |= world
                    .query::<(&sim::Owner, &sim::Building)>()
                    .iter(world)
                    .any(|(o, b)| o.0 == PlayerId(0) && b.kind == yard);
                ships |= world
                    .query::<(&sim::Owner, &sim::Unit)>()
                    .iter(world)
                    .any(|(o, u)| o.0 == PlayerId(0) && defs.domain(u.kind) == sim::Domain::Naval);
                if matches!(water, 1 | 3) && built && ships && fired {
                    break;
                }
            }
        }
        assert_eq!(
            (built, ships),
            (matches!(water, 1 | 3), matches!(water, 1 | 3)),
            "water case {water}; controller {:?}",
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
        );
        if matches!(water, 1 | 3) {
            assert!(
                fired,
                "the produced fleet could not hurt the enemy from its sea"
            );
            assert!(
                app.world()
                    .resource::<sim::AiPlayers>()
                    .controller(PlayerId(0))
                    .unwrap()
                    .naval_target()
                    .is_some()
            );
        }
    }
}

#[test]
fn ra8_paid_naval_detector_escorts_the_fleet_toward_a_known_shore_target() {
    let mut app = scenario("russia", "brutal", 100000, 0, false);
    quiet(&mut app);
    for y in 0..48 {
        for x in 16..=24 {
            app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0,
                },
            );
        }
    }
    let defs = app.world().resource::<sim::Definitions>().clone();
    let kind = defs.unit_kind("typhoon").unwrap();
    let mut paid = false;
    let mut escort = None;
    let mut participated = false;
    for _ in 0..24000 {
        let events = step(&mut app);
        paid |= events.iter().any(|event| matches!(event,
            sim::SimEvent::ProductionComplete { player: PlayerId(0), item: sim::ProductionItem::Unit(k) } if *k == kind));
        let goal = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target();
        let world = app.world_mut();
        for (id, owner, unit, mobile) in world
            .query::<(&sim::SimId, &sim::Owner, &sim::Unit, &sim::Mobile)>()
            .iter(world)
        {
            if owner.0 != PlayerId(0) || unit.kind != kind {
                continue;
            }
            let (_, origin) = *escort.get_or_insert((*id, mobile.cell));
            participated |= goal.is_some_and(|goal| {
                mobile.cell != origin
                    && (mobile.cell.x - goal.x).abs() + (mobile.cell.y - goal.y).abs()
                        < (origin.x - goal.x).abs() + (origin.y - goal.y).abs()
            });
        }
        if paid && participated {
            break;
        }
    }
    assert!(paid, "the detector escort must be paid production");
    assert!(
        participated,
        "the produced detector must move along its qualified fleet route"
    );
    assert!(
        defs.metadata(sim::ProductionItem::Unit(kind))
            .unwrap()
            .sensors_sight
            > 0
    );
}

#[test]
fn ra8_unseen_enemy_water_building_cannot_change_naval_planning() {
    let make = || {
        let mut app = scenario("great_britain", "brutal", 0, 0, false);
        let mut rules = app.world().resource::<sim::MatchState>().rules();
        rules.fog_of_war = true;
        app.world_mut()
            .resource_mut::<sim::MatchState>()
            .set_rules(rules);
        for y in 0..48 {
            for x in 16..19 {
                app.world_mut().resource_mut::<Grid>().set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        buildable: false,
                        water: true,
                        ore: 0,
                    },
                );
            }
        }
        put(&mut app, "naval_shipyard", 16, 8);
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "aircraft_carrier",
            CellPos::new(16, 11),
        )
        .unwrap();
        let scout = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(38, 34)).unwrap();
        for _ in 0..21 {
            step(&mut app);
        }
        assert!(
            !app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .known_buildings()
                .is_empty()
        );
        let e = unit_entity(&mut app, scout);
        app.world_mut()
            .get_mut::<sim::Health>(e)
            .unwrap()
            .hit_points = 0;
        step(&mut app);
        app
    };
    let mut actual = make();
    let mut control = make();
    let cell = CellPos::new(16, 23);
    assert!(
        !actual
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(cell)
    );
    sim::spawn_building(actual.world_mut(), PlayerId(1), "naval_shipyard", cell).unwrap();
    while actual.world().resource::<SimTick>().0 < 601 {
        step(&mut actual);
        step(&mut control);
    }
    assert!(
        actual
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target()
            .is_some()
    );
    assert_eq!(
        actual.world().resource::<sim::AiPlayers>(),
        control.world().resource::<sim::AiPlayers>()
    );
    assert_eq!(
        actual
            .world()
            .resource::<CommandQueue>()
            .iter()
            .collect::<Vec<_>>(),
        control
            .world()
            .resource::<CommandQueue>()
            .iter()
            .collect::<Vec<_>>()
    );
}

#[test]
fn ra8_raw_spy_spawn_with_empty_queue_is_concealed_before_actual_ai_thinks() {
    let mut app = start("great_britain", "easy");
    app.world_mut().run_schedule(SimSchedule);
    // Drain first-turn ordinary decisions, then start precisely on the next P0 think.
    app.world_mut().run_schedule(SimSchedule);
    app.world_mut().insert_resource(CommandQueue::default());
    app.world_mut().resource_mut::<SimTick>().0 = 40;
    for (owner, mut orders, weapon) in app
        .world_mut()
        .query::<(&sim::Owner, &mut sim::Orders, Option<&mut sim::Weapon>)>()
        .iter_mut(app.world_mut())
    {
        if owner.0 == PlayerId(0) {
            orders.current = None;
            if let Some(mut weapon) = weapon {
                weapon.order = None;
            }
        }
    }
    let target = CellPos::new(12, 10);
    let spy = spawn_unit(app.world_mut(), PlayerId(1), "spy", target).unwrap();
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(PlayerId(0), spy))
    );
    assert!(app.world().resource::<CommandQueue>().is_empty());
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(target)
    );
    app.world_mut().run_schedule(SimSchedule);
    let commands: Vec<_> = app.world().resource::<CommandQueue>().iter().collect();
    assert!(!commands.iter().any(|issued| matches!(issued.command, Command::AttackMove { target: found, .. } if found == target)), "concealed Spy affected actual controller decisions: {commands:?}");
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(PlayerId(0), spy))
    );
}
