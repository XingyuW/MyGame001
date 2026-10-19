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
    map_side: Option<u32>,
    human: bool,
    second_ai: bool,
    cadences: Option<[u32; 2]>,
    fallback_plan: bool,
    army: Option<&'static [&'static str]>,
    production_only: bool,
    opponent: Option<&'static str>,
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
    if let Some(army) = mode.army {
        let selected = skirmish
            .factions
            .iter()
            .find(|f| f.id == country)
            .unwrap()
            .country;
        for plan in ai.plans.iter_mut().filter(|p| p.country == selected) {
            plan.army = army
                .iter()
                .map(|id| definitions.unit_kind(id).unwrap())
                .collect();
            if mode.production_only {
                // Declared Soviet production fixture, with a complete legal
                // prerequisite chain that is actually present in the world.
                plan.build_order = [
                    "tesla_reactor",
                    "soviet_refinery",
                    "soviet_barracks",
                    "soviet_war_factory",
                    "radar_tower",
                    "soviet_battle_lab",
                    "soviet_naval_shipyard",
                ]
                .iter()
                .map(|id| definitions.building_kind(id).unwrap())
                .collect();
                plan.defences.clear();
                plan.support.clear();
                plan.tier_reserves = [0; 3];
            }
        }
    }
    if mode.fallback_plan {
        let selected = skirmish
            .factions
            .iter()
            .find(|f| f.id == country)
            .unwrap()
            .country;
        ai.plans
            .retain(|plan| plan.country.is_none() || plan.country != selected);
    }
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
                .position(|f| f.id == mode.opponent.unwrap_or("russia"))
                .unwrap(),
            start: 1,
            controller: if mode.second_ai {
                Controller::Ai { difficulty: other }
            } else {
                Controller::Human
            },
        },
    ];
    let side = mode.map_side.unwrap_or(48);
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 8 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            side,
            side,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0
                };
                (side * side) as usize
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
fn ra8_gap_hidden_shore_requires_a_real_naval_observation_path_but_allied_sight_keeps_long_range_fire()
 {
    for observed in [true, false] {
        let mut app = scenario("great_britain", "brutal", 0, 0, false);
        for y in 18..23 {
            for x in 2..7 {
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
        sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "nuclear_reactor",
            CellPos::new(38, 30),
        )
        .unwrap();
        let gap = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "gap_generator",
            CellPos::new(25, 20),
        )
        .unwrap();
        let observer = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(24, 19),
        )
        .unwrap();
        for _ in 0..40 {
            step(&mut app);
        }
        assert!(
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .known_buildings()
                .contains_key(&gap),
            "the target must be learned through actual local sight"
        );
        if !observed {
            issue(
                &mut app,
                Command::Move {
                    units: vec![observer],
                    target: CellPos::new(10, 9),
                },
            );
        }
        for _ in 0..700 {
            step(&mut app);
        }
        assert_eq!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(25, 20)),
            observed,
            "powered Gap must really hide the remembered shore without the observer"
        );
        let carrier = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "aircraft_carrier",
            CellPos::new(4, 20),
        )
        .unwrap();
        let mut fired = false;
        let mut sold = false;
        for _ in 0..5000 {
            for event in step(&mut app) {
                fired |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == carrier && target == gap);
                sold |= matches!(
                    event,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    }
                );
            }
            if fired
                || app
                    .world()
                    .resource::<sim::MatchState>()
                    .outcome()
                    .is_some()
            {
                break;
            }
        }
        assert_eq!(fired, observed, "long-range launch needs real observation");
        if !observed {
            assert!(
                sold,
                "a hidden target outside every reachable sight cell cannot indefinitely block recovery"
            );
            assert!(matches!(
                app.world().resource::<sim::MatchState>().outcome(),
                Some(sim::MatchOutcome::Won {
                    winner: PlayerId(1),
                    ..
                })
            ));
        } else {
            assert!(
                !sold,
                "a genuinely observed long-range target remains viable"
            );
        }
    }
}

#[test]
fn ra8_an_existing_hull_uses_its_reachable_shore_job_instead_of_a_prospective_yards_other_pond() {
    let mut app = scenario("great_britain", "brutal", 0, 0, false);
    for (left, top, right, bottom) in [(18, 8, 22, 12), (10, 28, 22, 32)] {
        for y in top..=bottom {
            for x in left..=right {
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
    }
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    put(&mut app, "naval_shipyard", 18, 8);
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(25, 8),
    )
    .unwrap();
    let shore = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(25, 30),
    )
    .unwrap();
    let hull = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "destroyer",
        CellPos::new(10, 30),
    )
    .unwrap();
    let rescan = load_ai(
        data().join("ai.ron"),
        app.world().resource::<sim::Definitions>(),
    )
    .unwrap()
    .policy
    .naval_rescan;
    for _ in 0..=rescan {
        step(&mut app);
    }
    for target in [CellPos::new(25, 8), CellPos::new(25, 30)] {
        assert!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(target),
            "the powered source satellite must really reveal both shore jobs"
        );
    }
    let future = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .naval_target()
        .unwrap();
    assert!(
        future.y < 20,
        "the real shipyard's qualified future goal must remain in the other pond: {future:?}"
    );
    let mut fired = false;
    let mut destroyed = false;
    let mut sent_to_other_pond = false;
    for _ in 0..5000 {
        for event in step(&mut app) {
            fired |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == hull && target == shore);
            destroyed |= matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == shore);
        }
        sent_to_other_pond |= app.world().resource::<CommandQueue>().iter().any(|issued| {
            matches!(&issued.command, Command::AttackMove { units, target } if units.contains(&hull) && target.y < 20)
        });
        if destroyed {
            break;
        }
    }
    eprintln!(
        "existing hull shore fired{fired} destroyed{destroyed} sent_to_other_pond{sent_to_other_pond}"
    );
    assert!(
        fired && destroyed,
        "the existing hull must approach, fire at and destroy its genuinely reachable shore target"
    );
    assert!(
        !sent_to_other_pond,
        "prospective yard planning must not assign an unreachable pond to a living hull"
    );
}

#[test]
fn ra8_an_existing_dreadnought_without_a_yard_backs_out_to_its_real_minimum_range_and_finishes() {
    let mut app = scenario("great_britain", "brutal", 0, 0, false);
    for y in 18..23 {
        for x in 2..15 {
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
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    let shore = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(17, 20),
    )
    .unwrap();
    let hull = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "dreadnought",
        CellPos::new(14, 20),
    )
    .unwrap();
    let defs = app.world().resource::<sim::Definitions>();
    let kind = defs.unit_kind("dreadnought").unwrap();
    let weapon = defs.unit_combat(kind).unwrap().weapons(false)[0].unwrap();
    assert!(
        defs.weapon(weapon).unwrap().min_range > 3 * sim::SUBCELLS_PER_CELL as u32,
        "the declared starting cell must genuinely be inside the source minimum range"
    );
    let mut first_fire = None;
    let mut destroyed = false;
    for _ in 0..5000 {
        for event in step(&mut app) {
            if matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == hull && target == shore)
                && first_fire.is_none()
            {
                let entity = unit_entity(&mut app, hull);
                first_fire = Some(app.world().get::<sim::Mobile>(entity).unwrap().cell);
            }
            destroyed |= matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == shore);
        }
        if destroyed {
            break;
        }
    }
    eprintln!("minimum-range hull first_fire{first_fire:?} destroyed{destroyed}");
    assert!(
        first_fire.is_some_and(|cell| cell.x < 14),
        "the too-close hull must really back away before an ordinary legal launcher shot"
    );
    assert!(
        destroyed,
        "normal source missiles must actually destroy the shore target"
    );
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target()
            .is_none(),
        "the existing-hull job must work independently of a prospective shipyard"
    );
}

fn dreadnought_minimum_pond(live: bool) -> (App, sim::SimId, Option<sim::SimId>) {
    let mut app = if live {
        scenario("great_britain", "brutal", 0, 0, false)
    } else {
        // Declared production-only naval projection: isolate the source weapon
        // with a minimum range; owners/prerequisites/costs remain exact.
        scenario_mode(
            "russia",
            "medium",
            20_000,
            0,
            false,
            ScenarioMode {
                army: Some(&["dreadnought"]),
                ..ScenarioMode::default()
            },
        )
    };
    for y in 18..23 {
        for x in 10..16 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0
                }
            ));
        }
    }
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(17, 20),
    )
    .unwrap();
    let hull = if live {
        Some(
            spawn_unit(
                app.world_mut(),
                PlayerId(0),
                "dreadnought",
                CellPos::new(14, 20),
            )
            .unwrap(),
        )
    } else {
        put(&mut app, "soviet_battle_lab", 12, 10);
        put(&mut app, "tesla_reactor", 10, 14);
        put(&mut app, "soviet_naval_shipyard", 10, 18);
        None
    };
    let defs = app.world().resource::<sim::Definitions>();
    let kind = defs.unit_kind("dreadnought").unwrap();
    let terms = defs.unit_combat(kind).unwrap();
    let weapon = defs.weapon(terms.weapons(false)[0].unwrap()).unwrap();
    assert!(weapon.min_range >= 8 * sim::SUBCELLS_PER_CELL as u32);
    // Every source-water centre is too close even to the nearest target edge;
    // the only other enemy building at38,38 is beyond source maximum range.
    assert!(weapon.range < 27 * sim::SUBCELLS_PER_CELL as u32);
    if !live {
        let kinds = {
            let world = app.world_mut();
            world
                .query::<(&sim::Owner, &sim::Building)>()
                .iter(world)
                .filter(|(o, _)| o.0 == PlayerId(0))
                .map(|(_, b)| b.kind)
                .collect::<Vec<_>>()
        };
        assert!(
            sim::check_buildable(
                app.world().resource::<sim::Definitions>(),
                app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap(),
                true,
                &kinds,
                sim::ProductionItem::Unit(kind)
            )
            .is_ok()
        );
    }
    (app, target, hull)
}

#[test]
fn ra8_a_dreadnought_wholly_inside_source_minimum_cannot_pin_last_stand() {
    let (mut app, target, hull) = dreadnought_minimum_pond(true);
    let mut sold = false;
    let mut fired = false;
    for _ in 0..6000 {
        for event in step(&mut app) {
            sold |= matches!(
                event,
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    ..
                }
            );
            fired |= matches!(event,sim::SimEvent::WeaponFired {attacker,target:found,..} if Some(attacker) == hull && found == target);
        }
        if sold {
            break;
        }
    }
    assert!(
        !fired,
        "the tiny pond must genuinely provide no legal source-range shot"
    );
    assert!(
        sold,
        "an incapable minimum-range remnant must not permanently claim finishing capability"
    );
}

#[test]
fn ra8_a_dreadnought_wholly_inside_source_minimum_is_not_recruited_for_that_shore() {
    let (mut app, _, _) = dreadnought_minimum_pond(false);
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("dreadnought")
        .unwrap();
    let mut queued = false;
    let mut produced = false;
    for _ in 0..4000 {
        for event in step(&mut app) {
            produced |= matches!(event,sim::SimEvent::ProductionComplete {player:PlayerId(0),item:sim::ProductionItem::Unit(found)} if found == kind);
        }
        queued |= app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command,Command::QueueProduction {item:sim::ProductionItem::Unit(found)} if found == kind));
    }
    assert!(
        !queued && !produced,
        "legally producible source hull has no reachable firing cell outside its minimum: queued{queued}/paid{produced}"
    );
}

#[test]
fn ra8_a_real_naval_edge_gap_shot_remains_an_eligible_paid_ai_shore_job() {
    for human in [true, false] {
        let mut app = scenario_mode(
            "great_britain",
            "medium",
            if human { 0 } else { 20_000 },
            0,
            false,
            ScenarioMode {
                human,
                army: Some(&["destroyer"]),
                ..ScenarioMode::default()
            },
        );
        let defs = app.world().resource::<sim::Definitions>();
        let kind = defs.unit_kind("destroyer").unwrap();
        let terms = defs.unit_combat(kind).unwrap();
        let weapon = defs.weapon(terms.weapons(false)[0].unwrap()).unwrap();
        let scale = sim::SUBCELLS_PER_CELL as u32;
        assert_eq!(weapon.range % scale, 0);
        let cells = i32::try_from(weapon.range / scale).unwrap();
        assert!(cells >= 3);
        for y in 18..21 {
            for x in 10..15 {
                assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        buildable: false,
                        water: true,
                        ore: 0
                    }
                ));
            }
        }
        put(&mut app, "nuclear_reactor", 2, 2);
        put(&mut app, "spy_satellite", 2, 8);
        let target = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "power_plant",
            CellPos::new(14 + cells, 22),
        )
        .unwrap();
        let hull = if human {
            Some(
                spawn_unit(
                    app.world_mut(),
                    PlayerId(0),
                    "destroyer",
                    CellPos::new(14, 20),
                )
                .unwrap(),
            )
        } else {
            put(&mut app, "radar_tower", 12, 4);
            put(&mut app, "naval_shipyard", 10, 18);
            None
        };
        // The nearest water cell has centre delta(range,2): outside the old
        // integer-cell circle, but inside the actual weapon's footprint-edge gap.
        let mut paid = human;
        let mut fired = false;
        for _ in 0..4000 {
            for event in step(&mut app) {
                paid |= matches!(event, sim::SimEvent::ProductionComplete { player: PlayerId(0), item: sim::ProductionItem::Unit(found) } if found == kind);
                fired |= matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if found == target && hull.is_none_or(|h| attacker == h));
            }
        }
        eprintln!("edge-gap human{human}: paid{paid}/fired{fired}");
        assert!(
            paid && fired,
            "source-legal footprint-edge shot must remain a paid shore job: human{human}/paid{paid}/fired{fired}"
        );
    }
}

#[test]
fn ra8_mixed_naval_recruitment_uses_each_hulls_legal_cells_in_the_same_sea() {
    let mut app = scenario_mode(
        "cuba",
        "medium",
        20_000,
        0,
        false,
        ScenarioMode {
            army: Some(&["typhoon", "dreadnought"]),
            production_only: true,
            opponent: Some("america"),
            ..ScenarioMode::default()
        },
    );
    for y in 18..23 {
        for x in 10..16 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0
                }
            ));
        }
    }
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    put(&mut app, "soviet_refinery", 2, 14);
    put(&mut app, "soviet_barracks", 6, 14);
    put(&mut app, "soviet_war_factory", 18, 4);
    put(&mut app, "radar_tower", 12, 4);
    put(&mut app, "soviet_battle_lab", 12, 10);
    put(&mut app, "tesla_reactor", 10, 14);
    put(&mut app, "soviet_naval_shipyard", 10, 18);
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(20, 22),
    )
    .unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "destroyer",
        CellPos::new(14, 22),
    )
    .unwrap();
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("dreadnought")
        .unwrap();
    let mut queued = false;
    let rescan = load_ai(
        data().join("ai.ron"),
        app.world().resource::<sim::Definitions>(),
    )
    .unwrap()
    .policy
    .naval_rescan;
    let cadence = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .difficulty()
        .think_interval;
    // queue_units reads the previous scan's goal. Include the entire next due
    // turn after strategy installs its first600-tick scan (AI620/SimTick621).
    for _ in 0..u64::from(rescan) + u64::from(cadence) + 1 {
        step(&mut app);
        queued |= app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::QueueProduction { item: sim::ProductionItem::Unit(found) } if found == kind));
    }
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target()
            .is_some(),
        "the actual rescan must qualify the shared sea before testing per-kind admission"
    );
    eprintln!(
        "mixed naval first funded decision: goal{:?}/Dreadqueued{queued}",
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target()
    );
    assert!(
        queued,
        "a shared close submarine goal cannot exclude a source-legal Dreadnought firing cell in the same reachable sea"
    );
    let mut paid = false;
    let mut fired = false;
    for _ in 0..5000 {
        for event in step(&mut app) {
            paid |= matches!(event, sim::SimEvent::ProductionComplete { player: PlayerId(0), item: sim::ProductionItem::Unit(found) } if found == kind);
            if let sim::SimEvent::WeaponFired {
                attacker,
                target: found,
                ..
            } = event
            {
                let world = app.world_mut();
                fired |= found == target
                    && world
                        .query::<(&sim::SimId, &sim::Unit)>()
                        .iter(world)
                        .any(|(id, unit)| *id == attacker && unit.kind == kind);
            }
        }
        if paid && fired {
            break;
        }
    }
    assert!(
        paid && fired,
        "admitted hull must actually be paid and fire at its legal shore target"
    );
}

fn naval_unit_boundary(
    human: bool,
    dread: bool,
    target: CellPos,
) -> (App, sim::SimId, sim::UnitKind, Option<sim::SimId>) {
    let mut app = scenario_mode(
        if dread { "cuba" } else { "great_britain" },
        "medium",
        if human { 0 } else { 20_000 },
        0,
        false,
        ScenarioMode {
            human,
            opponent: Some("america"),
            production_only: dread,
            army: Some(if dread {
                &["dreadnought"]
            } else {
                &["destroyer"]
            }),
            ..ScenarioMode::default()
        },
    );
    for y in 18..21 {
        for x in 10..15 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0
                }
            ));
        }
    }
    {
        assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
            target,
            Cell {
                passable: false,
                buildable: false,
                water: true,
                ore: 0
            }
        ));
    }
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    put(&mut app, "radar_tower", 12, 4);
    if dread {
        put(&mut app, "soviet_refinery", 2, 14);
        put(&mut app, "soviet_barracks", 6, 14);
        put(&mut app, "soviet_war_factory", 18, 4);
        put(&mut app, "soviet_battle_lab", 12, 10);
        put(&mut app, "tesla_reactor", 10, 14);
        put(&mut app, "soviet_naval_shipyard", 10, 18);
    } else if !human {
        put(&mut app, "naval_shipyard", 10, 18);
    }
    let id = if dread { "dreadnought" } else { "destroyer" };
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind(id)
        .unwrap();
    let own =
        human.then(|| spawn_unit(app.world_mut(), PlayerId(0), id, CellPos::new(14, 20)).unwrap());
    let enemy = spawn_unit(app.world_mut(), PlayerId(1), "destroyer", target).unwrap();
    (app, enemy, kind, own)
}

