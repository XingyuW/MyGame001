//! Match options added after M11: computer players allied against the human, the Super Me unit
//! speed, and destructible scenery (`sim::Props`).
//!
//! Every scenario is set up on fixture data and then runs only through `SimPlugin` and
//! `CommandQueue`.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AI_TEAM, AiDefinitions, Cell, CellPos, Command, CommandQueue, Controller, Definitions, Grid,
    Health, IssuedCommand, MatchOptions, MatchOutcome, MatchState, Mobile, PROP_ID_BASE, PlayerId,
    PlayerSetup, Players, PropKind, Props, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule,
    SkirmishDefinitions, load_ai, load_map, load_match_definitions, load_skirmish,
    setup_match_configured, spawn_unit, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

/// The fixture data, with a third start position on the 40x40 map.
fn data() -> Data {
    let definitions = load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut skirmish = load_skirmish(fixture("data/skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    skirmish.maps[0].starts.push(CellPos::new(32, 5));
    let ai =
        load_ai(fixture("data/ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

fn world(data: &Data) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    let grid = load_map(fixture("maps/skirmish_40x40.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    app
}

fn player(player: PlayerId, faction: usize, start: usize, controller: Controller) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller,
    }
}

const AI: Controller = Controller::Ai { difficulty: 1 };

fn three_players(data: &Data, options: &MatchOptions) -> App {
    let mut app = world(data);
    setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        &[
            player(P0, 0, 0, Controller::Human),
            player(P1, 1, 1, AI),
            player(P2, 1, 2, AI),
        ],
        options,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    app
}

fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(command);
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn hit_points(app: &mut App, id: SimId) -> Option<i32> {
    let mut query = app.world_mut().query::<(&SimId, &Health)>();
    query
        .iter(app.world())
        .find(|(found, _)| **found == id)
        .map(|(_, health)| health.hit_points)
}

/// The computer players allied against the human: the default since RA1 (fixed decision 3).
fn allied() -> MatchOptions {
    MatchOptions {
        free_for_all: false,
        ..MatchOptions::default()
    }
}

/// Every player for itself, which was the default before RA1.
fn free() -> MatchOptions {
    MatchOptions {
        free_for_all: true,
        ..MatchOptions::default()
    }
}

#[test]
fn allied_computer_players_are_hostile_to_the_human_only() {
    let d = data();
    let app = three_players(&d, &allied());
    let players = app.world().resource::<Players>();
    assert_eq!(players.get(P0).unwrap().team(), 0);
    assert_eq!(players.get(P1).unwrap().team(), AI_TEAM);
    assert_eq!(players.get(P2).unwrap().team(), AI_TEAM);
    assert!(players.hostile(P0, P1) && players.hostile(P1, P0));
    assert!(players.hostile(P0, P2) && players.hostile(P2, P0));
    assert!(!players.hostile(P1, P2) && !players.hostile(P2, P1));
    assert!(!players.hostile(P1, P1));

    // Without the option, and in a match without a human, everyone fights everyone.
    let free = three_players(&d, &free());
    assert!(free.world().resource::<Players>().hostile(P1, P2));
    let mut spectated = world(&d);
    setup_match_configured(
        spectated.world_mut(),
        &d.skirmish,
        &d.ai,
        0,
        &[player(P0, 0, 0, AI), player(P1, 1, 1, AI)],
        &allied(),
    )
    .unwrap();
    assert!(spectated.world().resource::<Players>().hostile(P0, P1));
}

#[test]
fn allied_units_standing_side_by_side_do_not_fire_but_fire_at_the_human() {
    let d = data();
    // Players the test controls, so that no computer player moves the units away.
    let mut app = world(&d);
    setup_match_configured(
        app.world_mut(),
        &d.skirmish,
        &d.ai,
        0,
        &[
            player(P0, 0, 0, Controller::Human),
            player(P1, 1, 1, Controller::Human),
            player(P2, 1, 2, Controller::Human),
        ],
        &free(),
    )
    .unwrap();
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        assert!(players.set_team(P1, AI_TEAM) && players.set_team(P2, AI_TEAM));
    }
    // Two tanks of the allied players next to each other, far from every base.
    let a = spawn_unit(app.world_mut(), P1, "tank", CellPos::new(20, 20)).unwrap();
    let b = spawn_unit(app.world_mut(), P2, "tank", CellPos::new(21, 20)).unwrap();
    let mut fired = false;
    for _ in 0..100 {
        fired |= tick(&mut app, vec![])
            .iter()
            .any(|event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == a || *attacker == b));
    }
    assert!(!fired, "allies fired at each other");
    assert_eq!(hit_points(&mut app, a), Some(300));
    assert_eq!(hit_points(&mut app, b), Some(300));

    // An attack order on an ally is not carried out either.
    let events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P1,
            command: Command::Attack {
                units: vec![a],
                target: b,
            },
        }],
    );
    drop(events);
    for _ in 0..40 {
        tick(&mut app, vec![]);
    }
    assert_eq!(hit_points(&mut app, b), Some(300));

    // A human unit beside them is fired at by both.
    let victim = spawn_unit(app.world_mut(), P0, "tank", CellPos::new(19, 20)).unwrap();
    let mut attackers = std::collections::BTreeSet::new();
    for _ in 0..60 {
        for event in tick(&mut app, vec![]) {
            if let SimEvent::WeaponFired {
                attacker, target, ..
            } = event
                && target == victim
            {
                attackers.insert(attacker);
            }
        }
    }
    assert!(
        attackers.contains(&a) && attackers.contains(&b),
        "{attackers:?}"
    );
}

