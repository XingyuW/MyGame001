//! RA4: missiles and aircraft that other units launch (docs/RA2_PARITY_PLAN.md section 4.8;
//! docs/DECISIONS.md R50), on the RA4 fixture (`tests/fixtures/data/ra4`; invariant I10),
//! through `CommandQueue` and `SimSchedule`.
//!
//! In the fixture the launcher (reach 3 to 12 cells) carries one rocket (50 hit points, speed
//! 64), replaced 200 ticks after it is gone; the rocket goes off with 150 through the blast
//! (2 cells, 25 percent at the edge, 50 percent against plate), or 300 from an elite launcher.
//! The carrier carries three drones (60 hit points, one dart of 40), ready again 100 ticks
//! after they return and replaced 400 ticks after they are lost. The boat sends its hunter
//! (one depth bomb of 50) with its secondary weapon against what is underwater. The sentry's
//! sam (30 every 20 ticks, 5 cells) engages aircraft only.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{
    CellPos, Domain, ELITE, RejectReason, SPAWN_LAUNCH_GAP, SimEvent, SimId, Slot, SpecialState,
    SubPos, VETERAN,
};

fn launched(events: &[SimEvent]) -> Vec<(SimId, SimId)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitLaunched { parent, unit, .. } => Some((*parent, *unit)),
            _ => None,
        })
        .collect()
}

fn recovered(events: &[SimEvent]) -> Vec<(SimId, SimId)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitRecovered { parent, unit, .. } => Some((*parent, *unit)),
            _ => None,
        })
        .collect()
}

fn detonated(events: &[SimEvent]) -> Vec<(SimId, SubPos)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::MissileDetonated { unit, at, .. } => Some((*unit, *at)),
            _ => None,
        })
        .collect()
}

