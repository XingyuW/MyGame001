//! UNCOMPILED / UNRUN: proposed native actorless-target tests after schema10 integration.
//! Runtime stimuli use CommandQueue/SimSchedule. Hidden injection is controlled admission setup,
//! not an original/native concealment producer. No garrison cooldown or beam history is forged.
mod ra9_soundness_common;
use bevy_ecs::{resource::Resource, schedule::IntoScheduleConfigs};
use ra9_soundness_common::*;
use sim::{
    Cell, CellPos, Command, Grid, Props, RejectReason, SimCoreSet, SimEvent, SimId, SimSchedule,
    SimSet, SpecialState,
};

fn mapped(app: &mut bevy_app::App, feature: u8, name: &str) -> SimId {
    let cell = CellPos::new(20, 20);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        cell,
        Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        },
    );
    let mut looks = vec![0; 48 * 48];
    looks[20 * 48 + 20] = feature << 4;
    let d = defs(app);
    let armour = d
        .building_combat(d.building_kind(name).expect("mapped kind"))
        .expect("terms")
        .armour;
    let props = Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
    let id = props.id_at(cell).expect("actual Prop");
    app.world_mut().insert_resource(props);
    id
}
fn prop_hp(app: &bevy_app::App, id: SimId) -> Option<i32> {
    app.world()
        .resource::<Props>()
        .get(id)
        .map(|(_, p)| p.hit_points)
}
fn shots(events: &[SimEvent], id: SimId) -> bool {
    events
        .iter()
        .any(|e| matches!(e,SimEvent::WeaponFired {attacker,..} if *attacker==id))
}
fn ok(events: &[SimEvent]) {
    assert!(rejected(events).is_empty(), "{events:?}");
}
fn reject(events: &[SimEvent], reason: RejectReason) {
    let reasons = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::CommandRejected { reason, .. } => Some(reason),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reasons, vec![&reason], "{events:?}");
}
#[test]
fn ra9_prop_temporal_freezes_real_host_fire_and_deploy_then_releases_after_real_beam_break() {
    for (feature, name) in [
        (5, "civilian_house"),
        (6, "civilian_city"),
        (7, "civilian_tower"),
    ] {
        let mut app = world();
        let host = mapped(&mut app, feature, name);
        let occupant = put(&mut app, P1, "conscript", 19, 20);
        let chrono = put(&mut app, P0, "chrono_legionnaire", 13, 20);
        let bait = put(&mut app, P0, "grizzly", 19, 21);
        let e = entity(&mut app, bait).expect("bait");
        app.world_mut().entity_mut(e).remove::<sim::Weapon>(); // passive pre-tick fixture
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Guard {
                    units: vec![chrono],
                },
            )],
        ));
        let mut events = tick(
            &mut app,
            vec![issue(
                P1,
                Command::Interact {
                    unit: occupant,
                    target: host,
                },
            )],
        );
        ok(&events);
        assert_eq!(cargo(&app, host), vec![occupant]);
        assert!(entity(&mut app, occupant).is_none());
        assert!(entity(&mut app, host).is_none(), "no fabricated host Actor");
        let mut fired_before = shots(&events, host);
        for _ in 0..100 {
            if fired_before {
                break;
            }
            events = tick(&mut app, vec![]);
            fired_before |= shots(&events, host);
        }
        assert!(
            fired_before,
            "actual occupied-host WeaponFired is a required premise"
        );
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![chrono],
                    target: CellPos::new(16, 20),
                },
            )],
        ));
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Guard {
                    units: vec![chrono],
                },
            )],
        ));
        steps(&mut app, 100); // natural disabled interval expires under Guard before explicit launch
        assert_eq!(cell(&mut app, chrono), CellPos::new(16, 20));
        let before = prop_hp(&app, host).expect("host alive");
        events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Attack {
                    units: vec![chrono],
                    target: host,
                },
            )],
        );
        ok(&events);
        let mut source_fired = events.iter().any(|e| {
            matches!(e,SimEvent::WeaponFired {attacker,target,..}
            if *attacker==chrono && *target==host)
        });
        for _ in 0..100 {
            if app
                .world()
                .resource::<SpecialState>()
                .erasures
                .contains_key(&host)
            {
                break;
            }
            events = tick(&mut app, vec![]);
            source_fired |= events.iter().any(|e| {
                matches!(e,SimEvent::WeaponFired {attacker,target,..}
                if *attacker==chrono && *target==host)
            });
        }
        assert!(source_fired);
        let initial = app
            .world()
            .resource::<SpecialState>()
            .erasures
            .get(&host)
            .expect("real beam")
            .remaining;
        let events = tick(&mut app, vec![issue(P1, Command::Deploy { unit: host })]);
        reject(
            &events,
            RejectReason::AbilityUnavailable("Target is being erased"),
        );
        assert_eq!(cargo(&app, host), vec![occupant]);
        assert_eq!(
            app.world().resource::<SpecialState>().garrisons[&host].owner,
            P1
        );
        assert!(!shots(&events, host));
        for _ in 0..120 {
            let events = tick(&mut app, vec![]);
            assert!(
                !shots(&events, host),
                "must outlast actual host reload, not just test a cooldown gap"
            );
        }
        let after = app
            .world()
            .resource::<SpecialState>()
            .erasures
            .get(&host)
            .expect("sustained across reload")
            .remaining;
        assert_eq!(
            initial - after,
            121 * 6,
            "Damage8 retained beam decrements6 each native tick"
        );
        assert_eq!(prop_hp(&app, host), Some(before));
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Guard {
                    units: vec![chrono],
                },
            )],
        ));
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .erasures
                .contains_key(&host),
            "real command breaks beam"
        );
        let mut resumed = false;
        for _ in 0..100 {
            let e = tick(&mut app, vec![]);
            resumed |= shots(&e, host);
            if resumed {
                break;
            }
        }
        assert!(resumed, "real live host fires after release");
        ok(&tick(
            &mut app,
            vec![issue(P1, Command::Deploy { unit: host })],
        ));
        assert!(
            entity(&mut app, occupant).is_some(),
            "same passenger returns through native unload"
        );
        assert!(cargo(&app, host).is_empty());
        assert_eq!(prop_hp(&app, host), Some(before));
        cells(&mut app);
    }
}
#[test]
fn ra9_prop_real_c4_plant_then_native_death_and_dead_target_rejection() {
    for (feature, name) in [
        (5, "civilian_house"),
        (6, "civilian_city"),
        (7, "civilian_tower"),
    ] {
        let mut app = world();
        let host = mapped(&mut app, feature, name);
        let occupant = put(&mut app, P0, "conscript", 19, 20);
        let tanya = put(&mut app, P1, "tanya", 19, 21);
        let engineer = put(&mut app, P1, "engineer", 18, 21);
        let e = entity(&mut app, tanya).expect("planter");
        app.world_mut().entity_mut(e).remove::<sim::Weapon>(); // pre-tick passive C4 fixture; metadata ability unchanged
        // Guard avoids an automatic pre-entry pistol kill; no runtime cooldown is edited.
        ok(&tick(
            &mut app,
            vec![issue(P1, Command::Guard { units: vec![tanya] })],
        ));
        ok(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: occupant,
                    target: host,
                },
            )],
        ));
        assert!(entity(&mut app, occupant).is_none());
        ok(&tick(
            &mut app,
            vec![issue(
                P1,
                Command::Interact {
                    unit: tanya,
                    target: host,
                },
            )],
        ));
        let c = app
            .world()
            .resource::<SpecialState>()
            .charges
            .get(&host)
            .expect("real C4")
            .clone();
        assert!(c.c4);
        assert_eq!(c.source, tanya);
        let mut destroyed = false;
        for _ in 0..80 {
            let e = tick(&mut app, vec![]);
            destroyed |= e
                .iter()
                .any(|e| matches!(e,SimEvent::PropDestroyed {prop,..} if *prop==host));
            if prop_hp(&app, host).is_none() {
                break;
            }
        }
        assert!(destroyed);
        assert!(prop_hp(&app, host).is_none());
        assert!(
            entity(&mut app, engineer).is_some(),
            "independent surviving issuer"
        );
        reject(
            &tick(
                &mut app,
                vec![issue(
                    P1,
                    Command::Interact {
                        unit: engineer,
                        target: host,
                    },
                )],
            ),
            RejectReason::UnknownTarget(host),
        );
        cells(&mut app);
    }
}
#[derive(Resource)]
struct AdmissionSnapshot {
    host: SimId,
    unit: SimId,
    hidden: bool,
    guard: bool,
    pending: bool,
}
fn after_commands(world: &mut bevy_ecs::world::World) {
    let (host, unit) = {
        let s = world.resource::<AdmissionSnapshot>();
        (s.host, s.unit)
    };
    let hidden = world
        .resource::<SpecialState>()
        .hidden
        .contains(&(P0, host));
    let guard = matches!(
        world.resource::<sim::TacticalState>().orders.get(&unit),
        Some(sim::TacticalOrder::Guard { .. })
    );
    let pending = world
        .resource::<SpecialState>()
        .interactions
        .contains_key(&unit);
    let mut s = world.resource_mut::<AdmissionSnapshot>();
    s.hidden = hidden;
    s.guard = guard;
    s.pending = pending;
}
#[test]
fn ra9_prop_command_unknown_target_matrix_preserves_order_before_clear() {
    // All three actual mapped Prop kinds. Unknown terrain is genuine native vision;
    // hidden is deliberately controlled setup because no native producer supplies this case.
    for (feature, name) in [
        (5, "civilian_house"),
        (6, "civilian_city"),
        (7, "civilian_tower"),
    ] {
        for case in ["hidden", "unexplored", "missing", "known"] {
            let mut app = world();
            let host = mapped(&mut app, feature, name);
            let x = if case == "unexplored" { 2 } else { 18 };
            let unit = put(&mut app, P0, "tanya", x, 20);
            ok(&tick(
                &mut app,
                vec![issue(P0, Command::Guard { units: vec![unit] })],
            ));
            assert_eq!(
                app.world()
                    .resource::<sim::Players>()
                    .get(P0)
                    .unwrap()
                    .vision()
                    .is_visible(CellPos::new(20, 20)),
                case != "unexplored"
            );
            if case == "hidden" {
                app.world_mut()
                    .resource_mut::<SpecialState>()
                    .hidden
                    .insert((P0, host));
            }
            app.world_mut().insert_resource(AdmissionSnapshot {
                host,
                unit,
                hidden: false,
                guard: false,
                pending: false,
            });
            app.add_systems(
                SimSchedule,
                after_commands
                    .in_set(SimSet::Commands)
                    .after(SimCoreSet(SimSet::Commands)),
            );
            let target = if case == "missing" {
                SimId(u32::MAX)
            } else {
                host
            };
            let events = tick(
                &mut app,
                vec![issue(P0, Command::Interact { unit, target })],
            );
            let s = app.world().resource::<AdmissionSnapshot>();
            if case == "known" {
                ok(&events);
                assert!(s.pending);
                assert!(!s.guard);
            } else {
                reject(&events, RejectReason::UnknownTarget(target));
                assert!(s.guard, "rejection precedes clear_orders");
                assert!(!s.pending);
                assert!(
                    !app.world()
                        .resource::<SpecialState>()
                        .charges
                        .contains_key(&host)
                );
            }
            if case == "hidden" {
                assert!(
                    s.hidden,
                    "Commands early concealment refresh must preserve controlled Prop filter"
                );
                assert!(
                    !app.world()
                        .resource::<SpecialState>()
                        .hidden
                        .contains(&(P0, host)),
                    "regular Visibility recomputes it; no fabricated permanent concealment"
                );
            }
            cells(&mut app);
        }
    }
}
// Remaining NOT implemented here: source death/target death between commands in one batch,
// and pending order invalidated by a genuine native mid-tick C4/source ownership transition.
// See acceptance.md for the exact stage-bound fixtures and observer requirements.
