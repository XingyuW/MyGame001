//! M3 tests: players, credits, power, production queues, placement, selling, and rally points
//! (PLAN.md sections 5 and 7, M3; invariants I3, I5, I6, I7, I8).
//!
//! Every scenario runs through the production path: `SimPlugin` installs the resources and
//! `SimSchedule`, the grid and definitions come from fixture files, and every order goes through
//! `CommandQueue`. The only direct state changes are scenario setup: `Players::add` for the
//! players and their starting credits, `spawn_building` and `spawn_unit` for what stands on the
//! map at the start, and `Grid::set_terrain` for a rock.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Definitions, Footprint, Grid,
    IssuedCommand, MAX_QUEUE_LENGTH, Mobile, Orders, PlacementError, PlayerId, Players, Position,
    Producer, ProductionItem, QueueItem, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule,
    SimTick, Unit, UnitKind, load_definitions, load_map, spawn_building, spawn_unit, state_hash,
};

const HUMAN: PlayerId = PlayerId(0);
const ENEMY: PlayerId = PlayerId(1);

const ROCK: Cell = Cell {
    water: false,
    passable: false,
    buildable: false,
    ore: 0,
};

// --- Scenario helpers -----------------------------------------------------------------------

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn definitions() -> Definitions {
    load_definitions(
        fixture("data/units_economy.ron"),
        fixture("data/economy.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The economy fixture map and definitions, with the human player (`credits` starting credits)
/// owning a construction yard whose footprint covers x 19..=21, y 6..=8, next to the ore field
/// (x 22..=25, y 6..=9) and the sand column (x 18).
fn scenario(credits: i64) -> (App, SimId) {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    let grid = load_map(fixture("maps/economy_32x32.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions());
    add_player(&mut app, HUMAN, credits);
    let yard = building(&mut app, HUMAN, "yard", 19, 6);
    (app, yard)
}

fn add_player(app: &mut App, player: PlayerId, credits: i64) {
    app.world_mut()
        .resource_mut::<Players>()
        .add(player, credits)
        .unwrap_or_else(|error| panic!("{error}"));
}

fn building(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn unit(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn building_kind(app: &App, id: &str) -> BuildingKind {
    app.world()
        .resource::<Definitions>()
        .building_kind(id)
        .unwrap_or_else(|| panic!("building type {id}"))
}

fn unit_kind(app: &App, id: &str) -> UnitKind {
    app.world()
        .resource::<Definitions>()
        .unit_kind(id)
        .unwrap_or_else(|| panic!("unit type {id}"))
}

fn b(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Building(building_kind(app, id))
}

fn u(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Unit(unit_kind(app, id))
}

fn issue(app: &mut App, player: PlayerId, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand { player, command });
}

/// Runs one tick and returns its events. Checks invariant I7's first half (credits are never
/// negative) and invariant I6 for buildings after every tick.
fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    for (player, state) in app.world().resource::<Players>().iter() {
        assert!(
            state.credits() >= 0,
            "player {player:?}: {}",
            state.credits()
        );
    }
    assert_units_avoid_buildings(app);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn ticks(app: &mut App, n: u32) -> Vec<SimEvent> {
    (0..n).flat_map(|_| tick(app)).collect()
}

/// The index of the tick the next run of the schedule executes.
fn next_tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .credits()
}

fn queue(app: &App, player: PlayerId, producer: &str) -> Vec<QueueItem> {
    let producer = building_kind(app, producer);
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .queue(producer)
        .to_vec()
}

fn hash(app: &mut App) -> u64 {
    state_hash(app.world_mut())
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

fn buildings(app: &mut App) -> BTreeMap<SimId, (PlayerId, Building, Option<Producer>)> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Owner, &Building, Option<&Producer>)>();
    query
        .iter(world)
        .map(|(id, owner, building, producer)| {
            (*id, (owner.0, building.clone(), producer.copied()))
        })
        .collect()
}

fn units(app: &mut App) -> BTreeMap<SimId, (PlayerId, UnitKind, Mobile, Orders)> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Owner, &Unit, &Mobile, &Orders, &Position)>();
    query
        .iter(world)
        .map(|(id, owner, unit, mobile, orders, _)| {
            (*id, (owner.0, unit.kind, *mobile, orders.clone()))
        })
        .collect()
}

/// Invariant I6 for buildings: no unit holds a cell covered by a building, and the grid's
/// building of every cell is exactly the building whose footprint covers it.
fn assert_units_avoid_buildings(app: &mut App) {
    let buildings = buildings(app);
    let units = units(app);
    let grid = app.world().resource::<Grid>().clone();
    let mut covered: BTreeMap<CellPos, SimId> = BTreeMap::new();
    for (&id, (_, building, _)) in &buildings {
        for cell in building.footprint.cells() {
            assert!(covered.insert(cell, id).is_none(), "{cell:?} covered twice");
            assert_eq!(grid.building(cell), Some(id), "grid at {cell:?}");
            assert_eq!(grid.occupant(cell), None, "a unit holds {cell:?} of {id:?}");
        }
    }
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            let cell = CellPos::new(x, y);
            assert_eq!(grid.building(cell), covered.get(&cell).copied(), "{cell:?}");
        }
    }
    for (id, (_, _, mobile, _)) in units {
        assert!(
            !covered.contains_key(&mobile.cell),
            "{id:?} stands in a building"
        );
        if let Some(step) = mobile.step {
            assert!(!covered.contains_key(&step.to), "{id:?} enters a building");
        }
    }
}

/// Queues `item` for `player` and runs ticks until it is complete; returns the credits charged
/// on each tick (0 on ticks without progress) until then.
fn produce(app: &mut App, player: PlayerId, item: ProductionItem) -> Vec<i64> {
    issue(app, player, Command::QueueProduction { item });
    let mut charges = Vec::new();
    for _ in 0..10_000 {
        let before = credits(app, player);
        let events = tick(app);
        assert_eq!(rejections(&events), vec![]);
        charges.push(before - credits(app, player));
        if events.contains(&SimEvent::ProductionComplete { player, item }) {
            return charges;
        }
    }
    panic!("{item:?} was not completed");
}

