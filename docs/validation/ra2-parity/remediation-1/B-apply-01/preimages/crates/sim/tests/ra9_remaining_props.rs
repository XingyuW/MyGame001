//! UNCOMPILED / UNRUN. Native full erasure, Ivan expiry, and explicit/automatic DefuseKit paths.
mod ra9_remaining_common;
use ra9_remaining_common::*;
use std::collections::BTreeSet;
fn occupied(feature: u8, name: &str) -> (bevy_app::App, SimId, SimId, SimId) {
    let mut app = world();
    let target = mapped(&mut app, feature, name);
    let occupant = put(&mut app, P1, "conscript", 19, 20);
    let source = put(&mut app, P0, "chrono_legionnaire", 13, 20); // outside initial infantry range
    let bait = put(&mut app, P0, "apocalypse", 19, 21);
    passive_before_tick(&mut app, bait);
    ok(&tick(&mut app, vec![guard(&[source])]));
    let mut events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: occupant,
                target,
            },
        )],
    );
    ok(&events);
    assert!(entity(&mut app, occupant).is_none());
    assert_eq!(
        app.world().resource::<SpecialState>().garrisons[&target].owner,
        P1
    );
    assert_eq!(
        app.world().resource::<SpecialState>().cargo[&target]
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![occupant]
    );
    let mut host_fired = events
        .iter()
        .any(|e| matches!(e,SimEvent::WeaponFired {attacker,..} if *attacker==target));
    for _ in 0..100 {
        if host_fired {
            break;
        }
        events = tick(&mut app, vec![]);
        host_fired |= events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired {attacker,..} if *attacker==target));
    }
    assert!(host_fired, "native occupation/fire premise");
    ok(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![source],
                target: CellPos::new(16, 20),
            },
        )],
    ));
    ok(&tick(&mut app, vec![guard(&[source])]));
    run(&mut app, 100); // real teleport disable/reload interval, never edited
    assert_eq!(cell(&mut app, source), CellPos::new(16, 20));
    assert!(hp(&mut app, bait).is_some_and(|h| h > 0));
    (app, target, source, occupant)
}
fn start_beam(app: &mut bevy_app::App, source: SimId, target: SimId) {
    let events = tick(
        app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![source],
                target,
            },
        )],
    );
    ok(&events);
    assert!(
        fired(&events, source, target),
        "actual native Temporal shot"
    );
    let state = app.world().resource::<SpecialState>();
    let beam = state
        .erasures
        .get(&target)
        .expect("actual mapped Prop channel");
    assert_eq!(beam.beams.len(), 1);
    assert_eq!(beam.beams.get(&source), Some(&8));
}
#[test]
fn ra9_full_sustained_native_temporal_erases_each_real_occupied_prop_and_releases_terrain() {
    for (feature, name) in KINDS {
        let (mut app, target, source, occupant) = occupied(feature, name);
        let before = prop_hp(&app, target).expect("full host");
        start_beam(&mut app, source, target);
        let mut prior = app.world().resource::<SpecialState>().erasures[&target].remaining;
        assert_eq!(prior, u64::try_from(before).unwrap() * 10 - 6);
        let mut erased = false;
        let mut reload_shots = 0;
        for _ in 0..2000 {
            let events = tick(&mut app, vec![]);
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e,SimEvent::WeaponFired {attacker,..} if *attacker==target)),
                "occupied host cannot fire during sustained erasure"
            );
            if fired(&events, source, target) {
                reload_shots += 1;
            }
            if prop_hp(&app, target).is_none() {
                assert!(
                    prop_destroyed(&events, target),
                    "actual Cleanup event required"
                );
                erased = true;
                break;
            }
            assert_eq!(
                prop_hp(&app, target),
                Some(before),
                "Temporal retains actual HP before erasure"
            );
            let remaining = app
                .world()
                .resource::<SpecialState>()
                .erasures
                .get(&target)
                .expect("retained across reload")
                .remaining;
            assert_eq!(prior - remaining, 6);
            prior = remaining;
        }
        assert!(erased);
        assert!(
            reload_shots > 0,
            "actual sustained reloads, not seeded beam state"
        );
        assert!(hp(&mut app, source).is_some_and(|h| h > 0));
        assert!(entity(&mut app, target).is_none());
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .erasures
                .contains_key(&target)
        );
        assert!(
            app.world()
                .resource::<SpecialState>()
                .temporal_removed
                .contains(&target)
        );
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .garrisons
                .contains_key(&target)
        );
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .cargo
                .contains_key(&target)
        );
        assert!(
            entity(&mut app, occupant).is_some(),
            "existing native garrison evacuation returns saved passenger"
        );
        let c = app.world().resource::<Grid>().cell(HOST).expect("terrain");
        assert!(c.passable && c.buildable);
        cells(&mut app);
    }
}
#[test]
fn ra9_real_beam_break_keeps_each_same_occupied_host_alive_for_the_full_uninterrupted_erase_window()
{
    for (feature, name) in KINDS {
        let (mut app, target, source, _) = occupied(feature, name);
        let before = prop_hp(&app, target).unwrap();
        start_beam(&mut app, source, target);
        run(&mut app, 10);
        ok(&tick(&mut app, vec![guard(&[source])]));
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .erasures
                .contains_key(&target)
        );
        let events = run(&mut app, 1800);
        assert!(!prop_destroyed(&events, target));
        assert_eq!(prop_hp(&app, target), Some(before));
        assert!(
            events
                .iter()
                .any(|e| matches!(e,SimEvent::WeaponFired {attacker,..} if *attacker==target)),
            "host resumes actual fire after real beam break"
        );
        assert!(hp(&mut app, source).is_some_and(|h| h > 0));
        cells(&mut app);
    }
}
#[test]
fn ra9_real_commands_source_or_prop_death_releases_a_genuine_temporal_channel_with_survival_twins()
{
    for (feature, name) in KINDS {
        for kill_source in [false, true] {
            for kill in [false, true] {
                let mut app = world_of(immediate_definitions());
                let target = mapped(&mut app, feature, name);
                let source = put(&mut app, P0, "chrono_legionnaire", 16, 20);
                let scout = put(&mut app, P0, "engineer", 18, 20);
                let launcher = silo(&mut app);
                ok(&tick(&mut app, vec![guard(&[source, scout])]));
                warm(&mut app, launcher);
                start_beam(&mut app, source, target);
                run(&mut app, 10);
                let source_before = hp(&mut app, source).unwrap();
                let target_before = prop_hp(&app, target).unwrap();
                probe(&mut app, source, target);
                let aim = if kill_source {
                    CellPos::new(16, 20)
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
                let events = tick(
                    &mut app,
                    if kill {
                        vec![strike(launcher, aim)]
                    } else {
                        vec![]
                    },
                );
                let c = app.world().resource::<Probe>().commands.clone().unwrap();
                if kill {
                    ok(&events);
                    strike_receipt(&app, &events, launcher, aim);
                    if kill_source {
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .erasures
                                .contains_key(&target)
                        );
                        assert!(c.source_hp.is_some_and(|h| h <= 0));
                        assert_eq!(c.target_hp, Some(target_before));
                        assert!(unit_destroyed(&events, source));
                        assert!(!prop_destroyed(&events, target));
                        assert_eq!(prop_hp(&app, target), Some(target_before));
                        assert!(hp(&mut app, source).is_none());
                        run(&mut app, 40);
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .erasures
                                .contains_key(&target)
                        );
                        assert_eq!(prop_hp(&app, target), Some(target_before));
                    } else {
                        // Temporal freeze intentionally makes its target invulnerable to ordinary
                        // damage. The actual native channel must then finish erasure and Cleanup.
                        assert_eq!(c.target_hp, Some(target_before));
                        assert_eq!(c.source_hp, Some(source_before));
                        assert!(!prop_destroyed(&events, target));
                        assert!(!unit_destroyed(&events, source));
                        assert!(
                            app.world()
                                .resource::<SpecialState>()
                                .erasures
                                .contains_key(&target)
                        );
                        let mut erased = false;
                        for _ in 0..2000 {
                            let later = tick(&mut app, vec![]);
                            if prop_hp(&app, target).is_none() {
                                assert!(prop_destroyed(&later, target));
                                erased = true;
                                break;
                            }
                            assert_eq!(prop_hp(&app, target), Some(target_before));
                        }
                        assert!(
                            erased,
                            "actual sustained native erasure, not ordinary damage"
                        );
                        assert!(
                            !app.world()
                                .resource::<SpecialState>()
                                .erasures
                                .contains_key(&target)
                        );
                        assert_eq!(hp(&mut app, source), Some(source_before));
                        let later = run(&mut app, 40);
                        assert!(prop_hp(&app, target).is_none());
                        assert!(!prop_destroyed(&later, target));
                    }
                } else {
                    ok(&events);
                    assert_eq!(c.source_hp, Some(source_before));
                    assert_eq!(c.target_hp, Some(target_before));
                    assert!(
                        app.world()
                            .resource::<SpecialState>()
                            .erasures
                            .contains_key(&target)
                    );
                    assert_eq!(hp(&mut app, source), Some(source_before));
                    assert_eq!(prop_hp(&app, target), Some(target_before));
                }
                cells(&mut app);
            }
        }
    }
}
fn ivan_world(
    feature: u8,
    name: &str,
    engineer_x: Option<i32>,
) -> (bevy_app::App, SimId, SimId, Option<SimId>) {
    let mut app = world();
    let target = mapped(&mut app, feature, name);
    let ivan = put(&mut app, P0, "crazy_ivan", 19, 20);
    let scout = put(&mut app, P0, "engineer", 18, 18);
    let defuser = put(&mut app, P0, "engineer", 19, 21);
    let engineer = engineer_x.map(|x| put(&mut app, P0, "engineer", x, 20));
    let mut held = vec![ivan, scout, defuser];
    held.extend(engineer);
    ok(&tick(&mut app, vec![guard(&held)]));
    let before = prop_hp(&app, target).unwrap();
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![ivan],
                target,
            },
        )],
    );
    ok(&events);
    assert!(
        fired(&events, ivan, target),
        "real IvanBomber WeaponFired required"
    );
    let charge = app
        .world()
        .resource::<SpecialState>()
        .charges
        .get(&target)
        .expect("real charge");
    assert!(!charge.c4);
    assert_eq!(charge.source, ivan);
    assert_eq!(charge.damage, 400);
    assert_eq!(charge.due - app.world().resource::<SimTick>().0, 599);
    assert_eq!(
        prop_hp(&app, target),
        Some(before),
        "plant is not physical HP damage"
    );
    ok(&tick(&mut app, vec![guard(&[ivan])]));
    (app, target, ivan, engineer)
}
#[test]
fn ra9_real_ivan_expiry_deals_four_hundred_base_damage_with_independent_armour_controls() {
    for ((feature, name), percent) in KINDS.into_iter().zip([100, 20, 250]) {
        let (mut app, target, ivan, _) = ivan_world(feature, name, None);
        let before = prop_hp(&app, target).unwrap();
        let due = app.world().resource::<SpecialState>().charges[&target].due;
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![ivan],
                    target: CellPos::new(10, 20),
                },
            )],
        ));
        let mut impact = false;
        let mut destroyed = false;
        for _ in 0..610 {
            let at = app.world().resource::<SimTick>().0;
            let events = tick(&mut app, vec![]);
            destroyed |= prop_destroyed(&events, target);
            if at < due {
                assert_eq!(prop_hp(&app, target), Some(before));
            }
            if at == due {
                let warhead = defs(&app).warhead_kind("ivan_wh").expect("IvanWH");
                impact = events
                    .iter()
                    .any(|e| matches!(e,SimEvent::Impact {warhead:h,..} if *h==warhead));
                let expected = before - 400 * percent / 100;
                if expected <= 0 {
                    assert!(prop_hp(&app, target).is_none());
                    assert!(destroyed);
                } else {
                    assert_eq!(prop_hp(&app, target), Some(expected));
                    assert!(!destroyed);
                }
                break;
            }
        }
        assert!(impact, "actual native expiry Impact at due tick");
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .charges
                .contains_key(&target)
        );
        assert!(
            hp(&mut app, ivan).is_some_and(|h| h > 0),
            "real retreat survives outside blast"
        );
        cells(&mut app);
    }
}
#[test]
fn ra9_real_explicit_defusekit_traverses_long_native_path_and_prevents_the_later_ivan_explosion() {
    for (feature, name) in KINDS {
        let (mut app, target, _ivan, engineer) = ivan_world(feature, name, Some(12));
        let engineer = engineer.unwrap();
        let before = prop_hp(&app, target).unwrap();
        let due = app.world().resource::<SpecialState>().charges[&target].due;
        assert_eq!(cell(&mut app, engineer), CellPos::new(12, 20));
        ok(&tick(&mut app, vec![interact(engineer, target)]));
        let mut seen = BTreeSet::new();
        let mut defused = false;
        for _ in 0..500 {
            seen.insert(cell(&mut app, engineer));
            if !app
                .world()
                .resource::<SpecialState>()
                .charges
                .contains_key(&target)
            {
                defused = true;
                break;
            }
            tick(&mut app, vec![]);
        }
        assert!(defused, "actual long approach must beat native fuse");
        assert!(seen.len() >= 3, "not an adjacent/teleported defuse setup");
        assert!(app.world().resource::<SimTick>().0 <= due);
        assert!(hp(&mut app, engineer).is_some_and(|h| h > 0));
        assert_eq!(prop_hp(&app, target), Some(before));
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .interactions
                .contains_key(&engineer)
        );
        let events = run(&mut app, 650);
        assert!(!prop_destroyed(&events, target));
        assert_eq!(prop_hp(&app, target), Some(before));
        cells(&mut app);
    }
}
#[test]
fn ra9_real_bombsight_approaches_from_detection_range_and_never_falls_back_after_another_real_defuse()
 {
    for (feature, name) in KINDS {
        for preempt in [false, true] {
            let (mut app, target, _ivan, engineer) = ivan_world(feature, name, Some(15));
            let engineer = engineer.unwrap();
            let before = prop_hp(&app, target).unwrap();
            // The separate adjacent Engineer exists before tick0 in this scenario variant.
            // In the preempt case that Engineer is held on Guard, then receives an actual
            // immediate explicit DEFUSE after the automatic job exists.
            let scout = {
                let world = app.world_mut();
                world
                    .query::<(&SimId, &sim::Unit, &sim::Mobile)>()
                    .iter(world)
                    .find(|(id, _, m)| **id != engineer && m.cell == CellPos::new(19, 21))
                    .map(|(id, _, _)| *id)
                    .expect("real adjacent defuser")
            };
            ok(&tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Stop {
                        units: vec![engineer],
                    },
                )],
            ));
            let d = defs(&app);
            let m = d
                .metadata(sim::ProductionItem::Unit(d.unit_kind("engineer").unwrap()))
                .unwrap();
            assert_eq!(m.bomb_sight, 4);
            let kit = d.weapon(d.weapon_kind("defuse_kit").unwrap()).unwrap();
            assert_eq!(kit.range, 384);
            assert!(d.warhead(kit.warhead).unwrap().bomb_disarm);
            let events = run(&mut app, 80);
            assert!(!prop_destroyed(&events, target));
            assert_eq!(cell(&mut app, engineer), CellPos::new(15, 20));
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .auto_defuses
                    .contains_key(&engineer)
            );
            assert!(
                app.world()
                    .resource::<SpecialState>()
                    .charges
                    .contains_key(&target),
                "genuinely idle outside4 does not detect"
            );
            ok(&tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Move {
                        units: vec![engineer],
                        target: CellPos::new(16, 20),
                    },
                )],
            ));
            let mut saw_job = false;
            let mut reached_detection = false;
            let mut seen = BTreeSet::new();
            for _ in 0..250 {
                let at = cell(&mut app, engineer);
                seen.insert(at);
                reached_detection |= at == CellPos::new(16, 20);
                if app
                    .world()
                    .resource::<SpecialState>()
                    .auto_defuses
                    .get(&engineer)
                    == Some(&(target, P0))
                {
                    saw_job = true;
                    break;
                }
                assert!(
                    app.world()
                        .resource::<SpecialState>()
                        .charges
                        .contains_key(&target)
                );
                tick(&mut app, vec![]);
            }
            assert!(
                reached_detection && saw_job,
                "native idle detection must create typed DEFUSE job outside weapon range"
            );
            let job_origin = cell(&mut app, engineer);
            if preempt {
                // Real adjacent command before the automatic approach can reach range.
                probe(&mut app, engineer, target);
                ok(&tick(&mut app, vec![interact(scout, target)]));
                assert!(
                    !app.world()
                        .resource::<Probe>()
                        .commands
                        .as_ref()
                        .unwrap()
                        .charged,
                    "another real DefuseKit removed it in Commands, before auto-defuse Economy"
                );
                assert!(hp(&mut app, scout).is_some_and(|h| h > 0));
            }
            let mut defused = false;
            for _ in 0..300 {
                seen.insert(cell(&mut app, engineer));
                if !app
                    .world()
                    .resource::<SpecialState>()
                    .charges
                    .contains_key(&target)
                {
                    defused = true;
                    break;
                }
                tick(&mut app, vec![]);
            }
            assert!(defused);
            assert!(seen.len() >= 2);
            if !preempt {
                assert!(
                    cell(&mut app, engineer).x > job_origin.x,
                    "automatic job made real physical approach"
                );
            }
            run(&mut app, 4);
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .auto_defuses
                    .contains_key(&engineer)
            );
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .interactions
                    .contains_key(&engineer)
            );
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .garrisons
                    .contains_key(&target),
                "no generic capture fallback"
            );
            assert_eq!(prop_hp(&app, target), Some(before));
            assert!(hp(&mut app, engineer).is_some_and(|h| h > 0));
            let events = run(&mut app, 650);
            assert!(!prop_destroyed(&events, target));
            assert_eq!(prop_hp(&app, target), Some(before));
            cells(&mut app);
        }
    }
}

