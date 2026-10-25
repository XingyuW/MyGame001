//! Stock SquidPunch is ordinary damage, not the no-Rules legacy role-disable path.
mod ra7_common;
use ra7_common::*;
use sim::{
    Cell, CellPos, Command, Grid, Health, Mobile, Orders, ProductionItem, SimEvent, SpecialState,
    Weapon,
};
#[test]
fn ra9_stock_squid_punch_keeps_organic_targets_movement_and_has_no_legacy_stun() {
    let mut app = world();
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    water: true,
                    buildable: false,
                    ore: 0,
                };
                48 * 48
            ],
        )
        .expect("sea"),
    );
    let squid = put(&mut app, P0, "giant_squid", 8, 8);
    let target = put(&mut app, P1, "dolphin", 9, 8);
    let sensor = put(&mut app, P0, "dolphin", 8, 10);
    for id in [target, sensor] {
        let e = entity(&mut app, id).expect("actor");
        app.world_mut().entity_mut(e).remove::<Weapon>();
    }
    let d = defs(&app);
    let kind = d.unit_kind("giant_squid").expect("Squid");
    let terms = d.unit_combat(kind).expect("terms");
    let punch = d.weapon_kind("squid_punch").expect("Punch");
    assert_eq!(terms.weapons(false)[1], Some(punch));
    assert_eq!(
        terms.naval_targeting, 3,
        "organic naval target selects secondary"
    );
    assert!(
        d.metadata(ProductionItem::Unit(
            d.unit_kind("dolphin").expect("Dolphin")
        ))
        .expect("metadata")
        .type_immune
    );
    assert!(
        d.unit_combat(d.unit_kind("dolphin").expect("Dolphin"))
            .expect("terms")
            .organic
    );
    let w = d.weapon(punch).expect("weapon");
    let h = d.warhead(w.warhead).expect("warhead");
    assert_eq!(w.damage, 50);
    assert_eq!(h.id, "he");
    assert!(!h.parasite);
    assert_eq!(h.paralyzes, 0);
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P0, target))
    );
    let e = entity(&mut app, target).expect("target");
    assert_eq!(
        app.world().get::<Health>(e).expect("health").hit_points,
        200
    );
    let destination = CellPos::new(12, 8);
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Attack {
                    units: vec![squid],
                    target,
                },
            ),
            issue(
                P1,
                Command::Move {
                    units: vec![target],
                    target: destination,
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
    assert!(
        events.iter().any(
            |e| matches!(e,SimEvent::WeaponFired{attacker,target:t,weapon}
        if *attacker==squid&&*t==target&&*weapon==punch)
        ),
        "actual selected Punch: {events:?}"
    );
    // HE50 × light-armour70% =35, independent of any legacy metadata strength/cooldown.
    assert_eq!(
        app.world().get::<Health>(e).expect("health").hit_points,
        165
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .parasites
            .contains_key(&target)
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .disabled
            .contains_key(&target),
        "ordinary non-Parasite Punch must not leak the old role-only100-tick stun"
    );
    assert_eq!(
        app.world()
            .get::<Orders>(e)
            .expect("orders")
            .current
            .as_ref()
            .map(|o| o.destination),
        Some(destination)
    );
    let progress = app
        .world()
        .get::<Mobile>(e)
        .expect("mobile")
        .step
        .expect("real movement step")
        .progress;
    tick(&mut app, vec![]);
    assert!(
        app.world()
            .get::<Mobile>(e)
            .expect("mobile")
            .step
            .expect("still moving")
            .progress
            > progress
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
