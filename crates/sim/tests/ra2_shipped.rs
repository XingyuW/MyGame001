//! RA2 with the shipped data: build times in a match, the two lanes, what low power stops, radar
//! and the spy's power outage, selling and repair, the Service Depot, and where a shipyard can
//! stand on the shipped maps (docs/RA2_PARITY_PLAN.md milestone RA2, "Tests" and "Acceptance";
//! docs/DECISIONS.md R22 to R27).
//!
//! These tests assert the shipped behaviour, so they load `assets/data` as the game does
//! (`sim::load_ruleset`) and the shipped maps, and drive `CommandQueue` and `SimSchedule`. The
//! rules themselves are tested on fixture data in the other `ra2_*.rs` files; the numbers are
//! audited in `ra9_data_parity.rs`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Definitions, Domain, Footprint,
    Grid, Health, IssuedCommand, Lane, Owner, PlayerId, Players, ProductionItem, RejectReason,
    SHIPYARD_PLACEMENT_DISTANCE, SimEvent, SimId, SimPlugin, SimSchedule, Unit, check_placement,
    load_map, load_ruleset, load_skirmish, map_asset_path, placement_reach, spawn_building,
    spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

/// No rejection.
const NONE: [RejectReason; 0] = [];

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn shipped() -> Definitions {
    load_ruleset(assets().join("data")).unwrap_or_else(|error| panic!("{error}"))
}

/// A world of 48 x 48 open cells with the shipped data and the players P0 (America) and P1
/// (Russia), who are enemies, each with `credits` credits.
fn world(credits: i64) -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    let cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, vec![cell; 48 * 48]).expect("a grid"));
    for (player, country) in [(P0, "america"), (P1, "russia")] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, credits).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