fn destroyed(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitDestroyed { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

fn state(app: &App) -> &SpecialState {
    app.world().resource::<SpecialState>()
}

/// A launcher of P0 at (5, 10), an unarmed spotter of P0 at (11, 10), and a truck of P1 (300
/// hit points, plate) at (15, 10), ten cells from the launcher; after one tick.
fn launcher_and_truck() -> (App, SimId, SimId) {
    let mut app = world();
    let launcher = spawn(&mut app, P0, "launcher", 5, 10);
    spawn(&mut app, P0, "engineer", 11, 10);
    let truck = spawn(&mut app, P1, "truck", 15, 10);
    tick(&mut app, Vec::new());
    (app, launcher, truck)
}

/// Orders the attack and runs until the first rocket has gone off; the events.
fn first_rocket(app: &mut App, launcher: SimId, truck: SimId) -> Vec<SimEvent> {
    let mut events = tick(app, vec![attack(P0, launcher, truck)]);
    let (more, _) = run_until(app, 200, |_, events| !detonated(events).is_empty());
    events.extend(more);
    events
}

// --- Missiles ------------------------------------------------------------------------------------

#[test]
fn a_launcher_launches_a_missile_that_flies_to_the_target_and_goes_off_there() {
    let (mut app, launcher, truck) = launcher_and_truck();
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    // The launcher's shot deals nothing; the rocket appears above the launcher a tick later.
    assert_eq!(shots(&events, launcher).len(), 1);
    assert_eq!(shots(&events, launcher)[0].1, weapon_kind(&app, "launch"));
    events.extend(tick(&mut app, Vec::new()));
    assert_eq!(hp(&mut app, truck), Some(300));
    let rockets = launched(&events);
    assert_eq!(rockets.len(), 1);
    let (parent, rocket) = rockets[0];
    assert_eq!(parent, launcher);
    assert_eq!(units_of(&mut app, P0, "rocket"), [rocket]);
    assert_eq!(mobile(&mut app, rocket).cell, CellPos::new(5, 10));
    assert_eq!(
        app.world().resource::<sim::Grid>().domain_of(rocket),
        Domain::Air
    );
    assert_eq!(hp(&mut app, rocket), Some(50));
    assert_eq!(state(&app).spawned.get(&rocket), Some(&launcher));
    assert_eq!(
        state(&app).bays[&launcher].slots,
        [Slot::Out(rocket)],
        "its place in the bay is empty"
    );
    // It flies ten cells at 64 subcells a tick, and the cells stay in order on its way.
    let mut flight = 0;
    let end = loop {
        let events = tick(&mut app, Vec::new());
        assert_cells(&mut app, "the flight");
        flight += 1;
        assert!(flight < 100);
        if !detonated(&events).is_empty() {
            break events;
        }
        assert_eq!(hp(&mut app, truck), Some(300), "nothing before it arrives");
    };
    assert!((40..=44).contains(&flight), "{flight} ticks");
    assert_eq!(
        detonated(&end),
        [(rocket, SubPos::centre_of(CellPos::new(15, 10)))]
    );
    assert_eq!(
        impacts(&end),
        [(SubPos::centre_of(CellPos::new(15, 10)), false)]
    );
    // 150 through the blast against plate, at the point of the impact: 75.
    assert_eq!(hp(&mut app, truck), Some(225));
    // The rocket is gone without having been destroyed.
    assert!(!alive(&mut app, rocket));
    assert!(destroyed(&end).is_empty());
    assert!(state(&app).spawned.is_empty() && state(&app).missiles.is_empty());
}

#[test]
fn the_next_missile_is_ready_after_the_time_to_replace_the_last() {
    let (mut app, launcher, truck) = launcher_and_truck();
    set_hp(&mut app, truck, 10_000);
    let mut launches = Vec::new();
    let mut ends = Vec::new();
    let mut fired = Vec::new();
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    for n in 0..700u32 {
        if !launched(&events).is_empty() {
            launches.push(n);
        }
        if !detonated(&events).is_empty() {
            ends.push(n);
        }
        if !shots(&events, launcher).is_empty() {
            fired.push(n);
        }
        events = tick(&mut app, Vec::new());
    }
    assert!(launches.len() >= 3, "{launches:?}");
    // The launcher does not fire while its bay is empty: one shot for every rocket, and the
    // next is launched 200 ticks after the last went off (and one tick for the shot).
    assert_eq!(fired.len(), launches.len());
    for (launch, end) in launches[1..].iter().zip(&ends) {
        assert!(
            (200..=202).contains(&(launch - end)),
            "{launches:?} after {ends:?}"
        );
    }
}

#[test]
fn a_missile_that_is_shot_down_deals_nothing() {
    let (mut app, launcher, truck) = launcher_and_truck();
    // Two sentries of P1 under its way.
    place(&mut app, P1, "sentry", 11, 12);
    place(&mut app, P1, "sentry", 12, 8);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    events.extend(ticks(&mut app, 120));
    let rocket = launched(&events)[0].1;
    assert_eq!(destroyed(&events), [rocket]);
    assert!(detonated(&events).is_empty() && impacts(&events).iter().all(|&(_, air)| air));
    assert_eq!(hp(&mut app, truck), Some(300));
    // Its place is filled again after the time to replace it.
    assert!(matches!(
        state(&app).bays[&launcher].slots[..],
        [Slot::Lost(_)]
    ));
    let (later, _) = run_until(&mut app, 250, |_, events| !launched(events).is_empty());
    assert_eq!(launched(&later).len(), 1);
}

#[test]
fn the_missile_of_a_veteran_and_of_an_elite_launcher() {
    for (rank, left) in [
        // 150 x 1.1 x 0.5 = 82.5; the elite effect: 300 x 1.1 x 0.5 = 165.
        (VETERAN, 300 - 82),
        (ELITE, 300 - 165),
    ] {
        let (mut app, launcher, truck) = launcher_and_truck();
        assert!(sim::promote(app.world_mut(), launcher, rank));
        first_rocket(&mut app, launcher, truck);
        assert_eq!(hp(&mut app, truck), Some(left), "rank {rank}");
    }
}

#[test]
fn what_a_missile_destroys_counts_for_its_launcher() {
    let (mut app, launcher, truck) = launcher_and_truck();
    set_hp(&mut app, truck, 10);
    let events = first_rocket(&mut app, launcher, truck);
    assert!(destroyed(&events).contains(&truck));
    // A truck costs 100; the launcher costs 400.
    assert_eq!(
        experience(&mut app, launcher).map(|found| found.value),
        Some(100)
    );
}

#[test]
fn a_missile_goes_off_where_its_target_was_and_outlives_its_launcher() {
    let (mut app, launcher, truck) = launcher_and_truck();
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    events.extend(tick(&mut app, Vec::new()));
    let rocket = launched(&events)[0].1;
    // The truck drives off, and the launcher is destroyed.
    tick(&mut app, vec![move_to(P1, &[truck], 15, 20)]);
    set_hp(&mut app, launcher, 0);
    let (events, _) = run_until(&mut app, 200, |_, events| !detonated(events).is_empty());
    assert_eq!(
        detonated(&events),
        [(rocket, SubPos::centre_of(CellPos::new(15, 10)))]
    );
    assert_eq!(hp(&mut app, truck), Some(300), "three cells away by then");
    assert!(state(&app).bays.is_empty() && state(&app).spawned.is_empty());
}

#[test]
fn a_launcher_keeps_its_minimum_range_and_a_launched_unit_takes_no_commands() {
    let mut app = world();
    let launcher = spawn(&mut app, P0, "launcher", 5, 10);
    let near = spawn(&mut app, P1, "truck", 7, 10);
    let events = ticks(&mut app, 80);
    assert!(shots(&events, launcher).is_empty(), "inside 3 cells");
    assert!(alive(&mut app, near));
    // One in reach is attacked without an order.
    spawn(&mut app, P1, "truck", 11, 10);
    let (events, _) = run_until(&mut app, 100, |_, events| !launched(events).is_empty());
    let rocket = launched(&events)[0].1;
    let events = tick(&mut app, vec![move_to(P0, &[rocket], 20, 20)]);
    assert_eq!(rejections(&events), [RejectReason::NotCommandable(rocket)]);
    let events = tick(&mut app, vec![attack(P0, rocket, near)]);
    assert_eq!(rejections(&events), [RejectReason::NotCommandable(rocket)]);
    let events = tick(
        &mut app,
        vec![issue(P0, sim::Command::Deploy { unit: rocket })],
    );
    assert_eq!(rejections(&events), [RejectReason::NotCommandable(rocket)]);
    // Another player's command is refused as for any unit that is not its own.
    let events = tick(&mut app, vec![move_to(P1, &[rocket], 20, 20)]);
    assert_eq!(rejections(&events), [RejectReason::NotOwned(rocket)]);
}

// --- Carried aircraft ---------------------------------------------------------------------------

/// Water south of row 20; a carrier of P0 at (10, 25), a spotter at (10, 19) and a truck of
/// P1 at (10, 16), nine cells from the carrier; after one tick.
fn carrier_and_truck() -> (App, SimId, SimId) {
    let mut app = world();
    flood(&mut app, (0, 20), (39, 39));
    let carrier = spawn(&mut app, P0, "carrier", 10, 25);
    spawn(&mut app, P0, "engineer", 10, 19);
    let truck = spawn(&mut app, P1, "truck", 10, 16);
    tick(&mut app, Vec::new());
    (app, carrier, truck)
}

#[test]
fn a_carrier_launches_its_aircraft_one_after_another_and_takes_them_back() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 10_000);
    let mut launches = Vec::new();
    let mut returns = Vec::new();
    let mut darts = 0;
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    for n in 0..260u32 {
        for (parent, unit) in launched(&events) {
            assert_eq!(parent, carrier);
            launches.push((n, unit));
        }
        for (parent, unit) in recovered(&events) {
            assert_eq!(parent, carrier);
            returns.push((n, unit));
        }
        for &(_, drone) in &launches {
            darts += shots(&events, drone).len();
        }
        assert_cells(&mut app, "the sortie");
        events = tick(&mut app, Vec::new());
    }
    // Three drones, 27 ticks apart; each throws its one dart of 40 and comes back.
    assert!(launches.len() >= 3, "{launches:?}");
    assert_eq!(launches[1].0 - launches[0].0, SPAWN_LAUNCH_GAP);
    assert_eq!(launches[2].0 - launches[1].0, SPAWN_LAUNCH_GAP);
    let first: Vec<SimId> = launches[..3].iter().map(|&(_, unit)| unit).collect();
    for drone in &first {
        assert!(returns.iter().any(|(_, unit)| unit == drone), "{drone:?}");
    }
    assert!(darts >= 3);
    assert_eq!(hp(&mut app, truck), Some(10_000 - 40 * darts as i32));
    // A drone that came back is launched again 100 ticks later, as a new unit.
    let back = returns[0].0;
    let again = launches
        .iter()
        .find(|&&(at, _)| at > back)
        .expect("a second sortie");
    assert!((100..=101 + SPAWN_LAUNCH_GAP).contains(&(again.0 - back)));
    assert!(!first.contains(&again.1));
    assert!(destroyed(&ticks(&mut app, 1)).is_empty());
}

