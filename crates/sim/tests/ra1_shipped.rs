//! RA1 with the shipped data: the nine countries, their trees and specials, and the default
//! match (docs/RA2_PARITY_PLAN.md milestone RA1, "Tests" and "Acceptance"; docs/DECISIONS.md R10
//! to R14).
//!
//! These tests assert the shipped behaviour, so they load `assets/data` as the game does
//! (`sim::load_ruleset`) and the shipped maps, and drive `setup_match_configured`,
//! `CommandQueue` and `SimSchedule`. The rule itself is tested on fixture data in
//! `ra1_buildability.rs` and `ra1_match.rs`; the numbers are audited in `ra9_data_parity.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AiDefinitions, Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Controller,
    Definitions, Grid, IssuedCommand, MatchOptions, MatchRules, MatchState, Mobile, Owner,
    PlayerId, PlayerSetup, Players, ProductionItem, RejectReason, STARTING_UNIT_DISTANCE, Side,
    SimEvent, SimId, SimPlugin, SimSchedule, SkirmishDefinitions, Unit, check_buildable,
    check_placement, country_tree, load_ai, load_map, load_ruleset, load_skirmish, map_asset_path,
    setup_match_configured, spawn_building,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

/// No rejection.
const NONE: [RejectReason; 0] = [];

/// The ids of the legacy armies, which left the shipped data in RA1 (fixed decision 2).
const LEGACY_UNITS: [&str; 10] = [
    "rifleman",
    "rocket_soldier",
    "grenadier",
    "scout",
    "raider",
    "light_tank",
    "heavy_tank",
    "artillery",
    "rocket_launcher",
    "harvester",
];
const LEGACY_BUILDINGS: [&str; 2] = ["gun_turret", "rocket_tower"];

/// The special of every country: its id, and whether it is a building.
const SPECIALS: [(&str, &str, bool); 9] = [
    ("america", "american_airforce_command", true),
    ("korea", "black_eagle", false),
    ("france", "grand_cannon", true),
    ("germany", "tank_destroyer", false),
    ("great_britain", "sniper", false),
    ("libya", "demolition_truck", false),
    ("iraq", "desolator", false),
    ("cuba", "terrorist", false),
    ("russia", "tesla_tank", false),
];

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let dir = assets().join("data");
    let definitions = load_ruleset(&dir).unwrap_or_else(|error| panic!("{error}"));
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

impl Data {
    fn faction(&self, id: &str) -> usize {
        self.skirmish
            .factions
            .iter()
            .position(|faction| faction.id == id)
            .unwrap_or_else(|| panic!("country {id}"))
    }

    fn building(&self, id: &str) -> BuildingKind {
        self.definitions
            .building_kind(id)
            .unwrap_or_else(|| panic!("building {id}"))
    }

    fn item(&self, id: &str) -> ProductionItem {
        self.definitions
            .building_kind(id)
            .map(ProductionItem::Building)
            .or_else(|| self.definitions.unit_kind(id).map(ProductionItem::Unit))
            .unwrap_or_else(|| panic!("item {id}"))
    }

    fn name(&self, item: ProductionItem) -> &str {
        match item {
            ProductionItem::Building(kind) => &self.definitions.building(kind).expect("kind").id,
            ProductionItem::Unit(kind) => &self.definitions.unit(kind).expect("kind").id,
        }
    }

    /// The own tech tree of the country `id`, as item names.
    fn tree(&self, id: &str) -> BTreeSet<&str> {
        let rules = self.definitions.rules().expect("rules");
        country_tree(&self.definitions, rules.country_id(id).expect("country"))
            .into_iter()
            .map(|item| self.name(item))
            .collect()
    }
}

fn human(player: PlayerId, faction: usize, start: usize) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller: Controller::Human,
    }
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

/// The buildings of `player`: type name and footprint origin, in id order.
fn buildings(app: &mut App, player: PlayerId) -> Vec<(String, CellPos)> {
    let definitions = app.world().resource::<Definitions>().clone();
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
    let mut found: Vec<(SimId, String, CellPos)> = query
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
                building.footprint.origin,
            )
        })
        .collect();
    found.sort_unstable();
    found
        .into_iter()
        .map(|(_, id, origin)| (id, origin))
        .collect()
}