/// Runs a control copy of a scenario alongside it: the state hashes must agree at every tick.
fn assert_same_hashes(a: &mut App, b: &mut App, n: u32) {
    for step in 0..n {
        assert_eq!(hash(a), hash(b), "tick {step}");
        tick(a);
        tick(b);
    }
    assert_eq!(hash(a), hash(b));
}

// --- Production: equation 3 and invariant I7 ------------------------------------------------

#[test]
fn a_completed_item_is_charged_exactly_its_cost() {
    // Power plant: cost 105, build time 10: 10 credits on each of the first nine ticks and
    // 10 + 5 on the last.
    let (mut app, _) = scenario(1_000);
    let power = b(&app, "power");
    let charges = produce(&mut app, HUMAN, power);
    assert_eq!(charges, [10, 10, 10, 10, 10, 10, 10, 10, 10, 15]);
    assert_eq!(credits(&app, HUMAN), 1_000 - 105);
    assert_eq!(
        queue(&app, HUMAN, "yard"),
        [QueueItem {
            item: power,
            progress: 10,
            charged: 105,
            fraction: 0,
        }]
    );
    // A complete building waits for placement and is never charged again.
    let events = ticks(&mut app, 50);
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(credits(&app, HUMAN), 1_000 - 105);

    // Boundaries of equation 3: build time 1 (the whole cost on the only tick), cost below the
    // build time (nothing until the last tick), and cost 0.
    for (item, expected) in [
        ("beacon", vec![7]),
        ("fence", vec![0, 0, 0, 0, 3]),
        ("flag", vec![0, 0]),
    ] {
        let (mut app, _) = scenario(1_000);
        let item = b(&app, item);
        assert_eq!(produce(&mut app, HUMAN, item), expected, "{item:?}");
        let total: i64 = expected.iter().sum();
        assert_eq!(credits(&app, HUMAN), 1_000 - total);
        assert_eq!(queue(&app, HUMAN, "yard")[0].charged, total);
    }
}

#[test]
fn every_item_of_the_fixture_is_charged_exactly_its_cost() {
    // Equation 3 for every producible item, through the production path: the credits charged
    // over the item's build time sum to its cost exactly (invariant I7).
    let definitions = definitions();
    let mut items: Vec<(ProductionItem, &str)> = Vec::new();
    for (index, def) in definitions.buildings().iter().enumerate() {
        items.push((
            ProductionItem::Building(BuildingKind(index as u16)),
            &def.id,
        ));
    }
    for (index, def) in definitions.units().iter().enumerate() {
        let item = ProductionItem::Unit(UnitKind(index as u16));
        if definitions.production(item).is_some() {
            items.push((item, &def.id));
        }
    }
    assert_eq!(items.len(), 8 + 2);
    for (item, name) in items {
        let terms = definitions.production(item).expect("terms").clone();
        let (mut app, _) = scenario(10_000);
        // Every producer and prerequisite of the fixture, with enough power for all of them.
        for (kind, x, y) in [
            ("power", 2, 2),
            ("power", 2, 5),
            ("refinery", 5, 1),
            ("barracks", 9, 2),
            ("factory", 13, 1),
        ] {
            building(&mut app, HUMAN, kind, x, y);
        }
        let charges = produce(&mut app, HUMAN, item);
        assert_eq!(charges.len(), terms.build_time as usize, "{name}");
        assert_eq!(charges.iter().sum::<i64>(), terms.cost, "{name}");
        let per_tick = terms.cost / i64::from(terms.build_time);
        let last = per_tick + terms.cost % i64::from(terms.build_time);
        assert!(
            charges[..charges.len() - 1].iter().all(|&c| c == per_tick),
            "{name}"
        );
        assert_eq!(charges.last(), Some(&last), "{name}");
        assert_eq!(credits(&app, HUMAN), 10_000 - terms.cost, "{name}");
    }
}

#[test]
fn progress_pauses_while_credits_are_insufficient() {
    // 100 credits: nine ticks of 10, then the last tick needs 15 and only 10 are left.
    let (mut app, _) = scenario(100);
    let power = b(&app, "power");
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    ticks(&mut app, 9);
    assert_eq!(credits(&app, HUMAN), 10);
    for _ in 0..100 {
        let events = tick(&mut app);
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(credits(&app, HUMAN), 10);
        assert_eq!(
            queue(&app, HUMAN, "yard"),
            [QueueItem {
                item: power,
                progress: 9,
                charged: 90,
                fraction: 0,
            }]
        );
    }
    // Selling a building (refund 50) resumes progress; the item is still charged exactly 105.
    let plant = building(&mut app, HUMAN, "power", 15, 2);
    issue(&mut app, HUMAN, Command::SellBuilding { building: plant });
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::ProductionComplete {
        player: HUMAN,
        item: power,
    }));
    assert_eq!(credits(&app, HUMAN), 10 + 50 - 15);
    assert_eq!(queue(&app, HUMAN, "yard")[0].charged, 105);

    // With no credits nothing progresses and nothing is charged.
    let (mut app, _) = scenario(0);
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    ticks(&mut app, 200);
    assert_eq!(credits(&app, HUMAN), 0);
    assert_eq!(queue(&app, HUMAN, "yard")[0].progress, 0);

    // Credits exactly equal to each tick's charge are enough: the item completes at 0 credits.
    let (mut app, _) = scenario(105);
    assert_eq!(produce(&mut app, HUMAN, power).iter().sum::<i64>(), 105);
    assert_eq!(credits(&app, HUMAN), 0);
}