#[test]
fn the_match_ends_when_only_allies_are_left() {
    let d = data();
    let mut app = three_players(&d, &allied());
    // The human's buildings vanish: the two computer players are left, and they are allies.
    let mut query = app
        .world_mut()
        .query::<(bevy_ecs::entity::Entity, &sim::Owner, &sim::Building)>();
    let own: Vec<_> = query
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == P0)
        .map(|(entity, ..)| entity)
        .collect();
    for entity in own {
        app.world_mut()
            .get_mut::<Health>(entity)
            .unwrap()
            .hit_points = 0;
    }
    let mut ended = None;
    for _ in 0..3 {
        for event in tick(&mut app, vec![]) {
            if let SimEvent::MatchEnded { winner } = event {
                ended = Some(winner);
            }
        }
    }
    assert_eq!(ended, Some(Some(P1)));
    assert!(matches!(
        app.world().resource::<MatchState>().outcome(),
        Some(MatchOutcome::Won { winner: P1, .. })
    ));
}

/// Cells a tank of `P0` covers along an open row in 40 ticks.
fn distance_covered(data: &Data, options: &MatchOptions) -> (i32, u64) {
    let mut app = world(data);
    setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        &[player(P0, 0, 0, Controller::Human), player(P1, 1, 1, AI)],
        options,
    )
    .unwrap();
    let tank = spawn_unit(app.world_mut(), P0, "tank", CellPos::new(2, 20)).unwrap();
    tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Move {
                units: vec![tank],
                target: CellPos::new(38, 20),
            },
        }],
    );
    for _ in 0..40 {
        tick(&mut app, vec![]);
    }
    let mut query = app.world_mut().query::<(&SimId, &Mobile)>();
    let x = query
        .iter(app.world())
        .find(|(id, _)| **id == tank)
        .map(|(_, mobile)| mobile.cell.x)
        .unwrap();
    (x - 2, state_hash(app.world_mut()))
}

#[test]
fn super_me_unit_speed_moves_the_humans_units_faster_and_nobody_elses() {
    let d = data();
    let (plain, plain_hash) = distance_covered(&d, &free());
    let boosted = MatchOptions {
        super_players: vec![P0, P1],
        unit_speed_percent: 300,
        ..free()
    };
    let (fast, fast_hash) = distance_covered(&d, &boosted);
    assert!(plain >= 2, "the tank moved {plain} cells");
    assert!(
        fast >= plain * 3 - 3 && fast <= plain * 3 + 3,
        "3x speed: {fast} cells against {plain}"
    );
    assert_ne!(plain_hash, fast_hash);

    // The option needs Super Me, and never reaches a computer player.
    let no_super = MatchOptions {
        unit_speed_percent: 300,
        ..free()
    };
    assert_eq!(distance_covered(&d, &no_super), (plain, plain_hash));
    let mut app = world(&d);
    setup_match_configured(
        app.world_mut(),
        &d.skirmish,
        &d.ai,
        0,
        &[player(P0, 0, 0, Controller::Human), player(P1, 1, 1, AI)],
        &MatchOptions {
            super_players: vec![P0, P1],
            unit_speed_percent: 9_999,
            ..free()
        },
    )
    .unwrap();
    let players = app.world().resource::<Players>();
    assert_eq!(players.unit_speed_percent(P0), 400, "clamped");
    assert_eq!(players.unit_speed_percent(P1), 100);
}

