//! RA7 actual tactical orders: command validation, execution, queue bounds and replay hashing.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{CellPos, Command, ForceTarget, RejectReason, SimEvent, SimId, TacticalState};
fn at(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).unwrap();
    app.world().get::<sim::Mobile>(e).unwrap().cell
}
fn hp(app: &mut App, id: SimId) -> i32 {
    let e = entity(app, id).unwrap();
    app.world().get::<sim::Health>(e).unwrap().hit_points
}
fn run(app: &mut App, n: usize) -> Vec<SimEvent> {
    let mut e = Vec::new();
    for _ in 0..n {
        e.extend(tick(app, vec![]));
    }
    e
}
fn send(app: &mut App, c: Command) -> Vec<SimEvent> {
    tick(app, vec![issue(P0, c)])
}
#[test]
fn ra7_follow_tracks_a_moving_friendly_then_stop_cancels_every_order() {
    let mut app = world();
    let a = put(&mut app, P0, "engineer", 5, 5);
    let b = put(&mut app, P0, "engineer", 9, 5);
    tick(&mut app, vec![]);
    send(
        &mut app,
        Command::Follow {
            units: vec![a],
            target: b,
        },
    );
    send(
        &mut app,
        Command::Move {
            units: vec![b],
            target: CellPos::new(18, 5),
        },
    );
    run(&mut app, 450);
    let ac = at(&mut app, a);
    let bc = at(&mut app, b);
    assert!(
        ac.x.abs_diff(bc.x).max(ac.y.abs_diff(bc.y)) <= 1,
        "{ac:?} {bc:?}"
    );
    send(&mut app, Command::Stop { units: vec![a] });
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .orders
            .contains_key(&a)
    );
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .waypoints
            .contains_key(&a)
    );
}
#[test]
fn ra7_scatter_moves_members_apart_and_guard_fires_then_returns() {
    let mut app = world();
    let a = put(&mut app, P0, "gi", 10, 10);
    let b = put(&mut app, P0, "gi", 10, 10);
    let c = put(&mut app, P0, "gi", 10, 10);
    send(
        &mut app,
        Command::Scatter {
            units: vec![c, b, a],
        },
    );
    run(&mut app, 150);
    assert_ne!(at(&mut app, a), at(&mut app, b));
    assert_ne!(at(&mut app, b), at(&mut app, c));
    let tank = put(&mut app, P0, "grizzly", 20, 20);
    let victim = put(&mut app, P1, "conscript", 23, 20);
    let e = entity(&mut app, victim).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    let mut events = send(&mut app, Command::Guard { units: vec![tank] });
    events.extend(run(&mut app, 150));
    assert!(events.iter().any(
        |e| matches!(e,SimEvent::WeaponFired{attacker,target,..}if *attacker==tank&&*target==victim)
    ));
    assert_eq!(at(&mut app, tank), CellPos::new(20, 20));
    assert!(
        app.world()
            .resource::<TacticalState>()
            .orders
            .contains_key(&tank)
    );
}
#[test]
fn ra7_ground_and_allied_force_fire_use_damage_reload_and_source_credit() {
    let mut app = world();
    let a = put(&mut app, P0, "grizzly", 10, 10);
    let b = put(&mut app, P0, "rhino", 13, 10);
    tick(&mut app, vec![]);
    let before = hp(&mut app, b);
    let mut events = send(
        &mut app,
        Command::ForceFire {
            units: vec![a],
            target: ForceTarget::Entity(b),
        },
    );
    events.extend(run(&mut app, 30));
    assert!(hp(&mut app, b) < before);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==a))
            .count(),
        1,
        "reload must apply"
    );
    send(&mut app, Command::Stop { units: vec![a] });
    run(&mut app, 120);
    let before = hp(&mut app, b);
    send(
        &mut app,
        Command::ForceFire {
            units: vec![a],
            target: ForceTarget::Cell(CellPos::new(13, 10)),
        },
    );
    run(&mut app, 30);
    assert!(
        hp(&mut app, b) < before,
        "ground blast hits friendly occupant"
    );
}
#[test]
fn ra7_force_move_crushes_without_acquiring_and_waypoints_run_in_order() {
    let mut app = world();
    let tank = put(&mut app, P0, "grizzly", 10, 10);
    let victim = put(&mut app, P1, "conscript", 12, 10);
    let e = entity(&mut app, victim).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    let mut events = send(
        &mut app,
        Command::ForceMove {
            units: vec![tank],
            target: CellPos::new(14, 10),
        },
    );
    events.extend(run(&mut app, 150));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::UnitCrushed{unit,..}if *unit==victim))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==tank))
    );
    let unit = put(&mut app, P0, "engineer", 4, 4);
    let targets = [CellPos::new(8, 4), CellPos::new(8, 8), CellPos::new(4, 8)];
    for target in targets {
        send(
            &mut app,
            Command::QueueMove {
                units: vec![unit],
                target,
            },
        );
    }
    let events = run(&mut app, 450);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,SimEvent::MoveCompleted{unit:id,..}if *id==unit))
            .count(),
        3
    );
    assert_eq!(at(&mut app, unit), targets[2]);
}
#[test]
fn ra7_new_orders_reject_empty_foreign_offmap_duplicates_and_waypoint_overflow_atomically() {
    let mut app = world();
    let own = put(&mut app, P0, "grizzly", 10, 10);
    let foreign = put(&mut app, P1, "grizzly", 13, 10);
    tick(&mut app, vec![]);
    let make = |u: Vec<SimId>| {
        vec![
            Command::Guard { units: u.clone() },
            Command::Scatter { units: u.clone() },
            Command::Follow {
                units: u.clone(),
                target: own,
            },
            Command::ForceFire {
                units: u.clone(),
                target: ForceTarget::Cell(CellPos::new(5, 5)),
            },
            Command::ForceMove {
                units: u.clone(),
                target: CellPos::new(5, 5),
            },
            Command::QueueMove {
                units: u,
                target: CellPos::new(5, 5),
            },
        ]
    };
    for (u, reason) in [
        (vec![], RejectReason::NoUnits),
        (vec![foreign], RejectReason::NotOwned(foreign)),
        (vec![own, own], RejectReason::DuplicateUnit(own)),
    ] {
        for c in make(u) {
            let e = send(&mut app, c);
            assert!(
                e.iter()
                    .any(|e| matches!(e,SimEvent::CommandRejected{reason:r,..}if *r==reason)),
                "{e:?}"
            );
        }
    }
    let bad = CellPos::new(-1, 0);
    for c in [
        Command::ForceMove {
            units: vec![own],
            target: bad,
        },
        Command::QueueMove {
            units: vec![own],
            target: bad,
        },
        Command::ForceFire {
            units: vec![own],
            target: ForceTarget::Cell(bad),
        },
    ] {
        let e = send(&mut app, c);
        assert!(e.iter().any(|e|matches!(e,SimEvent::CommandRejected{reason:RejectReason::OutsideMap(c),..}if *c==bad)));
    }
    // All commands are processed in one tick: exactly32 pending waypoints, never unbounded.
    let events = tick(
        &mut app,
        (0..=sim::MAX_WAYPOINTS)
            .map(|_| {
                issue(
                    P0,
                    Command::QueueMove {
                        units: vec![own],
                        target: CellPos::new(40, 40),
                    },
                )
            })
            .collect(),
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                SimEvent::CommandRejected {
                    reason: RejectReason::AbilityUnavailable("Waypoint queue is full"),
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        app.world().resource::<TacticalState>().waypoints[&own].len(),
        sim::MAX_WAYPOINTS - 1
    );
}
#[test]
fn ra7_new_order_state_and_fog_affect_hash_and_replay_is_deterministic() {
    fn replay() -> Vec<u64> {
        let mut app = world();
        let a = put(&mut app, P0, "grizzly", 5, 5);
        let b = put(&mut app, P0, "engineer", 10, 5);
        send(
            &mut app,
            Command::Follow {
                units: vec![a],
                target: b,
            },
        );
        let mut h = vec![];
        for _ in 0..70 {
            tick(&mut app, vec![]);
            h.push(sim::state_hash(app.world_mut()));
        }
        h
    }
    assert_eq!(replay(), replay());
    let mut app = world();
    let a = put(&mut app, P0, "grizzly", 5, 5);
    let base = sim::state_hash(app.world_mut());
    app.world_mut()
        .resource_mut::<TacticalState>()
        .orders
        .insert(
            a,
            sim::TacticalOrder::Guard {
                anchor: CellPos::new(5, 5),
            },
        );
    assert_ne!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<TacticalState>()
        .orders
        .clear();
    assert_eq!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<TacticalState>()
        .waypoints
        .entry(a)
        .or_default()
        .push_back(CellPos::new(6, 6));
    assert_ne!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<TacticalState>()
        .waypoints
        .clear();
    assert_eq!(base, sim::state_hash(app.world_mut()));
    let mut previous = base;
    for order in [
        sim::TacticalOrder::Guard {
            anchor: CellPos::new(6, 5),
        },
        sim::TacticalOrder::Follow {
            target: sim::SimId(99),
        },
        sim::TacticalOrder::Follow {
            target: sim::SimId(100),
        },
    ] {
        app.world_mut()
            .resource_mut::<TacticalState>()
            .orders
            .insert(a, order);
        let hash = sim::state_hash(app.world_mut());
        assert_ne!(previous, hash);
        previous = hash;
    }
    app.world_mut()
        .resource_mut::<TacticalState>()
        .orders
        .clear();
    let e = entity(&mut app, a).unwrap();
    for target in [
        ForceTarget::Cell(CellPos::new(10, 10)),
        ForceTarget::Cell(CellPos::new(11, 10)),
        ForceTarget::Entity(sim::SimId(99)),
    ] {
        app.world_mut().get_mut::<sim::Weapon>(e).unwrap().order =
            Some(sim::AttackOrder::ForceFire { target });
        let hash = sim::state_hash(app.world_mut());
        assert_ne!(previous, hash);
        previous = hash;
    }
    app.world_mut().get_mut::<sim::Weapon>(e).unwrap().order = None;
    assert_eq!(base, sim::state_hash(app.world_mut()));
    let mut rules = app.world().resource::<sim::MatchState>().rules();
    rules.fog_of_war = !rules.fog_of_war;
    app.world_mut()
        .resource_mut::<sim::MatchState>()
        .set_rules(rules);
    assert_ne!(base, sim::state_hash(app.world_mut()));
}

#[test]
fn ra7_guard_uses_explicit_reference_guard_range_instead_of_longer_weapon_range() {
    let mut app = world();
    let v3 = put(&mut app, P0, "v3_launcher", 20, 20);
    let observer = put(&mut app, P0, "engineer", 26, 18);
    let enemy = put(&mut app, P1, "rhino", 30, 20);
    assert_eq!(
        defs(&app)
            .metadata(sim::ProductionItem::Unit(
                defs(&app).unit_kind("v3_launcher").unwrap()
            ))
            .unwrap()
            .guard_range,
        9
    );
    for id in [observer, enemy] {
        let e = entity(&mut app, id).unwrap();
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    }
    tick(&mut app, vec![]);
    let mut events = send(&mut app, Command::Guard { units: vec![v3] });
    events.extend(run(&mut app, 30));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==v3)),
        "10 cells outside GuardRange9: {events:?}"
    );
}
#[test]
fn ra7_force_ground_fire_spends_air_ammo_and_launches_a_missile_with_fixed_aim() {
    let mut app = world();
    put(&mut app, P0, "nuclear_reactor", 1, 1);
    put(&mut app, P0, "airforce_command", 3, 3);
    let jet = put(&mut app, P0, "harrier", 10, 10);
    let events = send(
        &mut app,
        Command::ForceFire {
            units: vec![jet],
            target: ForceTarget::Cell(CellPos::new(12, 10)),
        },
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==jet)),
        "{events:?}"
    );
    assert_eq!(
        app.world().resource::<sim::SpecialState>().ammo.get(&jet),
        Some(&0)
    );
    let launcher = put(&mut app, P0, "v3_launcher", 20, 20);
    let bystander = put(&mut app, P0, "grizzly", 26, 22);
    let aim = CellPos::new(28, 20);
    send(
        &mut app,
        Command::ForceFire {
            units: vec![launcher],
            target: ForceTarget::Cell(aim),
        },
    );
    let mut launched = false;
    for _ in 0..80 {
        if tick(&mut app, vec![])
            .iter()
            .any(|e| matches!(e,SimEvent::UnitLaunched{parent,..}if *parent==launcher))
        {
            launched = true;
            break;
        }
    }
    assert!(launched, "force ground fire must launch the real missile");
    send(
        &mut app,
        Command::Stop {
            units: vec![launcher],
        },
    );
    let events = run(&mut app, 400);
    assert!(events.iter().any(|e|matches!(e,SimEvent::MissileDetonated{parent,at,..}if *parent==launcher&&at.cell()==aim)),"missile retains original ground aim after Stop: {events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==bystander)),
        "an uncommanded weapon cannot acquire the ground sentinel: {events:?}"
    );
}
#[test]
fn ra7_follow_forgets_concealed_target_without_tracking_new_position_and_interact_clears_waypoints()
{
    let mut app = world();
    let follower = put(&mut app, P0, "engineer", 10, 10);
    let dog = put(&mut app, P0, "attack_dog", 15, 10);
    let spy = put(&mut app, P1, "spy", 18, 10);
    let e = entity(&mut app, dog).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    let events = send(
        &mut app,
        Command::Follow {
            units: vec![follower],
            target: spy,
        },
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    run(&mut app, 3);
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .orders
            .contains_key(&follower)
    );
    let carrier = put(&mut app, P0, "ifv", 8, 8);
    let unit = put(&mut app, P0, "gi", 7, 8);
    send(
        &mut app,
        Command::QueueMove {
            units: vec![unit],
            target: CellPos::new(20, 20),
        },
    );
    send(
        &mut app,
        Command::QueueMove {
            units: vec![unit],
            target: CellPos::new(25, 25),
        },
    );
    send(
        &mut app,
        Command::Interact {
            unit,
            target: carrier,
        },
    );
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .waypoints
            .contains_key(&unit)
    );
}

