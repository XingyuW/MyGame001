//! M2 tests: units, `Move` and `Stop`, pathfinding, reservation, group orders, and repathing
//! (PLAN.md section 7, M2; invariants I3, I5, I6, I8).
//!
//! Every scenario runs through the production path: `SimPlugin` installs the resources and
//! `SimSchedule`, the grid and definitions come from fixture files, units are spawned with
//! `sim::spawn_unit` as scenario setup, and every order goes through `CommandQueue`. The only
//! other direct state changes are scenario setup: `Grid::set_terrain` to place an obstacle and
//! `spawn_unit` to place a unit in the way.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, DIAGONAL_COST, Definitions, Direction, Facing, Grid,
    IssuedCommand, MAX_UNIT_SPEED, MAX_UNITS_PER_COMMAND, Mobile, MoveFailure, ORTHOGONAL_COST,
    Orders, PATH_REQUESTS_PER_TICK, PathState, PlayerId, Position, REPATH_AFTER_BLOCKED_TICKS,
    RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, SpawnError, SubPos, Unit,
    destination_cells, find_path, load_map, load_unit_definitions, spawn_unit, state_hash,
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

fn load_grid(map: &str) -> Grid {
    load_map(fixture(&format!("maps/{map}.tmx")))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid
}

fn definitions() -> Definitions {
    load_unit_definitions(fixture("data/units.ron")).unwrap_or_else(|error| panic!("{error}"))
}

/// A simulation with the fixture map `map` and the fixture definitions installed (match setup).
fn scenario(map: &str) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(load_grid(map));
    app.world_mut().insert_resource(definitions());
    app
}

fn spawn(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("spawn at ({x}, {y}): {error}"))
}

fn issue(app: &mut App, player: PlayerId, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand { player, command });
}

fn move_to(app: &mut App, units: &[SimId], x: i32, y: i32) {
    issue(
        app,
        HUMAN,
        Command::Move {
            units: units.to_vec(),
            target: CellPos::new(x, y),
        },
    );
}

/// Runs one tick and returns its events.
fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn ticks(app: &mut App, n: u32) -> Vec<SimEvent> {
    (0..n).flat_map(|_| tick(app)).collect()
}

#[derive(Clone, Debug)]
struct UnitState {
    owner: PlayerId,
    mobile: Mobile,
    position: SubPos,
    facing: Direction,
    orders: Orders,
}

fn units(app: &mut App) -> BTreeMap<SimId, UnitState> {
    let world = app.world_mut();
    let mut query = world.query::<(
        &SimId,
        &sim::Owner,
        &Mobile,
        &Position,
        &Facing,
        &Orders,
        &Unit,
    )>();
    query
        .iter(world)
        .map(|(id, owner, mobile, position, facing, orders, _)| {
            (
                *id,
                UnitState {
                    owner: owner.0,
                    mobile: *mobile,
                    position: position.0,
                    facing: facing.0,
                    orders: orders.clone(),
                },
            )
        })
        .collect()
}

fn unit(app: &mut App, id: SimId) -> UnitState {
    units(app).remove(&id).expect("unit exists")
}

fn cell_of(app: &mut App, id: SimId) -> CellPos {
    unit(app, id).mobile.cell
}

fn hash(app: &mut App) -> u64 {
    state_hash(app.world_mut())
}

fn all_cells(grid: &Grid) -> impl Iterator<Item = CellPos> + '_ {
    (0..grid.height() as i32)
        .flat_map(move |y| (0..grid.width() as i32).map(move |x| CellPos::new(x, y)))
}

/// Whether `position` lies in the closed square of `cell` (a point on a shared edge or corner
/// touches every cell around it).
fn touches(position: SubPos, cell: CellPos) -> bool {
    let inside = |p: i32, c: i32| (c * 256..=c * 256 + 256).contains(&p);
    inside(position.x, cell.x) && inside(position.y, cell.y)
}

/// Invariant I6, checked from the units' own state and from the grid independently:
/// * no cell is held by two units (a unit holds the cell it stands in and, while stepping, the
///   cell it enters);
/// * each unit's fine position lies within the cells it holds (on a diagonal step the line
///   between the two centres passes exactly through the corner they share with two other
///   cells, which a unit crossing the other way can never pass at the same time);
/// * the grid's occupant of exactly those cells is the unit, and no other cell is held;
/// * no unit enters an impassable cell;
/// * no two units are at the same point.
fn assert_i6(app: &mut App, context: &str) {
    let units = units(app);
    let grid = app.world().resource::<Grid>().clone();
    let mut held: BTreeMap<CellPos, SimId> = BTreeMap::new();
    let mut positions: BTreeMap<SubPos, SimId> = BTreeMap::new();
    for (&id, state) in &units {
        let mut cells = vec![state.mobile.cell];
        if let Some(step) = state.mobile.step {
            assert!(grid.is_passable(step.to), "{context}: {id:?} enters rock");
            cells.push(step.to);
        }
        for cell in cells.iter().copied() {
            if let Some(other) = held.insert(cell, id) {
                panic!("{context}: cell {cell:?} held by {other:?} and {id:?}");
            }
            assert_eq!(grid.occupant(cell), Some(id), "{context}: grid at {cell:?}");
        }
        assert!(
            cells.iter().any(|&cell| touches(state.position, cell)),
            "{context}: {id:?} at {:?} outside its cells {cells:?}",
            state.position
        );
        if let Some(other) = positions.insert(state.position, id) {
            panic!(
                "{context}: {other:?} and {id:?} both at {:?}",
                state.position
            );
        }
    }
    for cell in all_cells(&grid) {
        if let Some(holder) = grid.occupant(cell) {
            assert_eq!(
                held.get(&cell),
                Some(&holder),
                "{context}: stray claim {cell:?}"
            );
        }
    }
}

// --- Reference pathfinder -------------------------------------------------------------------

/// Dijkstra over the same rules as PLAN.md section 5, equation 5, written independently of the
/// production A*: the cheapest cost from `start` to every cell.
fn reference_costs(grid: &Grid, start: CellPos) -> BTreeMap<CellPos, u32> {
    let passable = |c: CellPos| grid.is_passable(c);
    let mut best: BTreeMap<CellPos, u32> = BTreeMap::new();
    let mut open = BinaryHeap::new();
    open.push(Reverse((0_u32, start.x, start.y)));
    while let Some(Reverse((cost, x, y))) = open.pop() {
        let cell = CellPos::new(x, y);
        if best.contains_key(&cell) {
            continue;
        }
        best.insert(cell, cost);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if (dx, dy) == (0, 0) {
                    continue;
                }
                let next = CellPos::new(x + dx, y + dy);
                if !passable(next) {
                    continue;
                }
                let diagonal = dx != 0 && dy != 0;
                if diagonal
                    && !passable(CellPos::new(x + dx, y))
                    && !passable(CellPos::new(x, y + dy))
                {
                    continue;
                }
                let step = if diagonal { 14 } else { 10 };
                open.push(Reverse((cost + step, next.x, next.y)));
            }
        }
    }
    best
}

fn step_cost(from: CellPos, to: CellPos) -> u32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    assert!(
        dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0),
        "{from:?} -> {to:?}"
    );
    if dx != 0 && dy != 0 {
        DIAGONAL_COST
    } else {
        ORTHOGONAL_COST
    }
}

/// Checks that `cells` is a legal walk from `start` (each step to a passable neighbour, no
/// diagonal between two impassable cells) and returns its cost.
fn walk_cost(grid: &Grid, start: CellPos, cells: &[CellPos]) -> u32 {
    let mut cost = 0;
    let mut at = start;
    for &next in cells {
        cost += step_cost(at, next);
        assert!(grid.is_passable(next), "{next:?} is impassable");
        if next.x != at.x && next.y != at.y {
            assert!(
                grid.is_passable(CellPos::new(next.x, at.y))
                    || grid.is_passable(CellPos::new(at.x, next.y)),
                "diagonal step {at:?} -> {next:?} between two blocked cells"
            );
        }
        at = next;
    }
    cost
}

// --- Pathfinding ----------------------------------------------------------------------------

#[test]
fn path_cost_is_optimal_on_open_and_obstructed_fixture_maps() {
    for map in ["open_16x16", "obstructed_16x16"] {
        let grid = load_grid(map);
        let cells: Vec<CellPos> = all_cells(&grid).collect();
        let mut compared = 0;
        for &start in &cells {
            let reference = reference_costs(&grid, start);
            for &goal in cells.iter().step_by(3) {
                let found = find_path(&grid, start, goal, |_| false);
                let expected = if grid.is_passable(goal) || goal == start {
                    reference.get(&goal).copied()
                } else {
                    None
                };
                match (found, expected) {
                    (Some(path), Some(cost)) => {
                        assert_eq!(path.cost, cost, "{map}: {start:?} -> {goal:?}");
                        assert_eq!(walk_cost(&grid, start, &path.cells), cost);
                        assert_eq!(path.cells.last().copied().unwrap_or(start), goal);
                        compared += 1;
                    }
                    (None, None) => {}
                    (found, expected) => {
                        panic!("{map}: {start:?} -> {goal:?}: {found:?} vs {expected:?}")
                    }
                }
            }
        }
        assert!(compared > 10_000, "{map}: only {compared} paths compared");
    }
}