fn kind(app: &App, id: &str) -> BuildingKind {
    defs(app)
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

fn item(app: &App, id: &str) -> ProductionItem {
    let definitions = defs(app);
    definitions
        .building_kind(id)
        .map(ProductionItem::Building)
        .or_else(|| definitions.unit_kind(id).map(ProductionItem::Unit))
        .unwrap_or_else(|| panic!("item {id}"))
}

fn place(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn spawn(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
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

fn ticks(app: &mut App, count: u32) -> Vec<SimEvent> {
    (0..count).flat_map(|_| tick(app, Vec::new())).collect()
}

fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

fn queue(app: &App, player: PlayerId, id: &str) -> IssuedCommand {
    issue(
        player,
        Command::QueueProduction {
            item: item(app, id),
        },
    )
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

fn state(app: &App, player: PlayerId) -> sim::PlayerState {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .clone()
}

fn credits(app: &App, player: PlayerId) -> i64 {
    state(app, player).credits()
}

fn entity(app: &mut App, id: SimId) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &SimId)>();
    query
        .iter(app.world())
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no entity {}", id.0))
}

fn hit_points(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id);
    app.world()
        .get::<Health>(entity)
        .expect("health")
        .hit_points
}

fn set_hit_points(app: &mut App, id: SimId, value: i32) {
    let entity = entity(app, id);
    app.world_mut()
        .get_mut::<Health>(entity)
        .expect("health")
        .hit_points = value;
}

fn units_of(app: &mut App, player: PlayerId, id: &str) -> usize {
    let kind = defs(app).unit_kind(id).expect("unit");
    let mut query = app.world_mut().query::<(&Owner, &Unit)>();
    query
        .iter(app.world())
        .filter(|(owner, unit)| owner.0 == player && unit.kind == kind)
        .count()
}

/// The first origin, row by row, where `player` may place its ready building of type `id` now.
fn site(app: &mut App, player: PlayerId, id: &str) -> CellPos {
    let kind = kind(app, id);
    let owned: Vec<(BuildingKind, Footprint)> = {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        query
            .iter(app.world())
            .filter(|(owner, _)| owner.0 == player)
            .map(|(_, building)| (building.kind, building.footprint))
            .collect()
    };
    let world = app.world();
    let (grid, definitions) = (world.resource::<Grid>(), world.resource::<Definitions>());
    let state = world.resource::<Players>().get(player).expect("player");
    (0..48)
        .flat_map(|y| (0..48).map(move |x| CellPos::new(x, y)))
        .find(|&origin| {
            check_placement(
                grid,
                definitions,
                state,
                owned.iter().copied(),
                kind,
                origin,
            )
            .is_ok()
        })
        .unwrap_or_else(|| panic!("no place for {id}"))
}

// --- Where a shipyard can stand (plan RA2 item 7) --------------------------------------------------

/// Whether a shipyard may cover `footprint` on the unoccupied `grid`: open water under every
/// cell, and beside it a cell a ground unit and a cell a ship can stand on (the rule of
/// `PlaceBuilding` for a shipyard, `building::check_building_footprint`).
fn shipyard_site(grid: &Grid, footprint: Footprint) -> bool {
    footprint.inside(grid)
        && footprint
            .cells()
            .all(|cell| grid.cell(cell).is_some_and(|cell| cell.water))
        && footprint.ring().any(|cell| grid.is_passable(cell))
        && footprint
            .ring()
            .any(|cell| grid.passable_for(cell, Domain::Naval))
}

/// For every start of every shipped map as its file defines it: the distance
/// (`Footprint::distance`) from the Construction Yard at the start to the nearest place for a
/// shipyard, or `None` for a map without one.
fn shipyard_distances() -> BTreeMap<String, Vec<Option<u32>>> {
    let definitions = shipped();
    let skirmish = load_skirmish(assets().join("data/skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let yard = definitions
        .building(
            definitions
                .building_kind("construction_yard")
                .expect("yard"),
        )
        .expect("yard");
    let shipyard = definitions
        .building(
            definitions
                .building_kind("naval_shipyard")
                .expect("shipyard"),
        )
        .expect("shipyard");
    let mut distances = BTreeMap::new();
    for map in &skirmish.maps {
        let grid = load_map(assets().join(map_asset_path(&map.id)))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        let sites: Vec<Footprint> = (0..grid.height() as i32)
            .flat_map(|y| (0..grid.width() as i32).map(move |x| CellPos::new(x, y)))
            .map(|origin| Footprint {
                origin,
                width: shipyard.width,
                height: shipyard.height,
            })
            .filter(|&footprint| shipyard_site(&grid, footprint))
            .collect();
        let per_start = map
            .starts
            .iter()
            .map(|&origin| {
                let base = Footprint {
                    origin,
                    width: yard.width,
                    height: yard.height,
                };
                sites.iter().map(|site| site.distance(&base)).min()
            })
            .collect();
        distances.insert(map.id.clone(), per_start);
    }
    distances
}

#[test]
fn shipyard_water_lies_farther_from_the_starts_than_the_shipyards_adjacent_distance() {
    // The measurement behind docs/DECISIONS.md R25. The ini lets a shipyard stand 12 free cells
    // from a base building (`[GAYARD] Adjacent=12`), which is a distance of 13 between the
    // footprints. On the shipped maps that reaches water from the Construction Yard at 28 of
    // the 176 start positions, and from the first start (the human player's) on 2 of the 21
    // maps; the game's 31 free cells reach it at 132, and from the first start on 18.
    let distances = shipyard_distances();
    let all: Vec<u32> = distances
        .values()
        .flatten()
        .map(|distance| distance.expect("every shipped map has water for a shipyard"))
        .collect();
    let first: Vec<u32> = distances
        .values()
        .map(|per_start| per_start[0].expect("water"))
        .collect();
    let within = |distances: &[u32], reach: u32| distances.iter().filter(|&&d| d <= reach).count();
    assert_eq!((distances.len(), all.len()), (21, 176));
    assert_eq!((within(&all, 13), within(&first, 13)), (28, 2));
    assert_eq!(
        (
            within(&all, SHIPYARD_PLACEMENT_DISTANCE),
            within(&first, SHIPYARD_PLACEMENT_DISTANCE)
        ),
        (132, 18)
    );
    // So the shipped shipyards keep the reach they had: 31 free cells, a listed difference
    // (`ra2_differences.tsv`), where the reference has 12.
    let definitions = shipped();
    for id in ["naval_shipyard", "soviet_naval_shipyard"] {
        let kind = definitions.building_kind(id).expect("shipyard");
        let entry = definitions
            .metadata(ProductionItem::Building(kind))
            .expect("entry");
        assert_eq!(entry.adjacent, 31, "{id}");
        assert_eq!(
            placement_reach(&definitions, kind),
            SHIPYARD_PLACEMENT_DISTANCE,
            "{id}"
        );
    }
}

// --- Production -----------------------------------------------------------------------------------

/// Queues the building `id` for P0, waits until it is ready, places it on the first free site,
/// and returns the ticks that took (the tick of the queue command to the tick of the placement).
fn build(app: &mut App, id: &str) -> u32 {
    let command = queue(app, P0, id);
    let mut count = 1;
    assert_eq!(rejections(&tick(app, vec![command])), NONE, "queue {id}");
    let key = defs(app).queue_of(item(app, id)).expect("a queue");
    let time = defs(app)
        .production(item(app, id))
        .expect("terms")
        .build_time;
    while state(app, P0).lane(key)[0].progress < time {
        assert_eq!(rejections(&tick(app, Vec::new())), NONE, "build {id}");
        count += 1;
        assert!(count <= 2 * time, "{id} is not built");
    }
    let origin = site(app, P0, id);
    let building = kind(app, id);
    let events = tick(
        app,
        vec![issue(P0, Command::PlaceBuilding { building, origin })],
    );
    assert_eq!(rejections(&events), NONE, "place {id}");
    count + 1
}

#[test]
fn a_base_is_built_from_an_mcv_in_the_time_the_build_time_formula_gives() {
    // 10000 credits and an MCV, as a default match starts. Power Plant (800 credits), Barracks
    // (500), Ore Refinery (2000) and War Factory (2000) one after the other, each taking
    // round(cost x 21 / 25) ticks, with a Pillbox (500) built beside the refinery in the lane
    // of the defences.
    let mut app = world(10_000);
    let mcv = spawn(&mut app, P0, "mcv", 20, 20);
    tick(&mut app, Vec::new());
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: mcv })]);
    assert_eq!(rejections(&events), NONE);
    let time = |app: &App, id: &str| {
        defs(app)
            .production(item(app, id))
            .expect("terms")
            .build_time
    };
    assert_eq!(
        [
            "power_plant",
            "barracks",
            "refinery",
            "war_factory",
            "pillbox"
        ]
        .map(|id| time(&app, id)),
        [672, 420, 1680, 1680, 420]
    );
    let mut total = 0;
    for id in ["power_plant", "barracks"] {
        let took = build(&mut app, id);
        assert_eq!(
            took,
            time(&app, id) + 1,
            "{id}: its build time and the placement"
        );
        total += took;
    }
    // The pillbox and the refinery side by side: neither waits for the other.
    let pillbox = queue(&app, P0, "pillbox");
    assert_eq!(rejections(&tick(&mut app, vec![pillbox])), NONE);
    total += 1;
    let took = build(&mut app, "refinery");
    assert_eq!(took, 1680 + 1);
    total += took;
    let lanes = state(&app, P0);
    let defences = lanes
        .queues()
        .find(|(key, _)| key.lane == Lane::Defences)
        .map(|(_, items)| items[0])
        .expect("the pillbox waits in the lane of the defences");
    assert_eq!(
        (defences.item, defences.progress),
        (item(&app, "pillbox"), 420)
    );
    let origin = site(&mut app, P0, "pillbox");
    let building = kind(&app, "pillbox");
    let events = tick(
        &mut app,
        vec![issue(P0, Command::PlaceBuilding { building, origin })],
    );
    assert_eq!(rejections(&events), NONE);
    total += 1;
    total += build(&mut app, "war_factory");
    // 672 + 420 + 1680 + 1680 ticks of building, and a tick for each of the six commands
    // that queue the pillbox and place the five buildings.
    assert_eq!(total, 672 + 420 + 1680 + 1680 + 6);
    assert_eq!(credits(&app, P0), 10_000 - 800 - 500 - 2000 - 2000 - 500);
    assert!(!state(&app, P0).low_power());

    // A Grizzly (700 credits) takes 588 ticks at one War Factory, and 588 / 1.25 at two.
    let grizzly = |app: &mut App| {
        let command = queue(app, P0, "grizzly");
        let before = units_of(app, P0, "grizzly");
        assert_eq!(rejections(&tick(app, vec![command])), NONE);
        let mut count = 1;
        while units_of(app, P0, "grizzly") == before {
            tick(app, Vec::new());
            count += 1;
            assert!(count < 2_000);
        }
        count
    };
    assert_eq!(grizzly(&mut app), 588);
    place(&mut app, P0, "war_factory", 40, 40);
    assert_eq!(grizzly(&mut app), 471);
    assert_eq!(credits(&app, P0), 10_000 - 5_800 - 1_400);
}

