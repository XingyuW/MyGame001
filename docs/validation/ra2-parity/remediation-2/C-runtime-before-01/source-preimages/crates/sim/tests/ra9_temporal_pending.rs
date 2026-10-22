//! Pending source actions pause under genuine Temporal channels and resume after release.
mod ra9_remaining_common;
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{
    Building, CellPos, Command, Footprint, Grid, Mobile, SimEvent, SimId, SpecialState, Weapon,
};

fn ring(origin: CellPos) -> impl Iterator<Item = CellPos> {
    Footprint {
        origin,
        width: 1,
        height: 1,
    }
    .ring()
}
fn block_exits(app: &mut bevy_app::App, origin: CellPos) -> Vec<SimId> {
    let free: Vec<_> = ring(origin)
        .filter(|&c| app.world().resource::<Grid>().has_room(c, false))
        .collect();
    assert!(!free.is_empty());
    free.into_iter()
        .map(|c| {
            let id = put(app, P0, "grizzly", c.x, c.y);
            disarm(app, id);
            id
        })
        .collect()
}
fn open_exit(app: &mut bevy_app::App, blocker: SimId, origin: CellPos) {
    let events = tick(
        app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![blocker],
                target: CellPos::new(origin.x - 5, origin.y - 5),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..300 {
        if ring(origin).any(|c| app.world().resource::<Grid>().has_room(c, true)) {
            return;
        }
        tick(app, vec![]);
    }
    panic!("public blocker Move did not free an exit");
}
fn patient(app: &bevy_app::App, hospital: SimId, unit: SimId) -> i32 {
    let cargo = &app.world().resource::<SpecialState>().cargo[&hospital];
    assert_eq!(cargo.len(), 1);
    assert_eq!(cargo[0].id, unit);
    cargo[0].hp
}
fn captured_hospital() -> (bevy_app::App, SimId) {
    let mut app = world();
    let hospital = put(&mut app, sim::NEUTRAL, "hospital", 20, 20);
    let engineer = put(&mut app, P0, "engineer", 19, 20);
    tick(&mut app, vec![]);
    let events = interact(&mut app, engineer, hospital);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(owner(&mut app, hospital), Some(P0));
    assert!(entity(&mut app, engineer).is_none());
    (app, hospital)
}
fn hospital_case(blocked: bool) {
    let (mut app, hospital) = captured_hospital();
    let caster = put(&mut app, P1, "chrono_legionnaire", 17, 20);
    start_beam(&mut app, caster, hospital);
    let infantry = put(&mut app, P0, "gi", 19, 20);
    disarm(&mut app, infantry);
    let initial = if blocked { 105 } else { 5 };
    set_hp(&mut app, infantry, initial);
    let events = interact(&mut app, infantry, hospital);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, infantry).is_none(),
        "public admission retains original identity as cargo"
    );
    assert_eq!(
        patient(&app, hospital, infantry),
        initial,
        "frozen Hospital must not heal at admission tick's due service"
    );
    let blockers = if blocked {
        block_exits(&mut app, CellPos::new(20, 20))
    } else {
        vec![]
    };
    for _ in 0..8 {
        tick(&mut app, vec![]);
        frozen(&mut app, hospital);
        assert_eq!(patient(&app, hospital, infantry), initial);
        assert!(
            entity(&mut app, infantry).is_none(),
            "frozen service cannot create an exit birth"
        );
    }
    release_beam(&mut app, caster, hospital);
    if blocked {
        tick(&mut app, vec![]);
        assert_eq!(
            patient(&app, hospital, infantry),
            125,
            "released host heals even while lawful exit is blocked"
        );
        assert!(entity(&mut app, infantry).is_none());
        open_exit(&mut app, blockers[0], CellPos::new(20, 20));
    }
    for _ in 0..12 {
        if entity(&mut app, infantry).is_some() {
            break;
        }
        tick(&mut app, vec![]);
    }
    assert_eq!(hp(&mut app, infantry), 125);
    assert_eq!(owner(&mut app, infantry), Some(P0));
    assert!(cargo(&app, hospital).is_empty());
    cells(&mut app);
}
#[test]
fn ra9_erased_captured_hospital_holds_due_healing_and_exit_then_resumes() {
    hospital_case(false);
}
#[test]
fn ra9_erased_captured_hospital_preserves_near_full_patient_and_blocked_release() {
    hospital_case(true);
}

