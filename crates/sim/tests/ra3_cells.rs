//! RA3: infantry sub cells, the amendment of invariant I6 (docs/RA2_PARITY_PLAN.md section 5,
//! RA3 item 8; docs/DECISIONS.md R6; PLAN.md section 6 with the note under its table).
//!
//! I6' says: a surface cell holds at most one vehicle or at most three infantry and never both;
//! an air cell holds at most one aircraft; no unit occupies a cell covered by a building. The
//! tests drive the production path (`CommandQueue`, `SimSchedule`) on the RA3 fixture
//! (`tests/fixtures/data/ra3`), whose last seven unit types have the traits of the arsenal's
//! schema version 4: the recruit, the trooper and the hazmat are infantry on foot, the jumper is
//! infantry that flies, the truck is a vehicle, and the roller and the brute are crushers
//! (`ra3_crush.rs` has the tests of crushing). Fixture data, independent of the shipped balance
//! (invariant I10). `ra3_common::assert_cells` checks I6' after a tick.

mod ra3_common;

use std::collections::{BTreeMap, BTreeSet};

use bevy_app::App;
use ra3_common::*;
use sim::{
    CELL_ROOM, CellPos, Command, DIAGONAL_STEP, Domain, Grid, MoveFailure, ORTHOGONAL_STEP, Orders,
    ProductionItem, SPOT_OFFSETS, SPOTS_PER_CELL, SimEvent, SimId, SpawnError, SubPos,
    cell_invariant_violations, spawn_unit, splash_damage, state_hash, stray_claims,
};

fn cell(x: i32, y: i32) -> CellPos {
    CellPos::new(x, y)
}

fn grid(app: &App) -> &Grid {
    app.world().resource::<Grid>()
}

/// The cells of the surface that the unit `id` holds, by the grid.
fn spot(app: &App, x: i32, y: i32, id: SimId) -> Option<u8> {
    grid(app).spot(cell(x, y), id)
}

/// The units for which `events` report a completed move, with whether they were retargeted.
fn completed(events: &[SimEvent]) -> BTreeMap<SimId, bool> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::MoveCompleted { unit, retargeted } => Some((*unit, *retargeted)),
            _ => None,
        })
        .collect()
}

/// Runs until every unit of `units` has completed a move, checking I6' after every tick;
/// returns for each whether it was retargeted, and the ticks taken.
fn run_until_arrived(app: &mut App, units: &[SimId], limit: u32) -> (BTreeMap<SimId, bool>, u32) {
    let mut arrived = BTreeMap::new();
    for index in 0..limit {
        let events = tick(app, Vec::new());
        assert_cells(app, &format!("tick {index}"));
        arrived.extend(completed(&events));
        for event in &events {
            if let SimEvent::MoveFailed { unit, reason } = event {
                panic!("tick {index}: the move of {unit:?} failed: {reason:?}");
            }
        }
        if units.iter().all(|unit| arrived.contains_key(unit)) {
            return (arrived, index + 1);
        }
    }
    panic!("after {limit} ticks only {arrived:?} of {units:?} arrived");
}

/// Makes the cells (`x0`..=`x1`, `y0`..=`y1`) rock.
fn rock(app: &mut App, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    let mut grid = app.world_mut().resource_mut::<Grid>();
    for y in y0..=y1 {
        for x in x0..=x1 {
            grid.set_terrain(
                cell(x, y),
                sim::Cell {
                    passable: false,
                    water: false,
                    buildable: false,
                    ore: 0,
                },
            );
        }
    }
}

// --- The fixture and the spots ----------------------------------------------------------------

#[test]
fn the_spots_are_three_symmetric_apart_and_inside_the_cell() {
    assert_eq!(SPOTS_PER_CELL, 3);
    assert_eq!(usize::from(CELL_ROOM), SPOTS_PER_CELL);
    let [top, right, left] = SPOT_OFFSETS;
    // Around the centre: the offsets add up to nothing.
    assert_eq!(top.0 + right.0 + left.0, 0);
    assert_eq!(top.1 + right.1 + left.1, 0);
    // On screen a point of the cell (dx, dy) lies (dx - dy) to the right and (dx + dy) down
    // (PLAN.md section 5, equation 7): spot 0 is on the vertical axis above the centre, and
    // spots 1 and 2 are mirror images below it, at one height.
    let across = |(dx, dy): (i32, i32)| dx - dy;
    let down = |(dx, dy): (i32, i32)| dx + dy;
    assert_eq!(across(top), 0);
    assert!(down(top) < 0);
    assert_eq!(across(right), -across(left));
    assert!(across(right) > 0);
    assert_eq!(down(right), down(left));
    assert!(down(right) > 0);
    assert_eq!(right, (left.1, left.0));
    for (index, &(dx, dy)) in SPOT_OFFSETS.iter().enumerate() {
        // Well inside the cell: at least 32 subcells from every edge.
        assert!(dx.abs() <= 96 && dy.abs() <= 96, "spot {index}");
        for &(ox, oy) in &SPOT_OFFSETS[index + 1..] {
            // Visibly apart: at least half a cell between any two.
            let squared = (dx - ox).pow(2) + (dy - oy).pow(2);
            assert!(squared >= 128 * 128, "spot {index}: {squared}");
        }
        let at = SubPos::spot_of(cell(7, 9), index as u8);
        assert_eq!(at, SubPos::new(7 * 256 + 128 + dx, 9 * 256 + 128 + dy));
        assert_eq!(at.cell(), cell(7, 9));
    }
    // A spot index beyond the last counts from the first again (no panic, invariant I8).
    assert_eq!(
        SubPos::spot_of(cell(1, 1), 3),
        SubPos::spot_of(cell(1, 1), 0)
    );
}

