//! Super Me opens every unit of both sides and all countries (docs/DECISIONS.md R213), with the
//! shipped data: a Super Me player with a complete base of its own side may queue every unit of
//! the arsenal, builds the other side's units at its own factories, and a player without Super
//! Me keeps the buildability rule of Red Alert 2 (side, country, stolen technology).
//!
//! Driven through `CommandQueue` and `SimSchedule` on the shipped data (`assets/data`).

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, CellPos, Command, CommandQueue, Controller, Definitions, IssuedCommand, MatchOptions,
    Owner, PlayerId, PlayerSetup, Players, ProductionItem, RejectReason, SimEvent, SimPlugin,
    SimSchedule, check_buildable, load_ai, load_map, load_ruleset, load_skirmish, map_asset_path,
    setup_match_configured, spawn_building,
};

const HUMAN: PlayerId = PlayerId(0);
const AI: PlayerId = PlayerId(1);

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// A match on the demo map: the human player of `country`, with or without Super Me, against a
/// computer player; each starts with its Construction Yard.
fn game(country: &str, super_me: bool) -> App {
    let data = assets().join("data");
    let definitions = load_ruleset(&data).unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(data.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(data.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let map = skirmish
        .maps
        .iter()
        .position(|map| map.id == "demo")
        .expect("demo map");
    let grid = load_map(assets().join(map_asset_path("demo")))
        .unwrap_or_else(|e| panic!("{e}"))
        .grid;
    let faction = |id: &str| {
        skirmish
            .factions
            .iter()
            .position(|faction| faction.id == id)
            .unwrap_or_else(|| panic!("faction {id}"))
    };
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 3 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions);
    let players = [
        PlayerSetup {
            player: HUMAN,
            faction: faction(country),
            start: 0,
            controller: Controller::Human,
        },
        PlayerSetup {
            player: AI,
            faction: faction("russia"),
            start: 1,
            controller: Controller::Ai { difficulty: 0 },
        },
    ];
    let options = MatchOptions {
        super_players: if super_me { vec![HUMAN] } else { Vec::new() },
        mcv_start: false,
        ..MatchOptions::default()
    };
    setup_match_configured(app.world_mut(), &skirmish, &ai, map, &players, &options)
        .unwrap_or_else(|e| panic!("{e:?}"));
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

/// Spawns one building of each type of `ids` for the human player on the first free ground
/// around its start (the first water for a shipyard).
fn base(app: &mut App, ids: &[&str]) {
    let start = {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        query
            .iter(app.world())
            .find(|(owner, _)| owner.0 == HUMAN)
            .map(|(_, building)| building.footprint.origin)
            .expect("the human player's yard")
    };
    for id in ids {
        let spot = (1..40).find_map(|r: i32| {
            (-r..=r)
                .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
                .filter(|(dx, dy)| dx.abs().max(dy.abs()) == r)
                .find(|(dx, dy)| {
                    spawn_building(
                        app.world_mut(),
                        HUMAN,
                        id,
                        CellPos::new(start.x + dx, start.y + dy),
                    )
                    .is_ok()
                })
        });
        assert!(spot.is_some(), "no ground for {id}");
    }
}

const ALLIED_BASE: [&str; 11] = [
    "power_plant",
    "power_plant",
    "power_plant",
    "barracks",
    "war_factory",
    "refinery",
    "airforce_command",
    "tech_center",
    "service_depot",
    "naval_shipyard",
    "power_plant",
];
const SOVIET_BASE: [&str; 11] = [
    "tesla_reactor",
    "tesla_reactor",
    "tesla_reactor",
    "soviet_barracks",
    "soviet_war_factory",
    "soviet_refinery",
    "radar_tower",
    "soviet_battle_lab",
    "soviet_service_depot",
    "soviet_naval_shipyard",
    "tesla_reactor",
];

/// Every unit of the arsenal that can be queued at all (tech level 0 or more), by id.
fn queueable_units(definitions: &Definitions) -> Vec<(String, ProductionItem)> {
    definitions
        .units()
        .iter()
        .filter_map(|def| {
            let item = ProductionItem::Unit(definitions.unit_kind(&def.id)?);
            definitions
                .metadata(item)
                .is_some_and(|entry| entry.tech_level >= 0)
                .then(|| (def.id.clone(), item))
        })
        .collect()
}

/// The outcome of a `QueueProduction` of `item` for the human player as it stands.
fn buildable(app: &mut App, item: ProductionItem) -> Result<(), RejectReason> {
    let owned: Vec<_> = {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        query
            .iter(app.world())
            .filter(|(owner, _)| owner.0 == HUMAN)
            .map(|(_, building)| building.kind)
            .collect()
    };
    let world = app.world();
    let state = world.resource::<Players>().get(HUMAN).expect("human");
    check_buildable(world.resource::<Definitions>(), state, true, &owned, item)
}

/// The units the human player cannot queue, with the reason.
fn closed(app: &mut App) -> Vec<(String, RejectReason)> {
    let units = queueable_units(app.world().resource::<Definitions>());
    units
        .into_iter()
        .filter_map(|(id, item)| buildable(app, item).err().map(|reason| (id, reason)))
        .collect()
}

fn item(app: &App, id: &str) -> ProductionItem {
    let definitions = app.world().resource::<Definitions>();
    definitions
        .unit_kind(id)
        .map(ProductionItem::Unit)
        .or_else(|| definitions.building_kind(id).map(ProductionItem::Building))
        .unwrap_or_else(|| panic!("no item {id}"))
}

fn ids(closed: &[(String, RejectReason)]) -> Vec<&str> {
    closed.iter().map(|(id, _)| id.as_str()).collect()
}

/// Queues each unit of `wanted` for the human player and ticks until a unit of each type has
/// left a factory of the human player.
fn produce(app: &mut App, wanted: &[&str]) {
    let commands = wanted
        .iter()
        .map(|id| IssuedCommand {
            player: HUMAN,
            command: Command::QueueProduction {
                item: item(app, id),
            },
        })
        .collect();
    let mut events = tick(app, commands);
    let mut missing: Vec<&str> = wanted.to_vec();
    for _ in 0..600 {
        for event in &events {
            match event {
                SimEvent::CommandRejected { player, reason, .. } if *player == HUMAN => {
                    panic!("rejected: {reason:?}")
                }
                SimEvent::UnitSpawned { player, unit, .. } if *player == HUMAN => {
                    let kind = {
                        let mut query = app.world_mut().query::<(&sim::SimId, &sim::Unit)>();
                        query
                            .iter(app.world())
                            .find(|(id, _)| **id == *unit)
                            .map(|(_, found)| found.kind)
                    };
                    let definitions = app.world().resource::<Definitions>();
                    if let Some(def) = kind.and_then(|kind| definitions.unit(kind)) {
                        missing.retain(|id| *id != def.id);
                    }
                }
                _ => {}
            }
        }
        if missing.is_empty() {
            return;
        }
        events = tick(app, Vec::new());
    }
    panic!("never produced: {missing:?}");
}

#[test]
fn super_me_opens_every_unit_of_both_sides_and_all_countries() {
    // An Allied player with Super Me and a complete Allied base may queue every unit.
    let mut allied = game("america", true);
    base(&mut allied, &ALLIED_BASE);
    assert_eq!(ids(&closed(&mut allied)), Vec::<&str>::new());

    // A Soviet player: everything but the two Allied jets, which only an Airforce Command
    // builds and docks. Super Me opens that one Allied building (the Soviet side has no aircraft
    // factory), and no other: the Allied Barracks stays the Allied player's.
    let mut soviet = game("russia", true);
    base(&mut soviet, &SOVIET_BASE);
    assert_eq!(ids(&closed(&mut soviet)), ["harrier", "black_eagle"]);
    let airfield = item(&soviet, "airforce_command");
    assert_eq!(buildable(&mut soviet, airfield), Ok(()));
    let barracks = item(&soviet, "barracks");
    assert!(buildable(&mut soviet, barracks).is_err());
    base(&mut soviet, &["airforce_command"]);
    assert_eq!(ids(&closed(&mut soviet)), Vec::<&str>::new());
}

#[test]
fn without_super_me_the_side_country_and_stolen_technology_rules_hold() {
    let mut allied = game("america", false);
    base(&mut allied, &ALLIED_BASE);
    let closed = closed(&mut allied);
    for id in [
        "conscript",
        "rhino",
        "apocalypse",
        "kirov",
        "sniper",
        "tank_destroyer",
    ] {
        assert!(ids(&closed).contains(&id), "{id} is open without Super Me");
    }
    for id in ["chrono_commando", "psi_commando"] {
        assert!(ids(&closed).contains(&id), "{id}: stolen technology");
    }
    let mut soviet = game("russia", false);
    base(&mut soviet, &SOVIET_BASE);
    let airfield = item(&soviet, "airforce_command");
    assert!(buildable(&mut soviet, airfield).is_err());
}

#[test]
fn a_super_me_player_builds_the_other_sides_units_at_its_own_factories() {
    let mut allied = game("america", true);
    base(&mut allied, &ALLIED_BASE);
    // Soviet infantry and vehicles, an Iraqi, a Libyan and a stolen-technology unit.
    produce(
        &mut allied,
        &[
            "conscript",
            "apocalypse",
            "desolator",
            "demolition_truck",
            "chrono_ivan",
        ],
    );

    let mut soviet = game("russia", true);
    base(&mut soviet, &SOVIET_BASE);
    base(&mut soviet, &["airforce_command"]);
    // Allied infantry and vehicles, a British, a German and a Korean unit.
    produce(
        &mut soviet,
        &[
            "gi",
            "prism_tank",
            "sniper",
            "tank_destroyer",
            "black_eagle",
        ],
    );
}
