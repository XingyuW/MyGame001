//! AIRDOCK-01: bounded occupied-dock waiting, followed by public release and real rearm.
//! Shipped definitions and ordinary CommandQueue/SimSchedule routes only. This does not
//! certify indefinite blocking, all geometry, or an original-engine retry policy.
mod ra4_common;
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    Building, CellPos, Command, Domain, Grid, Health, Mobile, Orders, Owner, ProductionItem,
    SimEvent, SimId, SpecialState,
};

fn air_mobile(app: &mut App, id: SimId) -> Mobile {
    let e = entity(app, id).expect("live aircraft");
    assert!(app.world().get::<Health>(e).unwrap().hit_points > 0);
    assert_eq!(app.world().get::<Owner>(e).unwrap().0, P0);
    *app.world().get::<Mobile>(e).unwrap()
}
fn air_tick(app: &mut App, commands: Vec<sim::IssuedCommand>) -> Vec<SimEvent> {
    let events = tick(app, commands);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    ra4_common::assert_cells(app, "AIRDOCK-01 every native tick");
    events
}
fn air_move(id: SimId, target: CellPos) -> sim::IssuedCommand {
    issue(
        P0,
        Command::Move {
            units: vec![id],
            target,
        },
    )
}
fn air_ammo(app: &App, plane: SimId) -> u32 {
    app.world().resource::<SpecialState>().ammo_of(plane, 1)
}

