//! M3 tests: ore, the harvester cycle, refineries, and the end-to-end scenario in which a base is
//! built and funded from ore through commands alone (PLAN.md sections 5 and 7, M3, equation 4;
//! invariants I3, I6, I7, I8).
//!
//! As in `m3_economy.rs`, every scenario runs through `SimPlugin` and `CommandQueue` with fixture
//! data; `Players::add`, `spawn_building`, `spawn_unit`, and `Grid::set_terrain` (to set the ore
//! of a cell) are scenario setup only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, Cell, CellPos, Command, CommandQueue, Definitions, Grid, HarvestIdle, HarvestState,
    Harvester, IssuedCommand, MAX_HARVEST_ATTEMPTS, Mobile, Orders, PlayerId, Players,
    ProductionItem, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, Unit, load_definitions,
    load_map, spawn_building, spawn_unit, state_hash,
};

const HUMAN: PlayerId = PlayerId(0);
const ENEMY: PlayerId = PlayerId(1);

/// The ore field of the fixture map: x 22..=25, y 6..=9, 25 ore units per cell.
const ORE_FIELD: [(i32, i32); 2] = [(22, 6), (25, 9)];

/// The fixture harvester: capacity 20, harvest rate 3; the fixture refinery pays 2 per unit.
const CAPACITY: u32 = 20;
const RATE: u32 = 3;
const CREDITS_PER_ORE: i64 = 2;

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

/// The economy fixture map and definitions, with the human player and `credits`.
fn scenario(credits: i64) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    let grid = load_map(fixture("maps/economy_32x32.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions());
    app.world_mut()
        .resource_mut::<Players>()
        .add(HUMAN, credits)
        .unwrap_or_else(|error| panic!("{error}"));
    app
}

fn building(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn unit(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn issue(app: &mut App, player: PlayerId, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand { player, command });
}

/// Runs one tick, checks invariants I6 and I7 (credits never negative), and returns the tick's
/// events.
fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    for (player, state) in app.world().resource::<Players>().iter() {
        assert!(
            state.credits() >= 0,
            "player {player:?}: {}",
            state.credits()
        );
    }
    assert_i6(app);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn ticks(app: &mut App, n: u32) -> Vec<SimEvent> {
    (0..n).flat_map(|_| tick(app)).collect()
}

fn credits(app: &App) -> i64 {
    app.world()
        .resource::<Players>()
        .get(HUMAN)
        .expect("player")
        .credits()
}

fn ore(app: &App, x: i32, y: i32) -> u32 {
    app.world()
        .resource::<Grid>()
        .cell(CellPos::new(x, y))
        .expect("inside")
        .ore
}

fn total_ore(app: &App) -> u64 {
    let grid = app.world().resource::<Grid>();
    (0..grid.height() as i32)
        .flat_map(|y| (0..grid.width() as i32).map(move |x| CellPos::new(x, y)))
        .map(|cell| u64::from(grid.cell(cell).expect("inside").ore))
        .sum()
}

/// Sets the ore of a cell (scenario setup); the cell keeps its other terrain.
fn set_ore(app: &mut App, x: i32, y: i32, amount: u32) {
    let mut grid = app.world_mut().resource_mut::<Grid>();
    let cell = CellPos::new(x, y);
    let terrain = *grid.cell(cell).expect("inside");
    grid.set_terrain(
        cell,
        Cell {
            ore: amount,
            ..terrain
        },
    );
}

/// Empties the ore field except for the listed cells.
fn clear_ore_field_except(app: &mut App, keep: &[(i32, i32)]) {
    for y in ORE_FIELD[0].1..=ORE_FIELD[1].1 {
        for x in ORE_FIELD[0].0..=ORE_FIELD[1].0 {
            if !keep.contains(&(x, y)) {
                set_ore(app, x, y, 0);
            }
        }
    }
}

fn harvester(app: &mut App, id: SimId) -> (Harvester, Mobile, Orders) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Harvester, &Mobile, &Orders)>();
    query
        .iter(world)
        .find(|(other, ..)| **other == id)
        .map(|(_, harvester, mobile, orders)| (*harvester, *mobile, orders.clone()))
        .expect("harvester")
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

fn delivered(events: &[SimEvent]) -> Vec<(SimId, SimId, u32, i64)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::OreDelivered {
                harvester,
                refinery,
                ore,
                credits,
                ..
            } => Some((*harvester, *refinery, *ore, *credits)),
            _ => None,
        })
        .collect()
}

