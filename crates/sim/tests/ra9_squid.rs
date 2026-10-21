//! UNVALIDATED / PENDING. Native R83 attached-host firing witness.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Cell, Command, Grid, SimEvent, SpecialState};

#[test]
fn ra9_attached_squid_host_actually_fires_at_another_hostile_target() {
    let mut app = world();
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: false,
                    water: true,
                    buildable: false,
                    ore: 0,
                };
                48 * 48
            ],
        )
        .unwrap(),
    );
    let squid = put(&mut app, P0, "giant_squid", 8, 8);
    let host = put(&mut app, P1, "destroyer", 9, 8);
    steps(&mut app, 1);
    let events = attack(&mut app, squid, host);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, squid).is_none(),
        "real parasite attachment, not a seeded component"
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .parasites
            .contains_key(&host)
    );
    let target = put(&mut app, P0, "amphibious_transport", 12, 8);
    let e = entity(&mut app, target).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    let position = cell(&mut app, host);
    let target_hp = hp(&mut app, target);
    let mut events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![host],
                target,
            },
        )],
    );
    let mut fired = false;
    for _ in 0..240 {
        assert!(
            app.world()
                .resource::<SpecialState>()
                .parasites
                .contains_key(&host),
            "shot must occur while the original Squid remains attached"
        );
        assert_eq!(cell(&mut app, host), position);
        fired |= events.iter().any(|event| {
            matches!(event,
            SimEvent::WeaponFired { attacker, target: found, .. }
                if *attacker == host && *found == target)
        });
        if fired && hp(&mut app, target) < target_hp {
            cells(&mut app);
            return;
        }
        events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty(), "{events:?}");
    }
    panic!("attached destroyer emitted no actual WeaponFired to its distinct hostile target");
}