#[test]
fn cancelling_refunds_exactly_what_was_charged() {
    let (mut app, _) = scenario(1_000);
    let power = b(&app, "power");
    let fence = b(&app, "fence");
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    issue(&mut app, HUMAN, Command::QueueProduction { item: fence });
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    ticks(&mut app, 4);
    assert_eq!(credits(&app, HUMAN), 960);
    // The most recently queued power plant is cancelled: nothing was charged for it.
    issue(&mut app, HUMAN, Command::CancelProduction { item: power });
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::ProductionCancelled {
        player: HUMAN,
        item: power,
        refund: 0,
    }));
    assert_eq!(
        credits(&app, HUMAN),
        950,
        "the head made one more tick of progress"
    );
    assert_eq!(
        queue(&app, HUMAN, "yard")
            .iter()
            .map(|q| q.item)
            .collect::<Vec<_>>(),
        [power, fence]
    );
    // Cancelling the head in progress refunds the 50 charged; the fence becomes the head.
    issue(&mut app, HUMAN, Command::CancelProduction { item: power });
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::ProductionCancelled {
        player: HUMAN,
        item: power,
        refund: 50,
    }));
    // The fence progressed in the same tick (it costs nothing before its last tick).
    assert_eq!(credits(&app, HUMAN), 1_000);
    assert_eq!(queue(&app, HUMAN, "yard")[0].item, fence);

    // A complete building that was never placed is refunded in full.
    let (mut app, _) = scenario(1_000);
    produce(&mut app, HUMAN, power);
    issue(&mut app, HUMAN, Command::CancelProduction { item: power });
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::ProductionCancelled {
        player: HUMAN,
        item: power,
        refund: 105,
    }));
    assert_eq!(credits(&app, HUMAN), 1_000);
    assert!(queue(&app, HUMAN, "yard").is_empty());

    // Cancelling an item that is not queued is rejected and changes nothing.
    let (mut a, _) = scenario(1_000);
    let (mut control, _) = scenario(1_000);
    issue(&mut a, HUMAN, Command::CancelProduction { item: power });
    assert_eq!(rejections(&tick(&mut a)), [RejectReason::NotQueued(power)]);
    tick(&mut control);
    assert_same_hashes(&mut a, &mut control, 20);
}

// --- Power ----------------------------------------------------------------------------------

/// Ticks on which the item was charged or progressed, from queueing to completion.
fn progress_ticks(app: &mut App, item: ProductionItem) -> (u64, Vec<u64>, u64) {
    let queued_at = next_tick(app);
    issue(app, HUMAN, Command::QueueProduction { item });
    let mut progressed = Vec::new();
    loop {
        let at = next_tick(app);
        let before = queue(app, HUMAN, "yard")
            .first()
            .map_or(0, |head| head.progress);
        let events = tick(app);
        let after = queue(app, HUMAN, "yard")
            .first()
            .map_or(0, |head| head.progress);
        if after > before {
            progressed.push(at);
        }
        if events
            .iter()
            .any(|event| matches!(event, SimEvent::ProductionComplete { .. }))
        {
            return (queued_at, progressed, at);
        }
        assert!(at < queued_at + 1_000, "not completed");
    }
}

#[test]
fn low_power_halves_progress() {
    let (mut app, _) = scenario(10_000);
    let power = b(&app, "power");
    // Normal power: ten consecutive ticks.
    let (queued, progressed, done) = progress_ticks(&mut app, power);
    assert_eq!(progressed, (queued..queued + 10).collect::<Vec<_>>());
    assert_eq!(done, queued + 9);
    let players = app.world().resource::<Players>();
    assert!(!players.get(HUMAN).expect("player").low_power());

    // A refinery consumes 30 and the yard produces 10: low power. Progress (and charging) only
    // on even ticks, so ten ticks of progress take 19 or 20 ticks depending on the start.
    for start_parity in [0, 1] {
        let (mut app, _) = scenario(10_000);
        building(&mut app, HUMAN, "refinery", 15, 2);
        if next_tick(&app) % 2 != start_parity {
            tick(&mut app);
        }
        let before = credits(&app, HUMAN);
        let (queued, progressed, done) = progress_ticks(&mut app, power);
        let state = app
            .world()
            .resource::<Players>()
            .get(HUMAN)
            .expect("player");
        assert_eq!((state.power_produced(), state.power_consumed()), (10, 30));
        assert!(state.low_power());
        assert_eq!(progressed.len(), 10);
        assert!(progressed.iter().all(|t| t % 2 == 0), "{progressed:?}");
        let first_even = queued + queued % 2;
        assert_eq!(progressed[0], first_even);
        assert_eq!(done, first_even + 18);
        assert_eq!(done - queued + 1, if queued % 2 == 0 { 19 } else { 20 });
        assert_eq!(
            before - credits(&app, HUMAN),
            105,
            "still charged exactly the cost"
        );
    }

    // Boundary: consumption equal to production is not low power; one more consumer is.
    for (fences, low) in [(1, false), (2, true)] {
        let (mut app, _) = scenario(10_000);
        for index in 0..fences {
            building(&mut app, HUMAN, "fence", 15 + index, 2);
        }
        let (queued, progressed, _) = progress_ticks(&mut app, power);
        let state = app
            .world()
            .resource::<Players>()
            .get(HUMAN)
            .expect("player");
        assert_eq!(state.power_consumed(), 10 * fences);
        assert_eq!(state.low_power(), low);
        let expected: Vec<u64> = if low {
            (queued..).filter(|t| t % 2 == 0).take(10).collect()
        } else {
            (queued..queued + 10).collect()
        };
        assert_eq!(progressed, expected, "{fences} fences");
    }
}

#[test]
fn power_follows_the_buildings_placed_and_sold() {
    let (mut app, yard) = scenario(1_000);
    tick(&mut app);
    let state = |app: &App| {
        let state = app
            .world()
            .resource::<Players>()
            .get(HUMAN)
            .expect("player")
            .clone();
        (state.power_produced(), state.power_consumed())
    };
    assert_eq!(state(&app), (10, 0));
    let power = b(&app, "power");
    produce(&mut app, HUMAN, power);
    assert_eq!(rejections(&place(&mut app, "power", 16, 6)), []);
    assert_eq!(state(&app), (110, 0));
    issue(&mut app, HUMAN, Command::SellBuilding { building: yard });
    tick(&mut app);
    assert_eq!(state(&app), (100, 0));
}

// --- Queue validation -----------------------------------------------------------------------

