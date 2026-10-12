//! M6 tests: skirmish match setup and the win condition (PLAN.md section 7, M6: "win and lose
//! detection when all buildings of a player are destroyed"; invariants I3, I5, I6;
//! docs/DECISIONS.md D40, D41).
//!
//! Every scenario is set up with `setup_match` on fixture data (the fixture units, economy,
//! combat, skirmish, and AI files and the 40x40 skirmish fixture map) and then runs only through
//! `SimPlugin` and `CommandQueue`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AiDefinitions, AiPlayers, Building, CellPos, Command, CommandQueue, Controller, Definitions,
    Footprint, IssuedCommand, MatchOutcome, MatchSetupError, MatchStart, MatchState, Mobile, Owner,
    PlayerId, PlayerSetup, Players, SimEvent, SimId, SimPlugin, SimSchedule, SkirmishDefinitions,
    StartError, StartProblem, StartingBuilding, StartingGroup, Unit, load_ai, load_map,
    load_match_definitions, load_skirmish, plan_starting_units, setup_match, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

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

fn data() -> Data {
    let definitions = load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish = load_skirmish(fixture("data/skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai =
        load_ai(fixture("data/ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

/// A world with the skirmish fixture map and the fixture definitions installed, not set up.
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

fn human(player: PlayerId, faction: usize, start: usize) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller: Controller::Human,
    }
}

fn computer(player: PlayerId, faction: usize, start: usize, difficulty: usize) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller: Controller::Ai { difficulty },
    }
}

/// Two players controlled by the test: P0 plays faction 0 at start 0, P1 faction 1 at start 1.
fn two_humans(data: &Data) -> App {
    let mut app = world(data);
    setup_match(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), human(P1, 1, 1)],
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

fn buildings_of(app: &mut App, player: PlayerId) -> Vec<(SimId, Footprint)> {
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
    let mut found: Vec<(SimId, Footprint)> = query
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == player)
        .map(|(id, _, building)| (*id, building.footprint))
        .collect();
    found.sort_unstable_by_key(|(id, _)| *id);
    found
}

fn sell_all(app: &mut App, player: PlayerId) -> Vec<IssuedCommand> {
    buildings_of(app, player)
        .into_iter()
        .map(|(building, _)| IssuedCommand {
            player,
            command: Command::SellBuilding { building },
        })
        .collect()
}

// --- Setup ----------------------------------------------------------------------------------