/// Invariant I6: no cell is held by two units, the grid's occupants are exactly the cells the
/// units hold, and no unit holds a cell covered by a building.
fn assert_i6(app: &mut App) {
    let grid = app.world().resource::<Grid>().clone();
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Unit, &Mobile)>();
    let mut held: BTreeMap<CellPos, SimId> = BTreeMap::new();
    for (id, _, mobile) in query.iter(world) {
        let cells = std::iter::once(mobile.cell).chain(mobile.step.map(|step| step.to));
        for cell in cells {
            assert!(held.insert(cell, *id).is_none(), "{cell:?} held twice");
            assert_eq!(grid.occupant(cell), Some(*id), "grid at {cell:?}");
            assert_eq!(
                grid.building(cell),
                None,
                "{id:?} in a building at {cell:?}"
            );
        }
    }
    let mut buildings = world.query::<&Building>();
    let covered: BTreeSet<CellPos> = buildings
        .iter(world)
        .flat_map(|building| building.footprint.cells())
        .collect();
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            let cell = CellPos::new(x, y);
            assert_eq!(grid.occupant(cell), held.get(&cell).copied(), "{cell:?}");
            assert_eq!(grid.building(cell).is_some(), covered.contains(&cell));
        }
    }
}

// --- Equation 4: an ore cell depletes exactly to zero ---------------------------------------

#[test]
fn an_ore_cell_depletes_exactly_to_zero() {
    // One ore cell with 22 units, a harvester of capacity 20 and rate 3 standing on it, and a
    // refinery nearby (x 19..=21, y 10..=11). No other ore is reachable.
    let mut app = scenario(0);
    clear_ore_field_except(&mut app, &[(23, 7)]);
    set_ore(&mut app, 23, 7, 22);
    let refinery = building(&mut app, HUMAN, "refinery", 19, 10);
    let worker = unit(&mut app, HUMAN, "harvester", 23, 7);
    let ore_before = total_ore(&app);

    // First visit: 3 per tick until the harvester is full; the last take is limited by the
    // room left (2), not by the rate.
    let mut loads = Vec::new();
    let mut cell_ore = Vec::new();
    for _ in 0..7 {
        tick(&mut app);
        loads.push(harvester(&mut app, worker).0.load);
        cell_ore.push(ore(&app, 23, 7));
    }
    assert_eq!(loads, [3, 6, 9, 12, 15, 18, 20]);
    assert!(loads.windows(2).all(|pair| pair[1] - pair[0] <= RATE));
    assert_eq!(cell_ore, [19, 16, 13, 10, 7, 4, 2]);
    let (state, _, orders) = harvester(&mut app, worker);
    assert!(matches!(state.state, HarvestState::ToRefinery { refinery: r, .. } if r == refinery));
    assert!(orders.current.is_some(), "on its way to the refinery");

    // The rest of the cycle: unload 20 (40 credits), return, take the last 2 units (limited by
    // the cell, not by the rate), unload them (4 credits), and stop: no ore is left.
    let mut events = Vec::new();
    for _ in 0..2_000 {
        let tick_events = tick(&mut app);
        assert!(ore(&app, 23, 7) <= 2);
        let done = tick_events.iter().any(|event| {
            matches!(event, SimEvent::HarvesterIdle { unit, reason: HarvestIdle::NoOre } if *unit == worker)
        });
        events.extend(tick_events);
        if done {
            break;
        }
    }
    assert_eq!(
        delivered(&events),
        [(worker, refinery, 20, 40), (worker, refinery, 2, 4)]
    );
    assert_eq!(ore(&app, 23, 7), 0);
    assert_eq!(credits(&app), 44);
    assert_eq!(total_ore(&app), ore_before - 22);
    let (state, _, orders) = harvester(&mut app, worker);
    assert_eq!((state.load, state.state), (0, HarvestState::Idle));
    assert_eq!(orders.current, None);
}