#[test]
fn a_computer_player_builds_in_both_lanes_of_its_yard() {
    // Two computer players of the shipped data on a shipped map: the Allied one has a Pillbox
    // in its complete defence plan, and builds it beside its other buildings.
    let definitions = shipped();
    let dir = assets().join("data");
    let skirmish =
        load_skirmish(dir.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = sim::load_ai(dir.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let map = skirmish
        .maps
        .iter()
        .position(|map| map.id == "crossroads")
        .expect("the map");
    let grid = load_map(assets().join(map_asset_path("crossroads")))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    let faction = |id: &str| {
        skirmish
            .factions
            .iter()
            .position(|faction| faction.id == id)
            .expect("country")
    };
    let players: Vec<sim::PlayerSetup> = [(P0, "france"), (P1, "cuba")]
        .into_iter()
        .enumerate()
        .map(|(start, (player, country))| sim::PlayerSetup {
            player,
            faction: faction(country),
            start,
            controller: sim::Controller::Ai { difficulty: 1 },
        })
        .collect();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 5 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions);
    sim::setup_match(app.world_mut(), &skirmish, &ai, map, &players)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut both = None;
    for at in 0..6_000 {
        app.world_mut().run_schedule(SimSchedule);
        let state = state(&app, P0);
        assert!(state.credits() >= 0);
        let busy = |lane: Lane| {
            state
                .queues()
                .any(|(key, items)| key.lane == lane && !items.is_empty())
        };
        // A lane of buildings never holds more than one item.
        for (key, items) in state.queues() {
            let buildings = matches!(items[0].item, ProductionItem::Building(_));
            assert!(!buildings || items.len() == 1, "{key:?} at tick {at}");
        }
        if busy(Lane::Defences) {
            let main = state.queues().any(|(_, items)| {
                defs(&app).lane(items[0].item) == Lane::Main
                    && matches!(items[0].item, ProductionItem::Building(_))
            });
            if main {
                both = Some(at);
                break;
            }
        }
    }
    eprintln!(
        "first default main/defence overlap tick {:?}",
        both.map(|at| at + 1)
    );
    assert!(
        both.is_some(),
        "a building and a defence in production at once"
    );
}

// --- Power and radar ------------------------------------------------------------------------------

#[test]
fn low_power_stops_the_powered_buildings_that_consume_power_and_no_others() {
    // No power building, and a Prism Tower that consumes 75.
    let mut app = world(1_000);
    let pillbox = place(&mut app, P0, "pillbox", 10, 10);
    let prism = place(&mut app, P0, "prism_tower", 12, 10);
    spawn(&mut app, P1, "rhino", 11, 13);
    let events = ticks(&mut app, 40);
    let fired = |id: SimId| {
        events
            .iter()
            .any(|event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == id))
    };
    let low = state(&app, P0);
    assert!(low.low_power());
    assert!(fired(pillbox), "[GAPILL] is not Powered");
    assert!(!fired(prism), "[ATESLA] Powered=yes, Power=-75");
    let definitions = defs(&app);
    let works = |id: &str| low.powers(&definitions, kind(&app, id));
    for id in [
        "pillbox",
        "sentry_gun",
        "cloning_vats",
        "nuclear_silo",
        "barracks",
        "war_factory",
        "refinery",
        "service_depot",
        "airforce_command",
        "construction_yard",
    ] {
        assert!(works(id), "{id} works under low power");
    }
    for id in [
        "prism_tower",
        "tesla_coil",
        "patriot",
        "flak_cannon",
        "grand_cannon",
        "gap_generator",
        "spy_satellite",
        "psychic_sensor",
        "ore_purifier",
        "radar_tower",
        "chronosphere",
        "weather_control",
        "iron_curtain",
        "naval_shipyard",
    ] {
        assert!(!works(id), "{id} stops under low power");
    }
    // The four superweapons wait for power with their charge; a support power would not.
    for id in [
        "chronosphere",
        "weather_control",
        "iron_curtain",
        "nuclear_silo",
    ] {
        let entry = definitions.metadata(item(&app, id)).expect("entry");
        assert!(entry.is_powered && entry.build_limit == 1, "{id}");
    }
    for id in ["ore_purifier", "cloning_vats"] {
        let entry = definitions.metadata(item(&app, id)).expect("entry");
        assert_eq!(entry.build_limit, 1, "{id}");
    }
}

