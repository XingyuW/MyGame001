//! M7 tests: faction rosters and the computer player's plan per faction (PLAN.md section 7, M7:
//! "the second faction, the full roster"; invariants I3, I5, I8, I10; docs/DECISIONS.md D48,
//! D49).
//!
//! Every scenario runs on fixture data (the fixture units, economy, combat, skirmish, and AI
//! files and the 40x40 skirmish fixture map), through `setup_match`, `SimPlugin`, and
//! `CommandQueue`. File variants are derived from the committed fixtures by exact text edits and
//! written to Cargo's temporary directory for integration tests, under names unique to each test;
//! every load is bounded by the loader's file size limit (`MAX_DATA_FILE_BYTES`), and every match by
//! its tick limit and the AI file's `max_army`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AiDefinitions, BuildingKind, Command, CommandQueue, Controller, DataError, DataErrorKind,
    Definitions, IssuedCommand, MAX_ROSTER_ENTRIES, MatchOutcome, MatchSetupError, MatchState,
    PlayerId, PlayerSetup, Players, ProductionItem, RejectReason, SimEvent, SimPlugin, SimSchedule,
    SkirmishDefinitions, check_ai_factions, load_ai, load_map, load_match_definitions,
    load_skirmish, setup_match, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn definitions() -> Definitions {
    load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let definitions = definitions();
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

fn building(data: &Data, id: &str) -> ProductionItem {
    ProductionItem::Building(data.definitions.building_kind(id).expect("building"))
}

fn unit(data: &Data, id: &str) -> ProductionItem {
    ProductionItem::Unit(data.definitions.unit_kind(id).expect("unit"))
}

fn player(id: PlayerId, faction: usize, start: usize, controller: Controller) -> PlayerSetup {
    PlayerSetup {
        player: id,
        faction,
        start,
        controller,
    }
}

/// A match on the fixture map with `skirmish` and `ai` for `players`.
fn setup(
    data: &Data,
    skirmish: &SkirmishDefinitions,
    ai: &AiDefinitions,
    players: &[PlayerSetup],
) -> Result<App, MatchSetupError> {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 7 });
    let grid = load_map(fixture("maps/skirmish_40x40.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match(app.world_mut(), skirmish, ai, 0, players)?;
    Ok(app)
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

fn temp_file(name: &str, text: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m7_factions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(format!("{name}.ron"));
    std::fs::write(&path, text).expect("variant");
    path
}

/// The skirmish fixture with each `(from, to)` edit applied once; `from` must occur exactly once.
fn skirmish_variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = std::fs::read_to_string(fixture("data/skirmish.ron")).expect("fixture");
    for (from, to) in edits {
        assert_eq!(text.matches(from).count(), 1, "{name}: {from:?}");
        text = text.replacen(from, to, 1);
    }
    temp_file(name, &text)
}

/// The start of the first (red) faction's roster in the skirmish fixture.
const RED_ROSTER: &str =
    "            label: \"Red\",\n            side: \"none\",\n            roster_buildings: [\n";

fn assert_rejected(
    result: Result<SkirmishDefinitions, DataError>,
    field: &str,
    kind: DataErrorKind,
) {
    let error = result.expect_err("the file must be rejected");
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, &kind),
        "{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(field) && message.contains(&kind.to_string()),
        "{message}"
    );
}

// --- The roster in the skirmish file ---------------------------------------------------------

#[test]
fn the_rosters_of_the_skirmish_fixture_load_as_listed() {
    let data = data();
    assert_eq!(data.skirmish.factions.len(), 2);
    let every: BTreeSet<ProductionItem> = [
        "yard", "power", "refinery", "barracks", "factory", "fence", "turret", "beacon", "flag",
    ]
    .into_iter()
    .map(|id| building(&data, id))
    .chain(
        ["rifleman", "tank", "harvester"]
            .into_iter()
            .map(|id| unit(&data, id)),
    )
    .collect();
    for faction in &data.skirmish.factions {
        assert_eq!(faction.roster, every, "{}", faction.id);
        assert!(faction.offers(unit(&data, "tank")));
    }
}