#[test]
fn harvest_rate_equal_to_the_capacity_fills_in_one_tick() {
    // Boundary of the data rule `harvest_rate <= capacity`: written as a variant of the fixture
    // with rate 20, the harvester fills from a cell of 25 in one tick and leaves 5.
    let text = std::fs::read_to_string(fixture("data/economy.ron")).expect("fixture");
    let edited = text.replacen("harvest_rate: 3", "harvest_rate: 20", 1);
    assert_ne!(text, edited);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m3_harvest");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join("rate_equals_capacity.ron");
    std::fs::write(&path, edited).expect("variant");
    let mut app = scenario(0);
    app.world_mut().insert_resource(
        load_definitions(fixture("data/units_economy.ron"), &path)
            .unwrap_or_else(|error| panic!("{error}")),
    );
    building(&mut app, HUMAN, "refinery", 19, 10);
    let worker = unit(&mut app, HUMAN, "harvester", 24, 8);
    tick(&mut app);
    assert_eq!(harvester(&mut app, worker).0.load, CAPACITY);
    assert_eq!(ore(&app, 24, 8), 5);
}

// --- No reachable ore -----------------------------------------------------------------------

#[test]
fn a_harvester_with_no_reachable_ore_idles() {
    // Sealed in a rock ring around (28, 27): the ore field and the ore sealed around (3, 27)
    // exist, but neither is reachable.
    let mut app = scenario(0);
    building(&mut app, HUMAN, "refinery", 19, 10);
    let sealed = unit(&mut app, HUMAN, "harvester", 28, 27);
    // On open ground, with the field emptied: the only ore left is sealed in rock.
    let mut other = scenario(0);
    building(&mut other, HUMAN, "refinery", 19, 10);
    clear_ore_field_except(&mut other, &[]);
    assert_eq!(
        total_ore(&other),
        25,
        "only the sealed cell (3, 27) holds ore"
    );
    let open = unit(&mut other, HUMAN, "harvester", 10, 20);

    for (app, worker) in [(&mut app, sealed), (&mut other, open)] {
        let events = tick(app);
        assert_eq!(
            events,
            [SimEvent::HarvesterIdle {
                unit: worker,
                reason: HarvestIdle::NoOre,
            }]
        );
        let start = harvester(app, worker);
        assert_eq!(start.0.state, HarvestState::Idle);
        assert_eq!(start.2.current, None);
        // It stays idle without asking for paths or searching again.
        for _ in 0..500 {
            assert!(tick(app).is_empty());
        }
        assert_eq!(harvester(app, worker), start);
        // A `Harvest` command makes it look again, and it idles again.
        issue(
            app,
            HUMAN,
            Command::Harvest {
                units: vec![worker],
                target: CellPos::new(3, 27),
            },
        );
        // The ore cell nearest to the target is sealed in rock: the move to it fails, and the
        // harvester, finding no reachable ore from where it stands, idles again.
        let events = ticks(app, 2);
        assert_eq!(
            events,
            [
                SimEvent::MoveFailed {
                    unit: worker,
                    reason: sim::MoveFailure::Unreachable,
                },
                SimEvent::HarvesterIdle {
                    unit: worker,
                    reason: HarvestIdle::NoOre,
                }
            ]
        );
        assert!(ticks(app, 200).is_empty());
    }

    // A harvester that carries ore but has no reachable ore and no refinery stops with
    // `NoRefinery`, keeping its load.
    let mut app = scenario(0);
    let refinery = building(&mut app, HUMAN, "refinery", 19, 10);
    clear_ore_field_except(&mut app, &[(23, 7)]);
    set_ore(&mut app, 23, 7, 5);
    let worker = unit(&mut app, HUMAN, "harvester", 23, 7);
    ticks(&mut app, 2);
    issue(
        &mut app,
        HUMAN,
        Command::SellBuilding { building: refinery },
    );
    let events = ticks(&mut app, 5);
    assert!(events.contains(&SimEvent::HarvesterIdle {
        unit: worker,
        reason: HarvestIdle::NoRefinery,
    }));
    let (state, ..) = harvester(&mut app, worker);
    assert_eq!((state.load, state.state), (5, HarvestState::Idle));
}