#[test]
fn radar_needs_a_radar_building_and_power_and_a_spy_in_a_power_plant_takes_it_away() {
    let mut app = world(1_000);
    let plant = place(&mut app, P0, "power_plant", 10, 10);
    tick(&mut app, Vec::new());
    assert!(!state(&app, P0).radar(), "no radar building");
    for id in [
        "airforce_command",
        "american_airforce_command",
        "radar_tower",
        "spy_satellite",
    ] {
        assert!(
            defs(&app).metadata(item(&app, id)).expect("entry").radar,
            "{id}"
        );
    }
    place(&mut app, P0, "american_airforce_command", 20, 10);
    tick(&mut app, Vec::new());
    assert!(state(&app, P0).radar());
    // A spy of the other player walks into the power plant: no power for a while, no radar.
    let spy = spawn(&mut app, P1, "spy", 12, 12);
    tick(&mut app, Vec::new());
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: spy,
                target: plant,
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    let outage = state(&app, P0);
    assert_eq!(outage.power_produced(), 0);
    assert!(outage.low_power() && !outage.radar());
    // The outage ends, and the radar is back.
    let mut back = None;
    for at in 0..2_000 {
        tick(&mut app, Vec::new());
        if state(&app, P0).radar() {
            back = Some(at);
            break;
        }
    }
    assert!(back.is_some_and(|at| at > 100), "{back:?}");
    assert_eq!(state(&app, P0).power_produced(), 200);
}