fn naval_unit_range(dread: bool) -> i32 {
    let defs = load_ruleset(data()).unwrap();
    let terms = defs
        .unit_combat(
            defs.unit_kind(if dread { "dreadnought" } else { "destroyer" })
                .unwrap(),
        )
        .unwrap();
    let enemy = defs
        .unit_combat(defs.unit_kind("destroyer").unwrap())
        .unwrap();
    let (weapon, percent) = sim::weapon_against(
        &defs,
        &terms,
        terms.weapons(false),
        &sim::TargetTraits {
            armour: enemy.armour,
            on_water: true,
            underwater: enemy.underwater,
            organic: enemy.organic,
            ..sim::TargetTraits::default()
        },
    )
    .unwrap();
    assert!(percent > 1);
    let range = defs.weapon(weapon).unwrap().range;
    eprintln!("actual naval weapon {weapon:?} / range {range}");
    assert_eq!(range % sim::SUBCELLS_PER_CELL as u32, 0);
    i32::try_from(range / sim::SUBCELLS_PER_CELL as u32).unwrap()
}

#[test]
fn ra8_naval_unit_points_outside_source_maximum_are_not_footprint_edge_jobs() {
    for human in [true, false] {
        let (mut app, target, kind, own) =
            naval_unit_boundary(human, false, CellPos::new(14 + naval_unit_range(false), 22));
        let mut queued = false;
        let mut paid = false;
        let mut fired = false;
        for _ in 0..if human { 40 } else { 4000 } {
            for e in step(&mut app) {
                paid |= matches!(e,sim::SimEvent::ProductionComplete {player:PlayerId(0),item:sim::ProductionItem::Unit(k)} if k==kind);
                if let sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } = e
                    && found == target
                    && own.is_none_or(|id| id == attacker)
                {
                    let world = app.world_mut();
                    let positions: Vec<_> = world
                        .query::<(&sim::SimId, &sim::Mobile)>()
                        .iter(world)
                        .filter(|(id, _)| **id == target || **id == attacker)
                        .map(|(id, m)| (*id, m.position(world.resource::<Grid>(), *id)))
                        .collect();
                    eprintln!(
                        "actual unit-boundary fire {attacker:?}->{target:?} positions{positions:?}"
                    );
                    fired = true;
                }
            }
            queued|=app.world().resource::<CommandQueue>().iter().any(|c|matches!(c.command,Command::QueueProduction {item:sim::ProductionItem::Unit(k)} if k==kind));
        }
        eprintln!("unit outside maximum human{human}: queued{queued}/paid{paid}/fired{fired}");
        assert!(
            !fired,
            "the actual unit point must genuinely be outside source range"
        );
        assert!(
            !queued && !paid,
            "a unit point cannot inherit a building's footprint-edge range"
        );
    }
}

#[test]
fn ra8_naval_unit_points_at_source_maximum_support_real_paid_fire() {
    for human in [true, false] {
        let (mut app, target, kind, own) =
            naval_unit_boundary(human, false, CellPos::new(14 + naval_unit_range(false), 20));
        let mut paid = human;
        let mut fired = false;
        for _ in 0..4000 {
            for e in step(&mut app) {
                paid |= matches!(e,sim::SimEvent::ProductionComplete {player:PlayerId(0),item:sim::ProductionItem::Unit(k)} if k==kind);
                if let sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } = e
                    && found == target
                    && own.is_none_or(|id| id == attacker)
                {
                    let world = app.world_mut();
                    let positions: Vec<_> = world
                        .query::<(&sim::SimId, &sim::Mobile)>()
                        .iter(world)
                        .filter(|(id, _)| **id == target || **id == attacker)
                        .map(|(id, m)| (*id, m.position(world.resource::<Grid>(), *id)))
                        .collect();
                    eprintln!(
                        "actual unit-boundary fire {attacker:?}->{target:?} positions{positions:?}"
                    );
                    fired = true;
                }
            }
            if paid && fired {
                break;
            }
        }
        eprintln!("unit maximum human{human}: paid{paid}/fired{fired}");
        assert!(
            paid && fired,
            "source unit-point boundary remains a real paid firing job"
        );
    }
}

#[test]
fn ra8_naval_unit_movement_positions_and_observed_destinations_drive_jobs() {
    {
        let dread = true;
        let start = CellPos::new(35, 22);
        let (mut app, target, kind, _) = naval_unit_boundary(false, dread, start);
        assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(start.x - 1, start.y),
            Cell {
                passable: false,
                buildable: false,
                water: true,
                ore: 0
            }
        ));
        let radius = app
            .world()
            .resource::<sim::Definitions>()
            .metadata(sim::ProductionItem::Building(
                app.world()
                    .resource::<sim::Definitions>()
                    .building_kind("gap_generator")
                    .unwrap(),
            ))
            .unwrap()
            .radius;
        let gap = CellPos::new(start.x + i32::try_from(radius).unwrap(), 22);
        sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "power_plant",
            CellPos::new(gap.x - 2, 18),
        )
        .unwrap();
        sim::spawn_building(app.world_mut(), PlayerId(1), "gap_generator", gap).unwrap();
        for _ in 0..590 {
            step(&mut app);
        }
        // The enemy's ordinary move exposes its destination while its old cell
        // remains under a real powered Gap. No injected vision or hidden state.
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(sim::IssuedCommand {
                player: PlayerId(1),
                command: Command::Move {
                    units: vec![target],
                    target: CellPos::new(start.x - 1, start.y),
                },
            });
        while app.world().resource::<SimTick>().0 < 601 {
            step(&mut app);
        }
        let entity = unit_entity(&mut app, target);
        let mobile = app.world().get::<sim::Mobile>(entity).unwrap();
        assert_eq!(mobile.cell, start);
        assert!(mobile.step.is_some());
        let vision = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision();
        assert!(!vision.is_visible(start));
        assert!(
            vision.sees_unit(mobile),
            "the moving target must actually be observed through its destination"
        );
        assert!(
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .naval_target()
                .is_some(),
            "an observed unit uses actual subcell position and full source weapon range"
        );
        let mut paid = false;
        let mut fired = false;
        for _ in 0..5000 {
            for e in step(&mut app) {
                paid |= matches!(e,sim::SimEvent::ProductionComplete {player:PlayerId(0),item:sim::ProductionItem::Unit(k)} if k==kind);
                if let sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } = e
                {
                    let world = app.world_mut();
                    fired |= found == target
                        && world
                            .query::<(&sim::SimId, &sim::Unit)>()
                            .iter(world)
                            .any(|(id, u)| *id == attacker && u.kind == kind);
                }
            }
            if paid && fired {
                break;
            }
        }
        assert!(
            paid && fired,
            "the actual observed moving-unit job must pay and fire"
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
fn ra8_a_remote_replacement_tank_does_not_pull_advancing_artillery_back_home() {
    // The healthy-order twin runs first: its real forward order must remain intact.
    for healthy in [true, false] {
        let mut app = scenario("germany", "medium", 0, 0, false);
        quiet(&mut app);
        let tank =
            spawn_unit(app.world_mut(), PlayerId(0), "grizzly", CellPos::new(8, 18)).unwrap();
        let artillery = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "prism_tank",
            CellPos::new(8, 20),
        )
        .unwrap();
        // A genuinely observed armed unit retains the normal combined-arms branch,
        // without putting an unrelated local combat threat in this firing lane.
        spawn_unit(
            app.world_mut(),
            PlayerId(1),
            "rocketeer",
            CellPos::new(45, 25),
        )
        .unwrap();
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(33, 12),
        )
        .unwrap();
        spawn_unit(app.world_mut(), PlayerId(0), "spy", CellPos::new(44, 24)).unwrap();
        let target = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "barracks",
            CellPos::new(36, 14),
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
        for id in [tank, artillery] {
            assert!(
                controllers
                    .controller(PlayerId(0))
                    .unwrap()
                    .wave()
                    .contains(&id)
            );
        }
        // Preserve actual recruitment/memory, pause only decisions while normal
        // commands arrange an advanced hull and the remote rear replacement.
        issue(
            &mut app,
            Command::Stop {
                units: vec![tank, artillery],
            },
        );
        for (id, target) in [
            (tank, CellPos::new(12, 14)),
            (artillery, CellPos::new(24, 14)),
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
            if [tank, artillery].iter().all(|id| {
                let e = unit_entity(&mut app, *id);
                app.world().get::<sim::Orders>(e).unwrap().current.is_none()
            }) {
                break;
            }
        }
        for (id, target) in [
            (tank, CellPos::new(12, 14)),
            (artillery, CellPos::new(24, 14)),
        ] {
            let e = unit_entity(&mut app, id);
            assert_eq!(app.world().get::<sim::Mobile>(e).unwrap().cell, target);
        }
        if healthy {
            issue(
                &mut app,
                Command::AttackMove {
                    units: vec![artillery],
                    target: CellPos::new(33, 14),
                },
            );
            step(&mut app);
        }
        while !app.world().resource::<SimTick>().0.is_multiple_of(20) {
            step(&mut app);
        }
        for cell in [CellPos::new(36, 14), CellPos::new(45, 25)] {
            assert!(
                app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .vision()
                    .is_visible(cell),
                "real local observers must reveal target and armed-unit branch"
            );
        }
        app.world_mut().insert_resource(controllers);
        step(&mut app);
        let queued: Vec<_> = app
            .world()
            .resource::<CommandQueue>()
            .iter()
            .filter_map(|issued| match &issued.command {
                Command::AttackMove { units, target } if units.contains(&artillery) => {
                    Some(*target)
                }
                _ => None,
            })
            .collect();
        if healthy {
            assert!(
                queued.is_empty(),
                "a healthy forward approach must not be refreshed: {queued:?}"
            );
            let e = unit_entity(&mut app, artillery);
            assert_eq!(
                app.world()
                    .get::<sim::Orders>(e)
                    .unwrap()
                    .current
                    .as_ref()
                    .unwrap()
                    .destination,
                CellPos::new(33, 14)
            );
        }
        let mut shots = 0;
        let mut destroyed = false;
        for _ in 0..1000 {
            for event in step(&mut app) {
                shots += usize::from(
                    matches!(event, sim::SimEvent::WeaponFired { attacker, target: t, .. } if attacker == artillery && t == target),
                );
                destroyed |= matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == target);
            }
            if destroyed {
                break;
            }
        }
        let e = unit_entity(&mut app, tank);
        let front = app.world().get::<sim::Mobile>(e).unwrap().cell;
        eprintln!(
            "remote replacement healthy{healthy}: queued{queued:?}, artillery shots{shots}, destroyed{destroyed}, moving front{front:?}"
        );
        assert!(front.x > 12, "the ordinary front must genuinely advance");
        assert!(
            shots >= 2 && destroyed,
            "advanced artillery must retain a real offensive contribution instead of returning behind a remote newcomer"
        );
    }
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
fn ra8_a_locked_late_tech_item_leaves_its_lane_for_a_legal_early_defence() {
    for unlocked in [false, true] {
        // Declared owned prerequisites isolate lane admission. Production, payment and
        // controller decisions still use the shipped definitions and actual schedule.
        let mut app = scenario("france", "medium", 100000, 0, true);
        put(&mut app, "power_plant", 4, 4);
        put(&mut app, "refinery", 4, 12);
        put(&mut app, "barracks", 12, 4);
        if unlocked {
            put(&mut app, "war_factory", 12, 12);
            put(&mut app, "airforce_command", 18, 4);
            put(&mut app, "tech_center", 18, 12);
            put(&mut app, "power_plant", 24, 4);
            put(&mut app, "power_plant", 24, 8);
        }
        let definitions = app.world().resource::<sim::Definitions>().clone();
        let expected = definitions
            .building_kind(if unlocked { "spy_satellite" } else { "pillbox" })
            .unwrap();
        let mut paid = false;
        for tick in 0..100 {
            step(&mut app);
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            let main = state.queues().any(|(key, q)| {
                key.lane == sim::Lane::Main
                    && q.iter()
                        .any(|i| matches!(i.item, sim::ProductionItem::Building(_)))
            });
            if let Some(head) = state.queues().find_map(|(key, q)| {
                (key.lane == sim::Lane::Defences)
                    .then(|| q.first())
                    .flatten()
            }) {
                assert_eq!(head.item, sim::ProductionItem::Building(expected));
                if head.charged > 0 && main {
                    eprintln!(
                        "unlocked={unlocked}: paid {:?} with main at tick{}",
                        head.item,
                        tick + 1
                    );
                    paid = true;
                    break;
                }
            }
        }
        assert!(paid, "unlocked={unlocked}: legal paid lane was blocked");
    }
}

#[test]
fn ra8_land_defenders_use_reachable_firing_positions_and_leave_isolated_ships_to_the_fleet() {
    for reachable in [true, false] {
        let mut app = scenario("cuba", "brutal", 0, 0, false);
        quiet(&mut app);
        // Both ships are observed inside the ordinary base defence radius. Only the
        // near shore offers a land firing position for the Conscript's actual rifle.
        for y in 0..=12 {
            for x in 13..=25 {
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
        let actor = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "conscript",
            CellPos::new(11, 11),
        )
        .unwrap();
        // Ordinary owned sight exposes the offshore case too. The utility
        // observer is not an armed wave member.
        spawn_unit(app.world_mut(), PlayerId(0), "spy", CellPos::new(20, 13)).unwrap();
        let ship_cell = CellPos::new(if reachable { 14 } else { 20 }, 8);
        let ship = spawn_unit(app.world_mut(), PlayerId(1), "destroyer", ship_cell).unwrap();
        // Isolate the response, without changing the target's domain/combat metadata.
        let ship_entity = unit_entity(&mut app, ship);
        app.world_mut()
            .entity_mut(ship_entity)
            .remove::<sim::Weapon>();
        let land_target = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "soviet_barracks",
            CellPos::new(30, 15),
        )
        .unwrap();
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
        step(&mut app);
        assert!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(ship_cell)
        );
        app.world_mut().insert_resource(controllers);
        let expected = if reachable { ship } else { land_target };
        let mut fired = false;
        let mut joined = false;
        for _ in 0..10000 {
            let events = step(&mut app);
            joined |= app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&actor);
            fired |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == actor && *target == expected));
            if fired {
                break;
            }
        }
        assert!(
            fired,
            "reachable={reachable}: no useful response to {expected:?}"
        );
        if !reachable {
            assert!(
                joined,
                "an isolated sea threat monopolized the land reserve"
            );
        }
    }
}

#[test]
fn ra8_a_specialist_repair_reservation_removes_the_overlapping_defence_attack() {
    let mut app = scenario("france", "brutal", 100000, 0, false);
    quiet(&mut app);
    let depot = put(&mut app, "service_depot", 2, 20);
    let tank = spawn_unit(app.world_mut(), PlayerId(0), "grizzly", CellPos::new(5, 20)).unwrap();
    let entity = unit_entity(&mut app, tank);
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = 10;
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "conscript",
        CellPos::new(12, 20),
    )
    .unwrap();
    let enemy_entity = unit_entity(&mut app, enemy);
    app.world_mut()
        .entity_mut(enemy_entity)
        .remove::<sim::Weapon>();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    step(&mut app);
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(12, 20))
    );
    app.world_mut().insert_resource(controllers);
    let mut reserved = false;
    let mut repaired = false;
    for _ in 0..800 {
        step(&mut app);
        for issued in app.world().resource::<CommandQueue>().iter() {
            assert!(
                !matches!(&issued.command, Command::Attack { units, .. } if units.is_empty()),
                "an empty internal defence survived reservation filtering"
            );
            reserved |= matches!(issued.command, Command::Interact { unit, target } if unit == tank && target == depot);
        }
        repaired |= app
            .world()
            .get::<sim::Health>(entity)
            .is_some_and(|hp| hp.hit_points > 10);
        if repaired && reserved {
            break;
        }
    }
    assert!(
        reserved && repaired,
        "ordinary reserved service action must survive the overlapping defence intent"
    );
}

fn strike_sortie_fixture() -> (App, sim::SimId, sim::SimId) {
    strike_aircraft_fixture("harrier")
}

fn strike_aircraft_fixture(aircraft: &str) -> (App, sim::SimId, sim::SimId) {
    let mut app = scenario("germany", "brutal", 0, 0, false);
    let world = app.world_mut();
    let reserve = world
        .query::<(bevy_ecs::entity::Entity, &sim::Owner, &sim::Building)>()
        .iter(world)
        .find(|(_, owner, _)| owner.0 == PlayerId(1))
        .unwrap()
        .0;
    world.get_mut::<sim::Health>(reserve).unwrap().hit_points = 0;
    // Declared owned ordinary infrastructure; shots, ammo, dock return and refill
    // all run normally. No failure counter, target HP or ammo state is seeded.
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 8);
    put(&mut app, "airforce_command", 12, 4);
    let plane = spawn_unit(app.world_mut(), PlayerId(0), aircraft, CellPos::new(12, 8)).unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(40, 8),
    )
    .unwrap();
    spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(43, 40)).unwrap();
    (app, plane, target)
}