#[test]
fn no_diagonal_step_between_two_blocked_orthogonal_neighbours() {
    // Rocks at (3, 10) and (4, 11): the diagonal (3, 11) -> (4, 10) passes between them.
    let grid = load_grid("obstructed_16x16");
    let path = find_path(&grid, CellPos::new(3, 11), CellPos::new(4, 10), |_| false)
        .expect("a detour exists");
    assert!(path.cost > DIAGONAL_COST, "{path:?}");
    assert_eq!(
        path.cost,
        reference_costs(&grid, CellPos::new(3, 11))[&CellPos::new(4, 10)]
    );
    // With only one of the two blocked, the diagonal step is allowed.
    let open = load_grid("open_16x16");
    let mut one_rock = open.clone();
    one_rock.set_terrain(CellPos::new(3, 10), ROCK);
    let path = find_path(&one_rock, CellPos::new(3, 11), CellPos::new(4, 10), |_| {
        false
    })
    .expect("diagonal");
    assert_eq!(path.cost, DIAGONAL_COST);
}

#[test]
fn executed_route_is_optimal_through_the_production_path() {
    let mut app = scenario("obstructed_16x16");
    let start = CellPos::new(0, 15);
    let goal = CellPos::new(14, 0);
    let unit = spawn(&mut app, HUMAN, "walker", start.x, start.y);
    move_to(&mut app, &[unit], goal.x, goal.y);
    let mut visited = vec![];
    let mut completed = false;
    for _ in 0..2_000 {
        let events = tick(&mut app);
        let cell = cell_of(&mut app, unit);
        if cell != visited.last().copied().unwrap_or(start) {
            visited.push(cell);
        }
        assert_i6(&mut app, "route");
        if events.contains(&SimEvent::MoveCompleted {
            unit,
            retargeted: false,
        }) {
            completed = true;
            break;
        }
    }
    assert!(completed, "the unit did not arrive");
    let grid = app.world().resource::<Grid>().clone();
    assert_eq!(
        walk_cost(&grid, start, &visited),
        reference_costs(&grid, start)[&goal]
    );
    let state = unit_state_after(&mut app, unit);
    assert_eq!(state.mobile.cell, goal);
    assert_eq!(state.position, SubPos::centre_of(goal));
    assert!(state.orders.current.is_none());
}

fn unit_state_after(app: &mut App, id: SimId) -> UnitState {
    unit(app, id)
}

#[test]
fn unreachable_target_ends_the_order_with_an_event_and_no_panic() {
    let mut app = scenario("obstructed_16x16");
    let pocket = CellPos::new(12, 4);
    let unit = spawn(&mut app, HUMAN, "walker", 0, 0);
    move_to(&mut app, &[unit], pocket.x, pocket.y);
    let events = tick(&mut app);
    assert_eq!(
        events,
        vec![SimEvent::MoveFailed {
            unit,
            reason: MoveFailure::Unreachable
        }]
    );
    assert!(unit_state_after(&mut app, unit).orders.current.is_none());
    // The unit stays where it was, and the simulation keeps running.
    assert!(ticks(&mut app, 50).is_empty());
    assert_eq!(cell_of(&mut app, unit), CellPos::new(0, 0));

    // A group ordered into the one-cell pocket: one unit gets the pocket and finds no path, the
    // others get no destination at all.
    let others = [
        spawn(&mut app, HUMAN, "walker", 1, 0),
        spawn(&mut app, HUMAN, "walker", 2, 0),
    ];
    let group = [unit, others[0], others[1]];
    move_to(&mut app, &group, pocket.x, pocket.y);
    let events = tick(&mut app);
    let mut failures: Vec<_> = events
        .iter()
        .map(|event| match event {
            SimEvent::MoveFailed { unit, reason } => (*unit, *reason),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    failures.sort();
    assert_eq!(failures.len(), 3);
    assert_eq!(
        failures
            .iter()
            .filter(|(_, reason)| *reason == MoveFailure::Unreachable)
            .count(),
        1
    );
    assert_eq!(
        failures
            .iter()
            .filter(|(_, reason)| *reason == MoveFailure::NoDestination)
            .count(),
        2
    );

    // A target inside solid rock with no passable cell around it: no destination.
    let mut solid = scenario("open_16x16");
    for x in 5..=9 {
        for y in 5..=9 {
            solid
                .world_mut()
                .resource_mut::<Grid>()
                .set_terrain(CellPos::new(x, y), ROCK);
        }
    }
    let unit = spawn(&mut solid, HUMAN, "walker", 0, 0);
    move_to(&mut solid, &[unit], 7, 7);
    assert_eq!(
        tick(&mut solid),
        vec![SimEvent::MoveFailed {
            unit,
            reason: MoveFailure::NoDestination
        }]
    );
}

// --- Command validation ---------------------------------------------------------------------

/// Runs `command` in one scenario and nothing in an identical control scenario, and asserts that
/// the command is rejected with `reason` and that the state hashes agree at every tick.
fn assert_rejected_without_state_change(
    setup: impl Fn(&mut App) -> Vec<SimId>,
    player: PlayerId,
    command: impl Fn(&[SimId]) -> Command,
    reason: impl Fn(&[SimId]) -> RejectReason,
) {
    let mut app = scenario("open_16x16");
    let mut control = scenario("open_16x16");
    let ids = setup(&mut app);
    assert_eq!(setup(&mut control), ids);
    let command = command(&ids);
    issue(&mut app, player, command.clone());
    assert_eq!(hash(&mut app), hash(&mut control));
    let events = tick(&mut app);
    assert!(tick(&mut control).is_empty());
    assert_eq!(
        events,
        vec![SimEvent::CommandRejected {
            player,
            command,
            reason: reason(&ids),
        }]
    );
    for _ in 0..30 {
        assert_eq!(hash(&mut app), hash(&mut control));
        tick(&mut app);
        tick(&mut control);
    }
}

fn two_units_each(app: &mut App) -> Vec<SimId> {
    vec![
        spawn(app, HUMAN, "walker", 2, 2),
        spawn(app, HUMAN, "walker", 3, 2),
        spawn(app, ENEMY, "walker", 12, 12),
        spawn(app, ENEMY, "walker", 13, 12),
    ]
}

#[test]
fn a_move_outside_the_map_is_rejected() {
    for (x, y) in [
        (-1, 0),
        (0, -1),
        (16, 0),
        (0, 16),
        (16, 16),
        (i32::MIN, i32::MAX),
        (i32::MAX, 3),
    ] {
        assert_rejected_without_state_change(
            two_units_each,
            HUMAN,
            |ids| Command::Move {
                units: vec![ids[0], ids[1]],
                target: CellPos::new(x, y),
            },
            |_| RejectReason::OutsideMap(CellPos::new(x, y)),
        );
    }
}

#[test]
fn commands_for_missing_foreign_or_repeated_units_are_rejected() {
    let target = CellPos::new(8, 8);
    // No unit at all.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |_| Command::Move {
            units: vec![],
            target,
        },
        |_| RejectReason::NoUnits,
    );
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |_| Command::Stop { units: vec![] },
        |_| RejectReason::NoUnits,
    );
    // A unit that does not exist.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0], SimId(999)],
            target,
        },
        |_| RejectReason::UnknownUnit(SimId(999)),
    );
    // Another player's unit, alone or together with an own unit (the whole command is rejected,
    // so the own unit does not move either).
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0], ids[2]],
            target,
        },
        |ids| RejectReason::NotOwned(ids[2]),
    );
    assert_rejected_without_state_change(
        two_units_each,
        ENEMY,
        |ids| Command::Stop {
            units: vec![ids[0]],
        },
        |ids| RejectReason::NotOwned(ids[0]),
    );
    // The same unit twice.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0], ids[1], ids[0]],
            target,
        },
        |ids| RejectReason::DuplicateUnit(ids[0]),
    );
    // Too many units (boundary: the maximum itself is checked for duplicates, one more is too
    // many).
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0]; MAX_UNITS_PER_COMMAND + 1],
            target,
        },
        |_| RejectReason::TooManyUnits {
            count: MAX_UNITS_PER_COMMAND + 1,
            max: MAX_UNITS_PER_COMMAND,
        },
    );
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0]; MAX_UNITS_PER_COMMAND],
            target,
        },
        |ids| RejectReason::DuplicateUnit(ids[0]),
    );
}

