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

/// One real controller learns the hut/deck from a scout on the opposite bank. An unseen
/// hostile wall is initial scenario setup; no vision, hut memory or AI intent is seeded.
type BridgeDecisionObservation = (
    sim::Vision,
    sim::AiController,
    Vec<(sim::SimId, CellPos, sim::Owner)>,
    Vec<Command>,
);

fn ra9_privacy_bridge_decision(blocker: Option<CellPos>) -> BridgeDecisionObservation {
    let mut app = scenario_mode(
        "russia",
        "medium",
        0,
        0,
        false,
        ScenarioMode {
            fog: true,
            ..ScenarioMode::default()
        },
    );
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
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(10, 22),
    )
    .unwrap();
    let scout = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(26, 22),
    )
    .unwrap();
    let tank = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "grizzly",
        CellPos::new(20, 26),
    )
    .unwrap();
    let wall = blocker.map(|cell| {
        sim::spawn_building(app.world_mut(), PlayerId(1), "soviet_walls", cell).unwrap()
    });
    step(&mut app);
    if let Some(cell) = blocker {
        assert!(
            !app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(cell),
            "the initial wall must genuinely be outside both own units' sight"
        );
    }
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Attack {
                units: vec![tank],
                target: bridge,
            },
        });
    for _ in 0..4000 {
        step(&mut app);
        if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
            break;
        }
    }
    assert!(
        app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed,
        "actual public enemy attack must collapse the bridge"
    );
    let think = u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    while !app.world().resource::<SimTick>().0.is_multiple_of(think) {
        step(&mut app);
    }
    app.world_mut().insert_resource(controllers);
    step(&mut app); // Genuine due AI observation of hut + collapsed deck.
    assert!(
        app.world()
            .resource::<CommandQueue>()
            .iter()
            .any(|issued| issued.player == PlayerId(0)
                && matches!(issued.command,
            Command::Interact { unit, target } if unit == engineer && target == hut)),
        "current observed hut must first produce its ordinary native repair intent"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    // Let the real queued repair be admitted, then explicitly cancel it before moving the scout.
    issue(
        &mut app,
        Command::Stop {
            units: vec![engineer],
        },
    );
    issue(
        &mut app,
        Command::Move {
            units: vec![scout],
            target: CellPos::new(36, 22),
        },
    );
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Stop { units: vec![tank] },
        });
    for _ in 0..1200 {
        step(&mut app);
        let entity = unit_entity(&mut app, scout);
        let mobile = app.world().get::<sim::Mobile>(entity).unwrap();
        if mobile.cell == CellPos::new(36, 22) && mobile.step.is_none() {
            break;
        }
    }
    let scout_entity = unit_entity(&mut app, scout);
    assert_eq!(
        app.world().get::<sim::Mobile>(scout_entity).unwrap().cell,
        CellPos::new(36, 22)
    );
    let vision = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .vision();
    assert!(
        vision.is_explored(CellPos::new(21, 22)) && !vision.is_visible(CellPos::new(21, 22)),
        "hut is genuinely remembered through optional fog"
    );
    if let (Some(wall), Some(cell)) = (wall, blocker) {
        assert!(!vision.is_visible(cell));
        assert!(
            !controllers
                .controller(PlayerId(0))
                .unwrap()
                .known_buildings()
                .contains_key(&wall),
            "unseen wall must never enter controller memory"
        );
    }
    while !app.world().resource::<SimTick>().0.is_multiple_of(think) {
        step(&mut app);
    }
    let vision = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .vision()
        .clone();
    let knowledge = controllers.controller(PlayerId(0)).unwrap().clone();
    let own = {
        let world = app.world_mut();
        let mut own: Vec<_> = world
            .query::<(&sim::SimId, &sim::Mobile, &sim::Owner)>()
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == PlayerId(0))
            .map(|(id, mobile, owner)| (*id, mobile.cell, *owner))
            .collect();
        own.sort_by_key(|(id, ..)| *id);
        own
    };
    app.world_mut().insert_resource(controllers);
    step(&mut app); // Capture commands before their next Command admission/pathfinding.
    let decisions: Vec<_> = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .filter(|issued| issued.player == PlayerId(0))
        .filter_map(|issued| match &issued.command {
            Command::Move { units, .. } if units.contains(&engineer) => {
                Some(issued.command.clone())
            }
            _ => None,
        })
        .collect();
    assert!(
        matches!(decisions.as_slice(), [Command::Move { .. }]),
        "remembered bridge job must emit an actual queued Engineer approach: {decisions:?}"
    );
    (vision, knowledge, own, decisions)
}

#[test]
fn ra9_privacy_remembered_bridge_approach_ignores_unseen_endpoint_and_corridor_buildings() {
    let empty = ra9_privacy_bridge_decision(None);
    let blocked: Vec<_> = [CellPos::new(20, 22), CellPos::new(17, 22)]
        .into_iter()
        .map(|cell| (cell, ra9_privacy_bridge_decision(Some(cell))))
        .collect();
    eprintln!("native bridge empty-world observation: {empty:?}");
    for (cell, observation) in &blocked {
        eprintln!("native bridge blocker-world observation at {cell:?}: {observation:?}");
    }
    for (cell, blocked) in blocked {
        assert_eq!(
            blocked.0, empty.0,
            "same actual observer visibility/exploration"
        );
        assert_eq!(
            blocked.1, empty.1,
            "same actual controller memory/state before decision"
        );
        assert_eq!(blocked.2, empty.2, "same own native actors and positions");
        assert_eq!(
            blocked.3, empty.3,
            "unseen building at {cell:?} must not select the AI's endpoint or traversal"
        );
    }
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
    legacy_ai: bool,
    fog: bool,
    authored_terrain: Option<fn(&mut Grid)>,
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
    if mode.legacy_ai {
        ai.rules_difficulty = false;
    }
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
    if let Some(author) = mode.authored_terrain {
        author(&mut app.world_mut().resource_mut::<Grid>());
    }
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
            fog_of_war: mode.fog,
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
fn ra8_opted_in_last_stand_services_a_native_remnant_within_one_existing_period() {
    let (mut app, airfield, _) = native_last_stand_remnant(928);
    let start = app.world().resource::<SimTick>().0;
    let think = u64::from(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let bound = sim::LAST_STAND_PATIENCE + sim::LAST_STAND_TURN + think;
    let mut sold = None;
    let mut shots = 0;
    for _ in 0..=bound {
        for event in step(&mut app) {
            shots += usize::from(
                matches!(event, sim::SimEvent::WeaponFired { target, .. } if target == airfield),
            );
            if matches!(event, sim::SimEvent::BuildingSold { player: PlayerId(0), building, .. } if building == airfield)
            {
                sold = Some(app.world().resource::<SimTick>().0);
            }
        }
        if sold.is_some() {
            break;
        }
    }
    eprintln!(
        "native remnant start{start} bound{bound} shots{shots} sold{sold:?} outcome{:?}",
        app.world().resource::<sim::MatchState>().outcome()
    );
    assert!(
        shots > 0,
        "the real opponent must continue native source fire"
    );
    assert!(
        sold.is_some_and(|at| at.saturating_sub(start) >= sim::LAST_STAND_PATIENCE
            && at.saturating_sub(start) <= bound),
        "after the unchanged patience, an opted-in remnant must receive a unique ordinary sale opportunity within one existing period"
    );
    assert!(matches!(
        app.world().resource::<sim::MatchState>().outcome(),
        Some(sim::MatchOutcome::Won {
            winner: PlayerId(1),
            ..
        })
    ));
}

#[test]
fn ra8_opted_in_last_stand_keeps_live_force_working_income_and_funded_production() {
    for mode in ["force", "ore", "paid"] {
        let mut app = native_last_stand_remnant(if mode == "paid" { 1200 } else { 928 }).0;
        let mut actor = None;
        let harrier = app
            .world()
            .resource::<sim::Definitions>()
            .unit_kind("harrier")
            .unwrap();
        let item = sim::ProductionItem::Unit(harrier);
        if mode == "force" {
            actor = Some(
                spawn_unit(app.world_mut(), PlayerId(0), "grizzly", CellPos::new(4, 40)).unwrap(),
            );
        } else if mode == "ore" {
            put(&mut app, "refinery", 15, 3);
            let cell = CellPos::new(20, 8);
            let mut terrain = *app.world().resource::<Grid>().cell(cell).unwrap();
            terrain.ore = 5000;
            app.world_mut()
                .resource_mut::<Grid>()
                .set_terrain(cell, terrain);
            let world = app.world_mut();
            let worker = world
                .query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
                .iter(world)
                .find(|(_, owner, unit)| {
                    owner.0 == PlayerId(0)
                        && unit.kind
                            == world
                                .resource::<sim::Definitions>()
                                .unit_kind("chrono_miner")
                                .unwrap()
                })
                .map(|(id, _, _)| *id)
                .unwrap();
            actor = Some(worker);
            issue(
                &mut app,
                Command::Harvest {
                    units: vec![worker],
                    target: cell,
                },
            );
        } else {
            assert_eq!(
                app.world()
                    .resource::<sim::Definitions>()
                    .production(item)
                    .unwrap()
                    .cost,
                1200
            );
            put(&mut app, "power_plant", 10, 3);
            issue(&mut app, Command::QueueProduction { item });
        }
        let mut delivered = false;
        let mut charged = false;
        let mut completed = false;
        let mut live = false;
        for _ in 0..1600 {
            for event in step(&mut app) {
                assert!(
                    !matches!(
                        event,
                        sim::SimEvent::BuildingSold {
                            player: PlayerId(0),
                            ..
                        }
                    ),
                    "{mode}: recovery must not be liquidated"
                );
                delivered |= matches!(event, sim::SimEvent::OreDelivered { harvester, credits, .. } if Some(harvester) == actor && credits > 0);
                completed |= matches!(event, sim::SimEvent::ProductionComplete { player: PlayerId(0), item: found } if found == item);
            }
            charged |= app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .any(|(_, q)| q.iter().any(|head| head.item == item && head.charged > 0));
            let world = app.world_mut();
            live |= world
                .query::<(&sim::Owner, &sim::Unit)>()
                .iter(world)
                .any(|(owner, unit)| owner.0 == PlayerId(0) && unit.kind == harrier);
        }
        match mode {
            "force" => {
                let world = app.world_mut();
                assert!(
                    world
                        .query::<(&sim::SimId, &sim::Health)>()
                        .iter(world)
                        .any(|(id, hp)| Some(*id) == actor && hp.hit_points > 0)
                );
            }
            "ore" => assert!(
                delivered,
                "the working-income control must physically deliver ore"
            ),
            "paid" => assert!(
                charged && completed && live,
                "actual funded native work must charge, complete and spawn a live Harrier"
            ),
            _ => unreachable!(),
        }
    }
}

fn native_last_stand_remnant(credits: i64) -> (App, sim::SimId, sim::SimId) {
    let mut app = scenario("great_britain", "easy", credits, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let airfield = put(&mut app, "airforce_command", 14, 8);
    // Keep this real remnant alive through ordinary Cleanup of the old yard
    // and any native crew. No controller history, outcome or timer is seeded.
    for _ in 0..3 {
        let world = app.world_mut();
        for (id, owner, mut health) in world
            .query::<(&sim::SimId, &sim::Owner, &mut sim::Health)>()
            .iter_mut(world)
        {
            if owner.0 == PlayerId(0) && *id != airfield {
                health.hit_points = 0;
            }
        }
        step(&mut app);
    }
    let worker = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "chrono_miner",
        CellPos::new(20, 8),
    )
    .unwrap();
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "flak_track",
        CellPos::new(20, 10),
    )
    .unwrap();
    // A raw spawn is not an injected perception fact. Establish the attacker's
    // ordinary scheduled sight before submitting the public Attack command.
    step(&mut app);
    let airfield_entity = unit_entity(&mut app, airfield);
    let footprint = app
        .world()
        .get::<sim::Building>(airfield_entity)
        .unwrap()
        .footprint;
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(1))
            .unwrap()
            .vision()
            .sees_footprint(footprint)
    );
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Attack {
                units: vec![enemy],
                target: airfield,
            },
        });
    let mut fired = false;
    for _ in 0..20 {
        fired |= step(&mut app).iter().any(|e| matches!(e, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == enemy && *target == airfield));
        if fired {
            break;
        }
    }
    assert!(fired, "the fixture must establish actual legal native fire");
    let state = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap();
    assert_eq!(state.credits(), credits);
    assert!(state.low_power());
    assert!(state.queues().all(|(_, q)| q.is_empty()));
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    let definitions = app.world().resource::<sim::Definitions>();
    let harrier = definitions.unit_kind("harrier").unwrap();
    assert!(
        definitions
            .production(sim::ProductionItem::Unit(harrier))
            .unwrap()
            .cost
            > 928
    );
    app.world_mut().insert_resource(controllers);
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .plan()
            .rules_difficulty
    );
    (app, airfield, worker)
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
        ("crazy_ivan", false, true),
        ("crazy_ivan", true, false),
        ("tanya", false, false),
        ("attack_dog", false, false),
        ("conscript", true, false),
    ] {
        let mut app = scenario_mode(
            "russia",
            "easy",
            0,
            0,
            false,
            ScenarioMode {
                authored_terrain: if isolated {
                    Some(ra9_authored_control_barrier)
                } else {
                    None
                },
                ..ScenarioMode::default()
            },
        );
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
            for _ in 0..if victim_kind == "crazy_ivan" {
                1000
            } else {
                100
            } {
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
fn ra8_an_unaffordable_remaining_factory_does_not_renew_futile_paid_repairs() {
    remaining_factory_repair(false, false);
}

#[test]
fn ra8_an_affordable_remaining_factory_keeps_paid_repairs_and_real_unit_work() {
    remaining_factory_repair(true, false);
}

#[test]
fn ra8_a_financed_partly_paid_factory_head_keeps_repairs_below_its_full_price() {
    remaining_factory_repair(true, true);
}

fn remaining_factory_repair(funded: bool, partial: bool) {
    let definitions = load_ruleset(data()).unwrap();
    let tank = definitions.unit_kind("tesla_tank").unwrap();
    let radar = definitions.building_kind("radar_tower").unwrap();
    let rules = &definitions.rules().unwrap().economy;
    let maximum = definitions.building_combat(radar).unwrap().hit_points;
    let repair_budget =
        sim::repair_step_cost(
            rules,
            definitions
                .production(sim::ProductionItem::Building(radar))
                .unwrap()
                .cost,
            maximum,
            false,
        ) * i64::from(maximum.saturating_add(rules.repair_step - 1) / rules.repair_step.max(1));
    let partial_budget = definitions
        .production(sim::ProductionItem::Unit(tank))
        .unwrap()
        .cost
        + repair_budget;
    let mut app = scenario(
        "russia",
        "easy",
        if funded {
            if partial { partial_budget } else { 10000 }
        } else {
            42
        },
        0,
        false,
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    // Keep a real surviving building throughout the declared loss so MatchState
    // never defeats this controller before its remnant decision is exercised.
    let radar = put(&mut app, "radar_tower", 14, 8);
    // Resolve ordinary deaths and their crew, preserving that actual remnant.
    for _ in 0..2 {
        let world = app.world_mut();
        for (id, owner, mut hp) in world
            .query::<(&sim::SimId, &sim::Owner, &mut sim::Health)>()
            .iter_mut(world)
        {
            if owner.0 == PlayerId(0) && *id != radar {
                hp.hit_points = 0;
            }
        }
        step(&mut app);
    }
    let factory = put(&mut app, "soviet_war_factory", 8, 8);
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    {
        let world = app.world_mut();
        assert!(
            !world
                .query::<(&sim::Owner, &sim::Unit)>()
                .iter(world)
                .any(|(owner, _)| owner.0 == PlayerId(0)),
            "no hidden surviving crew may supply recovery"
        );
        let mut owned_ids = world
            .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
            .iter(world)
            .filter(|(_, owner, _)| owner.0 == PlayerId(0))
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        owned_ids.sort();
        let mut expected = vec![radar, factory];
        expected.sort();
        assert_eq!(owned_ids, expected);
        assert!(
            world
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .all(|(_, q)| q.is_empty())
        );
    }
    let entity = unit_entity(&mut app, radar);
    let definitions = app.world().resource::<sim::Definitions>().clone();
    let kind = definitions.unit_kind("rhino").unwrap();
    let item = sim::ProductionItem::Unit(kind);
    let cost = definitions.production(item).unwrap().cost;
    assert!(cost > 42);
    let owned = [
        definitions.building_kind("soviet_war_factory").unwrap(),
        definitions.building_kind("radar_tower").unwrap(),
    ];
    assert!(
        sim::check_buildable(
            &definitions,
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap(),
            false,
            &owned,
            item
        )
        .is_ok(),
        "the actual producer/prerequisite path must be legal independent of the wallet"
    );
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
    issue(&mut app, Command::RepairBuilding { building: radar });
    let mut paid = false;
    for _ in 0..40 {
        step(&mut app);
        paid |= app.world().get::<sim::Health>(entity).unwrap().hit_points > maximum - 200;
        if paid {
            break;
        }
    }
    assert!(
        paid && app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits()
            < initial,
        "real ordinary repair must first pay and heal"
    );
    spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(18, 9)).unwrap();
    // No later credit is injected; every payment/toggle is scheduled normally.
    app.world_mut().insert_resource(controllers);
    let mut stopped = false;
    let mut healed = false;
    let mut charged = false;
    let mut shot = false;
    let mut previous = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    let mut healed_partial = false;
    for _ in 0..if partial { 2000 } else { 200 } {
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        let partial_financed = state.queues().any(|(_, q)| {
            q.first().is_some_and(|head| {
                let terms = definitions.production(head.item).unwrap();
                matches!(head.item, sim::ProductionItem::Unit(_))
                    && head.charged > 0
                    && terms.cost > state.credits()
                    && terms.cost - head.charged <= state.credits()
            })
        });
        stopped |= state.repairing().contains(&radar) && app.world().resource::<CommandQueue>().iter().any(|issued| matches!(issued.command, Command::RepairBuilding { building } if building == radar));
        for event in step(&mut app) {
            assert!(
                !matches!(
                    event,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    } | sim::SimEvent::ProductionCancelled {
                        player: PlayerId(0),
                        ..
                    }
                ),
                "refund cannot fund this remnant: {event:?}"
            );
            shot |= matches!(event, sim::SimEvent::WeaponFired { target,.. } if target == radar);
        }
        let current = app.world().get::<sim::Health>(entity).unwrap().hit_points;
        healed |= current > previous;
        healed_partial |= partial_financed && current > previous;
        previous = current;
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q.iter())
            .any(|q| matches!(q.item, sim::ProductionItem::Unit(_)) && q.charged > 0);
    }
    assert!(
        shot,
        "actual enemy source fire must maintain the damaged remnant"
    );
    let factory_entity = unit_entity(&mut app, factory);
    assert!(app.world().get::<sim::Building>(factory_entity).is_some());
    if funded {
        assert!(
            !stopped && healed && charged,
            "an actually financed producer keeps repairs and real paid work"
        );
        assert!(
            !partial || healed_partial,
            "a real partly paid head must retain repairs while its remaining cost, not full price, fits the wallet"
        );
    } else {
        assert!(
            stopped,
            "a merely buildable but unaffordable army renewed futile repair spending"
        );
        assert!(!charged);
    }
}