fn counts(app: &mut App, player: PlayerId) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (_, id, _) in units(app, player) {
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
}

fn expected(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|(id, count)| (id.to_string(), *count))
        .collect()
}

// --- The shipped data ---------------------------------------------------------------------------

#[test]
fn the_shipped_data_holds_the_nine_countries_and_no_legacy_item() {
    let data = data();
    let rules = data.definitions.rules().expect("rules");
    let ids: Vec<&str> = data
        .skirmish
        .factions
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "america",
            "korea",
            "france",
            "germany",
            "great_britain",
            "libya",
            "iraq",
            "cuba",
            "russia"
        ]
    );
    for (index, faction) in data.skirmish.factions.iter().enumerate() {
        let side = if index < 5 {
            Side::Allied
        } else {
            Side::Soviet
        };
        assert_eq!(faction.side, Some(side), "{}", faction.id);
        assert_eq!(faction.country, rules.country_id(&faction.id));
        assert!(!faction.label.is_empty());
        assert!(faction.buildings.is_empty() && faction.units.is_empty());
        assert_eq!(
            faction.roster,
            country_tree(&data.definitions, faction.country.expect("country"))
        );
    }
    assert_eq!(
        data.skirmish.pairing_groups(),
        [vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8]]
    );
    // No legacy item can be built in a default match: none is defined any more.
    for id in LEGACY_UNITS {
        assert_eq!(data.definitions.unit_kind(id), None, "{id}");
    }
    for id in LEGACY_BUILDINGS {
        assert_eq!(data.definitions.building_kind(id), None, "{id}");
    }
    // The computer player has a plan for each side and the three difficulties of the lobby.
    assert!(
        data.ai.plan(Some(Side::Allied)).is_some() && data.ai.plan(Some(Side::Soviet)).is_some()
    );
    assert_eq!(data.ai.plan(None), None);
    let difficulties: Vec<&str> = data.ai.difficulties.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(difficulties, ["easy", "medium", "brutal"]);
    // The sides: base units and the yards they deploy into.
    let side = |side: Side| {
        let rules = rules.side(side).expect("side");
        (
            data.definitions
                .unit(rules.base_unit)
                .expect("unit")
                .id
                .as_str(),
            data.definitions
                .building(rules.construction_yard)
                .expect("yard")
                .id
                .as_str(),
        )
    };
    assert_eq!(side(Side::Allied), ("mcv", "construction_yard"));
    assert_eq!(
        side(Side::Soviet),
        ("soviet_mcv", "soviet_construction_yard")
    );
}

