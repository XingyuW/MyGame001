//! Building repair (`Command::RepairBuilding`): hit points restored in paid steps, with the
//! legacy ruleset fixture (`tests/fixtures/data/legacy`; docs/DECISIONS.md R15).
use bevy_app::App;
use bevy_ecs::{entity::Entity, message::Messages};
use sim::*;
use std::path::PathBuf;

const P: PlayerId = PlayerId(0);
const E: PlayerId = PlayerId(1);

fn app(credits: i64) -> App {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
    let mut a = App::new();
    a.add_plugins(SimPlugin { seed: 11 });
    let mut grid =
        load_map(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo/demo.tmx"))
            .unwrap()
            .grid;
    for y in 0..32 {
        for x in 0..32 {
            grid.set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0,
                },
            );
        }
    }
    a.world_mut().insert_resource(grid);
    a.world_mut().insert_resource(
        load_match_definitions(
            data.join("units.ron"),
            data.join("economy.ron"),
            data.join("combat.ron"),
        )
        .unwrap()
        .with_arsenal(data.join("arsenal.ron"))
        .unwrap(),
    );
    for p in [P, E] {
        a.world_mut()
            .resource_mut::<Players>()
            .add(p, credits)
            .unwrap();
    }
    a
}

fn tick(a: &mut App, n: u32) {
    for _ in 0..n {
        a.world_mut().run_schedule(SimSchedule);
    }
}

fn issue(a: &mut App, player: PlayerId, command: Command) {
    a.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand { player, command });
    tick(a, 1);
}

fn entity(a: &mut App, id: SimId) -> Entity {
    a.world_mut()
        .query::<(Entity, &SimId)>()
        .iter(a.world())
        .find(|(_, i)| **i == id)
        .unwrap()
        .0
}

fn hp(a: &mut App, id: SimId) -> i32 {
    let e = entity(a, id);
    a.world().get::<Health>(e).unwrap().hit_points
}

fn set_hp(a: &mut App, id: SimId, value: i32) {
    let e = entity(a, id);
    a.world_mut().get_mut::<Health>(e).unwrap().hit_points = value;
}

fn credits(a: &App, player: PlayerId) -> i64 {
    a.world()
        .resource::<Players>()
        .get(player)
        .unwrap()
        .credits()
}

fn repairing(a: &App, player: PlayerId) -> Vec<SimId> {
    a.world()
        .resource::<Players>()
        .get(player)
        .unwrap()
        .repairing()
        .iter()
        .copied()
        .collect()
}

fn rejected(a: &App) -> bool {
    a.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
}

/// A damaged power plant of `P` with its maximum hit points and price.
fn damaged_plant(a: &mut App) -> (SimId, i32, i64) {
    let plant = spawn_building(a.world_mut(), P, "power_plant", CellPos::new(10, 10)).unwrap();
    let defs = a.world().resource::<Definitions>();
    let kind = defs.building_kind("power_plant").unwrap();
    let max = defs.building_combat(kind).unwrap().hit_points;
    let price = defs.building(kind).unwrap().production.cost;
    set_hp(a, plant, max / 4);
    (plant, max, price)
}

#[test]
fn repair_restores_a_building_for_a_quarter_of_its_price_and_then_stops() {
    let mut a = app(100_000);
    let (plant, max, price) = damaged_plant(&mut a);
    let before = credits(&a, P);
    issue(&mut a, P, Command::RepairBuilding { building: plant });
    assert!(!rejected(&a));
    assert_eq!(repairing(&a, P), vec![plant]);
    tick(&mut a, 40);
    let partway = hp(&mut a, plant);
    assert!(partway > max / 4 && partway < max, "{partway} of {max}");
    // 80 steps from nothing to whole; three quarters is at most 60 steps of 5 ticks.
    tick(&mut a, 60 * REPAIR_INTERVAL as u32);
    assert_eq!(hp(&mut a, plant), max);
    assert!(repairing(&a, P).is_empty(), "the repair ends when whole");
    let spent = before - credits(&a, P);
    let expected = price * 3 / 4 / REPAIR_COST_DIVISOR;
    // Every step rounds up, so the total may exceed the exact share by at most a credit a step.
    assert!(
        spent >= expected && spent <= expected + i64::from(REPAIR_STEPS),
        "spent {spent}, expected about {expected}"
    );
    // Nothing more is charged once the building is whole.
    let after = credits(&a, P);
    tick(&mut a, 50);
    assert_eq!(credits(&a, P), after);
}

#[test]
fn a_second_repair_order_stops_the_repair() {
    let mut a = app(100_000);
    let (plant, max, _) = damaged_plant(&mut a);
    issue(&mut a, P, Command::RepairBuilding { building: plant });
    tick(&mut a, 20);
    issue(&mut a, P, Command::RepairBuilding { building: plant });
    assert!(repairing(&a, P).is_empty());
    let stopped = hp(&mut a, plant);
    let money = credits(&a, P);
    tick(&mut a, 100);
    assert_eq!(hp(&mut a, plant), stopped);
    assert!(stopped < max);
    assert_eq!(credits(&a, P), money);
}

#[test]
fn only_the_owner_may_repair_and_a_broke_player_waits_for_credits() {
    let mut a = app(0);
    let (plant, max, _) = damaged_plant(&mut a);
    issue(&mut a, E, Command::RepairBuilding { building: plant });
    assert!(rejected(&a));
    assert!(repairing(&a, E).is_empty());
    issue(&mut a, P, Command::RepairBuilding { building: plant });
    tick(&mut a, 100);
    assert_eq!(hp(&mut a, plant), max / 4, "no credits, no repair");
    assert_eq!(credits(&a, P), 0);
    assert_eq!(repairing(&a, P), vec![plant], "the repair waits");
    // Selling another building pays for the repair.
    let spare = spawn_building(a.world_mut(), P, "barracks", CellPos::new(20, 10)).unwrap();
    issue(&mut a, P, Command::SellBuilding { building: spare });
    assert!(credits(&a, P) > 0);
    tick(&mut a, 20);
    assert!(hp(&mut a, plant) > max / 4);
}

#[test]
fn selling_a_building_under_repair_ends_the_repair() {
    let mut a = app(100_000);
    let (plant, ..) = damaged_plant(&mut a);
    issue(&mut a, P, Command::RepairBuilding { building: plant });
    issue(&mut a, P, Command::SellBuilding { building: plant });
    assert!(repairing(&a, P).is_empty());
}