#[test]
fn setup_places_every_factions_buildings_and_units_at_its_start() {
    let data = data();
    let mut app = world(&data);
    let start = setup_match(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        // Given out of order; players are set up in id order.
        &[
            PlayerSetup {
                player: P1,
                faction: 1,
                start: 1,
                controller: Controller::Ai { difficulty: 1 },
            },
            human(P0, 0, 0),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        start,
        MatchStart {
            buildings: 3,
            units: 7
        }
    );
    let yard = data.definitions.building_kind("yard").expect("yard");
    let turret = data.definitions.building_kind("turret").expect("turret");
    let footprints = |app: &mut App, player| {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        let mut found: Vec<(sim::BuildingKind, CellPos)> = query
            .iter(app.world())
            .filter(|(owner, _)| owner.0 == player)
            .map(|(_, building)| (building.kind, building.footprint.origin))
            .collect();
        found.sort_unstable();
        found
    };
    assert_eq!(footprints(&mut app, P0), [(yard, CellPos::new(5, 5))]);
    assert_eq!(
        footprints(&mut app, P1),
        [(yard, CellPos::new(32, 32)), (turret, CellPos::new(30, 33))]
    );
    let mut units = app.world_mut().query::<(&Owner, &Unit, &Mobile)>();
    let tank = data.definitions.unit_kind("tank").expect("tank");
    let rifleman = data.definitions.unit_kind("rifleman").expect("rifleman");
    let mut counts = std::collections::BTreeMap::new();
    let mut cells = BTreeSet::new();
    for (owner, unit, mobile) in units.iter(app.world()) {
        *counts.entry((owner.0, unit.kind)).or_insert(0) += 1;
        assert!(cells.insert(mobile.cell), "one unit per cell (I6)");
        // Units stand around their group's anchor: within a few cells.
        let anchor = match (owner.0, unit.kind) {
            (P0, kind) if kind == tank => CellPos::new(9, 6),
            (P0, _) => CellPos::new(6, 9),
            _ => CellPos::new(36, 33),
        };
        assert!(
            (mobile.cell.x - anchor.x).abs() <= 2 && (mobile.cell.y - anchor.y).abs() <= 2,
            "{:?} far from {anchor:?}",
            mobile.cell
        );
    }
    assert_eq!(counts.get(&(P0, tank)), Some(&2));
    assert_eq!(counts.get(&(P0, rifleman)), Some(&2));
    assert_eq!(counts.get(&(P1, rifleman)), Some(&3));
    let players = app.world().resource::<Players>();
    assert_eq!(players.get(P0).map(|state| state.credits()), Some(1500));
    assert_eq!(players.get(P1).map(|state| state.credits()), Some(1500));
    assert_eq!(app.world().resource::<MatchState>().players(), [P0, P1]);
    let ai = app.world().resource::<AiPlayers>();
    assert_eq!(ai.players().collect::<Vec<_>>(), [P1]);
    let controller = ai.controller(P1).expect("the computer player");
    assert_eq!(controller.start(), CellPos::new(32, 32));
    assert_eq!(controller.difficulty().id, "normal");
    // The enemy start comes first among the waypoints, marked as a priority.
    assert_eq!(controller.waypoints()[0].cell, CellPos::new(5, 5));
    assert!(controller.waypoints()[0].priority);
    assert!(
        controller.waypoints()[1..]
            .iter()
            .all(|waypoint| !waypoint.priority)
    );
    // The gathering point lies toward the centre of the map, on passable ground.
    let gather = controller.gather();
    assert!(gather.x < 32 && gather.y < 32, "{gather:?}");
    assert!(app.world().resource::<sim::Grid>().is_passable(gather));
}

#[test]
fn setup_errors_are_typed_and_change_nothing_when_the_configuration_is_wrong() {
    let data = data();
    let ai = |difficulty| Controller::Ai { difficulty };
    let cases: Vec<(usize, Vec<PlayerSetup>, MatchSetupError)> = vec![
        (
            1,
            vec![human(P0, 0, 0), human(P1, 1, 1)],
            MatchSetupError::UnknownMap(1),
        ),
        (
            0,
            vec![human(P0, 0, 0)],
            MatchSetupError::Players("1 players, a skirmish match on this map has 2 to 2".into()),
        ),
        (
            0,
            vec![human(P0, 0, 0), human(P0, 1, 1)],
            MatchSetupError::Players("player 0 is given twice".into()),
        ),
        (
            0,
            vec![human(P0, 0, 0), human(P1, 1, 0)],
            MatchSetupError::Players("start position 0 is given twice".into()),
        ),
        (
            0,
            vec![human(P0, 2, 0), human(P1, 1, 1)],
            MatchSetupError::UnknownFaction {
                player: P0,
                faction: 2,
            },
        ),
        (
            0,
            vec![human(P0, 0, 0), human(P1, 1, 2)],
            MatchSetupError::UnknownStart {
                player: P1,
                start: 2,
            },
        ),
        (
            0,
            vec![
                human(P0, 0, 0),
                PlayerSetup {
                    player: P1,
                    faction: 1,
                    start: 1,
                    controller: ai(2),
                },
            ],
            MatchSetupError::UnknownDifficulty {
                player: P1,
                difficulty: 2,
            },
        ),
    ];
    for (map, players, expected) in cases {
        let mut app = world(&data);
        let before = state_hash(app.world_mut());
        let error = setup_match(app.world_mut(), &data.skirmish, &data.ai, map, &players)
            .expect_err("rejected");
        assert_eq!(error, expected);
        assert!(!error.to_string().is_empty());
        // Configuration errors are found before anything is changed.
        assert_eq!(state_hash(app.world_mut()), before, "{error}");
    }
    // Without a map installed.
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(data.definitions.clone());
    assert_eq!(
        setup_match(
            app.world_mut(),
            &data.skirmish,
            &data.ai,
            0,
            &[human(P0, 0, 0), human(P1, 1, 1)]
        ),
        Err(MatchSetupError::NotInstalled)
    );
}

#[test]
fn setup_errors_name_a_building_or_unit_group_that_does_not_fit() {
    let data = data();
    // A building that would stick out of the map at a start near the edge.
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[0].buildings.push(StartingBuilding {
        kind: data.definitions.building_kind("power").expect("power"),
        offset: (-6, 0),
    });
    let mut app = world(&data);
    let error = setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), human(P1, 1, 1)],
    )
    .expect_err("outside the map");
    let MatchSetupError::Building {
        player,
        ref building,
        origin,
        ..
    } = error
    else {
        panic!("{error:?}");
    };
    assert_eq!(
        (player, building.as_str(), origin),
        (P0, "power", CellPos::new(-1, 5))
    );
    assert!(
        error
            .to_string()
            .contains("starting building \"power\" of player 0 at (-1, 5)")
    );
    // A building on rock: the fixture map has rock at (25..=27, 9).
    let mut skirmish = data.skirmish.clone();
    skirmish.maps[0].starts[0] = CellPos::new(25, 8);
    let mut app = world(&data);
    let error = setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), human(P1, 1, 1)],
    )
    .expect_err("rock");
    assert!(
        matches!(error, MatchSetupError::Building { player: P0, .. }),
        "{error:?}"
    );
    // A group of units anchored outside the map.
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[0].units[0].offset = (-32, 0);
    let mut app = world(&data);
    let error = setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), human(P1, 1, 1)],
    )
    .expect_err("anchor outside");
    assert_eq!(
        error,
        MatchSetupError::Units(StartError {
            owner: P0,
            unit_type: "tank".into(),
            anchor: CellPos::new(-27, 5),
            found: 0,
            required: 2,
        })
    );
}

