//! UNCOMPILED/UNRUN first5 legacy904 route proposals. Proposed integration test file.
//! Uses existing ra7_common stock world/public spawns/tick helpers; no alternate simulation.
//! Guard/Attack/Move/Deploy go through real CommandQueue and SimSchedule in common::tick.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    CellPos, Command, Direction, Facing, Grid, Health, Players, Position, ProductionItem,
    Projectiles, Props, SimEvent, SimId, SimTick, SpecialState, SubPos, WeaponKind,
};

fn step(app: &mut App, commands: Vec<sim::IssuedCommand>) -> Vec<SimEvent> {
    let e = tick(app, commands);
    assert!(
        !e.iter()
            .any(|v| matches!(v, SimEvent::CommandRejected { .. })),
        "{e:?}"
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    assert!(
        app.world()
            .resource::<Players>()
            .iter()
            .all(|(_, p)| p.credits() >= 0)
    );
    e
}
fn health(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<Health>(e).map(|h| h.hit_points)
}
fn pos(app: &mut App, id: SimId) -> SubPos {
    let e = entity(app, id).expect("live actor");
    app.world().get::<Position>(e).expect("native position").0
}
fn facing(app: &mut App, id: SimId) -> Direction {
    let e = entity(app, id).expect("actor");
    app.world().get::<Facing>(e).expect("actual facing").0
}
fn weapon(app: &App, id: &str) -> WeaponKind {
    defs(app).weapon_kind(id).expect("actual named weapon")
}
fn fires(e: &[SimEvent], id: SimId) -> Vec<(SimId, WeaponKind)> {
    e.iter()
        .filter_map(|e| match e {
            SimEvent::WeaponFired {
                attacker,
                target,
                weapon,
            } if *attacker == id => Some((*target, *weapon)),
            _ => None,
        })
        .collect()
}
fn attack(id: SimId, target: SimId) -> sim::IssuedCommand {
    issue(
        P0,
        Command::Attack {
            units: vec![id],
            target,
        },
    )
}
fn go(id: SimId) -> sim::IssuedCommand {
    issue(
        P0,
        Command::Move {
            units: vec![id],
            target: CellPos::new(0, 0),
        },
    )
}
fn sea(app: &mut App) {
    for y in 0..48 {
        for x in 0..48 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                sim::Cell {
                    water: true,
                    ..OPEN
                }
            ));
        }
    }
}
fn unarmed_noheal(app: &App, id: &str, hp: i32, armour: &str) {
    let d = defs(app);
    let k = d.unit_kind(id).expect("unit");
    let m = d.metadata(ProductionItem::Unit(k)).expect("metadata");
    let t = d.unit_combat(k).expect("combat");
    assert!(t.weapons(false).into_iter().all(|w| w.is_none()));
    assert!(!m.self_healing);
    assert_eq!(t.hit_points, hp);
    assert_eq!(t.armour, d.armour_class(armour).expect("armour"));
}

