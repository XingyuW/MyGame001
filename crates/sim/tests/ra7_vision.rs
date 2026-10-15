//! RA7 vision acceptance through shipped commands and SimSchedule.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{Cell, CellPos, Command, Grid, Players, ProductionItem, SimEvent};
#[test]
fn ra7_new_spy_is_concealed_before_first_combat_and_command() {
    let mut app = world();
    let shooter = put(&mut app, P0, "grizzly", 10, 10);
    tick(&mut app, vec![]);
    let spy = put(&mut app, P1, "spy", 12, 10);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![shooter],
                target: spy,
            },
        )],
    );
    assert!(events.iter().any(|e| matches!(e, SimEvent::CommandRejected { reason: sim::RejectReason::UnknownTarget(id), .. } if *id == spy)), "first command must not identify a new spy: {events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::WeaponFired { target, .. } if *target == spy)),
        "new spy attacked before concealment: {events:?}"
    );
}

fn fog(app: &mut App, on: bool) {
    let mut rules = app.world().resource::<sim::MatchState>().rules();
    rules.fog_of_war = on;
    app.world_mut()
        .resource_mut::<sim::MatchState>()
        .set_rules(rules);
}
#[test]
fn ra7_explored_enemy_is_known_with_fog_off_but_hidden_with_fog_on() {
    for on in [false, true] {
        let mut app = world();
        fog(&mut app, on);
        let scout = put(&mut app, P0, "grizzly", 10, 10);
        let enemy = put(&mut app, P1, "rhino", 17, 10);
        // No combat disturbs this visibility scenario.
        for id in [scout, enemy] {
            let e = entity(&mut app, id).unwrap();
            app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        }
        tick(&mut app, vec![]);
        assert!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .vision()
                .is_visible(CellPos::new(17, 10))
        );
        tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![scout],
                    target: CellPos::new(3, 3),
                },
            )],
        );
        for _ in 0..400 {
            tick(&mut app, vec![]);
        }
        let view = app.world().resource::<Players>().get(P0).unwrap().vision();
        assert!(view.is_explored(CellPos::new(17, 10)));
        assert_eq!(view.is_visible(CellPos::new(17, 10)), !on);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Follow {
                    units: vec![scout],
                    target: enemy,
                },
            )],
        );
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
            on,
            "{events:?}"
        );
    }
}
#[test]
fn ra7_gap_restores_circle_shroud_every_tick_but_local_sight_and_allies_counter_it() {
    let mut app = world();
    put(&mut app, P0, "nuclear_reactor", 1, 1);
    put(&mut app, P1, "nuclear_reactor", 40, 40);
    put(&mut app, P0, "spy_satellite", 3, 3);
    put(&mut app, P1, "gap_generator", 24, 24);
    tick(&mut app, vec![]);
    let v = app.world().resource::<Players>().get(P0).unwrap().vision();
    assert!(!v.is_explored(CellPos::new(34, 24)));
    assert!(v.is_visible(CellPos::new(35, 24)));
    assert!(v.is_visible(CellPos::new(34, 34)), "circular boundary");
    let scout = put(&mut app, P0, "engineer", 30, 24);
    tick(&mut app, vec![]);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(CellPos::new(30, 24))
    );
    let e = entity(&mut app, scout).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_explored(CellPos::new(30, 24))
    );
    app.world_mut().resource_mut::<Players>().set_team(P0, 1);
    app.world_mut().resource_mut::<Players>().set_team(P1, 1);
    tick(&mut app, vec![]);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(CellPos::new(24, 24))
    );
}
#[test]
fn ra7_psychic_order_boundary_power_death_and_special_concealment() {
    let mut app = world();
    let power = put(&mut app, P0, "nuclear_reactor", 1, 1);
    let sensor = put(&mut app, P0, "psychic_sensor", 20, 20);
    let enemy = put(&mut app, P1, "engineer", 40, 30);
    let spy = put(&mut app, P1, "spy", 40, 31);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![
            issue(
                P1,
                Command::Move {
                    units: vec![enemy],
                    target: CellPos::new(35, 20),
                },
            ),
            issue(
                P1,
                Command::Move {
                    units: vec![spy],
                    target: CellPos::new(35, 20),
                },
            ),
        ],
    );
    let lines = sim::psychic_orders(app.world(), P0);
    assert_eq!(lines.len(), 1, "concealed spy must not leak: {lines:?}");
    assert_eq!(lines[0].target, CellPos::new(35, 20));
    tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![enemy],
                target: CellPos::new(36, 20),
            },
        )],
    );
    assert!(sim::psychic_orders(app.world(), P0).is_empty());
    tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![enemy],
                target: CellPos::new(35, 20),
            },
        )],
    );
    let e = entity(&mut app, power).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    tick(&mut app, vec![]);
    assert!(sim::psychic_orders(app.world(), P0).is_empty());
    let e = entity(&mut app, sensor).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    assert!(sim::psychic_orders(app.world(), P0).is_empty());
}
#[test]
fn ra7_sensor_detects_spy_at_fifteen_but_not_sixteen_and_needs_power() {
    let mut app = world();
    let power = put(&mut app, P0, "nuclear_reactor", 1, 1);
    put(&mut app, P0, "psychic_sensor", 20, 20);
    let a = put(&mut app, P1, "spy", 35, 20);
    let b = put(&mut app, P1, "spy", 36, 20);
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, a))
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, b))
    );
    let e = entity(&mut app, power).unwrap();
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .unwrap()
        .hit_points = 0;
    tick(&mut app, vec![]);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, a))
    );
}
#[test]
fn ra7_new_cloaked_sub_is_hidden_before_automatic_first_combat() {
    let mut app = world();
    let mut cells = vec![OPEN; 48 * 48];
    cells[10 * 48 + 12] = Cell {
        water: true,
        passable: false,
        buildable: false,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, cells).unwrap());
    put(&mut app, P0, "grizzly", 10, 10);
    tick(&mut app, vec![]);
    let sub = put(&mut app, P1, "typhoon", 12, 10);
    let events = tick(&mut app, vec![]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{target,..}if *target==sub)),
        "{events:?}"
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, sub))
    );
}
#[test]
fn ra7_paid_spy_production_is_concealed_before_automatic_spawn_tick_fire() {
    let mut app = world();
    let barracks = put(&mut app, P1, "barracks", 14, 10);
    put(&mut app, P1, "tech_center", 35, 35);
    put(&mut app, P1, "nuclear_reactor", 40, 1);
    let shooter = put(&mut app, P0, "grizzly", 10, 10);
    let e = entity(&mut app, shooter).unwrap();
    let weapon = *app.world().get::<sim::Weapon>(e).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    let kind = defs(&app).unit_kind("spy").unwrap();
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    let mut spawned = false;
    for _ in 0..3000 {
        let about = app
            .world()
            .resource::<Players>()
            .get(P1)
            .unwrap()
            .queues()
            .any(|(_, q)| {
                q.first().is_some_and(|q| {
                    q.item == ProductionItem::Unit(kind)
                        && q.progress + 1 >= defs(&app).production(q.item).unwrap().build_time
                })
            });
        if about {
            app.world_mut().entity_mut(e).insert(weapon);
        }
        let events = tick(&mut app, vec![]);
        if let Some(id) = events.iter().find_map(|e| match e {
            SimEvent::UnitSpawned { unit, building, .. } if *building == barracks => Some(*unit),
            _ => None,
        }) {
            assert!(about, "weapon must be live on actual spawn tick");
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e,SimEvent::WeaponFired{target,..}if *target==id)),
                "{events:?}"
            );
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .hidden
                    .contains(&(P0, id))
            );
            spawned = true;
            break;
        }
    }
    assert!(spawned, "actual production path must spawn the Spy");
}