#[test]
fn a_carrier_regains_an_aircraft_that_was_shot_down() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 10_000);
    // Sentries of P1 beside the truck shoot the drones down.
    let sentries = [8, 12].map(|x| place(&mut app, P1, "sentry", x, 16));
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    events.extend(ticks(&mut app, 200));
    let drones: Vec<SimId> = launched(&events).iter().map(|&(_, unit)| unit).collect();
    let lost: Vec<SimId> = destroyed(&events)
        .into_iter()
        .filter(|unit| drones.contains(unit))
        .collect();
    assert!(!lost.is_empty(), "the sentries shot nothing down");
    let lost_slots = |app: &App| {
        state(app).bays[&carrier]
            .slots
            .iter()
            .filter(|slot| matches!(slot, Slot::Lost(_)))
            .count()
    };
    assert!(lost_slots(&app) > 0);
    // With nothing left to attack, 400 ticks after the last loss it has three again.
    for id in [truck, sentries[0], sentries[1]] {
        set_hp(&mut app, id, 0);
    }
    ticks(&mut app, 500);
    assert_eq!(state(&app).bays[&carrier].slots, [Slot::Ready; 3]);
    assert!(units_of(&mut app, P0, "drone").is_empty());
}

#[test]
fn the_aircraft_of_a_carrier_are_destroyed_with_it_and_their_kills_are_its_own() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 30);
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    let (more, _) = run_until(&mut app, 200, |app, _| !alive(app, truck));
    events.extend(more);
    // The first dart destroyed the truck (100 credits): the carrier's experience.
    assert_eq!(
        experience(&mut app, carrier).map(|found| found.value),
        Some(100)
    );
    let out = units_of(&mut app, P0, "drone");
    assert!(!out.is_empty());
    set_hp(&mut app, carrier, 0);
    let events = ticks(&mut app, 3);
    let gone = destroyed(&events);
    assert!(gone.contains(&carrier));
    for drone in out {
        assert!(gone.contains(&drone), "{drone:?} outlived its carrier");
    }
    assert!(state(&app).bays.is_empty() && state(&app).spawned.is_empty());
}