#[test]
fn ra8_unlimited_bomber_focus_ignores_an_incapable_en_route_ground_unit() {
    let (mut app, bomber, target) = strike_aircraft_fixture("kirov");
    let distraction = spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(32, 8)).unwrap();
    let mut first = None;
    let mut destroyed = false;
    for _ in 0..12000 {
        for event in step(&mut app) {
            if let sim::SimEvent::WeaponFired {
                attacker,
                target: found,
                ..
            } = event
                && attacker == bomber
            {
                first.get_or_insert(found);
            }
            destroyed |= matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == target);
        }
        if destroyed {
            break;
        }
    }
    let world = app.world_mut();
    let distraction_lives = world
        .query::<(&sim::SimId, &sim::Health)>()
        .iter(world)
        .any(|(id, hp)| *id == distraction && hp.hit_points > 0);
    eprintln!(
        "unlimited bomber first{first:?}, objective{target:?}, destroyed{destroyed}, incapable GI alive{distraction_lives}"
    );
    assert_eq!(
        first,
        Some(target),
        "a ground-only unit cannot replace the bomber's observed building mission"
    );
    assert!(
        destroyed && distraction_lives,
        "the bomber must finish its real building job while leaving the incapable diversion alone"
    );
}

#[test]
fn ra8_paid_repairs_cannot_pin_real_reloaded_strikes_to_one_unchanged_building() {
    let (mut app, plane, target) = strike_sortie_fixture();
    let alternate = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "barracks",
        CellPos::new(40, 20),
    )
    .unwrap();
    let oil = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "oil_derrick",
        CellPos::new(44, 28),
    )
    .unwrap();
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "engineer",
        CellPos::new(43, 28),
    )
    .unwrap();
    step(&mut app); // Ordinary sight before the public capture command.
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Interact {
                unit: engineer,
                target: oil,
            },
        });
    let entity = unit_entity(&mut app, plane);
    let definitions = app.world().resource::<sim::Definitions>();
    let maximum = definitions
        .building_combat(definitions.building_kind("power_plant").unwrap())
        .unwrap()
        .hit_points;
    let kind = definitions.unit_kind("harrier").unwrap();
    let ammo = definitions
        .metadata(sim::ProductionItem::Unit(kind))
        .unwrap()
        .ammo;
    let burst = definitions
        .weapon(definitions.unit_combat(kind).unwrap().weapons(false)[0].unwrap())
        .unwrap()
        .burst as usize;
    let mut shots = 0;
    let mut other_shots = 0;
    let mut reloads = 0;
    let mut was_empty = false;
    let mut paid_healing = 0;
    for _ in 0..18000 {
        let world = app.world_mut();
        let hp_before = world
            .query::<(&sim::SimId, &sim::Health)>()
            .iter(world)
            .find(|(id, _)| **id == target)
            .map(|(_, hp)| hp.hit_points)
            .unwrap_or(0);
        let credits_before = world
            .resource::<sim::Players>()
            .get(PlayerId(1))
            .unwrap()
            .credits();
        let events = step(&mut app);
        let world = app.world_mut();
        let hp_after = world
            .query::<(&sim::SimId, &sim::Health)>()
            .iter(world)
            .find(|(id, _)| **id == target)
            .map(|(_, hp)| hp.hit_points)
            .unwrap_or(0);
        let state = world.resource::<sim::Players>().get(PlayerId(1)).unwrap();
        paid_healing += usize::from(hp_after > hp_before && state.credits() < credits_before);
        let repair = hp_after > 0
            && hp_after < maximum
            && !state.repairing().contains(&target)
            && state.credits() > 0;
        if repair {
            world
                .resource_mut::<CommandQueue>()
                .push(sim::IssuedCommand {
                    player: PlayerId(1),
                    command: Command::RepairBuilding { building: target },
                });
        }
        shots += events.iter().filter(|e| matches!(e, sim::SimEvent::WeaponFired {attacker,target: found,..} if *attacker==plane && *found==target)).count();
        other_shots += events.iter().filter(|e| matches!(e, sim::SimEvent::WeaponFired {attacker,target: found,..} if *attacker==plane && *found==alternate)).count();
        let empty = app
            .world()
            .resource::<sim::SpecialState>()
            .ammo_of(plane, ammo)
            == 0;
        if was_empty && !empty {
            reloads += 1;
        }
        was_empty = empty;
        assert!(
            app.world()
                .get::<sim::Health>(entity)
                .is_some_and(|hp| hp.hit_points > 0)
        );
        if other_shots > 0 {
            break;
        }
    }
    eprintln!(
        "paid-repair sortie shots{shots} other{other_shots} reloads{reloads} paid_healing{paid_healing}"
    );
    eprintln!(
        "paid-repair final controller{:?} weapon{:?} mobile{:?}",
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0)),
        app.world().get::<sim::Weapon>(entity),
        app.world().get::<sim::Mobile>(entity)
    );
    assert!(
        shots > burst && reloads > 1 && paid_healing > 0,
        "the actual source weapon, ammo, reload and paid repairs must establish repeated non-convergence"
    );
    assert!(
        other_shots > 0,
        "the repaired nearest building monopolized every real sortie"
    );
}

#[test]
fn ra8_a_remote_armed_unit_does_not_strand_real_strike_sorties_at_their_dock() {
    let (mut app, plane, target) = strike_sortie_fixture();
    let definitions = app.world().resource::<sim::Definitions>();
    let terms = definitions
        .unit_combat(definitions.unit_kind("harrier").unwrap())
        .unwrap();
    let burst = definitions
        .weapon(terms.weapons(false)[0].unwrap())
        .unwrap()
        .burst as usize;
    let mut destroyed = false;
    let mut shots = 0;
    let mut returned = false;
    for at in 0..18000 {
        let events = step(&mut app);
        if std::env::var_os("RA8_SORTIE_TRACE").is_some() && at >= 17940 && at % 10 == 0 {
            let entity = unit_entity(&mut app, plane);
            eprintln!(
                "sortie at{at} queue{:?} weapon{:?} mobile{:?} special{:?}",
                app.world().resource::<CommandQueue>(),
                app.world().get::<sim::Weapon>(entity),
                app.world().get::<sim::Mobile>(entity),
                app.world().resource::<sim::SpecialState>()
            );
        }
        shots += events.iter().filter(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == plane && *found == target)).count();
        returned |= app
            .world()
            .resource::<sim::SpecialState>()
            .reloading
            .contains_key(&plane);
        destroyed |= events.iter().any(|event| matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if *building == target));
        if destroyed {
            break;
        }
    }
    eprintln!(
        "strike shots{shots} returned{returned} controller{:?}",
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(40, 8))
    );
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .uses_fog()
    );
    assert!(
        returned && shots > burst,
        "the regression must exercise multiple actual sorties"
    );
    assert!(
        destroyed,
        "a distant ground-only unit must not pin loaded strike aircraft to their dock"
    );
}

#[test]
fn ra8_a_detector_escort_keeps_real_strike_sorties_and_its_own_wave_mission() {
    let (mut app, plane, target) = strike_sortie_fixture();
    let dog = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(14, 10),
    )
    .unwrap();
    let entity = unit_entity(&mut app, plane);
    let definitions = app.world().resource::<sim::Definitions>();
    let terms = definitions
        .unit_combat(definitions.unit_kind("harrier").unwrap())
        .unwrap();
    let burst = definitions
        .weapon(terms.weapons(false)[0].unwrap())
        .unwrap()
        .burst as usize;
    let mut shots = 0;
    let mut returned = false;
    let mut joined = false;
    let mut resumed = false;
    let mut destroyed = false;
    for _ in 0..18000 {
        let events = step(&mut app);
        let wave = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave();
        let together = wave.contains(&plane) && wave.contains(&dog);
        joined |= together;
        returned |= app
            .world()
            .resource::<sim::SpecialState>()
            .reloading
            .contains_key(&plane);
        resumed |= together
            && returned
            && matches!(app.world().get::<sim::Weapon>(entity).unwrap().order, Some(sim::AttackOrder::Attack { target: current, .. }) if current == target);
        shots += events.iter().filter(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == plane && *found == target)).count();
        destroyed |= events.iter().any(|event| matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if *building == target));
        if destroyed {
            break;
        }
    }
    eprintln!(
        "escorted strike joined{joined} returned{returned} resumed{resumed} shots{shots} destroyed{destroyed}"
    );
    assert!(
        joined,
        "the actual controller must admit the detector and aircraft into one wave"
    );
    assert!(
        resumed,
        "a detector escort must retain its mission without suppressing the reloaded aircraft's focused job"
    );
    assert!(
        returned && shots > burst && destroyed,
        "the escorted aircraft must really reload, fire again and destroy the observed base target"
    );
}

#[test]
fn ra8_a_local_eligible_air_threat_keeps_real_en_route_strike_combat() {
    let (mut app, plane, target) = strike_sortie_fixture();
    for _ in 0..6000 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&plane)
        {
            break;
        }
    }
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&plane)
    );
    while !app.world().resource::<SimTick>().0.is_multiple_of(10) {
        step(&mut app);
    }
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "flak_trooper",
        CellPos::new(18, 8),
    )
    .unwrap();
    let mut fired = false;
    for _ in 0..160 {
        let events = step(&mut app);
        assert!(!app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Attack { units, target: found } if *found == target && units.contains(&plane))), "base focus overrode an actually nearby anti-air threat");
        fired |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == plane && *found == enemy));
        if fired {
            break;
        }
    }
    assert!(
        fired,
        "ordinary strike combat must engage the local eligible threat"
    );
}

#[test]
fn ra8_unlimited_bombers_still_fight_an_actual_local_anti_air_threat() {
    let (mut app, plane, target) = strike_aircraft_fixture("kirov");
    for _ in 0..6000 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&plane)
        {
            break;
        }
    }
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&plane)
    );
    while !app.world().resource::<SimTick>().0.is_multiple_of(10) {
        step(&mut app);
    }
    // Kirov source speed4 cannot cross the Harrier fixture's six cells in160 ticks.
    // Place this actual source AA threat one cell from the live bomber instead.
    let entity = unit_entity(&mut app, plane);
    let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "flak_trooper",
        CellPos::new(cell.x + 1, cell.y),
    )
    .unwrap();
    let mut fired = false;
    for _ in 0..160 {
        let events = step(&mut app);
        assert!(!app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Attack { units, target: found } if *found == target && units.contains(&plane))), "base focus overrode an actually nearby anti-air threat");
        fired |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == plane && *found == enemy));
        if fired {
            break;
        }
    }
    assert!(
        fired,
        "ordinary strike combat must engage the local eligible threat"
    );
}

#[test]
fn ra8_a_resumed_strike_focus_preserves_its_healthy_chase_across_controller_turns() {
    let (mut app, plane, target) = strike_sortie_fixture();
    let entity = unit_entity(&mut app, plane);
    for _ in 0..10000 {
        step(&mut app);
        let weapon = *app.world().get::<sim::Weapon>(entity).unwrap();
        let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
        let scouting = matches!(
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .target(),
            Some(sim::WaveTarget::Waypoint(_))
        );
        if let Some(sim::AttackOrder::Attack {
            target: current,
            chases,
        }) = weapon.order
            && current == target
            && scouting
            && 40 - cell.x > 12
        {
            // Still well beyond firing range after forty ticks of actual source speed.
            for _ in 0..40 {
                step(&mut app);
                assert!(
                    matches!(app.world().get::<sim::Weapon>(entity).unwrap().order, Some(sim::AttackOrder::Attack { target: current, chases: now }) if current == target && now >= chases),
                    "a healthy resumed strike chase was replaced/reset by a controller turn"
                );
                assert!(!app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Move { units, .. } | Command::AttackMove { units, .. } | Command::Attack { units, .. } if units.contains(&plane))), "a matching healthy strike received a conflicting/repeated core order");
            }
            return;
        }
    }
    panic!("no actual reloaded strike resumed after the naturally completed scouting leg");
}

#[test]
fn ra8_a_defender_already_inside_its_source_air_bonus_range_remains_eligible() {
    let mut app = scenario("france", "brutal", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let cell = CellPos::new(11, 11);
    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
        app.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(cell.x + dx, cell.y + dy),
            Cell {
                passable: false,
                buildable: false,
                water: true,
                ore: 0,
            },
        );
    }
    let actor = spawn_unit(app.world_mut(), PlayerId(0), "ifv", cell).unwrap();
    let entity = unit_entity(&mut app, actor);
    let weapon = *app.world().get::<sim::Weapon>(entity).unwrap();
    let definitions = app.world().resource::<sim::Definitions>();
    let range = definitions.weapon(weapon.kind).unwrap().range;
    let bonus = definitions
        .unit_combat(definitions.unit_kind("ifv").unwrap())
        .unwrap()
        .air_range_bonus;
    let offset = range.div_ceil(sim::SUBCELLS_PER_CELL as u32) + 1;
    assert!(
        offset * sim::SUBCELLS_PER_CELL as u32 > range
            && offset * sim::SUBCELLS_PER_CELL as u32 <= range + bonus
    );
    let target_cell = CellPos::new(cell.x + offset as i32, cell.y);
    let target = spawn_unit(app.world_mut(), PlayerId(1), "rocketeer", target_cell).unwrap();
    let target_entity = unit_entity(&mut app, target);
    app.world_mut().entity_mut(entity).remove::<sim::Weapon>();
    app.world_mut()
        .entity_mut(target_entity)
        .remove::<sim::Weapon>();
    for _ in 0..10 {
        step(&mut app);
    }
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(target_cell)
    );
    app.world_mut().entity_mut(entity).insert(weapon);
    app.world_mut().insert_resource(controllers);
    let events = step(&mut app);
    assert!(app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Attack { units, target: found } if *found == target && units.contains(&actor))), "plain chase range incorrectly excluded an actual current-position air-bonus shot");
    assert!(events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if *attacker == actor && *found == target)));
    step(&mut app);
    assert_eq!(app.world().get::<sim::Mobile>(entity).unwrap().cell, cell);
}