#[test]
fn ra9_legacy_distributed_omni_current_idle_retains_first_live_target_and_fires_behind() {
    let mut app = world();
    sea(&mut app);
    let source = put(&mut app, P0, "aegis_cruiser", 16, 16);
    step(
        &mut app,
        vec![issue(
            P0,
            Command::Stop {
                units: vec![source],
            },
        )],
    );
    step(&mut app, vec![]);
    let first = put(&mut app, P1, "kirov", 12, 12);
    let d = defs(&app);
    let wk = weapon(&app, "medusa");
    let w = d.weapon(wk).expect("Medusa");
    assert_eq!(
        (w.damage, w.reload, w.projectile_speed, w.burst),
        (100, 20, 191, 1)
    );
    assert_eq!(facing(&mut app, source), Direction::SouthEast);
    let origin = pos(&mut app, source);
    let aim = pos(&mut app, first);
    assert!(
        aim.x < origin.x && aim.y < origin.y,
        "actual target behind SE, not arbitrary facing equality"
    );
    let e = step(&mut app, vec![]);
    assert_eq!(fires(&e, source), vec![(first, wk)]);
    let first_tick = app.world().resource::<SimTick>().0;
    let distance =
        ((i64::from(aim.x - origin.x).pow(2) + i64::from(aim.y - origin.y).pow(2)) as u64).isqrt();
    let flight = distance.div_ceil(191).max(1);
    assert_eq!(flight, 8);
    let p = app
        .world()
        .resource::<Projectiles>()
        .iter()
        .find(|p| p.attacker == source && p.target == first)
        .copied()
        .expect("actual missile");
    assert_eq!(
        (u64::from(p.flight), u64::from(p.remaining)),
        (flight, flight)
    );
    assert_eq!(p.weapon, wk);
    let closer = put(&mut app, P1, "kirov", 15, 12);
    let near = pos(&mut app, closer);
    assert!(
        i64::from(near.x - origin.x).pow(2) + i64::from(near.y - origin.y).pow(2)
            < i64::from(aim.x - origin.x).pow(2) + i64::from(aim.y - origin.y).pow(2)
    );
    let repair_interval = u64::from(
        d.rules()
            .expect("actual rules")
            .economy
            .repair_interval
            .unsigned_abs(),
    );
    assert_eq!(repair_interval, 19);
    let target_kind = d.unit_kind("kirov").expect("Kirov");
    assert!(
        d.metadata(ProductionItem::Unit(target_kind))
            .expect("actual Kirov metadata")
            .self_healing
    );
    let impact_at = first_tick - 1 + flight;
    assert!(!impact_at.is_multiple_of(repair_interval));
    let mut next = None;
    for _ in 0..25 {
        let processing = app.world().resource::<SimTick>().0;
        let e = step(&mut app, vec![]);
        if processing < impact_at {
            assert_eq!(health(&mut app, first), Some(2000));
        } else {
            let heals = (impact_at + 1..=processing)
                .filter(|t| t.is_multiple_of(repair_interval))
                .count() as i32;
            assert_eq!(
                health(&mut app, first),
                Some(1900 + heals),
                "actual100 first impact followed only by source SelfHealing/RepairRate cadence"
            );
        }
        assert_eq!(pos(&mut app, source), origin);
        assert_eq!(facing(&mut app, source), Direction::SouthEast);
        for (t, k) in fires(&e, source) {
            assert_eq!((t, k), (first, wk));
            next = Some(app.world().resource::<SimTick>().0);
        }
        if next.is_some() {
            break;
        }
    }
    assert_eq!(next, Some(first_tick + 20));
    assert_eq!(
        health(&mut app, first),
        Some(1901),
        "one actual100 impact, then one real Kirov self-heal at processing19 before second missile"
    );
    assert_eq!(health(&mut app, closer), Some(2000));
    // This proves the current retention/no-turn gate, not original DistributedFire or generic OmniFire.
}

fn cell_range_case(diagonal: bool) {
    let mut app = world();
    let dog = put(&mut app, P0, "attack_dog", 10, 10);
    let cell = if diagonal {
        CellPos::new(11, 11)
    } else {
        CellPos::new(11, 10)
    };
    let a = put(&mut app, P0, "engineer", cell.x, cell.y);
    let b = put(&mut app, P0, "engineer", cell.x, cell.y);
    step(
        &mut app,
        vec![issue(
            P0,
            Command::Stop {
                units: vec![dog, a, b],
            },
        )],
    );
    step(&mut app, vec![]);
    let spy = put(&mut app, P1, "spy", cell.x, cell.y);
    let d = defs(&app);
    let wk = weapon(&app, "good_teeth");
    let w = d.weapon(wk).expect("bite");
    assert_eq!((w.range, w.projectile_speed, w.damage), (384, 0, 30));
    let dogp = pos(&mut app, dog);
    let spyp = pos(&mut app, spy);
    let dx = i64::from(spyp.x - dogp.x);
    let dy = i64::from(spyp.y - dogp.y);
    assert_eq!(cell.x.abs_diff(10).max(cell.y.abs_diff(10)), 1);
    assert_eq!(app.world().resource::<Grid>().spot(cell, spy), Some(2));
    assert_eq!(
        app.world().resource::<Grid>().holders(cell),
        [Some(a), Some(b), Some(spy)]
    );
    assert_eq!(
        dx * dx + dy * dy > 384i64.pow(2),
        diagonal,
        "real nonfirst infantry spot changes Euclidean eligibility while cells remain adjacent"
    );
    let before = health(&mut app, spy).expect("live spy");
    assert_eq!(before, 100);
    let mut shots = Vec::new();
    for _ in 0..4 {
        let e = step(&mut app, vec![]);
        shots.extend(fires(&e, dog));
        assert_eq!(pos(&mut app, dog), dogp);
        if diagonal {
            assert_eq!(pos(&mut app, spy), spyp);
            assert_eq!(health(&mut app, spy), Some(before));
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .hidden
                    .contains(&(P0, spy)),
                "dog detection keeps actual target available"
            );
        } else if entity(&mut app, spy).is_none() {
            break;
        }
    }
    if diagonal {
        assert!(shots.is_empty());
    } else {
        assert_eq!(shots, vec![(spy, wk)]);
        assert!(
            entity(&mut app, spy).is_none(),
            "real parasite-dog bite, not nominal30 arithmetic"
        );
    }
}
#[test]
fn ra9_legacy_cellrange_adjacent_cells_use_actual_spot_circle_with_inside_control() {
    cell_range_case(true);
    cell_range_case(false);
}

