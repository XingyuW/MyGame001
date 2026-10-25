//! RA3: crushing (docs/RA2_PARITY_PLAN.md section 5, RA3 item 6; docs/DECISIONS.md R6).
//!
//! A vehicle of a type with the arsenal's `crusher` trait that is commanded to move or to attack
//! move drives over the crushable infantry of its enemies in its way: they die as it starts the
//! step into their cell. Never its own, never an ally's, never infantry that is not crushable,
//! never a cell that holds any of these beside its victims, never an aircraft, and never on an
//! order that is not a move or an attack move. The tests drive the production path
//! (`CommandQueue`, `SimSchedule`) on the RA3 fixture (`tests/fixtures/data/ra3`): the roller
//! and the brute are crushers, the truck is not; the recruit and the trooper are crushable, the
//! hazmat is not, the jumper is crushable infantry that flies. Fixture data (invariant I10).

mod ra3_common;

use bevy_app::App;
use ra3_common::*;
use sim::{
    AttackEnd, Cell, CellPos, Command, Grid, IssuedCommand, MoveFailure, Orders, PlayerId, Players,
    SimEvent, SimId, SubPos, state_hash,
};

fn cell(x: i32, y: i32) -> CellPos {
    CellPos::new(x, y)
}

fn grid(app: &App) -> &Grid {
    app.world().resource::<Grid>()
}

fn rock(app: &mut App, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    let mut grid = app.world_mut().resource_mut::<Grid>();
    for y in y0..=y1 {
        for x in x0..=x1 {
            grid.set_terrain(
                cell(x, y),
                Cell {
                    passable: false,
                    water: false,
                    buildable: false,
                    ore: 0,
                },
            );
        }
    }
}

/// A corridor one cell wide along y = 10 from x = 3 to x = 17, closed at its east end: the only
/// way from (4, 10) to (16, 10) leads through (10, 10).
fn corridor(app: &mut App) {
    rock(app, (3, 9), (18, 9));
    rock(app, (3, 11), (18, 11));
    rock(app, (18, 10), (18, 10));
}

fn attack_move(player: PlayerId, units: &[SimId], x: i32, y: i32) -> IssuedCommand {
    issue(
        player,
        Command::AttackMove {
            units: units.to_vec(),
            target: cell(x, y),
        },
    )
}

/// The crushings `events` report: (victim, its owner, the crusher, the crusher's owner, where).
fn crushed(events: &[SimEvent]) -> Vec<(SimId, PlayerId, SimId, PlayerId, SubPos)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitCrushed {
                player,
                unit,
                crusher,
                by,
                at,
            } => Some((*unit, *player, *crusher, *by, *at)),
            _ => None,
        })
        .collect()
}

fn destroyed(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitDestroyed { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

/// How the order of `unit` ended in `events`, if it did.
fn end_of(events: &[SimEvent], unit: SimId) -> Option<SimEvent> {
    events
        .iter()
        .find(|event| {
            matches!(
                event,
                SimEvent::MoveCompleted { unit: moved, .. } | SimEvent::MoveFailed { unit: moved, .. }
                    if *moved == unit
            )
        })
        .cloned()
}

/// Runs until the move of `unit` ends, checking I6' after every tick; the events of all the
/// ticks, and the ticks taken (the first is the tick of `commands`).
fn run_until_end(
    app: &mut App,
    mut commands: Vec<IssuedCommand>,
    unit: SimId,
    limit: u32,
) -> (Vec<SimEvent>, u32) {
    let mut all = Vec::new();
    for index in 1..=limit {
        let events = tick(app, std::mem::take(&mut commands));
        assert_cells(app, &format!("tick {index}"));
        let ended = end_of(&events, unit).is_some();
        all.extend(events);
        if ended {
            return (all, index);
        }
    }
    panic!("the order of {unit:?} did not end in {limit} ticks");
}

fn order_may_crush(app: &mut App, unit: SimId) -> Option<bool> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Orders)>();
    query
        .iter(world)
        .find(|(id, _)| **id == unit)
        .and_then(|(_, orders)| orders.current.as_ref().map(|order| order.crush))
}

// --- Positive ---------------------------------------------------------------------------------