#[test]
fn the_aircraft_of_an_elite_carrier_are_elite() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 10_000);
    assert!(sim::promote(app.world_mut(), carrier, ELITE));
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    let (more, _) = run_until(&mut app, 200, |app, _| {
        hp(app, truck).is_some_and(|left| left < 10_000)
    });
    events.extend(more);
    let drone = launched(&events)[0].1;
    // The elite weapon of the drone, 80, with the firepower of its veteran list: 88.
    assert_eq!(
        shots(&events, drone),
        [(truck, weapon_kind(&app, "heavy_dart"))]
    );
    assert_eq!(hp(&mut app, truck), Some(10_000 - 88));
}

// --- The boat's hunter against what is underwater -------------------------------------------------

#[test]
fn a_boat_sends_its_hunter_against_a_submerged_target_only() {
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    let boat = spawn(&mut app, P0, "boat", 10, 25);
    let sub = spawn(&mut app, P1, "sub", 14, 25);
    silence(&mut app, sub);
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(tick(&mut app, Vec::new()));
        assert_cells(&mut app, "the hunter");
    }
    // The boat's only shots are those of its launcher; the hunter's depth bomb is 50.
    let fired = shots(&events, boat);
    assert!(!fired.is_empty());
    assert!(
        fired
            .iter()
            .all(|&(target, weapon)| target == sub && weapon == weapon_kind(&app, "launch_hunter"))
    );
    let hunters: Vec<SimId> = launched(&events).iter().map(|&(_, unit)| unit).collect();
    assert_eq!(
        hunters.len(),
        1,
        "one hunter, and it is not ready again yet"
    );
    assert_eq!(
        shots(&events, hunters[0]),
        [(sub, weapon_kind(&app, "depth_bomb"))]
    );
    assert_eq!(hp(&mut app, sub), Some(250));
    assert_eq!(recovered(&events), [(boat, hunters[0])]);
    // A ship on the surface is shelled with the cannon, and no hunter is sent.
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    let boat = spawn(&mut app, P0, "boat", 10, 25);
    let other = spawn(&mut app, P1, "hovercraft", 14, 25);
    let events = ticks(&mut app, 60);
    assert!(launched(&events).is_empty());
    assert!(
        shots(&events, boat)
            .iter()
            .all(|&(target, weapon)| target == other && weapon == weapon_kind(&app, "cannon"))
    );
    assert!(!shots(&events, boat).is_empty());
}

