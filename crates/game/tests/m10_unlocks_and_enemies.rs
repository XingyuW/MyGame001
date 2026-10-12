//! Follow-up of 2026-10-05: unlock paths, the "unlock all" match option and several independent
//! computer players, with the legacy ruleset fixture (the data shipped before the RA2 parity
//! programme, arsenal included: `crates/sim/tests/fixtures/data/legacy`, docs/DECISIONS.md R15)
//! on the shipped maps.
//!
//! Every test drives the simulation only through the player's commands (`CommandQueue`), as the
//! interface does: production is queued, progresses, is placed or leaves its producer, and the
//! spawned units are then ordered to move. Nothing is spawned directly.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::app::App;
use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use sim::{
    AiDefinitions, AiPlayers, Building, BuildingKind, CellPos, Command, CommandQueue, Controller,
    Definitions, Footprint, Grid, IssuedCommand, MatchOutcome, MatchState, Mobile, Owner, PlayerId,
    PlayerSetup, Players, ProductionItem, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule,
    SkirmishDefinitions, Unit, check_placement, load_ai, load_map, load_match_definitions,
    load_skirmish, map_asset_path, setup_match_with_options, state_hash,
};

const HUMAN: PlayerId = PlayerId(0);

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

/// The legacy ruleset fixture of the `sim` crate.
fn legacy() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim/tests/fixtures/data/legacy")
}

fn data() -> Data {
    let dir = legacy();
    let definitions = load_match_definitions(
        dir.join("units.ron"),
        dir.join("economy.ron"),
        dir.join("combat.ron"),
    )
    .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(dir.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(dir.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

fn map_index(data: &Data, id: &str) -> usize {
    data.skirmish
        .maps
        .iter()
        .position(|map| map.id == id)
        .unwrap_or_else(|| panic!("map {id}"))
}

fn grid(data: &Data, map: usize) -> Grid {
    load_map(assets().join(map_asset_path(&data.skirmish.maps[map].id)))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid
}

/// A match on `map` set up as the menu does: `players` with their controllers, Super Me and
/// "unlock all" for the listed humans.
fn start(
    data: &Data,
    map: usize,
    players: &[PlayerSetup],
    super_me: &[PlayerId],
    unlock: &[PlayerId],
    seed: u64,
) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    app.world_mut().insert_resource(grid(data, map));
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match_with_options(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        map,
        players,
        super_me,
        unlock,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    app
}

fn human(faction: usize) -> PlayerSetup {
    PlayerSetup {
        player: HUMAN,
        faction,
        start: 0,
        controller: Controller::Human,
    }
}

fn computer(k: u8, faction: usize, difficulty: usize) -> PlayerSetup {
    PlayerSetup {
        player: PlayerId(k),
        faction,
        start: usize::from(k),
        controller: Controller::Ai { difficulty },
    }
}

/// Runs one tick with `commands` of the human player; returns its events.
fn tick(world: &mut World, commands: Vec<Command>) -> Vec<SimEvent> {
    {
        let mut queue = world.resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(IssuedCommand {
                player: HUMAN,
                command,
            });
        }
    }
    world.run_schedule(SimSchedule);
    world
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { player, reason, .. } if *player == HUMAN => {
                Some(reason.clone())
            }
            _ => None,
        })
        .collect()
}