// --- Refinery lost during the return trip ---------------------------------------------------

/// A full harvester on its way to `near`, the refinery nearest to the ore field.
fn returning(far: bool) -> (App, SimId, SimId, Option<SimId>) {
    let mut app = scenario(0);
    let near = building(&mut app, HUMAN, "refinery", 19, 10);
    let far = far.then(|| building(&mut app, HUMAN, "refinery", 4, 20));
    let worker = unit(&mut app, HUMAN, "harvester", 24, 8);
    for _ in 0..100 {
        tick(&mut app);
        let (state, mobile, orders) = harvester(&mut app, worker);
        if matches!(state.state, HarvestState::ToRefinery { refinery, .. } if refinery == near)
            && orders.current.is_some()
            && mobile.step.is_some()
        {
            assert_eq!(state.load, CAPACITY);
            return (app, worker, near, far);
        }
    }
    panic!("the harvester did not set out for the refinery");
}

#[test]
fn a_refinery_sold_during_the_return_trip_makes_the_harvester_retarget() {
    let (mut app, worker, near, far) = returning(true);
    let far = far.expect("second refinery");
    issue(&mut app, HUMAN, Command::SellBuilding { building: near });
    tick(&mut app);
    let (state, _, orders) = harvester(&mut app, worker);
    let HarvestState::ToRefinery { refinery, dock } = state.state else {
        panic!("{state:?}");
    };
    assert_eq!(refinery, far);
    assert_eq!(
        orders.current.as_ref().map(|order| order.destination),
        Some(dock)
    );
    // The far refinery covers x 4..=6, y 20..=21; the dock is in its ring.
    assert!(
        (3..=7).contains(&dock.x) && (19..=22).contains(&dock.y),
        "{dock:?}"
    );
    let mut events = Vec::new();
    for _ in 0..2_000 {
        events.extend(tick(&mut app));
        if !delivered(&events).is_empty() {
            break;
        }
    }
    assert_eq!(
        delivered(&events),
        [(worker, far, CAPACITY, i64::from(CAPACITY) * CREDITS_PER_ORE)]
    );
    assert_eq!(
        credits(&app),
        100 + 40,
        "sell value of the near refinery and the delivery"
    );
}

#[test]
fn a_refinery_sold_during_the_return_trip_with_no_other_leaves_the_harvester_idle() {
    let (mut app, worker, near, _) = returning(false);
    issue(&mut app, HUMAN, Command::SellBuilding { building: near });
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::HarvesterIdle {
        unit: worker,
        reason: HarvestIdle::NoRefinery,
    }));
    let (state, mobile, orders) = harvester(&mut app, worker);
    assert_eq!((state.load, state.state), (CAPACITY, HarvestState::Idle));
    assert_eq!(orders.current, None);
    // It completes its step and stands still; nothing panics.
    let events = ticks(&mut app, 300);
    assert!(events.is_empty(), "{events:?}");
    let (_, after, _) = harvester(&mut app, worker);
    assert_eq!(after.step, None);
    assert_eq!(Some(after.cell), mobile.step.map(|step| step.to));
    // With a refinery again, a `Harvest` command resumes the cycle: the full harvester goes
    // straight to the refinery and unloads.
    let refinery = building(&mut app, HUMAN, "refinery", 4, 20);
    issue(
        &mut app,
        HUMAN,
        Command::Harvest {
            units: vec![worker],
            target: CellPos::new(23, 7),
        },
    );
    tick(&mut app);
    assert!(matches!(
        harvester(&mut app, worker).0.state,
        HarvestState::ToRefinery { refinery: r, .. } if r == refinery
    ));
    let mut events = Vec::new();
    for _ in 0..2_000 {
        events.extend(tick(&mut app));
        if !delivered(&events).is_empty() {
            break;
        }
    }
    assert_eq!(delivered(&events), [(worker, refinery, CAPACITY, 40)]);
    assert_eq!(
        credits(&app),
        100 + 40,
        "the sold refinery's value and the delivery"
    );
}