/// Wait immediately before the genuine last movement step into interaction adjacency.
/// No Mobile, Orders, erasure, event, or command-stage state is forged.
fn freeze_last_step(
    app: &mut bevy_app::App,
    source: SimId,
    target: SimId,
    caster: SimId,
    native: Weapon,
) {
    for _ in 0..600 {
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .interactions
                .get(&source),
            Some(&target)
        );
        let e = entity(app, source).expect("pending source still on map");
        let m = *app.world().get::<Mobile>(e).unwrap();
        let foot = if let Some(e) = entity(app, target) {
            app.world().get::<Building>(e).map_or_else(
                || Footprint {
                    origin: app.world().get::<Mobile>(e).unwrap().cell,
                    width: 1,
                    height: 1,
                },
                |b| b.footprint,
            )
        } else {
            Footprint {
                origin: app.world().resource::<sim::Props>().get(target).unwrap().0,
                width: 1,
                height: 1,
            }
        };
        if m.step
            .is_some_and(|s| s.length - s.progress <= m.speed && foot.distance_to_cell(s.to) == 1)
        {
            let ce = entity(app, caster).unwrap();
            app.world_mut().entity_mut(ce).insert(native);
            let events = tick(
                app,
                vec![issue(
                    P1,
                    Command::Attack {
                        units: vec![caster],
                        target: source,
                    },
                )],
            );
            assert!(rejected(&events).is_empty(), "{events:?}");
            assert!(events.iter().any(|e| matches!(e, SimEvent::WeaponFired{attacker,target,..} if *attacker == caster && *target == source)), "real first Temporal shot at final movement boundary: {events:?}");
            frozen(app, source);
            assert_eq!(
                app.world()
                    .resource::<SpecialState>()
                    .interactions
                    .get(&source),
                Some(&target)
            );
            return;
        }
        tick(app, vec![]);
    }
    panic!("native pending approach never reached last-step boundary");
}
fn passive_caster(app: &mut bevy_app::App, x: i32, y: i32) -> (SimId, Weapon) {
    let caster = put(app, P1, "chrono_legionnaire", x, y);
    let e = entity(app, caster).unwrap();
    let weapon = *app.world().get::<Weapon>(e).unwrap();
    disarm(app, caster);
    (caster, weapon)
}
fn pending_actor_case(board: bool) {
    let mut app = world();
    let target = if board {
        put(&mut app, P0, "ifv", 20, 20)
    } else {
        put(&mut app, sim::NEUTRAL, "hospital", 20, 20)
    };
    if board {
        disarm(&mut app, target);
    }
    let source = put(&mut app, P0, "engineer", 16, 20);
    let (caster, native) = passive_caster(&mut app, 18, 17);
    tick(&mut app, vec![]);
    let events = interact(&mut app, source, target);
    assert!(rejected(&events).is_empty(), "{events:?}");
    freeze_last_step(&mut app, source, target, caster, native);
    let before_owner = owner(&mut app, target);
    for _ in 0..12 {
        tick(&mut app, vec![]);
        frozen(&mut app, source);
        assert_eq!(owner(&mut app, target), before_owner);
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .interactions
                .get(&source),
            Some(&target)
        );
        assert!(cargo(&app, target).is_empty());
    }
    release_beam(&mut app, caster, source);
    tick(&mut app, vec![]);
    assert!(
        entity(&mut app, source).is_none(),
        "released pending actor must execute original accepted action"
    );
    if board {
        assert_eq!(cargo(&app, target), vec![source]);
    } else {
        assert_eq!(owner(&mut app, target), Some(P0));
    }
    cells(&mut app);
}
#[test]
fn ra9_pending_engineer_capture_waits_for_real_temporal_release() {
    pending_actor_case(false);
}
#[test]
fn ra9_pending_engineer_boarding_waits_for_real_temporal_release() {
    pending_actor_case(true);
}
fn pending_prop_case(ivan: bool) {
    let mut app = world();
    let target = ra9_remaining_common::mapped(&mut app, 5, "civilian_house");
    let source = put(&mut app, P0, if ivan { "crazy_ivan" } else { "gi" }, 16, 20);
    disarm(&mut app, source);
    let (caster, native) = passive_caster(&mut app, 18, 17);
    tick(&mut app, vec![]);
    let events = interact(&mut app, source, target);
    assert!(rejected(&events).is_empty(), "{events:?}");
    freeze_last_step(&mut app, source, target, caster, native);
    for _ in 0..12 {
        tick(&mut app, vec![]);
        frozen(&mut app, source);
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .charges
                .contains_key(&target)
        );
        assert!(cargo(&app, target).is_empty());
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .interactions
                .get(&source),
            Some(&target)
        );
    }
    release_beam(&mut app, caster, source);
    tick(&mut app, vec![]);
    if ivan {
        assert!(
            app.world()
                .resource::<SpecialState>()
                .charges
                .contains_key(&target),
            "released real pending mapped-Prop Ivan action"
        );
    } else {
        assert!(entity(&mut app, source).is_none());
        assert_eq!(cargo(&app, target), vec![source]);
        assert_eq!(
            app.world().resource::<SpecialState>().garrisons[&target].owner,
            P0
        );
    }
    cells(&mut app);
}
#[test]
fn ra9_pending_occupier_entry_waits_for_real_temporal_release() {
    pending_prop_case(false);
}
#[test]
fn ra9_pending_prop_ivan_action_waits_for_real_temporal_release() {
    pending_prop_case(true);
}