#[test]
fn invalid_queue_commands_are_rejected_without_changing_state() {
    let (mut app, _) = scenario(1_000);
    let walker = u(&app, "walker");
    let refinery = b(&app, "refinery");
    let cases: Vec<(PlayerId, Command, RejectReason)> = vec![
        (
            HUMAN,
            Command::QueueProduction {
                item: u(&app, "runner"),
            },
            RejectReason::UnknownItem(u(&app, "runner")),
        ),
        (
            HUMAN,
            Command::QueueProduction {
                item: ProductionItem::Unit(UnitKind(99)),
            },
            RejectReason::UnknownItem(ProductionItem::Unit(UnitKind(99))),
        ),
        (
            HUMAN,
            Command::QueueProduction {
                item: ProductionItem::Building(BuildingKind(u16::MAX)),
            },
            RejectReason::UnknownItem(ProductionItem::Building(BuildingKind(u16::MAX))),
        ),
        (
            HUMAN,
            Command::QueueProduction { item: refinery },
            RejectReason::MissingPrerequisite {
                item: refinery,
                prerequisite: building_kind(&app, "power"),
            },
        ),
        (
            HUMAN,
            Command::QueueProduction { item: walker },
            RejectReason::NoProducer {
                item: walker,
                producer: building_kind(&app, "barracks"),
            },
        ),
        (
            ENEMY,
            Command::QueueProduction {
                item: b(&app, "power"),
            },
            RejectReason::UnknownPlayer(ENEMY),
        ),
        (
            ENEMY,
            Command::SellBuilding { building: SimId(0) },
            RejectReason::UnknownPlayer(ENEMY),
        ),
    ];
    for (player, command, reason) in cases {
        let (mut a, _) = scenario(1_000);
        let (mut control, _) = scenario(1_000);
        issue(&mut a, player, command.clone());
        assert_eq!(rejections(&tick(&mut a)), [reason], "{command:?}");
        tick(&mut control);
        assert_same_hashes(&mut a, &mut control, 10);
    }

    // A queue holds at most MAX_QUEUE_LENGTH items.
    let fence = b(&app, "fence");
    for _ in 0..MAX_QUEUE_LENGTH {
        issue(&mut app, HUMAN, Command::QueueProduction { item: fence });
    }
    assert_eq!(rejections(&tick(&mut app)), []);
    issue(&mut app, HUMAN, Command::QueueProduction { item: fence });
    assert_eq!(
        rejections(&tick(&mut app)),
        [RejectReason::QueueFull {
            producer: building_kind(&app, "yard"),
            max: MAX_QUEUE_LENGTH,
        }]
    );
    assert_eq!(queue(&app, HUMAN, "yard").len(), MAX_QUEUE_LENGTH);
}

#[test]
fn production_needs_a_producer_and_continues_with_another() {
    // Selling the only yard pauses the queue (no charge); a new yard resumes it.
    let (mut app, yard) = scenario(1_000);
    let power = b(&app, "power");
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    ticks(&mut app, 3);
    issue(&mut app, HUMAN, Command::SellBuilding { building: yard });
    tick(&mut app);
    let paused = credits(&app, HUMAN);
    assert_eq!(paused, 1_000 - 30 + 500);
    ticks(&mut app, 50);
    assert_eq!(credits(&app, HUMAN), paused);
    assert_eq!(queue(&app, HUMAN, "yard")[0].progress, 3);
    building(&mut app, HUMAN, "yard", 3, 3);
    ticks(&mut app, 7);
    assert_eq!(queue(&app, HUMAN, "yard")[0].charged, 105);
}

// --- Placement ------------------------------------------------------------------------------

/// A scenario in which the human's power plant is complete and waits for placement.
fn power_ready() -> (App, SimId) {
    let (mut app, yard) = scenario(1_000);
    let power = b(&app, "power");
    produce(&mut app, HUMAN, power);
    (app, yard)
}

fn place(app: &mut App, kind: &str, x: i32, y: i32) -> Vec<SimEvent> {
    let building = building_kind(app, kind);
    issue(
        app,
        HUMAN,
        Command::PlaceBuilding {
            building,
            origin: CellPos::new(x, y),
        },
    );
    tick(app)
}

#[test]
fn placement_rejections_change_no_state() {
    let footprint = |x, y| Footprint {
        origin: CellPos::new(x, y),
        width: 2,
        height: 2,
    };
    let cases: Vec<((i32, i32), PlacementError)> = vec![
        // Off the map: partly beyond the right and bottom edges, before the left and top
        // edges, and far away.
        ((31, 10), PlacementError::OutsideMap(footprint(31, 10))),
        ((10, 31), PlacementError::OutsideMap(footprint(10, 31))),
        ((-1, 5), PlacementError::OutsideMap(footprint(-1, 5))),
        ((5, -1), PlacementError::OutsideMap(footprint(5, -1))),
        (
            (i32::MAX, i32::MAX),
            PlacementError::OutsideMap(footprint(i32::MAX, i32::MAX)),
        ),
        (
            (i32::MIN, 0),
            PlacementError::OutsideMap(footprint(i32::MIN, 0)),
        ),
        // Terrain that is not buildable: sand (x 18), ore (x 22..=25, y 6..=9), and a rock.
        ((17, 6), PlacementError::NotBuildable(CellPos::new(18, 6))),
        ((22, 9), PlacementError::NotBuildable(CellPos::new(22, 9))),
        ((20, 2), PlacementError::NotBuildable(CellPos::new(21, 3))),
        // Overlap with the yard (x 19..=21, y 6..=8).
        (
            (20, 8),
            PlacementError::Overlap {
                cell: CellPos::new(20, 8),
                building: SimId(0),
            },
        ),
        // Too far: four cells from the yard; the player owns no other building.
        (
            (25, 12),
            PlacementError::TooFar {
                distance: Some(4),
                max: 3,
            },
        ),
        (
            (14, 2),
            PlacementError::TooFar {
                distance: Some(4),
                max: 3,
            },
        ),
    ];
    for ((x, y), error) in cases {
        let (mut a, _) = power_ready();
        let (mut control, _) = power_ready();
        a.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(21, 3), ROCK);
        control
            .world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(21, 3), ROCK);
        let events = place(&mut a, "power", x, y);
        assert_eq!(
            rejections(&events),
            [RejectReason::Placement(error.clone())],
            "({x}, {y})"
        );
        tick(&mut control);
        assert_same_hashes(&mut a, &mut control, 10);
    }
}

