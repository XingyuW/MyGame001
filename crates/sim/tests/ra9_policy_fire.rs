//! carry38 current moving-fire policy: exact stock source aliases, public commands.
//! Targets alone have Weapon removed before their first tick (controlled unarmed targets).
//! No source weapon/HP/ammo/reload/Orders/Mobile/Owner is forged; no OpportunityFire getter.
mod ra4_common;
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    CellPos, Command, Grid, HarvestState, Harvester, Health, Mobile, Orders, Owner, Players,
    Position, SimEvent, SimId, Weapon, WeaponKind,
};

fn pf_entity(app: &mut App, id: SimId) -> bevy_ecs::entity::Entity {
    entity(app, id).expect("live actual actor")
}
fn pf_hp(app: &mut App, id: SimId) -> i32 {
    let e = pf_entity(app, id);
    let hp = app.world().get::<Health>(e).unwrap().hit_points;
    assert!(hp > 0);
    hp
}
fn pf_pos(app: &mut App, id: SimId) -> sim::SubPos {
    let e = pf_entity(app, id);
    app.world().get::<Position>(e).unwrap().0
}
fn pf_weapon(app: &mut App, id: SimId) -> Weapon {
    let e = pf_entity(app, id);
    *app.world().get::<Weapon>(e).unwrap()
}
fn pf_tick(
    app: &mut App,
    commands: Vec<sim::IssuedCommand>,
    target: Option<SimId>,
) -> Vec<SimEvent> {
    let events = tick(app, commands);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    if let Some(target) = target {
        assert!(!events.iter().any(|e| matches!(e,
            SimEvent::WeaponFired { attacker, .. } if *attacker == target)));
        pf_hp(app, target);
    }
    ra4_common::assert_cells(app, "carry38 native every tick");
    assert!(
        app.world()
            .resource::<Players>()
            .iter()
            .all(|(_, p)| p.credits() >= 0)
    );
    events
}
fn pf_shot(events: &[SimEvent], source: SimId, target: SimId, weapon: WeaponKind) -> bool {
    events.iter().any(|e| {
        matches!(e,
        SimEvent::WeaponFired { attacker, target: found, weapon: used }
        if *attacker == source && *found == target && *used == weapon)
    })
}
fn pf_no_source_shot(events: &[SimEvent], source: SimId) {
    assert!(!events.iter().any(|e| matches!(e,
        SimEvent::WeaponFired { attacker, .. } if *attacker == source)));
}
fn pf_target(app: &mut App, name: &str, x: i32, y: i32) -> SimId {
    let target = put(app, P1, name, x, y);
    let e = pf_entity(app, target);
    app.world_mut().entity_mut(e).remove::<Weapon>();
    target
}
fn pf_sea(app: &mut App) {
    for y in 0..48 {
        for x in 0..48 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                sim::Cell {
                    passable: false,
                    water: true,
                    buildable: false,
                    ore: 0
                }
            ));
        }
    }
}

#[test]
fn ra9_c38_five_armed_stock_types_move_without_fire_but_idle_and_attackmove_fire() {
    // Independent exact source/mapped-primary census. Current no-Move-fire intentionally
    // differs from these five yes literals; the test does not actuate original flags.
    let cases = [
        ("AEGIS", "aegis_cruiser", "medusa", "kirov", true),
        (
            "DEST",
            "destroyer",
            "gun_155mm",
            "amphibious_transport",
            true,
        ),
        ("HARV", "war_miner", "gun_20mm_rapid", "mcv", false),
        ("HTNK", "rhino", "gun_120mm", "mcv", false),
        ("MTNK", "grizzly", "gun_105mm", "mcv", false),
    ];
    let table = include_str!("ra9_audit/carry38-contracts.tsv");
    let source: std::collections::BTreeSet<_> = table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let c: Vec<_> = line.split('\t').collect();
            (c[4] == "moving-fire").then_some((c[1], c[2], c[3]))
        })
        .collect();
    assert_eq!(
        source,
        cases
            .iter()
            .map(|c| (c.0, "OpportunityFire", "yes"))
            .collect()
    );
    for (section, name, primary, target_name, naval) in cases {
        for mode in ["Move", "Idle", "AttackMove"] {
            let mut app = world();
            if naval {
                pf_sea(&mut app);
            }
            let source = put(&mut app, P0, name, 16, 16);
            let expected = defs(&app).weapon_kind(primary).unwrap();
            assert_eq!(
                pf_weapon(&mut app, source).kind,
                expected,
                "{section}: actual primary"
            );
            assert_eq!(pf_weapon(&mut app, source).cooldown, 0);
            let initial_hp = pf_hp(&mut app, source);
            // Prime the source alone under a public Stop, before hostile target creation.
            let prime = pf_tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Stop {
                        units: vec![source],
                    },
                )],
                None,
            );
            pf_no_source_shot(&prime, source);
            let target = pf_target(&mut app, target_name, 19, 16);
            let target_hp = pf_hp(&mut app, target);
            let start = pf_pos(&mut app, source);
            let dest = CellPos::new(16, 28);
            let command = match mode {
                "Move" => Command::Move {
                    units: vec![source],
                    target: dest,
                },
                "Idle" => Command::Stop {
                    units: vec![source],
                },
                _ => Command::AttackMove {
                    units: vec![source],
                    target: dest,
                },
            };
            let mut events = pf_tick(&mut app, vec![issue(P0, command)], Some(target));
            if mode == "Move" {
                // After this actual Visibility pass, the proper-layer target is visible.
                assert!(
                    app.world()
                        .resource::<Players>()
                        .get(P0)
                        .unwrap()
                        .vision()
                        .is_visible(CellPos::new(19, 16))
                );
                for _ in 0..20 {
                    pf_no_source_shot(&events, source);
                    let e = pf_entity(&mut app, source);
                    let orders = app.world().get::<Orders>(e).unwrap();
                    assert_eq!(orders.current.as_ref().map(|o| o.destination), Some(dest));
                    let w = pf_weapon(&mut app, source);
                    assert_eq!(
                        (w.cooldown, w.burst_left, w.target, w.order),
                        (0, 0, None, None)
                    );
                    events = pf_tick(&mut app, vec![], Some(target));
                }
                pf_no_source_shot(&events, source);
                assert_ne!(
                    pf_pos(&mut app, source),
                    start,
                    "{section}: actual public Move travel"
                );
                assert_eq!(pf_hp(&mut app, target), target_hp);
            } else {
                let mut fired = false;
                let mut damaged = false;
                for _ in 0..80 {
                    if pf_shot(&events, source, target, expected) {
                        fired = true;
                        let e = pf_entity(&mut app, source);
                        assert!(
                            app.world().get::<Orders>(e).unwrap().current.is_none(),
                            "{section}: acquisition clears the ordinary move order"
                        );
                    }
                    damaged |= pf_hp(&mut app, target) < target_hp;
                    if fired && damaged {
                        break;
                    }
                    events = pf_tick(&mut app, vec![], Some(target));
                }
                assert!(
                    fired && damaged,
                    "{section}/{mode}: actual selected primary and positive damage"
                );
            }
            assert_eq!(
                pf_hp(&mut app, source),
                initial_hp,
                "{section}: no hostile damage"
            );
            let e = pf_entity(&mut app, source);
            assert_eq!(app.world().get::<Owner>(e).unwrap().0, P0);
        }
    }
}

