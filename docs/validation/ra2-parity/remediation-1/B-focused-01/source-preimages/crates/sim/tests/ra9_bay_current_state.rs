//! Pending launches obey actual Temporal and same-Economy incoming impact liveness.
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{Cell, Command, Grid, Mobile, SimEvent, SimId, SimTick, SpecialState, Weapon};

fn sea() -> bevy_app::App {
    let mut app = world();
    let mut cells = vec![
        Cell {
            passable: true,
            water: true,
            buildable: false,
            ore: 0
        };
        48 * 48
    ];
    // A genuine walkable shore for the hostile Chrono caster, outside either ship's cell.
    for y in 8..13 {
        for x in 10..13 {
            cells[y * 48 + x] = OPEN;
        }
    }
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, cells).unwrap());
    app
}

fn launched(events: &[SimEvent], parent: SimId) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitLaunched {
                parent: of, unit, ..
            } if *of == parent => Some(*unit),
            _ => None,
        })
        .collect()
}

fn pending_volley() -> (bevy_app::App, SimId, SimId, SimId, u64) {
    let mut app = sea();
    let source = put(&mut app, P0, "dreadnought", 14, 10);
    let target = put(&mut app, P1, "amphibious_transport", 30, 10);
    let observer = put(&mut app, P0, "rocketeer", 28, 13);
    disarm(&mut app, observer);
    tick(&mut app, vec![]); // real forward sight before long-range public Attack
    let mut events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![source],
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..100 {
        if let Some(first) = launched(&events, source).first() {
            let first = *first;
            let bay = &app.world().resource::<SpecialState>().bays[&source];
            assert!(bay.pending > 0 && bay.wait == sim::SPAWN_LAUNCH_GAP);
            let at = app.world().resource::<SimTick>().0;
            return (app, source, target, first, at);
        }
        events = tick(&mut app, vec![]);
    }
    panic!("public attack never made genuine pending native volley");
}

#[test]
fn ra9_unfrozen_pending_dreadnought_retains_exact_27_launch_spacing() {
    let (mut app, source, _, _, first_tick) = pending_volley();
    for elapsed in 1..=sim::SPAWN_LAUNCH_GAP {
        let events = tick(&mut app, vec![]);
        let births = launched(&events, source);
        assert_eq!(births.len(), usize::from(elapsed == sim::SPAWN_LAUNCH_GAP));
    }
    assert_eq!(app.world().resource::<SimTick>().0 - first_tick, 27);
    cells(&mut app);
}

#[test]
fn ra9_temporal_pending_dreadnought_cannot_launch_but_existing_missile_flies() {
    let (mut app, source, target, first, _) = pending_volley();
    let caster = put(&mut app, P1, "chrono_legionnaire", 11, 10);
    start_beam(&mut app, caster, source);
    let pending = app.world().resource::<SpecialState>().bays[&source].pending;
    assert!(pending > 0);
    let mut arrived = false;
    for _ in 0..400 {
        let events = tick(&mut app, vec![]);
        frozen(&mut app, source);
        assert!(
            launched(&events, source).is_empty(),
            "a frozen source cannot initiate the pending birth: {events:?}"
        );
        arrived |= events.iter().any(
            |event| matches!(event, SimEvent::MissileDetonated { unit, .. } if *unit == first),
        );
        if arrived {
            break;
        }
    }
    assert!(
        arrived,
        "already launched missile must retain legitimate continuation"
    );
    assert!(
        hp(&mut app, target) > 0,
        "living pending target is required for release control"
    );
    assert_eq!(
        app.world().resource::<SpecialState>().bays[&source].pending,
        pending
    );
    release_beam(&mut app, caster, source);
    let mut resumed = false;
    for _ in 0..30 {
        let events = tick(&mut app, vec![]);
        resumed |= !launched(&events, source).is_empty();
        if resumed {
            break;
        }
    }
    assert!(resumed, "same live released pending mission must resume");
    cells(&mut app);
}

fn impact_world(victim_hp: Option<i32>) -> (bevy_app::App, SimId, SimId, SimId, Weapon) {
    let mut app = sea();
    let victim = put(&mut app, P0, "dreadnought", 14, 10);
    let e = entity(&mut app, victim).unwrap();
    let original_weapon = *app.world().get::<Weapon>(e).unwrap();
    disarm(&mut app, victim); // passive until the timed public launch command
    if let Some(hp) = victim_hp {
        assert!(hp > 0);
        set_hp(&mut app, victim, hp);
    }
    let enemy = put(&mut app, P1, "dreadnought", 30, 10);
    let target = put(&mut app, P1, "amphibious_transport", 30, 16);
    let observer = put(&mut app, P1, "rocketeer", 16, 13);
    disarm(&mut app, observer);
    let victim_observer = put(&mut app, P0, "rocketeer", 28, 18);
    disarm(&mut app, victim_observer);
    // Both actual forward scouts establish their owners' sight before either public
    // long-range Attack; calibration and timed controls share this exact input prelude.
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![enemy],
                target: victim,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    (app, victim, enemy, target, original_weapon)
}

