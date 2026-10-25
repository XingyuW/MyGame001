//! Historical preparation note: UNVALIDATED / PENDING. Native lifecycle supplement for ra9_transport.rs.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Cell, CellPos, Command, Grid, SpecialState};

fn nested_payload() -> (bevy_app::App, sim::SimId, sim::SimId, sim::SimId) {
    let mut app = world();
    let outer = put(&mut app, P0, "amphibious_transport", 11, 10);
    let inner = put(&mut app, P0, "flak_track", 10, 10);
    let soldier = put(&mut app, P0, "gi", 9, 10);
    assert!(rejected(&interact(&mut app, soldier, inner)).is_empty());
    assert!(rejected(&interact(&mut app, inner, outer)).is_empty());
    steps(&mut app, 2);
    assert_eq!(cargo(&app, outer), vec![inner]);
    assert_eq!(cargo(&app, inner), vec![soldier]);
    (app, outer, inner, soldier)
}

#[test]
fn ra9_real_control_capture_and_nested_unloads_propagate_root_owner() {
    let (mut app, outer, inner, soldier) = nested_payload();
    let yuri = put(&mut app, P1, "yuri", 15, 10);
    steps(&mut app, 1);
    let mut events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![yuri],
                target: outer,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..300 {
        if owner(&mut app, outer) == Some(P1) {
            break;
        }
        events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty(), "{events:?}");
    }
    assert_eq!(
        owner(&mut app, outer),
        Some(P1),
        "native Controller shot must actually capture root"
    );
    assert_eq!(
        app.world().resource::<SpecialState>().controlled[&outer].0,
        yuri
    );
    let events = tick(&mut app, vec![issue(P1, Command::Deploy { unit: outer })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(owner(&mut app, inner), Some(P1));
    assert_eq!(cargo(&app, inner), vec![soldier]);
    let events = tick(&mut app, vec![issue(P1, Command::Deploy { unit: inner })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(owner(&mut app, soldier), Some(P1));
    cells(&mut app);
}

#[test]
fn ra9_real_invalid_chronoshift_kills_root_and_every_stored_descendant() {
    let (mut app, outer, inner, soldier) = nested_payload();
    power(&mut app);
    let sphere = put(&mut app, P0, "chronosphere", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let target = CellPos::new(30, 30);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        target,
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    steps(&mut app, 2);
    charge(&mut app, sphere);
    let source = cell(&mut app, outer);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sphere,
                source,
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, outer).is_none(),
        "real failed landing must kill carrier"
    );
    for id in [outer, inner, soldier] {
        assert!(entity(&mut app, id).is_none());
        let state = app.world().resource::<SpecialState>();
        assert!(!state.cargo.contains_key(&id));
        assert!(!state.cargo.values().flatten().any(|p| p.id == id));
    }
    steps(&mut app, 2);
    assert!(
        app.world().resource::<SpecialState>().cargo.is_empty(),
        "no orphan payload revival"
    );
    cells(&mut app);
}

#[test]
fn ra9_off_map_descendant_cannot_be_named_to_create_a_transport_cycle() {
    let (mut app, outer, inner, _) = nested_payload();
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: outer,
                target: inner,
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, sim::SimEvent::CommandRejected {
        reason: sim::RejectReason::UnknownTarget(id), ..
    } if *id == inner))
    );
    assert_eq!(cargo(&app, outer), vec![inner]);
    cells(&mut app);
}

#[test]
fn ra9_fully_blocked_unload_retains_payload_until_a_normal_move_frees_a_cell() {
    let mut app = world();
    let carrier = put(&mut app, P0, "amphibious_transport", 14, 14);
    let soldier = put(&mut app, P0, "gi", 13, 14);
    set_hp(&mut app, soldier, 37);
    assert!(sim::promote(app.world_mut(), soldier, sim::VETERAN));
    assert!(rejected(&interact(&mut app, soldier, carrier)).is_empty());
    let mut mover = None;
    for y in 13..=15 {
        for x in 13..=15 {
            if (x, y) == (14, 14) {
                continue;
            }
            let id = put(&mut app, P0, "grizzly", x, y);
            if (x, y) == (13, 14) {
                mover = Some(id);
            }
        }
    }
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: carrier })]);
    assert!(
        rejected(&events).is_empty(),
        "blocked placement keeps cargo, does not reject deploy"
    );
    assert_eq!(cargo(&app, carrier), vec![soldier]);
    assert!(entity(&mut app, soldier).is_none());
    let mover = mover.unwrap();
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![mover],
                target: CellPos::new(12, 14),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..200 {
        let e = entity(&mut app, mover).unwrap();
        let mobile = app.world().get::<sim::Mobile>(e).unwrap();
        if mobile.cell == CellPos::new(12, 14) && mobile.step.is_none() {
            break;
        }
        tick(&mut app, vec![]);
    }
    assert_eq!(cell(&mut app, mover), CellPos::new(12, 14));
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: carrier })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(cargo(&app, carrier).is_empty());
    assert_eq!(hp(&mut app, soldier), 37);
    let e = entity(&mut app, soldier).unwrap();
    assert_eq!(
        app.world().get::<sim::Experience>(e).unwrap().rank,
        sim::VETERAN
    );
    cells(&mut app);
}