#[test]
fn a_crusher_commanded_to_move_drives_over_a_hostile_soldier() {
    let mut app = world();
    let roller = spawn(&mut app, P0, "roller", 5, 20);
    let soldier = spawn(&mut app, P1, "recruit", 10, 20);
    let stood = position(&mut app, soldier);
    assert_eq!(stood, SubPos::spot_of(cell(10, 20), 0));
    let mut commands = vec![move_to(P0, &[roller], 15, 20)];
    let mut events = Vec::new();
    let mut crush_tick = None;
    for index in 1..=200 {
        let now = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        if index == 1 {
            assert_eq!(order_may_crush(&mut app, roller), Some(true));
        }
        if !crushed(&now).is_empty() {
            crush_tick = Some(index);
            // It dies as the roller starts the step into its cell: the roller stands a cell
            // before it, has reserved the soldier's cell, and nothing of the soldier is left.
            let state = mobile(&mut app, roller);
            assert_eq!(state.cell, cell(9, 20));
            assert_eq!(state.step.map(|step| step.to), Some(cell(10, 20)));
            assert_eq!(grid(&app).holders(cell(10, 20)), [Some(roller), None, None]);
            assert_eq!(
                hp(&mut app, soldier),
                None,
                "the soldier is gone in this tick"
            );
            assert!(!grid(&app).is_on_foot(soldier));
        }
        let ended = end_of(&now, roller).is_some();
        events.extend(now);
        if ended {
            break;
        }
    }
    // The soldier's owner lost it, the roller's owner is its killer, and the crushing is
    // followed by the destruction in the same tick.
    assert_eq!(
        crushed(&events),
        vec![(soldier, P1, roller, P0, stood)],
        "{events:?}"
    );
    assert_eq!(destroyed(&events), vec![soldier]);
    let crush = events
        .iter()
        .position(|event| matches!(event, SimEvent::UnitCrushed { .. }))
        .expect("crushed");
    assert!(
        matches!(events[crush + 1], SimEvent::UnitDestroyed { player, unit } if player == P1 && unit == soldier)
    );
    // The roller did not wait: 4 cells to the soldier's, 8 ticks each, and it starts the fifth
    // step in the tick after it has arrived beside the soldier.
    assert_eq!(crush_tick, Some(4 * 8 + 1));
    assert_eq!(
        end_of(&events, roller),
        Some(SimEvent::MoveCompleted {
            unit: roller,
            retargeted: false
        })
    );
    assert_eq!(mobile(&mut app, roller).cell, cell(15, 20));

    // The same drive without a soldier takes the same number of ticks.
    let drive = |with_soldier: bool| {
        let mut app = world();
        let roller = spawn(&mut app, P0, "roller", 5, 20);
        if with_soldier {
            spawn(&mut app, P1, "recruit", 10, 20);
        }
        run_until_end(&mut app, vec![move_to(P0, &[roller], 15, 20)], roller, 400).1
    };
    assert_eq!(drive(true), drive(false));
    assert_eq!(drive(true), 10 * 8);
}