#[test]
fn ra8_every_shipped_plan_places_a_paid_armed_defence_from_its_complete_plan() {
    let definitions = load_ruleset(data()).unwrap();
    let skirmish = load_skirmish(data().join("skirmish.ron"), &definitions).unwrap();
    let ai = load_ai(data().join("ai.ron"), &definitions).unwrap();
    assert_eq!(ai.plans.len(), 11);
    for expected in &ai.plans {
        let faction = if let Some(country) = expected.country {
            skirmish
                .factions
                .iter()
                .find(|f| f.country == Some(country))
                .unwrap()
        } else {
            let country = if expected.side == Some(sim::Side::Allied) {
                "great_britain"
            } else {
                "russia"
            };
            skirmish.factions.iter().find(|f| f.id == country).unwrap()
        };
        let mut app = scenario_mode(
            &faction.id,
            "easy",
            100000,
            0,
            false,
            ScenarioMode {
                fallback_plan: expected.country.is_none(),
                ..ScenarioMode::default()
            },
        );
        quiet(&mut app);
        assert_eq!(
            app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .plan()
                .defences,
            expected.defences
        );
        // Legacy side fallbacks retain their armed structures in build_order. Country
        // overrides must exercise the new sourced-count defence lane itself.
        let defence_kinds = if expected.country.is_some() {
            &expected.defences
        } else {
            &expected.build_order
        };
        let mut placed = None;
        for tick in 0..45000 {
            for event in step(&mut app) {
                if let sim::SimEvent::BuildingPlaced {
                    player: PlayerId(0),
                    building,
                    kind,
                    ..
                } = event
                    && defence_kinds.contains(&kind)
                    && definitions
                        .building_combat(kind)
                        .is_some_and(|terms| terms.weapon.is_some())
                {
                    eprintln!(
                        "{} fallback={}: paid defence {:?} at tick {}",
                        faction.id,
                        expected.country.is_none(),
                        kind,
                        tick + 1
                    );
                    placed = Some((building, kind));
                }
            }
            if placed.is_some() {
                break;
            }
        }
        let (id, kind) = placed.unwrap_or_else(|| {
            let world = app.world_mut();
            let owned: Vec<_> = world
                .query::<(&sim::Owner, &sim::Building)>()
                .iter(world)
                .filter(|(o, _)| o.0 == PlayerId(0))
                .map(|(_, b)| definitions.building(b.kind).unwrap().id.clone())
                .collect();
            panic!(
                "{} fallback={}: no paid armed defence, owned={owned:?}",
                faction.id,
                expected.country.is_none()
            )
        });
        let entity = unit_entity(&mut app, id);
        assert_eq!(
            app.world().get::<sim::Owner>(entity).unwrap().0,
            PlayerId(0)
        );
        assert_eq!(app.world().get::<sim::Building>(entity).unwrap().kind, kind);
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
fn ra8_aircraft_same_decision_admission_counts_existing_queued_and_new_dock_commitments() {
    let mut app = scenario("great_britain", "brutal", 100000, 0, false);
    quiet(&mut app);
    put(&mut app, "barracks", 2, 2);
    put(&mut app, "war_factory", 2, 10);
    put(&mut app, "airforce_command", 2, 16);
    put(&mut app, "tech_center", 2, 25);
    put(&mut app, "refinery", 2, 32);
    // Retain the original worker-plus-combat decision composition with a real
    // economic job; an empty map no longer admits speculative quota workers.
    let ore = app
        .world()
        .resource::<sim::Definitions>()
        .rules()
        .unwrap()
        .economy
        .ore_max_bails as u32;
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(8, 32),
        Cell {
            passable: true,
            buildable: true,
            water: false,
            ore,
        },
    );
    for x in [6, 10, 14] {
        put(&mut app, "power_plant", x, 2);
    }
    for y in 8..11 {
        spawn_unit(app.world_mut(), PlayerId(0), "harrier", CellPos::new(28, y)).unwrap();
    }
    let controllers = app.world_mut().remove_resource::<sim::AiPlayers>().unwrap();
    app.world_mut().insert_resource(sim::AiPlayers::default());
    step(&mut app); // Real Economy/flight initializes the three existing dock commitments.
    app.world_mut().insert_resource(controllers);
    let harrier = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("harrier")
        .unwrap();
    let mut admitted = false;
    for _ in 0..10 {
        step(&mut app);
        let count = app.world().resource::<CommandQueue>().iter().filter(|c| matches!(c.command, Command::QueueProduction { item: sim::ProductionItem::Unit(k) } if k == harrier)).count();
        assert!(
            count <= 1,
            "only one of four docks remains, admitted {count}"
        );
        admitted |= count == 1;
    }
    assert!(
        admitted,
        "the available dock must remain usable: pending={:?}",
        app.world().resource::<CommandQueue>()
    );
    let mut trained = false;
    for _ in 0..2000 {
        trained |= step(&mut app).iter().any(|e| matches!(e, sim::SimEvent::ProductionComplete { player: PlayerId(0), item: sim::ProductionItem::Unit(kind) } if *kind == harrier));
    }
    assert!(
        trained,
        "accepted aircraft must complete through paid production"
    );
}

#[test]
fn ra8_depot_reserves_known_incoming_and_occupied_bays_without_rejected_orders() {
    for first_x in [5, 16] {
        let mut app = scenario("russia", "medium", 100000, 0, false);
        quiet(&mut app);
        let depot = put(&mut app, "soviet_service_depot", 2, 20);
        let first = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "rhino",
            CellPos::new(first_x, 20),
        )
        .unwrap();
        let second =
            spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(7, 20)).unwrap();
        for id in [first, second] {
            let e = unit_entity(&mut app, id);
            app.world_mut()
                .get_mut::<sim::Health>(e)
                .unwrap()
                .hit_points = 10;
        }
        issue(
            &mut app,
            Command::Interact {
                unit: first,
                target: depot,
            },
        );
        step(&mut app);
        let special = app.world().resource::<sim::SpecialState>();
        assert_eq!(
            special
                .docked
                .get(&first)
                .or(special.interactions.get(&first)),
            Some(&depot)
        );
        assert!(!app.world().resource::<CommandQueue>().iter().any(|c| matches!(c.command, Command::Interact { unit, target } if unit == second && target == depot)));
        for _ in 0..600 {
            step(&mut app);
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .docked
                    .values()
                    .filter(|&&host| host == depot)
                    .count()
                    <= 1
            );
        }
        let e = unit_entity(&mut app, first);
        assert!(app.world().get::<sim::Health>(e).unwrap().hit_points > 10);
    }
}

#[test]
fn ra8_source_self_healing_after_a_real_ai_service_decision_cancels_the_healed_job() {
    let mut app = scenario("russia", "medium", 100000, 0, false);
    quiet(&mut app);
    let depot = put(&mut app, "soviet_service_depot", 2, 20);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(5, 20),
    )
    .unwrap();
    let entity = unit_entity(&mut app, miner);
    let definitions = app.world().resource::<sim::Definitions>();
    let kind = definitions.unit_kind("war_miner").unwrap();
    let maximum = definitions.unit_combat(kind).unwrap().hit_points;
    assert!(
        definitions
            .metadata(sim::ProductionItem::Unit(kind))
            .unwrap()
            .self_healing
    );
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = maximum - 1;
    step(&mut app);
    assert!(
        app.world().resource::<CommandQueue>().iter().any(
            |c| matches!(c.command, Command::Interact {unit,target} if unit==miner && target==depot)
        ),
        "actual controller must decide while the miner is damaged"
    );
    assert_eq!(
        app.world().get::<sim::Health>(entity).unwrap().hit_points,
        maximum,
        "the ordinary source self-heal runs after the decision"
    );
    step(&mut app);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .interactions
            .contains_key(&miner)
    );
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .docked
            .contains_key(&miner)
    );
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

fn paid_refinery_repair_after_producer_loss() -> (App, sim::SimId) {
    paid_refinery_repair_after_producer_loss_options(false)
}

fn paid_refinery_repair_after_producer_loss_options(superweapons: bool) -> (App, sim::SimId) {
    paid_refinery_repair_after_producer_loss_for("germany", superweapons)
}

fn paid_refinery_repair_after_producer_loss_for(
    country: &str,
    superweapons: bool,
) -> (App, sim::SimId) {
    let mut app = scenario(country, "medium", 10000, 0, superweapons);
    let refinery = put(
        &mut app,
        if country == "russia" {
            "soviet_refinery"
        } else {
            "refinery"
        },
        14,
        8,
    );
    let entity = unit_entity(&mut app, refinery);
    let maximum = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = maximum - 200;
    let initial = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    let mut enabled = false;
    for _ in 0..100 {
        step(&mut app);
        enabled |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .repairing()
            .contains(&refinery);
        if enabled && app.world().get::<sim::Health>(entity).unwrap().hit_points > maximum - 200 {
            break;
        }
    }
    assert!(
        enabled,
        "the actual controller must enable ordinary paid repair"
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits()
            < initial
    );
    assert!(app.world().get::<sim::Health>(entity).unwrap().hit_points > maximum - 200);
    // Declared loss of all production and its surviving crew; the already active
    // ordinary repair and the enemy's source weapon continue through the schedule.
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    for _ in 0..2 {
        let world = app.world_mut();
        for (id, owner, mut hp) in world
            .query::<(&sim::SimId, &sim::Owner, &mut sim::Health)>()
            .iter_mut(world)
        {
            if owner.0 == PlayerId(0) && *id != refinery {
                hp.hit_points = 0;
            }
        }
        step(&mut app);
    }
    app.world_mut().insert_resource(controllers);
    spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(18, 9)).unwrap();
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .repairing()
            .contains(&refinery)
    );
    (app, refinery)
}

#[test]
fn ra8_powered_support_remnants_without_a_force_do_not_renew_paid_repairs() {
    let mut renewed = Vec::new();
    for (country, source) in [("germany", "chronosphere"), ("russia", "iron_curtain")] {
        let (mut app, refinery) = paid_refinery_repair_after_producer_loss_for(country, true);
        put(&mut app, source, 2, 2);
        if country == "russia" {
            put(&mut app, "nuclear_reactor", 2, 8);
        } else {
            put(&mut app, "power_plant", 2, 14);
            put(&mut app, "power_plant", 2, 20);
        }
        let mut stopped = false;
        let mut paid_changes = 0;
        let mut credits = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        for _ in 0..200 {
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            stopped |= state.repairing().contains(&refinery) && app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery));
            step(&mut app);
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(
                !state.low_power(),
                "{source} control must really have restored power"
            );
            let next = state.credits();
            paid_changes += usize::from(next < credits);
            credits = next;
        }
        let world = app.world_mut();
        assert!(
            world
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .all(|(_, queue)| queue.is_empty())
        );
        assert_eq!(
            world
                .query::<(&sim::Owner, &sim::Unit)>()
                .iter(world)
                .filter(|(owner, _)| owner.0 == PlayerId(0))
                .count(),
            0
        );
        eprintln!("powered support {source} paid changes {paid_changes} repair-off {stopped}");
        if !stopped {
            renewed.push(source);
        }
    }
    assert!(
        renewed.is_empty(),
        "support-only remnants renewed repairs without a force: {renewed:?}"
    );
}

#[test]
fn ra8_powered_offensive_and_recruiting_sources_keep_real_paid_repairs() {
    for (country, source) in [
        ("germany", "weather_control"),
        ("russia", "nuclear_silo"),
        ("america", "american_airforce_command"),
    ] {
        let (mut app, refinery) = paid_refinery_repair_after_producer_loss_for(country, true);
        let source_id = put(&mut app, source, 2, 2);
        if country == "russia" {
            put(&mut app, "nuclear_reactor", 2, 8);
        } else {
            put(&mut app, "power_plant", 2, 14);
            put(&mut app, "power_plant", 2, 20);
        }
        let mut healed = false;
        let entity = unit_entity(&mut app, refinery);
        let mut hp = app.world().get::<sim::Health>(entity).unwrap().hit_points;
        let mut spent = false;
        let mut cash = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        for _ in 0..200 {
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(!state.repairing().contains(&refinery) || !app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery)), "{source}: independent strategic capability keeps repairs");
            step(&mut app);
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(!state.low_power());
            spent |= state.credits() < cash;
            cash = state.credits();
            let next = app.world().get::<sim::Health>(entity).unwrap().hit_points;
            healed |= next > hp;
            hp = next;
        }
        assert!(
            healed && spent,
            "{source} must actually keep paying/healing"
        );
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .charge
                .get(&source_id)
                .is_some_and(|charge| *charge > 0),
            "{source}: normal powered recharge must advance"
        );
    }
}

#[test]
fn ra8_ready_finite_cohorts_use_shorter_assembly_but_keep_the_source_team_delay() {
    let mut app = scenario_mode(
        "germany",
        "medium",
        0,
        0,
        false,
        ScenarioMode {
            map_side: Some(112),
            ..Default::default()
        },
    );
    // A second normal source base leaves a real later objective while the
    // first cohort fights, so an early first-base destruction does not end
    // the separate source-reinforcement observation.
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(100, 100),
    )
    .unwrap();
    let tank = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "grizzly",
        CellPos::new(14, 14),
    )
    .unwrap();
    let rule = app
        .world()
        .resource::<sim::Definitions>()
        .rules()
        .unwrap()
        .difficulties
        .iter()
        .find(|r| r.id == "medium")
        .unwrap()
        .clone();
    let source_delay = u64::from(rule.team_delay.unsigned_abs());
    let first_wave = u64::from(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .first_wave,
    );
    assert_eq!(
        first_wave, source_delay,
        "source first-wave gate stays exact"
    );
    let cadence = u64::from(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let mut joined = None;
    let mut fired = false;
    for _ in 0..first_wave + source_delay + cadence {
        for event in step(&mut app) {
            fired |=
                matches!(event, sim::SimEvent::WeaponFired { attacker, .. } if attacker == tank);
        }
        if app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&tank)
        {
            joined = Some(app.world().resource::<SimTick>().0);
            break;
        }
    }
    eprintln!("finite first cohort sourcefirst{first_wave} delay{source_delay} joined{joined:?}");
    let joined = joined
        .expect("a ready finite cohort must not add another complete source TeamDelay of assembly");
    assert!(joined >= first_wave && joined <= first_wave + source_delay + cadence);
    let gather = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .gather();
    let recruit = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "gi",
        CellPos::new(gather.x + 1, gather.y),
    )
    .unwrap();
    let mut reinforcement = None;
    for _ in 0..source_delay + cadence * 2 {
        for event in step(&mut app) {
            fired |=
                matches!(event, sim::SimEvent::WeaponFired { attacker, .. } if attacker == tank);
        }
        if app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&recruit)
        {
            reinforcement = Some(app.world().resource::<SimTick>().0);
            break;
        }
    }
    eprintln!(
        "finite reinforcement{reinforcement:?} outcome{:?}",
        app.world().resource::<sim::MatchState>().outcome()
    );
    let reinforcement =
        reinforcement.expect("normal later source-gated reinforcement must still join");
    assert!(
        reinforcement >= joined + source_delay,
        "shorter assembly cannot bypass source TeamDelay"
    );
    assert!(
        fired,
        "normal ready-cohort orders must lead to actual source combat"
    );
}

#[test]
fn ra8_an_unpowered_strategic_remnant_cannot_keep_futile_paid_repairs_alive() {
    let (mut app, refinery) = paid_refinery_repair_after_producer_loss_options(true);
    put(&mut app, "chronosphere", 2, 2);
    let mut stopped = false;
    let mut paid_changes = 0;
    let mut credits = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    for _ in 0..200 {
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        stopped |= state.repairing().contains(&refinery) && app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery));
        step(&mut app);
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        assert!(state.low_power());
        let next = state.credits();
        paid_changes += usize::from(next < credits);
        credits = next;
    }
    let world = app.world_mut();
    assert!(
        world
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .all(|(_, queue)| queue.is_empty()),
        "producer-loss cancellation must finish before judging the stranded source"
    );
    assert_eq!(
        world
            .query::<(&sim::Owner, &sim::Unit)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == PlayerId(0))
            .count(),
        0
    );
    eprintln!(
        "unpowered strategic remnant paid credit changes {paid_changes}, ordinary repair-off {stopped}"
    );
    assert!(
        stopped,
        "an unusable Chronosphere without any power-restoration path must not qualify renewed paid repairs"
    );
}

#[test]
fn ra8_an_unpowered_strategic_source_keeps_repairs_during_real_paid_power_recovery() {
    let (mut app, refinery) = paid_refinery_repair_after_producer_loss_options(true);
    put(&mut app, "chronosphere", 2, 2);
    put(&mut app, "construction_yard", 2, 20);
    let power = app
        .world()
        .resource::<sim::Definitions>()
        .building_kind("power_plant")
        .unwrap();
    // The original paid Power head survives the declared producer loss. The
    // replacement yard makes that real head attainable again; do not enqueue
    // a duplicate into the source's one-item construction lane.
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .any(|(_, queue)| queue
                .iter()
                .any(|q| q.item == sim::ProductionItem::Building(power)))
    );
    let mut paid_head = false;
    let mut placed = false;
    let mut recovered = false;
    for _ in 0..2500 {
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        paid_head |= state.queues().any(|(_, queue)| {
            queue
                .iter()
                .any(|q| q.item == sim::ProductionItem::Building(power) && q.charged > 0)
        });
        if state.low_power() {
            assert!(!state.repairing().contains(&refinery) || !app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery)), "actual queued/buildable power recovery keeps useful repairs");
        }
        for event in step(&mut app) {
            placed |= matches!(event, sim::SimEvent::BuildingPlaced { player: PlayerId(0), kind, .. } if kind == power);
        }
        recovered |= !app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power();
        if paid_head && placed && recovered {
            break;
        }
    }
    assert!(
        paid_head && placed && recovered,
        "ordinary paid power head must actually place and restore power"
    );
}

#[test]
fn ra8_an_unpowered_chronosphere_keeps_repairs_for_its_real_support_force() {
    for (country, source, kind) in [
        ("germany", "chronosphere", "grizzly"),
        ("russia", "iron_curtain", "rhino"),
    ] {
        let (mut app, refinery) = paid_refinery_repair_after_producer_loss_for(country, true);
        put(&mut app, source, 2, 2);
        let tank = spawn_unit(app.world_mut(), PlayerId(0), kind, CellPos::new(13, 12)).unwrap();
        for _ in 0..200 {
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(!state.repairing().contains(&refinery) || !app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery)), "{source}: a living support force is an independent recovery path");
            step(&mut app);
        }
        let tank_entity = unit_entity(&mut app, tank);
        assert!(
            app.world()
                .get::<sim::Health>(tank_entity)
                .is_some_and(|h| h.hit_points > 0)
        );
    }
}

#[test]
fn ra8_futile_paid_building_repairs_do_not_renew_a_helpless_last_stand_forever() {
    let (mut app, refinery) = paid_refinery_repair_after_producer_loss();
    let mut stopped = false;
    let mut changes = 0;
    let mut credits = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    for _ in 0..9000 {
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        stopped |= state.repairing().contains(&refinery) && app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery));
        step(&mut app);
        let next = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        changes += usize::from(next != credits);
        credits = next;
        if app
            .world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_some()
        {
            break;
        }
    }
    eprintln!(
        "helpless paid repair credit changes {changes}, stop {stopped}, outcome {:?}",
        app.world().resource::<sim::MatchState>().outcome()
    );
    assert!(
        stopped,
        "futile own repair must stop through its ordinary toggle"
    );
    assert!(
        matches!(
            app.world().resource::<sim::MatchState>().outcome(),
            Some(sim::MatchOutcome::Won { .. })
        ),
        "helpless repair spending kept resetting last-stand patience"
    );
}

