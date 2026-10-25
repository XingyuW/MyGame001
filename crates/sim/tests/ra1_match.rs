//! RA1: match setup for countries, the match options, the win conditions, undeploying, and the
//! computer player's start (docs/RA2_PARITY_PLAN.md sections 3.5 and 4.9 and milestone RA1;
//! docs/DECISIONS.md R12 to R14).
//!
//! Everything runs on the RA1 fixture (`tests/fixtures/data/ra1`, a small world with two sides
//! and three countries; invariant I10) and the fixture map `maps/skirmish_40x40.tmx`, through
//! `setup_match_configured`, `CommandQueue` and `SimSchedule`. With the fixture's costs the
//! mean starting cost is (400 + 100 + 650 + 50) / 4 = 300, its lobby offers 1000 to 5000
//! credits in steps of 500 and unit counts 0 to 10 with a default of 4.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use sim::{
    AI_TEAM, AiDefinitions, AiPlayers, Building, CellPos, Command, CommandQueue, Controller,
    Definitions, Footprint, Grid, Health, IssuedCommand, MatchOptions, MatchOutcome, MatchRules,
    MatchSetupError, MatchStart, MatchState, Mobile, Owner, PlayerId, PlayerSetup, Players,
    ProductionItem, RejectReason, STARTING_UNIT_DISTANCE, Side, SimEvent, SimId, SimPlugin, SimRng,
    SimSchedule, SkirmishDefinitions, Unit, UnitKind, load_ai, load_map, load_match_definitions,
    load_skirmish, setup_match_configured, spawn_building, spawn_unit, starting_units, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

/// The factions of the fixture's skirmish file.
const ALPHA: usize = 0;
const BETA: usize = 1;
const GAMMA: usize = 2;

/// The start positions of the fixture map.
const START_0: CellPos = CellPos { x: 5, y: 5 };
const START_1: CellPos = CellPos { x: 32, y: 32 };

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let dir = fixtures().join("data/ra1");
    let definitions = load_match_definitions(
        dir.join("units.ron"),
        dir.join("economy.ron"),
        dir.join("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(dir.join("rules.ron")))
    .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish = load_skirmish(dir.join("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai = load_ai(dir.join("ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    sim::check_ai_factions(dir.join("ai.ron"), &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

fn world(data: &Data, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    let grid = load_map(fixtures().join("maps/skirmish_40x40.tmx"))
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

/// A match of `players` on the fixture map with `options`.
fn setup(
    data: &Data,
    players: &[PlayerSetup],
    options: &MatchOptions,
    seed: u64,
) -> Result<(App, MatchStart), MatchSetupError> {
    let mut app = world(data, seed);
    let start = setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        players,
        options,
    )?;
    Ok((app, start))
}

/// Alpha (P0) against gamma (P1), both human.
fn duel(data: &Data, options: &MatchOptions) -> App {
    setup(
        data,
        &[human(P0, ALPHA, 0), human(P1, GAMMA, 1)],
        options,
        7,
    )
    .unwrap_or_else(|error| panic!("{error}"))
    .0
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

fn issued(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

/// No rejection.
const NONE: [RejectReason; 0] = [];

fn unit_kind(data: &Data, id: &str) -> UnitKind {
    data.definitions
        .unit_kind(id)
        .unwrap_or_else(|| panic!("unit {id}"))
}

/// The units of `player`: id, type name, and cell, in id order.
fn units(app: &mut App, player: PlayerId) -> Vec<(SimId, String, CellPos)> {
    let definitions = app.world().resource::<Definitions>().clone();
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Unit, &Mobile)>();
    let mut found: Vec<(SimId, String, CellPos)> = query
        .iter(app.world())
        .filter(|(_, owner, ..)| owner.0 == player)
        .map(|(id, _, unit, mobile)| {
            (
                *id,
                definitions.unit(unit.kind).expect("unit").id.clone(),
                mobile.cell,
            )
        })
        .collect();
    found.sort_unstable();
    found
}

/// How many units of each type `player` owns.
fn unit_counts(app: &mut App, player: PlayerId) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (_, id, _) in units(app, player) {
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
}

fn counts(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|(id, count)| (id.to_string(), *count))
        .collect()
}

/// The buildings of `player`: id, type name, and footprint, in id order.
fn buildings(app: &mut App, player: PlayerId) -> Vec<(SimId, String, Footprint)> {
    let definitions = app.world().resource::<Definitions>().clone();
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
    let mut found: Vec<(SimId, String, Footprint)> = query
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == player)
        .map(|(id, _, building)| {
            (
                *id,
                definitions
                    .building(building.kind)
                    .expect("building")
                    .id
                    .clone(),
                building.footprint,
            )
        })
        .collect();
    found.sort_unstable_by_key(|(id, ..)| *id);
    found
}

fn entity(app: &mut App, id: SimId) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &SimId)>();
    query
        .iter(app.world())
        .find(|(_, found)| **found == id)
        .unwrap_or_else(|| panic!("entity {id:?}"))
        .0
}

fn hit_points(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id);
    app.world()
        .get::<Health>(entity)
        .expect("health")
        .hit_points
}

/// Scenario setup: sets the hit points of `id`.
fn set_hit_points(app: &mut App, id: SimId, value: i32) {
    let entity = entity(app, id);
    app.world_mut()
        .get_mut::<Health>(entity)
        .expect("health")
        .hit_points = value;
}

fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .credits()
}

fn distance(a: CellPos, b: CellPos) -> u32 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