#[test]
fn ra8_income_production_and_a_usable_force_keep_useful_building_repairs() {
    for recovery in ["oil_derrick", "construction_yard", "grizzly"] {
        let (mut app, refinery) = paid_refinery_repair_after_producer_loss();
        let mut captured_income = None;
        if recovery == "grizzly" {
            spawn_unit(app.world_mut(), PlayerId(0), recovery, CellPos::new(13, 12)).unwrap();
        } else if recovery == "oil_derrick" {
            // R164: income alone is not useful recovery for a zero-actor,
            // producerless remnant. This positive owns a real legal producer.
            put(&mut app, "power_plant", 2, 8);
            put(&mut app, "barracks", 8, 14);
            let oil = sim::spawn_building(
                app.world_mut(),
                sim::NEUTRAL,
                "oil_derrick",
                CellPos::new(2, 2),
            )
            .unwrap();
            let engineer =
                spawn_unit(app.world_mut(), PlayerId(0), "engineer", CellPos::new(5, 2)).unwrap();
            step(&mut app);
            issue(
                &mut app,
                Command::Interact {
                    unit: engineer,
                    target: oil,
                },
            );
            captured_income = Some((oil, engineer));
        } else {
            put(&mut app, recovery, 2, 2);
        }
        let mut charged_unit_work = false;
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
            charged_unit_work |= app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .flat_map(|(_, q)| q.iter())
                .any(|q| matches!(q.item, sim::ProductionItem::Unit(_)) && q.charged > 0);
        }
        if let Some((oil, engineer)) = captured_income {
            let world = app.world_mut();
            assert!(
                world
                    .query::<(&sim::SimId, &sim::Owner)>()
                    .iter(world)
                    .any(|(id, owner)| *id == oil && owner.0 == PlayerId(0)),
                "ordinary capture must activate the income positive"
            );
            assert!(
                !world
                    .query::<&sim::SimId>()
                    .iter(world)
                    .any(|id| *id == engineer)
            );
            assert!(
                charged_unit_work,
                "useful income must finance native unit progress within the unchanged observation bound"
            );
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

#[test]
fn ra8_an_incapable_naval_survivor_does_not_suppress_a_real_armed_miner_attack() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    for y in 20..23 {
        for x in 20..23 {
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
    let submarine = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "typhoon",
        CellPos::new(21, 21),
    )
    .unwrap();
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(9, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(14, 8),
    )
    .unwrap();
    let mut shot = false;
    for _ in 0..4000 {
        for event in step(&mut app) {
            shot |= matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if attacker == miner && found == target);
        }
        if shot {
            break;
        }
    }
    let world = app.world_mut();
    let actors: Vec<_> = world
        .query::<(&sim::SimId, &sim::Health)>()
        .iter(world)
        .filter(|(id, _)| **id == miner || **id == submarine)
        .map(|(id, hp)| (*id, hp.hit_points))
        .collect();
    assert_eq!(
        actors.len(),
        2,
        "both real source actors must remain alive: {actors:?}"
    );
    assert!(actors.iter().all(|(_, hp)| *hp > 0));
    assert!(
        world
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    assert!(world.resource::<sim::MatchState>().outcome().is_none());
    assert!(
        world
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(14, 8)),
        "the ordinary own yard must observe the land target"
    );
    assert!(
        shot,
        "the useful empty miner must physically fire at the seen land building despite the incapable pond submarine"
    );
}

#[test]
fn ra8_a_capable_living_land_force_keeps_empty_miners_out_of_the_attack_wave() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(9, 13),
    )
    .unwrap();
    let tank = spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(12, 12)).unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(14, 8),
    )
    .unwrap();
    // R176: preserve useful worker roles with an actual known economic job,
    // rather than the superseded assumption that an empty map requires idleness.
    put(&mut app, "soviet_refinery", 12, 14);
    let ore_at = CellPos::new(9, 18);
    let mut ore = *app.world().resource::<Grid>().cell(ore_at).unwrap();
    ore.ore = 5000;
    app.world_mut()
        .resource_mut::<Grid>()
        .set_terrain(ore_at, ore);
    let mut delivered = false;
    let mut tank_shot = false;
    let mut miner_shot = false;
    let mut miner_order = false;
    for _ in 0..1000 {
        for event in step(&mut app) {
            delivered |= matches!(
                event,
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                }
            );
            tank_shot |= matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if attacker == tank && found == target);
            miner_shot |=
                matches!(event, sim::SimEvent::WeaponFired { attacker, .. } if attacker == miner);
        }
        miner_order |= app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::AttackMove {units,..} | Command::Attack {units,..} if units.contains(&miner)));
    }
    assert!(
        delivered,
        "the real worker job must produce ordinary income"
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(ore_at)
    );
    assert!(
        tank_shot,
        "the ordinary capable land defender must really fire"
    );
    assert!(
        !miner_shot && !miner_order,
        "the useful land force preserves worker roles"
    );
    let entity = unit_entity(&mut app, tank);
    assert!(app.world().get::<sim::Health>(entity).is_some());
}

#[test]
fn ra8_a_real_reachable_ore_cycle_survives_an_incapable_naval_companion() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    put(&mut app, "soviet_refinery", 12, 14);
    let mut cell = *app
        .world()
        .resource::<Grid>()
        .cell(CellPos::new(9, 16))
        .unwrap();
    cell.ore = 5000;
    app.world_mut()
        .resource_mut::<Grid>()
        .set_terrain(CellPos::new(9, 16), cell);
    for y in 20..23 {
        for x in 20..23 {
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
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "typhoon",
        CellPos::new(21, 21),
    )
    .unwrap();
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(9, 13),
    )
    .unwrap();
    issue(
        &mut app,
        Command::Harvest {
            units: vec![miner],
            target: CellPos::new(12, 14),
        },
    );
    let mut delivered = 0;
    let mut combat_order = false;
    for _ in 0..1000 {
        for event in step(&mut app) {
            if let sim::SimEvent::OreDelivered {
                harvester, credits, ..
            } = event
                && harvester == miner
            {
                delivered += credits;
            }
        }
        combat_order |= app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::AttackMove {units,..} | Command::Attack {units,..} if units.contains(&miner)));
    }
    assert!(
        delivered > 0,
        "the worker must complete a real source ore delivery"
    );
    assert!(
        !combat_order,
        "a genuine reachable economic job must retain the armed worker"
    );
}

#[test]
fn ra8_passive_income_without_actors_or_production_does_not_fund_an_endless_repair_duel() {
    producerless_income_repair(None);
}

#[test]
fn ra8_real_ore_income_with_only_an_unarmed_worker_does_not_renew_producerless_repairs() {
    producerless_income_repair(Some("chrono_miner"));
}

#[test]
fn ra8_real_armed_worker_income_keeps_native_paid_repairs_and_harvesting() {
    producerless_income_repair(Some("war_miner"));
}

fn producerless_income_repair(worker: Option<&str>) {
    let (mut app, target) = paid_refinery_repair_after_producer_loss();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    // Settle only the helper's already blocked former-yard work through native
    // cancellation before measuring this declared empty-queue remnant.
    let pending: Vec<_> = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .queues()
        .flat_map(|(_, q)| q.iter().rev().map(|item| item.item))
        .collect();
    assert!(pending.len() <= 32);
    for item in pending {
        issue(&mut app, Command::CancelProduction { item });
    }
    step(&mut app);
    let miner = if let Some(kind) = worker {
        let cell = CellPos::new(12, 12);
        let mut terrain = *app.world().resource::<Grid>().cell(cell).unwrap();
        terrain.ore = 5000;
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(cell, terrain);
        let miner = spawn_unit(app.world_mut(), PlayerId(0), kind, cell).unwrap();
        issue(
            &mut app,
            Command::Harvest {
                units: vec![miner],
                target: cell,
            },
        );
        Some(miner)
    } else {
        let oil = sim::spawn_building(
            app.world_mut(),
            sim::NEUTRAL,
            "oil_derrick",
            CellPos::new(22, 20),
        )
        .unwrap();
        let engineer = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "engineer",
            CellPos::new(21, 20),
        )
        .unwrap();
        step(&mut app);
        issue(
            &mut app,
            Command::Interact {
                unit: engineer,
                target: oil,
            },
        );
        let mut captured = false;
        for _ in 0..200 {
            step(&mut app);
            let world = app.world_mut();
            captured = !world
                .query::<&sim::SimId>()
                .iter(world)
                .any(|id| *id == engineer);
            if captured {
                break;
            }
        }
        assert!(captured, "ordinary capture must consume the real Engineer");
        None
    };
    let baseline = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    let mut paid_heals = 0;
    let mut ore_delivered = 0;
    for _ in 0..if worker == Some("war_miner") {
        1500
    } else {
        500
    } {
        let e = unit_entity(&mut app, target);
        let before_hp = app.world().get::<sim::Health>(e).unwrap().hit_points;
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        let before_cash = state.credits();
        let kind = app.world().get::<sim::Building>(e).unwrap().kind;
        let maximum = app
            .world()
            .resource::<sim::Definitions>()
            .building_combat(kind)
            .unwrap()
            .hit_points;
        if before_hp < maximum && !state.repairing().contains(&target) {
            issue(&mut app, Command::RepairBuilding { building: target });
        }
        for event in step(&mut app) {
            if let sim::SimEvent::OreDelivered {
                harvester, credits, ..
            } = event
                && Some(harvester) == miner
            {
                ore_delivered += credits;
            }
        }
        let after_cash = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        paid_heals += usize::from(
            app.world().get::<sim::Health>(e).unwrap().hit_points > before_hp
                && after_cash < before_cash,
        );
    }
    assert!(
        paid_heals > 0,
        "the existing remnant must really pay to repair native enemy damage"
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits()
            > baseline,
        "actual native income must genuinely exceed this repair drain"
    );
    assert!(
        worker.is_none() || ore_delivered > 0,
        "the real unarmed worker must physically deliver source ore before the decision"
    );
    let world = app.world_mut();
    let kinds: Vec<_> = world
        .query::<(&sim::Owner, &sim::Building)>()
        .iter(world)
        .filter(|(o, _)| o.0 == PlayerId(0))
        .map(|(_, b)| b.kind)
        .collect();
    if let Some(miner) = miner {
        let owned: Vec<_> = world
            .query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
            .iter(world)
            .filter(|(_, o, _)| o.0 == PlayerId(0))
            .map(|(id, _, _)| *id)
            .collect();
        assert_eq!(
            owned,
            vec![miner],
            "only the actually working source harvester may remain"
        );
    } else {
        assert!(
            !world
                .query::<(&sim::Owner, &sim::Unit)>()
                .iter(world)
                .any(|(o, _)| o.0 == PlayerId(0))
        );
    }
    let definitions = world.resource::<sim::Definitions>();
    let state = world.resource::<sim::Players>().get(PlayerId(0)).unwrap();
    assert!(definitions.units().iter().enumerate().all(|(i, _)| {
        sim::check_buildable(
            definitions,
            state,
            false,
            &kinds,
            sim::ProductionItem::Unit(sim::UnitKind(i as u16)),
        )
        .is_err()
    }));
    assert!(state.queues().all(|(_, q)| q.is_empty()));
    assert!(
        world
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    assert!(world.resource::<sim::MatchState>().outcome().is_none());
    // Start the decision with a genuinely damaged active ordinary repair.
    let mut active = false;
    for _ in 0..200 {
        let e = unit_entity(&mut app, target);
        let hp = app.world().get::<sim::Health>(e).unwrap().hit_points;
        let kind = app.world().get::<sim::Building>(e).unwrap().kind;
        let maximum = app
            .world()
            .resource::<sim::Definitions>()
            .building_combat(kind)
            .unwrap()
            .hit_points;
        let repairing = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .repairing()
            .contains(&target);
        if hp < maximum && repairing {
            active = true;
            break;
        }
        if hp < maximum && !repairing {
            issue(&mut app, Command::RepairBuilding { building: target });
        }
        step(&mut app);
    }
    assert!(active);
    app.world_mut().insert_resource(controllers);
    let mut stopped = false;
    let mut retained_heals = 0;
    for _ in 0..200 {
        let state = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap();
        stopped |= state.repairing().contains(&target)
            && app.world().resource::<CommandQueue>().iter().any(
                |c| matches!(c.command, Command::RepairBuilding { building } if building == target),
            );
        let entity = unit_entity(&mut app, target);
        let hp = app.world().get::<sim::Health>(entity).unwrap().hit_points;
        let cash = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        step(&mut app);
        retained_heals += usize::from(
            app.world().get::<sim::Health>(entity).unwrap().hit_points > hp
                && app
                    .world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits()
                    < cash,
        );
    }
    if worker == Some("war_miner") {
        assert!(
            retained_heals > 0 && !stopped,
            "a genuinely armed source worker must keep useful repair investment"
        );
        let entity = unit_entity(&mut app, miner.unwrap());
        assert!(app.world().get::<sim::Harvester>(entity).is_some());
        assert!(app.world().get::<sim::Health>(entity).unwrap().hit_points > 0);
        return;
    }
    assert!(
        stopped,
        "cash generation alone must not renew paid repairs without a useful actor or native unit production"
    );
    let mut destroyed = false;
    for _ in 0..10000 {
        for event in step(&mut app) {
            assert!(
                !matches!(
                    event,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    } | sim::SimEvent::ProductionCancelled {
                        player: PlayerId(0),
                        ..
                    }
                ),
                "ordinary physical enemy fire must finish without refunds/sale: {event:?}"
            );
            destroyed |= matches!(event, sim::SimEvent::BuildingDestroyed { building, .. } if building == target);
        }
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
        destroyed,
        "the existing actual enemy must physically destroy the remnant"
    );
    assert!(matches!(
        app.world().resource::<sim::MatchState>().outcome(),
        Some(sim::MatchOutcome::Won {
            winner: PlayerId(1),
            ..
        })
    ));
}

#[test]
fn ra8_passive_income_with_real_unit_production_keeps_paid_recovery_repairs() {
    let (mut app, target) = paid_refinery_repair_after_producer_loss();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let pending: Vec<_> = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .queues()
        .flat_map(|(_, q)| q.iter().rev().map(|item| item.item))
        .collect();
    assert!(pending.len() <= 32);
    for item in pending {
        issue(&mut app, Command::CancelProduction { item });
    }
    step(&mut app);
    put(&mut app, "power_plant", 2, 2);
    put(&mut app, "barracks", 8, 14);
    let oil = sim::spawn_building(
        app.world_mut(),
        sim::NEUTRAL,
        "oil_derrick",
        CellPos::new(22, 20),
    )
    .unwrap();
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(21, 20),
    )
    .unwrap();
    step(&mut app);
    issue(
        &mut app,
        Command::Interact {
            unit: engineer,
            target: oil,
        },
    );
    let mut captured = false;
    for _ in 0..200 {
        step(&mut app);
        let world = app.world_mut();
        captured = !world
            .query::<&sim::SimId>()
            .iter(world)
            .any(|id| *id == engineer);
        if captured {
            break;
        }
    }
    assert!(captured);
    let kind = app
        .world()
        .resource::<sim::Definitions>()
        .unit_kind("gi")
        .unwrap();
    let world = app.world_mut();
    assert!(
        !world
            .query::<(&sim::Owner, &sim::Unit)>()
            .iter(world)
            .any(|(o, _)| o.0 == PlayerId(0)),
        "the real producer must qualify before any owned actor exists"
    );
    let kinds: Vec<_> = world
        .query::<(&sim::Owner, &sim::Building)>()
        .iter(world)
        .filter(|(o, _)| o.0 == PlayerId(0))
        .map(|(_, b)| b.kind)
        .collect();
    assert!(
        sim::check_buildable(
            world.resource::<sim::Definitions>(),
            world.resource::<sim::Players>().get(PlayerId(0)).unwrap(),
            false,
            &kinds,
            sim::ProductionItem::Unit(kind)
        )
        .is_ok()
    );
    app.world_mut().insert_resource(controllers);
    let mut paid = false;
    let mut live = false;
    let mut paid_heals = 0;
    for _ in 0..1600 {
        let e = unit_entity(&mut app, target);
        let hp = app.world().get::<sim::Health>(e).unwrap().hit_points;
        let cash = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        for event in step(&mut app) {
            paid |= matches!(event, sim::SimEvent::ProductionComplete { player: PlayerId(0), item: sim::ProductionItem::Unit(found) } if found == kind);
        }
        paid_heals += usize::from(
            app.world().get::<sim::Health>(e).unwrap().hit_points > hp
                && app
                    .world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits()
                    < cash,
        );
        let world = app.world_mut();
        live |= world
            .query::<(&sim::Owner, &sim::Unit, &sim::Health)>()
            .iter(world)
            .any(|(o, u, h)| o.0 == PlayerId(0) && u.kind == kind && h.hit_points > 0);
        if paid && live && paid_heals > 0 {
            break;
        }
    }
    assert!(
        paid && live && paid_heals > 0,
        "useful income/producer must retain real paid heals and normal funded live infantry: paid{paid}/live{live}/heals{paid_heals}"
    );
}