/// The fixture skirmish with faction 1's start position moved `shift` cells east of (32, 32) and
/// its buildings and units moved back by as much, so that they stand where they stand in the
/// fixture, whatever the start position is.
fn shifted_start(data: &Data, shift: i32) -> SkirmishDefinitions {
    let building = |id| data.definitions.building_kind(id).expect("building");
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[1].buildings = vec![
        StartingBuilding {
            kind: building("yard"),
            offset: (-shift, 0),
        },
        StartingBuilding {
            kind: building("turret"),
            offset: (-2 - shift, 1),
        },
    ];
    skirmish.factions[1].units = vec![StartingGroup {
        kind: data.definitions.unit_kind("rifleman").expect("rifleman"),
        offset: (4 - shift, 1),
        count: 3,
    }];
    skirmish.maps[0].starts[1] = CellPos::new(32 + shift, 32);
    skirmish
}

#[test]
fn a_start_position_that_is_not_a_passable_cell_of_the_map_is_a_setup_error() {
    // M6 review 1 P1-1: a start position outside the map, or on impassable terrain, was accepted
    // whenever the faction's buildings and units, placed at offsets from it, lay on the map; the
    // computer player then derived its gathering point and waypoints from it, and every command
    // naming them was rejected as `OutsideMap`. Both are configuration errors now, found before
    // anything is changed, for the human player and the computer player alike.
    let data = data();
    let rock = sim::Cell {
        water: false,
        passable: false,
        buildable: false,
        ore: 0,
    };
    // The same buildings and units as the fixture's, from (52, 32) on the 40 x 40 map; on the
    // reviewed code the setup succeeded and the computer player gathered at (52, 32).
    let outside = shifted_start(&data, 20);
    // From (36, 32), a cell of the map turned to rock.
    let on_rock = shifted_start(&data, 4);
    let cases = [
        (
            outside.clone(),
            None,
            MatchSetupError::InvalidStart {
                start: 1,
                cell: CellPos::new(52, 32),
                problem: StartProblem::OutsideMap {
                    width: 40,
                    height: 40,
                },
            },
            "start position 1 at (52, 32) is outside the 40 x 40 cells of the map",
        ),
        (
            on_rock.clone(),
            Some(CellPos::new(36, 32)),
            MatchSetupError::InvalidStart {
                start: 1,
                cell: CellPos::new(36, 32),
                problem: StartProblem::Impassable,
            },
            "start position 1 at (36, 32) is impassable terrain of the map",
        ),
    ];
    for (skirmish, rock_at, expected, message) in cases {
        for players in [
            [human(P0, 0, 0), computer(P1, 1, 1, 0)],
            [human(P0, 0, 0), human(P1, 1, 1)],
            // The other player's start is checked too.
            [computer(P0, 0, 0, 1), human(P1, 1, 1)],
        ] {
            let mut app = world(&data);
            if let Some(cell) = rock_at {
                app.world_mut()
                    .resource_mut::<sim::Grid>()
                    .set_terrain(cell, rock);
            }
            let before = state_hash(app.world_mut());
            let error = setup_match(app.world_mut(), &skirmish, &data.ai, 0, &players)
                .expect_err("a start position off the map's passable cells is rejected");
            assert_eq!(error, expected, "{players:?}");
            assert!(error.to_string().contains(message), "{error}");
            assert_eq!(state_hash(app.world_mut()), before, "{error}");
        }
    }
    // The same buildings and units from a start position on the map are accepted, so only the
    // start position is at fault above.
    let mut app = world(&data);
    setup_match(
        app.world_mut(),
        &shifted_start(&data, 2),
        &data.ai,
        0,
        &[human(P0, 0, 0), computer(P1, 1, 1, 0)],
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn a_computer_player_whose_faction_lacks_a_producer_of_its_build_order_is_a_setup_error() {
    // M6 review 1 P2-1, the class of unobtainable buildings: a computer player whose faction
    // starts without the producer of its build order's buildings (the yard) could never build
    // anything. A human player may play such a faction.
    let data = data();
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[1].buildings = vec![StartingBuilding {
        kind: data.definitions.building_kind("power").expect("power"),
        offset: (0, 0),
    }];
    let mut app = world(&data);
    let before = state_hash(app.world_mut());
    let error = setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), computer(P1, 1, 1, 0)],
    )
    .expect_err("the computer player could not build");
    assert_eq!(
        error,
        MatchSetupError::UnobtainableProducer {
            player: P1,
            field: "plans[0].build_order[0]".into(),
            building: "power".into(),
            producer: "yard".into(),
        }
    );
    assert!(
        error
            .to_string()
            .contains("computer player 1: the AI file's plans[0].build_order[0] builds \"power\""),
        "{error}"
    );
    assert_eq!(state_hash(app.world_mut()), before);
    let mut app = world(&data);
    setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[computer(P0, 0, 0, 0), human(P1, 1, 1)],
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn a_computer_player_without_a_reachable_gathering_point_is_a_setup_error() {
    // M6 review 1 P1-1, the class of fallbacks to unusable cells: the gathering point fell back to
    // the start position when no passable cell was found, here a cell under the computer player's
    // own yard. Its yard is walled in by rock, so no cell free of buildings is reachable.
    let data = data();
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[1].buildings.truncate(1);
    skirmish.factions[1].units.clear();
    let rock = sim::Cell {
        water: false,
        passable: false,
        buildable: false,
        ore: 0,
    };
    let mut app = world(&data);
    {
        let mut grid = app.world_mut().resource_mut::<sim::Grid>();
        // The yard covers (32..=34, 32..=34).
        for y in 31..=35 {
            for x in 31..=35 {
                if !(32..=34).contains(&x) || !(32..=34).contains(&y) {
                    grid.set_terrain(CellPos::new(x, y), rock);
                }
            }
        }
    }
    let error = setup_match(
        app.world_mut(),
        &skirmish,
        &data.ai,
        0,
        &[human(P0, 0, 0), computer(P1, 1, 1, 0)],
    )
    .expect_err("no gathering point");
    assert_eq!(
        error,
        MatchSetupError::NoGatheringPoint {
            player: P1,
            start: CellPos::new(32, 32),
        }
    );
    assert!(error.to_string().contains("no gathering point"), "{error}");
}

