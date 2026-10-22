// Historical preparation note: UNCOMPILED / UNRUN / UNACCEPTED. Proposed crates/sim/tests/ra9_legacy904_last_five.rs.
// Existing ra7_common supplies shipped world/put/issue/tick/entity. No new runtime API.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    Cell, CellPos, Command, ForceTarget, Grid, Health, Players, Position, ProductionItem,
    Projectiles, SimEvent, SimId, SubPos,
};

fn ok(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}
fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<Health>(e).map(|h| h.hit_points)
}
fn invariants(app: &mut App) {
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    assert!(sim::stray_claims(app.world_mut()).is_empty());
    for (_, p) in app.world().resource::<Players>().iter() {
        assert!(p.credits() >= 0);
    }
}
fn force(shooter: SimId, target: SimId) -> sim::IssuedCommand {
    issue(
        P0,
        Command::ForceFire {
            units: vec![shooter],
            target: ForceTarget::Entity(target),
        },
    )
}
/// Same-owner measured actors avoid passive enemy acquisition. These are actual shots on
/// full-stock targets, not HP-zero setup. A bounded wait reports actual death tick events.
fn comet_death(app: &mut App, shooter: SimId, target: SimId) -> (Vec<SimEvent>, usize) {
    let d = defs(app);
    let kind = d.weapon_kind("comet").expect("stock Comet");
    let w = d.weapon(kind).expect("weapon");
    assert_eq!((w.damage, w.projectile_speed, w.reload), (100, 0, 133));
    let mut shots = 0;
    for _ in 0..3_000 {
        let events = tick(app, vec![force(shooter, target)]);
        ok(&events);
        for e in &events {
            if let SimEvent::WeaponFired {
                attacker,
                target: victim,
                weapon,
            } = e
            {
                assert_eq!(
                    (*attacker, *victim, *weapon),
                    (shooter, target, kind),
                    "no hidden source of target damage: {events:?}"
                );
                shots += 1;
            }
        }
        invariants(app);
        if entity(app, target).is_none() {
            assert!(shots > 0, "actual public shot precedes cleanup");
            return (events, shots);
        }
    }
    panic!("stock public Comet did not destroy target within bounded 3000 ticks");
}

#[test]
fn ra9_legacy904_oil_shot_death_is_immediate_bounded_damage_without_later_execution() {
    let mut app = world();
    let shooter = put(&mut app, P0, "prism_tank", 6, 8);
    let oil = put(&mut app, P0, "oil_derrick", 12, 8);
    let survivor = put(&mut app, P0, "mcv", 13, 8);
    let outside = put(&mut app, P0, "mcv", 18, 8);
    let wood = put(&mut app, P0, "civilian_house", 12, 9);
    let wall = put(&mut app, P0, "fortress_walls", 12, 7);
    ok(&tick(&mut app, vec![]));
    let d = defs(&app);
    let blast = d
        .weapon(d.weapon_kind("oil_explosion").expect("OilExplosion"))
        .expect("weapon");
    let wh = d.warhead(blast.warhead).expect("warhead");
    assert_eq!(
        (blast.damage, wh.cell_spread, wh.percent_at_max),
        (600, 1024, 50)
    );
    assert!(wh.wall && wh.wood);
    assert_eq!(
        (hp(&mut app, survivor), hp(&mut app, outside)),
        (Some(1000), Some(1000))
    );
    assert_eq!(
        (hp(&mut app, wood), hp(&mut app, wall)),
        (Some(300), Some(300))
    );
    let mcv = d.unit_kind("mcv").expect("MCV");
    assert!(
        !d.metadata(ProductionItem::Unit(mcv))
            .expect("metadata")
            .self_healing
    );
    let (events, shots) = comet_death(&mut app, shooter, oil);
    // Steel200%: five actual instant Comet hits of200 destroy the full1000HP oil derrick.
    assert_eq!(shots, 5);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::BuildingDestroyed { building, .. } if *building == oil))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::Impact { warhead, at, air }
        if *warhead == blast.warhead && *at == SubPos::centre_of(CellPos::new(12,8)) && !*air))
    );
    // Independent R33 integers at exactly one cell: heavy50%, concrete50%, wood200%.
    // 600 * (100*1024 - 50*256) / (100*1024) = 525 before armour.
    assert_eq!(
        600_i64 * (100 * 1024 - 50 * 256) * 50 / (100 * 1024 * 100),
        262
    );
    assert_eq!(hp(&mut app, survivor), Some(738));
    assert_eq!(hp(&mut app, wall), Some(38));
    assert_eq!(hp(&mut app, wood), None);
    assert!(
        events.iter().any(
            |e| matches!(e, SimEvent::BuildingDestroyed { building, .. } if *building == wood)
        )
    );
    assert_eq!(hp(&mut app, outside), Some(1000));
    // Original5-frame base *7 edge multiplier =35frames; ceil(35*4/3)=47ticks.
    // This is an observation window, not a claim these mapped victims are original
    // EligibleForDelayKill. CAMISC01/CAMISC02/AMMOCRAT are absent from current mappings.
    for _ in 0..47 {
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Stop {
                    units: vec![shooter],
                },
            )],
        );
        ok(&events);
        assert!(
            !events.iter().any(
                |e| matches!(e, SimEvent::Impact { warhead, .. } if *warhead == blast.warhead)
            )
        );
        assert_eq!(
            (
                hp(&mut app, survivor),
                hp(&mut app, outside),
                hp(&mut app, wall)
            ),
            (Some(738), Some(1000), Some(38))
        );
        assert!(entity(&mut app, wood).is_none());
        invariants(&mut app);
    }
}

