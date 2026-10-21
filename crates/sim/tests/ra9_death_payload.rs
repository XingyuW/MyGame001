//! UNVALIDATED / PENDING. Proposed crates/sim/tests/ra9_death_payload.rs.
//! Exact source witness; no direct HP-zero destruction. The native C4 charge kills the reactor.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{CellPos, Command, SimEvent, SpecialState};

#[test]
fn ra9_native_reactor_death_scales_only_payload_damage_keeps_source_radiation() {
    let mut app = world();
    let reactor = put(&mut app, P1, "nuclear_reactor", 8, 8);
    let witness = put(&mut app, P0, "apocalypse", 11, 8);
    // A passive measuring body must not fire at the reactor while C4's clock runs.
    let e = entity(&mut app, witness).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    let tanya = put(&mut app, P0, "tanya", 7, 8);
    steps(&mut app, 2);
    let before = hp(&mut app, witness);
    let events = interact(&mut app, tanya, reactor);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        app.world()
            .resource::<SpecialState>()
            .charges
            .contains_key(&reactor)
    );
    let mut death = events
        .iter()
        .any(|e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building==reactor));
    for _ in 0..40 {
        if death {
            break;
        }
        let events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty(), "{events:?}");
        death = events
            .iter()
            .any(|e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building==reactor));
    }
    assert!(
        death,
        "actual scheduled C4 must destroy the full-strength reactor"
    );
    assert!(entity(&mut app, reactor).is_none());
    // Independent reference arithmetic: NUKE range10 cells, edge2%, distance3 cells,
    // heavy100%, Damage600 x reactor DeathWeaponDamageModifier0.5.
    let expected = 300_i64 * (100 * 2560 - 98 * 768) / (100 * 2560);
    assert_eq!(expected, 211);
    assert_eq!(before - hp(&mut app, witness), expected as i32);
    let site = app
        .world()
        .resource::<SpecialState>()
        .radiation_sites
        .iter()
        .find(|site| site.source == Some(reactor))
        .expect("reactor radiation site");
    assert_eq!(site.level, 500);
    assert_eq!(site.spread, 2560);
    cells(&mut app);
}

#[test]
fn ra9_native_strategic_missile_retains_full_six_hundred_base_damage() {
    let mut app = world();
    power(&mut app);
    let silo = put(&mut app, P0, "nuclear_silo", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let witness = put(&mut app, P1, "apocalypse", 23, 20);
    let e = entity(&mut app, witness).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    steps(&mut app, 2);
    charge(&mut app, silo);
    let before = hp(&mut app, witness);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: silo,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    steps(&mut app, 199);
    assert_eq!(
        hp(&mut app, witness),
        before,
        "no payload before exact flight delay"
    );
    steps(&mut app, 1);
    let expected = 600_i64 * (100 * 2560 - 98 * 768) / (100 * 2560);
    assert_eq!(expected, 423);
    assert_eq!(before - hp(&mut app, witness), expected as i32);
    let definitions = defs(&app);
    assert_eq!(
        definitions
            .weapon(definitions.weapon_kind("nuke_payload").unwrap())
            .unwrap()
            .damage,
        600
    );
    cells(&mut app);
}