#[test]
fn ra9_legacy_he_direct_and_wood_damage_have_no_later_secondary_fire_in_this_model() {
    let mut app = world();
    let source = put(&mut app, P0, "kirov", 10, 10);
    step(
        &mut app,
        vec![issue(
            P0,
            Command::Guard {
                units: vec![source],
            },
        )],
    );
    step(&mut app, vec![]);
    let target = put(&mut app, P1, "mcv", 11, 10);
    let near = put(&mut app, P1, "mcv", 11, 11);
    let outside = put(&mut app, P1, "mcv", 11, 13);
    unarmed_noheal(&app, "mcv", 1000, "heavy");
    let inside_tree = CellPos::new(12, 10);
    let outside_tree = CellPos::new(14, 10);
    let mut looks = vec![0u8; 48 * 48];
    for c in [inside_tree, outside_tree] {
        assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
            c,
            sim::Cell {
                passable: false,
                buildable: false,
                ..OPEN
            }
        ));
        looks[c.y as usize * 48 + c.x as usize] = 4 << 4;
    }
    let d = defs(&app);
    let props = Props::from_looks(
        app.world().resource::<Grid>(),
        &looks,
        d.armour_class("wood").expect("wood"),
    );
    assert_eq!(
        props.at(inside_tree).expect("controlled tree").hit_points,
        60
    );
    let tree_id = props.id_at(inside_tree).expect("tree");
    app.world_mut().insert_resource(props);
    let wk = weapon(&app, "blimp_bomb");
    let w = d.weapon(wk).expect("bomb");
    let wh = d.warhead(w.warhead).expect("WH");
    assert_eq!((w.damage, w.projectile_speed, w.reload), (250, 38, 67));
    assert_eq!(
        (wh.cell_spread, wh.percent_at_max, wh.wood),
        (512, 50, true)
    );
    assert_eq!(
        d.versus(w.warhead, d.armour_class("heavy").expect("heavy")),
        Some(35)
    );
    assert!(d.weapon_kind("fire").is_none() && d.weapon_kind("fire2").is_none());
    let e = step(&mut app, vec![attack(source, target)]);
    assert_eq!(fires(&e, source), vec![(target, wk)]);
    let missile = app
        .world()
        .resource::<Projectiles>()
        .iter()
        .find(|p| p.attacker == source)
        .copied()
        .expect("real launched bomb");
    assert_eq!((missile.flight, missile.remaining), (7, 7));
    assert_eq!(missile.origin, SubPos::centre_of(CellPos::new(10, 10)));
    assert_eq!(missile.aim, SubPos::centre_of(CellPos::new(11, 10)));
    let mut events = Vec::new();
    for n in 1..=80 {
        let e = step(&mut app, if n == 1 { vec![go(source)] } else { vec![] });
        assert!(
            fires(&e, source).is_empty(),
            "Move suppresses additional volleys during observation"
        );
        events.extend(e);
        if n < 7 {
            assert_eq!(health(&mut app, target), Some(1000));
        }
        if n >= 7 {
            assert_eq!(
                health(&mut app, target),
                Some(913),
                "floor250*35/100=87, no secondary damage"
            );
            assert_eq!(
                health(&mut app, near),
                Some(935),
                "distance256: floor250*.75*.35=65"
            );
            assert_eq!(health(&mut app, outside), Some(1000));
            assert!(app.world().resource::<Props>().at(inside_tree).is_none());
            assert_eq!(
                app.world()
                    .resource::<Props>()
                    .at(outside_tree)
                    .map(|p| p.hit_points),
                Some(60)
            );
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,SimEvent::PropDestroyed{prop,..}if *prop==tree_id))
            .count(),
        1
    );
    assert!(app.world().resource::<Projectiles>().is_empty());
    // Props::from_looks60HP is a controlled public scenery fixture, not stock-map TreeStrength proof.
}