#[test]
fn ra7_detector_killed_by_an_earlier_command_cannot_reveal_spy_to_later_attack() {
    let mut app = world();
    let shooter = put(&mut app, P0, "grizzly", 10, 10);
    let dog = put(&mut app, P0, "attack_dog", 15, 10);
    let spy = put(&mut app, P1, "spy", 12, 10);
    put(&mut app, P1, "nuclear_reactor", 35, 35);
    let curtain = put(&mut app, P1, "iron_curtain", 1, 1);
    let e = entity(&mut app, dog).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    let e = entity(&mut app, shooter).unwrap();
    let weapon = *app.world().get::<sim::Weapon>(e).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, spy))
    );
    app.world_mut().entity_mut(e).insert(weapon);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(curtain, 6000);
    let events = tick(
        &mut app,
        vec![
            issue(
                P1,
                Command::Superweapon {
                    building: curtain,
                    target: CellPos::new(15, 10),
                    units: vec![],
                },
            ),
            issue(
                P0,
                Command::Attack {
                    units: vec![shooter],
                    target: spy,
                },
            ),
        ],
    );
    assert!(events.iter().any(|e|matches!(e,SimEvent::CommandRejected{reason:sim::RejectReason::UnknownTarget(id),..}if *id==spy)),"{events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{target,..}if *target==spy))
    );
    assert!(entity(&mut app, dog).is_none());
}