#[test]
fn three_soldiers_share_a_cell_each_at_its_spot_and_nothing_else_fits() {
    let mut app = world();
    let home = cell(10, 10);
    // Of two players: enemies share a cell too.
    let soldiers = [
        spawn(&mut app, P0, "recruit", 10, 10),
        spawn(&mut app, P1, "recruit", 10, 10),
        spawn(&mut app, P0, "hazmat", 10, 10),
    ];
    for (index, &soldier) in soldiers.iter().enumerate() {
        assert_eq!(
            position(&mut app, soldier),
            SubPos::spot_of(home, index as u8),
            "soldier {index}"
        );
        assert_eq!(spot(&app, 10, 10, soldier), Some(index as u8));
        assert_eq!(mobile(&mut app, soldier).cell, home);
        assert!(grid(&app).is_on_foot(soldier));
    }
    assert_eq!(grid(&app).holders(home), soldiers.map(Some));
    assert_eq!(grid(&app).occupant(home), Some(soldiers[0]));
    assert!(!grid(&app).has_room(home, true));
    assert!(!grid(&app).has_room(home, false));
    assert_eq!(grid(&app).room(home, true, |_| true), 0);
    // A fourth soldier and a vehicle do not fit, whoever owns them; a crusher is not spawned
    // onto infantry either.
    for kind in ["recruit", "trooper", "hazmat", "truck", "roller"] {
        for owner in [P0, P1] {
            assert_eq!(
                spawn_unit(app.world_mut(), owner, kind, home),
                Err(SpawnError::Occupied {
                    cell: home,
                    by: soldiers[0]
                }),
                "{kind}"
            );
        }
    }
    // The air above the cell is another layer: infantry that flies holds it whole, and stands
    // over the centre.
    let jumper = spawn(&mut app, P1, "jumper", 10, 10);
    assert_eq!(position(&mut app, jumper), SubPos::centre_of(home));
    assert!(!grid(&app).is_on_foot(jumper));
    assert_eq!(
        grid(&app).holders_for(home, Domain::Air),
        [Some(jumper), None, None]
    );
    assert_eq!(
        spawn_unit(app.world_mut(), P0, "jumper", home),
        Err(SpawnError::Occupied {
            cell: home,
            by: jumper
        })
    );
    assert_cells(&mut app, "after the spawns");
    // Nothing moved or changed over time.
    ticks(&mut app, 20);
    assert_cells(&mut app, "after 20 ticks");
    assert_eq!(grid(&app).holders(home), soldiers.map(Some));
}

#[test]
fn a_cell_holds_a_vehicle_or_infantry_and_never_both() {
    let mut app = world();
    let truck = spawn(&mut app, P0, "truck", 5, 5);
    assert_eq!(position(&mut app, truck), SubPos::centre_of(cell(5, 5)));
    assert_eq!(grid(&app).holders(cell(5, 5)), [Some(truck), None, None]);
    assert_eq!(spot(&app, 5, 5, truck), None);
    assert!(!grid(&app).has_room(cell(5, 5), true));
    for kind in ["recruit", "truck"] {
        assert_eq!(
            spawn_unit(app.world_mut(), P0, kind, cell(5, 5)),
            Err(SpawnError::Occupied {
                cell: cell(5, 5),
                by: truck
            }),
            "{kind}"
        );
    }
    // One soldier leaves room for two more, and for no vehicle.
    let soldier = spawn(&mut app, P0, "recruit", 6, 6);
    assert_eq!(grid(&app).room(cell(6, 6), true, |_| true), 2);
    assert!(grid(&app).has_room(cell(6, 6), true));
    assert!(!grid(&app).has_room(cell(6, 6), false));
    assert_eq!(
        spawn_unit(app.world_mut(), P0, "truck", cell(6, 6)),
        Err(SpawnError::Occupied {
            cell: cell(6, 6),
            by: soldier
        })
    );
    // A unit of a type without the infantry trait holds its cell whole, like a vehicle: the
    // rifleman of RA3-A.
    let rifleman = spawn(&mut app, P0, "rifleman", 7, 7);
    assert_eq!(position(&mut app, rifleman), SubPos::centre_of(cell(7, 7)));
    assert_eq!(
        spawn_unit(app.world_mut(), P0, "recruit", cell(7, 7)),
        Err(SpawnError::Occupied {
            cell: cell(7, 7),
            by: rifleman
        })
    );
    assert_cells(&mut app, "after the spawns");
}

// --- Walking ----------------------------------------------------------------------------------

/// The ticks each of `units` takes to complete its move, counted from the tick in which
/// `commands` are given.
fn arrival_ticks(
    app: &mut App,
    mut commands: Vec<sim::IssuedCommand>,
    units: &[SimId],
    limit: u32,
) -> BTreeMap<SimId, u32> {
    let mut arrival = BTreeMap::new();
    for index in 1..=limit {
        let events = tick(app, std::mem::take(&mut commands));
        assert_cells(app, &format!("tick {index}"));
        for unit in completed(&events).into_keys() {
            arrival.entry(unit).or_insert(index);
        }
        if units.iter().all(|unit| arrival.contains_key(unit)) {
            break;
        }
    }
    arrival
}