#[test]
fn ra8_unfunded_garrison_recovery_retains_its_released_infantry_for_real_offence() {
    let mut app = scenario("russia", "easy", 62, 0, false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let remnant = put(&mut app, "radar_tower", 15, 8);
    for _ in 0..2 {
        let world = app.world_mut();
        for (id, owner, mut hp) in world
            .query::<(&sim::SimId, &sim::Owner, &mut sim::Health)>()
            .iter_mut(world)
        {
            if owner.0 == PlayerId(0) && *id != remnant {
                hp.hit_points = 0;
            }
        }
        step(&mut app);
    }
    let host = civilian(&mut app, CellPos::new(13, 8));
    let infantry = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 8),
    )
    .unwrap();
    let enemy = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "tesla_reactor",
        CellPos::new(27, 8),
    )
    .unwrap();
    // A real unarmed observer establishes the intended building objective;
    // the released infantry must not depend on a seeded controller memory.
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(25, 10),
    )
    .unwrap();
    step(&mut app);
    issue(
        &mut app,
        Command::Interact {
            unit: infantry,
            target: host,
        },
    );
    for _ in 0..40 {
        step(&mut app);
        if app
            .world()
            .resource::<sim::SpecialState>()
            .cargo
            .get(&host)
            .is_some_and(|cargo| cargo.iter().any(|p| p.id == infantry))
        {
            break;
        }
    }
    assert!(
        app.world().resource::<sim::SpecialState>().cargo[&host]
            .iter()
            .any(|p| p.id == infantry)
    );
    assert_eq!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        62
    );
    let definitions = app.world().resource::<sim::Definitions>();
    assert_eq!(
        definitions
            .production(sim::ProductionItem::Unit(
                definitions.unit_kind("conscript").unwrap()
            ))
            .unwrap()
            .cost,
        100
    );
    app.world_mut().insert_resource(controllers);
    let mut released = false;
    let mut shot = false;
    for _ in 0..7000 {
        let events = step(&mut app);
        let world = app.world_mut();
        let live = world
            .query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
            .iter(world)
            .any(|(id, owner, _)| *id == infantry && owner.0 == PlayerId(0));
        released |= live;
        if released {
            assert!(!world.resource::<CommandQueue>().iter().any(|c| matches!(c.command, Command::Interact { unit, target } if unit == infantry && target == host)), "last-stand recovered infantry was immediately regarrisoned");
        }
        shot |= events.iter().any(|e| matches!(e, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == infantry && *target == enemy));
        if shot {
            break;
        }
        assert!(world.resource::<sim::MatchState>().outcome().is_none());
        assert!(
            world
                .resource::<sim::MatchState>()
                .defeated(PlayerId(0))
                .is_none()
        );
    }
    assert!(
        released,
        "ordinary last-stand Deploy must recover the actual occupant"
    );
    let units = {
        let world = app.world_mut();
        world
            .query::<(&sim::SimId, &sim::Mobile, &sim::Weapon)>()
            .iter(world)
            .map(|(id, m, w)| (*id, *m, *w))
            .collect::<Vec<_>>()
    };
    assert!(
        shot,
        "the recovered source infantry must actually fire on the known enemy building; controller {:?}; units {units:?}",
        app.world().resource::<sim::AiPlayers>()
    );
}

#[test]
fn ra8_garrison_admission_keeps_source_cost_cash_real_income_and_actual_defensive_fire() {
    for mode in ["cash", "income", "defence"] {
        let mut app = scenario(
            "russia",
            "easy",
            if mode == "cash" { 100 } else { 62 },
            0,
            false,
        );
        let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
        let remnant = put(&mut app, "radar_tower", 15, 12);
        for _ in 0..2 {
            let world = app.world_mut();
            for (id, owner, mut hp) in world
                .query::<(&sim::SimId, &sim::Owner, &mut sim::Health)>()
                .iter_mut(world)
            {
                if owner.0 == PlayerId(0) && *id != remnant {
                    hp.hit_points = 0;
                }
            }
            step(&mut app);
        }
        if mode == "income" {
            put(&mut app, "tesla_reactor", 2, 2);
            put(&mut app, "soviet_refinery", 2, 8);
            put(&mut app, "soviet_war_factory", 2, 14);
            put(&mut app, "soviet_battle_lab", 2, 20);
            let oil = sim::spawn_building(
                app.world_mut(),
                sim::NEUTRAL,
                "oil_derrick",
                CellPos::new(22, 20),
            )
            .unwrap();
            let engineer = spawn_unit(
                app.world_mut(),
                PlayerId(0),
                "engineer",
                CellPos::new(21, 20),
            )
            .unwrap();
            step(&mut app);
            issue(
                &mut app,
                Command::Interact {
                    unit: engineer,
                    target: oil,
                },
            );
            for _ in 0..200 {
                step(&mut app);
                let world = app.world_mut();
                if !world
                    .query::<&sim::SimId>()
                    .iter(world)
                    .any(|id| *id == engineer)
                {
                    break;
                }
            }
            let oil_entity = unit_entity(&mut app, oil);
            assert_eq!(
                app.world().get::<sim::Owner>(oil_entity).unwrap().0,
                PlayerId(0)
            );
            let item = sim::ProductionItem::Unit(
                app.world()
                    .resource::<sim::Definitions>()
                    .unit_kind("kirov")
                    .unwrap(),
            );
            issue(&mut app, Command::QueueProduction { item });
            let mut narrow_wallet = false;
            for _ in 0..3000 {
                step(&mut app);
                let state = app
                    .world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap();
                let paid: i64 = state
                    .queues()
                    .flat_map(|(_, q)| q)
                    .map(|head| head.charged)
                    .sum();
                if state.credits() > 0 && state.credits() < 100 && paid > 0 {
                    assert!(
                        state.credits() + paid > 1062,
                        "actual periodic income after capture must finance the native head"
                    );
                    narrow_wallet = true;
                    break;
                }
            }
            assert!(narrow_wallet);
        }
        let host = civilian(&mut app, CellPos::new(13, 8));
        let infantry = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "conscript",
            CellPos::new(12, 8),
        )
        .unwrap();
        let enemy = (mode == "defence")
            .then(|| spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(18, 8)).unwrap());
        step(&mut app);
        let wallet = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        if mode == "cash" {
            assert_eq!(wallet, 100);
        } else {
            assert!(wallet > 0 && wallet < 100);
        }
        app.world_mut().insert_resource(controllers);
        let mut entered = false;
        let mut fired = false;
        for _ in 0..500 {
            for event in step(&mut app) {
                fired |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == host && Some(target) == enemy);
            }
            entered |= app
                .world()
                .resource::<sim::SpecialState>()
                .cargo
                .get(&host)
                .is_some_and(|cargo| cargo.iter().any(|p| p.id == infantry));
            if entered && (mode != "defence" || fired) {
                break;
            }
        }
        assert!(
            entered,
            "{mode}: useful ordinary garrison admission was lost"
        );
        if mode == "defence" {
            assert!(
                fired,
                "the real source occupation gun must fire on the observed hostile infantry"
            );
        }
    }
}

fn surface_building_focus_fixture() -> (App, sim::SimId, sim::SimId) {
    surface_building_focus_fixture_for("rhino")
}

fn surface_building_focus_fixture_for(kind: &str) -> (App, sim::SimId, sim::SimId) {
    let mut app = scenario("russia", "medium", 0, 0, false);
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "radar_tower",
        CellPos::new(22, 8),
    )
    .unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(23, 11),
    )
    .unwrap();
    let tank = spawn_unit(app.world_mut(), PlayerId(0), kind, CellPos::new(14, 8)).unwrap();
    if kind != "rhino" {
        issue(
            &mut app,
            Command::ForceMove {
                units: vec![tank],
                target: CellPos::new(14, 40),
            },
        );
    }
    let mut focused = false;
    let mut mission = false;
    for _ in 0..7000 {
        let events = step(&mut app);
        let entity = unit_entity(&mut app, tank);
        mission |= matches!(app.world().get::<sim::Weapon>(entity).unwrap().order,
            Some(sim::AttackOrder::Attack { target: id, .. }) if id == target);
        if kind != "rhino" && !mission && app.world().resource::<SimTick>().0.is_multiple_of(200) {
            let x = if app.world().resource::<SimTick>().0.is_multiple_of(400) {
                14
            } else {
                2
            };
            issue(
                &mut app,
                Command::ForceMove {
                    units: vec![tank],
                    target: CellPos::new(x, 40),
                },
            );
        }
        focused |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target: victim, .. } if *attacker == tank && *victim == target));
        if focused {
            break;
        }
    }
    assert!(
        focused,
        "the healthy real building mission must reach native fire first"
    );
    let entity = unit_entity(&mut app, tank);
    assert!(
        matches!(app.world().get::<sim::Weapon>(entity).unwrap().order,
        Some(sim::AttackOrder::Attack { target: id, .. }) if id == target),
        "the premise is an actual direct building Attack, not incidental AttackMove fire"
    );
    (app, target, tank)
}

#[test]
fn ra8_surface_building_focus_responds_to_a_new_real_local_defender() {
    let (mut app, _, tank) = surface_building_focus_fixture();
    let entity = unit_entity(&mut app, tank);
    let at = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let hostile = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "rhino",
        CellPos::new(at.x, at.y + 4),
    )
    .unwrap();
    let mut threatened = false;
    let mut responded = false;
    for _ in 0..150 {
        for event in step(&mut app) {
            threatened |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == hostile && target == tank);
            responded |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == tank && target == hostile);
        }
        if threatened && responded {
            break;
        }
    }
    assert!(
        threatened,
        "the new visible source defender must actually fire at the focused tank"
    );
    assert!(
        responded,
        "building focus must yield to the actual local hostile instead of ignoring its fire"
    );
}

#[test]
fn ra8_a_first_stationary_hidden_mirage_cannot_change_surface_focus_intents() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    #[derive(bevy_ecs::resource::Resource, Default)]
    struct Boundary {
        controllers: sim::AiPlayers,
        commands: Vec<sim::IssuedCommand>,
        hidden: bool,
    }
    fn capture(world: &mut bevy_ecs::world::World) {
        let controllers = world.resource::<sim::AiPlayers>().clone();
        let commands = world.resource::<CommandQueue>().iter().cloned().collect();
        let hidden = world
            .resource::<sim::SpecialState>()
            .hidden
            .iter()
            .any(|(observer, _)| *observer == PlayerId(0));
        world.insert_resource(Boundary {
            controllers,
            commands,
            hidden,
        });
    }
    let (mut actual, target, tank) = surface_building_focus_fixture();
    let (mut control, control_target, control_tank) = surface_building_focus_fixture();
    assert_eq!((target, tank), (control_target, control_tank));
    while !actual.world().resource::<SimTick>().0.is_multiple_of(20) {
        step(&mut actual);
        step(&mut control);
    }
    let entity = unit_entity(&mut actual, tank);
    let at = actual.world().get::<sim::Mobile>(entity).unwrap().cell;
    let mirage = spawn_unit(
        actual.world_mut(),
        PlayerId(1),
        "mirage_tank",
        CellPos::new(at.x, at.y + 4),
    )
    .unwrap();
    for app in [&mut actual, &mut control] {
        app.init_resource::<Boundary>();
        app.add_systems(
            SimSchedule,
            capture
                .in_set(sim::SimSet::Ai)
                .after(sim::SimCoreSet(sim::SimSet::Ai)),
        );
        step(app);
    }
    let a = actual.world().resource::<Boundary>();
    let b = control.world().resource::<Boundary>();
    assert!(
        a.hidden,
        "the actual stationary source Mirage must conceal before this due AI decision"
    );
    assert!(!b.hidden);
    assert_eq!(a.controllers, b.controllers);
    assert_eq!(a.commands, b.commands);
    assert!(!a.commands.iter().any(
        |command| matches!(&command.command, Command::Attack { target, .. } if *target == mirage)
    ));
}

#[test]
fn ra8_surface_response_does_not_replace_the_existing_long_range_artillery_policy() {
    let (mut app, target, tank) = surface_building_focus_fixture_for("prism_tank");
    let entity = unit_entity(&mut app, tank);
    let at = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let hostile = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "rhino",
        CellPos::new(at.x, at.y + 4),
    )
    .unwrap();
    let mut threatened = false;
    for _ in 0..20 {
        for event in step(&mut app) {
            threatened |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == hostile && target == tank);
        }
        assert!(
            !app.world().resource::<CommandQueue>().iter().any(
                |c| matches!(&c.command, Command::AttackMove { units, .. } if units.contains(&tank))
            ),
            "the new short-range response must leave artillery's original policy untouched"
        );
        assert!(
            matches!(app.world().get::<sim::Weapon>(entity).unwrap().order,
            Some(sim::AttackOrder::Attack { target: id, .. }) if id == target)
        );
    }
    assert!(
        threatened,
        "the long-range exclusion must be checked under real incoming fire"
    );
}

#[test]
fn ra8_unseen_building_occupancy_cannot_change_a_surface_response_route_intent() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    #[derive(bevy_ecs::resource::Resource, Default)]
    struct Boundary {
        controllers: sim::AiPlayers,
        commands: Vec<sim::IssuedCommand>,
    }
    fn capture(world: &mut bevy_ecs::world::World) {
        world.insert_resource(Boundary {
            controllers: world.resource::<sim::AiPlayers>().clone(),
            commands: world.resource::<CommandQueue>().iter().cloned().collect(),
        });
    }
    let (mut actual, _, tank) = surface_building_focus_fixture();
    let (mut control, _, control_tank) = surface_building_focus_fixture();
    assert_eq!(tank, control_tank);
    for app in [&mut actual, &mut control] {
        let mut rules = app.world().resource::<sim::MatchState>().rules();
        rules.fog_of_war = true;
        app.world_mut()
            .resource_mut::<sim::MatchState>()
            .set_rules(rules);
        step(app);
    }
    while !actual.world().resource::<SimTick>().0.is_multiple_of(20) {
        step(&mut actual);
        step(&mut control);
    }
    let entity = unit_entity(&mut actual, tank);
    let at = actual.world().get::<sim::Mobile>(entity).unwrap().cell;
    let unseen = CellPos::new(18, 25);
    for app in [&mut actual, &mut control] {
        spawn_unit(
            app.world_mut(),
            PlayerId(1),
            "rhino",
            CellPos::new(at.x, at.y + 4),
        )
        .unwrap();
        let mut grid = app.world_mut().resource_mut::<Grid>();
        let cliff = Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        };
        // The only public route around the vertical wall crosses this unseen
        // one-cell gap. A raw occupancy query would falsely close that route.
        for y in 0..32 {
            grid.set_terrain(CellPos::new(20, y), cliff);
        }
        for x in 0..20 {
            if x != unseen.x {
                grid.set_terrain(CellPos::new(x, unseen.y), cliff);
            }
        }
    }
    sim::spawn_building(actual.world_mut(), PlayerId(1), "sentry_gun", unseen).unwrap();
    for app in [&mut actual, &mut control] {
        assert!(
            !app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(unseen)
        );
        app.init_resource::<Boundary>();
        app.add_systems(
            SimSchedule,
            capture
                .in_set(sim::SimSet::Ai)
                .after(sim::SimCoreSet(sim::SimSet::Ai)),
        );
        step(app);
    }
    let a = actual.world().resource::<Boundary>();
    let b = control.world().resource::<Boundary>();
    assert_eq!(a.controllers, b.controllers);
    assert_eq!(a.commands, b.commands);
    let destinations: Vec<_> = a
        .commands
        .iter()
        .filter_map(|command| match &command.command {
            Command::AttackMove { units, target } if units.contains(&tank) => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(
        destinations.len(),
        1,
        "the real local threat must run the new response planner"
    );
    assert_ne!(
        destinations[0], at,
        "the public detour must reach the outside ring rather than fall back"
    );
    assert!(
        actual
            .world()
            .resource::<Grid>()
            .cell(destinations[0])
            .unwrap()
            .passable
    );
    assert!(
        !actual
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(unseen)
    );
}

#[test]
fn ra8_an_idle_detector_discovers_an_unseen_start_after_initial_team_readiness() {
    let mut app = scenario("russia", "easy", 0, 0, false);
    // A real healthy combat actor preserves this base through the natural
    // opening. No controller history, wave deadline or SimTick is injected.
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(14, 8),
    )
    .unwrap();
    let remote = {
        let world = app.world_mut();
        world
            .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
            .iter(world)
            .find(|(_, owner, _)| owner.0 == PlayerId(1))
            .map(|(id, _, _)| *id)
            .unwrap()
    };
    let dog = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(8, 11),
    )
    .unwrap();
    let controller = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap();
    assert!(controller.wave().is_empty() && controller.target().is_none());
    assert!(!controller.known_buildings().contains_key(&remote));
    assert!(
        controller
            .waypoints()
            .iter()
            .any(|w| w.priority && w.seen == 0 && w.cell == CellPos::new(38, 38))
    );
    let first_wave = controller.difficulty().first_wave;
    while app.world().resource::<SimTick>().0 < u64::from(first_wave) {
        step(&mut app);
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        assert!(controller.wave().is_empty() && controller.target().is_none());
        assert!(!controller.known_buildings().contains_key(&remote));
        assert!(!app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::AttackMove { units, target } if units.contains(&dog) && *target == CellPos::new(38, 38))), "initial source team/escort assembly must remain intact");
        assert!(
            app.world()
                .resource::<sim::MatchState>()
                .outcome()
                .is_none()
        );
    }
    let mut scouting = false;
    let mut discovered = false;
    for _ in 0..1000 {
        step(&mut app);
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        assert!(
            !controller.wave().contains(&dog),
            "independent reconnaissance must not consume the waiting main cohort"
        );
        let issued = app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::AttackMove { units, target } if units.contains(&dog) && *target == CellPos::new(38, 38)));
        if issued && !scouting {
            assert!(
                controller.target().is_none(),
                "the native scout admission must occur without a main mission"
            );
            assert!(app.world().resource::<SimTick>().0 >= u64::from(first_wave));
        }
        scouting |= issued;
        discovered = controller.known_buildings().contains_key(&remote);
        if discovered {
            break;
        }
    }
    assert!(
        scouting && discovered,
        "the actual idle detector must move and physically discover the unknown start without waiting for a combat mission"
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(38, 38))
    );
}