#[test]
fn ra8_income_production_and_a_usable_force_keep_useful_building_repairs() {
    for recovery in ["oil_derrick", "construction_yard", "grizzly"] {
        let (mut app, refinery) = paid_refinery_repair_after_producer_loss();
        if recovery == "grizzly" {
            spawn_unit(app.world_mut(), PlayerId(0), recovery, CellPos::new(13, 12)).unwrap();
        } else {
            put(&mut app, recovery, 2, 2);
        }
        let entity = unit_entity(&mut app, refinery);
        let mut previous = app.world().get::<sim::Health>(entity).unwrap().hit_points;
        let mut healed = false;
        for _ in 0..200 {
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(!state.repairing().contains(&refinery) || !app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == refinery)), "{recovery}: useful repair was cancelled");
            step(&mut app);
            let hp = app.world().get::<sim::Health>(entity).unwrap().hit_points;
            healed |= hp > previous;
            previous = hp;
        }
        assert!(healed, "{recovery}: ordinary repair did not heal");
        assert!(
            app.world()
                .resource::<sim::MatchState>()
                .outcome()
                .is_none()
        );
    }
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
fn ra8_garrison_quota_and_capacity_include_known_incoming_occupiers() {
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
    let mut incoming = Vec::new();
    for y in 8..11 {
        let unit = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(24, y)).unwrap();
        incoming.push(unit);
    }
    let extra = spawn_unit(app.world_mut(), PlayerId(0), "gi", CellPos::new(12, 8)).unwrap();
    let controllers = app.world_mut().remove_resource::<sim::AiPlayers>().unwrap();
    app.world_mut().insert_resource(sim::AiPlayers::default());
    step(&mut app); // Establish real visibility before the three ordinary Interact orders.
    app.world_mut().insert_resource(controllers);
    for &unit in &incoming {
        issue(&mut app, Command::Interact { unit, target: host });
    }
    for _ in 0..20 {
        step(&mut app);
    }
    assert!(!app.world().resource::<CommandQueue>().iter().any(|c| matches!(c.command, Command::Interact { unit, target } if unit == extra && target == host)));
    for _ in 0..300 {
        step(&mut app);
    }
    let state = app.world().resource::<sim::SpecialState>();
    let cargo = state.cargo.get(&host).unwrap();
    assert_eq!(cargo.len(), 3);
    assert!(incoming.iter().all(|id| cargo.iter().any(|p| p.id == *id)));
    assert!(!cargo.iter().any(|p| p.id == extra));
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
    if !paid {
        let world = app.world_mut();
        let controller = world
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        eprintln!(
            "escort search target {:?}, known {:?}, naval {:?}, first {:?}",
            controller.target(),
            controller.known_buildings(),
            controller.naval_target(),
            controller.waypoints().first()
        );
        eprintln!(
            "escort fixture units {:?}",
            world
                .query::<(&sim::Owner, &sim::Unit, &sim::Mobile)>()
                .iter(world)
                .map(|(o, u, m)| (o.0, defs.unit(u.kind).unwrap().id.as_str(), m.cell))
                .collect::<Vec<_>>()
        );
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
    unseen_enemy_water_building(CellPos::new(16, 23));
}

#[test]
fn ra8_unseen_water_obstacle_before_the_edge_range_endpoint_cannot_change_naval_planning() {
    unseen_enemy_water_building(CellPos::new(16, 21));
}

fn unseen_enemy_water_building(cell: CellPos) {
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
        let carrier = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "aircraft_carrier",
            CellPos::new(16, 11),
        )
        .unwrap();
        // Hold through ordinary public orders until the comparison decision;
        // otherwise the live hull can genuinely reveal the interposed cell.
        issue(
            &mut app,
            Command::Guard {
                units: vec![carrier],
            },
        );
        // A real local observer keeps the distant firing target available while
        // the interposed water cell stays unseen. The separate Gap regression
        // covers losing that observation through ordinary movement.
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(38, 34),
        )
        .unwrap();
        for _ in 0..21 {
            issue(
                &mut app,
                Command::Guard {
                    units: vec![carrier],
                },
            );
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
        assert!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(38, 38))
        );
        issue(
            &mut app,
            Command::Guard {
                units: vec![carrier],
            },
        );
        step(&mut app);
        (app, carrier)
    };
    let (mut actual, actual_carrier) = make();
    let (mut control, control_carrier) = make();
    assert_eq!(actual_carrier, control_carrier);
    let definitions = load_ai(
        data().join("ai.ron"),
        actual.world().resource::<sim::Definitions>(),
    )
    .unwrap();
    let rescan = definitions.policy.naval_rescan;
    assert_eq!(rescan + 1, 601);
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
        if actual.world().resource::<SimTick>().0 < u64::from(rescan - 1) {
            for (app, carrier) in [
                (&mut actual, actual_carrier),
                (&mut control, control_carrier),
            ] {
                issue(
                    app,
                    Command::Guard {
                        units: vec![carrier],
                    },
                );
            }
        } else if actual.world().resource::<SimTick>().0 == u64::from(rescan - 1) {
            for (app, carrier) in [
                (&mut actual, actual_carrier),
                (&mut control, control_carrier),
            ] {
                issue(
                    app,
                    Command::Move {
                        units: vec![carrier],
                        target: CellPos::new(16, 11),
                    },
                );
            }
        }
        if actual.world().resource::<SimTick>().0 == u64::from(rescan) {
            for (app, carrier) in [
                (&mut actual, actual_carrier),
                (&mut control, control_carrier),
            ] {
                let entity = unit_entity(app, carrier);
                assert_eq!(
                    app.world().get::<sim::Mobile>(entity).unwrap().cell,
                    CellPos::new(16, 11)
                );
                assert!(
                    app.world()
                        .get::<sim::Orders>(entity)
                        .unwrap()
                        .current
                        .is_none(),
                    "ordinary Move must release Guard before the live-hull decision"
                );
                assert!(
                    app.world()
                        .get::<sim::Weapon>(entity)
                        .unwrap()
                        .order
                        .is_none()
                );
            }
        }
        step(&mut actual);
        step(&mut control);
        for app in [&actual, &control] {
            let vision = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision();
            assert!(
                vision.is_visible(CellPos::new(38, 38)),
                "the observer must keep the intended yard visible through comparison"
            );
            assert!(
                !vision.is_visible(cell),
                "the interposed water cell must remain unseen through comparison"
            );
        }
    }
    for (app, carrier) in [(&actual, actual_carrier), (&control, control_carrier)] {
        assert!(app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::AttackMove { units, target } if units.contains(&carrier) && *target != CellPos::new(16, 11))), "the released live hull must receive a fresh qualified mission at the comparison decision");
    }
    for app in [&actual, &control] {
        let vision = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision();
        assert!(
            vision.is_visible(CellPos::new(38, 38)),
            "the observer must really reveal the intended yard"
        );
        assert!(
            !vision.is_visible(cell),
            "the interposed water building must remain unseen in both twins"
        );
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

#[test]
fn ra8_observed_scouting_goals_release_a_crowded_wave_without_waiting_for_slow_aircraft() {
    for observed in [false, true] {
        let mut app = scenario("russia", "easy", 0, 0, false);
        let tanks: Vec<_> = (0..8)
            .map(|i| {
                spawn_unit(
                    app.world_mut(),
                    PlayerId(0),
                    "rhino",
                    CellPos::new(14 + i % 4, 14 + i / 4),
                )
                .unwrap()
            })
            .collect();
        let air = spawn_unit(app.world_mut(), PlayerId(0), "kirov", CellPos::new(12, 18)).unwrap();
        let old = loop {
            step(&mut app);
            let controller = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap();
            if controller.wave().contains(&air)
                && let Some(target @ sim::WaveTarget::Waypoint(_)) = controller.target()
            {
                break target;
            }
            assert!(app.world().resource::<SimTick>().0 < 10_000);
        };
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
        let sim::WaveTarget::Waypoint(index) = old else {
            unreachable!()
        };
        let goal = controllers.controller(PlayerId(0)).unwrap().waypoints()[index].cell;
        issue(
            &mut app,
            Command::Stop {
                units: tanks.iter().copied().chain([air]).collect(),
            },
        );
        if observed {
            for (i, &tank) in tanks.iter().enumerate() {
                issue(
                    &mut app,
                    Command::Move {
                        units: vec![tank],
                        target: CellPos::new(
                            goal.x - 3 - (i % 4) as i32,
                            goal.y - 3 - (i / 4) as i32,
                        ),
                    },
                );
            }
        }
        for _ in 0..2000 {
            step(&mut app);
            if tanks.iter().all(|id| {
                let e = unit_entity(&mut app, *id);
                app.world().get::<sim::Orders>(e).unwrap().current.is_none()
            }) {
                break;
            }
        }
        assert_eq!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(goal),
            observed
        );
        // A genuine slow aircraft move remains in progress at the scheduled decision.
        issue(
            &mut app,
            Command::Move {
                units: vec![air],
                target: CellPos::new(1, 46),
            },
        );
        step(&mut app);
        while !app.world().resource::<SimTick>().0.is_multiple_of(40) {
            step(&mut app);
        }
        let entity = unit_entity(&mut app, air);
        assert!(
            app.world()
                .get::<sim::Orders>(entity)
                .unwrap()
                .current
                .is_some()
        );
        app.world_mut().insert_resource(controllers);
        step(&mut app);
        let next = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .target();
        if observed {
            assert_ne!(
                next,
                Some(old),
                "an observed scouting objective is already complete despite the slow aircraft"
            );
        } else {
            assert_eq!(
                next,
                Some(old),
                "legitimate travel toward an unobserved objective must continue"
            );
        }
    }
}

#[test]
fn ra8_aircraft_complete_real_ammo_return_and_reload_before_ai_retasks_them() {
    let mut app = scenario("germany", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "power_plant", 4, 4);
    put(&mut app, "airforce_command", 16, 8);
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(34, 16),
    )
    .unwrap();
    let target_entity = unit_entity(&mut app, target);
    app.world_mut()
        .get_mut::<sim::Health>(target_entity)
        .unwrap()
        .hit_points = 1_000_000;
    spawn_unit(app.world_mut(), PlayerId(0), "spy", CellPos::new(32, 17)).unwrap();
    let aircraft: Vec<_> = [10, 12, 14]
        .into_iter()
        .map(|x| spawn_unit(app.world_mut(), PlayerId(0), "harrier", CellPos::new(x, 14)).unwrap())
        .collect();
    let entity = unit_entity(&mut app, aircraft[0]);
    let burst = app
        .world()
        .resource::<sim::Definitions>()
        .weapon(app.world().get::<sim::Weapon>(entity).unwrap().kind)
        .unwrap()
        .burst as usize;
    // Initialize real dock commitments/visibility before the first controller decision.
    step(&mut app);
    app.world_mut().insert_resource(controllers);
    let mut shots = std::collections::BTreeMap::<_, usize>::new();
    let mut empty = false;
    let mut reload_wait = false;
    for _ in 0..14000 {
        for event in step(&mut app) {
            if let sim::SimEvent::WeaponFired { attacker, .. } = event
                && aircraft.contains(&attacker)
            {
                *shots.entry(attacker).or_default() += 1;
            }
        }
        let special = app.world().resource::<sim::SpecialState>();
        empty |= aircraft.iter().any(|id| special.ammo.get(id) == Some(&0));
        reload_wait |= aircraft
            .iter()
            .any(|id| special.reloading.get(id).is_some_and(|wait| *wait > 20));
        if shots.values().any(|count| *count > burst) && reload_wait {
            break;
        }
    }
    eprintln!(
        "actual shots {shots:?}; sawempty {empty}, uninterrupted reload {reload_wait}, special {:?}",
        app.world().resource::<sim::SpecialState>()
    );
    assert!(empty, "actual combat must expend aircraft ammunition");
    assert!(
        reload_wait,
        "AI must leave the sourced reload timer uninterrupted"
    );
    assert!(
        shots.values().any(|count| *count > burst),
        "a real aircraft must fire again after returning and reloading"
    );
}

#[test]
fn ra8_lost_prerequisites_cancel_only_incomplete_paid_heads_then_rebuild_and_resume() {
    for completed in [false, true] {
        let mut app = scenario("russia", "medium", 10_000, 0, false);
        quiet(&mut app);
        let definitions = app.world().resource::<sim::Definitions>().clone();
        let factory = definitions.building_kind("soviet_war_factory").unwrap();
        let barracks = definitions.building_kind("soviet_barracks").unwrap();
        let item = sim::ProductionItem::Building(factory);
        let build_time = definitions.production(item).unwrap().build_time;
        for _ in 0..14000 {
            step(&mut app);
            if app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .any(|(_, queue)| {
                    queue.first().is_some_and(|head| {
                        head.item == item && head.charged > 0 && head.progress < build_time
                    })
                })
            {
                break;
            }
        }
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
        if completed {
            for _ in 0..4000 {
                if app
                    .world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .queues()
                    .any(|(_, queue)| {
                        queue
                            .first()
                            .is_some_and(|head| head.item == item && head.progress >= build_time)
                    })
                {
                    break;
                }
                step(&mut app);
            }
        }
        let head = *app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .find_map(|(_, queue)| queue.first().filter(|head| head.item == item))
            .unwrap();
        assert!(head.charged > 0);
        assert_eq!(head.progress >= build_time, completed);
        let missing = {
            let world = app.world_mut();
            *world
                .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
                .iter(world)
                .find(|(_, owner, building)| owner.0 == PlayerId(0) && building.kind == barracks)
                .unwrap()
                .0
        };
        issue(&mut app, Command::SellBuilding { building: missing });
        step(&mut app);
        let owned: Vec<_> = {
            let world = app.world_mut();
            world
                .query::<(&sim::Owner, &sim::Building)>()
                .iter(world)
                .filter(|(owner, _)| owner.0 == PlayerId(0))
                .map(|(_, building)| building.kind)
                .collect()
        };
        assert!(!owned.contains(&barracks));
        assert!(sim::missing_prerequisite(&definitions, &owned, item).is_some());
        app.world_mut().insert_resource(controllers);
        let mut refund = None;
        let mut rebuilt = false;
        let mut resumed = false;
        for _ in 0..10000 {
            for event in step(&mut app) {
                match event {
                    sim::SimEvent::ProductionCancelled {
                        player: PlayerId(0),
                        item: found,
                        refund: value,
                    } if found == item => refund = Some(value),
                    sim::SimEvent::BuildingPlaced {
                        player: PlayerId(0),
                        kind,
                        ..
                    } if kind == barracks => rebuilt = true,
                    sim::SimEvent::BuildingPlaced {
                        player: PlayerId(0),
                        kind,
                        ..
                    } if kind == factory => resumed = true,
                    _ => {}
                }
            }
            if rebuilt && resumed {
                break;
            }
        }
        if completed {
            assert_eq!(refund, None, "completed paid items remain placeable");
        } else {
            assert_eq!(
                refund,
                Some(head.charged),
                "ordinary cancellation refunds exactly the actual charged credits"
            );
        }
        assert!(
            rebuilt,
            "the missing prerequisite must be rebuilt through ordinary production"
        );
        assert!(resumed, "normal paid factory production must resume");
    }
}

#[test]
fn ra8_recovery_preserves_unpaid_low_power_and_disabled_heads_until_actual_producer_loss() {
    let mut app = scenario("russia", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "soviet_barracks", 16, 8);
    let factory = put(&mut app, "soviet_war_factory", 16, 12);
    spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(20, 20)).unwrap();
    step(&mut app);
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("rhino")
        .unwrap();
    let item = sim::ProductionItem::Unit(kind);
    issue(&mut app, Command::QueueProduction { item });
    step(&mut app);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .disabled
        .insert(factory, 1000);
    app.world_mut().insert_resource(controllers);
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power()
    );
    for _ in 0..300 {
        assert!(!step(&mut app).iter().any(|event| matches!(event, sim::SimEvent::ProductionCancelled { item: found, .. } if *found == item)));
    }
    let head = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .queues()
        .find_map(|(_, queue)| queue.first().filter(|head| head.item == item))
        .unwrap();
    assert_eq!((head.charged, head.progress), (0, 0));
    issue(&mut app, Command::SellBuilding { building: factory });
    let mut cancelled = false;
    for _ in 0..200 {
        cancelled |= step(&mut app).iter().any(|event| matches!(event, sim::SimEvent::ProductionCancelled { player: PlayerId(0), item: found, refund: 0 } if *found == item));
    }
    assert!(
        cancelled,
        "physical producer loss, unlike a temporary disable/funds/power shortage, releases the blocked lane"
    );
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .any(|(_, queue)| queue.iter().any(|queued| queued.item == item)),
        "ordinary repeated cancellation must release the head as well as any later duplicate"
    );
}