#[test]
fn ra7_selling_the_only_sensor_hides_spy_from_later_same_tick_regular_commands() {
    let mut app = world();
    put(&mut app, P0, "nuclear_reactor", 1, 1);
    let sensor = put(&mut app, P0, "psychic_sensor", 15, 14);
    let shooter = put(&mut app, P0, "grizzly", 10, 10);
    let spy = put(&mut app, P1, "spy", 12, 10);
    let e = entity(&mut app, shooter).unwrap();
    let weapon = *app.world().get::<sim::Weapon>(e).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, spy))
    );
    app.world_mut().entity_mut(e).insert(weapon);
    let events = tick(
        &mut app,
        vec![
            issue(P0, Command::SellBuilding { building: sensor }),
            issue(
                P0,
                Command::Attack {
                    units: vec![shooter],
                    target: spy,
                },
            ),
            issue(
                P0,
                Command::Follow {
                    units: vec![shooter],
                    target: spy,
                },
            ),
        ],
    );
    assert_eq!(events.iter().filter(|e|matches!(e,SimEvent::CommandRejected{reason:sim::RejectReason::UnknownTarget(id),..}if *id==spy)).count(),2,"Sensor sale must affect later same-batch commands: {events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{target,..}if *target==spy))
    );
}

#[test]
fn ra7_stopping_moving_mirage_conceals_it_from_later_same_tick_attack() {
    let mut app = world();
    let shooter = put(&mut app, P0, "grizzly", 10, 10);
    let mirage = put(&mut app, P1, "mirage_tank", 12, 10);
    let e = entity(&mut app, shooter).unwrap();
    let weapon = *app.world().get::<sim::Weapon>(e).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    // A blocked but valid movement order leaves the Mirage at its cell centre with an
    // active order. Stop changes concealment immediately, before the Movement stage.
    for (x, y) in [
        (11, 9),
        (12, 9),
        (13, 9),
        (11, 10),
        (13, 10),
        (11, 11),
        (12, 11),
        (13, 11),
    ] {
        let blocker = put(&mut app, P1, "rhino", x, y);
        let b = entity(&mut app, blocker).unwrap();
        app.world_mut().entity_mut(b).remove::<sim::Weapon>();
    }
    tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![mirage],
                target: CellPos::new(25, 10),
            },
        )],
    );
    let m = entity(&mut app, mirage).unwrap();
    assert!(app.world().get::<sim::Mobile>(m).unwrap().step.is_none());
    assert!(app.world().get::<sim::Orders>(m).unwrap().current.is_some());
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, mirage))
    );
    app.world_mut().entity_mut(e).insert(weapon);
    let events = tick(
        &mut app,
        vec![
            issue(
                P1,
                Command::Stop {
                    units: vec![mirage],
                },
            ),
            issue(
                P0,
                Command::Attack {
                    units: vec![shooter],
                    target: mirage,
                },
            ),
        ],
    );
    assert!(events.iter().any(|e|matches!(e,SimEvent::CommandRejected{reason:sim::RejectReason::UnknownTarget(id),..}if *id==mirage)),"{events:?}");
}