#[test]
fn every_gathering_point_and_waypoint_of_a_computer_player_is_a_passable_cell_of_the_map() {
    // The cells a computer player orders its units to are derived at setup from the start
    // positions and the terrain (M6 review 1 P1-1): all of them lie on the map's passable terrain,
    // and the gathering point is free of buildings.
    let data = data();
    for shift in [0, 2] {
        let mut app = world(&data);
        setup_match(
            app.world_mut(),
            &shifted_start(&data, shift),
            &data.ai,
            0,
            &[computer(P0, 0, 0, 0), computer(P1, 1, 1, 1)],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let grid = app.world().resource::<sim::Grid>();
        let ai = app.world().resource::<AiPlayers>();
        assert_eq!(ai.players().count(), 2);
        for player in ai.players() {
            let controller = ai.controller(player).expect("controller");
            assert!(grid.is_passable(controller.gather()), "{player:?}");
            for waypoint in controller.waypoints() {
                assert!(
                    grid.cell(waypoint.cell).is_some_and(|cell| cell.passable),
                    "{player:?}: {waypoint:?}"
                );
            }
        }
    }
}

#[test]
fn starting_units_are_planned_on_distinct_free_cells_or_rejected() {
    let data = data();
    let app = world(&data);
    let grid = app.world().resource::<sim::Grid>();
    let planned = plan_starting_units(
        grid,
        &[(P0, "tank", (10, 10), 9), (P1, "tank", (10, 10), 3)],
    )
    .expect("open ground");
    assert_eq!(planned.len(), 12);
    let cells: BTreeSet<CellPos> = planned.iter().map(|&(.., cell)| cell).collect();
    assert_eq!(cells.len(), 12, "distinct cells");
    // The rock bar at (17..=22, 17..=18) and (17..=22, 21..=22) leaves a passage of two rows; an
    // anchor on rock uses the passable cells nearest to it.
    let on_rock = plan_starting_units(grid, &[(P0, "tank", (18, 17), 2)]).expect("around rock");
    assert!(on_rock.iter().all(|&(.., cell)| grid.is_passable(cell)));
    let error = plan_starting_units(grid, &[(P0, "tank", (40, 3), 1)]).expect_err("outside");
    assert_eq!(error.found, 0);
    assert!(error.to_string().contains("outside the map or walled in"));
}

// --- Win condition --------------------------------------------------------------------------

#[test]
fn a_player_without_buildings_is_defeated_and_the_last_player_wins() {
    let data = data();
    let mut app = two_humans(&data);
    // Both have buildings: nothing happens.
    for _ in 0..5 {
        let events = tick(&mut app, Vec::new());
        assert!(
            !events.iter().any(|event| matches!(
                event,
                SimEvent::PlayerDefeated { .. } | SimEvent::MatchEnded { .. }
            )),
            "{events:?}"
        );
    }
    // P1 sells one of its two buildings: still in the match.
    let (first, _) = buildings_of(&mut app, P1)[0];
    let events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P1,
            command: Command::SellBuilding { building: first },
        }],
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::BuildingSold { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::PlayerDefeated { .. }))
    );
    assert_eq!(app.world().resource::<MatchState>().outcome(), None);
    // It sells its last building: defeated in that tick, and P0 wins.
    let tick_index = app.world().resource::<sim::SimTick>().0;
    let sell = sell_all(&mut app, P1);
    let events = tick(&mut app, sell);
    let ends: Vec<&SimEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                SimEvent::PlayerDefeated { .. } | SimEvent::MatchEnded { .. }
            )
        })
        .collect();
    assert_eq!(
        ends,
        [
            &SimEvent::PlayerDefeated { player: P1 },
            &SimEvent::MatchEnded { winner: Some(P0) }
        ]
    );
    let state = app.world().resource::<MatchState>().clone();
    assert_eq!(state.defeated(P1), Some(tick_index));
    assert_eq!(state.defeated(P0), None);
    assert_eq!(
        state.outcome(),
        Some(MatchOutcome::Won {
            winner: P0,
            tick: tick_index
        })
    );
    // The outcome is final: P0 loses its buildings later, and nothing changes or is reported.
    let sell = sell_all(&mut app, P0);
    for commands in [sell, Vec::new(), Vec::new()] {
        let events = tick(&mut app, commands);
        assert!(
            !events.iter().any(|event| matches!(
                event,
                SimEvent::PlayerDefeated { .. } | SimEvent::MatchEnded { .. }
            )),
            "{events:?}"
        );
    }
    assert_eq!(
        app.world().resource::<MatchState>().outcome(),
        state.outcome()
    );
    assert_eq!(app.world().resource::<MatchState>().defeated(P0), None);
}