#[test]
fn rejections_have_descriptive_messages() {
    let cell = CellPos::new(-1, 4);
    assert!(
        RejectReason::OutsideMap(cell)
            .to_string()
            .contains("(-1, 4) is outside the map")
    );
    assert!(
        RejectReason::NotOwned(SimId(7))
            .to_string()
            .contains("unit 7 belongs to another player")
    );
}

#[test]
fn spawning_is_checked() {
    let mut app = scenario("obstructed_16x16");
    let before = hash(&mut app);
    let world = app.world_mut();
    assert_eq!(
        spawn_unit(world, HUMAN, "tank", CellPos::new(0, 0)),
        Err(SpawnError::UnknownUnitType("tank".into()))
    );
    assert_eq!(
        spawn_unit(world, HUMAN, "walker", CellPos::new(16, 0)),
        Err(SpawnError::OutsideMap(CellPos::new(16, 0)))
    );
    assert_eq!(
        spawn_unit(world, HUMAN, "walker", CellPos::new(1, 1)),
        Err(SpawnError::Impassable(CellPos::new(1, 1)))
    );
    assert_eq!(hash(&mut app), before);
    let first = spawn(&mut app, HUMAN, "walker", 0, 0);
    let after_first = hash(&mut app);
    assert_eq!(
        spawn_unit(app.world_mut(), ENEMY, "runner", CellPos::new(0, 0)),
        Err(SpawnError::Occupied {
            cell: CellPos::new(0, 0),
            by: first
        })
    );
    assert_eq!(hash(&mut app), after_first);
    let state = unit(&mut app, first);
    assert_eq!(state.owner, HUMAN);
    assert_eq!(state.mobile.speed, 32);
    assert_eq!(state.position, SubPos::new(128, 128));
}

// --- Movement -------------------------------------------------------------------------------

#[test]
fn a_unit_ordered_to_its_own_cell_stays() {
    let mut app = scenario("open_16x16");
    let unit = spawn(&mut app, HUMAN, "walker", 5, 5);
    let before = unit_state_after(&mut app, unit);
    move_to(&mut app, &[unit], 5, 5);
    assert_eq!(
        tick(&mut app),
        vec![SimEvent::MoveCompleted {
            unit,
            retargeted: false
        }]
    );
    for _ in 0..100 {
        assert!(tick(&mut app).is_empty());
        let now = unit_state_after(&mut app, unit);
        assert_eq!(now.mobile, before.mobile);
        assert_eq!(now.position, before.position);
        assert_eq!(now.facing, before.facing);
        assert!(now.orders.current.is_none());
    }
}

#[test]
fn units_cover_exactly_their_speed_per_tick() {
    // PLAN.md section 5: cells per second = speed × 20 / 256. Along a straight orthogonal path,
    // the position advances by exactly `speed` subcells per tick, across cell boundaries.
    let mut app = scenario("open_16x16");
    let runner = spawn(&mut app, HUMAN, "runner", 0, 3);
    move_to(&mut app, &[runner], 15, 3);
    let start = unit_state_after(&mut app, runner).position;
    let total = 15 * 256;
    for k in 1..=(total / 100) {
        tick(&mut app);
        let now = unit_state_after(&mut app, runner).position;
        assert_eq!(now, SubPos::new(start.x + 100 * k, start.y), "tick {k}");
    }
    // The remainder of the way, then arrival exactly at the centre.
    let events = ticks(&mut app, 2);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: runner,
        retargeted: false
    }));
    assert_eq!(
        unit_state_after(&mut app, runner).position,
        SubPos::centre_of(CellPos::new(15, 3))
    );

    // Diagonally, a step is 362 subcells long and the position moves along the line between the
    // two centres.
    let mut app = scenario("open_16x16");
    let walker = spawn(&mut app, HUMAN, "walker", 2, 2);
    move_to(&mut app, &[walker], 3, 3);
    tick(&mut app);
    let state = unit_state_after(&mut app, walker);
    let step = state.mobile.step.expect("stepping");
    assert_eq!((step.progress, step.length), (32, 362));
    let offset = 256 * 32 / 362;
    assert_eq!(
        state.position,
        SubPos::new(2 * 256 + 128 + offset, 2 * 256 + 128 + offset)
    );
}

#[test]
fn facing_follows_each_step_direction() {
    for direction in Direction::ALL {
        let mut app = scenario("open_16x16");
        let unit = spawn(&mut app, HUMAN, "walker", 8, 8);
        let (dx, dy) = direction.delta();
        move_to(&mut app, &[unit], 8 + dx, 8 + dy);
        tick(&mut app);
        assert_eq!(unit_state_after(&mut app, unit).facing, direction);
        let events = ticks(&mut app, 20);
        assert!(
            events.contains(&SimEvent::MoveCompleted {
                unit,
                retargeted: false
            }),
            "{direction:?}"
        );
        assert_eq!(unit_state_after(&mut app, unit).facing, direction);
        assert_eq!(Direction::from_delta(dx, dy), Some(direction));
    }
    assert_eq!(Direction::from_delta(0, 0), None);
    assert_eq!(Direction::from_delta(2, 0), None);
}

#[test]
fn stop_ends_the_order_at_the_next_cell_centre() {
    let mut app = scenario("open_16x16");
    let unit = spawn(&mut app, HUMAN, "walker", 0, 0);
    move_to(&mut app, &[unit], 15, 0);
    ticks(&mut app, 12);
    let moving = unit_state_after(&mut app, unit);
    let step = moving.mobile.step.expect("the unit is between two cells");
    issue(&mut app, HUMAN, Command::Stop { units: vec![unit] });
    let events = ticks(&mut app, 40);
    assert!(events.is_empty(), "{events:?}");
    let stopped = unit_state_after(&mut app, unit);
    assert_eq!(stopped.mobile.cell, step.to);
    assert_eq!(stopped.mobile.step, None);
    assert_eq!(stopped.position, SubPos::centre_of(step.to));
    assert!(stopped.orders.current.is_none());
    // Stop on an idle unit is valid and changes nothing.
    let before = hash(&mut app);
    issue(&mut app, HUMAN, Command::Stop { units: vec![unit] });
    assert!(tick(&mut app).is_empty());
    assert_eq!(unit_state_after(&mut app, unit).mobile, stopped.mobile);
    assert_ne!(hash(&mut app), before, "the tick counter advanced");
}

#[test]
fn a_new_move_replaces_the_current_order() {
    let mut app = scenario("open_16x16");
    let unit = spawn(&mut app, HUMAN, "walker", 0, 8);
    move_to(&mut app, &[unit], 15, 8);
    ticks(&mut app, 5);
    move_to(&mut app, &[unit], 0, 15);
    let mut completed = false;
    for _ in 0..400 {
        if tick(&mut app).contains(&SimEvent::MoveCompleted {
            unit,
            retargeted: false,
        }) {
            completed = true;
            break;
        }
        assert_i6(&mut app, "retarget");
    }
    assert!(completed);
    assert_eq!(cell_of(&mut app, unit), CellPos::new(0, 15));
}

#[test]
fn a_newly_placed_obstacle_triggers_a_repath() {
    let mut app = scenario("open_16x16");
    let unit = spawn(&mut app, HUMAN, "walker", 0, 8);
    move_to(&mut app, &[unit], 15, 8);
    ticks(&mut app, 3);
    let order = unit_state_after(&mut app, unit)
        .orders
        .current
        .expect("moving");
    assert!(order.path.contains(&CellPos::new(8, 8)), "{:?}", order.path);
    assert_eq!(order.repaths, 0);
    // Scenario setup: a wall across the path at x = 8, open only at y = 0.
    for y in 1..16 {
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(8, y), ROCK);
    }
    let mut repathed = false;
    let mut completed = false;
    for _ in 0..1_000 {
        let events = tick(&mut app);
        assert_i6(&mut app, "obstacle");
        let state = unit_state_after(&mut app, unit);
        let grid = app.world().resource::<Grid>().clone();
        assert!(grid.is_passable(state.position.cell()), "{state:?}");
        if let Some(order) = &state.orders.current {
            repathed |= order.repaths > 0;
        }
        if events.contains(&SimEvent::MoveCompleted {
            unit,
            retargeted: false,
        }) {
            completed = true;
            break;
        }
    }
    assert!(repathed, "the obstacle did not trigger a repath");
    assert!(completed, "the unit did not arrive around the obstacle");
    assert_eq!(cell_of(&mut app, unit), CellPos::new(15, 8));

    // An obstacle on the destination itself makes it unreachable.
    let mut app = scenario("open_16x16");
    let unit = spawn(&mut app, HUMAN, "walker", 0, 2);
    move_to(&mut app, &[unit], 10, 2);
    ticks(&mut app, 3);
    app.world_mut()
        .resource_mut::<Grid>()
        .set_terrain(CellPos::new(10, 2), ROCK);
    let events = ticks(&mut app, 400);
    assert!(
        events.contains(&SimEvent::MoveFailed {
            unit,
            reason: MoveFailure::Unreachable
        }),
        "{events:?}"
    );
}

