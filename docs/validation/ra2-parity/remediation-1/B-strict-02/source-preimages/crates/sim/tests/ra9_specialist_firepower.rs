//! Native stored Squid and live Yuri rank times actual crate firepower products.
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{
    Cell, CellPos, Command, CrateContent, Experience, Grid, Position, ProductionItem, SimId,
    SimTick, SpecialState,
};

fn source_product(app: &mut bevy_app::App, source: SimId, rank: u8, crate_bonus: bool) -> u32 {
    if rank > 0 {
        assert!(sim::promote(app.world_mut(), source, rank));
    }
    let e = entity(app, source).unwrap();
    let xp = app.world().get::<Experience>(e).unwrap();
    assert_eq!(xp.rank, rank);
    assert_eq!(
        xp.bonus.firepower_pm,
        if crate_bonus { 2000 } else { 1000 },
        "real Firepower crate premise"
    );
    let ranked = if rank > 0 { 1100_u32 } else { 1000 };
    assert_eq!(xp.ranked.firepower_pm, ranked);
    ranked * if crate_bonus { 2000 } else { 1000 } / 1000
}

fn squid_case(rank: u8, crate_bonus: bool) {
    let mut app = world();
    if crate_bonus {
        app.world_mut().insert_resource(only_powerup("firepower"));
    }
    let mut terrain = vec![
        Cell {
            passable: true,
            water: true,
            buildable: false,
            ore: 0
        };
        48 * 48
    ];
    for y in 0..5 {
        for x in 0..5 {
            terrain[y * 48 + x] = OPEN;
        }
    }
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, terrain).unwrap());
    put(&mut app, P0, "power_plant", 0, 0); // excludes FreeMCV override, without changing crate rule
    if crate_bonus {
        crate_at(&mut app, CellPos::new(8, 8));
    }
    let source = put(&mut app, P0, "giant_squid", 8, 8);
    let events = tick(&mut app, vec![]);
    if crate_bonus {
        assert!(events.iter().any(|e| matches!(e, sim::SimEvent::CrateTaken { unit, content: CrateContent::Firepower, .. } if *unit == source)));
    }
    let pm = source_product(&mut app, source, rank, crate_bonus);
    let host = put(&mut app, P1, "destroyer", 9, 8);
    disarm(&mut app, host);
    let events = attack(&mut app, source, host);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, source).is_none(), "real native attachment");
    let stored = app.world().resource::<SpecialState>().parasites[&host].clone();
    assert_eq!(stored.passenger.id, source);
    assert_eq!(stored.passenger.experience.unwrap().firepower_pm(), pm);
    let d = defs(&app);
    let w = d.weapon(stored.weapon).unwrap();
    let h = d.warhead(w.warhead).unwrap();
    let armour = d
        .unit_combat(d.unit_kind("destroyer").unwrap())
        .unwrap()
        .armour;
    let versus = h.versus[usize::from(armour.0)];
    assert_eq!(versus, 100);
    let expected = i64::from(w.damage) * i64::from(pm) * i64::from(versus) / (1000 * 100);
    assert!(expected > 0);
    let before = hp(&mut app, host);
    assert!(
        before - expected as i32 > 300,
        "durable target safely above all culling thresholds"
    );
    for _ in 0..1000 {
        if app.world().resource::<SimTick>().0 == stored.due {
            break;
        }
        assert!(app.world().resource::<SimTick>().0 < stored.due);
        tick(&mut app, vec![]);
    }
    assert_eq!(app.world().resource::<SimTick>().0, stored.due);
    tick(&mut app, vec![]);
    assert_eq!(
        hp(&mut app, host),
        before - expected as i32,
        "stored source product must reach the real periodic hit exactly once"
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .parasites
            .contains_key(&host)
    );
    cells(&mut app);
}

#[test]
fn ra9_squid_base_periodic_damage_positive() {
    squid_case(0, false);
}
#[test]
fn ra9_squid_veteran_periodic_damage_uses_rank() {
    squid_case(1, false);
}
#[test]
fn ra9_squid_elite_periodic_damage_uses_rank_and_selected_weapon() {
    squid_case(2, false);
}
#[test]
fn ra9_squid_periodic_damage_uses_real_firepower_crate() {
    squid_case(0, true);
}
#[test]
fn ra9_squid_elite_periodic_damage_multiplies_rank_and_crate() {
    squid_case(2, true);
}