fn base_unit(app: &mut App, player: PlayerId) -> SimId {
    units(app, player)
        .into_iter()
        .find(|(_, id, _)| id.ends_with("_mcv"))
        .expect("a base unit")
        .0
}

// --- Positive: the start of a match --------------------------------------------------------------

#[test]
fn a_country_starts_with_its_base_unit_and_the_starting_units_of_its_side() {
    let data = data();
    let (mut app, start) = setup(
        &data,
        &[human(P0, ALPHA, 0), human(P1, GAMMA, 1)],
        &MatchOptions::default(),
        7,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    // No building; the base unit at the start position; the starting units of the lobby's
    // default unit count (4: a budget of 3 x 300 = 900): two tanks (800 >= 600) and a rifleman
    // for the Allied player, a heavy tank (650 >= 600) and five conscripts for the Soviet one.
    assert_eq!(
        start,
        MatchStart {
            buildings: 0,
            units: 4 + 7
        }
    );
    assert!(buildings(&mut app, P0).is_empty() && buildings(&mut app, P1).is_empty());
    assert_eq!(
        unit_counts(&mut app, P0),
        counts(&[("allied_mcv", 1), ("tank", 2), ("rifleman", 1)])
    );
    assert_eq!(
        unit_counts(&mut app, P1),
        counts(&[("soviet_mcv", 1), ("heavy", 1), ("conscript", 5)])
    );
    for (player, start) in [(P0, START_0), (P1, START_1)] {
        for (_, id, cell) in units(&mut app, player) {
            if id.ends_with("_mcv") {
                assert_eq!(cell, start, "{id}");
            } else {
                assert!(
                    distance(cell, start) >= STARTING_UNIT_DISTANCE,
                    "{id} at {cell:?}"
                );
                assert!(
                    distance(cell, start) <= STARTING_UNIT_DISTANCE + 2,
                    "{id} at {cell:?}"
                );
            }
        }
    }
    // The skirmish file's credits; the players' countries; no roster; the lobby's rules.
    let rules = data.definitions.rules().expect("rules");
    let players = app.world().resource::<Players>();
    for (player, country) in [(P0, "alpha"), (P1, "gamma")] {
        let state = players.get(player).expect("player");
        assert_eq!(state.credits(), 3000);
        assert_eq!(state.country(), rules.country_id(country));
        assert_eq!(state.roster(), None);
        assert_eq!(state.team(), 0);
    }
    assert_eq!(
        app.world().resource::<MatchState>().rules(),
        MatchRules::default()
    );
    // Nobody is defeated for owning no building: the base unit counts in a short game.
    for _ in 0..5 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(app.world().resource::<MatchState>().outcome(), None);
    assert_eq!(app.world().resource::<MatchState>().defeated(P0), None);
}

#[test]
fn without_an_mcv_start_the_construction_yard_stands_at_the_start_position() {
    let data = data();
    let options = MatchOptions {
        mcv_start: false,
        unit_count: Some(10),
        ..MatchOptions::default()
    };
    let (mut app, start) = setup(
        &data,
        &[human(P0, BETA, 0), human(P1, GAMMA, 1)],
        &options,
        7,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    // Unit count 10: a budget of 9 x 300 = 2700. Allied: five tanks (2000 >= 1800), then seven
    // riflemen; Soviet: three heavy tanks (1950 >= 1800), then fifteen conscripts.
    assert_eq!(
        start,
        MatchStart {
            buildings: 2,
            units: 12 + 18
        }
    );
    for (player, start, yard, expected) in [
        (
            P0,
            START_0,
            "allied_yard",
            counts(&[("tank", 5), ("rifleman", 7)]),
        ),
        (
            P1,
            START_1,
            "soviet_yard",
            counts(&[("heavy", 3), ("conscript", 15)]),
        ),
    ] {
        let owned = buildings(&mut app, player);
        assert_eq!(owned.len(), 1);
        let (_, id, footprint) = &owned[0];
        assert_eq!((id.as_str(), footprint.origin), (yard, start));
        assert_eq!(unit_counts(&mut app, player), expected);
        for (_, id, cell) in units(&mut app, player) {
            assert!(
                distance(cell, start) >= STARTING_UNIT_DISTANCE,
                "{id} at {cell:?}"
            );
            assert!(!footprint.contains(cell), "{id} stands in the yard");
        }
    }
}

#[test]
fn deploying_the_base_unit_in_place_puts_the_yard_where_a_yard_start_puts_it() {
    // `map_scale` and the start positions of the skirmish file assume that the yard's footprint
    // begins at the start position; an MCV that deploys where it starts must produce that.
    let data = data();
    let mut app = duel(&data, &MatchOptions::default());
    tick(&mut app, Vec::new());
    let mcv = base_unit(&mut app, P0);
    let events = tick(&mut app, vec![issued(P0, Command::Deploy { unit: mcv })]);
    assert_eq!(rejections(&events), NONE);
    let yard_kind = data.definitions.building_kind("allied_yard").expect("yard");
    assert!(events.contains(&SimEvent::BuildingPlaced {
        player: P0,
        building: mcv,
        kind: yard_kind,
    }));
    let deployed = buildings(&mut app, P0);
    let mut yard_start = duel(
        &data,
        &MatchOptions {
            mcv_start: false,
            ..MatchOptions::default()
        },
    );
    let placed = buildings(&mut yard_start, P0);
    assert_eq!(deployed.len(), 1);
    assert_eq!(
        (deployed[0].1.as_str(), deployed[0].2),
        (placed[0].1.as_str(), placed[0].2)
    );
    assert_eq!(deployed[0].2.origin, START_0);
    // The whole unit became a whole yard, and the base unit is gone.
    assert_eq!(hit_points(&mut app, mcv), 1000);
    assert!(!unit_counts(&mut app, P0).contains_key("allied_mcv"));
    // A player with a yard and no base unit lives on.
    for _ in 0..3 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(app.world().resource::<MatchState>().defeated(P0), None);
}

// --- Boundary: the options ------------------------------------------------------------------------

#[test]
fn starting_units_follow_the_budget_of_the_unit_count() {
    let data = data();
    let mut rng = SimRng::new(1);
    let mut names = |side: Side, count: u32| -> BTreeMap<String, usize> {
        let mut found = BTreeMap::new();
        for kind in starting_units(&data.skirmish, &data.definitions, side, count, &mut rng) {
            let id = data.definitions.unit(kind).expect("unit").id.clone();
            *found.entry(id).or_insert(0) += 1;
        }
        found
    };
    assert_eq!(data.skirmish.average_starting_cost(&data.definitions), 300);
    // 0 and 1: no budget. 2: 300, spent by one vehicle. 4: the default. 10: the largest.
    for side in Side::ALL {
        assert!(names(side, 0).is_empty() && names(side, 1).is_empty());
    }
    assert_eq!(names(Side::Allied, 2), counts(&[("tank", 1)]));
    assert_eq!(names(Side::Soviet, 2), counts(&[("heavy", 1)]));
    assert_eq!(
        names(Side::Allied, 3),
        counts(&[("tank", 1), ("rifleman", 2)])
    );
    assert_eq!(
        names(Side::Allied, 4),
        counts(&[("tank", 2), ("rifleman", 1)])
    );
    assert_eq!(
        names(Side::Soviet, 4),
        counts(&[("heavy", 1), ("conscript", 5)])
    );
    assert_eq!(
        names(Side::Allied, 10),
        counts(&[("tank", 5), ("rifleman", 7)])
    );
    assert_eq!(
        names(Side::Soviet, 10),
        counts(&[("heavy", 3), ("conscript", 15)])
    );
    // With one type per list nothing is drawn: the generator is untouched.
    assert_eq!(rng.next_u64(), SimRng::new(1).next_u64());
    // A side with several types draws among them, deterministically by seed.
    let mut varied = data.skirmish.clone();
    varied.sides[0].infantry.push(unit_kind(&data, "raider"));
    let draw = |seed: u64| {
        starting_units(
            &varied,
            &data.definitions,
            Side::Allied,
            10,
            &mut SimRng::new(seed),
        )
    };
    assert_eq!(draw(3), draw(3));
    assert!((0..16).any(|seed| draw(seed) != draw(seed + 1)));
    assert!(
        draw(3).contains(&unit_kind(&data, "raider"))
            || draw(4).contains(&unit_kind(&data, "raider"))
    );
}

#[test]
fn the_unit_count_is_checked_against_the_range_of_the_rules() {
    let data = data();
    let players = [human(P0, ALPHA, 0), human(P1, GAMMA, 1)];
    let with = |count: u32| MatchOptions {
        unit_count: Some(count),
        ..MatchOptions::default()
    };
    // 0 and 10 are the limits of the fixture's lobby: only the base units, or the full groups.
    let (mut none, start) = setup(&data, &players, &with(0), 7).expect("unit count 0");
    assert_eq!(start.units, 2);
    assert_eq!(unit_counts(&mut none, P0), counts(&[("allied_mcv", 1)]));
    let (_, start) = setup(&data, &players, &with(1), 7).expect("unit count 1");
    assert_eq!(start.units, 2);
    let (_, start) = setup(&data, &players, &with(10), 7).expect("unit count 10");
    assert_eq!(start.units, 2 + 12 + 18);
    // One above is rejected before anything changes.
    let mut app = world(&data, 7);
    let before = state_hash(app.world_mut());
    let error = setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        &players,
        &with(11),
    )
    .expect_err("unit count 11");
    assert_eq!(
        error,
        MatchSetupError::Options("a unit count of 11, the rules allow 0 to 10".into())
    );
    assert_eq!(
        error.to_string(),
        "options: a unit count of 11, the rules allow 0 to 10"
    );
    assert_eq!(state_hash(app.world_mut()), before);
}

#[test]
fn the_starting_credits_are_checked_against_the_range_and_step_of_the_rules() {
    let data = data();
    let players = [human(P0, ALPHA, 0), human(P1, GAMMA, 1)];
    let with = |credits: i64| MatchOptions {
        starting_credits: Some(credits),
        ..MatchOptions::default()
    };
    // The limits of the fixture's lobby and a step between them.
    for amount in [1000, 1500, 5000] {
        let (app, _) = setup(&data, &players, &with(amount), 7).expect("credits in range");
        assert_eq!((credits(&app, P0), credits(&app, P1)), (amount, amount));
    }
    // Without the option: the skirmish file's amount.
    let (app, _) = setup(&data, &players, &MatchOptions::default(), 7).expect("default");
    assert_eq!(credits(&app, P0), 3000);
    // One step outside either limit, and an amount off the step.
    for amount in [500, 5500, 1250, 999, 5001, -1000] {
        let mut app = world(&data, 7);
        let before = state_hash(app.world_mut());
        let error = setup_match_configured(
            app.world_mut(),
            &data.skirmish,
            &data.ai,
            0,
            &players,
            &with(amount),
        )
        .expect_err("credits outside the range");
        assert_eq!(
            error,
            MatchSetupError::Options(format!(
                "{amount} starting credits, the rules allow 1000 to 5000 in steps of 500"
            ))
        );
        assert_eq!(state_hash(app.world_mut()), before);
    }
}

#[test]
fn random_countries_are_drawn_from_the_generator_of_the_simulation() {
    let data = data();
    // The faction index of a random player is not read: it may be anything.
    let players = [human(P0, 99, 0), human(P1, 99, 1)];
    let options = MatchOptions {
        random_countries: vec![P0, P1],
        ..MatchOptions::default()
    };
    let rules = data.definitions.rules().expect("rules");
    let drawn = |seed: u64| {
        let (mut app, _) = setup(&data, &players, &options, seed).expect("random countries");
        let countries: Vec<_> = [P0, P1]
            .into_iter()
            .map(|player| {
                let state = app
                    .world()
                    .resource::<Players>()
                    .get(player)
                    .expect("player");
                state.country().expect("a country")
            })
            .collect();
        // The base unit is that of the drawn country's side.
        for (player, country) in [P0, P1].into_iter().zip(&countries) {
            let side = rules.country(*country).expect("country").side;
            let base = match side {
                Side::Allied => "allied_mcv",
                Side::Soviet => "soviet_mcv",
            };
            assert_eq!(unit_counts(&mut app, player).get(base), Some(&1));
        }
        (countries, state_hash(app.world_mut()))
    };
    // The same seed draws the same countries and gives the same state; over a few seeds every
    // country of the file is drawn, and the two players are drawn independently.
    assert_eq!(drawn(11), drawn(11));
    let mut seen = std::collections::BTreeSet::new();
    let mut differing = false;
    for seed in 0..40 {
        let (countries, _) = drawn(seed);
        differing |= countries[0] != countries[1];
        seen.extend(countries);
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
    assert!(differing);
    // A player with a fixed country is not drawn: only the listed one varies.
    let fixed = MatchOptions {
        random_countries: vec![P1],
        ..MatchOptions::default()
    };
    for seed in 0..8 {
        let (app, _) = setup(&data, &[human(P0, BETA, 0), human(P1, 99, 1)], &fixed, seed)
            .expect("one random country");
        let state = app.world().resource::<Players>().get(P0).expect("P0");
        assert_eq!(state.country(), rules.country_id("beta"));
    }
}

#[test]
fn a_random_country_needs_a_player_of_the_match_and_a_country_to_draw() {
    let data = data();
    let players = [human(P0, ALPHA, 0), human(P1, GAMMA, 1)];
    let random = |listed: Vec<PlayerId>| MatchOptions {
        random_countries: listed,
        ..MatchOptions::default()
    };
    let error = setup(&data, &players, &random(vec![P2]), 7)
        .map(|_| ())
        .expect_err("no such player");
    assert!(
        matches!(&error, MatchSetupError::Options(text) if text.contains("player 2")),
        "{error}"
    );
    // A skirmish file without countries has nothing to draw from.
    let mut without = Data {
        definitions: data.definitions.clone(),
        skirmish: data.skirmish.clone(),
        ai: data.ai.clone(),
    };
    for faction in &mut without.skirmish.factions {
        faction.side = None;
        faction.country = None;
    }
    let mut app = world(&without, 7);
    let before = state_hash(app.world_mut());
    let error = setup_match_configured(
        app.world_mut(),
        &without.skirmish,
        &without.ai,
        0,
        &players,
        &random(vec![P0]),
    )
    .expect_err("no countries");
    assert!(matches!(error, MatchSetupError::Options(_)), "{error}");
    assert_eq!(state_hash(app.world_mut()), before);
    // An unknown faction of a player that is not random is still an error.
    let error = setup(
        &data,
        &[human(P0, 99, 0), human(P1, GAMMA, 1)],
        &random(vec![P1]),
        7,
    )
    .map(|_| ())
    .expect_err("unknown faction");
    assert_eq!(
        error,
        MatchSetupError::UnknownFaction {
            player: P0,
            faction: 99
        }
    );
}

#[test]
fn computer_players_are_allied_against_the_human_unless_the_match_is_free_for_all() {
    let data = data();
    // A third start position on the fixture map.
    let mut three = Data {
        definitions: data.definitions.clone(),
        skirmish: data.skirmish.clone(),
        ai: data.ai.clone(),
    };
    three.skirmish.maps[0].starts.push(CellPos::new(32, 5));
    let players = [
        human(P0, ALPHA, 0),
        computer(P1, GAMMA, 1, 0),
        computer(P2, BETA, 2, 0),
    ];
    let teams = |options: &MatchOptions| {
        let (app, _) = setup(&three, &players, options, 7).expect("three players");
        let players = app.world().resource::<Players>();
        (
            [P0, P1, P2].map(|player| players.get(player).expect("player").team()),
            players.hostile(P1, P2),
            players.hostile(P0, P1),
        )
    };
    // The default (fixed decision 3): one team of computer players, hostile to the human only.
    assert_eq!(
        teams(&MatchOptions::default()),
        ([0, AI_TEAM, AI_TEAM], false, true)
    );
    assert_eq!(
        teams(&MatchOptions {
            free_for_all: true,
            ..MatchOptions::default()
        }),
        ([0, 0, 0], true, true)
    );
}

#[test]
fn the_options_of_a_match_are_part_of_its_state() {
    let data = data();
    let hash = |options: &MatchOptions| {
        let mut app = duel(&data, options);
        state_hash(app.world_mut())
    };
    let default = hash(&MatchOptions::default());
    assert_eq!(default, hash(&MatchOptions::default()));
    for options in [
        MatchOptions {
            short_game: false,
            ..MatchOptions::default()
        },
        MatchOptions {
            superweapons: false,
            ..MatchOptions::default()
        },
        MatchOptions {
            mcv_repacks: false,
            ..MatchOptions::default()
        },
        MatchOptions {
            crates: false,
            ..MatchOptions::default()
        },
        MatchOptions {
            mcv_start: false,
            ..MatchOptions::default()
        },
        MatchOptions {
            unit_count: Some(3),
            ..MatchOptions::default()
        },
        MatchOptions {
            starting_credits: Some(2500),
            ..MatchOptions::default()
        },
    ] {
        assert_ne!(hash(&options), default, "{options:?}");
    }
    // Different countries of one side differ too (the country is part of a player's state).
    let other = setup(
        &data,
        &[human(P0, BETA, 0), human(P1, GAMMA, 1)],
        &MatchOptions::default(),
        7,
    )
    .map(|(mut app, _)| state_hash(app.world_mut()))
    .expect("beta");
    assert_ne!(other, default);
}

// --- Integration: the win conditions ----------------------------------------------------------------

/// A duel in which P0 has three more tanks standing beside P1's base unit (scenario setup), with
/// the lobby's unit count 2: P1 has its base unit and one heavy tank.
fn siege(data: &Data, short_game: bool) -> (App, Vec<SimId>) {
    let mut app = duel(
        data,
        &MatchOptions {
            short_game,
            unit_count: Some(2),
            ..MatchOptions::default()
        },
    );
    let tanks = [(30, 31), (30, 32), (30, 33)]
        .map(|(x, y)| spawn_unit(app.world_mut(), P0, "tank", CellPos::new(x, y)).expect("a tank"))
        .to_vec();
    tick(&mut app, Vec::new());
    (app, tanks)
}

/// Orders `attackers` of P0 to attack `target` and runs until it is destroyed.
fn destroy(app: &mut App, attackers: &[SimId], target: SimId) {
    let events = tick(
        app,
        vec![issued(
            P0,
            Command::Attack {
                units: attackers.to_vec(),
                target,
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    for _ in 0..2000 {
        let events = tick(app, Vec::new());
        if events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == target))
        {
            return;
        }
    }
    panic!("{target:?} was not destroyed");
}

#[test]
fn in_a_short_game_a_player_without_buildings_and_base_unit_is_defeated() {
    let data = data();
    let (mut app, tanks) = siege(&data, true);
    // With only its base unit (and a tank) the player lives.
    assert_eq!(app.world().resource::<MatchState>().defeated(P1), None);
    assert_eq!(
        unit_counts(&mut app, P1),
        counts(&[("soviet_mcv", 1), ("heavy", 1)])
    );
    let mcv = base_unit(&mut app, P1);
    destroy(&mut app, &tanks, mcv);
    // The base unit is gone: defeated in that tick, though the heavy tank still stands.
    let state = app.world().resource::<MatchState>().clone();
    assert!(state.defeated(P1).is_some());
    assert!(matches!(state.outcome(), Some(MatchOutcome::Won { winner, .. }) if winner == P0));
    assert_eq!(unit_counts(&mut app, P1), counts(&[("heavy", 1)]));
}

#[test]
fn in_a_long_game_a_player_is_defeated_only_without_buildings_and_units() {
    let data = data();
    let (mut app, tanks) = siege(&data, false);
    let mcv = base_unit(&mut app, P1);
    destroy(&mut app, &tanks, mcv);
    // No building and no base unit, but a unit: the match goes on.
    assert_eq!(app.world().resource::<MatchState>().defeated(P1), None);
    assert_eq!(app.world().resource::<MatchState>().outcome(), None);
    let heavy = units(&mut app, P1)[0].0;
    // P0's whole army hunts the last unit down.
    let army: Vec<SimId> = units(&mut app, P0)
        .into_iter()
        .filter(|(_, id, _)| id == "tank")
        .map(|(id, ..)| id)
        .collect();
    destroy(&mut app, &army, heavy);
    let state = app.world().resource::<MatchState>().clone();
    assert!(state.defeated(P1).is_some());
    assert!(matches!(state.outcome(), Some(MatchOutcome::Won { winner, .. }) if winner == P0));
}

#[test]
fn a_base_unit_carried_by_a_transport_still_counts_for_its_owner() {
    // P1 has nothing but its base unit, which boards a transport of its own (scenario setup:
    // the hovercraft beside it): as a passenger it is no entity of the map, and still its owner
    // is not defeated. When the transport is destroyed its passenger is lost with it.
    let data = data();
    let mut app = duel(
        &data,
        &MatchOptions {
            unit_count: Some(0),
            ..MatchOptions::default()
        },
    );
    let hovercraft =
        spawn_unit(app.world_mut(), P1, "hovercraft", CellPos::new(33, 32)).expect("a hovercraft");
    let tanks = [(34, 31), (34, 32), (34, 33)]
        .map(|(x, y)| spawn_unit(app.world_mut(), P0, "tank", CellPos::new(x, y)).expect("a tank"))
        .to_vec();
    tick(&mut app, Vec::new());
    let mcv = base_unit(&mut app, P1);
    let events = tick(
        &mut app,
        vec![issued(
            P1,
            Command::Interact {
                unit: mcv,
                target: hovercraft,
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    assert_eq!(unit_counts(&mut app, P1), counts(&[("hovercraft", 1)]));
    for _ in 0..5 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(app.world().resource::<MatchState>().defeated(P1), None);
    assert_eq!(app.world().resource::<MatchState>().outcome(), None);
    destroy(&mut app, &tanks, hovercraft);
    let state = app.world().resource::<MatchState>().clone();
    assert!(state.defeated(P1).is_some());
    assert!(matches!(state.outcome(), Some(MatchOutcome::Won { winner, .. }) if winner == P0));
}

#[test]
fn a_player_with_buildings_lives_without_any_unit_in_both_games() {
    let data = data();
    for short_game in [true, false] {
        let mut app = duel(
            &data,
            &MatchOptions {
                short_game,
                mcv_start: false,
                unit_count: Some(0),
                ..MatchOptions::default()
            },
        );
        assert!(units(&mut app, P0).is_empty() && units(&mut app, P1).is_empty());
        for _ in 0..5 {
            tick(&mut app, Vec::new());
        }
        assert_eq!(app.world().resource::<MatchState>().outcome(), None);
    }
}

// --- Undeploying --------------------------------------------------------------------------------------

/// A duel that starts with the yards, without starting units.
fn yards(data: &Data, mcv_repacks: bool) -> App {
    let mut app = duel(
        data,
        &MatchOptions {
            mcv_start: false,
            mcv_repacks,
            unit_count: Some(0),
            ..MatchOptions::default()
        },
    );
    tick(&mut app, Vec::new());
    app
}

#[test]
fn a_construction_yard_undeploys_into_its_base_unit_and_deploys_again() {
    let data = data();
    let mut app = yards(&data, true);
    let (yard, _, footprint) = buildings(&mut app, P0)[0].clone();
    // Half the yard's hit points (scenario setup), and a building in production.
    set_hit_points(&mut app, yard, 500);
    let power = ProductionItem::Building(
        data.definitions
            .building_kind("allied_power")
            .expect("power"),
    );
    assert_eq!(
        rejections(&tick(
            &mut app,
            vec![issued(P0, Command::QueueProduction { item: power })]
        )),
        NONE
    );
    for _ in 0..5 {
        tick(&mut app, Vec::new());
    }
    let charged = 3000 - credits(&app, P0);
    assert!(charged > 0);

    let events = tick(
        &mut app,
        vec![issued(P0, Command::Undeploy { building: yard })],
    );
    assert_eq!(rejections(&events), NONE);
    let yard_kind = data.definitions.building_kind("allied_yard").expect("yard");
    assert!(events.contains(&SimEvent::BuildingUndeployed {
        player: P0,
        building: yard,
        kind: yard_kind,
        unit: yard,
        into: unit_kind(&data, "allied_mcv"),
    }));
    // The yard is the side's base unit now, with the same id, in the cell of the footprint's
    // origin, with the same share of hit points (500 of 1000: 300 of 600); its cells are free.
    assert!(buildings(&mut app, P0).is_empty());
    assert_eq!(
        units(&mut app, P0),
        [(yard, "allied_mcv".to_string(), footprint.origin)]
    );
    assert_eq!(hit_points(&mut app, yard), 300);
    {
        let grid = app.world().resource::<Grid>();
        assert!(footprint.cells().all(|cell| grid.building(cell).is_none()));
        assert_eq!(grid.occupant(footprint.origin), Some(yard));
    }
    // Without a yard the building in production waits (no charge), and the player lives on its
    // base unit.
    let waiting = credits(&app, P0);
    for _ in 0..30 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(credits(&app, P0), waiting);
    assert_eq!(app.world().resource::<MatchState>().defeated(P0), None);
    // The unit moves like any other, and deploys again where it stands.
    let target = CellPos::new(footprint.origin.x + 4, footprint.origin.y);
    tick(
        &mut app,
        vec![issued(
            P0,
            Command::Move {
                units: vec![yard],
                target,
            },
        )],
    );
    for _ in 0..200 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(units(&mut app, P0)[0].2, target);
    let events = tick(&mut app, vec![issued(P0, Command::Deploy { unit: yard })]);
    assert_eq!(rejections(&events), NONE);
    let redeployed = buildings(&mut app, P0);
    assert_eq!(
        (
            redeployed[0].0,
            redeployed[0].1.as_str(),
            redeployed[0].2.origin
        ),
        (yard, "allied_yard", target)
    );
    assert_eq!(hit_points(&mut app, yard), 500);
    // Production goes on with the yard.
    for _ in 0..30 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(credits(&app, P0), 3000 - 300);
}

#[test]
fn undeploy_is_rejected_without_changing_state() {
    let data = data();
    let yard_of = |app: &mut App, player: PlayerId| buildings(app, player)[0].0;
    // With "MCV repacks" off.
    let mut off = yards(&data, false);
    let yard = yard_of(&mut off, P0);
    let events = tick(
        &mut off,
        vec![issued(P0, Command::Undeploy { building: yard })],
    );
    assert_eq!(rejections(&events), [RejectReason::DisabledByOptions]);
    let mut quiet = yards(&data, false);
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(off.world_mut()), state_hash(quiet.world_mut()));

    // With the option on: another player's yard (unseen: reported as unknown, D30), a building
    // that does not exist, a unit, and a building that is no yard.
    let make = || {
        let mut app = yards(&data, true);
        let power = spawn_building(app.world_mut(), P0, "allied_power", CellPos::new(10, 5))
            .expect("a power plant");
        let tank = spawn_unit(app.world_mut(), P0, "tank", CellPos::new(5, 10)).expect("a tank");
        // An enemy building in P0's sight.
        let seen = spawn_building(app.world_mut(), P1, "soviet_power", CellPos::new(10, 9))
            .expect("an enemy plant");
        tick(&mut app, Vec::new());
        (app, power, tank, seen)
    };
    let (mut app, power, tank, seen) = make();
    let enemy_yard = yard_of(&mut app, P1);
    let events = tick(
        &mut app,
        [enemy_yard, seen, SimId(9999), tank, power]
            .map(|building| issued(P0, Command::Undeploy { building }))
            .to_vec(),
    );
    assert_eq!(
        rejections(&events),
        [
            RejectReason::UnknownBuilding(enemy_yard),
            RejectReason::NotOwned(seen),
            RejectReason::UnknownBuilding(SimId(9999)),
            RejectReason::UnknownBuilding(tank),
            RejectReason::AbilityUnavailable("This building does not undeploy"),
        ]
    );
    let (mut quiet, ..) = make();
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(quiet.world_mut()));
    assert_eq!(buildings(&mut app, P0).len(), 2);
}

#[test]
fn a_captured_yard_undeploys_into_the_base_unit_of_its_own_side() {
    // The yard's type decides, not its owner: a Soviet yard in Allied hands (scenario setup)
    // becomes a Soviet MCV, which deploys into a Soviet yard again.
    let data = data();
    let mut app = yards(&data, true);
    let captured = spawn_building(app.world_mut(), P0, "soviet_yard", CellPos::new(12, 12))
        .expect("a captured yard");
    tick(&mut app, Vec::new());
    let events = tick(
        &mut app,
        vec![issued(P0, Command::Undeploy { building: captured })],
    );
    assert_eq!(rejections(&events), NONE);
    assert_eq!(
        units(&mut app, P0),
        [(captured, "soviet_mcv".to_string(), CellPos::new(12, 12))]
    );
    let events = tick(
        &mut app,
        vec![issued(P0, Command::Deploy { unit: captured })],
    );
    assert_eq!(rejections(&events), NONE);
    let owned: Vec<String> = buildings(&mut app, P0)
        .into_iter()
        .map(|(_, id, _)| id)
        .collect();
    assert_eq!(owned, ["allied_yard", "soviet_yard"]);
}

// --- The computer player ---------------------------------------------------------------------------------

/// Runs a match of two computer players until it ends or `limit` ticks passed; returns the
/// outcome, the tick, the final hash, and every rejected command that what happened in the tick
/// between the decision and the command's validation does not explain (docs/DECISIONS.md D43,
/// as `m6_ai.rs` checks it): a unit or building the command names was destroyed then; for a
/// production or placement command, one of the player's units or buildings was; or a unit of
/// another player, unseen beside the footprint, stepped into a placement.
fn play(
    data: &Data,
    players: &[PlayerSetup],
    limit: u64,
) -> (Option<MatchOutcome>, u64, u64, Vec<SimEvent>) {
    let (mut app, _) =
        setup(data, players, &MatchOptions::default(), 7).unwrap_or_else(|e| panic!("{e}"));
    let mut unexplained = Vec::new();
    let mut lost_before: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
    let mut ticks = 0;
    while ticks < limit && app.world().resource::<MatchState>().outcome().is_none() {
        let events = tick(&mut app, Vec::new());
        let mut lost: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
        for event in &events {
            match event {
                SimEvent::UnitDestroyed { player, unit } => {
                    lost.entry(*player).or_default().push(*unit);
                }
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => lost.entry(*player).or_default().push(*building),
                _ => {}
            }
        }
        for event in events {
            let SimEvent::CommandRejected { player, reason, .. } = &event else {
                continue;
            };
            let losses = lost_before.get(player).map_or(&[][..], Vec::as_slice);
            let explained = match reason {
                RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                    losses.contains(id)
                }
                RejectReason::MissingPrerequisite { .. }
                | RejectReason::NoProducer { .. }
                | RejectReason::NoFactoryOfSide(_)
                | RejectReason::Placement(sim::PlacementError::NotVisible(_))
                | RejectReason::Placement(sim::PlacementError::TooFar { .. }) => !losses.is_empty(),
                RejectReason::Placement(sim::PlacementError::Occupied { unit, .. }) => {
                    let mut owners = app.world_mut().query::<(&SimId, &Owner)>();
                    owners
                        .iter(app.world())
                        .any(|(id, owner)| id == unit && owner.0 != *player)
                }
                _ => false,
            };
            if !explained {
                unexplained.push(event);
            }
        }
        lost_before = lost;
        ticks += 1;
    }
    let outcome = app.world().resource::<MatchState>().outcome();
    (outcome, ticks, state_hash(app.world_mut()), unexplained)
}

#[test]
fn a_computer_player_deploys_its_base_unit_at_once_and_builds_its_plan() {
    let data = data();
    let (mut app, _) = setup(
        &data,
        &[computer(P0, ALPHA, 0, 1), computer(P1, GAMMA, 1, 1)],
        &MatchOptions::default(),
        7,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    // The controllers follow the plan of their side.
    let ai = app.world().resource::<AiPlayers>().clone();
    for (player, side) in [(P0, Side::Allied), (P1, Side::Soviet)] {
        let (_, plan) = data.ai.plan(Some(side)).expect("a plan");
        let controller = ai.controller(player).expect("a controller");
        assert_eq!(controller.plan().build_order, plan.build_order);
        assert_eq!(controller.plan().army, plan.army);
        // The gathering point keeps clear of the yard the base unit deploys into.
        let start = controller.start();
        let yard = Footprint {
            origin: start,
            width: 3,
            height: 3,
        };
        assert!(!yard.contains(controller.gather()));
    }
    // Within its first turns (a think interval of 10 ticks) each has deployed where it started,
    // by a command of its own and without a rejection.
    let mut commands = Vec::new();
    for _ in 0..25 {
        commands.extend(
            app.world()
                .resource::<CommandQueue>()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        assert_eq!(rejections(&tick(&mut app, Vec::new())), NONE);
    }
    for (player, start, yard) in [(P0, START_0, "allied_yard"), (P1, START_1, "soviet_yard")] {
        let owned = buildings(&mut app, player);
        assert_eq!(owned.len(), 1, "{player:?}");
        assert_eq!((owned[0].1.as_str(), owned[0].2.origin), (yard, start));
        assert!(
            commands.iter().any(|issued| issued.player == player
                && matches!(issued.command, Command::Deploy { .. }))
        );
    }
    // Then it plays as before: within two minutes of game time it has placed every building of
    // its build order as often as the order lists it (some may have been destroyed since: the
    // other player attacks from tick 600).
    let mut placed: BTreeMap<(PlayerId, sim::BuildingKind), usize> = BTreeMap::new();
    for _ in 0..2400 {
        for event in tick(&mut app, Vec::new()) {
            match event {
                SimEvent::BuildingPlaced { player, kind, .. } => {
                    *placed.entry((player, kind)).or_insert(0) += 1;
                }
                SimEvent::CommandRejected { reason, .. } => {
                    // Only a loss in the tick before explains a rejection (`play`).
                    assert!(
                        matches!(
                            reason,
                            RejectReason::UnknownUnit(_) | RejectReason::UnknownBuilding(_)
                        ),
                        "{reason}"
                    );
                }
                _ => {}
            }
        }
    }
    for (player, side) in [(P0, Side::Allied), (P1, Side::Soviet)] {
        let (_, plan) = data.ai.plan(Some(side)).expect("a plan");
        for &kind in &plan.build_order {
            let listed = plan
                .build_order
                .iter()
                .filter(|&&other| other == kind)
                .count();
            assert!(
                placed.get(&(player, kind)).copied().unwrap_or(0) >= listed,
                "{player:?} placed {placed:?}"
            );
        }
    }
}

#[test]
fn an_armed_miner_of_a_computer_player_mines_and_is_not_drafted_into_its_army() {
    // Regression (RA1, found by `m13_grown_maps` with the shipped data): the Soviet miner has
    // a weapon, and the computer player took every armed unit for its army, called its miners
    // to the gathering point and sent them with the waves, so that a Soviet computer player
    // earned nothing. An armed harvester mines: it brings in ore, and never joins a wave.
    let data = data();
    let miner = unit_kind(&data, "miner_s");
    assert!(
        data.definitions
            .unit_combat(miner)
            .is_some_and(|terms| terms.weapon.is_some())
            && data.definitions.harvester(miner).is_some(),
        "the fixture's Soviet miner is an armed harvester"
    );
    let (mut app, _) = setup(
        &data,
        &[computer(P0, GAMMA, 0, 1), computer(P1, GAMMA, 1, 0)],
        &MatchOptions::default(),
        7,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut earned: BTreeMap<PlayerId, i64> = BTreeMap::new();
    for _ in 0..3000 {
        for event in tick(&mut app, Vec::new()) {
            if let SimEvent::OreDelivered {
                player, credits, ..
            } = event
            {
                *earned.entry(player).or_insert(0) += credits;
            }
        }
        let miners: Vec<SimId> = [P0, P1]
            .into_iter()
            .flat_map(|player| units(&mut app, player))
            .filter(|(_, id, _)| id == "miner_s")
            .map(|(id, ..)| id)
            .collect();
        let ai = app.world().resource::<AiPlayers>();
        for player in [P0, P1] {
            let wave = ai.controller(player).expect("a controller").wave();
            assert!(miners.iter().all(|id| !wave.contains(id)), "{player:?}");
        }
        if app.world().resource::<MatchState>().outcome().is_some() {
            break;
        }
    }
    for player in [P0, P1] {
        assert!(
            earned.get(&player).copied().unwrap_or(0) > 0,
            "{player:?} earned {earned:?}"
        );
    }
}

#[test]
fn computer_players_of_every_pairing_of_sides_finish_the_fixture_match() {
    // End to end on fixture data: every pairing of the two sides (with both Allied countries)
    // and of the two difficulties ends with a winner, without a rejected command that the
    // fighting does not explain, and the same way when it is played again.
    let data = data();
    for (first, second) in [(ALPHA, GAMMA), (GAMMA, BETA), (ALPHA, BETA), (GAMMA, GAMMA)] {
        for (easy, hard) in [(0, 1), (1, 0), (1, 1)] {
            let players = [computer(P0, first, 0, easy), computer(P1, second, 1, hard)];
            let name = format!("factions {first} and {second}, difficulties {easy} and {hard}");
            let (outcome, ticks, hash, rejected) = play(&data, &players, 72_000);
            assert!(
                matches!(outcome, Some(MatchOutcome::Won { .. })),
                "{name}: {outcome:?} after {ticks} ticks"
            );
            assert!(rejected.is_empty(), "{name}: {rejected:?}");
            assert_eq!(
                play(&data, &players, 72_000),
                (outcome, ticks, hash, rejected),
                "{name}"
            );
        }
    }
}