#[test]
fn players_who_lose_their_last_buildings_in_the_same_tick_draw() {
    let data = data();
    let mut app = two_humans(&data);
    tick(&mut app, Vec::new());
    let mut sell = sell_all(&mut app, P0);
    sell.extend(sell_all(&mut app, P1));
    let events = tick(&mut app, sell);
    let ends: Vec<&SimEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                SimEvent::PlayerDefeated { .. } | SimEvent::MatchEnded { .. }
            )
        })
        .collect();
    assert_eq!(
        ends,
        [
            &SimEvent::PlayerDefeated { player: P0 },
            &SimEvent::PlayerDefeated { player: P1 },
            &SimEvent::MatchEnded { winner: None }
        ]
    );
    assert!(matches!(
        app.world().resource::<MatchState>().outcome(),
        Some(MatchOutcome::Draw { tick: 1 })
    ));
}

#[test]
fn a_destroyed_last_building_defeats_its_owner() {
    // Ten tanks of P0 attack-move into P1's base (all through commands); P1's riflemen hardly hurt
    // tanks and its turret is its only defence. P1 is defeated in the tick its last building is
    // destroyed.
    let mut data = data();
    data.skirmish.factions[0].units[0].count = 10;
    let mut app = two_humans(&data);
    let tanks: Vec<SimId> = {
        let tank = data.definitions.unit_kind("tank").expect("tank");
        let mut query = app.world_mut().query::<(&SimId, &Owner, &Unit)>();
        let mut found: Vec<SimId> = query
            .iter(app.world())
            .filter(|(_, owner, unit)| owner.0 == P0 && unit.kind == tank)
            .map(|(id, ..)| *id)
            .collect();
        found.sort_unstable();
        found
    };
    let first = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::AttackMove {
                units: tanks.clone(),
                // The yard's left edge (its centre's neighbours are all yard cells).
                target: CellPos::new(32, 33),
            },
        }],
    );
    assert!(
        !first.iter().any(|event| matches!(
            event,
            SimEvent::MoveFailed { .. } | SimEvent::CommandRejected { .. }
        )),
        "{first:?}"
    );
    let mut destroyed = BTreeSet::new();
    let mut ended = None;
    for _ in 0..6_000 {
        let events = tick(&mut app, Vec::new());
        for event in &events {
            match event {
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => {
                    assert_eq!(*player, P1);
                    destroyed.insert(*building);
                }
                SimEvent::MatchEnded { winner } => ended = Some(*winner),
                _ => {}
            }
        }
        if ended.is_some() {
            assert!(buildings_of(&mut app, P1).is_empty());
            assert_eq!(destroyed.len(), 2);
            break;
        }
        // While P1 owns a building it is not defeated.
        assert!(!buildings_of(&mut app, P1).is_empty());
        assert_eq!(app.world().resource::<MatchState>().defeated(P1), None);
    }
    assert_eq!(ended, Some(Some(P0)), "the attack ends the match");
}