#[test]
fn ra9_legacy904_wall_between_shooter_and_seen_target_does_not_occlude_stock_shell() {
    let mut app = world();
    let shooter = put(&mut app, P0, "grizzly", 8, 8);
    // Prime actual source sight with no hostile actor available for passive acquisition.
    ok(&tick(&mut app, vec![]));
    let wall = put(&mut app, P1, "fortress_walls", 10, 8);
    let victim = put(&mut app, P1, "chrono_miner", 12, 8);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("player")
            .vision()
            .is_visible(CellPos::new(12, 8))
    );
    let d = defs(&app);
    let weapon = d.weapon_kind("gun_105mm").expect("105mm");
    let w = d.weapon(weapon).expect("weapon");
    assert_eq!(
        (w.damage, w.projectile_speed, w.scatter, w.burst),
        (65, 72, 0, 1)
    );
    let e = entity(&mut app, shooter).expect("shooter");
    let from = app.world().get::<Position>(e).expect("position").0;
    let e = entity(&mut app, victim).expect("victim");
    let to = app.world().get::<Position>(e).expect("position").0;
    assert_eq!(
        (from, to),
        (
            SubPos::centre_of(CellPos::new(8, 8)),
            SubPos::centre_of(CellPos::new(12, 8))
        )
    );
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![shooter],
                target: victim,
            },
        )],
    );
    ok(&events);
    assert!(events.iter().any(
        |e| matches!(e, SimEvent::WeaponFired { attacker, target, weapon: k }
        if *attacker == shooter && *target == victim && *k == weapon)
    ));
    // Four exact centre-to-centre cells; ceil(1024/72)=15 ticks, independent of list.remaining.
    let list = app
        .world()
        .resource::<Projectiles>()
        .iter()
        .collect::<Vec<_>>();
    let p = list
        .iter()
        .find(|p| p.attacker == shooter && p.target == victim)
        .expect("real shell");
    assert_eq!((p.flight, p.remaining, p.weapon), (15, 15, weapon));
    assert_eq!(hp(&mut app, victim), Some(1000));
    assert_eq!(
        w.reload, 80,
        "no second105mm shot can enter the15-tick first flight"
    );
    for elapsed in 1..15 {
        let events = tick(&mut app, vec![]);
        ok(&events);
        assert!(
            !events.iter().any(
                |event| matches!(event,SimEvent::WeaponFired{attacker,..}if *attacker==shooter)
            )
        );
        let shells = app
            .world()
            .resource::<Projectiles>()
            .iter()
            .filter(|p| p.attacker == shooter && p.target == victim && p.weapon == weapon)
            .collect::<Vec<_>>();
        assert_eq!(shells.len(), 1);
        assert_eq!((shells[0].flight, shells[0].remaining), (15, 15 - elapsed));
        assert_eq!(hp(&mut app, victim), Some(1000));
        assert_eq!(hp(&mut app, wall), Some(300));
    }
    let events = tick(&mut app, vec![]);
    ok(&events);
    // Accurate targeted Combat shots are represented by WeaponFired plus native flight/HP,
    // not the area-only Impact event used for a separately reported death payload.
    assert!(!events.iter().any(|e| matches!(e, SimEvent::Impact { .. })));
    assert!(
        !app.world()
            .resource::<Projectiles>()
            .iter()
            .any(|p| p.attacker == shooter && p.target == victim && p.weapon == weapon)
    );
    assert_eq!(
        hp(&mut app, victim),
        Some(935),
        "medium100%; 65-point first hit behind real wall"
    );
    assert_eq!(hp(&mut app, wall), Some(300));
    // Separate existing real WH.wall positive/negative routes stay mandatory:
    // ra3_shipped::shipped_weapons_deal_the_damage_worked_out_from_the_reference and
    // ra3_damage::only_a_warhead_for_walls_damages_a_wall. No trajectory credit from them.
    invariants(&mut app);
}

