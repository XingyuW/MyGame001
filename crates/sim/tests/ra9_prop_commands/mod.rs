//! Historical preparation note: UNCOMPILED / UNRUN. Child module of the actual ra1_commands regression target.
//! Admission privacy is exercised through actual CommandQueue/Commands/SimSchedule.
//! The hidden Prop premise is controlled setup; current code has no native producer for it.
#[path = "../ra9_remaining_common/mod.rs"]
mod common;
use common::*;
const ACTORS: [&str; 3] = ["tanya", "crazy_ivan", "engineer"];
#[test]
fn ra9_ra1_each_actorless_interact_kind_has_known_unexplored_missing_and_controlled_hidden_boundaries()
 {
    for (feature, name) in KINDS {
        for actor in ACTORS {
            for case in ["known", "unexplored", "missing", "hidden"] {
                let mut app = world();
                let target = mapped(&mut app, feature, name);
                let source = put(
                    &mut app,
                    P0,
                    actor,
                    if case == "unexplored" { 2 } else { 18 },
                    20,
                );
                let planter = if actor == "engineer" && case == "known" {
                    Some(put(&mut app, P0, "crazy_ivan", 19, 21))
                } else {
                    None
                };
                let mut held = vec![source];
                held.extend(planter);
                ok(&tick(&mut app, vec![guard(&held)]));
                if let Some(ivan) = planter {
                    let full = prop_hp(&app, target).expect("host");
                    ok(&tick(&mut app, vec![interact(ivan, target)]));
                    let c = app
                        .world()
                        .resource::<SpecialState>()
                        .charges
                        .get(&target)
                        .expect("native Ivan plant");
                    assert!(!c.c4);
                    assert_eq!(c.source, ivan);
                    assert_eq!(c.damage, 400);
                    assert_eq!(prop_hp(&app, target), Some(full));
                    ok(&tick(&mut app, vec![guard(&[ivan])]));
                }
                let vision = app.world().resource::<Players>().get(P0).unwrap().vision();
                assert_eq!(vision.is_visible(HOST), case != "unexplored");
                if case == "unexplored" {
                    assert!(!vision.is_explored(HOST), "genuine native unknown terrain");
                }
                if case == "hidden" {
                    // Sole injected gameplay premise in this matrix; no native concealment is claimed.
                    app.world_mut()
                        .resource_mut::<SpecialState>()
                        .hidden
                        .insert((P0, target));
                }
                probe(&mut app, source, target);
                let named = if case == "missing" {
                    SimId(u32::MAX)
                } else {
                    target
                };
                let before = prop_hp(&app, target);
                let events = tick(&mut app, vec![interact(source, named)]);
                let s = app
                    .world()
                    .resource::<Probe>()
                    .commands
                    .as_ref()
                    .expect("actual Commands observer");
                if case == "known" {
                    ok(&events);
                    assert!(s.pending);
                    assert!(!s.guard);
                    assert!(s.source_hp.is_some_and(|h| h > 0));
                    assert_eq!(s.target_hp, before);
                } else {
                    rejected(&events, RejectReason::UnknownTarget(named));
                    assert!(s.guard, "validate before clear_orders");
                    assert!(!s.pending);
                    assert!(!s.charged);
                    assert!(
                        !app.world()
                            .resource::<SpecialState>()
                            .auto_defuses
                            .contains_key(&source)
                    );
                    assert_eq!(prop_hp(&app, target), before);
                }
                if case == "hidden" {
                    assert!(
                        s.hidden,
                        "Commands early refresh retains controlled Prop filter"
                    );
                    assert!(
                        !app.world()
                            .resource::<SpecialState>()
                            .hidden
                            .contains(&(P0, target)),
                        "regular Visibility recomputes it; this is not permanent/native Prop concealment"
                    );
                }
                assert!(entity(&mut app, target).is_none());
                cells(&mut app);
            }
        }
    }
}
fn lethal_world(feature: u8, name: &str, actor: &str) -> (bevy_app::App, SimId, SimId, SimId) {
    let mut app = world_of(immediate_definitions());
    let target = mapped(&mut app, feature, name);
    let source = put(&mut app, P0, actor, 2, 20);
    // This owned unarmed scout supplies native current sight and survives either radius1 strike.
    let scout = put(&mut app, P0, "engineer", 18, 20);
    let planter = if actor == "engineer" {
        Some(put(&mut app, P0, "crazy_ivan", 19, 21))
    } else {
        None
    };
    let launcher = silo(&mut app);
    let mut held = vec![source, scout];
    held.extend(planter);
    ok(&tick(&mut app, vec![guard(&held)]));
    if let Some(ivan) = planter {
        ok(&tick(&mut app, vec![interact(ivan, target)]));
        let c = app
            .world()
            .resource::<SpecialState>()
            .charges
            .get(&target)
            .expect("native bomb");
        assert!(!c.c4);
        assert_eq!(c.source, ivan);
        ok(&tick(&mut app, vec![guard(&[ivan])]));
    }
    warm(&mut app, launcher);
    assert!(hp(&mut app, source).is_some_and(|h| h > 0));
    assert!(prop_hp(&app, target).is_some_and(|h| h > 0));
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(HOST)
    );
    probe(&mut app, source, target);
    (app, source, target, launcher)
}
#[test]
fn ra9_ra1_real_commands_death_precedes_same_tick_actorless_interact_and_economy_with_survival_twins()
 {
    for (feature, name) in KINDS {
        for actor in ACTORS {
            for kill_source in [false, true] {
                for kill in [false, true] {
                    let (mut app, source, target, launcher) = lethal_world(feature, name, actor);
                    let source_before = hp(&mut app, source).expect("full live source");
                    let target_before = prop_hp(&app, target).expect("full live Prop");
                    let aim = if kill_source {
                        CellPos::new(2, 20)
                    } else {
                        HOST
                    };
                    assert!(
                        app.world()
                            .resource::<Players>()
                            .get(P0)
                            .unwrap()
                            .vision()
                            .is_visible(aim),
                        "genuine native aim visibility"
                    );
                    let mut commands = vec![];
                    if kill {
                        commands.push(strike(launcher, aim));
                    }
                    commands.push(interact(source, target));
                    let events = tick(&mut app, commands);
                    let (c, e) = {
                        let p = app.world().resource::<Probe>();
                        (
                            p.commands.as_ref().expect("after Commands").clone(),
                            p.economy.as_ref().expect("before Economy").clone(),
                        )
                    };
                    if kill {
                        strike_receipt(&app, &events, launcher, aim);
                        if kill_source {
                            rejected(&events, RejectReason::UnknownUnit(source));
                            assert!(c.source_hp.is_some_and(|h| h <= 0));
                            assert!(e.source_hp.is_some_and(|h| h <= 0));
                            assert_eq!(c.target_hp, Some(target_before));
                            assert_eq!(e.target_hp, Some(target_before));
                            assert!(unit_destroyed(&events, source));
                            assert!(hp(&mut app, source).is_none());
                            assert_eq!(prop_hp(&app, target), Some(target_before));
                        } else {
                            rejected(&events, RejectReason::UnknownTarget(target));
                            assert!(c.target_hp.is_some_and(|h| h <= 0));
                            assert!(e.target_hp.is_some_and(|h| h <= 0));
                            assert_eq!(c.source_hp, Some(source_before));
                            assert_eq!(e.source_hp, Some(source_before));
                            assert!(c.guard, "rejected Interact leaves previous Guard intact");
                            assert!(prop_destroyed(&events, target));
                            assert!(prop_hp(&app, target).is_none());
                            assert_eq!(hp(&mut app, source), Some(source_before));
                        }
                        assert!(!c.pending);
                        assert!(!e.pending, "no dead-endpoint work admitted before Economy");
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .interactions
                                .contains_key(&source)
                        );
                    } else {
                        ok(&events);
                        assert_eq!(c.source_hp, Some(source_before));
                        assert_eq!(c.target_hp, Some(target_before));
                        assert!(
                            c.pending,
                            "the exact long-range Interact is accepted in the living twin"
                        );
                        assert!(!c.guard);
                        assert_eq!(e.source_hp, Some(source_before));
                        assert_eq!(e.target_hp, Some(target_before));
                        assert!(!unit_destroyed(&events, source));
                        assert!(!prop_destroyed(&events, target));
                    }
                    cells(&mut app);
                }
            }
        }
    }
}
#[test]
fn ra9_ra1_native_already_dead_sources_and_props_are_missing_for_all_added_interact_kinds() {
    for (feature, name) in KINDS {
        for actor in ACTORS {
            for kill_source in [false, true] {
                let (mut app, source, target, launcher) = lethal_world(feature, name, actor);
                let aim = if kill_source {
                    CellPos::new(2, 20)
                } else {
                    HOST
                };
                assert!(
                    app.world()
                        .resource::<Players>()
                        .get(P0)
                        .unwrap()
                        .vision()
                        .is_visible(aim),
                    "genuine native aim visibility"
                );
                let events = tick(&mut app, vec![strike(launcher, aim)]);
                ok(&events);
                strike_receipt(&app, &events, launcher, aim);
                let c = app
                    .world()
                    .resource::<Probe>()
                    .commands
                    .as_ref()
                    .expect("pre-Cleanup receipt");
                if kill_source {
                    assert!(c.source_hp.is_some_and(|h| h <= 0));
                    assert!(unit_destroyed(&events, source));
                    assert!(hp(&mut app, source).is_none());
                    assert!(prop_hp(&app, target).is_some_and(|h| h > 0));
                } else {
                    assert!(c.target_hp.is_some_and(|h| h <= 0));
                    assert!(prop_destroyed(&events, target));
                    assert!(prop_hp(&app, target).is_none());
                    assert!(hp(&mut app, source).is_some_and(|h| h > 0));
                }
                let events = tick(&mut app, vec![interact(source, target)]);
                rejected(
                    &events,
                    if kill_source {
                        RejectReason::UnknownUnit(source)
                    } else {
                        RejectReason::UnknownTarget(target)
                    },
                );
                assert!(
                    !app.world()
                        .resource::<SpecialState>()
                        .interactions
                        .contains_key(&source)
                );
                cells(&mut app);
            }
        }
    }
}