#[test]
fn without_a_match_set_up_there_is_no_win_condition() {
    // A world set up only by `SimPlugin`, with players who own no buildings (the M3 to M5 tests'
    // setup): nobody is defeated.
    let data = data();
    let mut app = world(&data);
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(P0, 0).expect("player");
        players.add(P1, 0).expect("player");
    }
    for _ in 0..10 {
        let events = tick(&mut app, Vec::new());
        assert!(events.is_empty(), "{events:?}");
    }
    assert_eq!(app.world().resource::<MatchState>().players(), []);
    assert_eq!(app.world().resource::<MatchState>().outcome(), None);
}

#[test]
fn the_hash_covers_the_match_and_the_computer_players() {
    let data = data();
    let setups = |difficulty| {
        [
            human(P0, 0, 0),
            PlayerSetup {
                player: P1,
                faction: 1,
                start: 1,
                controller: Controller::Ai { difficulty },
            },
        ]
    };
    let hash_of = |players: &[PlayerSetup]| {
        let mut app = world(&data);
        setup_match(app.world_mut(), &data.skirmish, &data.ai, 0, players).expect("setup");
        state_hash(app.world_mut())
    };
    let easy = hash_of(&setups(0));
    assert_eq!(easy, hash_of(&setups(0)));
    assert_ne!(easy, hash_of(&setups(1)), "the difficulty is hashed");
    assert_ne!(
        easy,
        hash_of(&[human(P0, 0, 0), human(P1, 1, 1)]),
        "the controllers are hashed"
    );
    // The outcome is hashed: the same world before and after P1 is defeated by selling.
    let mut app = two_humans(&data);
    tick(&mut app, Vec::new());
    let mut other = two_humans(&data);
    tick(&mut other, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(other.world_mut()));
    let sell = sell_all(&mut app, P1);
    tick(&mut app, sell);
    tick(&mut other, Vec::new());
    assert_ne!(state_hash(app.world_mut()), state_hash(other.world_mut()));
}