#[test]
fn ra8_bridge_intents_wait_for_known_holders_and_arrivals_without_reading_hidden_ships() {
    for mode in [
        "empty",
        "standing",
        "departing",
        "reserved",
        "incoming",
        "idle_adjacent",
        "visible_enemy",
        "hidden_enemy",
        "amphibious",
    ] {
        let mut app = scenario("russia", "medium", 0, 0, false);
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
        let mut looks = vec![0; 48 * 48];
        {
            let mut grid = app.world_mut().resource_mut::<Grid>();
            for y in 0..48 {
                for x in 22..=24 {
                    grid.set_terrain(
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
            for x in 22..=24 {
                grid.set_terrain(
                    CellPos::new(x, 24),
                    Cell {
                        passable: true,
                        water: false,
                        buildable: false,
                        ore: 0,
                    },
                );
                looks[24 * 48 + x as usize] = 8 << 4;
            }
            for (x, y) in [(21, 22), (25, 22)] {
                looks[y * 48 + x] = 15 << 4;
            }
            grid.mark_world_features(&looks);
        }
        sim::install_world_objects(app.world_mut(), true);
        let bridge = *app
            .world()
            .resource::<sim::Bridges>()
            .bridges
            .keys()
            .next()
            .unwrap();
        let hut = app
            .world()
            .resource::<Grid>()
            .building(CellPos::new(21, 22))
            .unwrap();
        let tank = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "grizzly",
            CellPos::new(20, 24),
        )
        .unwrap();
        step(&mut app);
        issue(
            &mut app,
            Command::Attack {
                units: vec![tank],
                target: bridge,
            },
        );
        for _ in 0..4000 {
            step(&mut app);
            if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
                break;
            }
        }
        assert!(app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
        issue(&mut app, Command::Stop { units: vec![tank] });
        step(&mut app);
        let engineer = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(20, 22),
        )
        .unwrap();
        let ship = if mode == "empty" {
            None
        } else {
            let owner = if mode.ends_with("enemy") {
                PlayerId(1)
            } else {
                PlayerId(0)
            };
            let kind = if mode == "amphibious" {
                "amphibious_transport"
            } else if mode == "visible_enemy" {
                "destroyer"
            } else {
                "typhoon"
            };
            let y = if ["reserved", "incoming", "idle_adjacent"].contains(&mode) {
                23
            } else {
                24
            };
            Some(spawn_unit(app.world_mut(), owner, kind, CellPos::new(23, y)).unwrap())
        };
        step(&mut app);
        let offset = if ["departing", "reserved"].contains(&mode) {
            19
        } else {
            0
        };
        while app.world().resource::<SimTick>().0 % 20 != offset {
            step(&mut app);
        }
        if let Some(ship) = ship {
            if ["departing", "reserved"].contains(&mode) {
                issue(
                    &mut app,
                    Command::Move {
                        units: vec![ship],
                        target: CellPos::new(23, if mode == "departing" { 20 } else { 24 }),
                    },
                );
                step(&mut app);
                let e = unit_entity(&mut app, ship);
                let mobile = app.world().get::<sim::Mobile>(e).unwrap();
                let travel = mobile.step.as_ref().unwrap();
                if mode == "reserved" {
                    assert_eq!(travel.to, CellPos::new(23, 24));
                } else {
                    assert_eq!(mobile.cell, CellPos::new(23, 24));
                }
            } else if mode == "incoming" {
                // Command stage installs the active path; Pathfinding reserves later this tick.
                issue(
                    &mut app,
                    Command::Move {
                        units: vec![ship],
                        target: CellPos::new(23, 24),
                    },
                );
            }
            if mode == "hidden_enemy" {
                assert!(
                    app.world()
                        .resource::<sim::SpecialState>()
                        .hidden
                        .contains(&(PlayerId(0), ship)),
                    "concealment must be derived by the real schedule"
                );
            }
        }
        let engineer_entity = unit_entity(&mut app, engineer);
        assert!(
            app.world()
                .get::<sim::Health>(engineer_entity)
                .unwrap()
                .hit_points
                > 0,
            "{mode}"
        );
        app.world_mut().insert_resource(controllers);
        step(&mut app);
        if mode == "incoming" {
            let e = unit_entity(&mut app, ship.unwrap());
            assert_eq!(
                app.world()
                    .get::<sim::Mobile>(e)
                    .unwrap()
                    .step
                    .as_ref()
                    .unwrap()
                    .to,
                CellPos::new(23, 24)
            );
        }
        let repairs = app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command,Command::Interact {unit,target} if unit==engineer && target==hut));
        assert_eq!(
            repairs,
            ["empty", "idle_adjacent", "hidden_enemy", "amphibious"].contains(&mode),
            "{mode}"
        );
    }
}

#[test]
fn ra8_slow_air_sweep_starts_at_enemy_origin_then_finds_an_unseen_remote_capture() {
    let definitions = load_ruleset(data()).unwrap();
    let mut skirmish = load_skirmish(data().join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(112, 112)];
    skirmish.starting_credits = 0;
    let ai = load_ai(data().join("ai.ron"), &definitions).unwrap();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 19 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            128,
            128,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0
                };
                128 * 128
            ],
        )
        .unwrap(),
    );
    app.world_mut().insert_resource(definitions);
    let players = ["russia", "great_britain"].map(|country| {
        skirmish
            .factions
            .iter()
            .position(|f| f.id == country)
            .unwrap()
    });
    let medium = ai
        .difficulties
        .iter()
        .position(|d| d.id == "medium")
        .unwrap();
    setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &[
            PlayerSetup {
                player: PlayerId(0),
                faction: players[0],
                start: 0,
                controller: Controller::Ai { difficulty: medium },
            },
            PlayerSetup {
                player: PlayerId(1),
                faction: players[1],
                start: 1,
                controller: Controller::Human,
            },
        ],
        &MatchOptions {
            mcv_start: false,
            unit_count: Some(0),
            crates: false,
            superweapons: false,
            ..MatchOptions::default()
        },
    )
    .unwrap();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let remote = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "refinery",
        CellPos::new(8, 104),
    )
    .unwrap();
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "engineer",
        CellPos::new(7, 104),
    )
    .unwrap();
    spawn_unit(app.world_mut(), PlayerId(0), "kirov", CellPos::new(12, 12)).unwrap();
    step(&mut app);
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Interact {
                unit: engineer,
                target: remote,
            },
        });
    for _ in 0..100 {
        step(&mut app);
        let entity = unit_entity(&mut app, remote);
        if app.world().get::<sim::Owner>(entity).unwrap().0 == PlayerId(1) {
            break;
        }
    }
    let entity = unit_entity(&mut app, remote);
    assert_eq!(
        app.world().get::<sim::Owner>(entity).unwrap().0,
        PlayerId(1)
    );
    let yard_kind = app
        .world()
        .resource::<sim::Definitions>()
        .building_kind("construction_yard")
        .unwrap();
    let yard = {
        let world = app.world_mut();
        *world
            .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
            .iter(world)
            .find(|(_, owner, building)| owner.0 == PlayerId(1) && building.kind == yard_kind)
            .unwrap()
            .0
    };
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::SellBuilding { building: yard },
        });
    step(&mut app);
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(8, 104))
    );
    assert!(
        !controllers
            .controller(PlayerId(0))
            .unwrap()
            .known_buildings()
            .contains_key(&remote)
    );
    app.world_mut().insert_resource(controllers);
    let mut first = None;
    let mut discovered = false;
    let mut outcome = None;
    // The unchanged R29 limit scaled to this 128x128 fixture.
    for _ in app.world().resource::<SimTick>().0..144000 {
        step(&mut app);
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        if first.is_none()
            && let Some(sim::WaveTarget::Waypoint(index)) = controller.target()
        {
            first = Some(controller.waypoints()[index]);
        }
        discovered |= controller.known_buildings().contains_key(&remote);
        outcome = app.world().resource::<sim::MatchState>().outcome();
        if outcome.is_some() {
            break;
        }
    }
    eprintln!(
        "remote captured refinery search: tick {:?}, first {first:?}, discovered {discovered}, outcome {outcome:?}",
        app.world().resource::<SimTick>()
    );
    let first = first.expect("a real scouting wave");
    assert!(
        first.priority && first.cell == CellPos::new(112, 112),
        "the original enemy start retains first-search priority: {first:?}"
    );
    assert!(
        discovered,
        "the remote captured building must first be discovered through real vision"
    );
    assert!(
        matches!(
            outcome,
            Some(sim::MatchOutcome::Won {
                winner: PlayerId(0),
                ..
            })
        ),
        "slow-air remote sweep: {outcome:?}"
    );
}

#[test]
fn ra8_air_strikes_wait_for_ground_to_clear_an_observed_outranging_aa_screen() {
    let mut app = scenario("germany", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 10, 2);
    put(&mut app, "airforce_command", 2, 12);
    let economic = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(30, 12),
    )
    .unwrap();
    // Isolate protection/clearance from the economic target dying to ground splash first.
    let economic_entity = unit_entity(&mut app, economic);
    app.world_mut()
        .get_mut::<sim::Health>(economic_entity)
        .unwrap()
        .hit_points = 1_000_000;
    let aa = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "patriot",
        CellPos::new(34, 14),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "nuclear_reactor",
        CellPos::new(34, 2),
    )
    .unwrap();
    let ground: Vec<_> = [
        ("prism_tank", 15, 15),
        ("prism_tank", 16, 15),
        ("grizzly", 15, 17),
        ("grizzly", 16, 17),
    ]
    .into_iter()
    .map(|(kind, x, y)| spawn_unit(app.world_mut(), PlayerId(0), kind, CellPos::new(x, y)).unwrap())
    .collect();
    let air: Vec<_> = [10, 12, 14]
        .into_iter()
        .map(|x| spawn_unit(app.world_mut(), PlayerId(0), "harrier", CellPos::new(x, 14)).unwrap())
        .collect();
    step(&mut app); // Real visibility, power, and dock assignment before the decision.
    app.world_mut().insert_resource(controllers);
    let mut ground_focus = false;
    let mut ground_hit = false;
    let mut aircraft_released = false;
    let mut screen_destroyed = false;
    for _ in 0..12000 {
        for event in step(&mut app) {
            if let sim::SimEvent::BuildingDestroyed { building, .. } = event
                && building == aa
            {
                screen_destroyed = true;
            }
            if let sim::SimEvent::WeaponFired {
                attacker, target, ..
            } = event
            {
                if ground.contains(&attacker) && target == aa {
                    ground_hit = true;
                }
                if air.contains(&attacker) && target == economic {
                    assert!(
                        screen_destroyed,
                        "strike aircraft attacked the economic building through its observed outranging AA screen"
                    );
                    aircraft_released = true;
                }
            }
        }
        ground_focus |= app.world().resource::<CommandQueue>().iter().any(|issued|matches!(&issued.command,Command::Attack {units,target} if *target==aa && units.iter().any(|id|ground.contains(id))));
        if screen_destroyed && ground_hit && aircraft_released {
            break;
        }
    }
    eprintln!(
        "AA screen destroyed {screen_destroyed}; eligible ground focus {ground_focus}/hit {ground_hit}; aircraft released {aircraft_released}"
    );
    if !ground_focus || !ground_hit || !aircraft_released {
        dump_screen(&mut app);
    }
    assert!(
        ground_focus && ground_hit,
        "eligible ground units must clear the known AA, not let strike aircraft repeatedly pay for the same barrier"
    );
    assert!(
        screen_destroyed && aircraft_released,
        "air strikes resume actual combat after the observed barrier is gone"
    );
}

#[test]
fn ra8_unlimited_ammo_bombers_wait_for_ground_to_clear_known_outranging_aa() {
    let mut app = scenario("germany", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 10, 2);
    put(&mut app, "airforce_command", 2, 12);
    let economic = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(30, 12),
    )
    .unwrap();
    // Isolate protection/clearance from the economic target dying to ground splash first.
    let economic_entity = unit_entity(&mut app, economic);
    app.world_mut()
        .get_mut::<sim::Health>(economic_entity)
        .unwrap()
        .hit_points = 1_000_000;
    let aa = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "patriot",
        CellPos::new(34, 14),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "nuclear_reactor",
        CellPos::new(34, 2),
    )
    .unwrap();
    let ground: Vec<_> = [
        ("prism_tank", 15, 15),
        ("prism_tank", 16, 15),
        ("grizzly", 15, 17),
        ("grizzly", 16, 17),
    ]
    .into_iter()
    .map(|(kind, x, y)| spawn_unit(app.world_mut(), PlayerId(0), kind, CellPos::new(x, y)).unwrap())
    .collect();
    let air: Vec<_> = [10, 12, 14]
        .into_iter()
        .map(|x| spawn_unit(app.world_mut(), PlayerId(0), "kirov", CellPos::new(x, 14)).unwrap())
        .collect();
    let (aa_footprint, aa_reach) = {
        let defs = app.world().resource::<sim::Definitions>();
        let bomber = defs.unit_kind("kirov").unwrap();
        assert_eq!(
            defs.metadata(sim::ProductionItem::Unit(bomber))
                .unwrap()
                .ammo,
            0
        );
        let terms = defs
            .building_combat(defs.building_kind("patriot").unwrap())
            .unwrap();
        let reach = terms
            .weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|kind| defs.weapon(kind))
            .filter(|weapon| weapon.anti_air)
            .map(|weapon| weapon.range.saturating_add(terms.air_range_bonus))
            .max()
            .unwrap();
        (
            sim::Footprint {
                origin: CellPos::new(34, 14),
                width: 1,
                height: 1,
            },
            reach.div_ceil(sim::SUBCELLS_PER_CELL as u32),
        )
    };
    step(&mut app); // Real visibility and power before the decision.
    app.world_mut().insert_resource(controllers);
    let mut safe_wait = false;
    let mut ground_focus = false;
    let mut ground_hit = false;
    let mut aircraft_released = false;
    let mut screen_destroyed = false;
    for _ in 0..12000 {
        for event in step(&mut app) {
            if let sim::SimEvent::BuildingDestroyed { building, .. } = event
                && building == aa
            {
                screen_destroyed = true;
            }
            if let sim::SimEvent::WeaponFired {
                attacker, target, ..
            } = event
            {
                if ground.contains(&attacker) && target == aa {
                    ground_hit = true;
                }
                if air.contains(&attacker) && target == economic {
                    assert!(
                        screen_destroyed,
                        "strike aircraft attacked the economic building through its observed outranging AA screen"
                    );
                    aircraft_released = true;
                }
            }
        }
        for issued in app.world().resource::<CommandQueue>().iter() {
            if let Command::Move { units, target } = &issued.command
                && units.iter().any(|id| air.contains(id))
            {
                assert!(
                    aa_footprint.distance_to_cell(*target) > aa_reach,
                    "bomber wait must stay beyond the known source AA envelope: {target:?}"
                );
                safe_wait = true;
            }
        }
        ground_focus |= app.world().resource::<CommandQueue>().iter().any(|issued|matches!(&issued.command,Command::Attack {units,target} if *target==aa && units.iter().any(|id|ground.contains(id))));
        if screen_destroyed && ground_hit && aircraft_released {
            break;
        }
    }
    eprintln!(
        "unlimited bombers safe wait {safe_wait}; AA screen destroyed {screen_destroyed}; eligible ground focus {ground_focus}/hit {ground_hit}; aircraft released {aircraft_released}"
    );
    if !ground_focus || !ground_hit || !aircraft_released {
        dump_screen(&mut app);
    }
    assert!(
        safe_wait && ground_focus && ground_hit,
        "eligible ground units must clear the known AA, not let strike aircraft repeatedly pay for the same barrier"
    );
    assert!(
        screen_destroyed && aircraft_released,
        "air strikes resume actual combat after the observed barrier is gone"
    );
}

fn distant_screen(blocked: bool) -> (App, Vec<sim::SimId>, Vec<sim::SimId>, sim::SimId) {
    distant_screen_options(blocked, false)
}

fn distant_screen_options(
    blocked: bool,
    superweapons: bool,
) -> (App, Vec<sim::SimId>, Vec<sim::SimId>, sim::SimId) {
    distant_screen_aircraft(blocked, superweapons, "harrier")
}

fn distant_screen_aircraft(
    blocked: bool,
    superweapons: bool,
    aircraft: &str,
) -> (App, Vec<sim::SimId>, Vec<sim::SimId>, sim::SimId) {
    let mut app = scenario("germany", "medium", 0, 0, superweapons);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 10, 2);
    put(&mut app, "airforce_command", 2, 12);
    let economic = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(34, 12),
    )
    .unwrap();
    // Keep the observed objective alive while testing the approach, not damage balance.
    let entity = unit_entity(&mut app, economic);
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = 1_000_000;
    let aa = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "patriot",
        CellPos::new(38, 14),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "nuclear_reactor",
        CellPos::new(42, 2),
    )
    .unwrap();
    if blocked {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 0..48 {
            grid.set_terrain(
                CellPos::new(22, y),
                Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0,
                },
            );
        }
    }
    // Establish a ready wave on the actual clock before actors exist. Otherwise idle
    // artillery can shoot the interceptor before the AI's first offensive decision.
    for _ in 0..6000 {
        step(&mut app);
    }
    let ground = [
        ("prism_tank", 15, 15),
        ("prism_tank", 16, 15),
        ("grizzly", 15, 17),
        ("grizzly", 16, 17),
    ]
    .into_iter()
    .map(|(kind, x, y)| spawn_unit(app.world_mut(), PlayerId(0), kind, CellPos::new(x, y)).unwrap())
    .collect();
    let air = [10, 12, 14]
        .into_iter()
        .map(|x| spawn_unit(app.world_mut(), PlayerId(0), aircraft, CellPos::new(x, 14)).unwrap())
        .collect();
    step(&mut app);
    app.world_mut().insert_resource(controllers);
    (app, ground, air, aa)
}

#[test]
fn ra8_distant_aa_clearance_fights_an_observed_interceptor_en_route() {
    let (mut app, ground, _, aa) = distant_screen(false);
    let mut interceptor = None;
    let mut fought = false;
    for _ in 0..12000 {
        for event in step(&mut app) {
            fought |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. }
                if ground.contains(&attacker) && Some(target) == interceptor);
        }
        // Introduce an observed tank after the clearance job really issued its AA
        // focus, so idle fire or incidental splash cannot satisfy the regression.
        if interceptor.is_none() && app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::Attack {units,target} if *target == aa && units.iter().any(|id|ground.contains(id)))) {
            let entity = unit_entity(&mut app, ground[0]);
            let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
            let cell = CellPos::new(cell.x + 3, cell.y + 3);
            interceptor = Some(spawn_unit(app.world_mut(),PlayerId(1),"rhino",cell).unwrap());
        }
        if fought {
            break;
        }
    }
    eprintln!("controlled interceptor installed {interceptor:?}; fought {fought}");
    if !fought {
        dump_screen(&mut app);
    }
    assert!(
        interceptor.is_some() && fought,
        "AA clearance bypassed the observed hostile tank instead of fighting along its approach"
    );
}