#[test]
fn ra7_force_fire_kill_credits_the_actual_attacker() {
    let mut app = world();
    let a = put(&mut app, P0, "grizzly", 10, 10);
    let b = put(&mut app, P1, "rhino", 13, 10);
    let e = entity(&mut app, b).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 1;
    tick(&mut app, vec![]);
    send(
        &mut app,
        Command::ForceFire {
            units: vec![a],
            target: ForceTarget::Entity(b),
        },
    );
    run(&mut app, 30);
    assert!(entity(&mut app, b).is_none());
    let e = entity(&mut app, a).unwrap();
    assert_eq!(app.world().get::<sim::Experience>(e).unwrap().value, 900);
}

#[test]
fn ra7_force_fire_backs_out_of_reference_minimum_range_before_launching() {
    let mut app = world();
    let launcher = put(&mut app, P0, "v3_launcher", 20, 20);
    let aim = CellPos::new(22, 20);
    send(
        &mut app,
        Command::ForceFire {
            units: vec![launcher],
            target: ForceTarget::Cell(aim),
        },
    );
    let events = run(&mut app, 300);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::UnitLaunched{parent,..}if *parent==launcher))
    );
    assert!(
        at(&mut app, launcher)
            .x
            .abs_diff(aim.x)
            .max(at(&mut app, launcher).y.abs_diff(aim.y))
            >= 5
    );
}