/// A two-player match with a house at (10, 5) and trees at (11, 5), near the human's start.
fn with_scenery(data: &Data) -> (App, CellPos, CellPos) {
    let mut app = world(data);
    let (house, trees) = (CellPos::new(10, 5), CellPos::new(11, 5));
    let rock = Cell {
        passable: false,
        water: false,
        buildable: false,
        ore: 0,
    };
    let (width, height) = {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        grid.set_terrain(house, rock);
        grid.set_terrain(trees, rock);
        (grid.width() as usize, grid.height() as usize)
    };
    let mut looks = vec![0u8; width * height];
    looks[house.y as usize * width + house.x as usize] = 5 << 4;
    looks[trees.y as usize * width + trees.x as usize] = 4 << 4;
    // A scenery look on open ground is not a prop: only impassable cells are.
    looks[0] = 4 << 4;
    let armour = data.definitions.armour_class("concrete").unwrap();
    let props = Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
    assert_eq!(props.len(), 2);
    app.world_mut().insert_resource(props);
    setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        &[
            player(P0, 0, 0, Controller::Human),
            player(P1, 1, 1, Controller::Human),
        ],
        &free(),
    )
    .unwrap();
    (app, house, trees)
}

#[test]
fn scenery_is_never_fired_at_unasked_and_falls_to_an_attack_order() {
    let d = data();
    let (mut app, house, trees) = with_scenery(&d);
    let tank = spawn_unit(app.world_mut(), P0, "tank", CellPos::new(12, 6)).unwrap();
    for _ in 0..60 {
        let events = tick(&mut app, vec![]);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SimEvent::WeaponFired { .. })),
            "nobody fires at scenery on its own"
        );
    }
    let props = app.world().resource::<Props>();
    assert_eq!(props.at(house).unwrap().hit_points, 300);
    let house_id = props.id_at(house).unwrap();
    let trees_id = props.id_at(trees).unwrap();
    assert!(house_id.0 >= PROP_ID_BASE);
    assert!(!app.world().resource::<Grid>().is_passable(house));

    let before = state_hash(app.world_mut());
    let mut destroyed = Vec::new();
    let mut events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Attack {
                units: vec![tank],
                target: house_id,
            },
        }],
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    for _ in 0..600 {
        for event in events.drain(..) {
            if let SimEvent::PropDestroyed { prop, cell, kind } = event {
                destroyed.push((prop, cell, kind));
            }
        }
        if !destroyed.is_empty() {
            break;
        }
        events = tick(&mut app, vec![]);
    }
    assert_eq!(destroyed, [(house_id, house, PropKind::House)]);
    assert_ne!(before, state_hash(app.world_mut()));
    let grid = app.world().resource::<Grid>();
    assert!(grid.is_passable(house), "the rubble is open ground");
    assert!(grid.cell(house).unwrap().buildable);
    let props = app.world().resource::<Props>();
    assert!(props.at(house).is_none() && props.get(house_id).is_none());
    assert_eq!(props.at(trees).unwrap().hit_points, 60, "the trees stand");

    // The order ended with its target; the trees fall to the next one.
    let mut gone = false;
    let mut events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Attack {
                units: vec![tank],
                target: trees_id,
            },
        }],
    );
    for _ in 0..300 {
        gone |= events
            .iter()
            .any(|event| matches!(event, SimEvent::PropDestroyed { cell, .. } if *cell == trees));
        events = tick(&mut app, vec![]);
    }
    assert!(gone);
    assert!(app.world().resource::<Props>().is_empty());
}

#[test]
fn scenery_the_player_has_not_explored_or_that_does_not_exist_cannot_be_attacked() {
    let d = data();
    let (mut app, house, _) = with_scenery(&d);
    let tank = spawn_unit(app.world_mut(), P1, "tank", CellPos::new(30, 30)).unwrap();
    tick(&mut app, vec![]);
    let house_id = app.world().resource::<Props>().id_at(house).unwrap();
    // P1 starts at the far corner and has never seen the house.
    for target in [house_id, SimId(PROP_ID_BASE + 3)] {
        let events = tick(
            &mut app,
            vec![IssuedCommand {
                player: P1,
                command: Command::Attack {
                    units: vec![tank],
                    target,
                },
            }],
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                SimEvent::CommandRejected {
                    reason: RejectReason::UnknownTarget(found),
                    ..
                } if *found == target
            )),
            "{events:?}"
        );
    }
}