fn dump_screen(app: &mut App) {
    let world = app.world_mut();
    eprintln!(
        "controlled screen tick{:?} {:?}",
        world.resource::<sim::SimTick>(),
        world.resource::<sim::AiPlayers>()
    );
    for (id, owner, unit, mobile, hp, weapon, orders) in world
        .query::<(
            &sim::SimId,
            &sim::Owner,
            &sim::Unit,
            &sim::Mobile,
            &sim::Health,
            Option<&sim::Weapon>,
            &sim::Orders,
        )>()
        .iter(world)
    {
        eprintln!("controlled {id:?} {owner:?} {unit:?} {mobile:?} {hp:?} {weapon:?} {orders:?}");
    }
}

#[test]
fn ra8_an_unreachable_ground_screen_does_not_hold_strike_aircraft_forever() {
    let (mut app, ground, air, aa) = distant_screen(true);
    let mut released = false;
    let mut clearance_ordered = false;
    for _ in 0..12000 {
        step(&mut app);
        clearance_ordered |=
            app.world()
                .resource::<CommandQueue>()
                .iter()
                .any(|issued| match &issued.command {
                    Command::Attack { units, target } => {
                        *target == aa && units.iter().any(|id| ground.contains(id))
                    }
                    Command::AttackMove { units, target } => {
                        *target == CellPos::new(38, 14)
                            && units.iter().any(|id| ground.contains(id))
                    }
                    _ => false,
                });
        released |= ordinary_air_raid_queued(&app, &air);
        if released {
            break;
        }
    }
    assert!(
        clearance_ordered && released,
        "aircraft stayed held behind ground units separated from the screen by impassable terrain"
    );
}

#[test]
fn ra8_unlimited_ammo_bombers_resume_when_ground_cannot_reach_the_screen() {
    let (mut app, ground, air, aa) = distant_screen_aircraft(true, false, "kirov");
    let mut released = false;
    let mut clearance_ordered = false;
    for _ in 0..12000 {
        step(&mut app);
        clearance_ordered |=
            app.world()
                .resource::<CommandQueue>()
                .iter()
                .any(|issued| match &issued.command {
                    Command::Attack { units, target } => {
                        *target == aa && units.iter().any(|id| ground.contains(id))
                    }
                    Command::AttackMove { units, target } => {
                        *target == CellPos::new(38, 14)
                            && units.iter().any(|id| ground.contains(id))
                    }
                    _ => false,
                });
        released |= ordinary_air_raid_queued(&app, &air);
        if released {
            break;
        }
    }
    assert!(
        clearance_ordered && released,
        "aircraft stayed held behind ground units separated from the screen by impassable terrain"
    );
}

#[test]
fn ra8_an_existing_aa_clearance_attack_keeps_its_chase_history() {
    let (mut app, ground, _, aa) = distant_screen(false);
    let aa_entity = unit_entity(&mut app, aa);
    let footprint = app
        .world()
        .get::<sim::Building>(aa_entity)
        .unwrap()
        .footprint;
    let defs = app.world().resource::<sim::Definitions>().clone();
    let mut tracked = None;
    for _ in 0..12000 {
        step(&mut app);
        let tick = app.world().resource::<SimTick>().0;
        if let Some((entity, since)) = tracked {
            if tick >= since + 40 {
                let weapon = app.world().get::<sim::Weapon>(entity).unwrap();
                assert!(
                    matches!(weapon.order, Some(sim::AttackOrder::Attack { target, chases }) if target == aa && chases >= 2),
                    "a matching healthy clearance attack lost its bounded chase history: {weapon:?}"
                );
                assert!(
                    app.world()
                        .get::<sim::Orders>(entity)
                        .unwrap()
                        .current
                        .is_some()
                );
                return;
            }
        } else {
            for id in &ground {
                let entity = unit_entity(&mut app, *id);
                let mobile = *app.world().get::<sim::Mobile>(entity).unwrap();
                let weapon = *app.world().get::<sim::Weapon>(entity).unwrap();
                let range = defs
                    .weapon(weapon.kind)
                    .unwrap()
                    .range
                    .div_ceil(sim::SUBCELLS_PER_CELL as u32);
                if matches!(weapon.order, Some(sim::AttackOrder::Attack { target, .. }) if target == aa)
                    && app
                        .world()
                        .get::<sim::Orders>(entity)
                        .unwrap()
                        .current
                        .is_some()
                    && footprint.distance_to_cell(mobile.cell) > range + 2
                {
                    // Preserve a valid prior chase count on an actual scheduled Attack.
                    // Forty ticks is two controller turns, short of its firing position.
                    app.world_mut()
                        .get_mut::<sim::Weapon>(entity)
                        .unwrap()
                        .order = Some(sim::AttackOrder::Attack {
                        target: aa,
                        chases: 2,
                    });
                    tracked = Some((entity, tick));
                    break;
                }
            }
        }
    }
    panic!("no healthy distant AA clearance attack was established");
}

#[test]
fn ra8_reinforcements_without_aa_job_progress_cannot_renew_the_aircraft_hold() {
    let (mut app, mut ground, air, aa) = distant_screen(true);
    let mut installed = false;
    let mut replacements = 0;
    let mut released = false;
    let mut installed_at = None;
    let replacement_interval = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .difficulty()
        .think_interval
        * 2;
    for elapsed in 0..12000 {
        step(&mut app);
        installed |= app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::Attack { units, target } if *target == aa && units.iter().any(|id| ground.contains(id)))
            || matches!(&issued.command, Command::AttackMove { units, target } if *target == CellPos::new(38,14) && units.iter().any(|id| ground.contains(id))));
        if installed && installed_at.is_none() {
            installed_at = Some(app.world().resource::<SimTick>().0);
            let c = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap();
            eprintln!(
                "AA lease installed{installed_at:?} patience{} {}",
                c.difficulty().wave_patience,
                format!("{c:?}").split_once("air_screens:").unwrap().1
            );
        }
        if installed {
            // Declared external orders keep every surviving recruit active on our side
            // of the wall. This isolates job progress from the separate all-idle fallback.
            issue(
                &mut app,
                Command::Move {
                    units: ground.clone(),
                    target: CellPos::new(2, 40),
                },
            );
            if elapsed % replacement_interval == replacement_interval - 1 {
                // Fresh, farther participants are real spawned units; the controller's
                // normal assembly/reinforcement cadence determines when they join.
                let recruit = (0..225)
                    .find_map(|index| {
                        let cell = CellPos::new(3 + index % 15, 27 + index / 15);
                        spawn_unit(app.world_mut(), PlayerId(0), "grizzly", cell).ok()
                    })
                    .expect("fixture has a free cell for a fresh recruit");
                ground.push(recruit);
                replacements += 1;
            }
        }
        released |= ordinary_air_raid_queued(&app, &air);
        if released {
            let c = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap();
            eprintln!(
                "AA lease released tick{} {}",
                app.world().resource::<SimTick>().0,
                format!("{c:?}").split_once("air_screens:").unwrap().1
            );
            break;
        }
    }
    let entity = unit_entity(&mut app, aa);
    let hp = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    eprintln!(
        "reinforced AA job installed={installed}, replacement batches={replacements}, released={released}, AA HP={hp}"
    );
    assert!(
        installed && replacements >= 20 && released && hp == 900,
        "fresh recruits renewed an unchanged failed AA clearance hold"
    );
}

fn slow_screen_ground(app: &mut App) {
    // A declared units-file fixture makes a healthy approach exceed Medium's
    // initial patience. Special advance correctly reapplies data speed each tick.
    let source = std::fs::read_to_string(data().join("units.ron")).unwrap();
    let text = source
        .replace("id: \"grizzly\", speed: 13", "id: \"grizzly\", speed: 1")
        .replace(
            "id: \"prism_tank\", speed: 8",
            "id: \"prism_tank\", speed: 1",
        );
    assert_ne!(source, text);
    let dir = PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap()).join("tmp/ra8_definitions");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slow-aa-units.ron");
    std::fs::write(&path, text).unwrap();
    let definitions =
        sim::load_match_definitions(&path, data().join("economy.ron"), data().join("combat.ron"))
            .unwrap()
            .with_rules(data().join("rules.ron"))
            .unwrap()
            .with_arsenal(data().join("arsenal.ron"))
            .unwrap();
    app.world_mut().insert_resource(definitions);
}

#[test]
fn ra8_a_slow_advancing_aa_team_keeps_its_aircraft_hold_beyond_initial_patience() {
    let (mut app, ground, air, aa) = distant_screen(false);
    slow_screen_ground(&mut app);
    let initial: Vec<_> = ground
        .iter()
        .map(|id| {
            let entity = unit_entity(&mut app, *id);
            app.world().get::<sim::Mobile>(entity).unwrap().cell
        })
        .collect();
    let mut installed = false;
    for _ in 0..2800 {
        step(&mut app);
        installed |= app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::AttackMove { units, target } if *target == CellPos::new(38,14) && units.iter().any(|id| ground.contains(id)))
            || matches!(&issued.command, Command::Attack { units, target } if *target == aa && units.iter().any(|id| ground.contains(id))));
        let held = !ordinary_air_raid_queued(&app, &air);
        if !held {
            eprintln!("slow probe installed={installed}");
            dump_screen(&mut app);
        }
        assert!(held, "healthy slow ground progress lost its aircraft hold");
    }
    let advanced = ground.iter().zip(initial).all(|(id, old)| {
        let entity = unit_entity(&mut app, *id);
        app.world().get::<sim::Mobile>(entity).unwrap().cell.x > old.x
    });
    let entity = unit_entity(&mut app, aa);
    assert!(
        installed && advanced && app.world().get::<sim::Health>(entity).unwrap().hit_points == 900,
        "probe must observe actual slower-than-patience approach before AA damage"
    );
}

#[test]
fn ra8_a_reserved_aa_clearance_wave_still_receives_legal_chronosphere_support() {
    let (mut app, ground, _, aa) = distant_screen_options(false, true);
    let source = put(&mut app, "chronosphere", 2, 18);
    step(&mut app); // Real source announcement initializes its charge.
    let mut installed = false;
    for _ in 0..100 {
        step(&mut app);
        installed |= app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::Attack { units, target } if *target == aa && units.iter().any(|id| ground.contains(id)))
            || matches!(&issued.command, Command::AttackMove { units, target } if *target == CellPos::new(38,14) && units.iter().any(|id| ground.contains(id))));
        if installed {
            break;
        }
    }
    assert!(
        installed,
        "probe must establish the actual AA job before readiness"
    );
    let defs = app.world().resource::<sim::Definitions>();
    let cooldown = defs
        .metadata(sim::ProductionItem::Building(
            defs.building_kind("chronosphere").unwrap(),
        ))
        .unwrap()
        .cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(source, cooldown);
    let mut explicit = false;
    let mut activated = false;
    for _ in 0..100 {
        let events = step(&mut app);
        explicit |= app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(issued.command, Command::Chronoshift { building, .. } if building == source));
        activated |= events.iter().any(|event|
            matches!(event, sim::SimEvent::StrategicActivated { building, .. } if *building == source));
        if activated {
            break;
        }
    }
    if !activated {
        dump_screen(&mut app);
    }
    assert!(
        explicit && activated,
        "movement reservation hid the actual clearance tanks from a ready legal Chronosphere"
    );
    assert!(
        app.world().resource::<sim::SpecialState>().charge[&source] < cooldown,
        "activation must consume ordinary source readiness"
    );
}

#[test]
fn ra8_strike_aircraft_exploit_a_short_observed_ground_clearance_opening() {
    let (mut app, ground, air, aa) = distant_screen(false);
    slow_screen_ground(&mut app);
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(38, 16),
    )
    .unwrap();
    let oil = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "oil_derrick",
        CellPos::new(42, 8),
    )
    .unwrap();
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "engineer",
        CellPos::new(41, 8),
    )
    .unwrap();
    step(&mut app); // Actual Engineer sight before the ordinary capture command.
    let replacement = sim::ProductionItem::Building(
        app.world()
            .resource::<sim::Definitions>()
            .building_kind("flak_cannon")
            .unwrap(),
    );
    for command in [
        Command::Interact {
            unit: engineer,
            target: oil,
        },
        Command::QueueProduction { item: replacement },
    ] {
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(sim::IssuedCommand {
                player: PlayerId(1),
                command,
            });
    }
    let mut paid_ready = false;
    let mut killed = None;
    let mut struck = false;
    let mut rebuilt = false;
    for elapsed in 0..12000 {
        let events = step(&mut app);
        for event in events {
            paid_ready |= matches!(event, sim::SimEvent::ProductionComplete { player, item } if player == PlayerId(1) && item == replacement);
            if matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == aa)
            {
                killed = Some(elapsed);
            }
            if matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. }
                if air.contains(&attacker) && target != aa && !ground.contains(&target))
                && killed.is_some()
                && !rebuilt
            {
                // Require the actual known economic objective, not an incidental unit.
                let world = app.world_mut();
                if let sim::SimEvent::WeaponFired { target, .. } = event {
                    struck |= world
                        .query::<(&sim::SimId, &sim::Building)>()
                        .iter(world)
                        .any(|(id, b)| *id == target && b.footprint.origin == CellPos::new(34, 12));
                }
            }
        }
        if let Some(at) = killed
            && elapsed >= at + 60
        {
            // The defender captured a real neutral derrick and fully paid this
            // replacement in advance. Only its normal placement is withheld sixty ticks.
            let ready = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(1))
                .unwrap()
                .queues()
                .flat_map(|(_, q)| q)
                .find(|head| head.item == replacement)
                .unwrap();
            let defs = app.world().resource::<sim::Definitions>();
            let production = defs.production(replacement).unwrap();
            assert!(
                paid_ready
                    && ready.charged == production.cost
                    && ready.progress == production.build_time
            );
            eprintln!(
                "ready replacement charged{} progress{} source_cost{} source_time{}",
                ready.charged, ready.progress, production.cost, production.build_time
            );
            let sim::ProductionItem::Building(building) = replacement else {
                unreachable!()
            };
            app.world_mut()
                .resource_mut::<CommandQueue>()
                .push(sim::IssuedCommand {
                    player: PlayerId(1),
                    command: Command::PlaceBuilding {
                        building,
                        origin: CellPos::new(38, 14),
                    },
                });
            rebuilt = step(&mut app).iter().any(|event| matches!(event, sim::SimEvent::BuildingPlaced { player, kind, .. } if *player == PlayerId(1) && *kind == building));
            break;
        }
    }
    eprintln!("short AA opening kill={killed:?}, rebuilt={rebuilt}, economic strike={struck}");
    if !struck {
        dump_screen(&mut app);
    }
    assert!(
        killed.is_some() && rebuilt && struck,
        "strike aircraft missed the observed clearance window while returning to distant home"
    );
}

fn ordinary_air_raid_queued(app: &App, air: &[sim::SimId]) -> bool {
    app.world()
        .resource::<CommandQueue>()
        .iter()
        .any(|issued| match &issued.command {
            Command::Attack { units, .. } | Command::AttackMove { units, .. } => {
                units.iter().any(|id| air.contains(id))
            }
            _ => false,
        })
}

fn forward_wait_fog_fixture(aircraft: &str) -> (App, Vec<sim::SimId>) {
    let (mut app, _, air, _) = distant_screen_aircraft(false, false, aircraft);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(34, 14),
    )
    .unwrap(); // Stationary source sight keeps the actual AA job observed.
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .building_kind("spy_satellite")
        .unwrap();
    let world = app.world_mut();
    let satellite = *world
        .query::<(&sim::SimId, &sim::Building)>()
        .iter(world)
        .find(|(_, b)| b.kind == kind)
        .unwrap()
        .0;
    let entity = unit_entity(&mut app, satellite);
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = 0;
    let mut rules = app.world().resource::<sim::MatchState>().rules();
    rules.fog_of_war = true;
    app.world_mut()
        .resource_mut::<sim::MatchState>()
        .set_rules(rules);
    step(&mut app); // Lose satellite sight with controllers absent; explored memory remains.
    app.world_mut().insert_resource(controllers);
    (app, air)
}

fn first_forward_wait(app: &mut App, air: &[sim::SimId]) -> CellPos {
    for _ in 0..100 {
        step(app);
        if let Some(target) = app
            .world()
            .resource::<CommandQueue>()
            .iter()
            .find_map(|issued| match &issued.command {
                Command::Move { units, target } if units.iter().any(|id| air.contains(id)) => {
                    Some(*target)
                }
                _ => None,
            })
        {
            return target;
        }
    }
    dump_screen(app);
    panic!("fixture must establish an actual forward aircraft wait");
}

