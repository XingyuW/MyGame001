//! RA2: what low power stops, superweapon charging, and radar (docs/RA2_PARITY_PLAN.md sections
//! 4.2 and 5, RA2 items 5 and 6; docs/DECISIONS.md R24).
//!
//! Driven through `CommandQueue` and `SimSchedule` on the RA2 fixture (`tests/fixtures/data/ra2`;
//! invariant I10): the bunker is a defence that is not `powered` (as the Pillbox), the sentry is
//! `powered` and consumes no power (as the Sentry Gun), the turret is `powered` and consumes
//! power (as the Prism Tower); the storm device is a superweapon that `is_powered`; the two
//! radars are radar buildings.

mod ra2_common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use bevy_app::App;
use ra2_common::*;
use sim::{
    CellPos, Command, PlayerId, Players, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule,
    SpecialState, load_definitions,
};

fn state(app: &App, player: PlayerId) -> sim::PlayerState {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .clone()
}

/// The buildings and units that fired in `events`.
fn attackers(events: &[SimEvent]) -> BTreeSet<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::WeaponFired { attacker, .. } => Some(*attacker),
            _ => None,
        })
        .collect()
}

/// The three defences of P0 around one spot, with an enemy heavy tank in the range of each.
fn defences() -> (App, [SimId; 3], SimId) {
    let mut app = world(1_000);
    let bunker = place(&mut app, P0, "bunker", 10, 10);
    let sentry = place(&mut app, P0, "sentry", 12, 10);
    let turret = place(&mut app, P0, "turret", 14, 10);
    let tank = spawn(&mut app, P1, "heavy", 12, 13);
    (app, [bunker, sentry, turret], tank)
}

// --- Positive -----------------------------------------------------------------------------------

#[test]
fn under_low_power_only_a_powered_defence_that_consumes_power_stops_firing() {
    // No power building: the turret's 5 are more than the player produces.
    let (mut app, [bunker, sentry, turret], _) = defences();
    let fired = attackers(&ticks(&mut app, 30));
    assert!(state(&app, P0).low_power());
    assert!(fired.contains(&bunker), "a bunker needs no power");
    assert!(fired.contains(&sentry), "a sentry consumes none");
    assert!(!fired.contains(&turret), "the turret is powered and short");
    // With power the turret fires too.
    let (mut app, [bunker, sentry, turret], _) = defences();
    place(&mut app, P0, "allied_power", 20, 20);
    let fired = attackers(&ticks(&mut app, 30));
    assert!(!state(&app, P0).low_power());
    assert!(fired.contains(&bunker) && fired.contains(&sentry) && fired.contains(&turret));
}

#[test]
fn powers_tells_what_low_power_stops() {
    let (mut app, _, _) = defences();
    tick(&mut app, Vec::new());
    let definitions = defs(&app);
    let low = state(&app, P0);
    assert!(low.low_power());
    let works = |state: &sim::PlayerState, id: &str| state.powers(&definitions, kind(&app, id));
    assert!(works(&low, "bunker") && works(&low, "sentry"));
    assert!(!works(&low, "turret"));
    // Buildings that are not powered work whatever they consume; a powered one that consumes
    // power does not.
    assert!(works(&low, "allied_barracks") && works(&low, "depot"));
    assert!(!works(&low, "allied_radar") && !works(&low, "storm_device"));
    // The other player has no building: nothing consumes, nothing stops.
    let fine = state(&app, P1);
    assert!(!fine.low_power());
    for id in ["bunker", "sentry", "turret", "allied_radar", "storm_device"] {
        assert!(works(&fine, id), "{id}");
    }
}