// --- Several harvesters ---------------------------------------------------------------------

#[test]
fn harvesters_share_the_field_and_every_unit_of_ore_is_paid_once() {
    let mut app = scenario(0);
    let refinery = building(&mut app, HUMAN, "refinery", 19, 10);
    let workers: Vec<SimId> = [(14, 14), (15, 14), (16, 14), (17, 14)]
        .into_iter()
        .map(|(x, y)| unit(&mut app, HUMAN, "harvester", x, y))
        .collect();
    let ore_before = total_ore(&app);
    let mut paid = 0_i64;
    let mut unloaded = 0_u64;
    for _ in 0..3_000 {
        let events = tick(&mut app);
        for (worker, at, ore, credits) in delivered(&events) {
            assert!(workers.contains(&worker));
            assert_eq!(at, refinery);
            assert!(ore <= CAPACITY);
            assert_eq!(credits, i64::from(ore) * CREDITS_PER_ORE);
            paid += credits;
            unloaded += u64::from(ore);
        }
        // No two harvesters set out for the same ore cell.
        let mut targets = BTreeSet::new();
        for &worker in &workers {
            if let HarvestState::ToOre(cell) = harvester(&mut app, worker).0.state {
                assert!(targets.insert(cell), "{cell:?} taken twice");
            }
        }
        // Conservation: ore removed from the map is either carried or paid for.
        let carried: u64 = workers
            .iter()
            .map(|&worker| u64::from(harvester(&mut app, worker).0.load))
            .sum();
        assert_eq!(ore_before - total_ore(&app), unloaded + carried);
        assert_eq!(credits(&app), paid);
    }
    assert!(
        unloaded >= u64::from(CAPACITY) * 4,
        "every harvester delivered"
    );
}

// --- Commands -------------------------------------------------------------------------------

#[test]
fn harvest_commands_are_validated_and_move_and_stop_end_the_cycle() {
    let mut app = scenario(0);
    building(&mut app, HUMAN, "refinery", 19, 10);
    let worker = unit(&mut app, HUMAN, "harvester", 10, 14);
    let walker = unit(&mut app, HUMAN, "walker", 11, 14);
    app.world_mut()
        .resource_mut::<Players>()
        .add(ENEMY, 0)
        .expect("enemy");
    let foreign = unit(&mut app, ENEMY, "harvester", 12, 14);
    let cases = [
        (
            Command::Harvest {
                units: vec![worker, walker],
                target: CellPos::new(23, 7),
            },
            RejectReason::NotAHarvester(walker),
        ),
        (
            Command::Harvest {
                units: vec![foreign],
                target: CellPos::new(23, 7),
            },
            RejectReason::NotOwned(foreign),
        ),
        (
            Command::Harvest {
                units: vec![worker],
                target: CellPos::new(40, 7),
            },
            RejectReason::OutsideMap(CellPos::new(40, 7)),
        ),
        (
            Command::Harvest {
                units: vec![],
                target: CellPos::new(23, 7),
            },
            RejectReason::NoUnits,
        ),
        (
            Command::Harvest {
                units: vec![worker, worker],
                target: CellPos::new(23, 7),
            },
            RejectReason::DuplicateUnit(worker),
        ),
    ];
    let expected: Vec<RejectReason> = cases.iter().map(|(_, reason)| reason.clone()).collect();
    for (command, _) in cases {
        issue(&mut app, HUMAN, command);
    }
    let events = tick(&mut app);
    assert_eq!(rejections(&events), expected);

    // A move ends the cycle: the harvester walks to the cell and stays.
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![worker],
            target: CellPos::new(5, 20),
        },
    );
    let events = ticks(&mut app, 400);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: worker,
        retargeted: false,
    }));
    let (state, mobile, _) = harvester(&mut app, worker);
    assert_eq!(state.state, HarvestState::Idle);
    assert_eq!(mobile.cell, CellPos::new(5, 20));
    // `Harvest` resumes it at the ore cell nearest to the target.
    issue(
        &mut app,
        HUMAN,
        Command::Harvest {
            units: vec![worker],
            target: CellPos::new(25, 9),
        },
    );
    tick(&mut app);
    assert_eq!(
        harvester(&mut app, worker).0.state,
        HarvestState::ToOre(CellPos::new(25, 9))
    );
    let events = ticks(&mut app, 400);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::MoveCompleted { unit, .. } if *unit == worker))
    );
    assert!(harvester(&mut app, worker).0.load > 0 || !delivered(&events).is_empty());
    // `Stop` ends it as well.
    issue(
        &mut app,
        HUMAN,
        Command::Stop {
            units: vec![worker],
        },
    );
    tick(&mut app);
    let load = harvester(&mut app, worker).0.load;
    ticks(&mut app, 100);
    let (state, ..) = harvester(&mut app, worker);
    assert_eq!((state.state, state.load), (HarvestState::Idle, load));
}

