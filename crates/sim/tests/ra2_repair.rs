//! RA2: selling, repairing buildings, and the depot that repairs vehicles for credits
//! (docs/RA2_PARITY_PLAN.md sections 4.6 and 5, RA2 items 8 and 9; docs/DECISIONS.md R26, R27).
//!
//! Driven through `CommandQueue` and `SimSchedule` on the RA2 fixture (`tests/fixtures/data/ra2`;
//! invariant I10), whose rules differ from the shipped ones: a sale refunds 40 percent of the
//! cost; a repair step restores 10 hit points every 5 ticks (a vehicle on a depot every 4) and
//! costs `max(1, (cost div (hit points div 10)) x 20 div 100)` credits.

mod ra2_common;

use bevy_app::App;
use ra2_common::*;
use sim::{CellPos, Command, Players, RejectReason, SimEvent, SimId, repair_step_cost, state_hash};

fn sell(building: SimId) -> sim::IssuedCommand {
    issue(P0, Command::SellBuilding { building })
}

fn repair(building: SimId) -> sim::IssuedCommand {
    issue(P0, Command::RepairBuilding { building })
}

fn dock(unit: SimId, target: SimId) -> sim::IssuedCommand {
    issue(P0, Command::Interact { unit, target })
}

fn repairing(app: &App) -> Vec<SimId> {
    app.world()
        .resource::<Players>()
        .get(P0)
        .expect("player")
        .repairing()
        .iter()
        .copied()
        .collect()
}

fn docked(app: &App) -> Vec<(SimId, SimId)> {
    app.world()
        .resource::<sim::SpecialState>()
        .docked
        .iter()
        .map(|(unit, building)| (*unit, *building))
        .collect()
}

/// The changes of hit points of `id` and of P0's credits over `count` ticks, one entry per
/// tick in which either changed.
fn steps(app: &mut App, id: SimId, count: u32) -> Vec<(i32, i64)> {
    let mut found = Vec::new();
    let (mut hp, mut money) = (hit_points(app, id), credits(app, P0));
    for _ in 0..count {
        tick(app, Vec::new());
        let (now, left) = (hit_points(app, id), credits(app, P0));
        if (now, left) != (hp, money) {
            found.push((now - hp, money - left));
        }
        (hp, money) = (now, left);
    }
    found
}

// --- Selling ------------------------------------------------------------------------------------

#[test]
fn a_sale_refunds_the_percentage_of_the_rules() {
    // A power building of 300 credits: 40 percent, not the economy file's sell value of 150.
    let mut app = world(1_000);
    let power = place(&mut app, P0, "allied_power", 10, 10);
    let events = tick(&mut app, vec![sell(power)]);
    assert!(events.contains(&SimEvent::BuildingSold {
        player: P0,
        building: power,
        refund: 120
    }));
    assert_eq!(credits(&app, P0), 1_120);
    assert!(!exists(&mut app, power));
    assert_eq!(defs(&app).refund(kind(&app, "allied_power")), 120);
}

#[test]
fn a_wall_is_neither_sold_nor_repaired() {
    let run = |commands: bool| {
        let mut app = world(1_000);
        let wall = place(&mut app, P0, "wall", 10, 10);
        set_hit_points(&mut app, wall, 50);
        let events = if commands {
            tick(&mut app, vec![sell(wall), repair(wall)])
        } else {
            tick(&mut app, Vec::new())
        };
        if commands {
            assert_eq!(
                rejections(&events),
                [
                    RejectReason::NotSellable(wall),
                    RejectReason::NotRepairable(wall)
                ]
            );
            assert!(repairing(&app).is_empty());
        }
        ticks(&mut app, 20);
        assert_eq!(hit_points(&mut app, wall), 50);
        assert_eq!(credits(&app, P0), 1_000);
        state_hash(app.world_mut())
    };
    assert_eq!(run(true), run(false), "the rejections change no state");
    assert_eq!(
        RejectReason::NotSellable(SimId(4)).to_string(),
        "building 4 cannot be sold"
    );
    assert_eq!(
        RejectReason::NotRepairable(SimId(4)).to_string(),
        "building 4 cannot be repaired"
    );
}

// --- Repairing buildings ------------------------------------------------------------------------

#[test]
fn a_building_is_repaired_in_steps_at_the_price_of_the_rules() {
    // A refinery of 600 credits and 600 hit points: 600 div (600 div 10) x 20% = 2 credits per
    // step of 10 hit points, every 5 ticks.
    let mut app = world(1_000);
    let refinery = place(&mut app, P0, "allied_refinery", 10, 10);
    let economy = defs(&app).rules().expect("rules").economy;
    assert_eq!(repair_step_cost(&economy, 600, 600, false), 2);
    assert_eq!(repair_step_cost(&economy, 600, 600, true), 1, "Super Me");
    assert_eq!(repair_step_cost(&economy, 0, 600, false), 1, "at least 1");
    assert_eq!(repair_step_cost(&economy, 600, 5, false), 120, "one step");
    set_hit_points(&mut app, refinery, 600 - 35);
    // The order arrives in a tick that is no repair step (every 5 ticks).
    tick(&mut app, Vec::new());
    assert_eq!(rejections(&tick(&mut app, vec![repair(refinery)])), NONE);
    assert_eq!(repairing(&app), [refinery]);
    // Three whole steps and a last one of 5 hit points at the price of a step, 5 ticks apart.
    assert_eq!(
        steps(&mut app, refinery, 40),
        [(10, 2), (10, 2), (10, 2), (5, 2)]
    );
    assert_eq!(hit_points(&mut app, refinery), 600);
    assert_eq!(credits(&app, P0), 1_000 - 8);
    assert!(
        repairing(&app).is_empty(),
        "a whole building ends its repair"
    );
}