#[test]
fn a_unit_standing_in_the_way_is_bypassed() {
    let mut app = scenario("open_16x16");
    let mover = spawn(&mut app, HUMAN, "walker", 0, 8);
    move_to(&mut app, &[mover], 15, 8);
    tick(&mut app);
    // Scenario setup: an idle unit appears on the path ahead.
    let blocker = spawn(&mut app, ENEMY, "walker", 6, 8);
    let mut completed = false;
    let mut waited = 0;
    for _ in 0..1_000 {
        let events = tick(&mut app);
        assert_i6(&mut app, "bypass");
        if let Some(order) = unit_state_after(&mut app, mover).orders.current {
            waited = waited.max(order.blocked_ticks);
        }
        if events.contains(&SimEvent::MoveCompleted {
            unit: mover,
            retargeted: false,
        }) {
            completed = true;
            break;
        }
    }
    assert!(completed);
    // It waited, then went round the unit after at most `REPATH_AFTER_BLOCKED_TICKS` ticks.
    assert!(
        (1..=REPATH_AFTER_BLOCKED_TICKS).contains(&waited),
        "{waited}"
    );
    assert_eq!(cell_of(&mut app, blocker), CellPos::new(6, 8));
    assert_eq!(cell_of(&mut app, mover), CellPos::new(15, 8));
}

#[test]
fn a_destination_held_by_an_idle_unit_is_replaced_by_the_nearest_reachable_cell() {
    let mut app = scenario("open_16x16");
    let mover = spawn(&mut app, HUMAN, "walker", 0, 4);
    move_to(&mut app, &[mover], 10, 4);
    tick(&mut app);
    // Scenario setup: an idle unit appears on the destination after it was assigned.
    let blocker = spawn(&mut app, ENEMY, "walker", 10, 4);
    let mut completed = false;
    for _ in 0..1_000 {
        let events = tick(&mut app);
        assert_i6(&mut app, "blocked destination");
        if events.contains(&SimEvent::MoveCompleted {
            unit: mover,
            retargeted: true,
        }) {
            completed = true;
            break;
        }
        assert!(events.is_empty(), "{events:?}");
    }
    assert!(completed);
    // It stopped next to the destination, in the cell where it found the way closed: the
    // reachable cell nearest to it.
    let state = unit_state_after(&mut app, mover);
    assert_eq!(state.mobile.cell, CellPos::new(9, 4));
    assert_eq!(state.mobile.step, None);
    assert_eq!(cell_of(&mut app, blocker), CellPos::new(10, 4));
}

/// Orders `a` to the cell of `b` and `b` to the cell of `a`. Cells held by idle units outside a
/// group are never handed out as destinations, so the orders are given in three commands of the
/// same tick: first `a` gets an order (to any cell), so that it is no longer idle when `b` is
/// ordered to its cell, and then `a` is ordered to the cell of `b`, which has an order by then.
fn order_swap(app: &mut App, a: SimId, b: SimId) {
    let (cell_a, cell_b) = (cell_of(app, a), cell_of(app, b));
    move_to(app, &[a], 0, 0);
    move_to(app, &[b], cell_a.x, cell_a.y);
    move_to(app, &[a], cell_b.x, cell_b.y);
}

/// Runs until every unit of `units` has completed its order, checking I6 on every tick, and
/// returns the cells each unit stood in, in order.
fn run_until_completed(
    app: &mut App,
    units: &[SimId],
    limit: u32,
    context: &str,
) -> BTreeMap<SimId, Vec<CellPos>> {
    let mut done = BTreeSet::new();
    let mut visited: BTreeMap<SimId, Vec<CellPos>> = units
        .iter()
        .map(|&id| (id, vec![cell_of(app, id)]))
        .collect();
    for _ in 0..limit {
        for event in tick(app) {
            match event {
                SimEvent::MoveCompleted { unit, .. } => {
                    done.insert(unit);
                }
                other => panic!("{context}: unexpected {other:?}"),
            }
        }
        assert_i6(app, context);
        for (&id, cells) in &mut visited {
            let cell = cell_of(app, id);
            if cells.last() != Some(&cell) {
                cells.push(cell);
            }
        }
        if done.len() == units.len() {
            return visited;
        }
    }
    panic!("{context}: only {done:?} of {units:?} completed");
}

fn set_rock(app: &mut App, cells: impl IntoIterator<Item = (i32, i32)>) {
    for (x, y) in cells {
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(x, y), ROCK);
    }
}

#[test]
fn two_units_that_want_each_others_cell_resolve_by_one_stepping_aside() {
    // Rocks above and below both cells, so neither can pass the other on the spot: the two
    // form a wait-for cycle, and the one with the higher id steps aside.
    let mut app = scenario("open_16x16");
    set_rock(&mut app, [(5, 4), (6, 4), (5, 6), (6, 6)]);
    let a = spawn(&mut app, HUMAN, "walker", 5, 5);
    let b = spawn(&mut app, HUMAN, "walker", 6, 5);
    order_swap(&mut app, a, b);
    let visited = run_until_completed(&mut app, &[a, b], 400, "swap");
    assert_eq!(cell_of(&mut app, a), CellPos::new(6, 5));
    assert_eq!(cell_of(&mut app, b), CellPos::new(5, 5));
    // `a` went straight to its cell; `b` (the higher id) stepped aside to (7, 5), the free
    // neighbour nearest to its destination, and came round once `a` had passed.
    assert_eq!(visited[&a], [CellPos::new(5, 5), CellPos::new(6, 5)]);
    assert_eq!(visited[&b][..2], [CellPos::new(6, 5), CellPos::new(7, 5)]);
}

#[test]
fn units_meeting_head_on_in_a_one_cell_corridor_both_pass() {
    // Scenario setup: a corridor one cell wide along y = 5 from x = 3 to x = 12, open at both
    // ends. The unit that gives way backs out of the corridor and steps aside.
    let mut app = scenario("open_16x16");
    set_rock(&mut app, (3..=12).flat_map(|x| [(x, 4), (x, 6)]));
    let west = spawn(&mut app, HUMAN, "walker", 1, 5);
    let east = spawn(&mut app, HUMAN, "runner", 14, 5);
    move_to(&mut app, &[west], 15, 5);
    move_to(&mut app, &[east], 0, 5);
    run_until_completed(&mut app, &[west, east], 2_000, "corridor");
    assert_eq!(cell_of(&mut app, west), CellPos::new(15, 5));
    assert_eq!(cell_of(&mut app, east), CellPos::new(0, 5));
}

#[test]
fn every_order_ends_even_when_units_are_walled_in_by_waiting_units() {
    // Two units in a sealed two-cell room, each ordered to the other's cell: neither can step
    // aside, so each counts the other as a lasting obstacle and ends its order where it stands.
    let mut app = scenario("open_16x16");
    set_rock(
        &mut app,
        (4..=7)
            .flat_map(|x| (4..=6).map(move |y| (x, y)))
            .filter(|&(x, y)| !(y == 5 && (x == 5 || x == 6))),
    );
    let a = spawn(&mut app, HUMAN, "walker", 5, 5);
    let b = spawn(&mut app, HUMAN, "walker", 6, 5);
    order_swap(&mut app, a, b);
    run_until_completed(&mut app, &[a, b], 400, "sealed room");
    assert_eq!(cell_of(&mut app, a), CellPos::new(5, 5));
    assert_eq!(cell_of(&mut app, b), CellPos::new(6, 5));
}

// --- Groups ---------------------------------------------------------------------------------

/// Spawns `count` units of `player` in a block starting at (x0, y0), `per_row` per row.
fn spawn_block(
    app: &mut App,
    player: PlayerId,
    count: usize,
    (x0, y0): (i32, i32),
    per_row: i32,
) -> Vec<SimId> {
    (0..count as i32)
        .map(|i| spawn(app, player, "walker", x0 + i % per_row, y0 + i / per_row))
        .collect()
}

#[test]
fn path_requests_are_served_within_the_per_tick_budget_oldest_first() {
    let mut app = scenario("arena_32x32");
    let group = spawn_block(&mut app, HUMAN, 50, (1, 1), 10);
    move_to(&mut app, &group, 25, 25);
    let mut served_before = 0;
    for round in 1..=4 {
        tick(&mut app);
        let states = units(&mut app);
        let following: Vec<SimId> = group
            .iter()
            .copied()
            .filter(|id| {
                states[id]
                    .orders
                    .current
                    .as_ref()
                    .is_some_and(|order| order.state == PathState::Following)
            })
            .collect();
        let expected = (round * PATH_REQUESTS_PER_TICK).min(50);
        assert_eq!(following.len(), expected, "after tick {round}");
        assert!(following.len() > served_before);
        served_before = following.len();
    }
    // Requests of the same tick are served in id order.
    let mut app = scenario("arena_32x32");
    let group = spawn_block(&mut app, HUMAN, 20, (1, 1), 10);
    move_to(&mut app, &group, 25, 25);
    tick(&mut app);
    let states = units(&mut app);
    for (index, id) in group.iter().enumerate() {
        let following = states[id]
            .orders
            .current
            .as_ref()
            .is_some_and(|order| order.state == PathState::Following);
        assert_eq!(following, index < PATH_REQUESTS_PER_TICK, "{id:?}");
    }
}