#[test]
fn a_soldier_walks_from_spot_to_spot_in_the_ticks_of_a_step_between_centres() {
    let mut app = world();
    // Both have 16 subcells a tick; the engineer of this fixture is not on foot.
    let soldier = spawn(&mut app, P0, "recruit", 5, 10);
    let rider = spawn(&mut app, P0, "engineer", 5, 14);
    let speed = |app: &mut App, id| u32::from(mobile(app, id).speed);
    assert_eq!(speed(&mut app, soldier), 16);
    assert_eq!(speed(&mut app, rider), 16);

    // Four cells east: a straight walk at spot 0 all the way. Ticks are counted from the tick
    // of the order, in which the walk begins.
    let mut commands = vec![move_to(P0, &[soldier], 9, 10), move_to(P0, &[rider], 9, 14)];
    let mut arrival = BTreeMap::new();
    for index in 1..200 {
        let events = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        for unit in completed(&events).into_keys() {
            arrival.entry(unit).or_insert(index);
        }
        // Step for step beside the rider, two rows above and at its spot: the same share of
        // the way, measured from spot 0 instead of the centre.
        let (walking, riding) = (mobile(&mut app, soldier), mobile(&mut app, rider));
        assert_eq!(walking.cell.x, riding.cell.x, "tick {index}");
        assert_eq!(
            walking
                .step
                .map(|step| (step.to.x, step.progress, step.length)),
            riding
                .step
                .map(|step| (step.to.x, step.progress, step.length)),
            "tick {index}"
        );
        let (at, beside) = (position(&mut app, soldier), position(&mut app, rider));
        let (dx, dy) = SPOT_OFFSETS[0];
        assert_eq!(
            at,
            SubPos::new(beside.x + dx, beside.y - 4 * 256 + dy),
            "tick {index}"
        );
        if arrival.len() == 2 {
            break;
        }
    }
    // 4 cells of 256 subcells at 16 a tick.
    assert_eq!(arrival[&soldier], 4 * u32::from(ORTHOGONAL_STEP) / 16);
    assert_eq!(arrival[&soldier], arrival[&rider]);
    assert_eq!(position(&mut app, soldier), SubPos::spot_of(cell(9, 10), 0));
    assert_eq!(spot(&app, 9, 10, soldier), Some(0));
    assert_eq!(spot(&app, 5, 10, soldier), None, "the cell it left is free");

    // Three cells along the diagonal: a diagonal step is 362 subcells long for both.
    let commands = vec![
        move_to(P0, &[soldier], 12, 13),
        move_to(P0, &[rider], 12, 17),
    ];
    let arrival = arrival_ticks(&mut app, commands, &[soldier, rider], 200);
    assert_eq!(arrival[&soldier], arrival[&rider]);
    assert_eq!(
        arrival[&soldier],
        (3 * u32::from(DIAGONAL_STEP)).div_ceil(16),
        "three diagonal steps"
    );
    assert_eq!(
        position(&mut app, soldier),
        SubPos::spot_of(cell(12, 13), 0)
    );
}

#[test]
fn a_soldier_keeps_its_spot_from_cell_to_cell_and_takes_the_first_free_one_otherwise() {
    let mut app = world();
    let first = spawn(&mut app, P0, "recruit", 5, 10);
    let second = spawn(&mut app, P0, "recruit", 5, 10);
    let third = spawn(&mut app, P0, "recruit", 6, 10);
    assert_eq!(spot(&app, 5, 10, first), Some(0));
    assert_eq!(spot(&app, 5, 10, second), Some(1));
    assert_eq!(spot(&app, 6, 10, third), Some(0));
    // The second keeps spot 1, which is free in the next cell.
    tick(&mut app, vec![move_to(P0, &[second], 6, 10)]);
    let (arrived, _) = run_until_arrived(&mut app, &[second], 100);
    assert!(!arrived[&second], "its own destination");
    assert_eq!(spot(&app, 6, 10, second), Some(1));
    assert_eq!(position(&mut app, second), SubPos::spot_of(cell(6, 10), 1));
    // The first had spot 0, which the third holds there: it takes the first free one, spot 2.
    tick(&mut app, vec![move_to(P0, &[first], 6, 10)]);
    let (arrived, _) = run_until_arrived(&mut app, &[first], 100);
    assert!(!arrived[&first]);
    assert_eq!(spot(&app, 6, 10, first), Some(2));
    assert_eq!(position(&mut app, first), SubPos::spot_of(cell(6, 10), 2));
    assert_eq!(
        grid(&app).holders(cell(6, 10)),
        [Some(third), Some(second), Some(first)]
    );
    assert_eq!(grid(&app).holders(cell(5, 10)), [None; 3]);
}

#[test]
fn a_fourth_soldier_waits_for_a_spot_while_the_cell_is_full() {
    let mut app = world();
    let ahead = [
        spawn(&mut app, P0, "recruit", 6, 10),
        spawn(&mut app, P0, "recruit", 6, 10),
        spawn(&mut app, P0, "recruit", 6, 10),
    ];
    let fourth = spawn(&mut app, P0, "recruit", 5, 10);
    // The three walk east and the fourth behind them: the cell ahead of it stays full until
    // they have reached the next one.
    let mut commands = vec![move_to(P0, &ahead, 12, 10), move_to(P0, &[fourth], 11, 10)];
    let mut waited = 0;
    let mut arrived = BTreeMap::new();
    for index in 0..400 {
        let events = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        arrived.extend(completed(&events));
        let full = grid(&app)
            .holders(cell(6, 10))
            .iter()
            .all(|holder| holder.is_some_and(|holder| holder != fourth));
        let state = mobile(&mut app, fourth);
        if full {
            // It stands where it was, at its spot, with nothing reserved.
            assert_eq!(
                (state.cell, state.step),
                (cell(5, 10), None),
                "tick {index}"
            );
            assert_eq!(
                position(&mut app, fourth),
                SubPos::spot_of(cell(5, 10), 0),
                "tick {index}"
            );
            waited += 1;
        }
        if arrived.len() == 4 {
            break;
        }
    }
    // A step of the three takes 16 ticks: after 15 of them the cell is still full, and the
    // fourth waited through every one.
    assert_eq!(waited, 15, "ticks the fourth waited for the full cell");
    assert_eq!(arrived.len(), 4, "{arrived:?}");
    assert!(
        arrived.values().all(|retargeted| !retargeted),
        "{arrived:?}"
    );
    assert_eq!(grid(&app).holders(cell(12, 10)), ahead.map(Some));
    assert_eq!(grid(&app).holders(cell(11, 10)), [Some(fourth), None, None]);
}

#[test]
fn a_soldier_ordered_into_a_cell_full_of_standing_soldiers_stops_beside_it() {
    let mut app = world();
    let standing = [
        spawn(&mut app, P0, "recruit", 10, 10),
        spawn(&mut app, P1, "recruit", 10, 10),
        spawn(&mut app, P0, "recruit", 10, 10),
    ];
    let fourth = spawn(&mut app, P0, "recruit", 4, 10);
    tick(&mut app, vec![move_to(P0, &[fourth], 10, 10)]);
    let (arrived, _) = run_until_arrived(&mut app, &[fourth], 400);
    // The existing rule for a destination held by units that stand: the nearest free cell.
    let end = mobile(&mut app, fourth).cell;
    assert_ne!(end, cell(10, 10));
    assert!(
        (end.x - 10).abs() <= 1 && (end.y - 10).abs() <= 1,
        "it stops beside the full cell: {end:?}"
    );
    assert!(arrived.contains_key(&fourth));
    assert_eq!(grid(&app).holders(cell(10, 10)), standing.map(Some));
}