/// Keeps `id` from firing and moving (scenario setup: a target that only takes hits).
fn silence(app: &mut App, id: SimId) {
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(id, u64::MAX);
}

// --- Integration and regression ------------------------------------------------------------------

#[test]
fn missiles_and_aircraft_in_flight_run_the_same_twice() {
    let run = || {
        let mut app = world();
        flood(&mut app, (0, 20), (39, 39));
        let carrier = spawn(&mut app, P0, "carrier", 10, 25);
        let launchers: Vec<SimId> = (0..3)
            .map(|n| spawn(&mut app, P0, "launcher", 4 + 2 * n, 6))
            .collect();
        spawn(&mut app, P0, "engineer", 10, 19);
        spawn(&mut app, P0, "engineer", 12, 12);
        let yard = place(&mut app, P1, "soviet_yard", 12, 14);
        place(&mut app, P1, "sentry", 16, 14);
        // The yard stands the whole bombardment.
        set_hp(&mut app, yard, 1_000_000);
        tick(&mut app, Vec::new());
        let mut commands: Vec<_> = launchers.iter().map(|&id| attack(P0, id, yard)).collect();
        commands.push(attack(P0, carrier, yard));
        let mut events = tick(&mut app, commands);
        let mut hashes = Vec::new();
        for _ in 0..900 {
            assert_cells(&mut app, "the bombardment");
            hashes.push(hash(&mut app));
            events.extend(tick(&mut app, Vec::new()));
        }
        (
            hashes,
            launched(&events).len(),
            detonated(&events).len(),
            alive(&mut app, yard),
        )
    };
    let first = run();
    assert_eq!(first, run());
    let (_, launches, ends, _) = first;
    assert!(
        launches > 6 && ends >= 3,
        "{launches} launches, {ends} ends"
    );
}