#[test]
fn ra9_airdock_occupied_cell_public_release_allows_native_return_and_rearm() {
    let mut app = world();
    let definitions = defs(&app);
    let harrier = definitions.unit_kind("harrier").unwrap();
    let rocketeer = definitions.unit_kind("rocketeer").unwrap();
    let aircraft = definitions.metadata(ProductionItem::Unit(harrier)).unwrap();
    let blocker_def = definitions
        .metadata(ProductionItem::Unit(rocketeer))
        .unwrap();
    assert!(aircraft.airport_bound);
    assert_eq!(aircraft.ammo, 1);
    assert!(
        !blocker_def.airport_bound,
        "blocker must not steal a dock assignment"
    );
    let interval = definitions.rules().unwrap().economy.reload_interval;
    assert_eq!(interval, 360);
    let pad = put(&mut app, P0, "airforce_command", 10, 10);
    let pad_entity = entity(&mut app, pad).unwrap();
    let dock = sim::dock_cell(
        app.world().get::<Building>(pad_entity).unwrap().footprint,
        0,
    );
    assert_eq!(dock, CellPos::new(10, 10));
    let plane = put(&mut app, P0, "harrier", dock.x, dock.y);
    let blocker = put(&mut app, P0, "rocketeer", 8, 10);
    // MCV has no weapon. At the dock it is outside Harrier range; on the outbound
    // move it is inside sight/range before the ordinary move finishes.
    let target = put(&mut app, P1, "mcv", 18, 10);
    air_tick(&mut app, vec![]);
    assert_eq!(
        app.world().resource::<SpecialState>().docks.get(&plane),
        Some(&(pad, 0))
    );
    assert_eq!(air_ammo(&app, plane), 1);
    let mut events = air_tick(
        &mut app,
        vec![
            air_move(plane, CellPos::new(14, 10)),
            air_move(blocker, dock),
        ],
    );
    let mut ready = false;
    // Existing pad-native movement controls use 400 ticks for an out-and-back path.
    // This is a bounded fixture observation, not a new gameplay timeout.
    for _ in 0..400 {
        assert!(
            !events.iter().any(|e| matches!(e,
            SimEvent::WeaponFired { attacker, .. } if *attacker == plane)),
            "outbound setup must not spend ammunition before explicit Attack"
        );
        let a = air_mobile(&mut app, plane);
        let b = air_mobile(&mut app, blocker);
        if b.cell == dock && b.step.is_none() && a.cell.x >= 13 && a.cell.x < 14 {
            ready = true;
            break;
        }
        events = air_tick(&mut app, vec![]);
    }
    assert!(
        ready,
        "legitimate blocker must land before outbound plane finishes moving"
    );
    assert_eq!(air_ammo(&app, plane), 1);
    let firing_cell = air_mobile(&mut app, plane).cell;
    events = air_tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![plane],
                target,
            },
        )],
    );
    let mut fired = false;
    for _ in 0..400 {
        fired |= events.iter().any(|e| {
            matches!(e,
            SimEvent::WeaponFired { attacker, target: found, .. }
                if *attacker == plane && *found == target)
        });
        if fired {
            break;
        }
        events = air_tick(&mut app, vec![]);
    }
    assert!(fired, "real public Attack must spend the stock volley");
    assert_eq!(air_ammo(&app, plane), 0);
    let mut approached = false;
    let mut requested_dock = false;
    for _ in 0..interval {
        air_tick(&mut app, vec![]);
        let a = air_mobile(&mut app, plane);
        let b = air_mobile(&mut app, blocker);
        assert_eq!((b.cell, b.step), (dock, None));
        let e = entity(&mut app, plane).unwrap();
        requested_dock |= app
            .world()
            .get::<Orders>(e)
            .unwrap()
            .current
            .as_ref()
            .is_some_and(|order| order.destination == dock);
        approached |= a.cell.x < firing_cell.x;
        assert_ne!(a.cell, dock, "occupied air cell cannot be a landing");
        let state = app.world().resource::<SpecialState>();
        assert_eq!(state.docks.get(&plane), Some(&(pad, 0)));
        assert!(
            !state.reloading.contains_key(&plane),
            "blocked approach cannot bank reload time"
        );
        assert_eq!(air_ammo(&app, plane), 0);
        let held = app
            .world()
            .resource::<Grid>()
            .holders_for(dock, Domain::Air);
        assert!(held.contains(&Some(blocker)));
        assert!(!held.contains(&Some(plane)));
    }
    assert!(
        approached && requested_dock,
        "actual ammo-return approach and dock destination"
    );
    air_tick(&mut app, vec![air_move(blocker, CellPos::new(7, 10))]);
    let mut started = false;
    for _ in 0..400 {
        air_tick(&mut app, vec![]);
        let a = air_mobile(&mut app, plane);
        air_mobile(&mut app, blocker);
        assert_eq!(air_ammo(&app, plane), 0);
        if app.world().resource::<SpecialState>().reloading.get(&plane) == Some(&1) {
            assert_eq!((a.cell, a.step), (dock, None));
            let e = entity(&mut app, plane).unwrap();
            assert!(app.world().get::<Orders>(e).unwrap().current.is_none());
            assert!(
                app.world()
                    .resource::<Grid>()
                    .holders_for(dock, Domain::Air)
                    .contains(&Some(plane))
            );
            started = true;
            break;
        }
    }
    assert!(
        started,
        "public blocker release must permit genuine landing and first service tick"
    );
    for elapsed in 2..interval {
        air_tick(&mut app, vec![]);
        assert_eq!(air_ammo(&app, plane), 0);
        assert_eq!(
            app.world().resource::<SpecialState>().reloading.get(&plane),
            Some(&(elapsed as u32))
        );
        assert_eq!(air_mobile(&mut app, plane).cell, dock);
    }
    air_tick(&mut app, vec![]);
    assert_eq!(
        air_ammo(&app, plane),
        1,
        "only a fresh full landed interval rearms"
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .reloading
            .contains_key(&plane)
    );
    assert_eq!(
        app.world().resource::<SpecialState>().docks.get(&plane),
        Some(&(pad, 0))
    );
    assert_eq!(air_mobile(&mut app, plane).cell, dock);
    let b = air_mobile(&mut app, blocker);
    assert_eq!((b.cell, b.step), (CellPos::new(7, 10), None));
}