#[test]
fn every_country_has_the_tree_of_its_side_and_its_own_special() {
    let data = data();
    let allied = [
        "power_plant",
        "refinery",
        "barracks",
        "war_factory",
        "tech_center",
        "gi",
        "grizzly",
        "prism_tank",
        "chrono_miner",
        "mcv",
        "engineer",
        "attack_dog",
    ];
    let soviet = [
        "tesla_reactor",
        "soviet_refinery",
        "soviet_barracks",
        "soviet_war_factory",
        "radar_tower",
        "soviet_battle_lab",
        "conscript",
        "rhino",
        "apocalypse",
        "kirov",
        "war_miner",
        "soviet_mcv",
        "engineer",
        "attack_dog",
    ];
    for (index, faction) in data.skirmish.factions.iter().enumerate() {
        let tree = data.tree(&faction.id);
        let (own, other) = if index < 5 {
            (&allied[..], &soviet[..])
        } else {
            (&soviet[..], &allied[..])
        };
        for id in own {
            assert!(tree.contains(id), "{}: {id}", faction.id);
        }
        for id in other.iter().filter(|id| !own.contains(id)) {
            assert!(!tree.contains(id), "{}: {id}", faction.id);
        }
        // The yards and the four units a spy unlocks cannot be queued by anyone.
        for id in [
            "construction_yard",
            "soviet_construction_yard",
            "chrono_commando",
            "psi_commando",
            "chrono_ivan",
            "yuri_prime",
        ] {
            assert!(!tree.contains(id), "{}: {id}", faction.id);
        }
        // Its own special and nobody else's.
        for (country, special, _) in SPECIALS {
            assert_eq!(
                tree.contains(special),
                country == faction.id,
                "{}: {special}",
                faction.id
            );
        }
    }
    // Korea flies the Black Eagle instead of the Harrier; America has its own Airforce Command
    // instead of the common one; every other Allied country the common ones.
    for country in ["america", "korea", "france", "germany", "great_britain"] {
        let tree = data.tree(country);
        assert_eq!(tree.contains("harrier"), country != "korea", "{country}");
        assert_eq!(
            tree.contains("airforce_command"),
            country != "america",
            "{country}"
        );
    }
    // The stolen tech units are in the data with their costs, for the milestones that unlock
    // them.
    for (id, cost) in [
        ("chrono_commando", 2000),
        ("psi_commando", 1000),
        ("chrono_ivan", 1000),
        ("yuri_prime", 2000),
    ] {
        assert_eq!(
            data.definitions
                .production(data.item(id))
                .expect("terms")
                .cost,
            cost,
            "{id}"
        );
    }
    // The Kirov and the NightHawk are built at the War Factory, as in Red Alert 2.
    for (id, factory) in [
        ("kirov", "soviet_war_factory"),
        ("nighthawk", "war_factory"),
    ] {
        assert!(
            sim::produces(&data.definitions, data.building(factory), data.item(id)),
            "{id}"
        );
    }
    // The factories of both sides are one group each and share their queue.
    for (allied, soviet) in [
        ("construction_yard", "soviet_construction_yard"),
        ("barracks", "soviet_barracks"),
        ("war_factory", "soviet_war_factory"),
        ("naval_shipyard", "soviet_naval_shipyard"),
        ("airforce_command", "american_airforce_command"),
    ] {
        let (a, b) = (data.building(allied), data.building(soviet));
        assert!(data.definitions.same_factory(a, b), "{allied}");
        assert_eq!(
            data.definitions.queue_key(a),
            data.definitions.queue_key(b),
            "{allied}"
        );
    }
}

// --- The default match ------------------------------------------------------------------------------

/// The shipped map `id` with a match of `players` and `options` set up on it.
fn shipped_match(data: &Data, id: &str, players: &[PlayerSetup], options: &MatchOptions) -> App {
    let map = data
        .skirmish
        .maps
        .iter()
        .position(|map| map.id == id)
        .unwrap_or_else(|| panic!("map {id}"));
    let grid = load_map(assets().join(map_asset_path(id)))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 1 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        map,
        players,
        options,
    )
    .unwrap_or_else(|error| panic!("{id}: {error}"));
    app
}