#[test]
fn a_group_move_fills_cells_with_three_soldiers_and_gives_each_vehicle_its_own() {
    let mut app = world();
    let soldiers: Vec<SimId> = (0..7)
        .map(|index| spawn(&mut app, P0, "recruit", 3 + index, 20))
        .collect();
    let trucks = [
        spawn(&mut app, P0, "truck", 3, 22),
        spawn(&mut app, P0, "truck", 4, 22),
    ];
    let group: Vec<SimId> = soldiers.iter().chain(&trucks).copied().collect();
    tick(&mut app, vec![move_to(P0, &group, 20, 20)]);
    // Every unit has the destination the command gave it: the vehicles the two cells nearest
    // to the target, the soldiers the next cells, three to a cell.
    let destination = |app: &mut App, id: SimId| {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &Orders)>();
        query
            .iter(world)
            .find(|(other, _)| **other == id)
            .and_then(|(_, orders)| orders.current.as_ref().map(|order| order.destination))
            .unwrap_or_else(|| panic!("{id:?} has no order"))
    };
    let truck_cells: BTreeSet<CellPos> =
        trucks.iter().map(|&id| destination(&mut app, id)).collect();
    assert_eq!(truck_cells, BTreeSet::from([cell(20, 20), cell(20, 19)]));
    let mut handed: BTreeMap<CellPos, usize> = BTreeMap::new();
    for &soldier in &soldiers {
        *handed.entry(destination(&mut app, soldier)).or_default() += 1;
    }
    assert_eq!(
        handed,
        BTreeMap::from([(cell(19, 20), 3), (cell(21, 20), 3), (cell(20, 21), 1)])
    );
    let (arrived, _) = run_until_arrived(&mut app, &group, 800);
    assert!(
        arrived.values().all(|retargeted| !retargeted),
        "{arrived:?}"
    );
    for (&at, &count) in &handed {
        let holders = grid(&app).holders(at);
        assert_eq!(holders.iter().flatten().count(), count, "{at:?}");
        assert!(holders.iter().flatten().all(|id| soldiers.contains(id)));
    }
    for &at in &truck_cells {
        let [Some(truck), None, None] = grid(&app).holders(at) else {
            panic!("{at:?} is not held by one vehicle");
        };
        assert!(trucks.contains(&truck));
    }
}

#[test]
fn a_group_move_counts_the_soldiers_that_stand_in_a_cell() {
    let mut app = world();
    // Two soldiers of another player stand idle at the target: one spot is left there.
    let standing = [
        spawn(&mut app, P1, "recruit", 20, 20),
        spawn(&mut app, P1, "recruit", 20, 20),
    ];
    let group: Vec<SimId> = (0..3)
        .map(|index| spawn(&mut app, P0, "recruit", 5 + index, 20))
        .collect();
    tick(&mut app, vec![move_to(P0, &group, 20, 20)]);
    let (arrived, _) = run_until_arrived(&mut app, &group, 600);
    assert!(
        arrived.values().all(|retargeted| !retargeted),
        "{arrived:?}"
    );
    let at_target = grid(&app).holders(cell(20, 20));
    assert_eq!(at_target[..2], standing.map(Some));
    assert!(at_target[2].is_some_and(|id| group.contains(&id)));
    // The other two share the next cell.
    assert_eq!(grid(&app).holders(cell(20, 19)).iter().flatten().count(), 2);
}

// --- A vehicle and a soldier keep each other out ------------------------------------------------

/// A corridor one cell wide along y = 10 from x = 3 to x = 17, closed at its east end: the only
/// way from (4, 10) to (16, 10) leads through (10, 10).
fn corridor(app: &mut App) {
    rock(app, (3, 9), (18, 9));
    rock(app, (3, 11), (18, 11));
    rock(app, (18, 10), (18, 10));
}

/// What happens when `mover` of P0, standing at (4, 10) in the corridor, is ordered to (16, 10)
/// with `blocker` of `blocker_owner` standing at (10, 10): whether it ever held that cell, how
/// its order ended, and whether the blocker is still there.
fn through_the_corridor(
    mover: &str,
    blocker: &str,
    blocker_owner: sim::PlayerId,
) -> (bool, SimEvent, bool) {
    let mut app = world();
    corridor(&mut app);
    let blocking = spawn(&mut app, blocker_owner, blocker, 10, 10);
    let moving = spawn(&mut app, P0, mover, 4, 10);
    tick(&mut app, vec![move_to(P0, &[moving], 16, 10)]);
    let mut entered = false;
    for index in 1..2_000 {
        let events = tick(&mut app, Vec::new());
        assert_cells(&mut app, &format!("{mover} and {blocker}, tick {index}"));
        let state = mobile(&mut app, moving);
        entered |=
            state.cell == cell(10, 10) || state.step.is_some_and(|step| step.to == cell(10, 10));
        let end = events.into_iter().find(|event| {
            matches!(
                event,
                SimEvent::MoveCompleted { unit, .. } | SimEvent::MoveFailed { unit, .. }
                    if *unit == moving
            )
        });
        if let Some(end) = end {
            let stands = hp(&mut app, blocking).is_some()
                && grid(&app).holders(cell(10, 10)).contains(&Some(blocking));
            return (entered, end, stands);
        }
    }
    panic!("{mover} and {blocker}: the order never ended");
}

#[test]
fn a_vehicle_cannot_enter_a_cell_with_a_friendly_soldier() {
    // Not a truck, and not a crusher either: its own soldier is never crushed.
    for mover in ["truck", "roller"] {
        let (entered, end, stands) = through_the_corridor(mover, "recruit", P0);
        assert!(!entered, "{mover} entered the soldier's cell");
        assert!(stands, "{mover}: the soldier is gone");
        assert!(
            matches!(
                end,
                SimEvent::MoveCompleted {
                    retargeted: true,
                    ..
                } | SimEvent::MoveFailed {
                    reason: MoveFailure::Blocked,
                    ..
                }
            ),
            "{mover}: {end:?}"
        );
    }
    // A vehicle that is no crusher does not enter a hostile soldier's cell either.
    let (entered, _, stands) = through_the_corridor("truck", "recruit", P1);
    assert!(!entered && stands);
}