#[test]
fn a_group_order_gives_every_unit_its_own_destination_cell() {
    let mut app = scenario("wall_32x32");
    let group = spawn_block(&mut app, HUMAN, 50, (1, 10), 10);
    let target = CellPos::new(24, 16);
    move_to(&mut app, &group, target.x, target.y);
    tick(&mut app);
    let states = units(&mut app);
    let destinations: Vec<CellPos> = group
        .iter()
        .map(|id| {
            states[id]
                .orders
                .current
                .as_ref()
                .expect("order")
                .destination
        })
        .collect();
    let distinct: BTreeSet<CellPos> = destinations.iter().copied().collect();
    assert_eq!(distinct.len(), 50, "destinations are not distinct");
    let grid = app.world().resource::<Grid>().clone();
    let expected: BTreeSet<CellPos> = destination_cells(&grid, target, 50, |_| false)
        .into_iter()
        .collect();
    assert_eq!(distinct, expected);
    assert!(distinct.contains(&target));
    assert!(distinct.iter().all(|&cell| grid.is_passable(cell)));
    // All destinations lie on the target's side of the wall.
    assert!(distinct.iter().all(|cell| cell.x > 16), "{distinct:?}");
}

/// Orders all `group` units to `target` and runs until each has completed its order, checking
/// invariant I6 on every tick. Returns the number of ticks taken.
fn run_group_to(app: &mut App, group: &[SimId], target: CellPos, limit: u32, context: &str) -> u32 {
    move_to(app, group, target.x, target.y);
    let mut pending: BTreeSet<SimId> = group.iter().copied().collect();
    for elapsed in 1..=limit {
        for event in tick(app) {
            match event {
                SimEvent::MoveCompleted { unit, .. } => {
                    pending.remove(&unit);
                }
                other => panic!("{context}: unexpected {other:?}"),
            }
        }
        assert_i6(app, context);
        if pending.is_empty() {
            return elapsed;
        }
    }
    panic!(
        "{context}: {} units did not arrive: {pending:?}",
        pending.len()
    );
}

#[test]
fn fifty_units_reach_a_clicked_destination_without_overlap_or_deadlock() {
    // Acceptance (PLAN.md section 7, M2), in the fixture scenarios.
    let cases = [
        ("arena_32x32", (1, 1), CellPos::new(24, 24)),
        ("arena_32x32", (20, 22), CellPos::new(3, 4)),
        ("wall_32x32", (1, 10), CellPos::new(24, 16)),
        ("wall_32x32", (20, 3), CellPos::new(5, 28)),
    ];
    for (map, origin, target) in cases {
        let mut app = scenario(map);
        let group = spawn_block(&mut app, HUMAN, 50, origin, 10);
        let context = format!("{map}, {origin:?} -> {target:?}");
        let took = run_group_to(&mut app, &group, target, 3_000, &context);
        let states = units(&mut app);
        let cells: BTreeSet<CellPos> = group.iter().map(|id| states[id].mobile.cell).collect();
        assert_eq!(cells.len(), 50, "{context}");
        // The units fill the 50 cells nearest to the target. A unit whose cell was closed off
        // by units that arrived first takes the nearest cell it can reach instead, so the group
        // may extend one ring beyond those cells; most units are at their own cells.
        let grid = app.world().resource::<Grid>().clone();
        let expected: BTreeSet<CellPos> = destination_cells(&grid, target, 50, |_| false)
            .into_iter()
            .collect();
        let radius = expected
            .iter()
            .map(|&cell| sim::octile_distance(cell, target))
            .max()
            .expect("50 cells");
        for &cell in &cells {
            assert!(
                sim::octile_distance(cell, target) <= radius + 2 * ORTHOGONAL_COST,
                "{context}: {cell:?} is too far from the target"
            );
        }
        let at_assigned = cells.intersection(&expected).count();
        assert!(
            at_assigned >= 35,
            "{context}: only {at_assigned} units at assigned cells"
        );
        println!("{context}: {took} ticks, {at_assigned} of 50 at the assigned cells");
    }
}

#[test]
fn invariant_i6_holds_while_fifty_units_cross_each_other_for_2000_ticks() {
    let mut app = scenario("arena_32x32");
    let west = spawn_block(&mut app, HUMAN, 25, (2, 11), 5);
    let east = spawn_block(&mut app, HUMAN, 25, (25, 11), 5);
    let mut completed = 0;
    let mut west_goes_east = true;
    let mut pending: BTreeSet<SimId> = BTreeSet::new();
    let mut crossings = 0;
    for tick_index in 0..2_000 {
        if pending.is_empty() {
            let (east_target, west_target) = if west_goes_east {
                (CellPos::new(27, 13), CellPos::new(4, 13))
            } else {
                (CellPos::new(4, 13), CellPos::new(27, 13))
            };
            move_to(&mut app, &west, east_target.x, east_target.y);
            move_to(&mut app, &east, west_target.x, west_target.y);
            pending = west.iter().chain(&east).copied().collect();
            west_goes_east = !west_goes_east;
            crossings += 1;
        }
        for event in tick(&mut app) {
            match event {
                SimEvent::MoveCompleted { unit, .. } => {
                    assert!(pending.remove(&unit));
                    completed += 1;
                }
                other => panic!("tick {tick_index}: unexpected {other:?}"),
            }
        }
        assert_i6(&mut app, &format!("tick {tick_index}"));
    }
    // No deadlock: the groups crossed repeatedly within the 2,000 ticks.
    println!("{crossings} crossings started, {completed} orders completed");
    assert!(crossings >= 3, "{crossings} crossings");
    assert!(completed >= 100, "{completed} completed orders");
}

/// SplitMix64, for seeded scenarios that are the same on every run.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

#[test]
fn invariant_i6_holds_and_every_order_ends_under_seeded_random_orders() {
    // 40 units of two players on the wall map receive random group orders (targets anywhere,
    // including rock and cells outside the map), random stops, and orders for units of the other
    // player, for 3,000 ticks; then they get no further orders. I6 must hold on every tick and
    // every order must end (no unit keeps an order forever).
    for seed in [1_u64, 2, 3] {
        let mut app = scenario("wall_32x32");
        let mut rng = SplitMix64(seed);
        let mut all = spawn_block(&mut app, HUMAN, 20, (2, 2), 5);
        all.extend(spawn_block(&mut app, ENEMY, 20, (22, 24), 5));
        for tick_index in 0..3_000 {
            if tick_index % 25 == 0 {
                let player = if rng.below(2) == 0 { HUMAN } else { ENEMY };
                let count = 1 + rng.below(12) as usize;
                let units: Vec<SimId> = (0..count)
                    .map(|_| all[rng.below(all.len() as u64) as usize])
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let command = if rng.below(5) == 0 {
                    Command::Stop { units }
                } else {
                    let target = CellPos::new(rng.below(36) as i32 - 2, rng.below(36) as i32 - 2);
                    Command::Move { units, target }
                };
                issue(&mut app, player, command);
            }
            tick(&mut app);
            assert_i6(&mut app, &format!("seed {seed}, tick {tick_index}"));
        }
        let mut quiet_after = None;
        for extra in 0..2_000 {
            tick(&mut app);
            assert_i6(&mut app, &format!("seed {seed}, tick {}", 3_000 + extra));
            if units(&mut app)
                .values()
                .all(|state| state.orders.current.is_none() && state.mobile.step.is_none())
            {
                quiet_after = Some(extra);
                break;
            }
        }
        assert!(
            quiet_after.is_some(),
            "seed {seed}: some orders never ended"
        );
    }
}

#[test]
fn the_same_scenario_gives_the_same_hash_at_every_tick() {
    fn trace(order_twice: bool) -> Vec<u64> {
        let mut app = scenario("wall_32x32");
        let west = spawn_block(&mut app, HUMAN, 25, (2, 10), 5);
        let east = spawn_block(&mut app, ENEMY, 25, (25, 10), 5);
        let mut trace = vec![hash(&mut app)];
        for tick_index in 0..600 {
            if tick_index == 0 || (order_twice && tick_index == 300) {
                move_to(&mut app, &west, 27, 20);
                issue(
                    &mut app,
                    ENEMY,
                    Command::Move {
                        units: east.clone(),
                        target: CellPos::new(4, 20),
                    },
                );
            }
            tick(&mut app);
            trace.push(hash(&mut app));
        }
        trace
    }
    let a = trace(false);
    let b = trace(false);
    assert_eq!(a, b);
    let c = trace(true);
    assert_eq!(a[..=300], c[..=300]);
    assert_ne!(a[301..], c[301..]);
}