#[test]
fn three_soldiers_in_a_cell_are_crushed_together_each_at_its_spot() {
    let mut app = world();
    corridor(&mut app);
    // Of two hostile players; one of them is walking out of the cell when the roller comes.
    let roller = spawn(&mut app, P0, "roller", 4, 10);
    let victims = [
        spawn(&mut app, P1, "recruit", 10, 10),
        spawn(&mut app, P1, "trooper", 10, 10),
        spawn(&mut app, P1, "recruit", 10, 10),
    ];
    let (events, _) = run_until_end(&mut app, vec![move_to(P0, &[roller], 16, 10)], roller, 600);
    let found = crushed(&events);
    assert_eq!(
        found,
        victims
            .iter()
            .enumerate()
            .map(|(index, &victim)| (
                victim,
                P1,
                roller,
                P0,
                SubPos::spot_of(cell(10, 10), index as u8)
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(destroyed(&events), victims.to_vec());
    assert_eq!(
        end_of(&events, roller),
        Some(SimEvent::MoveCompleted {
            unit: roller,
            retargeted: false
        })
    );
    assert_eq!(mobile(&mut app, roller).cell, cell(16, 10));
}

#[test]
fn a_crushed_soldier_is_gone_at_once_and_fires_no_more() {
    // An armed soldier fires at the roller that comes for it, every 10 ticks. Whatever the
    // tick of its next shot (the roller starts from seven distances), it fires none in or
    // after the tick in which it is crushed: it is removed in the `Movement` stage, before
    // the `Combat` stage of that tick.
    let mut fired_before = 0;
    for start in 1..=7 {
        let mut app = world();
        let roller = spawn(&mut app, P0, "roller", start, 20);
        let trooper = spawn(&mut app, P1, "trooper", 10, 20);
        let mut commands = vec![move_to(P0, &[roller], 15, 20)];
        let mut crushed_at = None;
        for index in 1..=200 {
            let events = tick(&mut app, std::mem::take(&mut commands));
            assert_cells(&mut app, &format!("start {start}, tick {index}"));
            let shots = shots(&events, trooper).len();
            if !crushed(&events).is_empty() {
                crushed_at = Some(index);
            }
            match crushed_at {
                None => fired_before += shots,
                Some(_) => assert_eq!(shots, 0, "start {start}, tick {index}"),
            }
        }
        assert!(
            crushed_at.is_some(),
            "start {start}: the trooper was not crushed"
        );
    }
    assert!(
        fired_before >= 7,
        "the trooper fired {fired_before} shots in all"
    );
}

#[test]
fn an_attack_move_crushes_too_and_goes_on_crushing_after_a_fight() {
    let mut app = world();
    // The brute's sam engages aircraft only: it cannot fire at the soldier in its way.
    let brute = spawn(&mut app, P0, "brute", 5, 20);
    let jumper = spawn(&mut app, P1, "jumper", 11, 23);
    let soldier = spawn(&mut app, P1, "recruit", 18, 20);
    let mut commands = vec![attack_move(P0, &[brute], 25, 20)];
    let mut events = Vec::new();
    let mut fought_at = None;
    let mut resumed = false;
    for index in 1..=1_000 {
        let now = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        if fought_at.is_none() && !shots(&now, brute).is_empty() {
            fought_at = Some(index);
        }
        if fought_at.is_some() && hp(&mut app, jumper).is_none() && !resumed {
            // The move it takes up again after the fight may crush as the first did.
            if let Some(crush) = order_may_crush(&mut app, brute) {
                assert!(crush, "tick {index}");
                resumed = true;
            }
        }
        let ended = end_of(&now, brute).is_some();
        events.extend(now);
        if ended {
            break;
        }
    }
    assert!(fought_at.is_some(), "the brute never fired at the jumper");
    assert!(resumed, "the brute never took up its move again");
    assert!(
        shots(&events, brute)
            .iter()
            .all(|&(target, _)| target == jumper)
    );
    assert_eq!(hp(&mut app, jumper), None, "the jumper was shot down");
    // The soldier was crushed, after the fight, and not shot at.
    let found = crushed(&events);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].0, found[0].2, found[0].3), (soldier, brute, P0));
    let crush = events
        .iter()
        .position(|event| matches!(event, SimEvent::UnitCrushed { .. }))
        .expect("crushed");
    let last_shot = events
        .iter()
        .rposition(|event| matches!(event, SimEvent::WeaponFired { .. }))
        .expect("fired");
    assert!(last_shot < crush);
    assert_eq!(mobile(&mut app, brute).cell, cell(25, 20));

    // An unarmed crusher: its attack move is a move that may crush.
    let mut app = world();
    let roller = spawn(&mut app, P0, "roller", 5, 20);
    let soldier = spawn(&mut app, P1, "recruit", 10, 20);
    let (events, _) = run_until_end(
        &mut app,
        vec![attack_move(P0, &[roller], 15, 20)],
        roller,
        400,
    );
    assert_eq!(destroyed(&events), vec![soldier]);
    assert_eq!(crushed(&events).len(), 1);
}