#[test]
fn a_soldier_cannot_enter_a_cell_with_a_vehicle() {
    for (blocker, owner) in [("truck", P0), ("truck", P1), ("roller", P1)] {
        let (entered, end, stands) = through_the_corridor("recruit", blocker, owner);
        assert!(!entered, "the soldier entered the cell of the {blocker}");
        assert!(stands, "{blocker} is gone");
        assert!(
            matches!(
                end,
                SimEvent::MoveCompleted {
                    retargeted: true,
                    ..
                } | SimEvent::MoveFailed {
                    reason: MoveFailure::Blocked,
                    ..
                }
            ),
            "{blocker}: {end:?}"
        );
    }
}

#[test]
fn a_soldier_walks_through_a_cell_with_room_and_a_vehicle_waits_for_soldiers_that_move_on() {
    // One soldier stands in the corridor: another passes it, in the same cell.
    let (entered, end, stands) = through_the_corridor("recruit", "recruit", P1);
    assert!(entered && stands);
    assert!(
        matches!(
            end,
            SimEvent::MoveCompleted {
                retargeted: false,
                ..
            }
        ),
        "{end:?}"
    );
    // A truck behind three soldiers that walk the same way follows them through the corridor,
    // and never shares a cell with them.
    let mut app = world();
    corridor(&mut app);
    let soldiers = [
        spawn(&mut app, P0, "recruit", 6, 10),
        spawn(&mut app, P0, "recruit", 6, 10),
        spawn(&mut app, P0, "recruit", 6, 10),
    ];
    let truck = spawn(&mut app, P0, "truck", 4, 10);
    tick(
        &mut app,
        vec![
            move_to(P0, &soldiers, 16, 10),
            move_to(P0, &[truck], 14, 10),
        ],
    );
    let all: Vec<SimId> = soldiers.iter().copied().chain([truck]).collect();
    let (arrived, _) = run_until_arrived(&mut app, &all, 2_000);
    assert!(!arrived[&truck], "{arrived:?}");
    assert_eq!(grid(&app).holders(cell(14, 10)), [Some(truck), None, None]);
}

// --- Range and splash from the spot -----------------------------------------------------------

#[test]
fn range_is_measured_from_the_spot_a_soldier_stands_at() {
    let mut app = world();
    // The rifle reaches 1024 subcells. The truck stands 4 cells east of the troopers' cell:
    // 1024 from its centre, 1088 from spot 0, 944 from spot 1, 1040 from spot 2.
    let troopers = [
        spawn(&mut app, P0, "trooper", 10, 10),
        spawn(&mut app, P0, "trooper", 10, 10),
        spawn(&mut app, P0, "trooper", 10, 10),
    ];
    let truck = spawn(&mut app, P1, "truck", 14, 10);
    let rifle = weapon_kind(&app, "rifle");
    assert_eq!(defs(&app).weapon(rifle).expect("rifle").range, 1024);
    let target = position(&mut app, truck);
    let within: Vec<bool> = troopers
        .iter()
        .map(|&trooper| {
            let from = position(&mut app, trooper);
            sim::in_range(
                i64::from(target.x - from.x),
                i64::from(target.y - from.y),
                1024,
            )
        })
        .collect();
    assert_eq!(within, [false, true, false]);
    let events = ticks(&mut app, 60);
    assert!(
        shots(&events, troopers[0]).is_empty(),
        "spot 0 is out of range"
    );
    assert!(
        shots(&events, troopers[2]).is_empty(),
        "spot 2 is out of range"
    );
    let fired = shots(&events, troopers[1]);
    assert!(fired.len() >= 5, "{fired:?}");
    assert!(fired.iter().all(|&shot| shot == (truck, rifle)));
    // None of them moved: a unit without an order fires from where it stands.
    for (index, &trooper) in troopers.iter().enumerate() {
        assert_eq!(
            position(&mut app, trooper),
            SubPos::spot_of(cell(10, 10), index as u8)
        );
    }
}

#[test]
fn soldiers_ordered_to_attack_stop_where_their_spots_are_in_range() {
    let mut app = world();
    let troopers = [
        spawn(&mut app, P0, "trooper", 5, 20),
        spawn(&mut app, P0, "trooper", 5, 20),
        spawn(&mut app, P0, "trooper", 5, 20),
    ];
    let truck = spawn(&mut app, P1, "truck", 20, 20);
    // A scout of the troopers' owner beside the truck, so that they see it.
    spawn(&mut app, P0, "engineer", 22, 20);
    ticks(&mut app, 1);
    let commands = troopers
        .iter()
        .map(|&trooper| attack(P0, trooper, truck))
        .collect();
    let mut events = tick(&mut app, commands);
    // The rifle deals 2 to plate every 10 ticks: three of them need 500 ticks for the truck.
    for index in 0..1_500 {
        events.extend(tick(&mut app, Vec::new()));
        assert_cells(&mut app, &format!("tick {index}"));
        if hp(&mut app, truck).is_none() {
            break;
        }
    }
    assert_eq!(hp(&mut app, truck), None, "the truck was destroyed");
    // They set out for the nearest cell from whose three spots the truck is in range, which is
    // 3 cells west of it (4 cells west, spot 0 is 1088 subcells away and spot 2 1040), and all
    // three chose it. The trooper at spot 1 was in range a cell earlier (944) and stopped
    // there to fire; the other two went on and share the cell.
    let target = SubPos::centre_of(cell(20, 20));
    for (index, &trooper) in troopers.iter().enumerate() {
        assert!(
            !shots(&events, trooper).is_empty(),
            "{trooper:?} never fired"
        );
        let at = position(&mut app, trooper);
        assert!(
            sim::in_range(i64::from(target.x - at.x), i64::from(target.y - at.y), 1024),
            "trooper {index} stands out of range at {at:?}"
        );
    }
    assert_eq!(
        grid(&app).holders(cell(17, 20)),
        [Some(troopers[0]), None, Some(troopers[2])]
    );
    assert_eq!(
        grid(&app).holders(cell(16, 20)),
        [None, Some(troopers[1]), None]
    );
}