#[test]
fn the_hash_covers_unit_state() {
    let mut a = scenario("open_16x16");
    let mut b = scenario("open_16x16");
    spawn(&mut a, HUMAN, "walker", 3, 3);
    spawn(&mut b, HUMAN, "walker", 3, 4);
    assert_ne!(hash(&mut a), hash(&mut b));
    let mut c = scenario("open_16x16");
    spawn(&mut c, ENEMY, "walker", 3, 3);
    assert_ne!(hash(&mut a), hash(&mut c));
    let mut d = scenario("open_16x16");
    spawn(&mut d, HUMAN, "runner", 3, 3);
    assert_ne!(hash(&mut a), hash(&mut d));
}

// --- Termination and event accuracy (M2 review 1) ---------------------------------------------

/// Runs with no further command until every unit has ended its order and stands at a cell
/// centre, checking I6 on every tick, and returns each unit's end event and the tick it came in.
/// Panics if some order is still live after `limit` ticks.
fn run_until_all_orders_end(
    app: &mut App,
    limit: u32,
    context: &str,
) -> BTreeMap<SimId, (u32, SimEvent)> {
    let mut ended = BTreeMap::new();
    for elapsed in 1..=limit {
        for event in tick(app) {
            match event {
                SimEvent::MoveCompleted { unit, .. } | SimEvent::MoveFailed { unit, .. } => {
                    let previous = ended.insert(unit, (elapsed, event.clone()));
                    assert!(previous.is_none(), "{context}: {unit:?} ended twice");
                }
                other => panic!("{context}: unexpected {other:?}"),
            }
        }
        assert_i6(app, &format!("{context}, tick {elapsed}"));
        if units(app)
            .values()
            .all(|state| state.orders.current.is_none() && state.mobile.step.is_none())
        {
            return ended;
        }
    }
    let live: Vec<(SimId, CellPos, Option<u32>)> = units(app)
        .into_iter()
        .filter_map(|(id, state)| {
            let order = state.orders.current?;
            Some((id, state.mobile.cell, Some(order.repaths)))
        })
        .collect();
    panic!("{context}: orders still live after {limit} ticks (unit, cell, repaths): {live:?}");
}

/// Ticks within which every order of the termination scenarios below must have ended. The
/// orders that cannot reach their destinations end after about 800 ticks (the slowest of 1,000
/// random scenarios settled 1,032 ticks after its last command); before the fix of M2 review 1
/// P1-1 they never ended (the reviewer ran 20,000 ticks).
const TERMINATION_LIMIT: u32 = 3_000;

#[test]
fn an_order_ends_when_two_units_keep_giving_way_to_each_other_in_a_sealed_room() {
    // M2 review 1, P1-1: a sealed room of three cells in a column, (7, 3), (7, 4), (7, 5).
    // Two units at its ends, each ordered to the other's cell, cannot pass each other. They
    // form a wait-for cycle, the yielder steps aside into the free cell, the other advances one
    // cell and blocks again, and the roles alternate: both keep moving, but neither gets closer
    // than it was after its first step.
    let mut app = scenario("open_16x16");
    set_rock(
        &mut app,
        (6..=8)
            .flat_map(|x| (2..=6).map(move |y| (x, y)))
            .filter(|&(x, y)| !(x == 7 && (3..=5).contains(&y))),
    );
    let a = spawn(&mut app, HUMAN, "walker", 7, 3);
    let b = spawn(&mut app, HUMAN, "walker", 7, 5);
    order_swap(&mut app, a, b);
    let ended = run_until_all_orders_end(&mut app, TERMINATION_LIMIT, "sealed room of three");
    assert_eq!(ended.len(), 2, "{ended:?}");
    // The first to give up fails as blocked; the other then finds its way closed by a unit that
    // stands still and stops at the reachable cell nearest to its destination.
    let failed: Vec<SimId> = ended
        .iter()
        .filter(|(_, (_, event))| {
            matches!(
                event,
                SimEvent::MoveFailed {
                    reason: MoveFailure::Blocked,
                    ..
                }
            )
        })
        .map(|(&id, _)| id)
        .collect();
    assert_eq!(failed.len(), 1, "{ended:?}");
    for (&id, (_, event)) in &ended {
        if !failed.contains(&id) {
            assert_eq!(
                *event,
                SimEvent::MoveCompleted {
                    unit: id,
                    retargeted: true
                }
            );
        }
    }
    // Neither stands in the other's starting cell.
    assert_ne!(cell_of(&mut app, a), CellPos::new(7, 5));
    assert_ne!(cell_of(&mut app, b), CellPos::new(7, 3));
}

#[test]
fn an_order_ends_when_units_keep_giving_way_in_a_dead_end_closed_by_idle_units() {
    // M2 review 1, P1-1, without a sealed room: a dead end one cell wide, x = 7 from y = 2 to
    // y = 6, with rock on both sides and at its end, whose opening at y = 7 is closed by three
    // idle units of the other player. Two units inside, each ordered to the other's cell.
    for (a_cell, b_cell) in [((7, 2), (7, 6)), ((7, 4), (7, 6)), ((7, 3), (7, 5))] {
        let context = format!("dead end, {a_cell:?} <-> {b_cell:?}");
        let mut app = scenario("open_16x16");
        set_rock(
            &mut app,
            (1..=6).flat_map(|y| [(6, y), (8, y)]).chain([(7, 1)]),
        );
        let idle: Vec<SimId> = (6..=8)
            .map(|x| spawn(&mut app, ENEMY, "walker", x, 7))
            .collect();
        let a = spawn(&mut app, HUMAN, "walker", a_cell.0, a_cell.1);
        let b = spawn(&mut app, HUMAN, "walker", b_cell.0, b_cell.1);
        order_swap(&mut app, a, b);
        let ended = run_until_all_orders_end(&mut app, TERMINATION_LIMIT, &context);
        assert_eq!(ended.len(), 2, "{context}: {ended:?}");
        // The idle units never moved.
        for (x, id) in (6..=8).zip(&idle) {
            assert_eq!(cell_of(&mut app, *id), CellPos::new(x, 7), "{context}");
        }
        // Nobody claims to have reached a cell it is not in.
        for (&id, (_, event)) in &ended {
            if let SimEvent::MoveCompleted {
                retargeted: false, ..
            } = event
            {
                let destination = if id == a { b_cell } else { a_cell };
                assert_eq!(
                    cell_of(&mut app, id),
                    CellPos::new(destination.0, destination.1),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn every_order_ends_on_random_maps_under_random_group_orders() {
    // M2 review 1, P1-1: seeded scenarios on the open 16x16 fixture with 0 to 34 % of the cells
    // turned to rock and 5 to 64 units, which receive random group orders for 1,500 ticks and
    // then no further command. The generator is the reviewer's (`simprobe livelock`): with the
    // reviewed code, seeds 62 and 154 (of the reviewer's 300) and 524, 754, 818, 909, and 928
    // (of 1,000) left two units with orders forever; seeds 0 to 47 are a sweep. All 1,000 were
    // run in release for the remediation report; in a debug build each takes about 0.4 s. I6
    // must hold on every tick, and every order must end within `TERMINATION_LIMIT` ticks of the
    // last command.
    let kinds = ["walker", "runner"];
    let mut scenarios = 0;
    for seed in (0..48_u64).chain([62, 154, 524, 754, 818, 909, 928]) {
        let mut app = scenario("open_16x16");
        let mut rng = SplitMix64(seed * 7919 + 1);
        let density = rng.below(35);
        for y in 0..16 {
            for x in 0..16 {
                if rng.below(100) < density {
                    set_rock(&mut app, [(x, y)]);
                }
            }
        }
        let count = 5 + rng.below(60) as usize;
        let mut ids = Vec::new();
        for i in 0..count {
            for _ in 0..50 {
                let cell = CellPos::new(rng.below(16) as i32, rng.below(16) as i32);
                if let Ok(id) = spawn_unit(app.world_mut(), HUMAN, kinds[i % kinds.len()], cell) {
                    ids.push(id);
                    break;
                }
            }
        }
        if ids.is_empty() {
            continue;
        }
        scenarios += 1;
        let context = format!("seed {seed}");
        for tick_index in 0..1_500 {
            if tick_index % (5 + seed % 40) == 0 {
                let k = 1 + rng.below(ids.len() as u64) as usize;
                let selection: BTreeSet<SimId> = (0..k)
                    .map(|_| ids[rng.below(ids.len() as u64) as usize])
                    .collect();
                let target = CellPos::new(rng.below(16) as i32, rng.below(16) as i32);
                move_to(
                    &mut app,
                    &selection.into_iter().collect::<Vec<_>>(),
                    target.x,
                    target.y,
                );
            }
            for event in tick(&mut app) {
                assert!(
                    !matches!(event, SimEvent::CommandRejected { .. }),
                    "{context}: {event:?}"
                );
            }
            assert_i6(&mut app, &format!("{context}, tick {tick_index}"));
        }
        run_until_all_orders_end(&mut app, TERMINATION_LIMIT, &context);
    }
    assert!(scenarios >= 50, "{scenarios} scenarios");
}

#[test]
fn ticks_spent_waiting_for_the_pathfinder_do_not_count_as_stalled() {
    // M2 review 1, P2-2: a unit whose path request waits in the queue is not blocked by other
    // units, so its wait must not bring it closer to `MoveFailed { Blocked }`.
    let mut app = scenario("arena_32x32");
    let group = spawn_block(&mut app, HUMAN, 50, (1, 1), 10);
    move_to(&mut app, &group, 25, 25);
    for round in 1..=3 {
        tick(&mut app);
        let states = units(&mut app);
        let mut waiting = 0;
        for id in &group {
            let order = states[id].orders.current.as_ref().expect("order");
            if matches!(order.state, PathState::Requested { .. }) {
                waiting += 1;
                assert_eq!(order.stalled_ticks, 0, "tick {round}: {id:?} {order:?}");
            }
        }
        assert_eq!(waiting, 50 - round * PATH_REQUESTS_PER_TICK, "tick {round}");
    }
}

#[test]
fn a_completion_says_whether_the_unit_stopped_short_of_its_destination() {
    // M2 review 1, P2-1: the sealed room of three cells, A at (7, 3) ordered to (7, 4) and B at
    // (7, 4) ordered to (7, 3). B steps aside to (7, 5), A reaches its destination, and B finds
    // its way closed by A, which now stands still: B stops in (7, 5), farther from its
    // destination than where it started, and must not report an arrival.
    let mut app = scenario("open_16x16");
    set_rock(
        &mut app,
        (6..=8)
            .flat_map(|x| (2..=6).map(move |y| (x, y)))
            .filter(|&(x, y)| !(x == 7 && (3..=5).contains(&y))),
    );
    let a = spawn(&mut app, HUMAN, "walker", 7, 3);
    let b = spawn(&mut app, HUMAN, "walker", 7, 4);
    order_swap(&mut app, a, b);
    let ended = run_until_all_orders_end(&mut app, TERMINATION_LIMIT, "P2-1");
    assert_eq!(cell_of(&mut app, a), CellPos::new(7, 4));
    assert_eq!(cell_of(&mut app, b), CellPos::new(7, 5));
    assert_eq!(
        ended[&a].1,
        SimEvent::MoveCompleted {
            unit: a,
            retargeted: false
        }
    );
    assert_eq!(
        ended[&b].1,
        SimEvent::MoveCompleted {
            unit: b,
            retargeted: true
        }
    );
}

#[test]
fn validation_checks_each_named_unit_in_turn() {
    // M2 review 1, P2-4 (docs/DECISIONS.md D19 item 3): after the count checks, each named unit
    // is checked in the order given, for repetition, then existence, then ownership; the first
    // failure names the command's reason.
    let target = CellPos::new(8, 8);
    // An unknown unit named before a repeated one.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![SimId(999), ids[0], ids[0]],
            target,
        },
        |_| RejectReason::UnknownUnit(SimId(999)),
    );
    // A repeated unit named before an unknown one.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[0], ids[0], SimId(999)],
            target,
        },
        |ids| RejectReason::DuplicateUnit(ids[0]),
    );
    // A foreign unit named before an unknown one.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Stop {
            units: vec![ids[2], SimId(999)],
        },
        |ids| RejectReason::NotOwned(ids[2]),
    );
    // Units are checked before the target.
    assert_rejected_without_state_change(
        two_units_each,
        HUMAN,
        |ids| Command::Move {
            units: vec![ids[2]],
            target: CellPos::new(-1, 0),
        },
        |ids| RejectReason::NotOwned(ids[2]),
    );
}