// --- Selling, repair and the Service Depot ------------------------------------------------------

#[test]
fn a_sale_refunds_half_the_cost_and_walls_are_neither_sold_nor_repaired() {
    let mut app = world(1_000);
    let plant = place(&mut app, P0, "power_plant", 10, 10);
    let walls = [
        place(&mut app, P0, "fortress_walls", 14, 10),
        place(&mut app, P0, "soviet_walls", 16, 10),
    ];
    tick(&mut app, Vec::new());
    for wall in walls {
        set_hit_points(&mut app, wall, 100);
        let events = tick(
            &mut app,
            vec![
                issue(P0, Command::SellBuilding { building: wall }),
                issue(P0, Command::RepairBuilding { building: wall }),
            ],
        );
        assert_eq!(
            rejections(&events),
            [
                RejectReason::NotSellable(wall),
                RejectReason::NotRepairable(wall)
            ]
        );
        ticks(&mut app, 40);
        assert_eq!(hit_points(&mut app, wall), 100);
    }
    assert_eq!(credits(&app, P0), 1_000);
    // [General] RefundPercent=50%: 400 for a Power Plant of 800.
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: plant })],
    );
    assert!(events.contains(&SimEvent::BuildingSold {
        player: P0,
        building: plant,
        refund: 400
    }));
    assert_eq!(credits(&app, P0), 1_400);
}