#[test]
fn ra8_idle_detectors_discover_an_unseen_start_while_the_real_wave_fights() {
    parallel_detector_discovery(false);
}

#[test]
fn ra8_a_healthy_detector_route_survives_the_next_main_wave_admission() {
    parallel_detector_discovery(true);
}

fn parallel_detector_fixture(long_route: bool) -> (App, sim::SimId, sim::SimId, sim::SimId) {
    let mut app = scenario_mode(
        "russia",
        "medium",
        0,
        0,
        false,
        ScenarioMode {
            map_side: Some(64),
            ..ScenarioMode::default()
        },
    );
    if long_route {
        for (index, y) in [12, 15, 18, 21, 24, 27, 30, 33].into_iter().enumerate() {
            for x in 0..64 {
                if x == if index % 2 == 0 { 61 } else { 2 } {
                    continue;
                }
                app.world_mut().resource_mut::<Grid>().set_terrain(
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
    let factory = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_war_factory",
        CellPos::new(22, 8),
    )
    .unwrap();
    // A normal weak anti-building weapon leaves a genuine, slow ongoing mission.
    let front = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "flak_track",
        CellPos::new(12, 8),
    )
    .unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(20, 8),
    )
    .unwrap();
    let remote = {
        let world = app.world_mut();
        world
            .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
            .iter(world)
            .find(|(id, owner, _)| owner.0 == PlayerId(1) && **id != factory)
            .map(|(id, _, _)| *id)
            .unwrap()
    };
    let mut fired = false;
    for _ in 0..6000 {
        for event in step(&mut app) {
            fired |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == front && target == factory);
        }
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        if fired
            && controller.wave().contains(&front)
            && controller.target() == Some(sim::WaveTarget::Building(factory))
        {
            break;
        }
    }
    let controller = app
        .world()
        .resource::<sim::AiPlayers>()
        .controller(PlayerId(0))
        .unwrap();
    assert!(fired && controller.wave().contains(&front));
    assert_eq!(
        controller.target(),
        Some(sim::WaveTarget::Building(factory))
    );
    assert!(
        !controller.known_buildings().contains_key(&remote),
        "the distant real base must not already be observed"
    );
    (app, factory, front, remote)
}

fn parallel_detector_discovery(long_route: bool) {
    let (mut app, factory, front, remote) = parallel_detector_fixture(long_route);
    let began = app.world().resource::<SimTick>().0;
    let dog = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(8, 11),
    )
    .unwrap();
    let mut scouting = false;
    let mut discovered = false;
    let mut real_fire = false;
    let mut crossed_admission = false;
    for _ in 0..if long_route { 10000 } else { 1500 } {
        for event in step(&mut app) {
            real_fire |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == front && target == factory);
        }
        scouting |= app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::AttackMove { units, target } if units.contains(&dog) && *target == CellPos::new(38, 38)));
        let controller = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        discovered = controller.known_buildings().contains_key(&remote);
        if scouting && !discovered && app.world().resource::<SimTick>().0 > began + 3333 {
            crossed_admission = true;
            let in_wave = controller.wave().contains(&dog);
            if in_wave {
                eprintln!(
                    "scout boundary tick{} target{:?} waypoints{:?}",
                    app.world().resource::<SimTick>().0,
                    controller.target(),
                    controller
                        .waypoints()
                        .iter()
                        .filter(|w| w.priority)
                        .collect::<Vec<_>>()
                );
                let e = unit_entity(&mut app, dog);
                eprintln!(
                    "scout actual {:?} {:?} {:?}",
                    app.world().get::<sim::Mobile>(e),
                    app.world().get::<sim::Orders>(e),
                    app.world().get::<sim::Weapon>(e)
                );
                let world = app.world_mut();
                eprintln!(
                    "factory remains {:?}",
                    world
                        .query::<(&sim::SimId, &sim::Health)>()
                        .iter(world)
                        .find(|(id, _)| **id == factory)
                );
            }
            assert!(
                !in_wave,
                "an independently scouting Dog must not silently join the unrelated combat wave"
            );
        }
        if discovered {
            break;
        }
    }
    assert!(
        scouting && discovered && real_fire,
        "an idle detector must physically reveal the unknown start while native building fire continues: scout{scouting}/known{discovered}/fire{real_fire}"
    );
    assert!(
        !long_route || crossed_admission,
        "the real route must span a complete source Medium team interval"
    );
    let e = unit_entity(&mut app, dog);
    assert!(app.world().get::<sim::Health>(e).unwrap().hit_points > 0);
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(CellPos::new(38, 38))
    );
}

#[test]
fn ra8_a_real_local_detector_defender_is_not_diverted_to_reconnaissance() {
    let (mut app, _, _, _) = parallel_detector_fixture(false);
    let dog = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(8, 11),
    )
    .unwrap();
    let enemy = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "conscript",
        CellPos::new(11, 12),
    )
    .unwrap();
    let mut actual_defense = false;
    let mut scout = false;
    for _ in 0..300 {
        scout |= app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command,
            Command::AttackMove { units, target } if units.contains(&dog) && *target == CellPos::new(38, 38)));
        for event in step(&mut app) {
            actual_defense |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == dog && target == enemy);
        }
        if actual_defense {
            break;
        }
    }
    assert!(
        actual_defense && !scout,
        "the actual visible local defender must retain its native source attack"
    );
}

#[test]
fn ra8_unseen_building_occupancy_does_not_change_parallel_detector_intents() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    #[derive(bevy_ecs::resource::Resource, Default)]
    struct Boundary {
        controllers: sim::AiPlayers,
        commands: Vec<sim::IssuedCommand>,
    }
    fn capture(world: &mut bevy_ecs::world::World) {
        let controllers = world.resource::<sim::AiPlayers>().clone();
        let commands = world.resource::<CommandQueue>().iter().cloned().collect();
        world.insert_resource(Boundary {
            controllers,
            commands,
        });
    }
    let (mut actual, _, _, _) = parallel_detector_fixture(false);
    let (mut control, _, _, _) = parallel_detector_fixture(false);
    while !actual.world().resource::<SimTick>().0.is_multiple_of(20) {
        step(&mut actual);
        step(&mut control);
    }
    let dog = spawn_unit(
        actual.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(8, 11),
    )
    .unwrap();
    assert_eq!(
        spawn_unit(
            control.world_mut(),
            PlayerId(0),
            "attack_dog",
            CellPos::new(8, 11)
        )
        .unwrap(),
        dog
    );
    let hidden = CellPos::new(35, 35);
    sim::spawn_building(actual.world_mut(), PlayerId(1), "sentry_gun", hidden).unwrap();
    for app in [&mut actual, &mut control] {
        assert!(
            !app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(hidden)
        );
        app.init_resource::<Boundary>();
        app.add_systems(
            SimSchedule,
            capture
                .in_set(sim::SimSet::Ai)
                .after(sim::SimCoreSet(sim::SimSet::Ai)),
        );
        step(app);
        assert!(
            !app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(hidden)
        );
    }
    let a = actual.world().resource::<Boundary>();
    let b = control.world().resource::<Boundary>();
    assert_eq!(a.controllers, b.controllers);
    assert_eq!(a.commands, b.commands);
    assert!(a.commands.iter().any(|issued| matches!(&issued.command, Command::AttackMove { units, target } if units.contains(&dog) && *target == CellPos::new(38, 38))), "the real new scout must run in both twins");
}

#[test]
fn ra8_empty_armed_workers_keep_fighting_while_an_optional_asset_funds_a_paid_recruit() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    put(&mut app, "tesla_reactor", 3, 5);
    let asset = put(&mut app, "soviet_service_depot", 18, 5);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let defs = app.world().resource::<sim::Definitions>();
    let kind = defs.unit_kind("conscript").unwrap();
    let item = sim::ProductionItem::Unit(kind);
    let cost = defs.production(item).unwrap().cost;
    let refund = defs.refund(defs.building_kind("soviet_service_depot").unwrap());
    assert!(refund >= cost && cost > 0);
    let mut sold = None;
    let mut paid = false;
    let mut charged = false;
    let mut recruited = std::collections::BTreeSet::new();
    let mut recruit_shot = false;
    let mut miner_shot = false;
    for _ in 0..6000 {
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q)
            .any(|q| q.item == item && q.charged > 0);
        for event in step(&mut app) {
            match event {
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    refund: amount,
                } => {
                    assert_eq!(building, asset, "retain all ordinary producers");
                    assert_eq!(amount, refund, "native exact refund only");
                    assert!(sold.replace(amount).is_none(), "one optional sale suffices");
                }
                sim::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: found,
                } if found == item => paid = true,
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    recruited.insert(unit);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if found == target => {
                    miner_shot |= attacker == miner;
                    recruit_shot |= recruited.contains(&attacker);
                }
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                } => panic!("declared empty map must have no income"),
                _ => {}
            }
        }
        if recruit_shot && miner_shot {
            break;
        }
    }
    let entity = unit_entity(&mut app, miner);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    eprintln!(
        "worker reinforcement sold{sold:?} exact_refund{refund} source_cost{cost} paid{paid} recruits{recruited:?} miner_shot{miner_shot} recruit_shot{recruit_shot}"
    );
    assert_eq!(
        sold,
        Some(refund),
        "active useful worker must not exclude financing ordinary reinforcement from an optional asset"
    );
    assert!(
        charged && paid && !recruited.is_empty() && recruit_shot && miner_shot,
        "real paid recruit and the retained living worker must both make native building shots"
    );
}

#[test]
fn ra8_paid_reinforcements_do_not_send_an_active_empty_worker_wave_home() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(21, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(25, 13),
    )
    .unwrap();
    let mut initial_shot = false;
    for _ in 0..6000 {
        initial_shot |= step(&mut app).iter().any(|e| matches!(e,sim::SimEvent::WeaponFired {attacker,target:found,..} if *attacker==miner && *found==target));
        if initial_shot
            && app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&miner)
        {
            break;
        }
    }
    assert!(
        initial_shot,
        "the living worker must first enter a real ordinary offensive mission"
    );
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&miner)
    );
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    put(&mut app, "tesla_reactor", 3, 5);
    let asset = put(&mut app, "soviet_service_depot", 18, 5);
    let mut paid = false;
    let mut spawned = None;
    let mut sale = false;
    let mut after_spawn_shot = false;
    let mut lost_membership = false;
    let mut replacement_seen_at = None;
    let mut redirected = false;
    for _ in 0..1500 {
        for event in step(&mut app) {
            match event {
                sim::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: sim::ProductionItem::Unit(kind),
                } if app
                    .world()
                    .resource::<sim::Definitions>()
                    .unit(kind)
                    .unwrap()
                    .id
                    == "conscript" =>
                {
                    paid = true
                }
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    ..
                } if building == asset => sale = true,
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    spawned = Some(unit);
                    replacement_seen_at.get_or_insert(app.world().resource::<SimTick>().0);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if attacker == miner && found == target && spawned.is_some() => {
                    after_spawn_shot |= app.world().resource::<SimTick>().0
                        >= replacement_seen_at.unwrap() + sim::HARVEST_RETRY_TICKS
                }
                _ => {}
            }
        }
        if spawned.is_some() {
            lost_membership |= !app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&miner);
            let gather = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .gather();
            redirected |= app
                .world()
                .resource::<CommandQueue>()
                .iter()
                .any(|issued| match &issued.command {
                    Command::Harvest { units, .. } => units.contains(&miner),
                    Command::AttackMove { units, target } => {
                        units.contains(&miner) && *target == gather
                    }
                    _ => false,
                });
            if app.world().resource::<SimTick>().0 >= replacement_seen_at.unwrap() + 400 {
                break;
            }
        }
    }
    eprintln!(
        "worker mission initial_shot{initial_shot} sale{sale} paid{paid} spawned{spawned:?} lost_membership{lost_membership} redirected{redirected} after_spawn_shot{after_spawn_shot}"
    );
    assert!(
        sale && paid && spawned.is_some(),
        "ordinary optional refund must first produce a real replacement"
    );
    assert!(
        !lost_membership && !redirected && after_spawn_shot,
        "a naturally serving living worker must retain its offensive mission and actual building fire after ordinary paid reinforcement arrives"
    );
    let entity = unit_entity(&mut app, miner);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    // A newly available real economic job still belongs to an ordinary Harvest.
    // This is declared terrain setup, followed by public command and native income.
    // R184 named setup migration: introduce the controlled refinery with its
    // genuinely observed economic job, after the no-income offensive phase.
    // Earlier no-ore ownership is now an optional financing asset, not a
    // guaranteed future income commitment known by the controller.
    let refinery = put(&mut app, "soviet_refinery", 12, 17);
    let ore_cell = CellPos::new(14, 21);
    let mut terrain = *app.world().resource::<Grid>().cell(ore_cell).unwrap();
    terrain.ore = 5000;
    app.world_mut()
        .resource_mut::<Grid>()
        .set_terrain(ore_cell, terrain);
    let refinery_entity = unit_entity(&mut app, refinery);
    let origin = app
        .world()
        .get::<sim::Building>(refinery_entity)
        .unwrap()
        .footprint
        .origin;
    issue(
        &mut app,
        Command::Harvest {
            units: vec![miner],
            target: origin,
        },
    );
    let mut delivered = false;
    for _ in 0..4000 {
        delivered |= step(&mut app).iter().any(|event| {
            matches!(
                event,
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                }
            )
        });
        if delivered {
            break;
        }
    }
    assert!(
        delivered,
        "real newly available ore must still produce ordinary income after the serving worker's public Harvest"
    );
    assert!(
        !app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&miner)
    );
}

#[test]
fn ra8_real_forces_economic_jobs_and_financed_heads_preserve_optional_worker_assets() {
    for control in ["force", "income", "paid_head"] {
        let mut app = scenario(
            "russia",
            if control == "paid_head" {
                "medium"
            } else {
                "brutal"
            },
            if control == "paid_head" { 100 } else { 0 },
            0,
            false,
        );
        let barracks = put(&mut app, "soviet_barracks", 5, 12);
        put(&mut app, "tesla_reactor", 3, 5);
        let asset = put(&mut app, "soviet_service_depot", 18, 5);
        let miner = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "war_miner",
            CellPos::new(12, 13),
        )
        .unwrap();
        let target = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "soviet_construction_yard",
            CellPos::new(17, 13),
        )
        .unwrap();
        let fighter = if control == "force" {
            Some(spawn_unit(app.world_mut(), PlayerId(0), "rhino", CellPos::new(13, 12)).unwrap())
        } else {
            None
        };
        if matches!(control, "force" | "paid_head") {
            // R182 setup migration: ordinary presence alone no longer fills
            // the declared reinforcement target. Preserve the original full
            // observation/no-sale/fire/paid-head assertions with a real cohort.
            // The paid branch remains one actor short until its funded head spawns.
            let cohort = app
                .world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .difficulty()
                .wave_size;
            for n in 0..cohort.saturating_sub(2) {
                spawn_unit(
                    app.world_mut(),
                    PlayerId(0),
                    if control == "force" {
                        "rhino"
                    } else {
                        "conscript"
                    },
                    CellPos::new(
                        3 + i32::try_from(n % 6).unwrap(),
                        25 + i32::try_from(n / 6).unwrap(),
                    ),
                )
                .unwrap();
            }
        }
        if control == "income" {
            put(&mut app, "soviet_refinery", 12, 17);
            let at = CellPos::new(14, 21);
            let mut cell = *app.world().resource::<Grid>().cell(at).unwrap();
            cell.ore = 5000;
            app.world_mut().resource_mut::<Grid>().set_terrain(at, cell);
        }
        if control == "income" {
            let controllers = std::mem::take(
                app.world_mut()
                    .resource_mut::<sim::AiPlayers>()
                    .into_inner(),
            );
            issue(&mut app, Command::Stop { units: vec![miner] });
            step(&mut app);
            let entity = unit_entity(&mut app, miner);
            assert_eq!(
                app.world().get::<sim::Harvester>(entity).unwrap().state,
                sim::HarvestState::Idle
            );
            assert!(
                app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .vision()
                    .is_visible(CellPos::new(14, 21))
            );
            let world = app.world_mut();
            assert!(
                !world
                    .query::<(&sim::Owner, &sim::Building)>()
                    .iter(world)
                    .any(|(o, b)| o.0 == PlayerId(0)
                        && world
                            .resource::<sim::Definitions>()
                            .building(b.kind)
                            .unwrap()
                            .id
                            == "soviet_war_factory")
            );
            app.world_mut().insert_resource(controllers);
        }
        if control == "paid_head" {
            let controllers = std::mem::take(
                app.world_mut()
                    .resource_mut::<sim::AiPlayers>()
                    .into_inner(),
            );
            let item = sim::ProductionItem::Unit(
                app.world()
                    .resource::<sim::Definitions>()
                    .unit_kind("conscript")
                    .unwrap(),
            );
            assert_eq!(
                app.world()
                    .resource::<sim::Definitions>()
                    .production(item)
                    .unwrap()
                    .cost,
                100
            );
            issue(&mut app, Command::QueueProduction { item });
            for _ in 0..20 {
                step(&mut app);
            }
            let state = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            let head = state
                .queues()
                .flat_map(|(_, q)| q)
                .find(|q| q.item == item)
                .unwrap();
            assert!(head.charged > 0 && head.charged < 100);
            assert_eq!(state.credits(), 100 - head.charged);
            app.world_mut().insert_resource(controllers);
        }
        let mut delivered = false;
        let mut fired = false;
        let mut spawned = false;
        for _ in 0..2000 {
            for event in step(&mut app) {
                assert!(
                    !matches!(
                        event,
                        sim::SimEvent::BuildingSold {
                            player: PlayerId(0),
                            ..
                        }
                    ),
                    "{control}: native financing must preserve this optional asset"
                );
                delivered |= matches!(
                    event,
                    sim::SimEvent::OreDelivered {
                        player: PlayerId(0),
                        ..
                    }
                );
                fired |= matches!(event,sim::SimEvent::WeaponFired {attacker,target:found,..} if Some(attacker)==fighter && found==target);
                spawned |= matches!(event,sim::SimEvent::UnitSpawned {player:PlayerId(0),building,..} if building==barracks);
            }
        }
        let asset_entity = unit_entity(&mut app, asset);
        assert!(app.world().get::<sim::Building>(asset_entity).is_some());
        match control {
            "force" => assert!(fired),
            "income" => assert!(delivered),
            "paid_head" => assert!(spawned),
            _ => unreachable!(),
        }
    }
}