fn forward_wait_unknown_twin(building: bool, aircraft: &str) {
    let (mut probe, air) = forward_wait_fog_fixture(aircraft);
    let endpoint = first_forward_wait(&mut probe, &air);
    let (mut actual, air) = forward_wait_fog_fixture(aircraft);
    let (mut control, control_air) = forward_wait_fog_fixture(aircraft);
    let vision = actual
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .vision();
    assert!(
        vision.is_explored(endpoint) && !vision.is_visible(endpoint),
        "endpoint must be remembered terrain outside current sight: {endpoint:?}"
    );
    if building {
        sim::spawn_building(actual.world_mut(), PlayerId(1), "flak_cannon", endpoint).unwrap();
    } else {
        spawn_unit(actual.world_mut(), PlayerId(1), "kirov", endpoint).unwrap();
    }
    assert_eq!(first_forward_wait(&mut actual, &air), endpoint);
    assert_eq!(first_forward_wait(&mut control, &control_air), endpoint);
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
fn ra8_an_unseen_air_unit_cannot_change_the_forward_wait_endpoint_or_commands() {
    forward_wait_unknown_twin(false, "harrier");
}

#[test]
fn ra8_an_unseen_new_building_cannot_change_the_forward_wait_endpoint_or_commands() {
    forward_wait_unknown_twin(true, "harrier");
}

#[test]
fn ra8_unseen_air_occupancy_cannot_change_unlimited_bomber_clearance_waits() {
    forward_wait_unknown_twin(false, "kirov");
}

#[test]
fn ra8_unseen_building_occupancy_cannot_change_unlimited_bomber_clearance_waits() {
    forward_wait_unknown_twin(true, "kirov");
}

fn paid_then_unfunded_service_fixture() -> (App, sim::SimId, i32) {
    let defs = load_ruleset(data()).unwrap();
    let kind = defs.unit_kind("grizzly").unwrap();
    let maximum = defs.unit_combat(kind).unwrap().hit_points;
    let rules = defs.rules().unwrap().economy;
    let cost = sim::repair_step_cost(
        &rules,
        defs.production(sim::ProductionItem::Unit(kind))
            .unwrap()
            .cost,
        maximum,
        false,
    );
    let mut app = scenario("germany", "medium", cost, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 10, 2);
    let depot = put(&mut app, "service_depot", 14, 8);
    let tank = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "grizzly",
        CellPos::new(13, 10),
    )
    .unwrap();
    let entity = unit_entity(&mut app, tank);
    let damaged = maximum - rules.repair_step * 2;
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .unwrap()
        .hit_points = damaged;
    step(&mut app);
    issue(
        &mut app,
        Command::Interact {
            unit: tank,
            target: depot,
        },
    );
    for _ in 0..200 {
        step(&mut app);
        if app.world().get::<sim::Health>(entity).unwrap().hit_points > damaged {
            break;
        }
    }
    let repaired = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    assert!(
        repaired > damaged && repaired < maximum,
        "normal paid repair must precede the funding loss"
    );
    assert_eq!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .docked
            .get(&tank),
        Some(&depot)
    );
    app.world_mut().insert_resource(controllers);
    (app, tank, repaired)
}

#[test]
fn ra8_an_unfunded_service_occupant_leaves_normally_and_rejoins_real_combat() {
    let (mut app, tank, repaired) = paid_then_unfunded_service_fixture();
    let mut ordered = false;
    let mut released = false;
    for _ in 0..80 {
        let events = step(&mut app);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, sim::SimEvent::BuildingSold { .. })),
            "a recoverable combat unit must precede base liquidation"
        );
        ordered |= app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::Move { units, .. } if units.contains(&tank)));
        released |= !app
            .world()
            .resource::<sim::SpecialState>()
            .docked
            .contains_key(&tank);
        if ordered && released {
            break;
        }
    }
    assert!(
        ordered && released,
        "a usable tank stayed at a depot with no repair funds or income"
    );
    let entity = unit_entity(&mut app, tank);
    assert_eq!(
        app.world().get::<sim::Health>(entity).unwrap().hit_points,
        repaired
    );
    let mut fought = false;
    for _ in 0..9000 {
        let events = step(&mut app);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, sim::SimEvent::BuildingSold { .. }))
        );
        assert!(
            !app.world()
                .resource::<sim::SpecialState>()
                .docked
                .contains_key(&tank),
            "the tank re-entered its unaffordable repair job"
        );
        fought |= events.iter().any(|event| matches!(event,sim::SimEvent::WeaponFired { attacker, .. } if *attacker == tank));
        if fought {
            break;
        }
    }
    assert!(
        fought,
        "released tank must make an actual normal combat contribution"
    );
}

#[test]
fn ra8_a_service_cash_gap_with_owned_income_keeps_the_repair_job() {
    let (mut app, tank, repaired) = paid_then_unfunded_service_fixture();
    // An actual owned income building will pay on its ordinary source timer;
    // the current zero-credit interval is not permanent economic stranding.
    put(&mut app, "oil_derrick", 2, 18);
    for _ in 0..80 {
        step(&mut app);
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .docked
                .contains_key(&tank)
        );
        assert!(!app.world().resource::<CommandQueue>().iter().any(
            |issued| matches!(&issued.command, Command::Move { units, .. } if units.contains(&tank))
        ));
    }
    let entity = unit_entity(&mut app, tank);
    assert!(app.world().get::<sim::Health>(entity).unwrap().hit_points >= repaired);
}

#[test]
fn ra8_a_decided_service_release_cancels_only_its_later_actor_loss() {
    for public in [false, true] {
        let (mut app, tank, _) = paid_then_unfunded_service_fixture();
        let mut release = None;
        for _ in 0..80 {
            step(&mut app);
            release = app.world().resource::<CommandQueue>().iter().find(|issued|
                matches!(&issued.command, Command::Move { units, .. } if units.contains(&tank))).cloned();
            if release.is_some() {
                break;
            }
        }
        let release = release.expect("actual AI service release must be queued");
        if public {
            *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
            app.world_mut().resource_mut::<CommandQueue>().push(release);
        }
        let entity = unit_entity(&mut app, tank);
        app.world_mut().get_mut::<sim::Owner>(entity).unwrap().0 = PlayerId(1);
        app.world_mut().run_schedule(SimSchedule);
        let rejected = app
            .world()
            .resource::<bevy_ecs::message::Messages<sim::SimEvent>>()
            .iter_current_update_messages()
            .any(|event| matches!(event, sim::SimEvent::CommandRejected { .. }));
        assert_eq!(
            rejected, public,
            "public ownership validation remains strict"
        );
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
}

fn chrono_boundary_fixture(reveal: bool) -> (App, sim::SimId, Vec<sim::SimId>) {
    let mut app = scenario("great_britain", "brutal", 0, 0, true);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let timing = controllers.controller(PlayerId(0)).unwrap().difficulty();
    let initial_assembly = timing.first_wave + timing.wave_patience + timing.think_interval;
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
    let source = put(&mut app, "chronosphere", 2, 5);
    if reveal {
        put(&mut app, "spy_satellite", 2, 12);
    } else {
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "rocketeer",
            CellPos::new(43, 36),
        )
        .unwrap();
    }
    let tanks = [(14, 14), (15, 14), (14, 15), (15, 15)]
        .map(|(x, y)| {
            spawn_unit(app.world_mut(), PlayerId(0), "grizzly", CellPos::new(x, y)).unwrap()
        })
        .to_vec();
    for _ in 0..initial_assembly {
        step(&mut app);
    }
    let defs = app.world().resource::<sim::Definitions>();
    let cooldown = defs
        .metadata(sim::ProductionItem::Building(
            defs.building_kind("chronosphere").unwrap(),
        ))
        .unwrap()
        .cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(source, cooldown);
    app.world_mut().insert_resource(controllers);
    (app, source, tanks)
}

fn queued_chronoshift(app: &mut App, source: sim::SimId) -> (CellPos, CellPos) {
    for _ in 0..40 {
        step(app);
        if let Some((from, to)) = app
            .world()
            .resource::<CommandQueue>()
            .iter()
            .find_map(|issued| match issued.command {
                Command::Chronoshift {
                    building,
                    source: from,
                    target,
                } if building == source => Some((from, target)),
                _ => None,
            })
        {
            return (from, to);
        }
    }
    panic!("ready legal source must support the actual wave");
}

#[test]
fn ra8_chronoshift_freeze_preserves_the_wave_and_resumes_offensive_combat() {
    let (mut app, source, tanks) = chrono_boundary_fixture(true);
    let (from, _) = queued_chronoshift(&mut app, source);
    let selected: Vec<_> = tanks
        .into_iter()
        .filter(|id| {
            let entity = unit_entity(&mut app, *id);
            let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
            cell.x.abs_diff(from.x) <= 1 && cell.y.abs_diff(from.y) <= 1
        })
        .collect();
    assert!(!selected.is_empty());
    let events = step(&mut app);
    assert!(events.iter().any(
        |event| matches!(event,sim::SimEvent::StrategicActivated {building,..} if *building==source)
    ));
    let mut saw_freeze = false;
    let mut resumed = false;
    let mut fired = false;
    for _ in 0..800 {
        let events = step(&mut app);
        let frozen = &app.world().resource::<sim::SpecialState>().disabled;
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        for id in &selected {
            assert!(
                controller.wave().contains(id),
                "temporary source freeze erased the offensive wave"
            );
            if frozen.contains_key(id) {
                saw_freeze = true;
                assert!(
                    !app.world().resource::<CommandQueue>().iter().any(|issued| {
                        match &issued.command {
                            Command::Move { units, .. }
                            | Command::AttackMove { units, .. }
                            | Command::Attack { units, .. } => units.contains(id),
                            Command::Interact { unit, .. } | Command::Deploy { unit } => unit == id,
                            _ => false,
                        }
                    }),
                    "disabled actor received an army command"
                );
            } else {
                resumed = true;
            }
        }
        assert!(!app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(&issued.command, Command::Move{units,target}|Command::AttackMove{units,target}
                if *target==controller.gather() && units.iter().any(|id|selected.contains(id)))), "teleported attack wave was sent home");
        fired |= events.iter().any(|event| matches!(event,sim::SimEvent::WeaponFired {attacker,..} if selected.contains(attacker)));
        if saw_freeze && resumed && fired {
            break;
        }
    }
    assert!(saw_freeze && resumed && fired);
}

#[test]
fn ra8_an_all_frozen_casualty_wave_defers_its_nonempty_retreat_until_release() {
    let (mut app, source, mut tanks) = chrono_boundary_fixture(true);
    let wanted = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .difficulty()
        .wave_size as usize;
    let additional = wanted.checked_sub(tanks.len()).unwrap();
    for i in 0..additional {
        tanks.push(
            spawn_unit(
                app.world_mut(),
                PlayerId(0),
                "grizzly",
                CellPos::new(10 + (i % 7) as i32, 19 + (i / 7) as i32),
            )
            .unwrap(),
        );
    }
    queued_chronoshift(&mut app, source);
    assert_eq!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .len(),
        wanted,
        "real full initial cohort must be non-exempt"
    );
    step(&mut app);
    let selected: Vec<_> = tanks
        .iter()
        .copied()
        .filter(|id| {
            app.world()
                .resource::<sim::SpecialState>()
                .disabled
                .contains_key(id)
        })
        .collect();
    assert!(!selected.is_empty() && selected.len() * 2 < wanted);
    // Real cleanup/casualty processing runs after this isolated damage setup. All
    // survivors are the actual Chronoshift arrivals, still in their source freeze.
    for id in tanks.iter().filter(|id| !selected.contains(id)) {
        let entity = unit_entity(&mut app, *id);
        app.world_mut()
            .get_mut::<sim::Health>(entity)
            .unwrap()
            .hit_points = 0;
    }
    let mut retreated = false;
    for _ in 0..200 {
        step(&mut app); // Also asserts zero rejected commands and physical invariants.
        let frozen = &app.world().resource::<sim::SpecialState>().disabled;
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        if selected.iter().any(|id| frozen.contains_key(id)) {
            assert!(
                selected.iter().all(|id| controller.wave().contains(id)),
                "an all-frozen casualty wave forgot its deferred retreat"
            );
            assert!(!app.world().resource::<CommandQueue>().iter().any(|issued|
                matches!(&issued.command,Command::Move {units,..}|Command::AttackMove {units,..}|Command::Attack {units,..}
                    if units.iter().any(|id|selected.contains(id)))),"retreat commanded a disabled member");
        }
        retreated |= app.world().resource::<CommandQueue>().iter().any(|issued| {
            matches!(&issued.command,Command::AttackMove {units,target} if !units.is_empty()
                && *target==controller.gather() && units.iter().any(|id|selected.contains(id))
                && units.iter().all(|id|!frozen.contains_key(id)))
        });
        if retreated {
            break;
        }
    }
    assert!(
        retreated,
        "the actual casualty retreat must resume after release"
    );
}

#[test]
fn ra8_an_already_erased_strategic_source_is_not_queued_by_the_controller() {
    let mut app = scenario("germany", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "spy_satellite", 2, 12);
    let source = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "airport",
        CellPos::new(14, 25),
    )
    .unwrap();
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(13, 25),
    )
    .unwrap();
    step(&mut app);
    issue(
        &mut app,
        Command::Interact {
            unit: engineer,
            target: source,
        },
    );
    let source_entity = unit_entity(&mut app, source);
    for _ in 0..100 {
        step(&mut app);
        if app.world().get::<sim::Owner>(source_entity).unwrap().0 == PlayerId(0) {
            break;
        }
    }
    assert_eq!(
        app.world().get::<sim::Owner>(source_entity).unwrap().0,
        PlayerId(0)
    );
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "chrono_legionnaire",
        CellPos::new(14, 24),
    )
    .unwrap();
    step(&mut app); // Actual owner sight before the ordinary targeted attack.
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Attack {
                units: vec![enemy],
                target: source,
            },
        });
    for _ in 0..100 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&source)
        {
            break;
        }
    }
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&source),
        "actual Temporal beam must precede the AI decision"
    );
    let defs = app.world().resource::<sim::Definitions>();
    let meta = defs
        .metadata(sim::ProductionItem::Building(
            defs.building_kind("airport").unwrap(),
        ))
        .unwrap();
    assert!(!meta.is_powered);
    let cooldown = meta.cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(source, cooldown);
    app.world_mut().insert_resource(controllers);
    for _ in 0..40 {
        step(&mut app);
        assert!(!app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(issued.command,Command::Chronoshift {building,..}|Command::Superweapon {building,..} if building==source)),"intrinsically unavailable source was queued");
    }
}

#[test]
fn ra8_chronoshift_frozen_membership_still_discards_death_and_ownership_loss() {
    for captured in [false, true] {
        let (mut app, source, tanks) = chrono_boundary_fixture(true);
        let (from, _) = queued_chronoshift(&mut app, source);
        let selected = tanks
            .into_iter()
            .find(|id| {
                let entity = unit_entity(&mut app, *id);
                let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
                cell.x.abs_diff(from.x) <= 1 && cell.y.abs_diff(from.y) <= 1
            })
            .unwrap();
        step(&mut app);
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .disabled
                .contains_key(&selected)
        );
        let entity = unit_entity(&mut app, selected);
        if captured {
            app.world_mut().get_mut::<sim::Owner>(entity).unwrap().0 = PlayerId(1);
        } else {
            app.world_mut()
                .get_mut::<sim::Health>(entity)
                .unwrap()
                .hit_points = 0;
        }
        for _ in 0..30 {
            step(&mut app);
        }
        assert!(
            !app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&selected)
        );
    }
}

#[test]
fn ra8_chronoshift_keeps_the_whole_translated_group_out_of_known_ground_fire() {
    let (mut app, source, tanks) = chrono_boundary_fixture(true);
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "power_plant",
        CellPos::new(22, 34),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "war_factory",
        CellPos::new(28, 30),
    )
    .unwrap();
    sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "prism_tower",
        CellPos::new(26, 36),
    )
    .unwrap();
    let (from, to) = queued_chronoshift(&mut app, source);
    eprintln!("known ground-fire probe from{from:?} to{to:?}");
    let defs = app.world().resource::<sim::Definitions>();
    let terms = defs
        .building_combat(defs.building_kind("prism_tower").unwrap())
        .unwrap();
    let range = defs.weapon(terms.weapon.unwrap()).unwrap().range;
    let mut moved = 0;
    for id in tanks {
        let entity = unit_entity(&mut app, id);
        let cell = app.world().get::<sim::Mobile>(entity).unwrap().cell;
        if cell.x.abs_diff(from.x) > 1 || cell.y.abs_diff(from.y) > 1 {
            continue;
        }
        moved += 1;
        let dest = CellPos::new(to.x + cell.x - from.x, to.y + cell.y - from.y);
        let point = sim::SubPos::centre_of(dest);
        let dx = (i64::from(point.x) - 27 * 256 + 1)
            .max(26 * 256 - i64::from(point.x))
            .max(0);
        let dy = (i64::from(point.y) - 37 * 256 + 1)
            .max(36 * 256 - i64::from(point.y))
            .max(0);
        assert!(
            !sim::in_range(dx, dy, range),
            "known tower can fire at frozen landing {dest:?}"
        );
    }
    assert!(moved > 0);
}

#[test]
fn ra8_chronoshift_landing_commands_ignore_unseen_ground_threats() {
    for building in [false, true] {
        let (mut actual, source, _) = chrono_boundary_fixture(false);
        let (mut control, control_source, _) = chrono_boundary_fixture(false);
        assert!(
            !actual
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(35, 40))
        );
        if building {
            sim::spawn_building(
                actual.world_mut(),
                PlayerId(1),
                "prism_tower",
                CellPos::new(35, 40),
            )
            .unwrap();
        } else {
            spawn_unit(
                actual.world_mut(),
                PlayerId(1),
                "prism_tank",
                CellPos::new(35, 40),
            )
            .unwrap();
        }
        assert_eq!(
            queued_chronoshift(&mut actual, source),
            queued_chronoshift(&mut control, control_source)
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
}