fn yuri_case(rank: u8, crate_bonus: bool) {
    let mut app = world();
    // Only the passive victim's maximum HP is enlarged. All source weapons, Verses,
    // immunities, projectile geometry, rank abilities and crate values stay shipped.
    let d = combat_variant(&[(
        "(unit: \"conscript\", hit_points: 125,",
        "(unit: \"conscript\", hit_points: 2000,",
    )]);
    // Use the independently checked powerup selector, then reapply the durable target fixture.
    let d = if crate_bonus {
        only_powerup("firepower")
            .with_combat(variant(
                "combat.ron",
                &[(
                    "(unit: \"conscript\", hit_points: 125,",
                    "(unit: \"conscript\", hit_points: 2000,",
                )],
            ))
            .unwrap()
    } else {
        d
    };
    app.world_mut().insert_resource(d);
    put(&mut app, P0, "power_plant", 0, 0);
    if crate_bonus {
        crate_at(&mut app, CellPos::new(10, 10));
    }
    let source = put(&mut app, P0, "yuri", 10, 10);
    let events = tick(&mut app, vec![]);
    if crate_bonus {
        assert!(events.iter().any(|e| matches!(e, sim::SimEvent::CrateTaken { unit, content: CrateContent::Firepower, .. } if *unit == source)));
    }
    let pm = source_product(&mut app, source, rank, crate_bonus);
    // Spawn passive victims immediately before Deploy, so no preceding ordinary mind control
    // or unrelated shot can supply the measured damage.
    let victim = put(&mut app, P1, "conscript", 11, 10);
    let immune = put(&mut app, P1, "yuri", 10, 11);
    let armour_zero = put(&mut app, P1, "rhino", 11, 11);
    for id in [victim, immune, armour_zero] {
        disarm(&mut app, id);
    }
    let source_e = entity(&mut app, source).unwrap();
    let victim_e = entity(&mut app, victim).unwrap();
    let from = app.world().get::<Position>(source_e).unwrap().0;
    let to = app.world().get::<Position>(victim_e).unwrap().0;
    let distance = ((i64::from(from.x) - i64::from(to.x)).pow(2)
        + (i64::from(from.y) - i64::from(to.y)).pow(2))
    .isqrt();
    let d = defs(&app);
    let w = d.weapon(d.weapon_kind("psi_wave").unwrap()).unwrap();
    let h = d.warhead(w.warhead).unwrap();
    let target_terms = d.unit_combat(d.unit_kind("conscript").unwrap()).unwrap();
    let versus = h.versus[usize::from(target_terms.armour.0)];
    let radius = i128::from(h.cell_spread);
    let expected = i128::from(w.damage)
        * i128::from(pm)
        * (100 * radius - i128::from(100 - h.percent_at_max) * i128::from(distance))
        * i128::from(versus)
        / (1000 * 100 * radius * 100);
    assert!(expected > 0 && expected < 2000);
    let before = hp(&mut app, victim);
    let immune_hp = hp(&mut app, immune);
    let armour_hp = hp(&mut app, armour_zero);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: source })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(
        hp(&mut app, victim),
        before - expected as i32,
        "live Yuri product must reach native Deploy pulse exactly once"
    );
    assert_eq!(hp(&mut app, immune), immune_hp, "psionic immunity remains");
    assert_eq!(
        hp(&mut app, armour_zero),
        armour_hp,
        "zero-Verses armour remains immune"
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .deployed
            .contains_key(&source)
    );
    let e = entity(&mut app, source).unwrap();
    assert!(app.world().get::<sim::Weapon>(e).unwrap().cooldown > 0);
    assert!(
        d.metadata(ProductionItem::Unit(d.unit_kind("yuri").unwrap()))
            .unwrap()
            .type_immune
    );
    cells(&mut app);
}

#[test]
fn ra9_yuri_base_deploy_pulse_positive() {
    yuri_case(0, false);
}
#[test]
fn ra9_yuri_veteran_deploy_pulse_uses_rank() {
    yuri_case(1, false);
}
#[test]
fn ra9_yuri_elite_deploy_pulse_uses_rank() {
    yuri_case(2, false);
}
#[test]
fn ra9_yuri_deploy_pulse_uses_real_firepower_crate() {
    yuri_case(0, true);
}
#[test]
fn ra9_yuri_elite_deploy_pulse_multiplies_rank_and_crate() {
    yuri_case(2, true);
}
