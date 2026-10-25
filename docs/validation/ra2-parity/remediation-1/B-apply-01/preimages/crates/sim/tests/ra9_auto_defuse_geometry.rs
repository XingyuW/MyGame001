//! UNCOMPILED / UNRUN. Run this SAME test under base370471e before the small geometry delta.
//! The west case is expected to fail actual approach/removal while all native premises pass.
//! Requires the unchanged common module from the remaining-native package; no parser variant.
mod ra9_remaining_common;
use bevy_ecs::{resource::Resource, schedule::IntoScheduleConfigs, world::World};
use ra9_remaining_common::*;
fn house_gap(point: sim::SubPos) -> (i64, i64) {
    // Independent explicit 1x1 House bounds at20,20, including its last physical subcell.
    let lo = 20_i64 * 256;
    let hi = 21_i64 * 256 - 1;
    (
        (lo - i64::from(point.x))
            .max(i64::from(point.x) - hi)
            .max(0),
        (lo - i64::from(point.y))
            .max(i64::from(point.y) - hi)
            .max(0),
    )
}
fn kit_reaches(point: sim::SubPos) -> bool {
    let (dx, dy) = house_gap(point);
    dx * dx + dy * dy <= 384_i64 * 384
}
#[derive(Resource, Clone, Debug)]
struct Trace {
    unit: SimId,
    target: SimId,
    pre: Option<sim::SubPos>,
    had_charge: bool,
    first_destination: Option<CellPos>,
    saw_job: bool,
    removed_at: Option<sim::SubPos>,
}
fn before_economy(world: &mut World) {
    let (id, target) = {
        let t = world.resource::<Trace>();
        (t.unit, t.target)
    };
    let p = world
        .query::<(&SimId, &sim::Position)>()
        .iter(world)
        .find(|(found, _)| **found == id)
        .map(|(_, p)| p.0);
    let charged = world
        .resource::<SpecialState>()
        .charges
        .contains_key(&target);
    let mut trace = world.resource_mut::<Trace>();
    trace.pre = p;
    trace.had_charge = charged;
}
fn after_economy(world: &mut World) {
    let (id, target) = {
        let t = world.resource::<Trace>();
        (t.unit, t.target)
    };
    let destination = world
        .query::<(&SimId, &sim::Orders)>()
        .iter(world)
        .find(|(found, _)| **found == id)
        .and_then(|(_, o)| o.current.as_ref().map(|o| o.destination));
    let state = world.resource::<SpecialState>();
    let charged = state.charges.contains_key(&target);
    let job = state.auto_defuses.get(&id) == Some(&(target, P0));
    let mut t = world.resource_mut::<Trace>();
    t.saw_job |= job;
    if t.first_destination.is_none() {
        t.first_destination = destination;
    }
    if t.had_charge && !charged && t.removed_at.is_none() {
        t.removed_at = t.pre;
    }
}
fn exercise(start: CellPos, west: bool) {
    let mut app = world();
    let target = mapped(&mut app, 5, "civilian_house");
    let engineer = put(&mut app, P0, "engineer", start.x, start.y);
    let ivan = put(&mut app, P0, "crazy_ivan", 19, 21);
    ok(&tick(&mut app, vec![guard(&[engineer, ivan])]));
    let before = prop_hp(&app, target).unwrap();
    assert_eq!(before, 300);
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
        "actual native IvanBomber shot"
    );
    let charge = app
        .world()
        .resource::<SpecialState>()
        .charges
        .get(&target)
        .expect("real native charge")
        .clone();
    assert!(!charge.c4);
    assert_eq!(charge.source, ivan);
    assert_eq!(charge.damage, 400);
    assert_eq!(
        prop_hp(&app, target),
        Some(before),
        "plant creates charge without HP damage"
    );
    let d = defs(&app);
    let metadata = d
        .metadata(sim::ProductionItem::Unit(d.unit_kind("engineer").unwrap()))
        .unwrap();
    assert_eq!(metadata.bomb_sight, 4);
    let kit = d.weapon(d.weapon_kind("defuse_kit").unwrap()).unwrap();
    assert_eq!(kit.range, 384);
    assert_eq!(kit.min_range, 0);
    assert!(d.warhead(kit.warhead).unwrap().bomb_disarm);
    let en = entity(&mut app, engineer).unwrap();
    let initial = app.world().get::<sim::Position>(en).unwrap().0;
    assert_eq!(
        app.world().resource::<Grid>().spot(start, engineer),
        Some(0)
    );
    assert_eq!(initial, sim::SubPos::spot_of(start, 0));
    assert!(app.world().get::<sim::Mobile>(en).unwrap().step.is_none());
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(HOST)
    );
    let (dx, dy) = house_gap(initial);
    assert!(
        dx * dx + dy * dy <= 1024_i64 * 1024,
        "real current Position is within BombSight4"
    );
    if west {
        assert_eq!(start, CellPos::new(18, 20));
        assert_eq!(house_gap(sim::SubPos::centre_of(start)), (384, 0));
        assert_eq!(house_gap(initial), (448, 0));
        assert!(!kit_reaches(initial));
        // An unheld destination offers no assigned spot; standing_point would merely lie
        // about future infantry range by returning centre here, exactly like the old code.
        assert_eq!(
            app.world()
                .resource::<Grid>()
                .standing_point(CellPos::new(19, 20), engineer),
            sim::SubPos::centre_of(CellPos::new(19, 20))
        );
    } else {
        assert!(kit_reaches(initial), "native in-range action control");
    }
    app.world_mut().insert_resource(Trace {
        unit: engineer,
        target,
        pre: None,
        had_charge: false,
        first_destination: None,
        saw_job: false,
        removed_at: None,
    });
    app.add_systems(
        SimSchedule,
        (
            before_economy
                .in_set(SimSet::Economy)
                .before(SimCoreSet(SimSet::Economy)),
            after_economy
                .in_set(SimSet::Economy)
                .after(SimCoreSet(SimSet::Economy)),
        ),
    );
    // Actual accepted commands hold the planter and release the Engineer to native idle scan.
    ok(&tick(
        &mut app,
        vec![
            guard(&[ivan]),
            issue(
                P0,
                Command::Stop {
                    units: vec![engineer],
                },
            ),
        ],
    ));
    for _ in 0..180 {
        if !app
            .world()
            .resource::<SpecialState>()
            .charges
            .contains_key(&target)
        {
            break;
        }
        ok(&tick(&mut app, vec![]));
    }
    let trace = app.world().resource::<Trace>().clone();
    assert!(
        app.world().resource::<SimTick>().0 < charge.due,
        "fuse expiry cannot masquerade as defuse"
    );
    assert_eq!(prop_hp(&app, target), Some(before));
    assert!(hp(&mut app, engineer).is_some_and(|h| h > 0));
    assert!(hp(&mut app, ivan).is_some_and(|h| h > 0));
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(HOST)
    );
    if trace.removed_at.is_none() {
        let c = app
            .world()
            .resource::<SpecialState>()
            .charges
            .get(&target)
            .expect("failed approach retains native charge");
        assert_eq!(c.due, charge.due);
        assert_eq!(c.damage, 400);
        assert_eq!(c.source, ivan);
    }
    assert!(
        trace.removed_at.is_some(),
        "intended native approach/removal failure; all real premises intact: {trace:?}"
    );
    let actual = trace.removed_at.unwrap();
    assert!(
        kit_reaches(actual),
        "native final action used actual pre-Economy Position: {actual:?}"
    );
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
    if west {
        assert!(trace.saw_job);
        assert_ne!(actual, initial, "real movement into actual DefuseKit range");
        assert_eq!(trace.first_destination, Some(CellPos::new(19, 20)));
        for spot in 0..sim::SPOTS_PER_CELL as u8 {
            assert!(kit_reaches(sim::SubPos::spot_of(
                CellPos::new(19, 20),
                spot
            )));
        }
    } else {
        assert_eq!(
            trace.first_destination, None,
            "do not impose all-spots planning on already in-range actual action"
        );
        assert_eq!(actual, initial);
    }
    run(&mut app, 40); // finish any native in-flight step after actual range was reached
    assert_eq!(prop_hp(&app, target), Some(before));
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .garrisons
            .contains_key(&target)
    );
    cells(&mut app);
}
#[test]
fn ra9_bombsight_west_edge_spot_zero_must_approach_into_real_defusekit_range() {
    exercise(CellPos::new(18, 20), true);
}
#[test]
fn ra9_bombsight_actual_adjacent_and_east_in_range_points_defuse_without_a_planned_move() {
    exercise(CellPos::new(19, 20), false);
    exercise(CellPos::new(22, 20), false);
}