#[test]
fn ra7_deploy_and_chronoshift_clear_pending_waypoints() {
    let mut app = world();
    let gi = put(&mut app, P0, "gi", 10, 10);
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::QueueMove {
                    units: vec![gi],
                    target: CellPos::new(20, 20),
                },
            ),
            issue(
                P0,
                Command::QueueMove {
                    units: vec![gi],
                    target: CellPos::new(25, 25),
                },
            ),
            issue(P0, Command::Deploy { unit: gi }),
        ],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .waypoints
            .contains_key(&gi)
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .deployed
            .contains_key(&gi)
    );
    put(&mut app, P0, "nuclear_reactor", 0, 0);
    let sw = put(&mut app, P0, "chronosphere", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let tank = put(&mut app, P0, "grizzly", 14, 14);
    tick(&mut app, vec![]);
    let e = entity(&mut app, sw).unwrap();
    let kind = app.world().get::<sim::Building>(e).unwrap().kind;
    let charge = defs(&app)
        .metadata(sim::ProductionItem::Building(kind))
        .unwrap()
        .cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(sw, charge);
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::QueueMove {
                    units: vec![tank],
                    target: CellPos::new(20, 20),
                },
            ),
            issue(
                P0,
                Command::QueueMove {
                    units: vec![tank],
                    target: CellPos::new(25, 25),
                },
            ),
            issue(
                P0,
                Command::Chronoshift {
                    building: sw,
                    source: CellPos::new(14, 14),
                    target: CellPos::new(30, 30),
                },
            ),
        ],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert_eq!(at(&mut app, tank), CellPos::new(30, 30));
    assert!(
        !app.world()
            .resource::<TacticalState>()
            .waypoints
            .contains_key(&tank)
    );
}