#[test]
fn placement_is_rejected_on_cells_held_by_units() {
    // A unit standing in a cell of the footprint.
    let (mut app, _) = power_ready();
    let walker = unit(&mut app, HUMAN, "walker", 23, 4);
    let events = place(&mut app, "power", 22, 3);
    assert_eq!(
        rejections(&events),
        [RejectReason::Placement(PlacementError::Occupied {
            cell: CellPos::new(23, 4),
            unit: walker,
        })]
    );
    // A unit entering a cell of the footprint has reserved it.
    let (mut app, _) = power_ready();
    let walker = unit(&mut app, HUMAN, "walker", 25, 3);
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![walker],
            target: CellPos::new(20, 3),
        },
    );
    let entering = loop {
        tick(&mut app);
        let (_, _, mobile, _) = units(&mut app)[&walker];
        if let Some(step) = mobile.step
            && step.progress > 0
        {
            break step.to;
        }
    };
    assert_eq!(entering, CellPos::new(24, 3));
    let events = place(&mut app, "power", 23, 2);
    assert_eq!(
        rejections(&events),
        [RejectReason::Placement(PlacementError::Occupied {
            cell: CellPos::new(24, 3),
            unit: walker,
        })]
    );
}

#[test]
fn placement_within_the_distance_is_accepted_and_covers_the_cells() {
    // Exactly three cells from the yard (x 19..=21, y 6..=8), diagonally: accepted.
    let (mut app, yard) = power_ready();
    let events = place(&mut app, "power", 24, 11);
    assert_eq!(rejections(&events), []);
    let placed = events
        .iter()
        .find_map(|event| match event {
            SimEvent::BuildingPlaced {
                player,
                building,
                kind,
            } => Some((*player, *building, *kind)),
            _ => None,
        })
        .expect("placed");
    assert_eq!(placed.0, HUMAN);
    assert_eq!(placed.2, building_kind(&app, "power"));
    assert!(queue(&app, HUMAN, "yard").is_empty());
    let all = buildings(&mut app);
    assert_eq!(all.len(), 2);
    let (owner, building, producer) = all[&placed.1].clone();
    assert_eq!(owner, HUMAN);
    assert_eq!(producer, None, "a power plant produces no units");
    assert_eq!(
        building.footprint,
        Footprint {
            origin: CellPos::new(24, 11),
            width: 2,
            height: 2,
        }
    );
    let grid = app.world().resource::<Grid>().clone();
    for cell in building.footprint.cells() {
        assert_eq!(grid.building(cell), Some(placed.1));
        assert!(!grid.is_passable(cell));
    }
    assert_eq!(all[&yard].2, None);
    // Units route around the new building.
    let walker = unit(&mut app, HUMAN, "walker", 23, 12);
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![walker],
            target: CellPos::new(27, 12),
        },
    );
    let events = ticks(&mut app, 200);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: walker,
        retargeted: false,
    }));

    // A second building may lie within the distance of the first placed one.
    let power = b(&app, "power");
    produce(&mut app, HUMAN, power);
    let events = place(&mut app, "power", 27, 14);
    assert_eq!(rejections(&events), [], "two cells from the placed plant");
}

#[test]
fn placement_needs_a_ready_building_and_an_owned_building_nearby() {
    let (mut app, yard) = scenario(1_000);
    let power = building_kind(&app, "power");
    let barracks = building_kind(&app, "barracks");
    assert_eq!(
        rejections(&place(&mut app, "power", 16, 6)),
        [RejectReason::NotReady(power)]
    );
    // In progress is not ready.
    issue(
        &mut app,
        HUMAN,
        Command::QueueProduction {
            item: ProductionItem::Building(power),
        },
    );
    tick(&mut app);
    assert_eq!(
        rejections(&place(&mut app, "power", 16, 6)),
        [RejectReason::NotReady(power)]
    );
    ticks(&mut app, 10);
    // Only the type that is ready.
    assert_eq!(
        rejections(&place(&mut app, "barracks", 16, 6)),
        [RejectReason::NotReady(barracks)]
    );
    assert_eq!(
        rejections(&place_kind(&mut app, BuildingKind(500), 16, 6)),
        [RejectReason::UnknownItem(ProductionItem::Building(
            BuildingKind(500)
        ))]
    );
    // Without any building of its own the player cannot place anything.
    issue(&mut app, HUMAN, Command::SellBuilding { building: yard });
    tick(&mut app);
    assert_eq!(
        rejections(&place(&mut app, "power", 16, 6)),
        [RejectReason::Placement(PlacementError::TooFar {
            distance: None,
            max: 3,
        })]
    );
    // A building of another player does not count.
    add_player(&mut app, ENEMY, 0);
    building(&mut app, ENEMY, "yard", 13, 6);
    assert_eq!(
        rejections(&place(&mut app, "power", 16, 6)),
        [RejectReason::Placement(PlacementError::TooFar {
            distance: None,
            max: 3,
        })]
    );
}

fn place_kind(app: &mut App, building: BuildingKind, x: i32, y: i32) -> Vec<SimEvent> {
    issue(
        app,
        HUMAN,
        Command::PlaceBuilding {
            building,
            origin: CellPos::new(x, y),
        },
    );
    tick(app)
}

#[test]
fn later_commands_of_a_tick_see_the_buildings_sold_before_them() {
    let (mut app, _) = scenario(1_000);
    let power = b(&app, "power");
    let beacon = b(&app, "beacon");
    produce(&mut app, HUMAN, power);
    let power_kind = building_kind(&app, "power");
    issue(
        &mut app,
        HUMAN,
        Command::PlaceBuilding {
            building: power_kind,
            origin: CellPos::new(16, 6),
        },
    );
    issue(&mut app, HUMAN, Command::QueueProduction { item: beacon });
    let events = tick(&mut app);
    assert_eq!(rejections(&events), []);
    // The beacon completed in the same tick (build time 1).
    assert!(events.contains(&SimEvent::ProductionComplete {
        player: HUMAN,
        item: beacon,
    }));
    let beacon_kind = building_kind(&app, "beacon");
    let plant = buildings(&mut app)
        .into_iter()
        .find(|(_, (_, building, _))| building.kind == power_kind)
        .map(|(id, _)| id)
        .expect("plant");
    // On the plant's cells the beacon overlaps it; after the sale, in the same tick, the cell
    // is free; a second sale of the plant finds nothing.
    let place_beacon = Command::PlaceBuilding {
        building: beacon_kind,
        origin: CellPos::new(17, 7),
    };
    issue(&mut app, HUMAN, place_beacon.clone());
    issue(&mut app, HUMAN, Command::SellBuilding { building: plant });
    issue(&mut app, HUMAN, place_beacon);
    issue(&mut app, HUMAN, Command::SellBuilding { building: plant });
    let events = tick(&mut app);
    assert_eq!(
        rejections(&events),
        [
            RejectReason::Placement(PlacementError::Overlap {
                cell: CellPos::new(17, 7),
                building: plant,
            }),
            RejectReason::UnknownBuilding(plant),
        ]
    );
    assert!(events.contains(&SimEvent::BuildingSold {
        player: HUMAN,
        building: plant,
        refund: 50,
    }));
    let all = buildings(&mut app);
    assert!(!all.contains_key(&plant));
    assert!(
        all.values()
            .any(|(_, building, _)| building.kind == beacon_kind
                && building.footprint.origin == CellPos::new(17, 7))
    );
}