#[test]
fn splash_on_a_cell_with_three_soldiers_hits_each_by_its_own_distance() {
    let mut app = world();
    let soldiers = [
        spawn(&mut app, P1, "recruit", 20, 20),
        spawn(&mut app, P1, "recruit", 20, 20),
        spawn(&mut app, P1, "recruit", 20, 20),
    ];
    // The mortar bomb: 80 through the blast warhead, 2 cells of spread, 25 percent at its
    // edge, 100 percent against flesh. Its minimum range is 2 cells, its range 6.
    let mortar = spawn(&mut app, P0, "mortar", 15, 20);
    ticks(&mut app, 1);
    tick(&mut app, vec![attack(P0, mortar, soldiers[1])]);
    let mut hit = false;
    for _ in 0..60 {
        ticks(&mut app, 1);
        if hp(&mut app, soldiers[1]) != Some(100) {
            hit = true;
            break;
        }
    }
    assert!(hit, "the bomb never landed");
    // The impact is where the struck soldier stands, at spot 1: it is hit in full, and the
    // other two by their distance from that spot, each its own.
    let at = |index: usize| SubPos::spot_of(cell(20, 20), index as u8);
    let distance = |index: usize| {
        let (dx, dy) = (
            i64::from(at(index).x - at(1).x),
            i64::from(at(index).y - at(1).y),
        );
        ((dx * dx + dy * dy) as u64).isqrt()
    };
    assert_eq!([distance(0), distance(1), distance(2)], [151, 0, 135]);
    let damage = |index: usize| splash_damage(80, 100, 512, 25, distance(index));
    assert_eq!([damage(0), damage(1), damage(2)], [62, 80, 64]);
    for (index, &soldier) in soldiers.iter().enumerate() {
        assert_eq!(
            hp(&mut app, soldier),
            Some(100 - damage(index)),
            "soldier {index}"
        );
    }
}

// --- Leaving a building and a transport ---------------------------------------------------------

#[test]
fn trained_soldiers_leave_the_barracks_three_to_a_cell() {
    let mut app = world();
    place(&mut app, P0, "allied_yard", 4, 4);
    place(&mut app, P0, "allied_power", 9, 4);
    let barracks = place(&mut app, P0, "allied_barracks", 10, 10);
    let recruit = ProductionItem::Unit(defs(&app).unit_kind("recruit").expect("recruit"));
    let commands = (0..4)
        .map(|_| issue(P0, Command::QueueProduction { item: recruit }))
        .collect();
    let mut events = tick(&mut app, commands);
    for index in 0..400 {
        events.extend(tick(&mut app, Vec::new()));
        assert_cells(&mut app, &format!("tick {index}"));
    }
    for event in &events {
        assert!(
            !matches!(event, SimEvent::CommandRejected { .. }),
            "{event:?}"
        );
    }
    let trained: Vec<SimId> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitSpawned { unit, building, .. } if *building == barracks => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(trained.len(), 4, "{trained:?}");
    // The first three stand in one cell of the ring, at the three spots in the order they
    // came out, and the fourth in another cell of the ring.
    let first = mobile(&mut app, trained[0]).cell;
    assert_eq!(
        grid(&app).holders(first),
        [Some(trained[0]), Some(trained[1]), Some(trained[2])]
    );
    let fourth = mobile(&mut app, trained[3]).cell;
    assert_ne!(fourth, first);
    assert_eq!(grid(&app).holders(fourth), [Some(trained[3]), None, None]);
    assert_eq!(position(&mut app, trained[3]), SubPos::spot_of(fourth, 0));
}

