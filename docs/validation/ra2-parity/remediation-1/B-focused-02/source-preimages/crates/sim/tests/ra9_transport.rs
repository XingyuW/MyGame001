//! Historical preparation note: UNVALIDATED / PENDING. Native scheduled shipped transport regressions.
//! Historical preparation note: Proposed integration path: crates/sim/tests/ra9_transport.rs.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Command, ProductionItem, SimId, Weapon};

fn full_landing_craft() -> (bevy_app::App, SimId, SimId) {
    let mut app = world();
    let craft = put(&mut app, P0, "amphibious_transport", 14, 14);
    assert_eq!(
        defs(&app)
            .metadata(ProductionItem::Unit(
                defs(&app).unit_kind("amphibious_transport").unwrap()
            ))
            .unwrap()
            .size_limit,
        6
    );
    for _ in 0..4 {
        let tank = put(&mut app, P0, "grizzly", 13, 14);
        let events = interact(&mut app, tank, craft);
        assert!(rejected(&events).is_empty(), "{events:?}");
        assert!(entity(&mut app, tank).is_none());
        cells(&mut app);
    }
    assert_eq!(cargo(&app, craft).len(), 4);
    let fifth = put(&mut app, P0, "grizzly", 13, 14);
    (app, craft, fifth)
}

#[test]
fn ra9_weighted_capacity_accepts_exact_twelve_and_refuses_thirteen_or_more_without_state_change() {
    // Four Size=3 tanks use all Passengers=12 slots. The old count-only rule accepts five.
    let (mut actual, craft, fifth) = full_landing_craft();
    let (mut control, same_craft, same_fifth) = full_landing_craft();
    assert_eq!((craft, fifth), (same_craft, same_fifth));
    let events = interact(&mut actual, fifth, craft);
    tick(&mut control, vec![]);
    assert_eq!(rejected(&events).len(), 1, "{events:?}");
    assert_eq!(cargo(&actual, craft).len(), 4);
    assert!(entity(&mut actual, fifth).is_some());
    assert_eq!(
        sim::state_hash(actual.world_mut()),
        sim::state_hash(control.world_mut())
    );
    cells(&mut actual);
}

#[test]
fn ra9_flak_track_uses_size_two_drones_and_exact_five_capacity() {
    let mut app = world();
    let track = put(&mut app, P0, "flak_track", 14, 14);
    for passenger in ["terror_drone", "terror_drone", "gi"] {
        let id = put(&mut app, P0, passenger, 13, 14);
        let events = interact(&mut app, id, track);
        assert!(rejected(&events).is_empty(), "{passenger}: {events:?}");
        assert!(entity(&mut app, id).is_none());
    }
    assert_eq!(
        cargo(&app, track).len(),
        3,
        "2+2+1 uses all five size slots"
    );
    let id = put(&mut app, P0, "gi", 13, 14);
    assert_eq!(rejected(&interact(&mut app, id, track)).len(), 1);
    cells(&mut app);
}

#[test]
fn ra9_landing_craft_refuses_oversize_and_airborne_passengers() {
    for passenger in ["amphibious_transport", "rocketeer"] {
        let mut app = world();
        let craft = put(&mut app, P0, "amphibious_transport", 14, 14);
        let id = put(&mut app, P0, passenger, 13, 14);
        let events = interact(&mut app, id, craft);
        assert_eq!(rejected(&events).len(), 1, "{passenger}: {events:?}");
        assert!(cargo(&app, craft).is_empty());
        assert!(entity(&mut app, id).is_some());
        cells(&mut app);
    }
}

#[test]
fn ra9_loaded_flak_track_keeps_five_passengers_identity_health_rank_and_direct_child_weight() {
    let mut app = world();
    let craft = put(&mut app, P0, "amphibious_transport", 11, 10);
    let track = put(&mut app, P0, "flak_track", 10, 10);
    let mut crew = Vec::new();
    for _ in 0..5 {
        let soldier = put(&mut app, P0, "gi", 9, 10);
        // Positive pre-existing injury/rank are scenario setup, never a death injector.
        set_hp(&mut app, soldier, 37);
        assert!(sim::promote(app.world_mut(), soldier, sim::VETERAN));
        let events = interact(&mut app, soldier, track);
        assert!(rejected(&events).is_empty(), "{events:?}");
        crew.push(soldier);
    }
    assert_eq!(cargo(&app, track), crew);
    let events = interact(&mut app, track, craft);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, track).is_none());
    steps(&mut app, 3); // In particular cross advance's cargo retention, not just Commands.
    assert_eq!(cargo(&app, track), crew);
    for _ in 0..3 {
        let tank = put(&mut app, P0, "grizzly", 10, 10);
        let events = interact(&mut app, tank, craft);
        assert!(
            rejected(&events).is_empty(),
            "inner payload must not add to outer weight: {events:?}"
        );
    }
    assert_eq!(cargo(&app, craft).len(), 4, "3 + 3 + 3 + 3 = 12");
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: craft })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, track).is_some());
    assert_eq!(cargo(&app, track), crew);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: track })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(cargo(&app, track).is_empty());
    for soldier in crew {
        assert_eq!(hp(&mut app, soldier), 37);
        let e = entity(&mut app, soldier).unwrap();
        assert_eq!(
            app.world().get::<sim::Experience>(e).unwrap().rank,
            sim::VETERAN
        );
        assert_eq!(owner(&mut app, soldier), Some(P0));
    }
    cells(&mut app);
}

#[test]
fn ra9_loaded_ifv_recreates_its_engineer_weapon_when_restored() {
    let mut app = world();
    let craft = put(&mut app, P0, "amphibious_transport", 11, 10);
    let ifv = put(&mut app, P0, "ifv", 10, 10);
    let engineer = put(&mut app, P0, "engineer", 9, 10);
    assert!(rejected(&interact(&mut app, engineer, ifv)).is_empty());
    let e = entity(&mut app, ifv).unwrap();
    let mode = app.world().get::<Weapon>(e).unwrap().kind;
    assert!(rejected(&interact(&mut app, ifv, craft)).is_empty());
    steps(&mut app, 2);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: craft })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(cargo(&app, ifv), vec![engineer]);
    let e = entity(&mut app, ifv).unwrap();
    assert_eq!(app.world().get::<Weapon>(e).unwrap().kind, mode);
    cells(&mut app);
}
