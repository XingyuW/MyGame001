//! UNVALIDATED / PENDING. Append to native ra9_liveness test after initial draft compiles.
mod ra9_soundness_common;
use bevy_ecs::{resource::Resource, schedule::IntoScheduleConfigs, world::World};
use ra9_soundness_common::*;
use sim::{Cell, CellPos, Command, Grid, SimCoreSet, SimId, SimSchedule, SimSet, SubPos};

#[derive(Resource)]
struct Trace {
    unit: SimId,
    after_movement: Option<(SubPos, i32)>,
}
fn record_after_movement(world: &mut World) {
    let id = world.resource::<Trace>().unit;
    let found = world
        .query::<(&SimId, &sim::Position, &sim::Health)>()
        .iter(world)
        .find(|(other, ..)| **other == id)
        .map(|(_, p, h)| (p.0, h.hit_points));
    world.resource_mut::<Trace>().after_movement = found;
}
fn moving_pair_member() -> (bevy_app::App, SimId, SimId) {
    let mut app = world();
    power(&mut app);
    let sphere = put(&mut app, P0, "chronosphere", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(30, 30),
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    let tank = put(&mut app, P0, "grizzly", 12, 12);
    steps(&mut app, 2);
    charge(&mut app, sphere);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![tank],
                target: CellPos::new(20, 12),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let e = entity(&mut app, tank).unwrap();
    assert!(app.world().get::<sim::Mobile>(e).unwrap().step.is_some());
    app.insert_resource(Trace {
        unit: tank,
        after_movement: None,
    });
    app.add_systems(
        SimSchedule,
        record_after_movement
            .in_set(SimSet::Movement)
            .after(SimCoreSet(SimSet::Movement)),
    );
    (app, sphere, tank)
}
#[test]
fn ra9_shipped_command_killed_half_step_unit_cannot_move_before_cleanup() {
    let (mut actual, sphere, tank) = moving_pair_member();
    let (mut control, _, _) = moving_pair_member();
    let e = entity(&mut actual, tank).unwrap();
    let before = actual.world().get::<sim::Position>(e).unwrap().0;
    let source = actual.world().get::<sim::Mobile>(e).unwrap().cell;
    let events = tick(
        &mut actual,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sphere,
                source,
                target: CellPos::new(30, 30),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    tick(&mut control, vec![]);
    let (position, hp) = actual.world().resource::<Trace>().after_movement.unwrap();
    assert!(
        hp <= 0,
        "native failed landing must actually kill before Movement"
    );
    assert_eq!(
        position, before,
        "dead mover must stay at the position of death"
    );
    assert_ne!(
        control
            .world()
            .resource::<Trace>()
            .after_movement
            .unwrap()
            .0,
        before,
        "living control must actually advance in the same stage"
    );
    assert!(entity(&mut actual, tank).is_none());
    cells(&mut actual);
    cells(&mut control);
}