#[test]
fn the_default_match_is_that_of_red_alert_2s_lobby() {
    // America against Russia on the demo map with the default options: 10000 credits, the MCV,
    // the starting units of unit count 10, a short game, superweapons, crates and MCV repacks.
    let data = data();
    let players = [
        human(P0, data.faction("america"), 0),
        human(P1, data.faction("russia"), 1),
    ];
    let mut app = shipped_match(&data, "demo", &players, &MatchOptions::default());
    assert_eq!(
        counts(&mut app, P0),
        expected(&[("mcv", 1), ("grizzly", 5), ("gi", 4)])
    );
    assert_eq!(
        counts(&mut app, P1),
        expected(&[("soviet_mcv", 1), ("rhino", 4), ("conscript", 7)])
    );
    assert!(buildings(&mut app, P0).is_empty() && buildings(&mut app, P1).is_empty());
    let rules = data.definitions.rules().expect("rules");
    for (player, country) in [(P0, "america"), (P1, "russia")] {
        let state = app
            .world()
            .resource::<Players>()
            .get(player)
            .expect("player")
            .clone();
        assert_eq!(state.credits(), 10_000);
        assert_eq!(state.country(), rules.country_id(country));
        assert_eq!(state.roster(), None);
    }
    assert_eq!(
        app.world().resource::<MatchState>().rules(),
        MatchRules::default()
    );
    assert_eq!(
        MatchRules::default(),
        MatchRules {
            short_game: true,
            superweapons: true,
            mcv_repacks: true,
            crates: true,
        }
    );
    // Deploying where it starts gives each player the yard of its side at its start position,
    // and with it the first buildings of its tree and nothing of the other side.
    tick(&mut app, Vec::new());
    let deploy: Vec<IssuedCommand> = [P0, P1]
        .into_iter()
        .map(|player| {
            let mcv = units(&mut app, player)
                .into_iter()
                .find(|(_, id, _)| id.ends_with("mcv"))
                .expect("a base unit")
                .0;
            issued(player, Command::Deploy { unit: mcv })
        })
        .collect();
    assert_eq!(rejections(&tick(&mut app, deploy)), NONE);
    let starts = &data.skirmish.maps[0].starts;
    assert_eq!(
        buildings(&mut app, P0),
        [("construction_yard".to_string(), starts[0])]
    );
    assert_eq!(
        buildings(&mut app, P1),
        [("soviet_construction_yard".to_string(), starts[1])]
    );
    let queue = |player, id: &str| {
        issued(
            player,
            Command::QueueProduction {
                item: data.item(id),
            },
        )
    };
    let events = tick(
        &mut app,
        vec![
            queue(P0, "power_plant"),
            queue(P1, "tesla_reactor"),
            queue(P0, "tesla_reactor"),
            queue(P1, "power_plant"),
            queue(P0, "construction_yard"),
            queue(P0, "american_airforce_command"),
        ],
    );
    assert_eq!(
        rejections(&events),
        [
            RejectReason::MissingPrerequisite {
                item: data.item("tesla_reactor"),
                prerequisite: data.building("soviet_construction_yard"),
            },
            RejectReason::MissingPrerequisite {
                item: data.item("power_plant"),
                prerequisite: data.building("construction_yard"),
            },
            RejectReason::UnknownItem(data.item("construction_yard")),
            // America's own Airforce Command needs a refinery first.
            RejectReason::MissingPrerequisite {
                item: data.item("american_airforce_command"),
                prerequisite: data.building("refinery"),
            },
        ]
    );
}

#[test]
fn every_country_sets_up_on_every_map_with_an_mcv_and_0_1_and_10_starting_units() {
    // On every shipped map, every country at the first start position against a country of the
    // other side at the second, with unit counts 0, 1 and 10: the base units stand on the start
    // positions, the starting units 3 or more cells from them, and both base units deploy
    // where they stand, which puts the yard's footprint origin on the start position (what the
    // start positions of the skirmish file and `map_scale` are made for).
    let data = data();
    let groups = data.skirmish.pairing_groups();
    let yard = |faction: usize| {
        let side = data.skirmish.factions[faction].side.expect("side");
        let kind = data
            .definitions
            .rules()
            .expect("rules")
            .side(side)
            .expect("side")
            .construction_yard;
        data.definitions.building(kind).expect("yard").id.clone()
    };
    for (map, def) in data.skirmish.maps.iter().enumerate() {
        let grid = load_map(assets().join(map_asset_path(&def.id)))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        for faction in 0..data.skirmish.factions.len() {
            let other_side = usize::from(faction < groups[0].len());
            let opponent = SkirmishDefinitions::rotating(&groups[other_side], map + faction)
                .expect("opponent");
            for (unit_count, own_units, other_units) in [(0, 0, 0), (1, 0, 0), (10, 9, 11)] {
                let name = format!(
                    "{}: {} with unit count {unit_count}",
                    def.id, data.skirmish.factions[faction].id
                );
                let mut app = App::new();
                app.add_plugins(SimPlugin { seed: 1 });
                app.world_mut().insert_resource(grid.clone());
                app.world_mut().insert_resource(data.definitions.clone());
                let start = setup_match_configured(
                    app.world_mut(),
                    &data.skirmish,
                    &data.ai,
                    map,
                    &[human(P0, faction, 0), human(P1, opponent, 1)],
                    &MatchOptions {
                        unit_count: Some(unit_count),
                        ..MatchOptions::default()
                    },
                )
                .unwrap_or_else(|error| panic!("{name}: {error}"));
                // Allied: 5 + 4 units, Soviet: 4 + 7 (unit count 10).
                let (first, second) = if faction < groups[0].len() {
                    (own_units, other_units)
                } else {
                    (other_units, own_units)
                };
                assert_eq!(
                    (start.buildings, start.units),
                    (0, 2 + first + second),
                    "{name}"
                );
                let mut bases = Vec::new();
                for (player, cell) in [(P0, def.starts[0]), (P1, def.starts[1])] {
                    for (id, kind, at) in units(&mut app, player) {
                        if kind.ends_with("mcv") {
                            assert_eq!(at, cell, "{name}");
                            bases.push(issued(player, Command::Deploy { unit: id }));
                        } else {
                            let distance = at.x.abs_diff(cell.x).max(at.y.abs_diff(cell.y));
                            assert!(
                                distance >= STARTING_UNIT_DISTANCE,
                                "{name}: {kind} at {at:?}"
                            );
                        }
                    }
                }
                assert_eq!(bases.len(), 2, "{name}");
                tick(&mut app, Vec::new());
                assert_eq!(rejections(&tick(&mut app, bases)), NONE, "{name}");
                assert_eq!(
                    buildings(&mut app, P0),
                    [(yard(faction), def.starts[0])],
                    "{name}"
                );
                assert_eq!(
                    buildings(&mut app, P1),
                    [(yard(opponent), def.starts[1])],
                    "{name}"
                );
            }
        }
    }
}