#[test]
fn roster_entries_are_checked_against_the_definitions() {
    let definitions = definitions();
    let load = |path: PathBuf| load_skirmish(path, &definitions);
    let unknown = |kind: &str, id: &str| DataErrorKind::UnknownReference {
        kind: kind.into(),
        id: id.into(),
    };
    // An unknown building, and a unit named as a building.
    let path = skirmish_variant(
        "roster_unknown_building",
        &[(
            RED_ROSTER,
            &format!("{RED_ROSTER}                \"castle\",\n"),
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_buildings[0]",
        unknown("building", "castle"),
    );
    let path = skirmish_variant(
        "roster_unit_as_building",
        &[(
            RED_ROSTER,
            &format!("{RED_ROSTER}                \"tank\",\n"),
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_buildings[0]",
        unknown("building", "tank"),
    );
    // An unknown unit, and a unit without production terms.
    let red_units = "            roster_units: [\"rifleman\", \"tank\", \"harvester\"],\n            buildings: [(building: \"yard\", x: 0, y: 0)],";
    let path = skirmish_variant(
        "roster_unknown_unit",
        &[(
            red_units,
            &red_units.replace("[\"rifleman\",", "[\"dragon\", \"rifleman\","),
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_units[0]",
        unknown("unit", "dragon"),
    );
    let path = skirmish_variant(
        "roster_unproducible_unit",
        &[(
            red_units,
            &red_units.replace("\"harvester\"]", "\"harvester\", \"crawler\"]"),
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_units[3]",
        DataErrorKind::InvalidValue {
            found: "\"crawler\"".into(),
            expected: "a unit type with production terms".into(),
        },
    );
    // An entry listed twice.
    let path = skirmish_variant(
        "roster_duplicate",
        &[(
            RED_ROSTER,
            &format!("{RED_ROSTER}                \"flag\",\n"),
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_buildings[9]",
        DataErrorKind::DuplicateId("flag".into()),
    );
    // More entries than the limit, before anything is resolved.
    let many = format!(
        "{RED_ROSTER}{}",
        "                \"flag\",\n".repeat(MAX_ROSTER_ENTRIES)
    );
    let path = skirmish_variant("roster_too_long", &[(RED_ROSTER, &many)]);
    assert_rejected(
        load(path),
        "factions[0].roster_buildings",
        DataErrorKind::TooManyEntries {
            max: MAX_ROSTER_ENTRIES,
        },
    );
    // No silent default: a faction without a roster field is rejected.
    let path = skirmish_variant(
        "roster_missing",
        &[(
            red_units,
            "            buildings: [(building: \"yard\", x: 0, y: 0)],",
        )],
    );
    assert_rejected(
        load(path),
        "factions[0].roster_units",
        DataErrorKind::MissingField,
    );
    // An empty roster is a faction that produces nothing; it loads.
    let path = skirmish_variant(
        "roster_empty",
        &[(
            red_units,
            &red_units.replace("[\"rifleman\", \"tank\", \"harvester\"]", "[]"),
        )],
    );
    let skirmish = load(path).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        skirmish.factions[0]
            .roster
            .iter()
            .all(|item| matches!(item, ProductionItem::Building(_)))
    );
}

// --- The roster in play ----------------------------------------------------------------------

#[test]
fn production_outside_the_roster_is_rejected_without_changing_state() {
    let data = data();
    let beacon = building(&data, "beacon");
    let flag = building(&data, "flag");
    let tank = unit(&data, "tank");
    let mut skirmish = data.skirmish.clone();
    // The blue faction (P1) offers neither the beacon nor the tank.
    skirmish.factions[1].roster.remove(&beacon);
    skirmish.factions[1].roster.remove(&tank);
    let mut app = setup(
        &data,
        &skirmish,
        &data.ai,
        &[
            player(P0, 0, 0, Controller::Human),
            player(P1, 1, 1, Controller::Human),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    tick(&mut app, Vec::new());
    let before = state_hash(app.world_mut());
    let events = tick(
        &mut app,
        vec![
            issued(P1, Command::QueueProduction { item: beacon }),
            // The tank needs a factory, which blue does not own: the roster is checked first, so
            // the rejection does not depend on what the player owns.
            issued(P1, Command::QueueProduction { item: tank }),
            issued(P1, Command::CancelProduction { item: beacon }),
        ],
    );
    let rejections: Vec<&RejectReason> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { player, reason, .. } if *player == P1 => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejections,
        [
            &RejectReason::NotInRoster(beacon),
            &RejectReason::NotInRoster(tank),
            &RejectReason::NotQueued(beacon),
        ]
    );
    assert_eq!(
        RejectReason::NotInRoster(beacon).to_string(),
        format!("{beacon:?} is not in the roster of the player's faction")
    );
    // Nothing changed but the tick counter and the event buffer: the same tick without the
    // commands gives the same hash.
    let mut quiet = setup(
        &data,
        &skirmish,
        &data.ai,
        &[
            player(P0, 0, 0, Controller::Human),
            player(P1, 1, 1, Controller::Human),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(quiet.world_mut()), before);
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(quiet.world_mut()));
    // An item the roster offers is queued (blue's flag; red's beacon, which blue may not build).
    let events = tick(
        &mut app,
        vec![
            issued(P1, Command::QueueProduction { item: flag }),
            issued(P0, Command::QueueProduction { item: beacon }),
        ],
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    let players = app.world().resource::<Players>();
    let yard = data.definitions.building_kind("yard").expect("yard");
    assert_eq!(players.get(P1).expect("blue").queue(yard)[0].item, flag);
    assert_eq!(players.get(P0).expect("red").queue(yard)[0].item, beacon);
    assert!(!players.get(P1).expect("blue").offers(beacon));
    assert_eq!(
        players.get(P1).expect("blue").roster(),
        Some(&skirmish.factions[1].roster)
    );
}

#[test]
fn a_player_added_without_a_faction_may_produce_every_item() {
    // The scenario setups of the M3 to M6 tests add players without a faction.
    let mut players = Players::default();
    players.add(P0, 0).expect("player");
    let state = players.get(P0).expect("player");
    assert_eq!(state.roster(), None);
    assert!(state.offers(ProductionItem::Building(BuildingKind(0))));
    assert!(state.offers(ProductionItem::Unit(sim::UnitKind(u16::MAX))));
    // With an empty roster, nothing.
    let mut players = Players::default();
    players
        .add_with_roster(P0, 0, BTreeSet::new())
        .expect("player");
    assert!(
        !players
            .get(P0)
            .expect("player")
            .offers(ProductionItem::Building(BuildingKind(0)))
    );
    // Negative credits are rejected as for `add`.
    assert!(
        Players::default()
            .add_with_roster(P0, -1, BTreeSet::new())
            .is_err()
    );
}

// --- The computer player's plan per faction ---------------------------------------------------

#[test]
fn the_ai_file_is_checked_against_the_roster_of_every_faction() {
    let data = data();
    let path = fixture("data/ai.ron");
    let check = |skirmish: &SkirmishDefinitions| {
        check_ai_factions(&path, &data.ai, skirmish, &data.definitions)
    };
    check(&data.skirmish).unwrap_or_else(|error| panic!("{error}"));
    let without = |faction: usize, items: &[ProductionItem]| {
        let mut skirmish = data.skirmish.clone();
        for item in items {
            skirmish.factions[faction].roster.remove(item);
        }
        skirmish
    };
    let invalid_found = |error: DataError| match error.kind {
        DataErrorKind::InvalidValue { found, .. } => (error.field, found),
        other => panic!("{other:?}"),
    };
    // An entry no faction offers is dead data.
    let turret = building(&data, "turret");
    let error = check(&without(0, &[turret]));
    assert!(error.is_ok(), "blue still offers the turret: {error:?}");
    let mut neither = without(0, &[turret]);
    neither.factions[1].roster.remove(&turret);
    let (field, found) = invalid_found(check(&neither).expect_err("dead entry"));
    assert_eq!(field, "plans[0].build_order[5]");
    assert_eq!(
        found,
        "\"turret\", which no faction of the skirmish file offers"
    );
    let rifleman = unit(&data, "rifleman");
    let mut neither = without(0, &[rifleman]);
    neither.factions[1].roster.remove(&rifleman);
    let (field, found) = invalid_found(check(&neither).expect_err("dead army entry"));
    assert_eq!(
        (field.as_str(), found.as_str()),
        (
            "plans[0].army[2]",
            "\"rifleman\", which no faction of the skirmish file offers"
        )
    );
    // A faction that does not offer the power building or the harvester.
    let (field, found) =
        invalid_found(check(&without(1, &[building(&data, "power")])).expect_err("power"));
    assert_eq!(
        (field.as_str(), found.as_str()),
        (
            "plans[0].power",
            "\"power\", which faction \"blue\" does not offer"
        )
    );
    let (field, found) =
        invalid_found(check(&without(1, &[unit(&data, "harvester")])).expect_err("harvester"));
    assert_eq!(
        (field.as_str(), found.as_str()),
        (
            "plans[0].harvester",
            "\"harvester\", which faction \"blue\" does not offer"
        )
    );
    // A faction whose plan lacks a prerequisite: the factory needs the refinery.
    let (field, found) =
        invalid_found(check(&without(1, &[building(&data, "refinery")])).expect_err("refinery"));
    assert_eq!(field, "plans[0].build_order[3]");
    assert_eq!(
        found,
        "\"factory\", whose prerequisite \"refinery\" is not listed before it in the plan of \
         faction \"blue\""
    );
    // A faction whose plan has no army, or an army that cannot damage the concrete buildings
    // (bullet against concrete: 0 %).
    let tank = unit(&data, "tank");
    let (field, found) = invalid_found(check(&without(1, &[tank, rifleman])).expect_err("no army"));
    assert_eq!(
        (field.as_str(), found.as_str()),
        (
            "plans[0].army",
            "no unit type in the plan of faction \"blue\""
        )
    );
    let error = check(&without(1, &[tank])).expect_err("riflemen only");
    assert_eq!(error.field, "plans[0].army");
    assert_eq!(
        error.kind,
        DataErrorKind::MissingEntry {
            kind: "unit of faction \"blue\" able to damage building".into(),
            id: "yard".into(),
        }
    );
    // A faction that lacks the factory, which produces the harvester and the tank.
    let (field, found) =
        invalid_found(check(&without(1, &[building(&data, "factory")])).expect_err("factory"));
    assert_eq!(field, "plans[0].harvester");
    assert!(
        found.contains("whose producer \"factory\" is not listed in the build order")
            && found.ends_with("in the plan of faction \"blue\""),
        "{found}"
    );
}

#[test]
fn match_setup_rejects_a_computer_player_of_a_faction_without_a_complete_plan() {
    let data = data();
    let mut skirmish = data.skirmish.clone();
    skirmish.factions[1]
        .roster
        .remove(&building(&data, "power"));
    let setups = [
        player(P0, 0, 0, Controller::Human),
        player(P1, 1, 1, Controller::Ai { difficulty: 0 }),
    ];
    match setup(&data, &skirmish, &data.ai, &setups) {
        Err(MatchSetupError::AiPlan {
            player,
            faction,
            field,
            problem,
        }) => {
            assert_eq!(
                (player, faction.as_str(), field.as_str()),
                (P1, "blue", "plans[0].power")
            );
            assert!(problem.contains("does not offer"), "{problem}");
        }
        other => panic!("{:?}", other.map(|_| ())),
    }
    // The same faction may be played by a person, against a computer player of the other.
    let setups = [
        player(P0, 1, 0, Controller::Human),
        player(P1, 0, 1, Controller::Ai { difficulty: 0 }),
    ];
    let app = setup(&data, &skirmish, &data.ai, &setups).unwrap_or_else(|error| panic!("{error}"));
    let message = MatchSetupError::AiPlan {
        player: P1,
        faction: "blue".into(),
        field: "plans[0].power".into(),
        problem: "x".into(),
    }
    .to_string();
    assert_eq!(
        message,
        "computer player 1 of faction \"blue\": the AI file's plans[0].power: x"
    );
    drop(app);
}

/// Runs a match of two computer players to its end (at most `limit` ticks); returns the outcome,
/// every production item each player queued, and the state hash of every tick.
fn play(
    data: &Data,
    skirmish: &SkirmishDefinitions,
    limit: u64,
) -> (
    Option<MatchOutcome>,
    [BTreeSet<ProductionItem>; 2],
    Vec<u64>,
) {
    let mut app = setup(
        data,
        skirmish,
        &data.ai,
        &[
            player(P0, 0, 0, Controller::Ai { difficulty: 1 }),
            player(P1, 1, 1, Controller::Ai { difficulty: 1 }),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut queued = [BTreeSet::new(), BTreeSet::new()];
    let mut hashes = Vec::new();
    for at in 0..limit {
        let events = tick(&mut app, Vec::new());
        for event in &events {
            if let SimEvent::CommandRejected {
                command, reason, ..
            } = event
            {
                assert!(
                    !matches!(reason, RejectReason::NotInRoster(_)),
                    "tick {at}: {command:?} rejected: {reason}"
                );
            }
        }
        for command in app.world().resource::<CommandQueue>().iter() {
            if let Command::QueueProduction { item } = command.command {
                queued[usize::from(command.player.0)].insert(item);
            }
        }
        hashes.push(state_hash(app.world_mut()));
        if let Some(outcome) = app.world().resource::<MatchState>().outcome() {
            return (Some(outcome), queued, hashes);
        }
    }
    (None, queued, hashes)
}

#[test]
fn computer_players_build_only_what_their_faction_offers_and_finish_the_match() {
    let data = data();
    let rifleman = unit(&data, "rifleman");
    let turret = building(&data, "turret");
    let mut skirmish = data.skirmish.clone();
    // Red fields no riflemen, blue builds no turrets (blue starts with one; starting buildings
    // need not be in the roster).
    skirmish.factions[0].roster.remove(&rifleman);
    skirmish.factions[1].roster.remove(&turret);
    let red_plan = data.ai.plan_for(&skirmish.factions[0]).expect("a plan");
    let blue_plan = data.ai.plan_for(&skirmish.factions[1]).expect("a plan");
    let rifleman_kind = data.definitions.unit_kind("rifleman").expect("rifleman");
    let turret_kind = data.definitions.building_kind("turret").expect("turret");
    assert!(!red_plan.army.contains(&rifleman_kind));
    assert!(blue_plan.army.contains(&rifleman_kind));
    assert!(red_plan.build_order.contains(&turret_kind));
    assert!(!blue_plan.build_order.contains(&turret_kind));
    assert_eq!(
        red_plan.build_order.len(),
        data.ai.plans[0].build_order.len()
    );
    let (outcome, queued, hashes) = play(&data, &skirmish, 36_000);
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{outcome:?}"
    );
    assert!(!queued[0].contains(&rifleman), "red queued a rifleman");
    assert!(!queued[1].contains(&turret), "blue queued a turret");
    assert!(queued[0].contains(&turret) && queued[1].contains(&rifleman));
    // The controllers follow their faction's plan, which is part of the state hash: the same
    // match gives the same hash on every tick.
    let (again, _, repeat) = play(&data, &skirmish, 36_000);
    assert_eq!(again, outcome);
    assert_eq!(repeat, hashes);
    let app = setup(
        &data,
        &skirmish,
        &data.ai,
        &[
            player(P0, 0, 0, Controller::Ai { difficulty: 1 }),
            player(P1, 1, 1, Controller::Ai { difficulty: 1 }),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let ai = app.world().resource::<sim::AiPlayers>();
    assert_eq!(ai.controller(P0).expect("red").plan(), &red_plan);
    assert_eq!(ai.controller(P1).expect("blue").plan(), &blue_plan);
}