/// Calibrate actual native travel timing; no missile, pending bay or damage is injected.
/// The actual control below proves its observed due/arrival premises independently again.
fn incoming_boundary() -> (u64, i32) {
    let (mut app, victim, enemy, _, _) = impact_world(None);
    let before = hp(&mut app, victim);
    for _ in 0..1000 {
        let events = tick(&mut app, vec![]);
        if events.iter().any(
            |event| matches!(event, SimEvent::MissileDetonated { parent, .. } if *parent == enemy),
        ) {
            let damage = before - hp(&mut app, victim);
            assert!(
                damage > 0 && hp(&mut app, victim) > 0,
                "native positive impact calibration"
            );
            return (app.world().resource::<SimTick>().0, damage);
        }
    }
    panic!("genuine incoming missile never arrived");
}

fn due_impact_case(dies: bool) {
    let (impact_tick, damage) = incoming_boundary();
    assert!(impact_tick > 30);
    let (mut app, victim, enemy, target, original_weapon) =
        impact_world(Some(damage + i32::from(!dies)));
    let fire_tick = impact_tick - 28;
    let mut first = None;
    while app.world().resource::<SimTick>().0 + 1 < impact_tick {
        let next = app.world().resource::<SimTick>().0 + 1;
        let events = if next == fire_tick {
            // Restore exactly the original loaded weapon at this scenario's activation
            // boundary; the actual shot/order/pending state is produced publicly afterward.
            let e = entity(&mut app, victim).unwrap();
            app.world_mut().entity_mut(e).insert(original_weapon);
            tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Attack {
                        units: vec![victim],
                        target,
                    },
                )],
            )
        } else {
            tick(&mut app, vec![])
        };
        assert!(rejected(&events).is_empty(), "{events:?}");
        if let Some(unit) = launched(&events, victim).first() {
            assert!(
                first.is_none(),
                "only first volley member before exact boundary"
            );
            first = Some(*unit);
            assert_eq!(next, impact_tick - 27);
        }
    }
    assert!(
        first.is_some(),
        "native public victim volley genuinely launched"
    );
    let bay = &app.world().resource::<SpecialState>().bays[&victim];
    assert!(bay.pending > 0);
    assert_eq!(
        bay.wait, 1,
        "a ready pending launch is due this exact Economy"
    );
    let state = app.world().resource::<SpecialState>();
    let incoming = state
        .spawned
        .iter()
        .find_map(|(&unit, &parent)| {
            (parent == enemy)
                .then(|| state.missiles.get(&unit).map(|f| (unit, f.aim)))
                .flatten()
        })
        .expect("genuine incoming native missile remains in flight");
    let e = entity(&mut app, incoming.0).unwrap();
    let mobile = app.world().get::<Mobile>(e).unwrap();
    assert!(
        mobile.step.is_none() && mobile.cell == incoming.1.cell(),
        "incoming impact is truly due before bays in this Economy"
    );
    assert!(hp(&mut app, victim) > 0);
    let events = tick(&mut app, vec![]);
    assert!(events.iter().any(
        |event| matches!(event, SimEvent::MissileDetonated { unit, .. } if *unit == incoming.0)
    ));
    if dies {
        assert!(
            entity(&mut app, victim).is_none(),
            "actual incoming impact kills source"
        );
        assert!(
            events.iter().any(
                |event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == victim)
            )
        );
        assert!(
            launched(&events, victim).is_empty(),
            "a same-stage missile-killed source cannot launch from stale Actor HP: {events:?}"
        );
    } else {
        assert_eq!(hp(&mut app, victim), 1, "one-HP surviving source positive");
        assert_eq!(
            launched(&events, victim).len(),
            1,
            "same actual due slot must launch when source survives"
        );
    }
    cells(&mut app);
}

#[test]
fn ra9_same_economy_incoming_missile_killed_spawner_cannot_launch() {
    due_impact_case(true);
}
#[test]
fn ra9_same_economy_one_hp_surviving_spawner_launches_positive() {
    due_impact_case(false);
}