// --- Building the trees through commands ---------------------------------------------------------------

/// A world of 64 x 64 open cells in which P0 plays `country` and owns the buildings `owned`
/// (scenario setup), with plenty of credits.
fn base(data: &Data, country: &str, owned: &[&str]) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 2 });
    let cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(64, 64, vec![cell; 64 * 64]).expect("a grid"));
    app.world_mut().insert_resource(data.definitions.clone());
    let id = data
        .definitions
        .rules()
        .expect("rules")
        .country_id(country)
        .expect("country");
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(P0, 1_000_000).expect("a new player");
        assert!(players.set_country(P0, Some(id)));
    }
    for (index, building) in owned.iter().enumerate() {
        let origin = CellPos::new(4 + (index as i32 % 6) * 6, 4 + (index as i32 / 6) * 6);
        spawn_building(app.world_mut(), P0, building, origin)
            .unwrap_or_else(|e| panic!("{building}: {e}"));
    }
    tick(&mut app, Vec::new());
    app
}

/// Queues the building `id` for P0, waits for it, and places it on the first cell the command's
/// own validation accepts. Panics on a rejection or if it does not complete in two minutes.
fn build(app: &mut App, data: &Data, id: &str) {
    let item = data.item(id);
    let kind = data.building(id);
    let queue = issued(P0, Command::QueueProduction { item });
    assert_eq!(rejections(&tick(app, vec![queue])), NONE, "queue {id}");
    let mut ready = false;
    for _ in 0..2400 {
        let events = tick(app, Vec::new());
        assert_eq!(rejections(&events), NONE, "{id}");
        if events.contains(&SimEvent::ProductionComplete { player: P0, item }) {
            ready = true;
            break;
        }
    }
    assert!(ready, "{id} was not completed");
    let footprints: Vec<_> = {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        query
            .iter(app.world())
            .filter(|(owner, _)| owner.0 == P0)
            .map(|(_, b)| (b.kind, b.footprint))
            .collect()
    };
    let origin = {
        let world = app.world();
        let (grid, definitions) = (world.resource::<Grid>(), world.resource::<Definitions>());
        let state = world.resource::<Players>().get(P0).expect("player");
        (0..64)
            .flat_map(|y| (0..64).map(move |x| CellPos::new(x, y)))
            .find(|&origin| {
                check_placement(
                    grid,
                    definitions,
                    state,
                    footprints.iter().copied(),
                    kind,
                    origin,
                )
                .is_ok()
            })
            .unwrap_or_else(|| panic!("no place for {id}"))
    };
    let place = issued(
        P0,
        Command::PlaceBuilding {
            building: kind,
            origin,
        },
    );
    assert_eq!(rejections(&tick(app, vec![place])), NONE, "place {id}");
}