#[test]
fn ra9_c38_real_war_miner_harvest_travel_suppresses_fire_but_stationary_work_can_fire() {
    let mut app = world();
    let field = CellPos::new(20, 16);
    assert!(
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(field, sim::Cell { ore: 20, ..OPEN })
    );
    let miner = put(&mut app, P0, "war_miner", 16, 16);
    let d = defs(&app);
    let terms = d.harvester(d.unit_kind("war_miner").unwrap()).unwrap();
    assert_eq!((terms.harvest_rate, terms.harvest_interval), (1, 19));
    let expected = d.weapon_kind("gun_20mm_rapid").unwrap();
    assert_eq!(pf_weapon(&mut app, miner).kind, expected);
    pf_tick(
        &mut app,
        vec![issue(P0, Command::Stop { units: vec![miner] })],
        None,
    );
    let target = pf_target(&mut app, "mcv", 19, 19);
    let target_hp = pf_hp(&mut app, target);
    let source_hp = pf_hp(&mut app, miner);
    let start = pf_pos(&mut app, miner);
    let mut events = pf_tick(
        &mut app,
        vec![issue(
            P0,
            Command::Harvest {
                units: vec![miner],
                target: field,
            },
        )],
        Some(target),
    );
    let mut travel = 0;
    let mut ready_visible_travel = 0;
    let mut working_shot = false;
    let mut harvested = false;
    for _ in 0..400 {
        let e = pf_entity(&mut app, miner);
        let mobile = *app.world().get::<Mobile>(e).unwrap();
        let h = *app.world().get::<Harvester>(e).unwrap();
        let moving_order = app.world().get::<Orders>(e).unwrap().current.is_some();
        if moving_order {
            travel += 1;
            pf_no_source_shot(&events, miner);
            assert_eq!(h.work, 0);
            let w = pf_weapon(&mut app, miner);
            assert_eq!(
                w.cooldown, 0,
                "travel negative must not be reload suppression"
            );
            if app
                .world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .vision()
                .is_visible(CellPos::new(19, 19))
            {
                ready_visible_travel += 1;
            }
        }
        if mobile.cell == field && mobile.step.is_none() && h.state == HarvestState::ToOre(field) {
            harvested |= h.load > 0;
            if h.work > 0 && pf_shot(&events, miner, target, expected) {
                working_shot = true;
            }
        }
        if working_shot && harvested {
            break;
        }
        events = pf_tick(&mut app, vec![], Some(target));
    }
    assert!(
        travel > 0 && ready_visible_travel > 0,
        "real Harvest movement with visible legal target and ready weapon"
    );
    assert_ne!(pf_pos(&mut app, miner), start);
    assert!(
        working_shot && harvested,
        "same public Harvest workflow reaches actual work/extraction and selected WeaponFired"
    );
    assert!(pf_hp(&mut app, target) < target_hp);
    assert_eq!(pf_hp(&mut app, miner), source_hp);
    assert!(app.world().resource::<Grid>().cell(field).unwrap().ore < 20);
}