// --- Boundary speeds (M2 review 2) ------------------------------------------------------------

/// The fixture unit types at the two ends of the speed range the units file admits: "crawler"
/// (speed 1) and "sprinter" (speed `MAX_UNIT_SPEED`).
fn boundary_scenario(map: &str) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(load_grid(map));
    app.world_mut().insert_resource(
        load_unit_definitions(fixture("data/units_boundary_speeds.ron"))
            .unwrap_or_else(|error| panic!("{error}")),
    );
    app
}

const BOUNDARY_KINDS: [(&str, u16); 2] = [("crawler", 1), ("sprinter", MAX_UNIT_SPEED)];

/// Ticks within which the termination scenarios must end at `speed`. The rule that ends an order
/// counts steps and waits behind units that stand still, not elapsed ticks, so the time it takes
/// grows with the time a step takes: `TERMINATION_LIMIT` (which holds for the walker, speed 32)
/// scaled by 32 / speed for slower units.
fn termination_limit(speed: u16) -> u32 {
    TERMINATION_LIMIT * (32 / u32::from(speed)).max(1)
}

#[test]
fn the_boundary_speed_fixture_holds_the_slowest_and_fastest_speeds() {
    let app = boundary_scenario("open_16x16");
    let definitions = app.world().resource::<Definitions>();
    let speeds: Vec<(&str, u16)> = definitions
        .units()
        .iter()
        .map(|unit| (unit.id.as_str(), unit.speed))
        .collect();
    assert_eq!(speeds, BOUNDARY_KINDS);
    assert_eq!(MAX_UNIT_SPEED, 128);
}

#[test]
fn a_unit_of_either_boundary_speed_crosses_an_open_map_and_arrives_on_time() {
    // M2 review 2, P0-1: a unit of speed 1 failed every move longer than one cell with
    // `MoveFailed { Blocked }` on an empty map, because the ticks of its own steps (256 and 362
    // ticks long at speed 1) counted toward `MAX_STALLED_TICKS` = 200. Alone on the open map,
    // a unit of either boundary speed must arrive at its destination exactly when it has covered
    // the length of its path at `speed` subcells per tick, with no stalled tick on the way.
    let orders = [
        ((1, 1), (15, 1)),
        ((1, 1), (1, 15)),
        ((1, 1), (15, 15)),
        ((1, 1), (11, 8)),
        ((14, 14), (1, 8)),
        ((14, 1), (1, 14)),
        ((8, 8), (8, 0)),
        ((3, 3), (5, 3)),
        ((3, 3), (5, 5)),
    ];
    for (kind, speed) in BOUNDARY_KINDS {
        for (from, to) in orders {
            let context = format!("{kind} {from:?} -> {to:?}");
            let mut app = boundary_scenario("open_16x16");
            let unit = spawn(&mut app, HUMAN, kind, from.0, from.1);
            move_to(&mut app, &[unit], to.0, to.1);
            let (dx, dy) = (
                (to.0 - from.0).unsigned_abs(),
                (to.1 - from.1).unsigned_abs(),
            );
            let diagonal = dx.min(dy);
            let length = 256 * (dx.max(dy) - diagonal) + 362 * diagonal;
            let arrival = length.div_ceil(u32::from(speed));
            let start = SubPos::centre_of(CellPos::new(from.0, from.1));
            for elapsed in 1..=arrival {
                let events = tick(&mut app);
                let state = unit_state_after(&mut app, unit);
                if elapsed < arrival {
                    assert!(events.is_empty(), "{context}, tick {elapsed}: {events:?}");
                    let order = state.orders.current.expect("the order is live");
                    assert_eq!(order.stalled_ticks, 0, "{context}, tick {elapsed}");
                    if dy == 0 {
                        // Straight east: exactly `speed` subcells per tick.
                        let travelled = elapsed * u32::from(speed);
                        assert_eq!(
                            state.position,
                            SubPos::new(start.x + travelled as i32, start.y),
                            "{context}, tick {elapsed}"
                        );
                    }
                } else {
                    assert_eq!(
                        events,
                        vec![SimEvent::MoveCompleted {
                            unit,
                            retargeted: false
                        }],
                        "{context}, tick {elapsed}"
                    );
                    assert_eq!(state.mobile.cell, CellPos::new(to.0, to.1), "{context}");
                    assert_eq!(state.mobile.step, None, "{context}");
                    assert_eq!(
                        state.position,
                        SubPos::centre_of(CellPos::new(to.0, to.1)),
                        "{context}"
                    );
                }
            }
        }
    }
}

/// Runs until every unit of `group` has ended its order, checking I6 and calling `each_tick`
/// with the units' states on every tick; panics on any event other than a completion and returns
/// each unit's completion.
fn run_until_each_completes(
    app: &mut App,
    group: &[SimId],
    limit: u32,
    context: &str,
    mut each_tick: impl FnMut(u32, &BTreeMap<SimId, UnitState>),
) -> BTreeMap<SimId, SimEvent> {
    let mut ended = BTreeMap::new();
    for elapsed in 1..=limit {
        for event in tick(app) {
            match event {
                SimEvent::MoveCompleted { unit, .. } => {
                    ended.insert(unit, event);
                }
                other => panic!("{context}, tick {elapsed}: unexpected {other:?}"),
            }
        }
        assert_i6(app, &format!("{context}, tick {elapsed}"));
        each_tick(elapsed, &units(app));
        if ended.len() == group.len() {
            return ended;
        }
    }
    panic!("{context}: only {ended:?} of {group:?} completed within {limit} ticks");
}