#[test]
fn a_harvester_that_cannot_get_through_stops_after_its_attempts() {
    // The only ore cell is surrounded by walkers that stand still: the harvester sets out,
    // cannot reach it, and stops after MAX_HARVEST_ATTEMPTS attempts.
    let mut app = scenario(0);
    building(&mut app, HUMAN, "refinery", 19, 10);
    clear_ore_field_except(&mut app, &[(24, 8)]);
    for (x, y) in [
        (23, 7),
        (24, 7),
        (25, 7),
        (23, 8),
        (25, 8),
        (23, 9),
        (24, 9),
        (25, 9),
    ] {
        unit(&mut app, HUMAN, "walker", x, y);
    }
    // A walker stands on the ore cell as well, so the cell is never free.
    unit(&mut app, HUMAN, "walker", 24, 8);
    let worker = unit(&mut app, HUMAN, "harvester", 10, 14);
    let events = ticks(&mut app, 50);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, SimEvent::HarvesterIdle { .. }))
            .collect::<Vec<_>>(),
        [&SimEvent::HarvesterIdle {
            unit: worker,
            reason: HarvestIdle::NoOre,
        }],
        "an ore cell held by a unit is not a target"
    );

    // Now the ore cell is free but every way to it is held: the harvester sets out and stops
    // after its attempts, each ending when the move order cannot get through.
    let mut app = scenario(0);
    building(&mut app, HUMAN, "refinery", 19, 10);
    clear_ore_field_except(&mut app, &[(24, 8)]);
    for (x, y) in [
        (23, 7),
        (24, 7),
        (25, 7),
        (23, 8),
        (25, 8),
        (23, 9),
        (24, 9),
        (25, 9),
    ] {
        unit(&mut app, HUMAN, "walker", x, y);
    }
    let worker = unit(&mut app, HUMAN, "harvester", 10, 14);
    let mut idle = None;
    let mut set_out = 0;
    let mut moving = false;
    for _ in 0..20_000 {
        let events = tick(&mut app);
        let has_order = harvester(&mut app, worker).2.current.is_some();
        if has_order && !moving {
            set_out += 1;
        }
        moving = has_order;
        for event in events {
            if let SimEvent::HarvesterIdle { unit, reason } = event {
                assert_eq!(unit, worker);
                idle = Some(reason);
            }
        }
        if idle.is_some() {
            break;
        }
    }
    assert_eq!(idle, Some(HarvestIdle::Blocked));
    let (state, ..) = harvester(&mut app, worker);
    assert_eq!((state.state, state.load), (HarvestState::Idle, 0));
    assert_eq!(set_out, usize::from(MAX_HARVEST_ATTEMPTS));
}

// --- End to end: a base built and funded from ore through commands --------------------------