#[test]
fn ra9_blocked_garrison_eviction_cannot_birth_occupant_while_host_erased() {
    let mut app = world();
    let host = ra9_remaining_common::mapped(&mut app, 5, "civilian_house");
    let occupant = put(&mut app, P0, "gi", 19, 20);
    disarm(&mut app, occupant);
    let (caster, native) = passive_caster(&mut app, 17, 20);
    tick(&mut app, vec![]);
    let events = interact(&mut app, occupant, host);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(cargo(&app, host), vec![occupant]);
    let blockers = block_exits(&mut app, CellPos::new(20, 20));
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: host })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(app.world().resource::<SpecialState>().garrisons[&host].evicting);
    assert!(entity(&mut app, occupant).is_none());
    let ce = entity(&mut app, caster).unwrap();
    app.world_mut().entity_mut(ce).insert(native);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![caster],
                target: host,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        app.world().resource::<SpecialState>().erasures[&host]
            .beams
            .contains_key(&caster)
    );
    open_exit(&mut app, blockers[0], CellPos::new(20, 20));
    for _ in 0..12 {
        tick(&mut app, vec![]);
        assert!(
            app.world().resource::<SpecialState>().erasures[&host]
                .beams
                .contains_key(&caster)
        );
        assert!(
            app.world()
                .resource::<sim::Props>()
                .get(host)
                .unwrap()
                .1
                .hit_points
                > 0
        );
        assert_eq!(
            cargo(&app, host),
            vec![occupant],
            "a persisted voluntary exit is frozen host work"
        );
        assert!(entity(&mut app, occupant).is_none());
        assert!(app.world().resource::<SpecialState>().garrisons[&host].evicting);
        cells(&mut app);
    }
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![caster],
                target: CellPos::new(3, 3),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .erasures
            .contains_key(&host)
    );
    assert!(entity(&mut app, occupant).is_some());
    assert_eq!(owner(&mut app, occupant), Some(P0));
    assert_eq!(hp(&mut app, occupant), 125);
    assert!(cargo(&app, host).is_empty());
    cells(&mut app);
}