#[test]
fn radar_follows_the_radar_building_and_the_power() {
    let mut app = world(10_000);
    place(&mut app, P0, "allied_yard", 2, 2);
    place(&mut app, P0, "allied_power", 8, 2);
    tick(&mut app, Vec::new());
    assert!(!state(&app, P0).radar(), "no radar building");
    // On: a radar building and enough power.
    let radar = place(&mut app, P0, "allied_radar", 12, 2);
    tick(&mut app, Vec::new());
    assert!(state(&app, P0).radar());
    assert!(
        !state(&app, P1).radar(),
        "another player's radar is not mine"
    );
    // Off by power: five factories (20 each) and the radar (20) consume 120 of 100.
    let factories: Vec<SimId> = (0..5)
        .map(|index| place(&mut app, P0, "allied_factory", 2 + 4 * index, 30))
        .collect();
    tick(&mut app, Vec::new());
    assert!(state(&app, P0).low_power() && !state(&app, P0).radar());
    // Back on: one factory sold, 100 of 100.
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::SellBuilding {
                building: factories[0],
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    assert!(!state(&app, P0).low_power() && state(&app, P0).radar());
    // Off again without the radar building.
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: radar })],
    );
    assert_eq!(rejections(&events), NONE);
    assert!(!state(&app, P0).radar());
}

#[test]
fn a_superweapon_that_is_powered_charges_only_with_enough_power() {
    let mut app = world(10_000);
    place(&mut app, P0, "allied_power", 8, 2);
    let device = place(&mut app, P0, "storm_device", 12, 2);
    let charge = |app: &App| {
        app.world()
            .resource::<SpecialState>()
            .charge
            .get(&device)
            .copied()
            .unwrap_or(0)
    };
    ticks(&mut app, 10);
    assert_eq!(charge(&app), 10);
    // Low power: the charge waits.
    let factories: Vec<SimId> = (0..4)
        .map(|index| place(&mut app, P0, "allied_factory", 2 + 4 * index, 30))
        .collect();
    ticks(&mut app, 10);
    assert!(state(&app, P0).low_power());
    assert_eq!(charge(&app), 10);
    // Power again: it goes on from where it was.
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::SellBuilding {
                building: factories[0],
            },
        )],
    );
    assert_eq!(rejections(&events), NONE);
    ticks(&mut app, 9);
    assert_eq!(charge(&app), 20);
}

// --- Negative -----------------------------------------------------------------------------------

#[test]
fn a_charged_superweapon_is_not_fired_under_low_power() {
    let mut app = world(10_000);
    place(&mut app, P0, "allied_yard", 2, 2);
    place(&mut app, P0, "allied_power", 8, 2);
    let device = place(&mut app, P0, "storm_device", 12, 2);
    // 600 ticks of charge.
    ticks(&mut app, 600);
    for index in 0..4 {
        place(&mut app, P0, "allied_factory", 2 + 4 * index, 30);
    }
    tick(&mut app, Vec::new());
    assert!(state(&app, P0).low_power());
    let fire = || {
        issue(
            P0,
            Command::Superweapon {
                building: device,
                target: CellPos::new(4, 8),
                units: Vec::new(),
            },
        )
    };
    let events = tick(&mut app, vec![fire()]);
    assert_eq!(
        rejections(&events),
        [RejectReason::AbilityUnavailable(
            "Ability requires power and a full charge"
        )]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::StrategicActivated { .. }))
    );
    // With power it fires.
    place(&mut app, P0, "allied_power", 20, 2);
    tick(&mut app, Vec::new());
    let events = tick(&mut app, vec![fire()]);
    assert_eq!(rejections(&events), NONE);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::StrategicActivated { .. }))
    );
}

// --- Regression: a world without a rules file ------------------------------------------------------

#[test]
fn a_world_without_a_rules_file_has_radar_and_the_power_rule_of_m3() {
    // The fixture of M3: units and economy, no rules, no arsenal. Its players always have
    // radar, and low power stops every building.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
    let definitions = load_definitions(dir.join("units_economy.ron"), dir.join("economy.ron"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(definitions.rules().is_none());
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 1 });
    app.world_mut()
        .resource_mut::<Players>()
        .add(P0, 0)
        .expect("a player");
    app.world_mut().insert_resource(definitions.clone());
    app.world_mut().run_schedule(SimSchedule);
    let state = state(&app, P0);
    assert!(state.radar());
    let any = sim::BuildingKind(0);
    assert!(state.powers(&definitions, any), "enough power");
}