#[test]
fn bays_launched_units_and_missiles_are_hashed() {
    let scenario = |edit: fn(&mut SpecialState, SimId, SimId)| {
        let (mut app, launcher, truck) = launcher_and_truck();
        let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
        events.extend(tick(&mut app, Vec::new()));
        let rocket = launched(&events)[0].1;
        edit(
            &mut app.world_mut().resource_mut::<SpecialState>(),
            launcher,
            rocket,
        );
        hash(&mut app)
    };
    let base = scenario(|_, _, _| {});
    assert_eq!(base, scenario(|_, _, _| {}));
    for (name, edit) in [
        (
            "the slot",
            (|state, launcher, _| {
                state.bays.get_mut(&launcher).expect("bay").slots[0] = Slot::Lost(5);
            }) as fn(&mut SpecialState, SimId, SimId),
        ),
        ("the bay's target", |state, launcher, _| {
            state.bays.get_mut(&launcher).expect("bay").target = None;
        }),
        ("the bay's wait", |state, launcher, _| {
            state.bays.get_mut(&launcher).expect("bay").wait += 1;
        }),
        ("the bay's pending launches", |state, launcher, _| {
            state.bays.get_mut(&launcher).expect("bay").pending += 1;
        }),
        ("the launcher of a unit", |state, _, rocket| {
            state.spawned.remove(&rocket);
        }),
        ("a missile's aim", |state, _, rocket| {
            state.missiles.get_mut(&rocket).expect("flight").aim.x += 1;
        }),
        ("a missile's rank", |state, _, rocket| {
            state.missiles.get_mut(&rocket).expect("flight").elite = true;
        }),
        ("a missile's firepower", |state, _, rocket| {
            state
                .missiles
                .get_mut(&rocket)
                .expect("flight")
                .firepower_pm = 1100;
        }),
    ] {
        assert_ne!(scenario(edit), base, "{name} is not hashed");
    }
}

// --- Regression: advisory review 2, sim P1-1 and P1-2 (docs/DECISIONS.md R56) -----------------------

#[test]
fn a_launcher_ordered_onto_scenery_sends_its_rocket_to_the_cell() {
    // Scenery is no actor of the simulation: the launcher's bay never had its target, and the
    // launcher fired its launching shot every 200 ticks for ever without a rocket or an end.
    let mut app = world();
    let launcher = spawn(&mut app, P0, "launcher", 5, 10);
    spawn(&mut app, P0, "engineer", 14, 11);
    let scenery = tower(&mut app, 15, 10, "plate");
    ticks(&mut app, 2);
    let mut events = tick(&mut app, vec![attack(P0, launcher, scenery)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    let (more, _) = run_until(&mut app, 200, |_, events| !detonated(events).is_empty());
    events.extend(more);
    let rockets = launched(&events);
    assert_eq!(rockets.len(), 1);
    assert_eq!(rockets[0].0, launcher);
    assert_eq!(
        detonated(&events),
        [(rockets[0].1, SubPos::centre_of(CellPos::new(15, 10)))]
    );
    // The rocket's 150 through `blast` (plate 50 percent) at the tower's own cell.
    assert_eq!(scenery_hp(&app, 15, 10), Some(1000 - 75));
}

#[test]
fn a_launcher_that_boards_a_transport_and_leaves_it_has_no_rocket_sooner() {
    // The bay of a spawner that went aboard was dropped, and a spawner without a bay has all
    // it carries: the launcher launched at tick 2, boarded, left, and launched again at tick 8,
    // where the fixture replaces a rocket 200 ticks after it is gone.
    let mut app = world();
    let launcher = spawn(&mut app, P0, "launcher", 5, 10);
    let craft = spawn(&mut app, P0, "hovercraft", 5, 11);
    spawn(&mut app, P0, "engineer", 11, 10);
    let truck = spawn(&mut app, P1, "truck", 15, 10);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(truck, u64::MAX);
    set_hp(&mut app, truck, 100_000);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    events.extend(ticks(&mut app, 3));
    assert_eq!(launched(&events).len(), 1);
    assert_eq!(
        state(&app).bays[&launcher].slots,
        [Slot::Out(launched(&events)[0].1)]
    );
    // Aboard: the bay goes with it, and the rocket that is out counts as lost from now on.
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            sim::Command::Interact {
                unit: launcher,
                target: craft,
            },
        )],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert!(!state(&app).bays.contains_key(&launcher));
    let aboard = &state(&app).cargo[&craft];
    assert_eq!(aboard.len(), 1);
    assert_eq!(
        aboard[0].bay.as_ref().map(|bay| bay.slots.clone()),
        Some(vec![Slot::Lost(200)])
    );
    // What a passenger's bay holds is part of the state.
    let before = hash(&mut app);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .cargo
        .get_mut(&craft)
        .expect("cargo")[0]
        .bay = None;
    assert_ne!(hash(&mut app), before);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .cargo
        .get_mut(&craft)
        .expect("cargo")[0]
        .bay = Some(sim::Bay {
        slots: vec![Slot::Lost(200)],
        target: None,
        pending: 0,
        wait: 0,
    });
    // The timers stand still aboard.
    ticks(&mut app, 50);
    let events = tick(
        &mut app,
        vec![issue(P0, sim::Command::Deploy { unit: craft })],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    // (Its first tick on the map has passed.)
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(199)]);
    // Ordered to attack again, it launches when the place is filled: 200 ticks after it left
    // the transport, and not before.
    let mut events = tick(&mut app, vec![attack(P0, launcher, truck)]);
    events.extend(ticks(&mut app, 195));
    assert!(launched(&events).is_empty(), "{:?}", launched(&events));
    let (events, _) = run_until(&mut app, 240, |_, events| !launched(events).is_empty());
    assert_eq!(launched(&events).len(), 1);
}