#[test]
fn the_pathfinder_leads_a_crusher_through_infantry_it_may_crush() {
    // A wall from top to bottom with two gaps, at y = 10 and far away at y = 34. A hostile
    // soldier stands in the near gap, and a truck of the mover's owner stands in the mover's
    // way before the wall, so that the mover asks for a path around the units that stand.
    let run = |mover: &str| {
        let mut app = world();
        rock(&mut app, (12, 0), (12, 9));
        rock(&mut app, (12, 11), (12, 33));
        rock(&mut app, (12, 35), (12, 39));
        let soldier = spawn(&mut app, P1, "recruit", 12, 10);
        spawn(&mut app, P0, "truck", 8, 10);
        let moving = spawn(&mut app, P0, mover, 4, 10);
        let (events, ticks) = run_until_end(
            &mut app,
            vec![move_to(P0, &[moving], 16, 10)],
            moving,
            3_000,
        );
        assert_eq!(
            end_of(&events, moving),
            Some(SimEvent::MoveCompleted {
                unit: moving,
                retargeted: false
            }),
            "{mover}"
        );
        (
            crushed(&events).len(),
            hp(&mut app, soldier).is_some(),
            ticks,
        )
    };
    // The roller goes round the truck and through the near gap, over the soldier.
    let (crushings, alive, direct) = run("roller");
    assert_eq!((crushings, alive), (1, false));
    // The truck, which is no crusher, finds the near gap closed and takes the far one.
    let (crushings, alive, detour) = run("truck");
    assert_eq!((crushings, alive), (0, true));
    assert!(
        direct < 150 && detour > 2 * direct,
        "through the gap {direct} ticks, round the wall {detour}"
    );
}

// --- Negative ---------------------------------------------------------------------------------

/// The roller of P0 at (4, 10) in the corridor, commanded to (16, 10), with `blockers` (owner,
/// type) standing in (10, 10); P0 and P1 are allies if `allied`. Returns whether the roller ever
/// held (10, 10), how many units were crushed, how many of the blockers are left, and how the
/// order ended.
fn roller_in_the_corridor(
    blockers: &[(PlayerId, &str)],
    allied: bool,
) -> (bool, usize, usize, SimEvent) {
    let mut app = world();
    corridor(&mut app);
    if allied {
        let mut players = app.world_mut().resource_mut::<Players>();
        assert!(players.set_team(P0, 1) && players.set_team(P1, 1));
    }
    let standing: Vec<SimId> = blockers
        .iter()
        .map(|&(owner, kind)| spawn(&mut app, owner, kind, 10, 10))
        .collect();
    let roller = spawn(&mut app, P0, "roller", 4, 10);
    let mut commands = vec![move_to(P0, &[roller], 16, 10)];
    let mut entered = false;
    let mut crushings = 0;
    for index in 1..=3_000 {
        let events = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("{blockers:?}, tick {index}"));
        let state = mobile(&mut app, roller);
        entered |=
            state.cell == cell(10, 10) || state.step.is_some_and(|step| step.to == cell(10, 10));
        crushings += crushed(&events).len();
        if let Some(end) = end_of(&events, roller) {
            let left = standing
                .iter()
                .filter(|&&id| hp(&mut app, id).is_some())
                .count();
            return (entered, crushings, left, end);
        }
    }
    panic!("{blockers:?}: the order never ended");
}

/// The order ended short of its destination, as the rules for cells held by standing units
/// have it.
fn stopped_short(end: &SimEvent) -> bool {
    matches!(
        end,
        SimEvent::MoveCompleted {
            retargeted: true,
            ..
        } | SimEvent::MoveFailed {
            reason: MoveFailure::Blocked,
            ..
        }
    )
}