/// Queues the unit `id` for P0 and waits until it has left its factory.
fn train(app: &mut App, data: &Data, id: &str) {
    let item = data.item(id);
    let queue = issued(P0, Command::QueueProduction { item });
    assert_eq!(rejections(&tick(app, vec![queue])), NONE, "queue {id}");
    for _ in 0..4800 {
        let events = tick(app, Vec::new());
        assert_eq!(rejections(&events), NONE, "{id}");
        if events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitSpawned { player, .. } if *player == P0))
            && counts(app, P0).contains_key(id)
        {
            return;
        }
    }
    panic!("{id} did not leave its factory");
}

#[test]
fn an_allied_player_builds_the_allied_tree_to_the_battle_lab() {
    let data = data();
    let mut app = base(&data, "france", &["construction_yard"]);
    for id in [
        "power_plant",
        "refinery",
        "barracks",
        "war_factory",
        "power_plant",
        "airforce_command",
        "tech_center",
    ] {
        build(&mut app, &data, id);
    }
    let owned: BTreeSet<String> = buildings(&mut app, P0)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(owned.contains("tech_center") && owned.contains("airforce_command"));
    // With the Battle Lab the late Allied items open, and France's Grand Cannon; the Soviet
    // tree and the specials of other countries stay closed.
    for id in ["gi", "grizzly", "prism_tank", "harrier"] {
        train(&mut app, &data, id);
    }
    build(&mut app, &data, "grand_cannon");
    let state = app
        .world()
        .resource::<Players>()
        .get(P0)
        .expect("P0")
        .clone();
    let kinds: Vec<BuildingKind> = owned.iter().map(|id| data.building(id)).collect();
    let check = |id: &str| check_buildable(&data.definitions, &state, true, &kinds, data.item(id));
    for id in [
        "chronosphere",
        "weather_control",
        "spy_satellite",
        "mirage_tank",
        "tanya",
        "spy",
        "mcv",
    ] {
        if id == "mcv" {
            // The MCV needs a Service Depot.
            assert_eq!(
                check(id),
                Err(RejectReason::MissingPrerequisite {
                    item: data.item(id),
                    prerequisite: data.building("service_depot"),
                })
            );
        } else {
            assert_eq!(check(id), Ok(()), "{id}");
        }
    }
    assert_eq!(
        check("tank_destroyer"),
        Err(RejectReason::CountryRequired(data.item("tank_destroyer")))
    );
    assert_eq!(
        check("black_eagle"),
        Err(RejectReason::CountryRequired(data.item("black_eagle")))
    );
    assert_eq!(
        check("american_airforce_command"),
        Err(RejectReason::CountryRequired(
            data.item("american_airforce_command")
        ))
    );
    for id in [
        "tesla_reactor",
        "conscript",
        "rhino",
        "kirov",
        "soviet_battle_lab",
    ] {
        assert!(
            matches!(check(id), Err(RejectReason::MissingPrerequisite { .. })),
            "{id}"
        );
    }
    assert_eq!(
        check("chrono_commando"),
        Err(RejectReason::UnknownItem(data.item("chrono_commando")))
    );
}