#[test]
fn ra9_real_ivan_charge_survives_native_commands_source_death_and_later_deals_the_same_damage() {
    for ((feature, name), percent) in KINDS.into_iter().zip([100, 20, 250]) {
        for kill in [false, true] {
            let mut app = world_of(immediate_definitions());
            let target = mapped(&mut app, feature, name);
            let ivan = put(&mut app, P0, "crazy_ivan", 19, 20);
            let scout = put(&mut app, P0, "engineer", 18, 18);
            let launcher = silo(&mut app);
            ok(&tick(&mut app, vec![guard(&[ivan, scout])]));
            warm(&mut app, launcher);
            let before = prop_hp(&app, target).unwrap();
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Attack {
                        units: vec![ivan],
                        target,
                    },
                )],
            );
            ok(&events);
            assert!(fired(&events, ivan, target));
            let charge = app
                .world()
                .resource::<SpecialState>()
                .charges
                .get(&target)
                .expect("real plant")
                .clone();
            assert!(!charge.c4);
            assert_eq!(charge.damage, 400);
            assert_eq!(charge.source, ivan);
            assert_eq!(prop_hp(&app, target), Some(before));
            // This is actual native standing-point geometry, never a position write:
            // Ivan is first infantry in19,20 at(19.25,20.25). Aim18,20 is <1cell from him,
            // but2cells from the host centre. Thus the declared radius1 fixture kills ONLY Ivan.
            assert_eq!(
                app.world()
                    .resource::<Grid>()
                    .spot(CellPos::new(19, 20), ivan),
                Some(0)
            );
            let en = entity(&mut app, ivan).unwrap();
            assert_eq!(
                app.world().get::<sim::Position>(en).unwrap().0,
                sim::SubPos::spot_of(CellPos::new(19, 20), 0)
            );
            probe(&mut app, ivan, target);
            let aim = CellPos::new(18, 20);
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
                    vec![guard(&[ivan])]
                },
            );
            ok(&events);
            let c = app.world().resource::<Probe>().commands.clone().unwrap();
            assert!(
                c.charged,
                "native planted charge remains after Commands regardless of source death"
            );
            assert_eq!(c.target_hp, Some(before));
            if kill {
                strike_receipt(&app, &events, launcher, aim);
                assert!(c.source_hp.is_some_and(|h| h <= 0));
                assert!(unit_destroyed(&events, ivan));
                assert!(hp(&mut app, ivan).is_none());
            } else {
                assert!(c.source_hp.is_some_and(|h| h > 0));
                assert!(!unit_destroyed(&events, ivan));
                ok(&tick(
                    &mut app,
                    vec![issue(
                        P0,
                        Command::Move {
                            units: vec![ivan],
                            target: CellPos::new(10, 20),
                        },
                    )],
                ));
            }
            assert_eq!(
                app.world().resource::<SpecialState>().charges[&target].due,
                charge.due
            );
            let mut impact = false;
            for _ in 0..610 {
                let at = app.world().resource::<SimTick>().0;
                let events = tick(&mut app, vec![]);
                if at < charge.due {
                    assert_eq!(prop_hp(&app, target), Some(before));
                }
                if at == charge.due {
                    let wh = defs(&app).warhead_kind("ivan_wh").unwrap();
                    impact = events
                        .iter()
                        .any(|e| matches!(e,SimEvent::Impact {warhead,..} if *warhead==wh));
                    let expected = before - 400 * percent / 100;
                    if expected <= 0 {
                        assert!(prop_destroyed(&events, target));
                        assert!(prop_hp(&app, target).is_none());
                    } else {
                        assert_eq!(prop_hp(&app, target), Some(expected));
                        assert!(!prop_destroyed(&events, target));
                    }
                    break;
                }
            }
            assert!(impact);
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .charges
                    .contains_key(&target)
            );
            if !kill {
                assert!(hp(&mut app, ivan).is_some_and(|h| h > 0));
            }
            cells(&mut app);
        }
    }
}