#[test]
fn a_repair_is_paid_to_the_last_credit_and_waits_for_the_next() {
    // 40 hit points lost: four steps of 2 credits. With 8 credits the repair completes and
    // leaves none; with 7 the fourth step waits, and the credits never fall below zero.
    let damaged = |credits: i64| {
        let mut app = world(credits);
        let refinery = place(&mut app, P0, "allied_refinery", 10, 10);
        set_hit_points(&mut app, refinery, 560);
        tick(&mut app, Vec::new());
        assert_eq!(rejections(&tick(&mut app, vec![repair(refinery)])), NONE);
        (app, refinery)
    };
    let (mut exact, refinery) = damaged(8);
    assert_eq!(steps(&mut exact, refinery, 40), [(10, 2); 4]);
    assert_eq!(
        (hit_points(&mut exact, refinery), credits(&exact, P0)),
        (600, 0)
    );
    let (mut short, refinery) = damaged(7);
    assert_eq!(steps(&mut short, refinery, 40), [(10, 2); 3]);
    assert_eq!(
        (hit_points(&mut short, refinery), credits(&short, P0)),
        (590, 1)
    );
    assert_eq!(repairing(&short), [refinery], "it waits for credits");
    // A sale brings credits, and the repair goes on.
    let bunker = place(&mut short, P0, "bunker", 20, 20);
    assert_eq!(rejections(&tick(&mut short, vec![sell(bunker)])), NONE);
    ticks(&mut short, 10);
    assert_eq!(
        (hit_points(&mut short, refinery), credits(&short, P0)),
        (600, 1 + 100 - 2)
    );
    assert!(repairing(&short).is_empty());
}

#[test]
fn a_second_repair_order_stops_the_repair() {
    let mut app = world(1_000);
    let refinery = place(&mut app, P0, "allied_refinery", 10, 10);
    set_hit_points(&mut app, refinery, 300);
    assert_eq!(rejections(&tick(&mut app, vec![repair(refinery)])), NONE);
    ticks(&mut app, 12);
    let (hp, money) = (hit_points(&mut app, refinery), credits(&app, P0));
    assert!(hp > 300 && money < 1_000);
    assert_eq!(rejections(&tick(&mut app, vec![repair(refinery)])), NONE);
    ticks(&mut app, 20);
    assert_eq!(
        (hit_points(&mut app, refinery), credits(&app, P0)),
        (hp, money)
    );
}

// --- The depot ----------------------------------------------------------------------------------

/// P0 with a depot on (20, 20) to (21, 21) and a tank beside it at (22, 20) that lacks `lost`
/// hit points of its 300; one tick has run.
fn depot(credits: i64, lost: i32) -> (App, SimId, SimId) {
    let mut app = world(credits);
    place(&mut app, P0, "allied_power", 4, 4);
    let depot = place(&mut app, P0, "depot", 20, 20);
    let tank = spawn(&mut app, P0, "tank", 22, 20);
    set_hit_points(&mut app, tank, 300 - lost);
    tick(&mut app, Vec::new());
    (app, depot, tank)
}

#[test]
fn a_damaged_vehicle_docks_at_the_depot_and_is_repaired_for_credits() {
    // A tank of 400 credits and 300 hit points: 400 div (300 div 10) x 20% = 2 credits per
    // step of 10 hit points, every 4 ticks.
    let (mut app, depot, tank) = depot(1_000, 35);
    assert_eq!(rejections(&tick(&mut app, vec![dock(tank, depot)])), NONE);
    assert_eq!(docked(&app), [(tank, depot)]);
    assert_eq!(
        steps(&mut app, tank, 30),
        [(10, 2), (10, 2), (10, 2), (5, 2)]
    );
    assert_eq!(hit_points(&mut app, tank), 300);
    assert_eq!(credits(&app, P0), 1_000 - 8);
    // Whole, it has left the dock and made room.
    assert!(docked(&app).is_empty());
    ticks(&mut app, 60);
    let cell = cell_of(&mut app, tank);
    let footprint = sim::Footprint {
        origin: CellPos::new(20, 20),
        width: 2,
        height: 2,
    };
    assert_eq!(footprint.distance_to_cell(cell), 2, "{cell:?}");
}