#[test]
fn later_commands_of_a_tick_see_the_buildings_placed_before_them() {
    // Two players with ready power plants: the second placement overlaps the first, placed
    // earlier in the same tick, before its entity exists.
    let (mut app, _) = scenario(1_000);
    add_player(&mut app, ENEMY, 1_000);
    building(&mut app, ENEMY, "yard", 13, 9);
    let power = b(&app, "power");
    issue(&mut app, HUMAN, Command::QueueProduction { item: power });
    issue(&mut app, ENEMY, Command::QueueProduction { item: power });
    ticks(&mut app, 10);
    let power_kind = building_kind(&app, "power");
    let next = SimId(app.world().resource::<sim::SimIdAllocator>().peek_next());
    issue(
        &mut app,
        HUMAN,
        Command::PlaceBuilding {
            building: power_kind,
            origin: CellPos::new(16, 7),
        },
    );
    issue(
        &mut app,
        ENEMY,
        Command::PlaceBuilding {
            building: power_kind,
            origin: CellPos::new(17, 8),
        },
    );
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::BuildingPlaced {
        player: HUMAN,
        building: next,
        kind: power_kind,
    }));
    assert_eq!(
        rejections(&events),
        [RejectReason::Placement(PlacementError::Overlap {
            cell: CellPos::new(17, 8),
            building: next,
        })]
    );

    // A rally point set in the tick of a barracks' placement applies to it.
    let (mut app, _) = scenario(1_000);
    building(&mut app, HUMAN, "power", 15, 2);
    let barracks = b(&app, "barracks");
    produce(&mut app, HUMAN, barracks);
    let next = SimId(app.world().resource::<sim::SimIdAllocator>().peek_next());
    let barracks_kind = building_kind(&app, "barracks");
    issue(
        &mut app,
        HUMAN,
        Command::PlaceBuilding {
            building: barracks_kind,
            origin: CellPos::new(16, 6),
        },
    );
    issue(
        &mut app,
        HUMAN,
        Command::SetRallyPoint {
            building: next,
            cell: CellPos::new(3, 3),
        },
    );
    assert_eq!(rejections(&tick(&mut app)), []);
    assert_eq!(
        buildings(&mut app)[&next].2,
        Some(Producer {
            rally: Some(CellPos::new(3, 3))
        })
    );
}

// --- Selling --------------------------------------------------------------------------------

#[test]
fn selling_refunds_the_sell_value_and_frees_the_cells() {
    let (mut app, yard) = scenario(1_000);
    let barracks = building(&mut app, HUMAN, "barracks", 10, 10);
    add_player(&mut app, ENEMY, 0);
    let foreign = building(&mut app, ENEMY, "power", 10, 20);
    for (command, reason) in [
        (
            Command::SellBuilding { building: foreign },
            RejectReason::NotOwned(foreign),
        ),
        (
            Command::SellBuilding {
                building: SimId(999),
            },
            RejectReason::UnknownBuilding(SimId(999)),
        ),
    ] {
        issue(&mut app, HUMAN, command);
        assert_eq!(rejections(&tick(&mut app)), [reason]);
    }
    // A unit is not a building.
    let walker = unit(&mut app, HUMAN, "walker", 2, 2);
    issue(&mut app, HUMAN, Command::SellBuilding { building: walker });
    assert_eq!(
        rejections(&tick(&mut app)),
        [RejectReason::UnknownBuilding(walker)]
    );

    issue(
        &mut app,
        HUMAN,
        Command::SellBuilding { building: barracks },
    );
    let events = tick(&mut app);
    assert_eq!(
        events,
        [SimEvent::BuildingSold {
            player: HUMAN,
            building: barracks,
            refund: 75,
        }]
    );
    assert_eq!(credits(&app, HUMAN), 1_075);
    assert!(!buildings(&mut app).contains_key(&barracks));
    let grid = app.world().resource::<Grid>().clone();
    for y in 10..12 {
        for x in 10..12 {
            assert_eq!(grid.building(CellPos::new(x, y)), None);
            assert!(grid.is_passable(CellPos::new(x, y)));
        }
    }
    // A unit can walk through where it stood.
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![walker],
            target: CellPos::new(11, 11),
        },
    );
    let events = ticks(&mut app, 300);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: walker,
        retargeted: false,
    }));
    issue(&mut app, HUMAN, Command::SellBuilding { building: yard });
    tick(&mut app);
    assert_eq!(credits(&app, HUMAN), 1_575);
    assert!(buildings(&mut app).keys().all(|&id| id == foreign));
}

// --- Units leaving, rally points ------------------------------------------------------------

/// A scenario with a power plant and barracks (x 10..=11, y 10..=11), and `credits`.
fn barracks_scenario(credits: i64) -> (App, SimId) {
    let (mut app, _) = scenario(credits);
    building(&mut app, HUMAN, "power", 15, 2);
    let barracks = building(&mut app, HUMAN, "barracks", 10, 10);
    (app, barracks)
}