#[test]
fn units_queued_behind_a_unit_of_the_slowest_speed_follow_it_to_their_destinations() {
    // M2 review 2, P0-1, the class: a wait behind a unit that is moving on lasts as long as that
    // unit's step, up to 362 ticks at speed 1. Such waits counted toward `MAX_STALLED_TICKS`, and
    // the paths a waiting unit asks for every `REPATH_AFTER_BLOCKED_TICKS` ticks, which only
    // confirmed its way, counted toward `MAX_PATHS_WITHOUT_PROGRESS`, so units queued behind a
    // slow unit gave up as `Blocked` while the queue advanced. A convoy of six in a corridor one
    // cell wide (y = 5), led by a crawler, followed by crawlers or by sprinters, each ordered to
    // its own cell at the far end: every unit must arrive, and no wait counts as stalled.
    for follower in ["crawler", "sprinter"] {
        let context = format!("convoy of crawler and {follower}s");
        let mut app = boundary_scenario("open_16x16");
        set_rock(&mut app, (0..16).flat_map(|x| [(x, 4), (x, 6)]));
        let convoy: Vec<SimId> = (0..6)
            .map(|i| {
                let kind = if i == 0 { "crawler" } else { follower };
                spawn(&mut app, HUMAN, kind, 5 - i, 5)
            })
            .collect();
        for (i, &unit) in convoy.iter().enumerate() {
            move_to(&mut app, &[unit], 15 - i as i32, 5);
        }
        let ended = run_until_each_completes(
            &mut app,
            &convoy,
            termination_limit(1),
            &context,
            |elapsed, states| {
                for (id, state) in states {
                    if let Some(order) = &state.orders.current {
                        assert_eq!(order.stalled_ticks, 0, "{context}, tick {elapsed}: {id:?}");
                        assert!(
                            order.paths_since_progress <= 1,
                            "{context}, tick {elapsed}: {id:?} {order:?}"
                        );
                    }
                }
            },
        );
        for (i, &unit) in convoy.iter().enumerate() {
            assert_eq!(
                ended[&unit],
                SimEvent::MoveCompleted {
                    unit,
                    retargeted: false
                },
                "{context}"
            );
            assert_eq!(cell_of(&mut app, unit), CellPos::new(15 - i as i32, 5));
        }
    }

    // On open ground, a sprinter right behind a crawler on the same diagonal waits behind it
    // for each of its steps (362 ticks).
    let mut app = boundary_scenario("open_16x16");
    let leader = spawn(&mut app, HUMAN, "crawler", 2, 2);
    let follower = spawn(&mut app, HUMAN, "sprinter", 1, 1);
    move_to(&mut app, &[leader], 14, 14);
    move_to(&mut app, &[follower], 13, 13);
    let ended = run_until_each_completes(
        &mut app,
        &[leader, follower],
        termination_limit(1),
        "diagonal",
        |elapsed, states| {
            for (id, state) in states {
                if let Some(order) = &state.orders.current {
                    assert_eq!(order.stalled_ticks, 0, "diagonal, tick {elapsed}: {id:?}");
                }
            }
        },
    );
    assert_eq!(ended.len(), 2);
    assert_eq!(cell_of(&mut app, leader), CellPos::new(14, 14));
    assert_eq!(cell_of(&mut app, follower), CellPos::new(13, 13));
}

#[test]
fn orders_end_at_both_boundary_speeds_when_units_keep_giving_way() {
    // M2 review 1, P1-1, at the boundary speeds: the sealed room of three cells and the dead
    // end closed by idle units of `an_order_ends_when_*`. Every order must end, as at speed 32:
    // in the sealed room one unit gives up as `Blocked` and the other stops short of its
    // destination. Before the fix of M2 review 2 P0-1, both crawlers of the sealed room ended as
    // `Blocked` after their first step.
    for (kind, speed) in BOUNDARY_KINDS {
        let limit = termination_limit(speed);
        let context = format!("{kind}: sealed room of three");
        let mut app = boundary_scenario("open_16x16");
        set_rock(
            &mut app,
            (6..=8)
                .flat_map(|x| (2..=6).map(move |y| (x, y)))
                .filter(|&(x, y)| !(x == 7 && (3..=5).contains(&y))),
        );
        let a = spawn(&mut app, HUMAN, kind, 7, 3);
        let b = spawn(&mut app, HUMAN, kind, 7, 5);
        order_swap(&mut app, a, b);
        let ended = run_until_all_orders_end(&mut app, limit, &context);
        assert_eq!(ended.len(), 2, "{context}: {ended:?}");
        let failed: Vec<SimId> = ended
            .iter()
            .filter(|(_, (_, event))| {
                matches!(
                    event,
                    SimEvent::MoveFailed {
                        reason: MoveFailure::Blocked,
                        ..
                    }
                )
            })
            .map(|(&id, _)| id)
            .collect();
        assert_eq!(failed.len(), 1, "{context}: {ended:?}");
        for (&id, (_, event)) in &ended {
            if !failed.contains(&id) {
                assert_eq!(
                    *event,
                    SimEvent::MoveCompleted {
                        unit: id,
                        retargeted: true
                    },
                    "{context}"
                );
            }
        }

        for (a_cell, b_cell) in [((7, 2), (7, 6)), ((7, 4), (7, 6)), ((7, 3), (7, 5))] {
            let context = format!("{kind}: dead end, {a_cell:?} <-> {b_cell:?}");
            let mut app = boundary_scenario("open_16x16");
            set_rock(
                &mut app,
                (1..=6).flat_map(|y| [(6, y), (8, y)]).chain([(7, 1)]),
            );
            for x in 6..=8 {
                spawn(&mut app, ENEMY, kind, x, 7);
            }
            let a = spawn(&mut app, HUMAN, kind, a_cell.0, a_cell.1);
            let b = spawn(&mut app, HUMAN, kind, b_cell.0, b_cell.1);
            order_swap(&mut app, a, b);
            let ended = run_until_all_orders_end(&mut app, limit, &context);
            assert_eq!(ended.len(), 2, "{context}: {ended:?}");
            for (&id, (_, event)) in &ended {
                if let SimEvent::MoveCompleted {
                    retargeted: false, ..
                } = event
                {
                    let destination = if id == a { b_cell } else { a_cell };
                    assert_eq!(
                        cell_of(&mut app, id),
                        CellPos::new(destination.0, destination.1),
                        "{context}"
                    );
                }
            }
        }
    }
}

#[test]
fn fifty_units_of_the_boundary_speeds_reach_a_clicked_destination() {
    // The acceptance scenario (PLAN.md section 7, M2) with crawlers and sprinters taking turns:
    // every unit must arrive (no `Blocked`), I6 must hold on every tick, and the group must fill
    // the cells around the target as at speed 32. Before the fix of M2 review 2 P0-1, 41 of the
    // 50 units ended as `Blocked` on the arena.
    let cases = [
        ("arena_32x32", (1, 1), CellPos::new(24, 24)),
        ("wall_32x32", (1, 10), CellPos::new(24, 16)),
    ];
    for (map, origin, target) in cases {
        let mut app = boundary_scenario(map);
        let group: Vec<SimId> = (0..50)
            .map(|i| {
                let kind = BOUNDARY_KINDS[i % 2].0;
                let i = i as i32;
                spawn(&mut app, HUMAN, kind, origin.0 + i % 10, origin.1 + i / 10)
            })
            .collect();
        let context = format!("{map}, {origin:?} -> {target:?}, crawlers and sprinters");
        let took = run_group_to(&mut app, &group, target, termination_limit(1), &context);
        let states = units(&mut app);
        let cells: BTreeSet<CellPos> = group.iter().map(|id| states[id].mobile.cell).collect();
        assert_eq!(cells.len(), 50, "{context}");
        let grid = app.world().resource::<Grid>().clone();
        let expected: BTreeSet<CellPos> = destination_cells(&grid, target, 50, |_| false)
            .into_iter()
            .collect();
        let radius = expected
            .iter()
            .map(|&cell| sim::octile_distance(cell, target))
            .max()
            .expect("50 cells");
        for &cell in &cells {
            assert!(
                sim::octile_distance(cell, target) <= radius + 2 * ORTHOGONAL_COST,
                "{context}: {cell:?} is too far from the target"
            );
        }
        let at_assigned = cells.intersection(&expected).count();
        assert!(
            at_assigned >= 35,
            "{context}: only {at_assigned} units at assigned cells"
        );
        println!("{context}: {took} ticks, {at_assigned} of 50 at the assigned cells");
    }
}
