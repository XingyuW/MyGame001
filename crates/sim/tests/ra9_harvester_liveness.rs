//! Historical preparation note: UNVALIDATED / PENDING. Shipped native pre-Economy worker death and positive living twin.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Cell, CellPos, Command, Grid, Harvester};

fn ready_to_mine() -> (bevy_app::App, sim::SimId, sim::SimId) {
    let mut app = world();
    power(&mut app);
    let sphere = put(&mut app, P0, "chronosphere", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(12, 12),
        Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 40,
        },
    );
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(30, 30),
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    let worker = put(&mut app, P0, "war_miner", 12, 12);
    steps(&mut app, 2);
    charge(&mut app, sphere);
    let interval = defs(&app)
        .harvester(defs(&app).unit_kind("war_miner").unwrap())
        .unwrap()
        .harvest_interval;
    for _ in 0..2000 {
        let e = entity(&mut app, worker).unwrap();
        let h = app.world().get::<Harvester>(e).unwrap();
        if h.work + 1 == interval && h.load == 0 {
            return (app, sphere, worker);
        }
        tick(&mut app, vec![]);
    }
    panic!("native work cycle never reached its first actual harvest boundary");
}

#[test]
fn ra9_native_command_killed_worker_does_not_remove_ore_in_later_economy() {
    let (mut actual, sphere, worker) = ready_to_mine();
    let (mut control, _, control_worker) = ready_to_mine();
    assert_eq!(worker, control_worker);
    assert_eq!(
        sim::state_hash(actual.world_mut()),
        sim::state_hash(control.world_mut())
    );
    let at = CellPos::new(12, 12);
    let before = actual.world().resource::<Grid>().cell(at).unwrap().ore;
    assert_eq!(
        control.world().resource::<Grid>().cell(at).unwrap().ore,
        before
    );
    let events = tick(
        &mut actual,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sphere,
                source: at,
                target: CellPos::new(30, 30),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut actual, worker).is_none(),
        "real failed landing must actually kill worker"
    );
    tick(&mut control, vec![]);
    assert_eq!(
        actual.world().resource::<Grid>().cell(at).unwrap().ore,
        before,
        "a command-dead worker cannot mine later in Economy"
    );
    assert!(
        control.world().resource::<Grid>().cell(at).unwrap().ore < before,
        "living twin must actually complete the native harvest in this tick"
    );
    let e = entity(&mut control, control_worker).unwrap();
    assert!(control.world().get::<Harvester>(e).unwrap().load > 0);
    cells(&mut actual);
    cells(&mut control);
}