// These pin the current scheduled fixture contracts, not unsupported original-engine flight physics.
#[test]
fn ra9_missile_replacement_has_exact_ready_and_next_launch_boundaries() {
    let (mut app, launcher, truck) = launcher_and_truck();
    set_hp(&mut app, truck, 10_000);
    let events = first_rocket(&mut app, launcher, truck);
    assert_eq!(detonated(&events).len(), 1);
    // spawned_units installs Lost(200), then bays consumes this tick's first timer step.
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(199)]);
    // End this fixture target after its real impact; idle acquisition must not queue another shot.
    set_hp(&mut app, truck, 0);
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![launcher] })]);
    assert!(rejections(&events).is_empty());
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(198)]);
    for remaining in (1..198).rev() {
        let events = tick(&mut app, vec![]);
        assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(remaining)]);
        assert!(launched(&events).is_empty());
    }
    let events = tick(&mut app, vec![]);
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Ready]);
    assert!(launched(&events).is_empty(), "ready alone is not a new attack");
    let next_target = spawn(&mut app, P1, "truck", 15, 10);
    let events = tick(&mut app, vec![attack(P0, launcher, next_target)]);
    assert_eq!(shots(&events, launcher).len(), 1);
    assert!(launched(&events).is_empty(), "Combat's launch shot is after Economy's bay pass");
    let events = tick(&mut app, vec![]);
    assert_eq!(launched(&events).len(), 1, "next Economy launches the ready missile");
    assert_cells(&mut app, "exact missile regeneration");
}

#[test]
fn ra9_returned_carrier_aircraft_rearms_at_the_exact_scheduled_boundary() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 10_000);
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    let mut first = None;
    for _ in 0..20 {
        if let Some((_, unit)) = launched(&events).first() { first = Some(*unit); break; }
        events = tick(&mut app, vec![]);
    }
    let first = first.expect("first actual launch");
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![carrier] })]);
    assert!(rejections(&events).is_empty());
    let mut came_back = false;
    for _ in 0..200 {
        let events = tick(&mut app, vec![]);
        if recovered(&events).contains(&(carrier, first)) { came_back = true; break; }
    }
    assert!(came_back, "real launched aircraft must fire and return");
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(99));
    // End the durable fixture target after a real sortie; Stop alone permits later idle acquisition.
    set_hp(&mut app, truck, 0);
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![carrier] })]);
    assert!(rejections(&events).is_empty());
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(98));
    for remaining in (1..98).rev() {
        let events = tick(&mut app, vec![]);
        assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(remaining));
        assert!(launched(&events).is_empty());
    }
    tick(&mut app, vec![]);
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Ready);
    assert_cells(&mut app, "exact carrier rearm");
}