#[test]
fn a_construction_yard_is_repaired_in_steps_of_eight_hit_points_for_three_credits() {
    // The plan's example (appendix D, topic 9): 1000 hit points and 3000 credits give
    // 3000 div (1000 div 8) x 15% = 3 credits per step of 8 hit points, every 19 ticks.
    let mut app = world(1_000);
    let yard = place(&mut app, P0, "construction_yard", 10, 10);
    tick(&mut app, Vec::new());
    set_hit_points(&mut app, yard, 1000 - 84);
    let events = tick(
        &mut app,
        vec![issue(P0, Command::RepairBuilding { building: yard })],
    );
    assert_eq!(rejections(&events), NONE);
    let mut steps = Vec::new();
    let (mut hp, mut money) = (hit_points(&mut app, yard), credits(&app, P0));
    for _ in 0..19 * 12 {
        tick(&mut app, Vec::new());
        let (now, left) = (hit_points(&mut app, yard), credits(&app, P0));
        if now != hp {
            steps.push((now - hp, money - left));
        }
        (hp, money) = (now, left);
    }
    // Ten whole steps and a last one of 4 hit points at the price of a step.
    let mut expected = vec![(8, 3); 10];
    expected.push((4, 3));
    assert_eq!(steps, expected);
    assert_eq!(hit_points(&mut app, yard), 1000);
    assert!(state(&app, P0).repairing().is_empty());
}

#[test]
fn a_damaged_tank_sent_to_the_service_depot_is_repaired_for_credits() {
    // A Grizzly: 700 credits, 300 hit points: 700 div (300 div 8) x 15% = 2 credits per step
    // of 8 hit points, every 19 ticks ([General] URepairRate=.016).
    let mut app = world(1_000);
    place(&mut app, P0, "power_plant", 4, 4);
    let depot = place(&mut app, P0, "service_depot", 20, 20);
    let near = spawn(&mut app, P0, "grizzly", 22, 20);
    let far = spawn(&mut app, P0, "grizzly", 30, 30);
    let idle = spawn(&mut app, P0, "grizzly", 19, 20);
    for tank in [near, far, idle] {
        set_hit_points(&mut app, tank, 300 - 40);
    }
    tick(&mut app, Vec::new());
    let order = |unit| {
        issue(
            P0,
            Command::Interact {
                unit,
                target: depot,
            },
        )
    };
    let events = tick(&mut app, vec![order(near), order(far)]);
    assert_eq!(rejections(&events), NONE);
    ticks(&mut app, 19 * 5 + 400);
    // Both were repaired, each for 5 steps of 2 credits; the one that was never sent was not.
    assert_eq!(hit_points(&mut app, near), 300);
    assert_eq!(hit_points(&mut app, far), 300);
    assert_eq!(
        hit_points(&mut app, idle),
        300 - 40,
        "no free repair beside a depot"
    );
    assert_eq!(credits(&app, P0), 1_000 - 2 * 5 * 2);
    // A whole tank is not taken, and the depot of another player is not for mine.
    // (It stands in the sight of the tank that was never sent, and out of its range.)
    let theirs = place(&mut app, P1, "soviet_service_depot", 10, 20);
    tick(&mut app, Vec::new());
    let events = tick(
        &mut app,
        vec![
            order(near),
            issue(
                P0,
                Command::Interact {
                    unit: idle,
                    target: theirs,
                },
            ),
        ],
    );
    assert_eq!(
        rejections(&events),
        [
            RejectReason::AbilityUnavailable("This unit is not damaged"),
            RejectReason::AbilityUnavailable("Choose your own repair building")
        ]
    );
}