#[test]
fn a_produced_unit_leaves_its_building_and_moves_to_the_rally_point() {
    let (mut app, barracks) = barracks_scenario(1_000);
    let walker = u(&app, "walker");
    // Without a rally point the unit stands at the ring cell lowest on screen.
    produce(&mut app, HUMAN, walker);
    let spawned: Vec<_> = units(&mut app).into_iter().collect();
    assert_eq!(spawned.len(), 1);
    let (id, (owner, kind, mobile, orders)) = spawned[0].clone();
    assert_eq!((owner, kind), (HUMAN, unit_kind(&app, "walker")));
    assert_eq!(mobile.cell, CellPos::new(12, 12));
    assert_eq!(orders.current, None);
    assert!(queue(&app, HUMAN, "barracks").is_empty());
    assert_eq!(credits(&app, HUMAN), 950);

    // With a rally point it leaves at the ring cell nearest to it and walks there.
    issue(
        &mut app,
        HUMAN,
        Command::SetRallyPoint {
            building: barracks,
            cell: CellPos::new(5, 20),
        },
    );
    tick(&mut app);
    assert_eq!(
        buildings(&mut app)[&barracks].2,
        Some(Producer {
            rally: Some(CellPos::new(5, 20))
        })
    );
    issue(&mut app, HUMAN, Command::QueueProduction { item: walker });
    let mut spawned = None;
    let mut arrived = false;
    for _ in 0..400 {
        for event in tick(&mut app) {
            match event {
                SimEvent::UnitSpawned {
                    player,
                    unit,
                    building,
                } => {
                    assert_eq!((player, building), (HUMAN, barracks));
                    assert_ne!(unit, id);
                    assert_eq!(units(&mut app)[&unit].2.cell, CellPos::new(9, 12));
                    spawned = Some(unit);
                }
                SimEvent::MoveCompleted { unit, retargeted } => {
                    assert_eq!((Some(unit), retargeted), (spawned, false));
                    arrived = true;
                }
                _ => {}
            }
        }
        if arrived {
            break;
        }
    }
    assert!(arrived);
    let new = spawned.expect("spawned");
    assert_eq!(units(&mut app)[&new].2.cell, CellPos::new(5, 20));
}

#[test]
fn rally_point_commands_are_validated() {
    let setup = || {
        let (mut app, barracks) = barracks_scenario(1_000);
        add_player(&mut app, ENEMY, 0);
        let foreign = building(&mut app, ENEMY, "barracks", 2, 20);
        (app, barracks, foreign)
    };
    let (_, barracks, foreign) = setup();
    let yard = SimId(0);
    let cases = [
        (
            barracks,
            CellPos::new(32, 0),
            RejectReason::OutsideMap(CellPos::new(32, 0)),
        ),
        (
            barracks,
            CellPos::new(0, -1),
            RejectReason::OutsideMap(CellPos::new(0, -1)),
        ),
        (yard, CellPos::new(5, 5), RejectReason::NotAProducer(yard)),
        (foreign, CellPos::new(5, 5), RejectReason::NotOwned(foreign)),
        (
            SimId(77),
            CellPos::new(5, 5),
            RejectReason::UnknownBuilding(SimId(77)),
        ),
    ];
    for (building, cell, reason) in cases {
        let (mut app, ..) = setup();
        let (mut control, ..) = setup();
        issue(&mut app, HUMAN, Command::SetRallyPoint { building, cell });
        assert_eq!(rejections(&tick(&mut app)), [reason]);
        tick(&mut control);
        assert_same_hashes(&mut app, &mut control, 5);
        assert_eq!(
            buildings(&mut app)[&barracks].2,
            Some(Producer { rally: None })
        );
    }
}

/// The barracks of `barracks_scenario` covers x 10..=11, y 10..=11; every one of the 12 cells
/// of its ring is held by a unit.
fn crowd_the_barracks(app: &mut App) -> Vec<SimId> {
    let ring = Footprint {
        origin: CellPos::new(10, 10),
        width: 2,
        height: 2,
    };
    let blockers: Vec<SimId> = ring
        .ring()
        .map(|cell| unit(app, HUMAN, "runner", cell.x, cell.y))
        .collect();
    assert_eq!(blockers.len(), 12);
    blockers
}

#[test]
fn a_unit_leaves_beyond_a_ring_whose_every_cell_is_held() {
    let (mut app, barracks) = barracks_scenario(1_000);
    crowd_the_barracks(&mut app);
    let footprint = buildings(&mut app)[&barracks].1.footprint;
    let walker = u(&app, "walker");
    let spawned_by = |events: &[SimEvent]| -> Vec<SimId> {
        events
            .iter()
            .filter_map(|event| match event {
                SimEvent::UnitSpawned { building, unit, .. } if *building == barracks => {
                    Some(*unit)
                }
                _ => None,
            })
            .collect()
    };
    for _ in 0..3 {
        issue(&mut app, HUMAN, Command::QueueProduction { item: walker });
    }
    let spawned = spawned_by(&ticks(&mut app, 60));
    assert_eq!(
        spawned.len(),
        3,
        "a crowd around it does not stop production"
    );
    assert!(queue(&app, HUMAN, "barracks").is_empty());
    // They stand in the second ring, lowest on screen first.
    let all = units(&mut app);
    assert_eq!(all[&spawned[0]].2.cell, CellPos::new(13, 13));
    for id in &spawned {
        assert_eq!(footprint.distance_to_cell(all[id].2.cell), 2);
    }
    // With a rally point, the next unit comes out on the side of the rally point.
    issue(
        &mut app,
        HUMAN,
        Command::SetRallyPoint {
            building: barracks,
            cell: CellPos::new(2, 10),
        },
    );
    issue(&mut app, HUMAN, Command::QueueProduction { item: walker });
    let spawned = spawned_by(&ticks(&mut app, 3 + 7));
    assert_eq!(spawned.len(), 1);
    let unit = &units(&mut app)[&spawned[0]];
    assert_eq!(
        unit.2.cell,
        CellPos::new(8, 10),
        "the west side of ring two"
    );
    assert!(unit.3.current.is_some(), "and walks on to the rally point");
}