#[test]
fn passengers_leave_a_transport_at_spots_and_a_vehicle_alone() {
    let mut app = world();
    // The hovercraft of the fixture carries two units, vehicles included.
    let transport = spawn(&mut app, P0, "hovercraft", 20, 20);
    let soldiers = [
        spawn(&mut app, P0, "recruit", 19, 20),
        spawn(&mut app, P0, "recruit", 19, 20),
    ];
    let commands = soldiers
        .iter()
        .map(|&unit| {
            issue(
                P0,
                Command::Interact {
                    unit,
                    target: transport,
                },
            )
        })
        .collect();
    let events = tick(&mut app, commands);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    // Aboard: off the map, their spots free.
    assert_eq!(grid(&app).holders(cell(19, 20)), [None; 3]);
    for &soldier in &soldiers {
        assert_eq!(hp(&mut app, soldier), None);
        assert!(!grid(&app).is_on_foot(soldier));
    }
    assert_cells(&mut app, "aboard");
    // A truck stands in the first cell of the ring: the soldiers come out into the next one,
    // together.
    let truck = spawn(&mut app, P0, "truck", 19, 19);
    let events = tick(
        &mut app,
        vec![issue(P0, Command::Deploy { unit: transport })],
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert_cells(&mut app, "unloaded");
    assert_eq!(grid(&app).holders(cell(19, 19)), [Some(truck), None, None]);
    assert_eq!(
        grid(&app).holders(cell(20, 19)),
        [Some(soldiers[0]), Some(soldiers[1]), None]
    );
    for (index, &soldier) in soldiers.iter().enumerate() {
        assert_eq!(
            position(&mut app, soldier),
            SubPos::spot_of(cell(20, 19), index as u8)
        );
        assert!(grid(&app).is_on_foot(soldier));
    }
}

// --- The invariant on every tick ----------------------------------------------------------------

/// A block of `count` units of `kind` for `owner`, `per_row` to a row from (`x`, `y`), `stack`
/// units to a cell.
fn block(
    app: &mut App,
    owner: sim::PlayerId,
    kind: &str,
    count: usize,
    (x, y): (i32, i32),
    per_row: i32,
    stack: usize,
) -> Vec<SimId> {
    (0..count)
        .map(|index| {
            let place = (index / stack) as i32;
            spawn(app, owner, kind, x + place % per_row, y + place / per_row)
        })
        .collect()
}

/// Two mixed groups of infantry on foot and vehicles, of two players who are enemies, west and
/// east of a 40 x 40 field. None of the types is armed and none is a crusher, so nothing but
/// movement happens.
fn two_groups(app: &mut App) -> (Vec<SimId>, Vec<SimId>) {
    let mut west = block(app, P0, "recruit", 18, (2, 14), 3, 3);
    west.extend(block(app, P0, "hazmat", 6, (2, 17), 3, 2));
    west.extend(block(app, P0, "truck", 8, (6, 14), 2, 1));
    let mut east = block(app, P1, "recruit", 18, (35, 14), 3, 3);
    east.extend(block(app, P1, "hazmat", 6, (35, 17), 3, 2));
    east.extend(block(app, P1, "truck", 8, (32, 14), 2, 1));
    (west, east)
}

#[test]
fn invariant_i6_prime_holds_while_mixed_infantry_and_vehicles_cross_for_2000_ticks() {
    let mut app = world();
    let (west, east) = two_groups(&mut app);
    assert_eq!(west.len() + east.len(), 64);
    assert_cells(&mut app, "start");
    let mut completed_orders = 0;
    let mut west_goes_east = true;
    let mut pending: BTreeSet<SimId> = BTreeSet::new();
    let mut crossings = 0;
    let mut shared = 0;
    for index in 0..2_000 {
        let mut commands = Vec::new();
        if pending.is_empty() {
            let (east_target, west_target) = if west_goes_east {
                ((36, 16), (3, 16))
            } else {
                ((3, 16), (36, 16))
            };
            commands.push(move_to(P0, &west, east_target.0, east_target.1));
            commands.push(move_to(P1, &east, west_target.0, west_target.1));
            pending = west.iter().chain(&east).copied().collect();
            west_goes_east = !west_goes_east;
            crossings += 1;
        }
        for event in tick(&mut app, commands) {
            match event {
                SimEvent::MoveCompleted { unit, .. } => {
                    assert!(pending.remove(&unit), "tick {index}: {unit:?}");
                    completed_orders += 1;
                }
                other => panic!("tick {index}: unexpected {other:?}"),
            }
        }
        assert_cells(&mut app, &format!("tick {index}"));
        // Cells that soldiers of both players share while the groups pass through each other.
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &sim::Owner)>();
        let owner: BTreeMap<SimId, sim::PlayerId> = query
            .iter(world)
            .map(|(id, owner)| (*id, owner.0))
            .collect();
        let grid = world.resource::<Grid>();
        for y in 10..24 {
            for x in 0..40 {
                let owners: BTreeSet<_> = grid
                    .holders(cell(x, y))
                    .iter()
                    .flatten()
                    .map(|id| owner[id])
                    .collect();
                shared += usize::from(owners.len() > 1);
            }
        }
    }
    println!("{crossings} crossings started, {completed_orders} orders completed, {shared} shared");
    // No deadlock: the groups crossed repeatedly, and nobody was lost.
    assert!(crossings >= 3, "{crossings} crossings");
    assert!(
        completed_orders >= 128,
        "{completed_orders} completed orders"
    );
    assert!(shared > 0, "soldiers of both players never shared a cell");
    for &unit in west.iter().chain(&east) {
        assert!(hp(&mut app, unit).is_some(), "{unit:?} is gone");
    }
}

#[test]
fn the_crossing_gives_the_same_hash_at_every_tick() {
    fn trace(order_twice: bool) -> Vec<u64> {
        let mut app = world();
        let (west, east) = two_groups(&mut app);
        let mut trace = vec![state_hash(app.world_mut())];
        for index in 0..500 {
            let mut commands = Vec::new();
            if index == 0 || (order_twice && index == 250) {
                commands.push(move_to(P0, &west, 36, 16));
                commands.push(move_to(P1, &east, 3, 16));
            }
            tick(&mut app, commands);
            trace.push(state_hash(app.world_mut()));
        }
        trace
    }
    let a = trace(false);
    let b = trace(false);
    assert_eq!(a, b);
    let c = trace(true);
    assert_eq!(a[..=250], c[..=250]);
    assert_ne!(a[251..], c[251..]);
}

#[test]
fn the_hash_covers_which_spot_a_soldier_holds() {
    // The same units in the same cells; in the second world the two soldiers of the cell came
    // to their spots in the other order.
    let spots = |swap: bool| {
        let mut app = world();
        let a = spawn(&mut app, P0, "recruit", 9, 10);
        let b = spawn(&mut app, P0, "recruit", 11, 10);
        let (first, second) = if swap { (b, a) } else { (a, b) };
        tick(&mut app, vec![move_to(P0, &[first], 10, 10)]);
        run_until_arrived(&mut app, &[first], 100);
        // The second arrives when the first stands, and takes the next free spot.
        while app.world().resource::<sim::SimTick>().0 < 40 {
            ticks(&mut app, 1);
        }
        tick(&mut app, vec![move_to(P0, &[second], 10, 10)]);
        run_until_arrived(&mut app, &[second], 100);
        while app.world().resource::<sim::SimTick>().0 < 80 {
            ticks(&mut app, 1);
        }
        (
            spot(&app, 10, 10, a),
            spot(&app, 10, 10, b),
            state_hash(app.world_mut()),
        )
    };
    let (a, b, plain) = spots(false);
    assert_eq!((a, b), (Some(0), Some(1)));
    let (a, b, swapped) = spots(true);
    assert_eq!((a, b), (Some(1), Some(0)));
    assert_ne!(plain, swapped);
    assert_eq!(plain, spots(false).2);
}

// --- The check itself --------------------------------------------------------------------------

/// Puts the unit `id` into `to` behind the simulation's back: scenario setup that breaks the
/// invariant, which the check must report.
fn misplace(app: &mut App, id: SimId, to: CellPos) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut sim::Mobile)>();
    for (other, mut mobile) in query.iter_mut(world) {
        if *other == id {
            mobile.cell = to;
        }
    }
}