#[test]
fn ra8_redundant_source_power_can_finance_a_real_recruit_without_selling_its_yard() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    let first = put(&mut app, "tesla_reactor", 3, 5);
    let second = put(&mut app, "tesla_reactor", 3, 8);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let mut sold = None;
    let mut charged = false;
    let mut spawned = std::collections::BTreeSet::new();
    let mut shot = false;
    for _ in 0..6000 {
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q)
            .any(|q| matches!(q.item, sim::ProductionItem::Unit(_)) && q.charged > 0);
        for event in step(&mut app) {
            match event {
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    refund,
                } => {
                    assert!(
                        [first, second].contains(&building),
                        "sell a genuinely redundant source power member before the real construction yard"
                    );
                    assert!(sold.replace((building, refund)).is_none());
                }
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    spawned.insert(unit);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if spawned.contains(&attacker) && found == target => shot = true,
                _ => {}
            }
        }
        if shot {
            break;
        }
    }
    eprintln!("redundant source power sold{sold:?} charged{charged} spawned{spawned:?} shot{shot}");
    assert!(sold.is_some() && charged && !spawned.is_empty() && shot);
    let state = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap();
    assert!(!state.low_power());
    let world = app.world_mut();
    let kinds = world
        .query::<(&sim::Owner, &sim::Building)>()
        .iter(world)
        .filter(|(o, _)| o.0 == PlayerId(0))
        .map(|(_, b)| b.kind)
        .collect::<Vec<_>>();
    let defs = world.resource::<sim::Definitions>();
    assert!(kinds.contains(&defs.building_kind("soviet_construction_yard").unwrap()));
    assert!(kinds.contains(&defs.building_kind("tesla_reactor").unwrap()));
    assert!(
        sim::check_buildable(
            defs,
            world.resource::<sim::Players>().get(PlayerId(0)).unwrap(),
            false,
            &kinds,
            sim::ProductionItem::Unit(defs.unit_kind("conscript").unwrap())
        )
        .is_ok()
    );
    let entity = unit_entity(&mut app, miner);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
}

fn paid_worker_team_fixture(
    legacy_ai: bool,
    fog: bool,
) -> (App, sim::SimId, sim::SimId, sim::SimId) {
    let mut app = scenario_mode(
        "russia",
        "medium",
        400,
        0,
        false,
        ScenarioMode {
            legacy_ai,
            fog,
            ..Default::default()
        },
    );
    let controllers = std::mem::take(
        app.world_mut()
            .resource_mut::<sim::AiPlayers>()
            .into_inner(),
    );
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    put(&mut app, "tesla_reactor", 3, 5);
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "war_miner",
        CellPos::new(21, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(25, 13),
    )
    .unwrap();
    let item = sim::ProductionItem::Unit(
        app.world()
            .resource::<sim::Definitions>()
            .unit_kind("conscript")
            .unwrap(),
    );
    assert_eq!(
        app.world()
            .resource::<sim::Definitions>()
            .production(item)
            .unwrap()
            .cost,
        100
    );
    issue(&mut app, Command::QueueProduction { item });
    let mut recruit = None;
    let mut charged = false;
    for _ in 0..300 {
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q)
            .any(|q| q.item == item && q.charged > 0);
        for event in step(&mut app) {
            if let sim::SimEvent::UnitSpawned {
                player: PlayerId(0),
                building,
                unit,
            } = event
                && building == barracks
            {
                recruit = Some(unit);
            }
        }
        if recruit.is_some() {
            break;
        }
    }
    assert!(charged && recruit.is_some());
    app.world_mut().insert_resource(controllers);
    (app, miner, target, recruit.unwrap())
}

#[test]
fn ra8_zero_job_armed_workers_join_real_paid_infantry_through_ordinary_team_readiness() {
    let (mut app, miner, target, recruit) = paid_worker_team_fixture(false, false);
    let first_wave = u64::from(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .first_wave,
    );
    let mut infantry_shot = false;
    let mut worker_wave_shot = false;
    let mut worker_joined = false;
    for _ in 0..6000 {
        for event in step(&mut app) {
            assert!(!matches!(
                event,
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                }
            ));
            if let sim::SimEvent::WeaponFired {
                attacker,
                target: found,
                ..
            } = event
                && found == target
            {
                infantry_shot |= attacker == recruit;
                worker_wave_shot |= attacker == miner
                    && app
                        .world()
                        .resource::<sim::AiPlayers>()
                        .controller(PlayerId(0))
                        .unwrap()
                        .wave()
                        .contains(&miner);
            }
        }
        worker_joined |= app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&miner);
        if app.world().resource::<SimTick>().0 <= first_wave {
            assert!(
                !worker_joined,
                "ordinary source first-wave readiness still gates admission"
            );
        }
        if infantry_shot && worker_wave_shot {
            break;
        }
    }
    eprintln!(
        "zero-job combined arms paid{recruit:?} infantry_shot{infantry_shot} worker_joined{worker_joined} worker_wave_shot{worker_wave_shot}"
    );
    assert!(
        infantry_shot,
        "the ordinary paid infantry must really approach and fire"
    );
    assert!(
        worker_joined && worker_wave_shot,
        "a worker with no actual or known economic job must contribute to the ordinary ready infantry wave"
    );
    let entity = unit_entity(&mut app, miner);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
}

#[test]
fn ra8_legacy_paid_teams_keep_the_original_worker_admission_policy() {
    let (mut app, miner, target, recruit) = paid_worker_team_fixture(true, false);
    let mut infantry_shot = false;
    for _ in 0..6000 {
        for event in step(&mut app) {
            infantry_shot |= matches!(event, sim::SimEvent::WeaponFired { attacker, target: found, .. } if attacker == recruit && found == target);
        }
        assert!(
            !app.world()
                .resource::<sim::AiPlayers>()
                .controller(PlayerId(0))
                .unwrap()
                .wave()
                .contains(&miner),
            "the neutral plan must retain its original worker roles alongside a real paid army"
        );
    }
    assert!(
        infantry_shot,
        "the legacy paid infantry must actually fight"
    );
}

#[test]
fn ra8_unseen_ore_does_not_change_real_worker_team_admission_or_intents() {
    let (mut actual, miner, target, recruit) = paid_worker_team_fixture(false, true);
    let (mut control, other_miner, other_target, other_recruit) =
        paid_worker_team_fixture(false, true);
    assert_eq!(
        (miner, target, recruit),
        (other_miner, other_target, other_recruit)
    );
    let hidden = CellPos::new(44, 2);
    let mut cell = *actual.world().resource::<Grid>().cell(hidden).unwrap();
    cell.ore = 5000;
    actual
        .world_mut()
        .resource_mut::<Grid>()
        .set_terrain(hidden, cell);
    let mut joined = false;
    let mut worker_shot = false;
    let mut infantry_shot = false;
    for _ in 0..6000 {
        let a = step(&mut actual);
        let b = step(&mut control);
        assert_eq!(
            a, b,
            "unseen ore must not alter native events before observation"
        );
        for app in [&actual, &control] {
            assert!(
                !app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .vision()
                    .is_visible(hidden),
                "the ore twin's premise must remain genuinely unseen"
            );
        }
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
        let wave = actual
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave();
        joined |= wave.contains(&miner);
        for event in a {
            if let sim::SimEvent::WeaponFired {
                attacker,
                target: found,
                ..
            } = event
                && found == target
            {
                worker_shot |= attacker == miner && wave.contains(&miner);
                infantry_shot |= attacker == recruit;
            }
        }
        if worker_shot && infantry_shot {
            break;
        }
    }
    assert!(
        joined && worker_shot && infantry_shot,
        "both genuinely independent hidden-ore twins must reach real paid-infantry and worker wave fire"
    );
}

#[test]
fn ra8_surface_counterfire_prioritizes_a_real_defender_over_a_nearer_unarmed_miner() {
    let (mut app, building, gunner) = surface_building_focus_fixture_for("flak_track");
    let entity = unit_entity(&mut app, gunner);
    let at = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let miner = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "chrono_miner",
        CellPos::new(at.x, at.y + 1),
    )
    .unwrap();
    let defender = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "gi",
        CellPos::new(at.x + 3, at.y + 1),
    )
    .unwrap();
    let defender_entity = unit_entity(&mut app, defender);
    let defender_initial = app
        .world()
        .get::<sim::Health>(defender_entity)
        .unwrap()
        .hit_points;
    let definitions = app.world().resource::<sim::Definitions>();
    let source = definitions
        .unit_combat(definitions.unit_kind("flak_track").unwrap())
        .unwrap();
    let victim = definitions
        .unit_combat(definitions.unit_kind("gi").unwrap())
        .unwrap();
    let traits = sim::TargetTraits {
        armour: victim.armour,
        organic: victim.organic,
        ..Default::default()
    };
    let (gun, versus) =
        sim::weapon_against(definitions, &source, source.weapons(false), &traits).unwrap();
    let gun = definitions.weapon(gun).unwrap();
    let threshold = load_ai(data().join("ai.ron"), definitions)
        .unwrap()
        .policy
        .artillery_min_range
        * sim::SUBCELLS_PER_CELL as u32;
    assert!(versus > 1 && gun.anti_ground && gun.damage > 0 && gun.range < threshold);
    assert!(
        source
            .weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|w| definitions.weapon(w))
            .any(|w| w.range >= threshold && w.anti_air && !w.anti_ground),
        "the longer AA source weapon must not misclassify the selected normal ground gun"
    );
    let preserve_ticks = 2 * u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let initial = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    issue(
        &mut app,
        Command::AttackMove {
            units: vec![gunner],
            target: CellPos::new(at.x, at.y + 4),
        },
    );
    let mut minimum_hp = initial;
    let mut incoming = false;
    let mut acquired = false;
    for _ in 0..120 {
        let events = step(&mut app);
        if let Some(hp) = app.world().get::<sim::Health>(entity) {
            minimum_hp = minimum_hp.min(hp.hit_points);
        }
        incoming |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == defender && *target == gunner));
        let weapon = app.world().get::<sim::Weapon>(entity).unwrap();
        acquired |= weapon.target == Some(miner)
            && matches!(
                weapon.order,
                Some(sim::AttackOrder::AttackMove { moving: false, .. })
            );
        if incoming && acquired {
            break;
        }
    }
    assert!(
        incoming && acquired,
        "actual stationary AttackMove must acquire the unarmed miner while an observed armed defender fires"
    );
    app.world_mut().insert_resource(controllers);
    let mut returned = false;
    let mut resumed = false;
    let mut defender_damaged = false;
    let mut defender_destroyed = false;
    let mut preserved_ticks = 0;
    let mut distraction_shots = 0;
    for _ in 0..1800 {
        let events = step(&mut app);
        if let Some(hp) = app.world().get::<sim::Health>(entity) {
            minimum_hp = minimum_hp.min(hp.hit_points);
        }
        let defender_alive = app
            .world()
            .get::<sim::Health>(defender_entity)
            .is_some_and(|h| {
                defender_damaged |= h.hit_points < defender_initial;
                h.hit_points > 0
            });
        if returned && defender_alive {
            assert!(
                matches!(app.world().get::<sim::Weapon>(entity).unwrap().order, Some(sim::AttackOrder::Attack { target, .. }) if target == defender),
                "a matching healthy counter-Attack must retain its native history"
            );
            assert!(!app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::Attack { units, .. } | Command::AttackMove { units, .. } if units.contains(&gunner))), "do not repeatedly reset the matching counter-Attack");
            preserved_ticks += 1;
        }
        for event in events {
            defender_destroyed |=
                matches!(event, sim::SimEvent::UnitDestroyed { unit, .. } if unit == defender);
            incoming |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == defender && target == gunner);
            returned |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == gunner && target == defender);
            distraction_shots += u32::from(
                matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == gunner && target == miner),
            );
            resumed |= returned
                && defender_destroyed
                && matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == gunner && target == building);
        }
        if resumed {
            break;
        }
        assert!(
            app.world()
                .resource::<sim::MatchState>()
                .outcome()
                .is_none()
        );
    }
    let final_hp = app.world().get::<sim::Health>(entity).map(|h| h.hit_points);
    assert!(
        incoming && minimum_hp < initial,
        "the actual source defender must fire and damage the focused gunner; incoming{incoming} initial{initial} minimum{minimum_hp} final{final_hp:?} returned{returned} resumed{resumed} distractions{distraction_shots}"
    );
    assert!(
        returned,
        "counterfire must hit the armed source defender instead of only a nearer unarmed miner; distraction_shots{distraction_shots}"
    );
    assert!(
        defender_damaged && defender_destroyed,
        "the source gun must actually damage and destroy the defender, not only emit WeaponFired"
    );
    assert!(
        preserved_ticks >= preserve_ticks,
        "the healthy counter-Attack must survive multiple actual AI decisions"
    );
    assert!(
        resumed,
        "after actual counterfire the natural building mission must resume native fire"
    );
}

#[test]
fn ra8_distant_unrelated_unit_chase_is_not_a_current_counterfire_job() {
    let (mut app, building, gunner) = surface_building_focus_fixture();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    assert!(
        matches!(controllers.controller(PlayerId(0)).unwrap().target(), Some(sim::WaveTarget::Building(id)) if id == building)
    );
    let interval = u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let distant = spawn_unit(app.world_mut(), PlayerId(1), "gi", CellPos::new(36, 14)).unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(34, 16),
    )
    .unwrap();
    step(&mut app);
    let distant_entity = unit_entity(&mut app, distant);
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .sees_unit(app.world().get::<sim::Mobile>(distant_entity).unwrap())
    );
    issue(
        &mut app,
        Command::Attack {
            units: vec![gunner],
            target: distant,
        },
    );
    step(&mut app);
    step(&mut app);
    let gunner_entity = unit_entity(&mut app, gunner);
    assert!(
        matches!(app.world().get::<sim::Weapon>(gunner_entity).unwrap().order, Some(sim::AttackOrder::Attack { target, .. }) if target == distant)
    );
    assert!(
        app.world()
            .get::<sim::Orders>(gunner_entity)
            .unwrap()
            .current
            .is_some()
    );
    let from = app.world().get::<sim::Mobile>(gunner_entity).unwrap().cell;
    let definitions = app.world().resource::<sim::Definitions>();
    let gi = definitions
        .unit_combat(definitions.unit_kind("gi").unwrap())
        .unwrap();
    let range = definitions.weapon(gi.weapon.unwrap()).unwrap().range;
    let dx = i64::from(from.x - 36) * i64::from(sim::SUBCELLS_PER_CELL);
    let dy = i64::from(from.y - 14) * i64::from(sim::SUBCELLS_PER_CELL);
    assert!(
        !sim::in_range(dx, dy, range),
        "the visible unrelated unit is outside its actual incoming source range"
    );
    app.world_mut().insert_resource(controllers);
    let mut issued = false;
    let mut resumed = false;
    let mut incoming = false;
    for _ in 0..2 * interval + 2 {
        let events = step(&mut app);
        incoming |= events.iter().any(|e| matches!(e, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == distant && *target == gunner));
        issued |= app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::Attack { units, target } if units.contains(&gunner) && *target == building));
        resumed |= matches!(app.world().get::<sim::Weapon>(gunner_entity).unwrap().order, Some(sim::AttackOrder::Attack { target, .. }) if target == building);
    }
    assert!(!incoming);
    assert!(
        issued && resumed,
        "a distant unrelated chase must not steal counterfire ownership from the actual building mission"
    );
    let mut fired = false;
    for _ in 0..500 {
        let events = step(&mut app);
        fired |= events.iter().any(|e| matches!(e, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == gunner && *target == building));
        assert!(
            app.world()
                .get::<sim::Health>(distant_entity)
                .is_some_and(|hp| hp.hit_points > 0),
            "the mission must resume before the unrelated unit dies"
        );
        if fired {
            break;
        }
    }
    assert!(
        fired,
        "the restored real building mission must reach native fire"
    );
}

