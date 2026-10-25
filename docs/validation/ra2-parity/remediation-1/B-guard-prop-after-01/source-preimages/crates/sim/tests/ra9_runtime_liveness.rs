//! Round-one remediation controls: native parasite damage and effective Guard acquisition.
//! Defective-source failures and corrected-source checks: docs/validation/ra2-parity/remediation-1/.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Command, SimEvent, SimId, SimTick, SpecialState, Weapon};

fn disarm(app: &mut bevy_app::App, id: SimId) {
    let e = entity(app, id).expect("living passive fixture actor");
    app.world_mut().entity_mut(e).remove::<Weapon>();
}

/// Real native Drone attachment and due timer; Engineer stays on map until due Commands.
/// Positive HP is controlled initial damage, never a direct dead-actor setup.
fn due_ifv_case(healer_is_host: bool, host_hp: i32) -> (bevy_app::App, SimId, SimId, SimId) {
    assert!(host_hp > 0);
    let mut app = world();
    let healer = put(&mut app, P0, "ifv", 10, 10);
    let patient = put(&mut app, P0, "grizzly", 10, 11);
    let engineer = put(&mut app, P0, "engineer", 9, 10);
    // Both positive-HP targets are passive during attachment; public boarding restores the
    // shipped IFV RepairBullet through the actual gunner mode path.
    disarm(&mut app, healer);
    disarm(&mut app, patient);
    set_hp(&mut app, patient, 100);
    let host = if healer_is_host { healer } else { patient };
    set_hp(&mut app, host, host_hp);
    put(&mut app, P1, "engineer", 8, 9);
    tick(&mut app, vec![]); // actual P1 sight before Drone Attack; no parasite exists yet
    let drone = put(&mut app, P1, "terror_drone", 9, 11);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![drone],
                target: host,
            },
        )],
    );
    assert!(
        rejected(&events).is_empty(),
        "native attachment command rejected: {events:?}"
    );
    let mut attached = false;
    for _ in 0..300 {
        if app
            .world()
            .resource::<SpecialState>()
            .parasites
            .contains_key(&host)
        {
            attached = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(attached, "native Drone never attached");
    assert!(
        entity(&mut app, drone).is_none(),
        "parasite must actually leave ECS"
    );
    let parasite = app.world().resource::<SpecialState>().parasites[&host].clone();
    assert_eq!(parasite.passenger.id, drone);
    assert!(!parasite.squid);
    assert_eq!(
        hp(&mut app, host),
        host_hp,
        "must still be positive before first native hit"
    );
    for _ in 0..2000 {
        if app.world().resource::<SimTick>().0 == parasite.due {
            assert!(entity(&mut app, engineer).is_some());
            let healer_entity = entity(&mut app, healer).unwrap();
            assert!(app.world().get::<Weapon>(healer_entity).is_none());
            return (app, healer, patient, engineer);
        }
        assert!(
            app.world().resource::<SimTick>().0 < parasite.due,
            "missed native due tick"
        );
        tick(&mut app, vec![]);
    }
    panic!("native first damage boundary not reached");
}

fn board_on_due(app: &mut bevy_app::App, engineer: SimId, healer: SimId) -> Vec<SimEvent> {
    let events = interact(app, engineer, healer);
    assert!(
        rejected(&events).is_empty(),
        "actual Engineer boarding rejected: {events:?}"
    );
    assert!(
        entity(app, engineer).is_none(),
        "Engineer must really embark in Commands"
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .cargo
            .get(&healer)
            .is_some_and(|p| p.iter().any(|p| p.id == engineer)),
        "real cargo premise"
    );
    events
}

#[test]
fn ra9_due_drone_repair_living_patient_positive_control() {
    let (mut app, healer, patient, engineer) = due_ifv_case(false, 51);
    board_on_due(&mut app, engineer, healer);
    assert_eq!(
        hp(&mut app, patient),
        51,
        "native50 damage leaves1, then50 repair"
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .parasites
            .contains_key(&patient)
    );
    let e = entity(&mut app, healer).unwrap();
    assert!(app.world().get::<Weapon>(e).is_some_and(|w| w.cooldown > 0));
    cells(&mut app);
}

#[test]
fn ra9_due_drone_killed_patient_cannot_be_revived_by_ifv() {
    let (mut app, healer, patient, engineer) = due_ifv_case(false, 25);
    let events = board_on_due(&mut app, engineer, healer);
    assert!(
        entity(&mut app, patient).is_none(),
        "a scheduled lethal Drone hit must remain lethal through the later IFV pass"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == patient))
    );
    cells(&mut app);
}

#[test]
fn ra9_due_drone_repair_living_healer_positive_control() {
    let (mut app, healer, patient, engineer) = due_ifv_case(true, 51);
    board_on_due(&mut app, engineer, healer);
    assert!(hp(&mut app, healer) > 0);
    assert_eq!(
        hp(&mut app, patient),
        150,
        "live due-tick healer must actually repair"
    );
    cells(&mut app);
}

#[test]
fn ra9_due_drone_killed_ifv_cannot_repair_patient() {
    let (mut app, healer, patient, engineer) = due_ifv_case(true, 25);
    // Unlike patient-host case, Cleanup drops dead IFV's cargo, so board_on_due's retained
    // cargo assertion would be wrong after this schedule. Check actual boarding via Engineer
    // departure and absence of rejection; pre-Commands owned adjacent live premise is above.
    let events = interact(&mut app, engineer, healer);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, engineer).is_none());
    assert!(
        entity(&mut app, healer).is_none(),
        "real due-tick damage kills IFV"
    );
    assert_eq!(
        hp(&mut app, patient),
        100,
        "the later repair pass must reread healer Health"
    );
    cells(&mut app);
}

#[test]
fn ra9_guard_chooses_legal_ground_threat_behind_closer_aircraft() {
    let mut app = world();
    let guard = put(&mut app, P0, "rhino", 10, 10);
    let closer = put(&mut app, P1, "rocketeer", 11, 10);
    let legal = put(&mut app, P1, "grizzly", 13, 10);
    disarm(&mut app, closer);
    disarm(&mut app, legal);
    let mut events = tick(
        &mut app,
        vec![issue(P0, Command::Guard { units: vec![guard] })],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let before = hp(&mut app, legal);
    for _ in 0..200 {
        if events.iter().any(|event| matches!(event, SimEvent::WeaponFired { attacker, target, .. } if *attacker == guard && *target == legal)) {
            // A projectile may land after the firing observation.
            for _ in 0..100 {
                if hp(&mut app, legal) < before { cells(&mut app); return; }
                tick(&mut app, vec![]);
            }
            panic!("legal acquired threat received no native damage");
        }
        events = tick(&mut app, vec![]);
    }
    panic!("closer unattackable aircraft starved an in-range legal Guard target");
}