fn no_fragments(kind: &str, elite: bool, selected: &str, base_damage: i32, versus: i32, loss: i32) {
    let mut app = world();
    let source = put(&mut app, P0, kind, 10, 10);
    if elite {
        assert!(sim::promote(app.world_mut(), source, sim::ELITE));
    }
    step(
        &mut app,
        vec![issue(
            P0,
            Command::Guard {
                units: vec![source],
            },
        )],
    );
    step(&mut app, vec![]);
    let target = put(&mut app, P1, "mcv", 13, 10);
    let a = put(&mut app, P1, "mcv", 13, 11);
    let b = put(&mut app, P1, "mcv", 14, 10);
    unarmed_noheal(&app, "mcv", 1000, "heavy");
    let d = defs(&app);
    let wk = weapon(&app, selected);
    let w = d.weapon(wk).expect("selected native weapon");
    let wh = d.warhead(w.warhead).expect("warhead");
    assert_eq!(
        (w.damage, w.projectile_speed, wh.cell_spread),
        (base_damage, 0, 0)
    );
    assert_eq!(
        d.versus(w.warhead, d.armour_class("heavy").expect("heavy")),
        Some(versus)
    );
    let se = entity(&mut app, source).expect("source");
    assert_eq!(
        app.world()
            .get::<sim::Experience>(se)
            .expect("actual rank")
            .firepower_pm(),
        if elite { 1100 } else { 1000 }
    );
    let expected = (i64::from(base_damage) * if elite { 1100 } else { 1000 } * i64::from(versus)
        / (1000 * 100)) as i32;
    assert_eq!(expected, loss);
    let e = step(&mut app, vec![attack(source, target)]);
    assert_eq!(fires(&e, source), vec![(target, wk)]);
    assert_eq!(health(&mut app, target), Some(1000 - loss));
    assert_eq!(health(&mut app, a), Some(1000));
    assert_eq!(health(&mut app, b), Some(1000));
    assert!(app.world().resource::<Projectiles>().is_empty());
    for n in 0..80 {
        let e = step(&mut app, if n == 0 { vec![go(source)] } else { vec![] });
        assert!(fires(&e, source).is_empty());
        assert!(
            app.world().resource::<Projectiles>().is_empty(),
            "no secondary fragment launch"
        );
        assert_eq!(health(&mut app, target), Some(1000 - loss));
        assert_eq!(health(&mut app, a), Some(1000));
        assert_eq!(health(&mut app, b), Some(1000));
    }
}
#[test]
fn ra9_legacy_prism_base_and_elite_have_primary_damage_without_shrapnel() {
    no_fragments("prism_tank", false, "comet", 100, 50, 50);
    no_fragments("prism_tank", true, "super_comet", 150, 50, 82);
}
#[test]
fn ra9_legacy_elite_tesla_tank_and_trooper_have_primary_damage_without_shrapnel() {
    no_fragments("tesla_tank", true, "tank_bolt_e", 150, 100, 165);
    no_fragments("tesla_trooper", true, "electric_bolt_e", 50, 100, 55);
}

fn sonic(elite: bool) {
    let mut app = world();
    sea(&mut app);
    let source = put(&mut app, P0, "dolphin", 10, 10);
    if elite {
        assert!(sim::promote(app.world_mut(), source, sim::ELITE));
    }
    step(
        &mut app,
        vec![issue(
            P0,
            Command::Guard {
                units: vec![source],
            },
        )],
    );
    step(&mut app, vec![]);
    let mid = put(&mut app, P1, "amphibious_transport", 12, 10);
    let target = put(&mut app, P1, "amphibious_transport", 14, 10);
    let side = put(&mut app, P1, "amphibious_transport", 13, 11);
    unarmed_noheal(&app, "amphibious_transport", 300, "light");
    let d = defs(&app);
    let wk = weapon(&app, if elite { "sonic_zap_e" } else { "sonic_zap" });
    let w = d.weapon(wk).expect("sonic");
    let wh = d.warhead(w.warhead).expect("WH");
    assert_eq!(
        (w.damage, w.projectile_speed, w.burst),
        (if elite { 8 } else { 4 }, 191, if elite { 2 } else { 1 })
    );
    assert!(w.sonic);
    assert_eq!((wh.cell_spread, wh.percent_at_max), (26, 100));
    assert_eq!(
        d.versus(w.warhead, d.armour_class("light").expect("light")),
        Some(100)
    );
    let e = step(&mut app, vec![attack(source, target)]);
    assert_eq!(fires(&e, source), vec![(target, wk)]);
    let p = app
        .world()
        .resource::<Projectiles>()
        .iter()
        .find(|p| p.attacker == source)
        .copied()
        .expect("real sonic projectile");
    assert_eq!(p.firepower_pm, if elite { 1100 } else { 1000 });
    assert_eq!((p.flight, p.remaining), (6, 6));
    assert_eq!(p.origin, SubPos::centre_of(CellPos::new(10, 10)));
    assert_eq!(p.aim, SubPos::centre_of(CellPos::new(14, 10)));
    let mut launched = 1;
    for elapsed in 1..=12 {
        let e = step(&mut app, vec![]);
        for (t, k) in fires(&e, source) {
            assert_eq!((t, k), (target, wk));
            assert!(elite && elapsed == 5);
            launched += 1;
        }
        assert_eq!(
            health(&mut app, mid),
            Some(300),
            "on-line intermediate lies outside the26-subcell endpoint spread"
        );
        assert_eq!(health(&mut app, side), Some(300));
        let first = elapsed >= 6;
        let second = elite && elapsed >= 11;
        let damage = if elite {
            8 * (i32::from(first) + i32::from(second))
        } else {
            4 * i32::from(first)
        };
        assert_eq!(health(&mut app, target), Some(300 - damage));
    }
    assert_eq!(launched, if elite { 2 } else { 1 });
    for n in 0..40 {
        step(&mut app, if n == 0 { vec![go(source)] } else { vec![] });
        assert_eq!(health(&mut app, mid), Some(300));
        assert_eq!(health(&mut app, side), Some(300));
        assert_eq!(
            health(&mut app, target),
            Some(if elite { 284 } else { 296 })
        );
    }
}
#[test]
fn ra9_legacy_sonic_base_and_elite_damage_endpoint_but_not_ambient_line() {
    sonic(false);
    sonic(true);
}