#[test]
fn ra8_an_understrength_living_field_force_can_finance_a_paid_reinforcement() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    put(&mut app, "tesla_reactor", 3, 5);
    let asset = put(&mut app, "soviet_service_depot", 18, 5);
    let fighter = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let defs = app.world().resource::<sim::Definitions>();
    let kind = defs.unit_kind("conscript").unwrap();
    let item = sim::ProductionItem::Unit(kind);
    let cost = defs.production(item).unwrap().cost;
    let refund = defs.refund(defs.building_kind("soviet_service_depot").unwrap());
    assert!(refund >= cost && cost > 0);
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .wave_size
            > 1
    );
    let mut sold = None;
    let mut paid = false;
    let mut charged = false;
    let mut recruited = std::collections::BTreeSet::new();
    let mut recruit_shot = false;
    let mut fighter_shot = false;
    for _ in 0..6000 {
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q)
            .any(|q| q.item == item && q.charged > 0);
        for event in step(&mut app) {
            match event {
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    refund: amount,
                } => {
                    assert_eq!(building, asset, "retain all ordinary producers");
                    assert_eq!(amount, refund, "native exact refund only");
                    assert!(sold.replace(amount).is_none(), "one optional sale suffices");
                }
                sim::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: found,
                } if found == item => paid = true,
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    recruited.insert(unit);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if found == target => {
                    fighter_shot |= attacker == fighter;
                    recruit_shot |= recruited.contains(&attacker);
                }
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                } => panic!("declared empty map must have no income"),
                _ => {}
            }
        }
        if recruit_shot && fighter_shot {
            break;
        }
    }
    let entity = unit_entity(&mut app, fighter);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    eprintln!(
        "understrength field reinforcement sold{sold:?} exact_refund{refund} source_cost{cost} paid{paid} recruits{recruited:?} fighter_shot{fighter_shot} recruit_shot{recruit_shot}"
    );
    assert!(
        fighter_shot,
        "the original living ordinary field infantry must actually fight before judging financing"
    );
    assert_eq!(
        sold,
        Some(refund),
        "living understrength field force must not exclude financing ordinary reinforcement from an optional asset"
    );
    assert!(
        charged && paid && !recruited.is_empty() && recruit_shot && fighter_shot,
        "real paid recruit and the retained living field force must both make native building shots"
    );
}

#[test]
fn ra8_understrength_financing_preserves_legacy_and_real_service_assets() {
    // A native paid docking job is still useful after its cash is exhausted.
    let (mut repair, tank, repaired) = paid_then_unfunded_service_fixture();
    let depot = repair.world().resource::<sim::SpecialState>().docked[&tank];
    let entity = unit_entity(&mut repair, tank);
    let maximum = repair
        .world()
        .resource::<sim::Definitions>()
        .unit_combat(repair.world().get::<sim::Unit>(entity).unwrap().kind)
        .unwrap()
        .hit_points;
    assert!(repaired < maximum);
    put(&mut repair, "barracks", 5, 12);
    let mut released = false;
    for _ in 0..2000 {
        for event in step(&mut repair) {
            if matches!(event, sim::SimEvent::BuildingSold { building, .. } if building == depot) {
                assert_eq!(
                    repair
                        .world()
                        .get::<sim::Health>(entity)
                        .unwrap()
                        .hit_points,
                    maximum,
                    "a native service asset must remain until the actual damaged vehicle is fully repaired"
                );
            }
        }
        released |= !repair
            .world()
            .resource::<sim::SpecialState>()
            .docked
            .contains_key(&tank);
    }
    assert!(
        released
            && repair
                .world()
                .get::<sim::Health>(entity)
                .unwrap()
                .hit_points
                >= repaired
    );

    // Captured foreign docks can support an owned aircraft absent from the
    // declared Soviet army; a new recruit must not consume its reload facility.
    let mut air = scenario("russia", "medium", 0, 0, false);
    let controllers = std::mem::take(&mut *air.world_mut().resource_mut::<sim::AiPlayers>());
    put(&mut air, "tesla_reactor", 4, 4);
    put(&mut air, "soviet_barracks", 5, 12);
    let dock = put(&mut air, "airforce_command", 16, 8);
    let target = sim::spawn_building(
        air.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(34, 16),
    )
    .unwrap();
    let target_entity = unit_entity(&mut air, target);
    // Declared durable combat fixture, as in the retained reload control.
    air.world_mut()
        .get_mut::<sim::Health>(target_entity)
        .unwrap()
        .hit_points = 1_000_000;
    spawn_unit(air.world_mut(), PlayerId(0), "spy", CellPos::new(32, 17)).unwrap();
    spawn_unit(
        air.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 14),
    )
    .unwrap();
    let plane = spawn_unit(
        air.world_mut(),
        PlayerId(0),
        "harrier",
        CellPos::new(10, 14),
    )
    .unwrap();
    step(&mut air);
    air.world_mut().insert_resource(controllers);
    let mut shots = 0;
    let mut empty = false;
    let mut reloading = false;
    let burst = {
        let entity = unit_entity(&mut air, plane);
        air.world()
            .resource::<sim::Definitions>()
            .weapon(air.world().get::<sim::Weapon>(entity).unwrap().kind)
            .unwrap()
            .burst
    };
    for _ in 0..14000 {
        for event in step(&mut air) {
            assert!(
                !matches!(event, sim::SimEvent::BuildingSold { building, .. } if building == dock),
                "captured native reload capability must remain available to its real aircraft"
            );
            shots += u32::from(
                matches!(event, sim::SimEvent::WeaponFired {attacker,..} if attacker == plane),
            );
        }
        let special = air.world().resource::<sim::SpecialState>();
        empty |= special.ammo.get(&plane) == Some(&0);
        reloading |= special.reloading.get(&plane).is_some_and(|wait| *wait > 20);
        if reloading && shots > burst {
            break;
        }
    }
    assert!(
        empty && reloading && shots > burst,
        "real captured aircraft must fire, return, reload and fire again"
    );

    let mut legacy = scenario_mode(
        "russia",
        "brutal",
        0,
        0,
        false,
        ScenarioMode {
            legacy_ai: true,
            ..Default::default()
        },
    );
    put(&mut legacy, "tesla_reactor", 3, 5);
    put(&mut legacy, "soviet_barracks", 5, 12);
    let asset = put(&mut legacy, "soviet_service_depot", 18, 5);
    let fighter = spawn_unit(
        legacy.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        legacy.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let mut fired = false;
    for _ in 0..2000 {
        for event in step(&mut legacy) {
            assert!(
                !matches!(event, sim::SimEvent::BuildingSold { building, .. } if building == asset),
                "legacy presence-based financing remains unchanged"
            );
            fired |= matches!(event, sim::SimEvent::WeaponFired { attacker,target:found,..} if attacker == fighter && found == target);
        }
    }
    assert!(fired);
}

#[test]
fn ra8_surface_counterfire_leaves_an_unarmed_economy_building_to_fight_its_real_defender() {
    let (mut app, building, gunner) = surface_building_focus_fixture_for("flak_track");
    let entity = unit_entity(&mut app, gunner);
    let at = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let distraction = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_barracks",
        CellPos::new(at.x, at.y + 1),
    )
    .unwrap();
    let distraction_entity = unit_entity(&mut app, distraction);
    let kind = app
        .world()
        .get::<sim::Building>(distraction_entity)
        .unwrap()
        .kind;
    assert!(
        app.world()
            .resource::<sim::Definitions>()
            .building_combat(kind)
            .is_none_or(|terms| terms.weapons(false).into_iter().all(|w| w.is_none()))
    );
    let defender = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "gi",
        CellPos::new(at.x + 3, at.y + 1),
    )
    .unwrap();
    let defender_entity = unit_entity(&mut app, defender);
    let defender_initial = app
        .world()
        .get::<sim::Health>(defender_entity)
        .unwrap()
        .hit_points;
    let definitions = app.world().resource::<sim::Definitions>();
    let source = definitions
        .unit_combat(definitions.unit_kind("flak_track").unwrap())
        .unwrap();
    let victim = definitions
        .unit_combat(definitions.unit_kind("gi").unwrap())
        .unwrap();
    let traits = sim::TargetTraits {
        armour: victim.armour,
        organic: victim.organic,
        ..Default::default()
    };
    let (gun, versus) =
        sim::weapon_against(definitions, &source, source.weapons(false), &traits).unwrap();
    let gun = definitions.weapon(gun).unwrap();
    let threshold = load_ai(data().join("ai.ron"), definitions)
        .unwrap()
        .policy
        .artillery_min_range
        * sim::SUBCELLS_PER_CELL as u32;
    assert!(versus > 1 && gun.anti_ground && gun.damage > 0 && gun.range < threshold);
    assert!(
        source
            .weapons(false)
            .into_iter()
            .flatten()
            .filter_map(|w| definitions.weapon(w))
            .any(|w| w.range >= threshold && w.anti_air && !w.anti_ground),
        "the longer AA source weapon must not misclassify the selected normal ground gun"
    );
    let preserve_ticks = 2 * u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let initial = app.world().get::<sim::Health>(entity).unwrap().hit_points;
    issue(
        &mut app,
        Command::AttackMove {
            units: vec![gunner],
            target: CellPos::new(at.x, at.y + 4),
        },
    );
    let mut minimum_hp = initial;
    let mut incoming = false;
    let mut acquired = false;
    for _ in 0..120 {
        let events = step(&mut app);
        if let Some(hp) = app.world().get::<sim::Health>(entity) {
            minimum_hp = minimum_hp.min(hp.hit_points);
        }
        incoming |= events.iter().any(|event| matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if *attacker == defender && *target == gunner));
        let weapon = app.world().get::<sim::Weapon>(entity).unwrap();
        acquired |= weapon.target == Some(distraction)
            && matches!(
                weapon.order,
                Some(sim::AttackOrder::AttackMove { moving: false, .. })
            );
        if incoming && acquired {
            break;
        }
    }
    assert!(
        incoming && acquired,
        "actual stationary AttackMove must acquire the unarmed economy building while an observed armed defender fires"
    );
    app.world_mut().insert_resource(controllers);
    let mut returned = false;
    let mut resumed = false;
    let mut defender_damaged = false;
    let mut defender_destroyed = false;
    let mut preserved_ticks = 0;
    let mut distraction_shots = 0;
    for _ in 0..1800 {
        let events = step(&mut app);
        if let Some(hp) = app.world().get::<sim::Health>(entity) {
            minimum_hp = minimum_hp.min(hp.hit_points);
        }
        let defender_alive = app
            .world()
            .get::<sim::Health>(defender_entity)
            .is_some_and(|h| {
                defender_damaged |= h.hit_points < defender_initial;
                h.hit_points > 0
            });
        if returned && defender_alive {
            assert!(
                matches!(app.world().get::<sim::Weapon>(entity).unwrap().order, Some(sim::AttackOrder::Attack { target, .. }) if target == defender),
                "a matching healthy counter-Attack must retain its native history"
            );
            assert!(!app.world().resource::<CommandQueue>().iter().any(|c| matches!(&c.command, Command::Attack { units, .. } | Command::AttackMove { units, .. } if units.contains(&gunner))), "do not repeatedly reset the matching counter-Attack");
            preserved_ticks += 1;
        }
        for event in events {
            defender_destroyed |=
                matches!(event, sim::SimEvent::UnitDestroyed { unit, .. } if unit == defender);
            incoming |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == defender && target == gunner);
            returned |= matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == gunner && target == defender);
            distraction_shots += u32::from(
                matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. } if attacker == gunner && target == distraction),
            );
            resumed |= returned
                && defender_destroyed
                && matches!(event, sim::SimEvent::WeaponFired { attacker, target, .. }
                    if attacker == gunner && [building, distraction].contains(&target)
                    && app.world().resource::<sim::AiPlayers>().controller(PlayerId(0)).unwrap().target() == Some(sim::WaveTarget::Building(target)));
        }
        if resumed {
            break;
        }
        assert!(
            app.world()
                .resource::<sim::MatchState>()
                .outcome()
                .is_none()
        );
    }
    let final_hp = app.world().get::<sim::Health>(entity).map(|h| h.hit_points);
    eprintln!(
        "building counterfire returned{returned} damaged{defender_damaged} destroyed{defender_destroyed} resumed{resumed} distractionshots{distraction_shots} target{:?} order{:?}",
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .target(),
        app.world().get::<sim::Weapon>(entity)
    );
    assert!(
        incoming && minimum_hp < initial,
        "the actual source defender must fire and damage the focused gunner; incoming{incoming} initial{initial} minimum{minimum_hp} final{final_hp:?} returned{returned} resumed{resumed} distractions{distraction_shots}"
    );
    assert!(
        returned,
        "counterfire must hit the armed source defender instead of only a nearer unarmed economy building; distraction_shots{distraction_shots}"
    );
    assert!(
        defender_damaged && defender_destroyed,
        "the source gun must actually damage and destroy the defender, not only emit WeaponFired"
    );
    assert!(
        preserved_ticks >= preserve_ticks,
        "the healthy counter-Attack must survive multiple actual AI decisions"
    );
    assert!(
        resumed,
        "after actual defender destruction the naturally selected observed building mission must receive native fire"
    );
}

#[test]
fn ra8_surface_counterfire_preserves_a_real_armed_building_engagement() {
    let (mut app, _, gunner) = surface_building_focus_fixture_for("flak_track");
    let entity = unit_entity(&mut app, gunner);
    let at = app.world().get::<sim::Mobile>(entity).unwrap().cell;
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let defended = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "pillbox",
        CellPos::new(at.x, at.y + 1),
    )
    .unwrap();
    let defender = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "gi",
        CellPos::new(at.x + 3, at.y + 1),
    )
    .unwrap();
    let interval = controllers
        .controller(PlayerId(0))
        .unwrap()
        .difficulty()
        .think_interval;
    let terms = app
        .world()
        .resource::<sim::Definitions>()
        .building_combat(
            app.world()
                .resource::<sim::Definitions>()
                .building_kind("pillbox")
                .unwrap(),
        )
        .unwrap();
    assert!(
        terms
            .weapons(false)
            .into_iter()
            .any(|weapon| weapon.is_some())
    );
    issue(
        &mut app,
        Command::AttackMove {
            units: vec![gunner],
            target: CellPos::new(at.x, at.y + 4),
        },
    );
    let mut acquired = false;
    let mut building_incoming = false;
    let mut infantry_incoming = false;
    for _ in 0..120 {
        for event in step(&mut app) {
            building_incoming |= matches!(event, sim::SimEvent::WeaponFired {attacker,target,..} if attacker == defended && target == gunner);
            infantry_incoming |= matches!(event, sim::SimEvent::WeaponFired {attacker,target,..} if attacker == defender && target == gunner);
        }
        let weapon = app.world().get::<sim::Weapon>(entity).unwrap();
        acquired |= weapon.target == Some(defended)
            && matches!(
                weapon.order,
                Some(sim::AttackOrder::AttackMove { moving: false, .. })
            );
        if acquired && building_incoming && infantry_incoming {
            break;
        }
    }
    assert!(
        acquired && building_incoming && infantry_incoming,
        "actual armed-building engagement and both incoming sources must precede policy observation"
    );
    // Two AI intervals can be shorter than the native gun's remaining reload.
    let observation = u64::from(app.world().get::<sim::Weapon>(entity).unwrap().cooldown)
        + 2 * u64::from(interval)
        + 1;
    app.world_mut().insert_resource(controllers);
    let mut building_fire = false;
    for _ in 0..observation {
        for event in step(&mut app) {
            assert!(
                !matches!(event, sim::SimEvent::WeaponFired {attacker,target,..} if attacker == gunner && target == defender),
                "an armed building is not an unarmed economic distraction"
            );
            building_fire |= matches!(event, sim::SimEvent::WeaponFired {attacker,target,..} if attacker == gunner && target == defended);
        }
        assert!(!app.world().resource::<CommandQueue>().iter().any(|issued| matches!(&issued.command, Command::Attack {units,target} if units.contains(&gunner) && *target == defender)));
    }
    assert!(
        building_fire,
        "the preserved engagement must produce real native armed-building fire"
    );
}

#[test]
fn ra8_an_understrength_force_can_consolidate_a_producer_for_real_paid_infantry() {
    let mut app = scenario("russia", "brutal", 0, 0, false);
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    // No optional Reactor refund: this controlled ordinary infantry producer
    // remains source-buildable without a powered-factory prerequisite.
    let asset = put(&mut app, "soviet_war_factory", 18, 5);
    let fighter = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let defs = app.world().resource::<sim::Definitions>();
    let kind = defs.unit_kind("conscript").unwrap();
    let item = sim::ProductionItem::Unit(kind);
    let cost = defs.production(item).unwrap().cost;
    let refund = defs.refund(defs.building_kind("soviet_war_factory").unwrap());
    assert!(refund >= cost && cost > 0);
    assert!(
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .wave_size
            > 1
    );
    let remaining = {
        let world = app.world_mut();
        world
            .query::<(&sim::SimId, &sim::Owner, &sim::Building)>()
            .iter(world)
            .filter(|(id, owner, _)| owner.0 == PlayerId(0) && **id != asset)
            .map(|(_, _, building)| building.kind)
            .collect::<Vec<_>>()
    };
    let definitions = app.world().resource::<sim::Definitions>();
    let state = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap();
    assert!(
        sim::check_buildable(definitions, state, false, &remaining, item).is_ok(),
        "ordinary source prerequisites/owner/country permit the replacement after the actual proposed factory loss"
    );
    let rhino = sim::ProductionItem::Unit(definitions.unit_kind("rhino").unwrap());
    assert!(
        sim::check_buildable(definitions, state, false, &remaining, rhino).is_err(),
        "the controlled choice actually gives up heavy production rather than calling every producer optional"
    );
    let mut sold = None;
    let mut paid = false;
    let mut charged = false;
    let mut recruited = std::collections::BTreeSet::new();
    let mut recruit_shot = false;
    let mut fighter_shot = false;
    for _ in 0..6000 {
        charged |= app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q)
            .any(|q| q.item == item && q.charged > 0);
        for event in step(&mut app) {
            match event {
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    refund: amount,
                } => {
                    assert_eq!(
                        building, asset,
                        "sell only the counterfactually redundant producer"
                    );
                    assert_eq!(amount, refund, "native exact refund only");
                    assert!(sold.replace(amount).is_none(), "one optional sale suffices");
                }
                sim::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: found,
                } if found == item => paid = true,
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    recruited.insert(unit);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if found == target => {
                    fighter_shot |= attacker == fighter;
                    recruit_shot |= recruited.contains(&attacker);
                }
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                } => panic!("declared empty map must have no income"),
                _ => {}
            }
        }
        if recruit_shot && fighter_shot {
            break;
        }
    }
    let entity = unit_entity(&mut app, fighter);
    assert!(
        app.world()
            .get::<sim::Health>(entity)
            .is_some_and(|h| h.hit_points > 0)
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .outcome()
            .is_none()
    );
    assert!(
        app.world()
            .resource::<sim::MatchState>()
            .defeated(PlayerId(0))
            .is_none()
    );
    eprintln!(
        "productive consolidation sold{sold:?} exact_refund{refund} source_cost{cost} paid{paid} recruits{recruited:?} fighter_shot{fighter_shot} recruit_shot{recruit_shot}"
    );
    assert!(
        fighter_shot,
        "the original living ordinary field infantry must actually fight before judging financing"
    );
    assert_eq!(
        sold,
        Some(refund),
        "a retained legal infantry producer must permit native consolidation of an otherwise unfundable second producer"
    );
    assert!(
        charged && paid && !recruited.is_empty() && recruit_shot && fighter_shot,
        "real paid recruit and the retained living field force must both make native building shots"
    );
}