#[test]
fn super_me_is_human_opt_in_and_part_of_deterministic_state() {
    let d = data();
    let players = [human(P0, 0, 0), computer(P1, 1, 1, 1)];
    let mut normal = world(&d);
    setup_match(normal.world_mut(), &d.skirmish, &d.ai, 0, &players).unwrap();
    let mut boosted = world(&d);
    sim::setup_match_with_super_me(
        boosted.world_mut(),
        &d.skirmish,
        &d.ai,
        0,
        &players,
        &[P0, P1],
    )
    .unwrap();
    let states = boosted.world().resource::<Players>();
    assert!(states.get(P0).unwrap().super_me());
    assert!(
        !states.get(P1).unwrap().super_me(),
        "AI must reject bonuses even if requested"
    );
    assert_ne!(
        state_hash(normal.world_mut()),
        state_hash(boosted.world_mut())
    );
    for _ in 0..200 {
        tick(&mut boosted, vec![]);
    }
    assert!(
        boosted
            .world()
            .resource::<AiPlayers>()
            .controller(P1)
            .is_some()
    );
    // A fresh/restarted normal match never inherits bonuses from the preceding game.
    let restarted = two_humans(&d);
    assert!(
        restarted
            .world()
            .resource::<Players>()
            .iter()
            .all(|(_, s)| !s.super_me())
    );
}

#[test]
fn computer_armies_grow_with_the_map_on_large_maps_only() {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    // The shipped AI file has a plan per side of the shipped rules (RA1), so it is loaded
    // against the whole shipped ruleset.
    let defs = sim::load_ruleset(assets.join("data")).expect("shipped definitions");
    let ai = sim::load_ai(assets.join("data/ai.ron"), &defs).expect("shipped AI");
    let grid = |id: &str| {
        sim::load_map(assets.join(sim::map_asset_path(id)))
            .expect("shipped map")
            .grid
    };
    for difficulty in &ai.difficulties {
        // Up to ARMY_SCALE_SPAN cells (every M7 map), the AI file's values apply unchanged.
        for id in ["demo", "crossroads", "riverlands"] {
            assert_eq!(
                &sim::scaled_for_map(difficulty.clone(), &grid(id)),
                difficulty
            );
        }
        // 128 + 128 cells and more (every large shipped map, up to 256 + 256): twice the army
        // and the wave, and no more.
        let open = |side: u32| {
            let cell = sim::Cell {
                passable: true,
                water: false,
                buildable: true,
                ore: 0,
            };
            sim::Grid::from_cells(side, side, vec![cell; (side * side) as usize]).expect("a grid")
        };
        for big in [open(128), grid("liberty_bay"), grid("red_canyon")] {
            let big = sim::scaled_for_map(difficulty.clone(), &big);
            assert_eq!(big.max_army, difficulty.max_army * 2);
            assert_eq!(big.wave_size, difficulty.wave_size * 2);
            assert!(big.wave_size <= big.max_army && big.max_army <= sim::MAX_AI_ARMY);
            assert_eq!(big.think_interval, difficulty.think_interval);
        }
        // 112 + 112 cells: in between.
        let mid = sim::scaled_for_map(difficulty.clone(), &open(112));
        assert_eq!(mid.max_army, difficulty.max_army * 175 / 100);
    }
}