#[test]
fn a_soviet_player_builds_the_soviet_tree_to_the_battle_lab() {
    let data = data();
    let mut app = base(&data, "russia", &["soviet_construction_yard"]);
    for id in [
        "tesla_reactor",
        "soviet_refinery",
        "soviet_barracks",
        "soviet_war_factory",
        "tesla_reactor",
        "radar_tower",
        "soviet_battle_lab",
    ] {
        build(&mut app, &data, id);
    }
    let owned: BTreeSet<String> = buildings(&mut app, P0)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(owned.contains("soviet_battle_lab") && owned.contains("radar_tower"));
    for id in ["conscript", "rhino", "tesla_tank", "kirov"] {
        train(&mut app, &data, id);
    }
    let state = app
        .world()
        .resource::<Players>()
        .get(P0)
        .expect("P0")
        .clone();
    let kinds: Vec<BuildingKind> = owned.iter().map(|id| data.building(id)).collect();
    let check = |id: &str| check_buildable(&data.definitions, &state, true, &kinds, data.item(id));
    for id in [
        "nuclear_reactor",
        "nuclear_silo",
        "iron_curtain",
        "apocalypse",
        "yuri",
        "v3_launcher",
        "tesla_coil",
    ] {
        assert_eq!(check(id), Ok(()), "{id}");
    }
    assert_eq!(
        check("desolator"),
        Err(RejectReason::CountryRequired(data.item("desolator")))
    );
    for id in ["power_plant", "gi", "grizzly", "tech_center"] {
        assert!(
            matches!(check(id), Err(RejectReason::MissingPrerequisite { .. })),
            "{id}"
        );
    }
    assert_eq!(
        check("yuri_prime"),
        Err(RejectReason::UnknownItem(data.item("yuri_prime")))
    );
}

#[test]
fn korea_flies_the_black_eagle_and_america_has_its_own_airforce_command() {
    let data = data();
    let airfield = [
        "construction_yard",
        "power_plant",
        "refinery",
        "airforce_command",
    ];
    // Korea: the Black Eagle, and not the Harrier, through the commands.
    let mut korea = base(&data, "korea", &airfield);
    train(&mut korea, &data, "black_eagle");
    let harrier = data.item("harrier");
    let queue = issued(P0, Command::QueueProduction { item: harrier });
    assert_eq!(
        rejections(&tick(&mut korea, vec![queue])),
        [RejectReason::CountryForbidden(harrier)]
    );
    // Germany: the Harrier, and not the Black Eagle.
    let mut germany = base(&data, "germany", &airfield);
    train(&mut germany, &data, "harrier");
    let eagle = data.item("black_eagle");
    let queue = issued(P0, Command::QueueProduction { item: eagle });
    assert_eq!(
        rejections(&tick(&mut germany, vec![queue])),
        [RejectReason::CountryRequired(eagle)]
    );
    // America builds its own Airforce Command, which also trains the Harrier, and may not
    // build the common one; for every other country it is the other way round.
    let mut america = base(
        &data,
        "america",
        &["construction_yard", "power_plant", "refinery"],
    );
    let common = data.item("airforce_command");
    let queue = issued(P0, Command::QueueProduction { item: common });
    assert_eq!(
        rejections(&tick(&mut america, vec![queue])),
        [RejectReason::CountryForbidden(common)]
    );
    build(&mut america, &data, "american_airforce_command");
    train(&mut america, &data, "harrier");
    let mut france = base(
        &data,
        "france",
        &["construction_yard", "power_plant", "refinery"],
    );
    let own = data.item("american_airforce_command");
    let queue = issued(P0, Command::QueueProduction { item: own });
    assert_eq!(
        rejections(&tick(&mut france, vec![queue])),
        [RejectReason::CountryRequired(own)]
    );
}

#[test]
fn a_captured_soviet_barracks_trains_conscripts_for_an_allied_player() {
    // A factory's side is that of its building type (assumption RA-A3): an Allied player who
    // owns a Soviet Barracks (captured; here by scenario setup) trains Conscripts, engineers and
    // dogs there, and still no national special of a Soviet country.
    let data = data();
    let mut app = base(
        &data,
        "america",
        &[
            "construction_yard",
            "power_plant",
            "soviet_barracks",
            "radar_tower",
        ],
    );
    for id in ["conscript", "engineer", "attack_dog", "flak_trooper"] {
        train(&mut app, &data, id);
    }
    let events = tick(
        &mut app,
        ["gi", "desolator", "terrorist"]
            .map(|id| {
                issued(
                    P0,
                    Command::QueueProduction {
                        item: data.item(id),
                    },
                )
            })
            .to_vec(),
    );
    assert_eq!(
        rejections(&events),
        [
            RejectReason::MissingPrerequisite {
                item: data.item("gi"),
                prerequisite: data.building("barracks"),
            },
            RejectReason::CountryRequired(data.item("desolator")),
            RejectReason::CountryRequired(data.item("terrorist")),
        ]
    );
}