#[test]
fn ra8_a_rear_wave_member_assists_its_actually_threatened_front_before_own_contact() {
    let mut app = scenario("russia", "medium", 0, 0, false);
    let building = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "radar_tower",
        CellPos::new(22, 8),
    )
    .unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(23, 11),
    )
    .unwrap();
    let front = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(14, 8),
    )
    .unwrap();
    let rear = [
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "conscript",
            CellPos::new(12, 8),
        )
        .unwrap(),
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "conscript",
            CellPos::new(12, 10),
        )
        .unwrap(),
    ];
    let mut mission_fire = false;
    for _ in 0..7000 {
        mission_fire |= step(&mut app).iter().any(|e| matches!(e,sim::SimEvent::WeaponFired{attacker,target,..} if *attacker==front && *target==building));
        let wave = app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave();
        if mission_fire && [front, rear[0], rear[1]].iter().all(|id| wave.contains(id)) {
            break;
        }
    }
    assert!(mission_fire);
    assert!(
        [front, rear[0], rear[1]].iter().all(|id| app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(id)),
        "all actors must join the same ordinary source-ready wave naturally"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    let front_entity = unit_entity(&mut app, front);
    let at = app.world().get::<sim::Mobile>(front_entity).unwrap().cell;
    issue(&mut app, Command::Stop { units: vec![front] });
    let destinations = [
        CellPos::new(at.x - 6, at.y),
        CellPos::new(at.x - 6, at.y + 2),
    ];
    for (id, destination) in rear.into_iter().zip(destinations) {
        issue(
            &mut app,
            Command::Move {
                units: vec![id],
                target: destination,
            },
        );
    }
    let entities = rear.map(|id| unit_entity(&mut app, id));
    let mut arrived = false;
    for _ in 0..1000 {
        step(&mut app);
        arrived = entities
            .into_iter()
            .zip(destinations)
            .all(|(entity, destination)| {
                app.world()
                    .get::<sim::Mobile>(entity)
                    .is_some_and(|mobile| mobile.cell == destination && mobile.step.is_none())
            });
        if arrived {
            break;
        }
    }
    assert!(
        arrived,
        "rear spacing must come from real public Move completion"
    );
    let defender = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "gi",
        CellPos::new(at.x + 3, at.y + 1),
    )
    .unwrap();
    let defender_entity = unit_entity(&mut app, defender);
    let initial_defender = app
        .world()
        .get::<sim::Health>(defender_entity)
        .unwrap()
        .hit_points;
    issue(
        &mut app,
        Command::Attack {
            units: rear.to_vec(),
            target: building,
        },
    );
    issue(
        &mut app,
        Command::AttackMove {
            units: vec![front],
            target: CellPos::new(at.x, at.y + 3),
        },
    );
    let front_initial = app
        .world()
        .get::<sim::Health>(front_entity)
        .unwrap()
        .hit_points;
    let mut incoming = false;
    for _ in 0..100 {
        incoming |= step(&mut app).iter().any(|e|matches!(e,sim::SimEvent::WeaponFired{attacker,target,..} if *attacker==defender && *target==front));
        if incoming
            && app
                .world()
                .get::<sim::Health>(front_entity)
                .unwrap()
                .hit_points
                < front_initial
        {
            break;
        }
    }
    assert!(
        incoming
            && app
                .world()
                .get::<sim::Health>(front_entity)
                .unwrap()
                .hit_points
                < front_initial,
        "native incoming fire must actually damage the same-wave front"
    );
    let definitions = app.world().resource::<sim::Definitions>();
    let terms = definitions
        .unit_combat(definitions.unit_kind("gi").unwrap())
        .unwrap();
    let victim = definitions
        .unit_combat(definitions.unit_kind("conscript").unwrap())
        .unwrap();
    let traits = sim::TargetTraits {
        armour: victim.armour,
        organic: victim.organic,
        ..Default::default()
    };
    let (weapon, percent) =
        sim::weapon_against(definitions, &terms, terms.weapons(false), &traits).unwrap();
    let range = definitions.weapon(weapon).unwrap().range;
    assert!(percent > 1);
    let outside_incoming = |app: &App, entity| {
        let world = app.world();
        let grid = world.resource::<Grid>();
        let (Some(own), Some(enemy), Some(id)) = (
            world.get::<sim::Mobile>(entity),
            world.get::<sim::Mobile>(defender_entity),
            world.get::<sim::SimId>(entity),
        ) else {
            return false;
        };
        let a = own.position(grid, *id);
        let b = enemy.position(grid, defender);
        let dx = i64::from(a.x) - i64::from(b.x);
        let dy = i64::from(a.y) - i64::from(b.y);
        dx * dx + dy * dy > i64::from(range) * i64::from(range)
    };
    assert!(
        entities
            .iter()
            .all(|entity| outside_incoming(&app, *entity)),
        "the controlled rear is genuinely outside the source incoming range at restoration"
    );
    app.world_mut().insert_resource(controllers);
    let mut assisted_before_contact = false;
    let mut rear_fire = false;
    let mut first_rear_fire = None;
    let mut damaged = false;
    let mut destroyed = false;
    let mut resumed = false;
    for _ in 0..2000 {
        for (id, entity) in rear.iter().zip(entities) {
            assisted_before_contact |= outside_incoming(&app, entity)
                && app
                    .world()
                    .get::<sim::Mobile>(entity)
                    .unwrap()
                    .step
                    .is_some()
                && matches!(app.world().get::<sim::Weapon>(entity).unwrap().order,
                    Some(sim::AttackOrder::Attack {target,..}) if target == defender)
                && app
                    .world()
                    .resource::<sim::AiPlayers>()
                    .controller(PlayerId(0))
                    .unwrap()
                    .wave()
                    .contains(id);
        }
        let events = step(&mut app);
        damaged |= app
            .world()
            .get::<sim::Health>(defender_entity)
            .is_some_and(|hp| hp.hit_points < initial_defender);
        for event in events {
            rear_fire |= matches!(event,sim::SimEvent::WeaponFired{attacker,target,..} if rear.contains(&attacker) && target==defender);
            if rear_fire && first_rear_fire.is_none() {
                first_rear_fire = Some((
                    app.world().resource::<SimTick>().0,
                    app.world()
                        .get::<sim::Health>(front_entity)
                        .map(|hp| hp.hit_points),
                ));
            }
            destroyed |= matches!(event,sim::SimEvent::UnitDestroyed{unit,..} if unit==defender);
            resumed |= destroyed
                && matches!(event,sim::SimEvent::WeaponFired{attacker,target,..} if rear.contains(&attacker) && target==building);
        }
        if resumed {
            break;
        }
    }
    eprintln!(
        "cohort assistance early{assisted_before_contact} rear_fire{rear_fire} damaged{damaged} destroyed{destroyed} resumed{resumed} first_rear{first_rear_fire:?}"
    );
    assert!(
        assisted_before_contact,
        "a capable rear wave member must coordinate against the observed source actually harming its front before being attacked itself"
    );
    assert!(
        rear_fire && damaged && destroyed && resumed,
        "the actual assistance must hurt/kill the defender and resume the original real building mission"
    );
}

#[test]
fn ra8_understrength_financing_keeps_a_ground_support_sources_real_power_path() {
    let mut app = scenario("germany", "brutal", 0, 0, true);
    let reactor = put(&mut app, "nuclear_reactor", 2, 2);
    put(&mut app, "barracks", 5, 12);
    let source = put(&mut app, "chronosphere", 2, 18);
    let controllers = app.world().resource::<sim::AiPlayers>().clone();
    let think = u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    let cooldown = {
        let defs = app.world().resource::<sim::Definitions>();
        defs.metadata(sim::ProductionItem::Building(
            defs.building_kind("chronosphere").unwrap(),
        ))
        .unwrap()
        .cooldown
    };
    app.world_mut().insert_resource(sim::AiPlayers::default());
    let threshold = cooldown.saturating_sub((think * 3) as u32);
    for _ in 0..u64::from(cooldown) + think {
        if app
            .world()
            .resource::<sim::SpecialState>()
            .charge
            .get(&source)
            .copied()
            .unwrap_or(0)
            >= threshold
        {
            break;
        }
        step(&mut app);
    }
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .charge
            .get(&source)
            .copied()
            .unwrap_or(0)
            >= threshold,
        "native source charge must reach the actual near-ready premise within its source-derived bound"
    );
    let tank = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "grizzly",
        CellPos::new(12, 13),
    )
    .unwrap();
    spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "attack_dog",
        CellPos::new(41, 39),
    )
    .unwrap();
    step(&mut app); // Actual observation; no remembered-target/controller seeding.
    let world = app.world_mut();
    let enemy_base = world
        .query::<(&sim::Owner, &sim::Building)>()
        .iter(world)
        .find(|(owner, _)| owner.0 == PlayerId(1))
        .unwrap()
        .1
        .footprint;
    assert!(
        world
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .sees_footprint(enemy_base),
        "the untouched controller must receive an actually observed legal support objective"
    );
    let offset = controllers
        .controller(PlayerId(0))
        .unwrap()
        .plan()
        .policy
        .chrono_min_offset as i32;
    let landing = CellPos::new(enemy_base.origin.x + offset, enemy_base.origin.y + offset);
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(landing),
        "the actual source-policy landing cell must be revealed too, not just the target footprint"
    );
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power()
    );
    assert_eq!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        0
    );
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .all(|(_, q)| q.is_empty())
    );
    let world = app.world_mut();
    assert!(
        world
            .query::<(&sim::Owner, &sim::Unit)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == PlayerId(0))
            .all(
                |(_, unit)| world.resource::<sim::Definitions>().domain(unit.kind)
                    != sim::Domain::Air
            )
    );
    app.world_mut().insert_resource(controllers);
    let mut activated = false;
    let mut waved = false;
    for _ in 0..think * 12 {
        for event in step(&mut app) {
            assert!(
                !matches!(
                    event,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    }
                ),
                "an understrength ground cohort must preserve the real charging support source and its sole reactor"
            );
            activated |= matches!(event, sim::SimEvent::StrategicActivated {building, ..} if building == source);
        }
        waved |= app
            .world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave()
            .contains(&tank);
        if activated {
            break;
        }
    }
    let reactor_entity = unit_entity(&mut app, reactor);
    assert!(
        app.world()
            .get::<sim::Health>(reactor_entity)
            .is_some_and(|hp| hp.hit_points > 0)
    );
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power()
    );
    eprintln!(
        "ground support waved{waved} activated{activated} tick{:?} cooldown{cooldown} charge{:?} known{:?} wave{:?} commands{:?}",
        app.world().resource::<SimTick>(),
        app.world()
            .resource::<sim::SpecialState>()
            .charge
            .get(&source),
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .known_buildings(),
        app.world()
            .resource::<sim::AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .wave(),
        app.world().resource::<CommandQueue>()
    );
    assert!(
        waved && activated,
        "the actual ground-only wave must receive native Chronoshift after natural source charge completes"
    );
}

// RA9: source Cost/Airspeed/Groundspeed are neutral1.0 in all three source sections.
// Current house difficulty changes other explicit fields; it does not scale these consumers.
#[test]
fn ra9_difficulty_neutral_cost_ground_and_air_motion_use_actual_same_rules() {
    fn position(app: &mut App, id: sim::SimId) -> sim::SubPos {
        let entity = app
            .world_mut()
            .query::<(bevy_ecs::entity::Entity, &sim::SimId)>()
            .iter(app.world())
            .find(|(_, found)| **found == id)
            .unwrap()
            .0;
        app.world().get::<sim::Position>(entity).unwrap().0
    }
    for difficulty in ["easy", "medium", "brutal"] {
        let mut app = scenario("great_britain", difficulty, 10000, 0, true);
        app.world_mut().insert_resource(sim::AiPlayers::default());
        let definitions = app.world().resource::<sim::Definitions>().clone();
        let power = definitions.building_kind("power_plant").unwrap();
        assert_eq!(
            definitions
                .production(sim::ProductionItem::Building(power))
                .unwrap()
                .cost,
            800
        );
        let terms = definitions
            .production(sim::ProductionItem::Building(power))
            .unwrap();
        assert_eq!(terms.build_time, 672);
        let house_build_pm = if difficulty == "brutal" { 800 } else { 1000 };
        assert_eq!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .ai_build_time_pm(),
            house_build_pm
        );
        let ground = spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "grizzly",
            CellPos::new(20, 20),
        )
        .unwrap();
        let air = spawn_unit(app.world_mut(), PlayerId(0), "kirov", CellPos::new(20, 23)).unwrap();
        assert_eq!(
            definitions
                .unit(definitions.unit_kind("grizzly").unwrap())
                .unwrap()
                .speed,
            13
        );
        assert_eq!(
            definitions
                .unit(definitions.unit_kind("kirov").unwrap())
                .unwrap()
                .speed,
            4
        );
        let ground_start = position(&mut app, ground);
        let air_start = position(&mut app, air);
        let credits = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits();
        issue(
            &mut app,
            Command::QueueProduction {
                item: sim::ProductionItem::Building(power),
            },
        );
        issue(
            &mut app,
            Command::Move {
                units: vec![ground],
                target: CellPos::new(26, 20),
            },
        );
        issue(
            &mut app,
            Command::Move {
                units: vec![air],
                target: CellPos::new(26, 23),
            },
        );
        for elapsed in 1i32..=10 {
            let expected_progress = u32::try_from(elapsed).unwrap() * 1000 / house_build_pm;
            step(&mut app);
            assert!(
                app.world()
                    .resource::<sim::Players>()
                    .iter()
                    .all(|(_, p)| p.credits() >= 0)
            );
            assert_eq!(
                app.world()
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits(),
                credits - i64::from(expected_progress) * (terms.cost / i64::from(terms.build_time)),
                "{difficulty}: exact source cost charged per actual production progress, no house cost scale"
            );
            let player = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            let head = &player.queues().next().unwrap().1[0];
            assert_eq!(head.progress, expected_progress);
            assert_eq!(
                head.charged,
                i64::from(head.progress) * (terms.cost / i64::from(terms.build_time))
            );
            let g = position(&mut app, ground);
            let a = position(&mut app, air);
            assert_eq!(
                (g.x - ground_start.x, g.y - ground_start.y),
                (13 * elapsed, 0),
                "{difficulty}: loaded ground speed, no house scale"
            );
            assert_eq!(
                (a.x - air_start.x, a.y - air_start.y),
                (4 * elapsed, 0),
                "{difficulty}: loaded air speed, no house scale"
            );
        }
        // Preserve the original full-cost proposition at the actual paid completion boundary.
        // The source price is paid across progress, with its division remainder on the last unit.
        let mut completed = false;
        for _ in 10..=terms.build_time {
            step(&mut app);
            let player = app
                .world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap();
            assert!(player.credits() >= 0);
            let head = &player.queues().next().unwrap().1[0];
            if head.progress == terms.build_time {
                assert_eq!(head.charged, 800);
                assert_eq!(
                    player.credits(),
                    credits - 800,
                    "{difficulty}: identical full stock price"
                );
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "{difficulty}: real paid production never reached source build time"
        );
    }
}