#[test]
fn a_crusher_never_crushes_its_own_an_allys_or_what_is_not_crushable() {
    // The control: one hostile crushable soldier is crushed, and the roller arrives.
    let (entered, crushings, left, end) = roller_in_the_corridor(&[(P1, "recruit")], false);
    assert!(entered && crushings == 1 && left == 0, "{end:?}");
    assert!(!stopped_short(&end), "{end:?}");

    type Blockers<'a> = &'a [(PlayerId, &'a str)];
    let cases: [(&str, Blockers, bool); 6] = [
        ("its own soldier", &[(P0, "recruit")], false),
        ("an ally's soldier", &[(P1, "recruit")], true),
        (
            "a hostile soldier that is not crushable",
            &[(P1, "hazmat")],
            false,
        ),
        (
            "a hostile crushable soldier beside its own",
            &[(P1, "recruit"), (P0, "recruit")],
            false,
        ),
        (
            "hostile soldiers of whom one is not crushable",
            &[(P1, "recruit"), (P1, "hazmat"), (P1, "trooper")],
            false,
        ),
        ("a hostile vehicle", &[(P1, "truck")], false),
    ];
    for (name, blockers, allied) in cases {
        let (entered, crushings, left, end) = roller_in_the_corridor(blockers, allied);
        assert!(!entered, "{name}: the roller entered the cell");
        assert_eq!(crushings, 0, "{name}");
        assert_eq!(left, blockers.len(), "{name}: a unit is gone");
        assert!(stopped_short(&end), "{name}: {end:?}");
    }
}

#[test]
fn a_crusher_passes_under_infantry_that_flies_and_aircraft_crush_nothing() {
    let mut app = world();
    corridor(&mut app);
    // A hostile jumper hovers over the corridor: crushable by its data, and in the air.
    let jumper = spawn(&mut app, P1, "jumper", 10, 10);
    let roller = spawn(&mut app, P0, "roller", 4, 10);
    let (events, _) = run_until_end(&mut app, vec![move_to(P0, &[roller], 16, 10)], roller, 600);
    assert!(crushed(&events).is_empty());
    assert_eq!(hp(&mut app, jumper), Some(100));
    assert_eq!(mobile(&mut app, roller).cell, cell(16, 10));
    assert_eq!(
        end_of(&events, roller),
        Some(SimEvent::MoveCompleted {
            unit: roller,
            retargeted: false
        })
    );

    // An aircraft commanded over a cell of hostile soldiers flies over them.
    let mut app = world();
    let soldiers = [
        spawn(&mut app, P1, "recruit", 10, 20),
        spawn(&mut app, P1, "recruit", 10, 20),
    ];
    let gunship = spawn(&mut app, P0, "jumper", 5, 20);
    let (events, _) = run_until_end(
        &mut app,
        vec![move_to(P0, &[gunship], 10, 20)],
        gunship,
        400,
    );
    assert!(crushed(&events).is_empty() && destroyed(&events).is_empty());
    assert_eq!(mobile(&mut app, gunship).cell, cell(10, 20));
    for soldier in soldiers {
        assert_eq!(hp(&mut app, soldier), Some(100));
    }
    assert_eq!(order_may_crush(&mut app, gunship), None);
}

#[test]
fn a_vehicle_that_is_no_crusher_and_a_soldier_crush_nothing() {
    for mover in ["truck", "recruit", "trooper"] {
        let mut app = world();
        let soldier = spawn(&mut app, P1, "recruit", 10, 20);
        let moving = spawn(&mut app, P0, mover, 5, 20);
        let mut commands = vec![move_to(P0, &[moving], 15, 20)];
        let mut events = Vec::new();
        for index in 1..=600 {
            let now = tick(&mut app, std::mem::take(&mut commands));
            assert_cells(&mut app, &format!("{mover}, tick {index}"));
            if index == 1 {
                assert_eq!(order_may_crush(&mut app, moving), Some(false), "{mover}");
            }
            let ended = end_of(&now, moving).is_some();
            events.extend(now);
            if ended {
                break;
            }
        }
        assert!(crushed(&events).is_empty(), "{mover}");
        assert_eq!(hp(&mut app, soldier), Some(100), "{mover}");
        assert_eq!(mobile(&mut app, moving).cell, cell(15, 20), "{mover}");
    }
}

#[test]
fn a_crusher_that_chases_its_target_does_not_crush() {
    let mut app = world();
    // A corridor closed at its west end and open to the east, a hostile soldier in it, and
    // beyond its east end a hostile jumper that the brute is ordered to attack. A scout of the
    // brute's owner stands near the jumper, so that the brute's owner sees it.
    rock(&mut app, (2, 9), (17, 9));
    rock(&mut app, (2, 11), (17, 11));
    rock(&mut app, (2, 10), (2, 10));
    let soldier = spawn(&mut app, P1, "recruit", 10, 10);
    let jumper = spawn(&mut app, P1, "jumper", 26, 10);
    spawn(&mut app, P0, "engineer", 28, 10);
    let brute = spawn(&mut app, P0, "brute", 4, 10);
    ticks(&mut app, 1);
    let mut commands = vec![attack(P0, brute, jumper)];
    let mut events = Vec::new();
    let mut ended = None;
    for index in 1..=3_000 {
        let now = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        // The moves toward the target are not commanded moves: they may not crush.
        if let Some(crush) = order_may_crush(&mut app, brute) {
            assert!(!crush, "tick {index}");
        }
        ended = now.iter().find_map(|event| match event {
            SimEvent::AttackEnded { unit, reason, .. } if *unit == brute => Some(*reason),
            _ => None,
        });
        events.extend(now);
        if ended.is_some() {
            break;
        }
    }
    assert_eq!(ended, Some(AttackEnd::OutOfReach));
    assert!(crushed(&events).is_empty());
    assert_eq!(hp(&mut app, soldier), Some(100));
    assert!(shots(&events, brute).is_empty());
    assert!(
        mobile(&mut app, brute).cell.x < 10,
        "it never passed the soldier"
    );

    // The same brute, commanded to move, drives over the soldier.
    let (events, _) = run_until_end(&mut app, vec![move_to(P0, &[brute], 16, 10)], brute, 600);
    assert_eq!(destroyed(&events), vec![soldier]);
    assert_eq!(crushed(&events).len(), 1);
}

// --- Boundary -----------------------------------------------------------------------------------

/// A hostile soldier at (10, 10) is commanded north in tick 1; its step out of the cell takes
/// 16 ticks, so it frees the cell in tick 16. A roller at (8, 10) is commanded east `delay`
/// ticks later; it takes 8 ticks to the cell beside the soldier's and decides in the tick
/// after whether it enters the soldier's cell. Returns whether the soldier was crushed, and
/// the tick of that decision.
fn leaving_soldier(soldier_first: bool, delay: u32) -> (bool, u32) {
    let mut app = world();
    let (soldier, roller) = if soldier_first {
        let soldier = spawn(&mut app, P1, "recruit", 10, 10);
        (soldier, spawn(&mut app, P0, "roller", 8, 10))
    } else {
        let roller = spawn(&mut app, P0, "roller", 8, 10);
        (spawn(&mut app, P1, "recruit", 10, 10), roller)
    };
    assert_eq!(soldier < roller, soldier_first);
    let mut events = Vec::new();
    let mut decided = 0;
    for index in 1..=60 {
        let mut commands = Vec::new();
        if index == 1 {
            commands.push(move_to(P1, &[soldier], 10, 5));
        }
        if index == 1 + delay {
            commands.push(move_to(P0, &[roller], 14, 10));
        }
        let now = tick(&mut app, commands);
        assert_cells(&mut app, &format!("delay {delay}, tick {index}"));
        let state = mobile(&mut app, roller);
        if decided == 0
            && (state.cell == cell(10, 10)
                || state.step.is_some_and(|step| step.to == cell(10, 10)))
        {
            decided = index;
        }
        events.extend(now);
    }
    let found = crushed(&events);
    assert!(found.len() <= 1);
    assert_eq!(found.is_empty(), hp(&mut app, soldier).is_some());
    (!found.is_empty(), decided)
}

#[test]
fn a_soldier_is_crushed_until_the_tick_in_which_it_has_left_the_cell() {
    for soldier_first in [true, false] {
        // A tick before the soldier is out: crushed, whichever of the two moves first.
        assert_eq!(leaving_soldier(soldier_first, 6), (true, 15));
        // A tick after: the cell is free.
        assert_eq!(leaving_soldier(soldier_first, 8), (false, 17));
    }
    // In the tick in which the soldier's step ends, the units move in the order of their ids
    // (docs/DECISIONS.md D19): a soldier that moves first has left the cell when the roller
    // decides, and lives; one that moves after the roller still holds its spot, and dies.
    assert_eq!(leaving_soldier(true, 7), (false, 16));
    assert_eq!(leaving_soldier(false, 7), (true, 16));
}

#[test]
fn a_soldier_walking_into_the_cell_is_crushed_with_it() {
    let mut app = world();
    // The soldier walks west toward the roller: when the roller starts into (10, 10) the
    // soldier has reserved a spot there and is still on its way from (11, 10).
    let roller = spawn(&mut app, P0, "roller", 8, 10);
    let soldier = spawn(&mut app, P1, "recruit", 11, 10);
    let mut commands = vec![
        move_to(P0, &[roller], 14, 10),
        move_to(P1, &[soldier], 10, 10),
    ];
    let mut events = Vec::new();
    for index in 1..=60 {
        let now = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        if !crushed(&now).is_empty() {
            // Both cells it held are free of it.
            assert_eq!(grid(&app).holders(cell(11, 10)), [None; 3]);
            assert_eq!(grid(&app).holders(cell(10, 10)), [Some(roller), None, None]);
            assert_eq!(index, 9);
        }
        events.extend(now);
    }
    let found = crushed(&events);
    assert_eq!(found.len(), 1);
    // It is reported where it was going to stand in the crushed cell.
    assert_eq!(
        (found[0].0, found[0].4),
        (soldier, SubPos::spot_of(cell(10, 10), 0))
    );
    assert_eq!(mobile(&mut app, roller).cell, cell(14, 10));
}

// --- Determinism and the invariant under crushing ----------------------------------------------

/// Rollers and soldiers of two hostile players cross a field: the soldiers that are crushable
/// die under the rollers of the other player, the others block them. Returns the units of the
/// west and of the east, each with its type.
type Group = Vec<(SimId, &'static str)>;

fn melee(app: &mut App) -> (Group, Group) {
    let mut west = Vec::new();
    let mut east = Vec::new();
    for row in 0..6 {
        west.push((spawn(app, P0, "roller", 3, 12 + 2 * row), "roller"));
        east.push((spawn(app, P1, "roller", 36, 13 + 2 * row), "roller"));
        for (kind, column) in [
            ("recruit", 0),
            ("recruit", 0),
            ("hazmat", 1),
            ("trooper", 1),
        ] {
            west.push((spawn(app, P0, kind, 6 + column, 12 + 2 * row), kind));
            east.push((spawn(app, P1, kind, 33 - column, 13 + 2 * row), kind));
        }
    }
    (west, east)
}

fn ids(group: &Group) -> Vec<SimId> {
    group.iter().map(|&(id, _)| id).collect()
}

#[test]
fn invariant_i6_prime_holds_while_crushers_and_infantry_of_two_players_cross() {
    let mut app = world();
    let (west, east) = melee(&mut app);
    let kind_of = |id: SimId| {
        west.iter()
            .chain(&east)
            .find(|&&(other, _)| other == id)
            .map(|&(_, kind)| kind)
            .unwrap_or_else(|| panic!("{id:?} is of neither group"))
    };
    let (west, east) = (ids(&west), ids(&east));
    assert_cells(&mut app, "start");
    let mut commands = vec![move_to(P0, &west, 36, 18), move_to(P1, &east, 3, 18)];
    let mut victims = Vec::new();
    let mut lost = Vec::new();
    for index in 1..=1_500 {
        let events = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(&mut app, &format!("tick {index}"));
        for (victim, owner, crusher, by, _) in crushed(&events) {
            assert_ne!(owner, by, "tick {index}: a unit crushed its own");
            assert!(
                west.contains(&crusher) != west.contains(&victim),
                "tick {index}"
            );
            assert_eq!(hp(&mut app, victim), None, "tick {index}");
            // Only what is crushable, by a crusher.
            assert!(
                ["recruit", "trooper"].contains(&kind_of(victim)),
                "tick {index}: a {} was crushed",
                kind_of(victim)
            );
            assert_eq!(kind_of(crusher), "roller", "tick {index}");
            victims.push(victim);
        }
        lost.extend(destroyed(&events));
        if index == 750 {
            // The groups turn round and cross again.
            commands = vec![move_to(P0, &west, 3, 18), move_to(P1, &east, 36, 18)];
        }
    }
    println!("{} crushed, {} lost", victims.len(), lost.len());
    assert!(victims.len() >= 6, "{} crushed", victims.len());
    // Every crushed unit is reported destroyed, once.
    assert!(victims.iter().all(|victim| lost.contains(victim)));
    let mut once = victims.clone();
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), victims.len());
    // No roller and no hazmat was driven over: all of them are still there.
    for &id in west.iter().chain(&east) {
        if ["roller", "hazmat"].contains(&kind_of(id)) {
            assert!(hp(&mut app, id).is_some(), "a {} is gone", kind_of(id));
        }
    }
}

#[test]
fn a_battle_with_crushing_gives_the_same_hash_at_every_tick() {
    fn trace(turn: bool) -> Vec<u64> {
        let mut app = world();
        let (west, east) = melee(&mut app);
        let (west, east) = (ids(&west), ids(&east));
        let mut commands = vec![move_to(P0, &west, 36, 18), move_to(P1, &east, 3, 18)];
        let mut trace = vec![state_hash(app.world_mut())];
        for index in 1..=500 {
            if turn && index == 200 {
                commands = vec![move_to(P0, &west, 3, 18)];
            }
            tick(&mut app, std::mem::take(&mut commands));
            trace.push(state_hash(app.world_mut()));
        }
        trace
    }
    let a = trace(false);
    assert_eq!(a, trace(false));
    let b = trace(true);
    assert_eq!(a[..200], b[..200]);
    assert_ne!(a[200..], b[200..]);
}
