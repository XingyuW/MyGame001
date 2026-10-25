//! Native current ownership/visibility boundaries; omitted source flags gain no read credit.
//! Public spawns/promote are controlled setup; payment, commands and observations are native.
mod ra4_common;
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{Command, Health, Players, ProductionItem, SimEvent, SpecialState, Weapon};
fn checked(app: &mut App, commands: Vec<sim::IssuedCommand>) -> Vec<SimEvent> {
    let events = tick(app, commands);
    ra4_common::assert_cells(app, "RA9 ownership/privacy every tick");
    assert!(
        app.world()
            .resource::<Players>()
            .iter()
            .all(|(_, p)| p.credits() >= 0)
    );
    events
}
#[test]
fn ra9_paid_placement_uses_own_anchor_but_not_allied_or_hostile_owner() {
    for (anchor_owner, allied) in [(P0, false), (P1, true), (P1, false)] {
        let mut app = world();
        if allied {
            let mut players = app.world_mut().resource_mut::<Players>();
            assert!(players.set_team(P0, 1));
            assert!(players.set_team(P1, 1));
        }
        put(&mut app, P0, "construction_yard", 1, 1);
        let anchor = put(&mut app, anchor_owner, "construction_yard", 20, 20);
        put(&mut app, P0, "engineer", 24, 18); // real sight, no visibility mutation
        let d = defs(&app);
        let power = d.building_kind("power_plant").unwrap();
        let terms = d.production(ProductionItem::Building(power)).unwrap();
        assert_eq!((terms.cost, terms.build_time), (800, 672));
        let before = app.world().resource::<Players>().get(P0).unwrap().credits();
        let events = checked(
            &mut app,
            vec![issue(
                P0,
                Command::QueueProduction {
                    item: ProductionItem::Building(power),
                },
            )],
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
        );
        for _ in 0..terms.build_time {
            checked(&mut app, vec![]);
        }
        assert_eq!(
            app.world().resource::<Players>().get(P0).unwrap().credits(),
            before - 800
        );
        let e = entity(&mut app, anchor).unwrap();
        assert_eq!(app.world().get::<sim::Owner>(e).unwrap().0, anchor_owner);
        assert_eq!(app.world().resource::<Players>().hostile(P0, P1), !allied);
        let origin = sim::CellPos::new(23, 20);
        let events = checked(
            &mut app,
            vec![issue(
                P0,
                Command::PlaceBuilding {
                    building: power,
                    origin,
                },
            )],
        );
        if anchor_owner == P0 {
            assert!(events.iter().any(|e|matches!(e,SimEvent::BuildingPlaced{player,kind,..}if *player==P0&&*kind==power)),"{events:?}");
            assert!(
                app.world()
                    .resource::<sim::Grid>()
                    .building(origin)
                    .is_some()
            );
        } else {
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    SimEvent::CommandRejected {
                        reason: sim::RejectReason::Placement(sim::PlacementError::TooFar { .. }),
                        ..
                    }
                )),
                "allied={allied}: {events:?}"
            );
            assert!(
                app.world()
                    .resource::<sim::Grid>()
                    .building(origin)
                    .is_none()
            );
            let player = app.world().resource::<Players>().get(P0).unwrap();
            let head = &player.queues().next().unwrap().1[0];
            assert_eq!(head.item, ProductionItem::Building(power));
            assert_eq!((head.progress, head.charged), (672, 800));
        }
        assert_eq!(
            app.world().resource::<Players>().get(P0).unwrap().credits(),
            before - 800
        );
    }
}
#[test]
fn ra9_base_elite_sniper_shots_and_spy_interaction_do_not_add_shooter_reveal() {
    for elite in [false, true] {
        let mut app = world();
        put(&mut app, P0, "engineer", 18, 9);
        let target = put(&mut app, P1, "mcv", 20, 10);
        let target_entity = entity(&mut app, target).unwrap();
        app.world_mut().entity_mut(target_entity).remove::<Weapon>(); // controlled unarmed target
        checked(&mut app, vec![]);
        checked(&mut app, vec![]);
        let source_cell = sim::CellPos::new(10, 10);
        assert!(
            !app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_visible(source_cell)
        );
        assert!(
            !app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_explored(source_cell)
        );
        let source = put(&mut app, P0, "sniper", 10, 10);
        if elite {
            assert!(sim::promote(app.world_mut(), source, sim::ELITE));
        }
        let d = defs(&app);
        let selected = d.weapon_kind(if elite { "awp_e" } else { "awp" }).unwrap();
        let source_entity = entity(&mut app, source).unwrap();
        assert_eq!(
            app.world().get::<Weapon>(source_entity).unwrap().kind,
            selected
        );
        let before = app.world().get::<Health>(target_entity).unwrap().hit_points;
        let mut events = checked(
            &mut app,
            vec![issue(
                P0,
                Command::Attack {
                    units: vec![source],
                    target,
                },
            )],
        );
        for _ in 0..3 {
            events.extend(checked(&mut app, vec![]));
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        assert_eq!(events.iter().filter(|e|matches!(e,SimEvent::WeaponFired{attacker,target:found,weapon}if *attacker==source&&*found==target&&*weapon==selected)).count(),1);
        assert_eq!(
            app.world().get::<Health>(target_entity).unwrap().hit_points,
            before - 1,
            "stock125 x heavy1% floors to1 for both current ranks"
        );
        let vision = app.world().resource::<Players>().get(P1).unwrap().vision();
        assert!(!vision.is_visible(source_cell));
        assert!(!vision.is_explored(source_cell));
        // Actual ordinary sight is a positive control, independent of the shot/nonread flag.
        put(&mut app, P1, "engineer", 12, 10);
        checked(&mut app, vec![]);
        assert!(
            app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_visible(source_cell)
        );
    }
    let mut app = world();
    let spy = put(&mut app, P0, "spy", 10, 10);
    let target = put(&mut app, P1, "conscript", 11, 10);
    let e = entity(&mut app, target).unwrap();
    app.world_mut().entity_mut(e).remove::<Weapon>();
    checked(&mut app, vec![]);
    checked(&mut app, vec![]);
    assert!(
        app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P1, spy))
    );
    let before = app.world().get::<Health>(e).unwrap().hit_points;
    let events = checked(
        &mut app,
        vec![issue(P0, Command::Interact { unit: spy, target })],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==spy)),
        "MakeupKit is an interaction, never claim it fired"
    );
    assert_eq!(app.world().get::<Health>(e).unwrap().hit_points, before);
    assert_eq!(
        app.world().resource::<SpecialState>().disguise_for(P1, spy),
        Some(defs(&app).unit_kind("conscript").unwrap())
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P1, spy))
    );
    put(&mut app, P1, "attack_dog", 14, 10);
    checked(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P1, spy)),
        "real source-supported detector positive"
    );
}