#[test]
fn ra9_legacy904_unarmed_explodes_shipyards_have_no_payload_selector() {
    for name in ["naval_shipyard", "soviet_naval_shipyard"] {
        let mut app = world();
        // Legal stock3x3 water footprint, a free naval ring row and accessible land shore.
        for y in 8..=11 {
            for x in 12..=14 {
                assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        water: true,
                        ..OPEN
                    }
                ));
            }
        }
        let shooter = put(&mut app, P0, "prism_tank", 6, 8);
        let yard = put(&mut app, P0, name, 12, 8);
        let beside = put(&mut app, P0, "mcv", 11, 8);
        ok(&tick(&mut app, vec![]));
        let d = defs(&app);
        let k = d.building_kind(name).expect("yard kind");
        let m = d.metadata(ProductionItem::Building(k)).expect("metadata");
        assert!(m.explodes && !m.gunner && m.death_weapon.is_none());
        assert!(d.building_combat(k).expect("terms").weapon.is_none());
        assert!(d.weapon_kind("default_death_weapon").is_none());
        assert_eq!(
            (hp(&mut app, yard), hp(&mut app, beside)),
            (Some(1500), Some(1000))
        );
        let (events, shots) = comet_death(&mut app, shooter, yard);
        assert_eq!(shots, 8, "concrete200%: ceil(1500/200) actual Comet shots");
        assert!(events.iter().any(
            |e| matches!(e, SimEvent::BuildingDestroyed { building, .. } if *building == yard)
        ));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SimEvent::Impact { .. }))
                .count(),
            0,
            "targeted Comet reports WeaponFired, and unarmed yard has NO separately reported death payload"
        );
        assert_eq!(hp(&mut app, beside), Some(1000));
        for _ in 0..20 {
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Stop {
                        units: vec![shooter],
                    },
                )],
            );
            ok(&events);
            assert!(!events.iter().any(|e| matches!(e, SimEvent::Impact { .. })));
            assert_eq!(hp(&mut app, beside), Some(1000));
            invariants(&mut app);
        }
    }
}

#[test]
fn ra9_legacy904_actual_primary_death_fallback_is_distinct_from_missing_default() {
    for (name, primary, neighbour_damage) in [
        ("crazy_ivan", "ivan_bomber", 0),
        ("apocalypse", "gun_120mmx", 0),
    ] {
        let mut app = world();
        let shooter = put(&mut app, P0, "prism_tank", 6, 8);
        let target = put(&mut app, P0, name, 12, 8);
        let beside = put(&mut app, P0, "mcv", 13, 8);
        ok(&tick(&mut app, vec![]));
        let d = defs(&app);
        let k = d.unit_kind(name).expect("kind");
        let m = d.metadata(ProductionItem::Unit(k)).expect("metadata");
        assert!(m.explodes && !m.gunner && m.death_weapon.is_none());
        let primary = d.weapon_kind(primary).expect("primary");
        assert_eq!(d.unit_combat(k).expect("terms").weapon, Some(primary));
        let wh = d.weapon(primary).expect("weapon").warhead;
        let target_entity = entity(&mut app, target).expect("live stock target");
        let payload_point = app
            .world()
            .get::<Position>(target_entity)
            .expect("actual fine position")
            .0;
        assert_eq!(
            payload_point,
            app.world()
                .resource::<Grid>()
                .standing_point(CellPos::new(12, 8), target)
        );
        if name == "crazy_ivan" {
            assert_ne!(
                payload_point,
                SubPos::centre_of(CellPos::new(12, 8)),
                "stock infantry holds its actual subcell spot, not the cell centre"
            );
        } else {
            assert_eq!(payload_point, SubPos::centre_of(CellPos::new(12, 8)));
        }
        let (events, _) = comet_death(&mut app, shooter, target);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::UnitDestroyed { unit, .. } if *unit == target))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::Impact { warhead, at, air }
            if *warhead == wh && *at == payload_point && !*air)),
            "real selected death payload at the destroyed actor's fine position: {events:?}"
        );
        assert_eq!(
            hp(&mut app, beside),
            Some(1000 - neighbour_damage),
            "IvanBomber400 has zero spread and no direct target; Apoc radius77 does not reach256"
        );
        invariants(&mut app);
    }
}

#[test]
fn ra9_legacy904_death_selector_premise_enumerates_the_complete_current_loaded_set() {
    let app = world();
    let d = defs(&app);
    let mut rows = Vec::new();
    for (item, m) in &d.arsenal().0 {
        if !(m.explodes || m.gunner || m.death_weapon.is_some()) {
            continue;
        }
        let (id, t) = match *item {
            ProductionItem::Unit(k) => (d.unit(k).expect("unit").id.as_str(), d.unit_combat(k)),
            ProductionItem::Building(k) => (
                d.building(k).expect("building").id.as_str(),
                d.building_combat(k),
            ),
        };
        let name = |w: Option<sim::WeaponKind>| {
            w.map(|k| d.weapon(k).expect("weapon").id.as_str())
                .unwrap_or("none")
        };
        rows.push(format!(
            "{id}\t{}\t{}\t{}\t{}\n",
            m.explodes,
            m.gunner,
            name(m.death_weapon),
            name(t.and_then(|t| t.weapon))
        ));
    }
    rows.sort();
    assert_eq!(
        rows.concat(),
        include_str!("ra9_audit/legacy904-death-selectors.tsv")
    );
    assert!(d.weapon_kind("default_death_weapon").is_none());
    // These real loaded premises alone are not native selector/flag coverage.
    // Shipyard and Primary tests above + explicit Oil damage are the actual routes.
}