fn violations(app: &mut App) -> Vec<String> {
    cell_invariant_violations(app.world_mut())
}

#[test]
fn the_check_of_the_invariant_reports_every_kind_of_violation() {
    let scene = || {
        let mut app = world();
        let trucks = [
            spawn(&mut app, P0, "truck", 5, 5),
            spawn(&mut app, P1, "truck", 6, 5),
        ];
        let soldiers = [
            spawn(&mut app, P0, "recruit", 8, 8),
            spawn(&mut app, P0, "recruit", 8, 8),
            spawn(&mut app, P1, "recruit", 8, 8),
            spawn(&mut app, P1, "recruit", 9, 8),
        ];
        (app, trucks, soldiers)
    };
    let (mut app, ..) = scene();
    assert_eq!(violations(&mut app), Vec::<String>::new());
    assert_eq!(stray_claims(app.world_mut()), Vec::<String>::new());

    let has =
        |problems: &[String], text: &str| problems.iter().any(|problem| problem.contains(text));
    // Two vehicles in a cell.
    let (mut app, trucks, _) = scene();
    misplace(&mut app, trucks[1], cell(5, 5));
    let problems = violations(&mut app);
    assert!(has(&problems, "is held by the vehicles"), "{problems:?}");
    // A vehicle with infantry.
    let (mut app, trucks, _) = scene();
    misplace(&mut app, trucks[0], cell(8, 8));
    let problems = violations(&mut app);
    assert!(has(&problems, "is held by the vehicle"), "{problems:?}");
    assert!(has(&problems, "and the infantry"), "{problems:?}");
    // Four infantry.
    let (mut app, _, soldiers) = scene();
    misplace(&mut app, soldiers[3], cell(8, 8));
    let problems = violations(&mut app);
    assert!(has(&problems, "is held by 4 infantry"), "{problems:?}");
    // Anything on a building: a vehicle and a soldier.
    for victim in [0, 1] {
        let (mut app, trucks, soldiers) = scene();
        place(&mut app, P0, "allied_power", 20, 20);
        misplace(&mut app, [trucks[0], soldiers[0]][victim], cell(20, 20));
        let problems = violations(&mut app);
        assert!(has(&problems, "which a building covers"), "{problems:?}");
    }
    // A unit that is not where its cell and spot put it.
    let (mut app, _, soldiers) = scene();
    {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &mut sim::Position)>();
        for (id, mut position) in query.iter_mut(world) {
            if *id == soldiers[1] {
                position.0 = SubPos::centre_of(cell(8, 8));
            }
        }
    }
    let problems = violations(&mut app);
    assert!(has(&problems, "its cells put it at"), "{problems:?}");
    // A claim whose unit is gone, and a claim of a cell the unit has left.
    let (mut app, trucks, soldiers) = scene();
    {
        let world = app.world_mut();
        let mut query = world.query::<(bevy_ecs::entity::Entity, &SimId)>();
        let gone: Vec<_> = query
            .iter(world)
            .filter(|(_, id)| **id == soldiers[2])
            .map(|(entity, _)| entity)
            .collect();
        for entity in gone {
            world.despawn(entity);
        }
    }
    misplace(&mut app, trucks[1], cell(30, 30));
    let stray = stray_claims(app.world_mut());
    assert_eq!(stray.len(), 2, "{stray:?}");
    assert!(
        has(&stray, &format!("unit {} in", soldiers[2].0)),
        "{stray:?}"
    );
    assert!(
        has(&stray, &format!("unit {} in", trucks[1].0)),
        "{stray:?}"
    );
    assert!(
        has(&violations(&mut app), "the grid does not record it there"),
        "the moved truck holds a cell the grid does not give it"
    );
    // Two aircraft in a cell of the air layer.
    let (mut app, ..) = scene();
    let jumpers = [
        spawn(&mut app, P0, "jumper", 12, 12),
        spawn(&mut app, P0, "jumper", 13, 12),
    ];
    misplace(&mut app, jumpers[1], cell(12, 12));
    let problems = violations(&mut app);
    assert!(has(&problems, "is held by the aircraft"), "{problems:?}");
}

// --- Worlds without the trait ------------------------------------------------------------------

#[test]
fn no_unit_of_the_older_fixtures_is_on_foot_or_a_crusher() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
    for name in ["legacy", "ra1", "ra2"] {
        let dir = root.join(name);
        let mut definitions = sim::load_match_definitions(
            dir.join("units.ron"),
            dir.join("economy.ron"),
            dir.join("combat.ron"),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        if dir.join("rules.ron").exists() {
            definitions = definitions
                .with_rules(dir.join("rules.ron"))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
        let definitions = definitions
            .with_arsenal(dir.join("arsenal.ron"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!definitions.arsenal().0.is_empty(), "{name}");
        assert!(!definitions.has_crushers(), "{name}");
        for index in 0..definitions.units().len() {
            let kind = sim::UnitKind(index as u16);
            assert!(!definitions.on_foot(kind), "{name}: unit {index}");
            assert!(!definitions.crushable(kind), "{name}: unit {index}");
        }
    }
    // The types of RA3-A in the RA3 fixture, and a world without an arsenal.
    let ra3 = definitions();
    for id in [
        "rifleman",
        "conscript",
        "engineer",
        "sniper",
        "tank",
        "heavy",
        "mortar",
    ] {
        assert!(!ra3.on_foot(ra3.unit_kind(id).expect(id)), "{id}");
    }
    let bare = sim::load_unit_definitions(root.join("units.ron")).expect("M2 fixture");
    assert!(!bare.has_crushers());
    assert!((0..bare.units().len()).all(|index| !bare.on_foot(sim::UnitKind(index as u16))));
    // Such a world has no spots: every unit holds its cell whole, at the centre.
    let mut app = world();
    let rifleman = spawn(&mut app, P0, "rifleman", 3, 3);
    assert!(!grid(&app).is_on_foot(rifleman));
    assert_eq!(spot(&app, 3, 3, rifleman), None);
    assert_eq!(grid(&app).room(cell(3, 3), true, |_| true), 0);
}