#[test]
fn a_vehicle_far_from_the_depot_drives_there_first() {
    let (mut app, depot, _) = depot(1_000, 0);
    let far = spawn(&mut app, P0, "tank", 30, 30);
    set_hit_points(&mut app, far, 250);
    tick(&mut app, Vec::new());
    assert_eq!(rejections(&tick(&mut app, vec![dock(far, depot)])), NONE);
    assert!(docked(&app).is_empty(), "not there yet");
    ticks(&mut app, 200);
    assert_eq!(hit_points(&mut app, far), 300);
    assert_eq!(credits(&app, P0), 1_000 - 5 * 2);
}

#[test]
fn a_docked_vehicle_is_repaired_to_the_last_credit_and_waits_for_the_next() {
    // Four steps of 2 credits are due; 5 credits pay two, and the third waits with 1 left.
    let (mut app, depot, tank) = depot(5, 40);
    assert_eq!(rejections(&tick(&mut app, vec![dock(tank, depot)])), NONE);
    assert_eq!(steps(&mut app, tank, 40), [(10, 2); 2]);
    assert_eq!((hit_points(&mut app, tank), credits(&app, P0)), (280, 1));
    assert_eq!(docked(&app), [(tank, depot)], "it waits on the dock");
    // Exactly the credits of the two steps that are left.
    let bunker = place(&mut app, P0, "bunker", 30, 30);
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: bunker })],
    );
    assert_eq!(rejections(&events), NONE);
    ticks(&mut app, 12);
    assert_eq!(
        (hit_points(&mut app, tank), credits(&app, P0)),
        (300, 1 + 100 - 4)
    );
}

#[test]
fn no_vehicle_is_repaired_for_free_beside_a_depot() {
    // The free repair around a Service Depot of M9 is gone with a rules file.
    let (mut app, _, tank) = depot(1_000, 40);
    ticks(&mut app, 100);
    assert_eq!(
        (hit_points(&mut app, tank), credits(&app, P0)),
        (260, 1_000)
    );
}

#[test]
fn an_order_or_the_loss_of_the_depot_ends_the_repair() {
    // A move order.
    let (mut app, building, tank) = depot(1_000, 100);
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(tank, building)])),
        NONE
    );
    ticks(&mut app, 6);
    let hp = hit_points(&mut app, tank);
    assert!(hp > 200 && hp < 300);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![tank],
                target: CellPos::new(30, 20),
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    assert!(docked(&app).is_empty());
    let money = credits(&app, P0);
    ticks(&mut app, 40);
    assert_eq!((hit_points(&mut app, tank), credits(&app, P0)), (hp, money));
    // The depot sold.
    let (mut app, building, tank) = depot(1_000, 100);
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(tank, building)])),
        NONE
    );
    ticks(&mut app, 6);
    let hp = hit_points(&mut app, tank);
    assert_eq!(rejections(&tick(&mut app, vec![sell(building)])), NONE);
    let money = credits(&app, P0);
    ticks(&mut app, 40);
    assert!(docked(&app).is_empty());
    assert_eq!((hit_points(&mut app, tank), credits(&app, P0)), (hp, money));
}

#[test]
fn a_depot_takes_only_damaged_vehicles_of_its_owner() {
    let unavailable = |text| vec![RejectReason::AbilityUnavailable(text)];
    // An undamaged vehicle.
    let (mut app, building, tank) = depot(1_000, 0);
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(tank, building)])),
        unavailable("This unit is not damaged")
    );
    assert!(docked(&app).is_empty());
    // Infantry.
    let soldier = spawn(&mut app, P0, "rifleman", 22, 22);
    set_hit_points(&mut app, soldier, 10);
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(soldier, building)])),
        unavailable("Only vehicles are repaired here")
    );
    // Another player's vehicle is not mine to order.
    let foreign = spawn(&mut app, P1, "heavy", 19, 22);
    set_hit_points(&mut app, foreign, 100);
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(foreign, building)])),
        [RejectReason::NotOwned(foreign)]
    );
    // And another player's depot does not repair mine.
    let (mut app, _, tank) = depot(1_000, 50);
    let theirs = place(&mut app, P1, "depot", 24, 22);
    tick(&mut app, Vec::new());
    assert_eq!(
        rejections(&tick(&mut app, vec![dock(tank, theirs)])),
        unavailable("Choose your own repair building")
    );
    assert!(docked(&app).is_empty());
    ticks(&mut app, 20);
    assert_eq!(credits(&app, P0), 1_000);
}

#[test]
fn a_rejected_order_changes_no_state_and_a_dock_is_part_of_the_state() {
    // The rejected order of an undamaged tank leaves the hash of the same tick without it.
    let run = |lost: i32, order: bool| {
        let (mut app, building, tank) = depot(1_000, lost);
        // The order arrives in a tick that is no repair step (every 4 ticks).
        let commands = if order {
            vec![dock(tank, building)]
        } else {
            Vec::new()
        };
        let events = tick(&mut app, commands);
        (rejections(&events).len(), state_hash(app.world_mut()))
    };
    assert_eq!(run(0, true), (1, run(0, false).1));
    // The accepted order of a damaged one changes it: the dock is hashed (invariant I3).
    let (accepted, docked) = run(50, true);
    assert_eq!(accepted, 0);
    assert_ne!(docked, run(50, false).1);
}