#[test]
fn ra9_ra1_a_real_pending_prop_approach_cannot_complete_after_native_commands_endpoint_death() {
    for (feature, name) in KINDS {
        for actor in ACTORS {
            for kill_source in [false, true] {
                for kill in [false, true] {
                    let (mut app, source, target, launcher) = lethal_world(feature, name, actor);
                    ok(&tick(&mut app, vec![interact(source, target)]));
                    assert_eq!(
                        app.world()
                            .resource::<SpecialState>()
                            .interactions
                            .get(&source),
                        Some(&target)
                    );
                    let mut travelling = false;
                    for _ in 0..500 {
                        let at = cell(&mut app, source);
                        if (8..=14).contains(&at.x) {
                            travelling = true;
                            break;
                        }
                        tick(&mut app, vec![]);
                    }
                    assert!(
                        travelling,
                        "actual public Interact must create and advance a long native approach"
                    );
                    assert_eq!(
                        app.world()
                            .resource::<SpecialState>()
                            .interactions
                            .get(&source),
                        Some(&target)
                    );
                    let at = cell(&mut app, source);
                    let en = entity(&mut app, source).unwrap();
                    let position = app.world().get::<sim::Position>(en).unwrap().0;
                    let centre = sim::SubPos::centre_of(at);
                    let dx = i64::from(position.x) - i64::from(centre.x);
                    let dy = i64::from(position.y) - i64::from(centre.y);
                    assert!(
                        dx * dx + dy * dy <= 256 * 256,
                        "real current standing point is in this declared fixture blast"
                    );
                    assert!(
                        at.x.abs_diff(HOST.x) > 1,
                        "source survives target strike; target survives source strike"
                    );
                    if let Some(c) = app.world().resource::<SpecialState>().charges.get(&target) {
                        assert!(
                            c.due > app.world().resource::<SimTick>().0 + 2,
                            "no expiry may masquerade as liveness"
                        );
                    }
                    let source_before = hp(&mut app, source).unwrap();
                    let target_before = prop_hp(&app, target).unwrap();
                    let charge_before = app
                        .world()
                        .resource::<SpecialState>()
                        .charges
                        .get(&target)
                        .map(|c| c.due);
                    let aim = if kill_source { at } else { HOST };
                    assert!(
                        app.world()
                            .resource::<Players>()
                            .get(P0)
                            .unwrap()
                            .vision()
                            .is_visible(aim),
                        "genuine native aim visibility"
                    );
                    let events = tick(
                        &mut app,
                        if kill {
                            vec![strike(launcher, aim)]
                        } else {
                            vec![]
                        },
                    );
                    ok(&events);
                    let (c, e) = {
                        let p = app.world().resource::<Probe>();
                        (p.commands.clone().unwrap(), p.economy.clone().unwrap())
                    };
                    assert!(
                        c.pending && e.pending,
                        "the old order still exists before Economy; it was not pre-cleared setup"
                    );
                    if kill {
                        strike_receipt(&app, &events, launcher, aim);
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .interactions
                                .contains_key(&source),
                            "later native special advance must invalidate the pending source/target"
                        );
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .auto_defuses
                                .contains_key(&source)
                        );
                        if kill_source {
                            assert!(c.source_hp.is_some_and(|h| h <= 0));
                            assert!(e.source_hp.is_some_and(|h| h <= 0));
                            assert_eq!(c.target_hp, Some(target_before));
                            assert!(unit_destroyed(&events, source));
                            assert!(hp(&mut app, source).is_none());
                            assert_eq!(prop_hp(&app, target), Some(target_before));
                            assert_eq!(
                                app.world()
                                    .resource::<SpecialState>()
                                    .charges
                                    .get(&target)
                                    .map(|c| c.due),
                                charge_before,
                                "dead approaching C4/Ivan/Engineer cannot create/remove a charge"
                            );
                        } else {
                            assert!(c.target_hp.is_some_and(|h| h <= 0));
                            assert!(e.target_hp.is_some_and(|h| h <= 0));
                            assert_eq!(c.source_hp, Some(source_before));
                            assert!(prop_destroyed(&events, target));
                            assert!(prop_hp(&app, target).is_none());
                            assert_eq!(hp(&mut app, source), Some(source_before));
                            assert!(
                                !app.world()
                                    .resource::<SpecialState>()
                                    .charges
                                    .contains_key(&target)
                            );
                        }
                    } else {
                        assert_eq!(c.source_hp, Some(source_before));
                        assert_eq!(c.target_hp, Some(target_before));
                        assert!(
                            app.world()
                                .resource::<SpecialState>()
                                .interactions
                                .contains_key(&source),
                            "surviving twin still executes its real pending approach"
                        );
                        assert_eq!(prop_hp(&app, target), Some(target_before));
                        assert_eq!(hp(&mut app, source), Some(source_before));
                    }
                    cells(&mut app);
                }
            }
        }
    }
}