#[test]
fn ra9_legacy_radiation_current_cell_model_ignores_subcell_spot_with_immune_and_edge_controls() {
    let mut app = world();
    let source = put(&mut app, P0, "desolator", 20, 20);
    let ids = (0..3)
        .map(|_| put(&mut app, P0, "conscript", 21, 20))
        .collect::<Vec<_>>();
    let immune = put(&mut app, P0, "desolator", 22, 20);
    let edge = put(&mut app, P0, "conscript", 30, 20);
    let d = defs(&app);
    let k = d.unit_kind("desolator").expect("Desolator");
    let wk = d
        .unit_combat(k)
        .expect("terms")
        .secondary
        .expect("real deploy selector");
    assert_eq!(wk, weapon(&app, "rad_eruption_weapon"));
    let w = d.weapon(wk).expect("weapon");
    assert!(w.area_fire);
    assert_eq!((w.rad_level, w.reload), (500, 80));
    assert_eq!(d.warhead(w.warhead).expect("WH").cell_spread, 2560);
    let r = d.rules().expect("rules").radiation;
    assert_eq!(
        (r.application_delay, r.level_delay, r.level_factor_pm),
        (21, 120, 200)
    );
    for i in 0..3 {
        assert_eq!(
            app.world()
                .resource::<Grid>()
                .spot(CellPos::new(21, 20), ids[i]),
            Some(i as u8)
        );
        for j in 0..i {
            assert_ne!(pos(&mut app, ids[i]), pos(&mut app, ids[j]));
        }
        assert_eq!(health(&mut app, ids[i]), Some(125));
    }
    step(&mut app, vec![]);
    step(&mut app, vec![]);
    let born = app.world().resource::<SimTick>().0;
    assert_eq!(born, 2);
    let e = step(&mut app, vec![issue(P0, Command::Deploy { unit: source })]);
    assert!(
        fires(&e, source).is_empty(),
        "site deploy is not a fabricated WeaponFired event"
    );
    let state = app.world().resource::<SpecialState>();
    assert!(state.deployed.contains_key(&source));
    assert_eq!(state.eruption_next.get(&source), Some(&(born + 80)));
    assert_eq!(state.radiation_sites.len(), 1);
    let site = &state.radiation_sites[0];
    assert_eq!(
        (site.cell, site.spread, site.level, site.born),
        (CellPos::new(20, 20), 2560, 500, born)
    );
    assert_eq!(site.cell_level(21, 120, CellPos::new(21, 20)), 450);
    while app.world().resource::<SimTick>().0 <= 21 {
        let processing = app.world().resource::<SimTick>().0;
        step(&mut app, vec![]);
        for id in &ids {
            assert_eq!(
                health(&mut app, *id),
                Some(if processing < 21 { 125 } else { 35 }),
                "500*(10-1)/10=450; floor450*.2=90, same cell at three real spots"
            );
        }
        assert_eq!(health(&mut app, immune), Some(150));
        assert_eq!(health(&mut app, edge), Some(125));
    }
    // Actual current cell application + real static dispatcher proves model boundary; no native
    // CellInset flag getter or claimed original inset formula. No broad positional immunity claim.
}