#[test]
fn a_unit_waits_inside_while_no_cell_around_can_be_reached() {
    let (mut app, barracks) = barracks_scenario(1_000);
    crowd_the_barracks(&mut app);
    // Rock all around the crowded ring: the cells at distance 2.
    let rock = Cell {
        passable: false,
        water: false,
        buildable: false,
        ore: 0,
    };
    let sealed: Vec<CellPos> = (8..=13)
        .flat_map(|y| (8..=13).map(move |x| CellPos::new(x, y)))
        .filter(|cell| [8, 13].contains(&cell.x) || [8, 13].contains(&cell.y))
        .collect();
    assert_eq!(sealed.len(), 20);
    let open = *app
        .world()
        .resource::<Grid>()
        .cell(sealed[0])
        .expect("inside the map");
    for &cell in &sealed {
        assert!(
            app.world_mut()
                .resource_mut::<Grid>()
                .set_terrain(cell, rock)
        );
    }
    let walker = u(&app, "walker");
    produce(&mut app, HUMAN, walker);
    let events = ticks(&mut app, 50);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitSpawned { .. }))
    );
    assert_eq!(queue(&app, HUMAN, "barracks")[0].progress, 7);
    assert_eq!(credits(&app, HUMAN), 950, "charged once");
    // A gap opens in the rock; the walker leaves through it.
    assert!(
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(13, 13), open)
    );
    let events = ticks(&mut app, 5);
    let spawned = events
        .iter()
        .filter(|event| {
            matches!(event, SimEvent::UnitSpawned { building, .. } if *building == barracks)
        })
        .count();
    assert_eq!(spawned, 1);
    assert!(queue(&app, HUMAN, "barracks").is_empty());
    let all = units(&mut app);
    assert!(all.values().any(|unit| unit.2.cell == CellPos::new(13, 13)));
}

#[test]
fn a_placed_refinery_delivers_its_free_harvester() {
    let (mut app, _) = scenario(1_000);
    building(&mut app, HUMAN, "power", 15, 2);
    let refinery = b(&app, "refinery");
    produce(&mut app, HUMAN, refinery);
    let events = place(&mut app, "refinery", 15, 9);
    assert_eq!(rejections(&events), []);
    let placed = events
        .iter()
        .find_map(|event| match event {
            SimEvent::BuildingPlaced { building, .. } => Some(*building),
            _ => None,
        })
        .expect("placed");
    // The free harvester leaves in the tick of the placement (the building exists from the
    // `Economy` stage on).
    let harvester = events
        .iter()
        .find_map(|event| match event {
            SimEvent::UnitSpawned {
                player,
                unit,
                building,
            } => {
                assert_eq!((*player, *building), (HUMAN, placed));
                Some(*unit)
            }
            _ => None,
        })
        .expect("the free harvester left the refinery");
    let all = units(&mut app);
    assert_eq!(all[&harvester].1, unit_kind(&app, "harvester"));
    assert!(buildings(&mut app)[&placed].1.pending_units.is_empty());
    // A building spawned as scenario setup delivers no free unit.
    let (mut app, _) = scenario(1_000);
    building(&mut app, HUMAN, "refinery", 15, 9);
    ticks(&mut app, 5);
    assert!(units(&mut app).is_empty());
}

// --- Determinism ----------------------------------------------------------------------------

fn scripted_run() -> Vec<u64> {
    let (mut app, _) = scenario(2_000);
    let script: Vec<(u64, Command)> = vec![
        (
            0,
            Command::QueueProduction {
                item: b(&app, "power"),
            },
        ),
        (
            12,
            Command::PlaceBuilding {
                building: building_kind(&app, "power"),
                origin: CellPos::new(16, 6),
            },
        ),
        (
            13,
            Command::QueueProduction {
                item: b(&app, "barracks"),
            },
        ),
        (
            14,
            Command::QueueProduction {
                item: b(&app, "refinery"),
            },
        ),
        (
            40,
            Command::PlaceBuilding {
                building: building_kind(&app, "barracks"),
                origin: CellPos::new(16, 2),
            },
        ),
        (
            41,
            Command::QueueProduction {
                item: u(&app, "walker"),
            },
        ),
        (
            42,
            Command::QueueProduction {
                item: u(&app, "walker"),
            },
        ),
        (
            70,
            Command::PlaceBuilding {
                building: building_kind(&app, "refinery"),
                origin: CellPos::new(19, 10),
            },
        ),
        (
            50,
            Command::CancelProduction {
                item: u(&app, "walker"),
            },
        ),
    ];
    let mut trace = Vec::new();
    for at in 0..600 {
        for (when, command) in &script {
            if *when == at {
                issue(&mut app, HUMAN, command.clone());
            }
        }
        let events = tick(&mut app);
        assert_eq!(rejections(&events), [], "tick {at}");
        trace.push(hash(&mut app));
    }
    trace
}

#[test]
fn the_same_economy_scenario_gives_the_same_hash_at_every_tick() {
    let first = scripted_run();
    let second = scripted_run();
    assert_eq!(first, second);
    let mut distinct = first.clone();
    distinct.dedup();
    assert_eq!(distinct.len(), first.len(), "the hash changes every tick");
}

#[test]
fn the_hash_covers_the_economy_state() {
    // Two worlds that differ only in one piece of economy state hash differently.
    type Variant = Box<dyn Fn(&mut App)>;
    let variants: Vec<Variant> = vec![
        Box::new(|_| {}),
        Box::new(|app| {
            let item = b(app, "power");
            issue(app, HUMAN, Command::QueueProduction { item });
        }),
        Box::new(|app| {
            let item = b(app, "fence");
            issue(app, HUMAN, Command::QueueProduction { item });
        }),
        Box::new(|app| {
            building(app, HUMAN, "barracks", 10, 10);
        }),
        Box::new(|app| {
            let barracks = building(app, HUMAN, "barracks", 10, 10);
            issue(
                app,
                HUMAN,
                Command::SetRallyPoint {
                    building: barracks,
                    cell: CellPos::new(1, 1),
                },
            );
        }),
        Box::new(|app| {
            add_player(app, ENEMY, 0);
        }),
    ];
    let mut hashes = Vec::new();
    for variant in &variants {
        let (mut app, _) = scenario(1_000);
        variant(&mut app);
        tick(&mut app);
        hashes.push(hash(&mut app));
    }
    let mut distinct = hashes.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), hashes.len(), "{hashes:?}");
    // Credits alone.
    let (mut a, _) = scenario(1_000);
    let (mut b, _) = scenario(1_001);
    assert_ne!(hash(&mut a), hash(&mut b));
}