/// The scripted build order: power plant, refinery (whose free harvester starts harvesting),
/// barracks. The starting credits pay exactly for the power plant and the refinery, so the
/// barracks can only be paid for with ore. Returns the hash trace, the tick of the barracks'
/// placement, the credits then, and the final credits.
fn build_a_base(budget: u64) -> (Vec<u64>, u64, i64, i64, Vec<SimEvent>) {
    let mut app = scenario(105 + 200);
    building(&mut app, HUMAN, "yard", 19, 6);
    let defs = app.world().resource::<Definitions>().clone();
    let kind = |id: &str| defs.building_kind(id).expect("type");
    let steps = [
        ("power", CellPos::new(16, 6)),
        ("refinery", CellPos::new(19, 10)),
        ("barracks", CellPos::new(15, 2)),
    ];
    let mut step = 0;
    let mut queued = false;
    let mut placed_barracks = None;
    let mut trace = Vec::new();
    let mut all = Vec::new();
    for at in 0..budget {
        if step < steps.len() && !queued {
            issue(
                &mut app,
                HUMAN,
                Command::QueueProduction {
                    item: ProductionItem::Building(kind(steps[step].0)),
                },
            );
            queued = true;
        }
        let events = tick(&mut app);
        assert_eq!(rejections(&events), [], "tick {at}");
        if step < steps.len()
            && events.contains(&SimEvent::ProductionComplete {
                player: HUMAN,
                item: ProductionItem::Building(kind(steps[step].0)),
            })
        {
            issue(
                &mut app,
                HUMAN,
                Command::PlaceBuilding {
                    building: kind(steps[step].0),
                    origin: steps[step].1,
                },
            );
            let placed = tick(&mut app);
            assert_eq!(rejections(&placed), [], "placing {}", steps[step].0);
            if steps[step].0 == "barracks" {
                placed_barracks = Some((at + 1, credits(&app)));
            }
            all.extend(placed);
            step += 1;
            queued = false;
        }
        all.extend(events);
        trace.push(state_hash(app.world_mut()));
    }
    let (placed_at, credits_then) = placed_barracks.expect("the barracks was placed");
    (trace, placed_at, credits_then, credits(&app), all)
}

#[test]
fn a_base_is_built_and_funded_from_ore_through_commands_alone() {
    const BUDGET: u64 = 3_000;
    let (trace, placed_at, credits_then, final_credits, events) = build_a_base(BUDGET);
    // Everything was placed, in order, within the budget.
    let placed: Vec<SimId> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::BuildingPlaced { building, .. } => Some(*building),
            _ => None,
        })
        .collect();
    assert_eq!(placed.len(), 3);
    // The barracks could only be paid for with ore: the starting credits covered the first two
    // buildings exactly.
    let deliveries = delivered(&events);
    let income: i64 = deliveries.iter().map(|&(.., credits)| credits).sum();
    assert!(!deliveries.is_empty());
    assert_eq!(
        final_credits,
        305 - (105 + 200 + 150) + income,
        "the ledger balances"
    );
    // Income is positive after the base is complete: credits keep rising from ore.
    let later: i64 = events
        .iter()
        .skip_while(|event| !matches!(event, SimEvent::BuildingPlaced { building, .. } if *building == placed[2]))
        .filter_map(|event| match event {
            SimEvent::OreDelivered { credits, .. } => Some(*credits),
            _ => None,
        })
        .sum();
    assert!(
        later > 0,
        "income after the barracks was placed at tick {placed_at}"
    );
    assert!(final_credits > credits_then);
    assert!(placed_at < BUDGET);
    // Determinism: the same script gives the same hash at every tick.
    let (again, ..) = build_a_base(BUDGET);
    assert_eq!(trace, again);
}

#[test]
fn the_hash_covers_the_harvester_cycle() {
    // Worlds that differ only in one field of a harvester's cycle (set directly, as scenario
    // setup) hash differently.
    let edits: [fn(&mut Harvester); 4] = [
        |_| {},
        |harvester| harvester.load = 1,
        |harvester| harvester.attempts = 1,
        |harvester| harvester.state = HarvestState::ToOre(CellPos::new(1, 1)),
    ];
    let mut hashes = Vec::new();
    for edit in edits {
        let mut app = scenario(0);
        unit(&mut app, HUMAN, "harvester", 10, 14);
        let world = app.world_mut();
        let mut query = world.query::<&mut Harvester>();
        for mut harvester in query.iter_mut(world) {
            edit(&mut harvester);
        }
        hashes.push(state_hash(app.world_mut()));
    }
    let distinct: BTreeSet<u64> = hashes.iter().copied().collect();
    assert_eq!(distinct.len(), hashes.len(), "{hashes:?}");
}
