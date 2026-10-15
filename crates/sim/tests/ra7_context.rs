//! Context labels share production Interact validation and never expose a concealed target.
mod ra7_common;
use ra7_common::*;
use sim::{CellPos, Command, SimEvent};
#[test]
fn ra7_context_actions_validate_engineer_spy_boarding_depot_garrison_and_tanya() {
    let mut app = world();
    let engineer = put(&mut app, P0, "engineer", 5, 5);
    let spy = put(&mut app, P0, "spy", 6, 5);
    let gi = put(&mut app, P0, "gi", 7, 5);
    let tanya = put(&mut app, P0, "tanya", 8, 5);
    let tank = put(&mut app, P0, "grizzly", 9, 5);
    let friendly = put(&mut app, P0, "power_plant", 5, 8);
    let enemy = put(&mut app, P1, "power_plant", 10, 8);
    let infantry = put(&mut app, P1, "conscript", 10, 6);
    let transport = put(&mut app, P0, "ifv", 7, 6);
    let depot = put(&mut app, P0, "service_depot", 12, 4);
    let e = entity(&mut app, tank).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 100;
    app.world_mut().resource_mut::<sim::Grid>().set_terrain(
        CellPos::new(7, 9),
        sim::Cell {
            passable: false,
            buildable: false,
            ..OPEN
        },
    );
    let mut looks = vec![0; 48 * 48];
    looks[9 * 48 + 7] = 5 << 4;
    app.world_mut()
        .resource_mut::<sim::Grid>()
        .mark_world_features(&looks);
    sim::install_world_objects(app.world_mut(), true);
    let house = app
        .world()
        .resource::<sim::Props>()
        .id_at(CellPos::new(7, 9))
        .unwrap();
    // Observer scenario setup stops incidental firefights; command path remains unchanged.
    for id in [gi, tanya, tank, infantry] {
        let e = entity(&mut app, id).unwrap();
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    }
    tick(&mut app, vec![]);
    for (unit, target, label) in [
        (engineer, friendly, "REPAIR"),
        (engineer, enemy, "CAPTURE"),
        (spy, enemy, "INFILTRATE"),
        (spy, infantry, "DISGUISE"),
        (gi, transport, "BOARD"),
        (tank, depot, "REPAIR"),
        (gi, house, "GARRISON"),
        (tanya, enemy, "C4"),
    ] {
        assert_eq!(
            sim::special::interaction_label(app.world(), P0, unit, target),
            Some(label),
            "{label}"
        );
    }
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: gi,
                target: transport,
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    let hidden = put(&mut app, P1, "spy", 9, 7);
    tick(&mut app, vec![]);
    assert_eq!(
        sim::special::interaction_label(app.world(), P0, engineer, hidden),
        None
    );
    assert_eq!(
        sim::special::interaction_label(app.world(), P0, engineer, sim::SimId(99999)),
        None
    );
    let e = entity(&mut app, engineer).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    assert_eq!(
        sim::special::interaction_label(app.world(), P0, engineer, enemy),
        None
    );
}