fn building(data: &Data, id: &str) -> BuildingKind {
    data.definitions
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

fn unit_item(data: &Data, id: &str) -> ProductionItem {
    ProductionItem::Unit(
        data.definitions
            .unit_kind(id)
            .unwrap_or_else(|| panic!("unit {id}")),
    )
}

fn footprints(world: &mut World, player: PlayerId) -> Vec<Footprint> {
    world
        .query::<(&Owner, &Building)>()
        .iter(world)
        .filter(|(owner, _)| owner.0 == player)
        .map(|(_, building)| building.footprint)
        .collect()
}

fn owned_buildings(world: &mut World, player: PlayerId) -> BTreeSet<BuildingKind> {
    world
        .query::<(&Owner, &Building)>()
        .iter(world)
        .filter(|(owner, _)| owner.0 == player)
        .map(|(_, building)| building.kind)
        .collect()
}

fn units_of(world: &mut World, player: PlayerId) -> BTreeMap<SimId, (sim::UnitKind, CellPos)> {
    world
        .query::<(&SimId, &Owner, &Unit, &Mobile)>()
        .iter(world)
        .filter(|(_, owner, ..)| owner.0 == player)
        .map(|(id, _, unit, mobile)| (*id, (unit.kind, mobile.cell)))
        .collect()
}

/// The free origin nearest to `near` (by Chebyshev distance, then row and column) where the
/// player may place `kind` now, as the interface's placement preview finds it.
fn site(world: &mut World, kind: BuildingKind, near: CellPos, reach: i32) -> Option<CellPos> {
    let owned = footprints(world, HUMAN);
    let grid = world.resource::<Grid>();
    let definitions = world.resource::<Definitions>();
    let state = world.resource::<Players>().get(HUMAN)?;
    let mut cells: Vec<CellPos> = (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| CellPos::new(near.x + dx, near.y + dy)))
        .collect();
    cells.sort_by_key(|c| ((c.x - near.x).abs().max((c.y - near.y).abs()), c.y, c.x));
    cells.into_iter().find(|&origin| {
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
}

/// Builds `id` through the commands: queue, wait until ready, place at the nearest valid site.
fn build(world: &mut World, data: &Data, id: &str, near: CellPos, reach: i32) -> SimId {
    let kind = building(data, id);
    let rejected = rejections(&tick(
        world,
        vec![Command::QueueProduction {
            item: ProductionItem::Building(kind),
        }],
    ));
    assert!(rejected.is_empty(), "queue {id}: {rejected:?}");
    let producer = data
        .definitions
        .building(kind)
        .expect("defined")
        .production
        .producer;
    for _ in 0..20_000 {
        let ready = world
            .resource::<Players>()
            .get(HUMAN)
            .and_then(|state| state.queue(producer).first().copied())
            .is_some_and(|head| {
                head.item == ProductionItem::Building(kind)
                    && head.progress
                        >= data
                            .definitions
                            .building(kind)
                            .expect("def")
                            .production
                            .build_time
            });
        if ready {
            break;
        }
        tick(world, Vec::new());
    }
    let origin = site(world, kind, near, reach).unwrap_or_else(|| panic!("no site for {id}"));
    let events = tick(
        world,
        vec![Command::PlaceBuilding {
            building: kind,
            origin,
        }],
    );
    assert!(rejections(&events).is_empty(), "place {id}: {events:?}");
    events
        .iter()
        .find_map(|event| match event {
            SimEvent::BuildingPlaced {
                player,
                building,
                kind: placed,
            } if *player == HUMAN && *placed == kind => Some(*building),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{id} was not placed"))
}

/// Trains unit `id` through the commands; returns the new unit once it has left its producer.
fn train(world: &mut World, data: &Data, id: &str) -> SimId {
    let item = unit_item(data, id);
    let rejected = rejections(&tick(world, vec![Command::QueueProduction { item }]));
    assert!(rejected.is_empty(), "queue {id}: {rejected:?}");
    let kind = match item {
        ProductionItem::Unit(kind) => kind,
        ProductionItem::Building(_) => unreachable!(),
    };
    for _ in 0..20_000 {
        for event in tick(world, Vec::new()) {
            if let SimEvent::UnitSpawned { player, unit, .. } = event
                && player == HUMAN
                && units_of(world, HUMAN)
                    .get(&unit)
                    .is_some_and(|(k, _)| *k == kind)
            {
                return unit;
            }
        }
    }
    panic!("{id} never left its producer");
}

/// Orders `unit` to move to the reachable cell `target` and checks that it gets there or at
/// least leaves its spawn cell (it is under the player's control).
fn control(world: &mut World, unit: SimId, target: CellPos) {
    let before = units_of(world, HUMAN)[&unit].1;
    let rejected = rejections(&tick(
        world,
        vec![Command::Move {
            units: vec![unit],
            target,
        }],
    ));
    assert!(rejected.is_empty(), "move {unit:?}: {rejected:?}");
    let mut failures = Vec::new();
    for _ in 0..400 {
        for event in tick(world, Vec::new()) {
            match event {
                SimEvent::MoveFailed {
                    unit: failed,
                    reason,
                } if failed == unit => failures.push(reason),
                SimEvent::MoveCompleted { unit: done, .. }
                    if done == unit
                        && units_of(world, HUMAN)
                            .get(&unit)
                            .is_some_and(|(_, cell)| *cell != before) =>
                {
                    return;
                }
                _ => {}
            }
        }
        if units_of(world, HUMAN)
            .get(&unit)
            .is_some_and(|(_, cell)| *cell != before)
        {
            return;
        }
    }
    let kind = units_of(world, HUMAN).get(&unit).map(|(kind, _)| *kind);
    let name = kind.and_then(|k| {
        world
            .resource::<Definitions>()
            .unit(k)
            .map(|d| d.id.clone())
    });
    panic!(
        "unit {unit:?} ({name:?}) at {before:?} did not respond to a move order to {target:?}: \
         {failures:?}"
    );
}

#[test]
fn every_human_production_item_has_a_reachable_unlock_path() {
    let data = data();
    let demo = map_index(&data, "demo");
    let mut app = start(&data, demo, &[human(0), computer(1, 1, 0)], &[], &[], 0);
    let roster: BTreeSet<ProductionItem> = app
        .world()
        .resource::<Players>()
        .get(HUMAN)
        .and_then(|state| state.roster().cloned())
        .expect("a roster");
    // The arsenal (Allied and Soviet) and both original faction rosters, so the Meridian's
    // heavy units that the arsenal-only roster had hidden are back.
    for id in [
        "grenadier",
        "raider",
        "heavy_tank",
        "rocket_launcher",
        "rifleman",
        "rocket_soldier",
        "scout",
        "light_tank",
        "artillery",
        "harvester",
        "conscript",
        "tesla_trooper",
        "apocalypse",
        "kirov",
        "dreadnought",
        "prism_tank",
        "tanya",
    ] {
        assert!(roster.contains(&unit_item(&data, id)), "{id} is offered");
    }
    for id in [
        "gun_turret",
        "rocket_tower",
        "radar_tower",
        "tesla_coil",
        "nuclear_silo",
    ] {
        assert!(
            roster.contains(&ProductionItem::Building(building(&data, id))),
            "{id} is offered"
        );
    }
    // Every offered item's producer and prerequisites are themselves offered, and every chain
    // ends at the construction yard the player starts with.
    let start = owned_buildings(app.world_mut(), HUMAN);
    let defs = &data.definitions;
    let mut reachable: BTreeSet<BuildingKind> = start.clone();
    loop {
        let before = reachable.len();
        for item in &roster {
            if let (ProductionItem::Building(kind), Some(terms)) = (*item, defs.production(*item))
                && reachable.contains(&terms.producer)
                && terms
                    .prerequisites
                    .iter()
                    .all(|k| defs.prerequisite_met(*k, reachable.iter().copied()))
            {
                reachable.insert(kind);
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    for item in &roster {
        let terms = defs.production(*item).expect("production terms");
        assert!(reachable.contains(&terms.producer), "{item:?}: producer");
        for k in &terms.prerequisites {
            assert!(
                defs.prerequisite_met(*k, reachable.iter().copied()),
                "{item:?}: prerequisite {k:?}"
            );
        }
    }
    // Naval units need a shipyard site within reach of a base on every map with open water.
    for map in ["demo", "crossroads", "riverlands"] {
        let index = map_index(&data, map);
        let grid = grid(&data, index);
        let start = data.skirmish.maps[index].starts[0];
        let yard = Footprint {
            origin: start,
            width: 3,
            height: 3,
        };
        let near = (0..grid.height() as i32 - 2)
            .flat_map(|y| (0..grid.width() as i32 - 2).map(move |x| CellPos::new(x, y)))
            .filter(|&origin| {
                let site = Footprint {
                    origin,
                    width: 3,
                    height: 3,
                };
                site.cells().all(|c| grid.cell(c).is_some_and(|c| c.water))
                    && site.ring().any(|c| grid.is_passable(c))
            })
            .map(|origin| {
                Footprint {
                    origin,
                    width: 3,
                    height: 3,
                }
                .distance(&yard)
            })
            .min()
            .expect("water");
        assert!(near <= sim::SHIPYARD_PLACEMENT_DISTANCE, "{map}: {near}");
    }
}

#[test]
fn allied_and_soviet_units_unlock_through_their_structures_and_obey_orders() {
    let data = data();
    let demo = map_index(&data, "demo");
    // A real match: the human player (with Super Me so the chain runs quickly; prerequisites,
    // producers, placement and exits still apply) against a computer player on the far side.
    let mut app = start(
        &data,
        demo,
        &[human(1), computer(1, 0, 0)],
        &[HUMAN],
        &[],
        3,
    );
    let world = app.world_mut();
    let base = data.skirmish.maps[demo].starts[0];
    let near = CellPos::new(base.x + 1, base.y + 1);
    // Before any structure: advanced units are locked by their prerequisites.
    let events = tick(
        world,
        vec![Command::QueueProduction {
            item: unit_item(&data, "tesla_trooper"),
        }],
    );
    assert!(matches!(
        rejections(&events).as_slice(),
        [RejectReason::MissingPrerequisite { .. }]
    ));
    for id in [
        "power_plant",
        "refinery",
        "barracks",
        "war_factory",
        "radar_tower",
    ] {
        build(world, &data, id, near, 12);
    }
    // Soviet radar-tier units, then Allied ones, from the same unified roster. Each one is
    // ordered away from its producer as soon as it leaves, as a player would rally it, which
    // also checks that it obeys orders.
    let rally = CellPos::new(base.x + 6, base.y - 8);
    let mut trained = Vec::new();
    for id in [
        "conscript",
        "tesla_trooper",
        "desolator",
        "rhino",
        "tesla_tank",
        "v3_launcher",
        "gi",
        "sniper",
        "grizzly",
        // The Meridian's and Vanguard's original units.
        "grenadier",
        "heavy_tank",
        "rifleman",
        "light_tank",
    ] {
        let unit = train(world, &data, id);
        control(world, unit, rally);
        trained.push((id, unit));
    }
    // The Battle Lab tier.
    build(world, &data, "tech_center", near, 12);
    build(world, &data, "airforce_command", near, 12);
    for id in [
        "apocalypse",
        "prism_tank",
        "tanya",
        "yuri",
        "rocket_launcher",
        "artillery",
        "harrier",
        "kirov",
    ] {
        let unit = train(world, &data, id);
        control(world, unit, rally);
        trained.push((id, unit));
    }
    for id in ["tesla_coil", "prism_tower", "rocket_tower", "gun_turret"] {
        build(world, &data, id, near, 12);
    }
    // Every produced unit is still the player's and alive.
    let units = units_of(world, HUMAN);
    for (id, unit) in &trained {
        assert!(units.contains_key(unit), "{id}");
    }
    // Naval: the nearest open-water site with shore (as a player looks for one), scouted by the
    // starting tanks, then a shipyard placed there within the shipyard reach.
    let yard = footprints(world, HUMAN)
        .into_iter()
        .find(|f| f.origin == base)
        .expect("yard");
    let site = {
        let grid = world.resource::<Grid>();
        (0..grid.height() as i32 - 2)
            .flat_map(|y| (0..grid.width() as i32 - 2).map(move |x| CellPos::new(x, y)))
            .map(|origin| Footprint {
                origin,
                width: 3,
                height: 3,
            })
            .filter(|f| {
                f.cells().all(|c| grid.cell(c).is_some_and(|c| c.water))
                    && f.ring().any(|c| grid.is_passable(c))
            })
            .min_by_key(|f| (f.distance(&yard), f.origin.y, f.origin.x))
            .expect("a shipyard site")
    };
    assert!(site.distance(&yard) <= sim::SHIPYARD_PLACEMENT_DISTANCE);
    let shore = {
        let grid = world.resource::<Grid>();
        site.ring().find(|&c| grid.is_passable(c)).expect("shore")
    };
    let scouts: Vec<SimId> = units_of(world, HUMAN)
        .into_iter()
        .filter(|(_, (kind, _))| {
            Some(*kind) == data.definitions.unit_kind("heavy_tank")
                || Some(*kind) == data.definitions.unit_kind("raider")
        })
        .map(|(id, _)| id)
        .collect();
    assert!(!scouts.is_empty());
    tick(
        world,
        vec![Command::Move {
            units: scouts,
            target: shore,
        }],
    );
    let mut seen = false;
    for _ in 0..3000 {
        tick(world, Vec::new());
        let players = world.resource::<Players>();
        if players
            .get(HUMAN)
            .is_some_and(|state| state.vision().sees_footprint(site))
        {
            seen = true;
            break;
        }
    }
    assert!(seen, "the scouts reveal the shipyard site {site:?}");
    build(world, &data, "naval_shipyard", site.origin, 0);
    for id in ["destroyer", "typhoon", "dreadnought"] {
        let ship = train(world, &data, id);
        let water = {
            let here = units_of(world, HUMAN)[&ship].1;
            let grid = world.resource::<Grid>();
            (3..9)
                .flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r)])
                .map(|(dx, dy)| CellPos::new(here.x + dx, here.y + dy))
                .find(|&c| grid.passable_for(c, sim::Domain::Naval) && grid.occupant(c).is_none())
                .expect("open water near the new ship")
        };
        control(world, ship, water);
    }
    // Nothing the human did changed the computer player.
    let ai = world.resource::<Players>().get(PlayerId(1)).expect("ai");
    assert!(!ai.super_me() && !ai.unlock_all());
}

#[test]
fn advanced_requirements_hold_and_unlock_all_waives_only_prerequisites() {
    let data = data();
    let demo = map_index(&data, "demo");
    // Standard rules: the Nuclear Silo needs the Nuclear Reactor itself; an ordinary power plant
    // no longer counts as one.
    let mut app = start(
        &data,
        demo,
        &[human(0), computer(1, 1, 0)],
        &[HUMAN],
        &[],
        1,
    );
    let world = app.world_mut();
    let base = data.skirmish.maps[demo].starts[0];
    let near = CellPos::new(base.x + 1, base.y + 1);
    for id in ["power_plant", "refinery", "war_factory", "tech_center"] {
        build(world, &data, id, near, 12);
    }
    let silo = ProductionItem::Building(building(&data, "nuclear_silo"));
    let rejected = rejections(&tick(world, vec![Command::QueueProduction { item: silo }]));
    assert!(
        matches!(
            rejected.as_slice(),
            [RejectReason::MissingPrerequisite { prerequisite, .. }]
                if *prerequisite == building(&data, "nuclear_reactor")
        ),
        "{rejected:?}"
    );
    build(world, &data, "nuclear_reactor", near, 12);
    build(world, &data, "nuclear_silo", near, 12);

    // "Unlock all": from the construction yard alone every structure may be queued, and a war
    // factory (still needed: units leave it) builds the Apocalypse at once.
    let mut app = start(
        &data,
        demo,
        &[human(1), computer(1, 0, 0)],
        &[HUMAN],
        &[HUMAN],
        2,
    );
    let world = app.world_mut();
    for id in ["tesla_coil", "war_factory"] {
        build(world, &data, id, near, 12);
    }
    let barracks_unit = rejections(&tick(
        world,
        vec![Command::QueueProduction {
            item: unit_item(&data, "yuri"),
        }],
    ));
    assert!(
        matches!(barracks_unit.as_slice(), [RejectReason::NoProducer { .. }]),
        "{barracks_unit:?}"
    );
    let tank = train(world, &data, "apocalypse");
    control(world, tank, CellPos::new(base.x + 6, base.y - 6));
    let players = world.resource::<Players>();
    assert!(players.get(HUMAN).expect("human").unlock_all());
    assert!(!players.get(PlayerId(1)).expect("ai").unlock_all());
    // Without the option the same order is refused for the missing prerequisites.
    let mut app = start(
        &data,
        demo,
        &[human(1), computer(1, 0, 0)],
        &[HUMAN],
        &[],
        2,
    );
    let rejected = rejections(&tick(
        app.world_mut(),
        vec![Command::QueueProduction {
            item: ProductionItem::Building(building(&data, "tesla_coil")),
        }],
    ));
    assert!(matches!(
        rejected.as_slice(),
        [RejectReason::MissingPrerequisite { .. }]
    ));
}

/// Players' independent state after `ticks` ticks of an all-computer match on `map` with `count`
/// players: each has its own buildings, credits, queues and AI controller.
fn check_independent(world: &mut World, count: usize) {
    let ids: Vec<PlayerId> = (0..count).map(|k| PlayerId(k as u8)).collect();
    let state = world.resource::<MatchState>();
    assert_eq!(state.players(), ids.as_slice());
    let ai = world.resource::<AiPlayers>();
    for &id in &ids {
        assert!(ai.controller(id).is_some(), "{id:?} has an AI of its own");
    }
    let players = world.resource::<Players>();
    for &id in &ids {
        assert!(players.get(id).is_some(), "{id:?} has its own economy");
    }
    let owners: BTreeSet<PlayerId> = world
        .query::<&Owner>()
        .iter(world)
        .map(|owner| owner.0)
        .collect();
    assert!(owners.iter().all(|owner| ids.contains(owner)), "{owners:?}");
}

/// Most players of the all-computer matches below: the six start positions the first large maps
/// were made with. (What ten computer players do to each other is measured by
/// `crates/sim/examples/m13_ten_players.rs`; docs/DECISIONS.md, "Ten players", item 11.)
const INDEPENDENT_PLAYERS: usize = 6;

/// The tick limit of an all-computer match of several players on `grid`: an hour of game time on
/// maps up to 128 x 128 cells, and proportionally longer on larger ones, whose armies have
/// farther to go.
fn several_players_tick_limit(grid: &Grid) -> u64 {
    72_000 * u64::from(grid.width() + grid.height()).max(256) / 256
}

#[test]
fn several_independent_computer_players_play_complete_matches_on_every_map() {
    let data = data();
    // The maps are independent of each other, so they are played on all cores; a failed
    // assertion of any of them fails the test when the workers are joined.
    let data = &data;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let played = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(data.skirmish.maps.len()) {
            scope.spawn(|| {
                loop {
                    let map = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(def) = data.skirmish.maps.get(map) else {
                        break;
                    };
                    several_players_play(data, map, def);
                    played.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(played.into_inner(), data.skirmish.maps.len());
}

/// An all-computer match of several players on map `map`, played twice.
fn several_players_play(data: &Data, map: usize, def: &sim::MapDef) {
    let count = def.starts.len().min(INDEPENDENT_PLAYERS);
    assert!(count >= 4, "{}: room for at least three enemies", def.id);
    let limit = several_players_tick_limit(&grid(data, map));
    let players: Vec<PlayerSetup> = (0..count)
        .map(|k| computer(k as u8, k % data.skirmish.factions.len(), 1))
        .collect();
    let run = |seed: u64| {
        let mut app = start(data, map, &players, &[], &[], seed);
        let world = app.world_mut();
        check_independent(world, count);
        // Each player starts on its own start position with its own yard.
        for (k, start) in def.starts.iter().take(count).enumerate() {
            let yard = footprints(world, PlayerId(k as u8));
            assert!(yard.iter().any(|f| f.origin == *start), "{}: {k}", def.id);
        }
        let mut credits_moved = BTreeSet::new();
        let mut outcome = None;
        for at in 0..limit {
            world.run_schedule(SimSchedule);
            if at == 3_000 {
                check_independent(world, count);
                let players = world.resource::<Players>();
                for k in 0..count {
                    let id = PlayerId(k as u8);
                    if players.get(id).is_some_and(|s| s.credits() != 5000) {
                        credits_moved.insert(id);
                    }
                }
            }
            outcome = world.resource::<MatchState>().outcome();
            if outcome.is_some() {
                break;
            }
        }
        (outcome, credits_moved, state_hash(world))
    };
    let (outcome, credits_moved, hash) = run(7);
    assert_eq!(credits_moved.len(), count, "{}: every economy runs", def.id);
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{}: {count} players: {outcome:?} after {limit} ticks",
        def.id
    );
    // Deterministic with many players too.
    assert_eq!(run(7).2, hash, "{}", def.id);
}

#[test]
fn a_defeated_human_ends_while_several_computer_players_fight_on() {
    let data = data();
    let demo = map_index(&data, "demo");
    let players = [
        human(0),
        computer(1, 1, 2),
        computer(2, 0, 2),
        computer(3, 1, 2),
    ];
    let mut app = start(&data, demo, &players, &[], &[], 11);
    let world = app.world_mut();
    // The human sells the construction yard, its only building: defeated at once.
    let yard = world
        .query::<(&SimId, &Owner, &Building)>()
        .iter(world)
        .find(|(_, owner, _)| owner.0 == HUMAN)
        .map(|(id, ..)| *id)
        .expect("yard");
    tick(world, vec![Command::SellBuilding { building: yard }]);
    tick(world, Vec::new());
    let state = world.resource::<MatchState>();
    assert!(state.defeated(HUMAN).is_some());
    assert_eq!(state.outcome(), None, "three computer players remain");
    // The others keep playing independently.
    for _ in 0..600 {
        tick(world, Vec::new());
    }
    let state = world.resource::<MatchState>();
    for k in 1..=3 {
        assert!(state.defeated(PlayerId(k)).is_none());
    }
}