fn optional_reinforcement_with_independent_recovery(strategic: bool, observed_ore: bool) {
    let mut app = scenario("russia", "brutal", 0, 0, strategic);
    let reactor = put(&mut app, "nuclear_reactor", 2, 2);
    let barracks = put(&mut app, "soviet_barracks", 5, 12);
    let source = strategic.then(|| put(&mut app, "nuclear_silo", 2, 18));
    let asset = put(&mut app, "cloning_vats", 18, 5);
    let ore = CellPos::new(7, 15);
    if observed_ore {
        app.world_mut().resource_mut::<Grid>().set_terrain(
            ore,
            Cell {
                passable: true,
                buildable: true,
                water: false,
                ore: 12,
            },
        );
    }
    let fighter = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "conscript",
        CellPos::new(12, 13),
    )
    .unwrap();
    let target = sim::spawn_building(
        app.world_mut(),
        PlayerId(1),
        "soviet_construction_yard",
        CellPos::new(17, 13),
    )
    .unwrap();
    let definitions = app.world().resource::<sim::Definitions>();
    let item = sim::ProductionItem::Unit(definitions.unit_kind("conscript").unwrap());
    let cost = definitions.production(item).unwrap().cost;
    let refund = definitions.refund(definitions.building_kind("cloning_vats").unwrap());
    assert!(refund >= cost && cost > 0);
    let mut sold = None;
    let mut charged = false;
    let mut paid = false;
    let mut recruits = std::collections::BTreeSet::new();
    let mut fighter_shot = false;
    let mut recruit_shot = false;
    let mut source_charged = false;
    for _ in 0..6000 {
        let world = app.world_mut();
        assert!(
            world
                .query::<(&sim::Owner, &sim::Harvester)>()
                .iter(world)
                .all(|(owner, _)| owner.0 != PlayerId(0)),
            "observed ore alone is not actual income without an owned worker"
        );
        if observed_ore && world.resource::<SimTick>().0 > 0 {
            assert!(
                world
                    .resource::<sim::Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .vision()
                    .is_visible(ore)
                    && world.resource::<Grid>().cell(ore).unwrap().ore > 0,
                "the ordinary field must actually be observed while no worker exists"
            );
        }
        charged |= world
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, queue)| queue)
            .any(|head| head.item == item && head.charged > 0);
        source_charged |= source.is_some_and(|source| {
            world
                .resource::<sim::SpecialState>()
                .charge
                .get(&source)
                .is_some_and(|charge| *charge > 0)
        });
        for event in step(&mut app) {
            match event {
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    building,
                    refund: amount,
                } => {
                    assert_eq!(building, asset, "retain source, power and real producers");
                    assert_eq!(amount, refund, "ordinary exact refund only");
                    assert!(sold.replace(amount).is_none());
                }
                sim::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: found,
                } if found == item => paid = true,
                sim::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    building,
                    unit,
                } if building == barracks => {
                    recruits.insert(unit);
                }
                sim::SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    ..
                } if found == target => {
                    fighter_shot |= attacker == fighter;
                    recruit_shot |= recruits.contains(&attacker);
                }
                sim::SimEvent::OreDelivered {
                    player: PlayerId(0),
                    ..
                } => panic!("this declared no-worker scenario must have no income"),
                _ => {}
            }
        }
        if recruit_shot && fighter_shot {
            break;
        }
    }
    let reactor_entity = unit_entity(&mut app, reactor);
    assert!(
        app.world()
            .get::<sim::Health>(reactor_entity)
            .unwrap()
            .hit_points
            > 0
    );
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power()
    );
    if let Some(source) = source {
        let entity = unit_entity(&mut app, source);
        assert!(app.world().get::<sim::Health>(entity).unwrap().hit_points > 0);
        assert!(source_charged, "retain actual natural strategic charging");
    }
    eprintln!(
        "independent recovery strategic{strategic} ore{observed_ore} sold{sold:?} charged{charged} paid{paid} fighter_shot{fighter_shot} recruit_shot{recruit_shot} source_charged{source_charged}"
    );
    assert!(
        fighter_shot,
        "the original living field force must actually engage"
    );
    assert_eq!(
        sold,
        Some(refund),
        "independent recovery must not veto an unrelated optional reinforcement refund"
    );
    assert!(charged && paid && !recruits.is_empty() && recruit_shot);
}

#[test]
fn ra8_optional_reinforcement_preserves_strategic_sources_and_power() {
    optional_reinforcement_with_independent_recovery(true, false);
}

#[test]
fn ra8_optional_reinforcement_is_not_vetoed_by_observed_ore_without_workers() {
    optional_reinforcement_with_independent_recovery(false, true);
}

// These are declared public scenario geometry, installed before controller construction.
fn ra9_authored_control_barrier(grid: &mut Grid) {
    for y in 0..48 {
        for x in 17..=22 {
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
}
fn ra9_authored_ivan_island(grid: &mut Grid) {
    for y in 32..48 {
        for x in 32..48 {
            if !(38..=40).contains(&x) || !(38..=40).contains(&y) {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: false,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
    }
}

fn ra9_authored_terrain(grid: &mut Grid, bridge_endpoint: bool) {
    let mut looks = vec![0; 48 * 48];
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
    let mut deck = vec![
        CellPos::new(22, 24),
        CellPos::new(23, 24),
        CellPos::new(24, 24),
    ];
    if bridge_endpoint {
        deck.extend([
            CellPos::new(20, 22),
            CellPos::new(20, 23),
            CellPos::new(20, 24),
            CellPos::new(21, 24),
        ]);
    }
    for cell in deck {
        grid.set_terrain(
            cell,
            Cell {
                passable: true,
                water: false,
                buildable: false,
                ore: 0,
            },
        );
        looks[cell.y as usize * 48 + cell.x as usize] = 8 << 4;
    }
    if !bridge_endpoint {
        grid.set_terrain(
            CellPos::new(20, 22),
            Cell {
                passable: false,
                water: false,
                buildable: false,
                ore: 0,
            },
        );
        looks[22 * 48 + 20] = 5 << 4;
    }
    for (x, y) in [(21, 22), (25, 22)] {
        looks[y * 48 + x] = 15 << 4;
    }
    if bridge_endpoint {
        // The bent deck extends left of the remembered hut. Author its outward
        // entrance outside the scout's sight so install does not add a nearer
        // visible hut and silently change the actual repair objective.
        looks[20 * 48 + 19] = 15 << 4;
    }
    grid.mark_world_features(&looks);
}
fn ra9_authored_house_endpoint(grid: &mut Grid) {
    ra9_authored_terrain(grid, false);
}
fn ra9_authored_bridge_endpoint(grid: &mut Grid) {
    ra9_authored_terrain(grid, true);
}

fn ra9_privacy_assert_mutable_terrain(bridge: bool) {
    let kept = ra9_privacy_mutable_terrain_decision(bridge, false, false);
    let changed = ra9_privacy_mutable_terrain_decision(bridge, true, false);
    assert_eq!(kept.0, changed.0, "equal actual vision/exploration");
    assert_eq!(
        kept.1, changed.1,
        "same public map and legitimate controller memory"
    );
    assert_eq!(kept.2, changed.2, "same own actors and positions");
    assert_eq!(
        kept.3, changed.3,
        "hidden physical terrain must not select an outgoing AI command"
    );
    let seen_kept = ra9_privacy_mutable_terrain_decision(bridge, false, true);
    let seen_changed = ra9_privacy_mutable_terrain_decision(bridge, true, true);
    assert_ne!(
        seen_kept.3, seen_changed.3,
        "actual re-observation must update the native approach decision"
    );
}

#[test]
fn ra9_privacy_remembered_bridge_approach_ignores_real_hidden_house_destruction() {
    ra9_privacy_assert_mutable_terrain(false);
}

#[test]
fn ra9_privacy_remembered_bridge_approach_ignores_real_hidden_bridge_repair() {
    ra9_privacy_assert_mutable_terrain(true);
}

fn ra9_privacy_mutable_terrain_decision(
    bridge_change: bool,
    mutate: bool,
    reveal: bool,
) -> BridgeDecisionObservation {
    let mut app = scenario_mode(
        "russia",
        "medium",
        0,
        0,
        false,
        ScenarioMode {
            fog: true,
            authored_terrain: Some(if bridge_change {
                ra9_authored_bridge_endpoint
            } else {
                ra9_authored_house_endpoint
            }),
            ..ScenarioMode::default()
        },
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
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
    let engineer = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "engineer",
        CellPos::new(10, 22),
    )
    .unwrap();
    let scout = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "rocketeer",
        CellPos::new(27, 22),
    )
    .unwrap();
    let tank = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "grizzly",
        CellPos::new(20, 26),
    )
    .unwrap();
    let enemy_engineer = spawn_unit(
        app.world_mut(),
        PlayerId(1),
        "engineer",
        CellPos::new(20, 21),
    )
    .unwrap();
    step(&mut app);
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Attack {
                units: vec![tank],
                target: bridge,
            },
        });
    for _ in 0..4000 {
        step(&mut app);
        if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
            break;
        }
    }
    assert!(
        app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed,
        "actual public enemy attack must collapse the bridge"
    );
    let think = u64::from(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty()
            .think_interval,
    );
    while !app.world().resource::<SimTick>().0.is_multiple_of(think) {
        step(&mut app);
    }
    app.world_mut().insert_resource(controllers);
    step(&mut app); // Genuine due AI observation of hut + collapsed deck.
    assert!(
        app.world()
            .resource::<CommandQueue>()
            .iter()
            .any(|issued| issued.player == PlayerId(0)
                && matches!(issued.command,
            Command::Interact { unit, target } if unit == engineer && target == hut)),
        "current observed hut must first produce its ordinary native repair intent"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    // Let the real queued repair be admitted, then explicitly cancel it before moving the scout.
    issue(
        &mut app,
        Command::Stop {
            units: vec![engineer],
        },
    );
    issue(
        &mut app,
        Command::Move {
            units: vec![scout],
            target: CellPos::new(36, 22),
        },
    );
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Stop { units: vec![tank] },
        });
    for _ in 0..1200 {
        step(&mut app);
        let entity = unit_entity(&mut app, scout);
        let mobile = app.world().get::<sim::Mobile>(entity).unwrap();
        if mobile.cell == CellPos::new(36, 22) && mobile.step.is_none() {
            break;
        }
    }
    let scout_entity = unit_entity(&mut app, scout);
    assert_eq!(
        app.world().get::<sim::Mobile>(scout_entity).unwrap().cell,
        CellPos::new(36, 22)
    );
    let vision = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .vision();
    assert!(
        vision.is_explored(CellPos::new(21, 22)) && !vision.is_visible(CellPos::new(21, 22)),
        "hut is genuinely remembered through optional fog"
    );
    let endpoint = CellPos::new(20, 22);
    assert!(
        !vision.is_visible(endpoint),
        "native terrain mutation must be hidden"
    );
    let changed_target = if bridge_change {
        bridge
    } else {
        app.world()
            .resource::<sim::Props>()
            .id_at(endpoint)
            .unwrap()
    };
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: if mutate {
                if bridge_change {
                    Command::Interact {
                        unit: enemy_engineer,
                        target: hut,
                    }
                } else {
                    Command::Attack {
                        units: vec![tank],
                        target: changed_target,
                    }
                }
            } else {
                Command::Stop {
                    units: vec![tank, enemy_engineer],
                }
            },
        });
    for _ in 0..1600 {
        step(&mut app);
    }
    assert!(
        !app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(endpoint)
    );
    if bridge_change {
        assert_eq!(
            app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed,
            !mutate,
            "public Engineer repair premise"
        );
    } else {
        assert_eq!(
            app.world()
                .resource::<sim::Props>()
                .id_at(endpoint)
                .is_none(),
            mutate,
            "public enemy destruction premise"
        );
    }
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(sim::IssuedCommand {
            player: PlayerId(1),
            command: Command::Stop { units: vec![tank] },
        });
    step(&mut app);
    if reveal {
        issue(
            &mut app,
            Command::Move {
                units: vec![scout],
                target: CellPos::new(14, 27),
            },
        );
        for _ in 0..1600 {
            step(&mut app);
            let e = unit_entity(&mut app, scout);
            let m = app.world().get::<sim::Mobile>(e).unwrap();
            if m.cell == CellPos::new(14, 27) && m.step.is_none() {
                break;
            }
        }
        let v = app
            .world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision();
        assert!(v.is_visible(endpoint));
        assert!(
            !v.is_visible(CellPos::new(21, 22)),
            "re-observe changed terrain while remembered hut stays outside sight"
        );
    }
    while !app.world().resource::<SimTick>().0.is_multiple_of(think) {
        step(&mut app);
    }
    let vision = app
        .world()
        .resource::<sim::Players>()
        .get(PlayerId(0))
        .unwrap()
        .vision()
        .clone();
    let knowledge = controllers.controller(PlayerId(0)).unwrap().clone();
    let own = {
        let world = app.world_mut();
        let mut own: Vec<_> = world
            .query::<(&sim::SimId, &sim::Mobile, &sim::Owner)>()
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == PlayerId(0))
            .map(|(id, mobile, owner)| (*id, mobile.cell, *owner))
            .collect();
        own.sort_by_key(|(id, ..)| *id);
        own
    };
    app.world_mut().insert_resource(controllers);
    step(&mut app); // Capture commands before their next Command admission/pathfinding.
    let decisions: Vec<_> = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .filter(|issued| issued.player == PlayerId(0))
        .filter_map(|issued| match &issued.command {
            Command::Move { units, .. } if units.contains(&engineer) => {
                Some(issued.command.clone())
            }
            _ => None,
        })
        .collect();
    assert!(
        (reveal && bridge_change && mutate)
            || matches!(decisions.as_slice(), [Command::Move { .. }]),
        "remembered bridge job must emit an actual queued Engineer approach: {decisions:?}"
    );
    (vision, knowledge, own, decisions)
}

#[test]
fn ra9_lone_ivan_keeps_actual_no_income_last_stand_capability() {
    let mut app = scenario_mode(
        "russia",
        "easy",
        0,
        0,
        false,
        ScenarioMode {
            authored_terrain: Some(ra9_authored_ivan_island),
            ..ScenarioMode::default()
        },
    );
    let ivan = spawn_unit(
        app.world_mut(),
        PlayerId(0),
        "crazy_ivan",
        CellPos::new(15, 8),
    )
    .unwrap();
    let metadata = app.world().resource::<sim::Definitions>();
    assert!(
        metadata
            .metadata(sim::ProductionItem::Building(
                metadata.building_kind("power_plant").unwrap()
            ))
            .unwrap()
            .bombable
    );
    for _ in 0..4200 {
        let events = step(&mut app);
        assert!(
            !events.iter().any(|e| matches!(
                e,
                sim::SimEvent::BuildingSold {
                    player: PlayerId(0),
                    ..
                }
            )),
            "a living no-income Ivan is a real building finisher"
        );
        assert_eq!(
            app.world()
                .resource::<sim::Players>()
                .get(PlayerId(0))
                .unwrap()
                .credits(),
            0,
            "unfunded construction is not income or a paid replacement force"
        );
    }
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<sim::AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    issue(&mut app, Command::Stop { units: vec![ivan] });
    step(&mut app);
    let ivan_entity = unit_entity(&mut app, ivan);
    let at = app.world().get::<sim::Mobile>(ivan_entity).unwrap().cell;
    let mut target = None;
    for (dx, dy) in [(-3, 0), (0, -3), (3, 0), (0, 3)] {
        if let Ok(id) = sim::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "power_plant",
            CellPos::new(at.x + dx, at.y + dy),
        ) {
            target = Some(id);
            break;
        }
    }
    let target = target.expect("an adjacent real bombable target on legal terrain");
    step(&mut app); // Ordinary visibility sees this new adjacent building before Attack.
    let target_entity = unit_entity(&mut app, target);
    let initial = app
        .world()
        .get::<sim::Health>(target_entity)
        .unwrap()
        .hit_points;
    issue(
        &mut app,
        Command::Attack {
            units: vec![ivan],
            target,
        },
    );
    let mut charged = false;
    for _ in 0..1200 {
        step(&mut app);
        charged |= app
            .world()
            .resource::<sim::SpecialState>()
            .charges
            .contains_key(&target);
        if app
            .world()
            .get::<sim::Health>(target_entity)
            .is_none_or(|h| h.hit_points < initial)
        {
            break;
        }
    }
    assert!(charged, "same retained actor must place a real Ivan charge");
    assert!(
        app.world()
            .get::<sim::Health>(target_entity)
            .is_none_or(|h| h.hit_points < initial)
    );
    app.world_mut().insert_resource(controllers);
}

fn ra9_building_capability_metadata_fixture(app: &mut App, nonbombable: bool) {
    let source = std::fs::read_to_string(data().join("arsenal.ron")).unwrap();
    let definitions = app.world().resource::<sim::Definitions>();
    let mut count = 0;
    let altered = source
        .lines()
        .map(|line| {
            let id = line
                .trim_start()
                .strip_prefix("(id: \"")
                .and_then(|rest| rest.split('"').next());
            if id.is_some_and(|id| definitions.building_kind(id).is_some()) {
                count += 1;
                if nonbombable {
                    line.replace(",bombable: 1", ",bombable: 0")
                } else {
                    line.replace(",immune: 0", ",immune: 1")
                }
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        count,
        definitions.buildings().len(),
        "each actual target type is represented by the fixture"
    );
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir =
        std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra9-ai-selector-metadata");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!(
        "targets-{}-{}.ron",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .unwrap()
        .write_all(altered.as_bytes())
        .unwrap();
    let fixture = definitions.clone().with_arsenal(&file).unwrap();
    std::fs::remove_file(file).unwrap();
    app.world_mut().insert_resource(fixture);
}

#[test]
fn ra9_lone_ivan_cannot_renew_last_stand_when_all_real_target_types_are_nonbombable_or_immune() {
    for nonbombable in [true, false] {
        let mut app = scenario_mode(
            "russia",
            "easy",
            0,
            0,
            false,
            ScenarioMode {
                authored_terrain: Some(ra9_authored_ivan_island),
                ..ScenarioMode::default()
            },
        );
        ra9_building_capability_metadata_fixture(&mut app, nonbombable);
        spawn_unit(
            app.world_mut(),
            PlayerId(0),
            "crazy_ivan",
            CellPos::new(15, 8),
        )
        .unwrap();
        let mut sold = false;
        for _ in 0..4200 {
            sold |= step(&mut app).iter().any(|e| {
                matches!(
                    e,
                    sim::SimEvent::BuildingSold {
                        player: PlayerId(0),
                        ..
                    }
                )
            });
            if sold {
                break;
            }
        }
        assert!(
            sold,
            "real selector-ineligible targets must preserve the helpless liquidation boundary; nonbombable={nonbombable}"
        );
    }
}
