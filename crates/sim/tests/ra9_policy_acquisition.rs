//! carry38 CanPassiveAquire=false: current eligible ordinary acquisition, not original flag use.
//! Real stock actors/CommandQueue/SimSchedule. Only hostile target Weapon is removed as
//! controlled unarmed setup; source HP/ammo/cooldown/bays/Owner/Orders are not injected.
mod ra4_common;
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    Building, CellPos, Command, Domain, Grid, Health, Owner, Players, ProductionItem, SimEvent,
    SimId, SpecialState, Unit, Weapon,
};

fn ac_entity(app: &mut App, id: SimId) -> bevy_ecs::entity::Entity {
    entity(app, id).expect("real live actor")
}
fn ac_hp(app: &mut App, id: SimId) -> i32 {
    let e = ac_entity(app, id);
    let hp = app.world().get::<Health>(e).unwrap().hit_points;
    assert!(hp > 0);
    hp
}
fn ac_owner(app: &mut App, id: SimId) -> sim::PlayerId {
    let e = ac_entity(app, id);
    app.world().get::<Owner>(e).unwrap().0
}
fn ac_weapon(app: &mut App, id: SimId) -> Weapon {
    let e = ac_entity(app, id);
    *app.world().get::<Weapon>(e).unwrap()
}
fn ac_tick(
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
        assert!(
            !events.iter().any(|e| matches!(e,
            SimEvent::WeaponFired { attacker, .. } if *attacker == target)),
            "controlled target fired"
        );
        ac_hp(app, target);
    }
    ra4_common::assert_cells(app, "carry38 acquisition every tick");
    assert!(
        app.world()
            .resource::<Players>()
            .iter()
            .all(|(_, p)| p.credits() >= 0)
    );
    events
}
fn ac_unarmed(app: &mut App, id: SimId) {
    let e = ac_entity(app, id);
    app.world_mut().entity_mut(e).remove::<Weapon>();
}
fn ac_no_fire(events: &[SimEvent], source: SimId) {
    assert!(!events.iter().any(|e| matches!(e,
        SimEvent::WeaponFired { attacker, .. } if *attacker == source)));
    assert!(!events.iter().any(|e| matches!(e,
        SimEvent::UnitLaunched { parent, .. } if *parent == source)));
}
fn ac_world(section: &str, name: &str) -> (App, SimId, CellPos, Option<SimId>) {
    let mut app = world();
    if matches!(section, "CARRIER" | "DRED") {
        for y in 0..48 {
            for x in 16..48 {
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
        // A genuine allied observer supplies the far shore target's sight (Dred minrange).
        put(&mut app, P0, "engineer", 2, 14);
    } else if section == "PDPLANE" {
        // Stock PDPLANE Sight0 cannot prime its target. Engineer Sight4 at17,14
        // naturally sees target19,16 (distance sqrt8); old15,13 was distance5.
        put(&mut app, P0, "engineer", 17, 14);
    } else {
        put(&mut app, P0, "engineer", 15, 13);
    }
    let (at, pad) = if matches!(section, "BEAG" | "ORCA") {
        let pad = put(&mut app, P0, "airforce_command", 10, 10);
        let e = ac_entity(&mut app, pad);
        let cell = sim::dock_cell(app.world().get::<Building>(e).unwrap().footprint, 0);
        (cell, Some(pad))
    } else {
        (CellPos::new(16, 16), None)
    };
    let source = put(&mut app, P0, name, at.x, at.y);
    // Prime real sight/pad assignment with source alone: no initial auto-combat.
    ac_tick(
        &mut app,
        vec![issue(
            P0,
            Command::Stop {
                units: vec![source],
            },
        )],
        None,
    );
    assert_eq!(ac_owner(&mut app, source), P0);
    assert_eq!(ac_weapon(&mut app, source).cooldown, 0);
    assert!(ac_weapon(&mut app, source).order.is_none());
    if let Some(pad) = pad {
        let d = defs(&app);
        let k = d.unit_kind(name).unwrap();
        let m = d.metadata(ProductionItem::Unit(k)).unwrap();
        assert!(m.airport_bound);
        assert_eq!(m.ammo, 1);
        let state = app.world().resource::<SpecialState>();
        assert_eq!(state.docks.get(&source), Some(&(pad, 0)));
        assert_eq!(state.ammo_of(source, m.ammo), 1);
    }
    (app, source, at, pad)
}
fn ac_target(app: &mut App, section: &str, at: CellPos) -> (SimId, CellPos) {
    let (target, cell) = if matches!(section, "CARRIER" | "DRED") {
        (
            put(app, P1, "construction_yard", 4, 15),
            CellPos::new(4, 15),
        )
    } else if matches!(section, "DTRUCK" | "TERROR") {
        // Concrete yard1000 survives the bounded close-range blast/footprint hits;
        // a750-HP wood power plant is not an independent survival premise.
        (
            put(app, P1, "construction_yard", at.x + 1, at.y),
            CellPos::new(at.x + 1, at.y),
        )
    } else if section == "V3" {
        // Seven cells is inside source Sight7/Range18 and outside MinimumRange5.
        (
            put(app, P1, "mcv", at.x + 7, at.y),
            CellPos::new(at.x + 7, at.y),
        )
    } else if section == "YURIPR" {
        (
            put(app, P1, "grizzly", at.x + 3, at.y),
            CellPos::new(at.x + 3, at.y),
        )
    } else {
        (
            put(app, P1, "mcv", at.x + 3, at.y),
            CellPos::new(at.x + 3, at.y),
        )
    };
    ac_unarmed(app, target);
    assert_eq!(ac_owner(app, target), P1);
    // The source/real scout primed these cells before target creation. No Vision mutation.
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(cell),
        "{section}: real source/scout prime must see target cell {cell:?}"
    );
    (target, cell)
}

#[test]
fn ra9_c38_ordinary_capable_stock_idle_and_explicit_acquire_with_real_effects_and_move_controls() {
    let cases = [
        ("BEAG", "black_eagle", "maverick2", ""),
        ("CARRIER", "aircraft_carrier", "hornet_launcher", "hornet"),
        ("DRED", "dreadnought", "dred_launcher", "dred_missile"),
        ("DTRUCK", "demolition_truck", "demobomb", ""),
        ("ORCA", "harrier", "maverick", ""),
        ("SHAD", "nighthawk", "black_hawk_cannon", ""),
        ("TERROR", "terrorist", "terror_bomb", ""),
        ("V3", "v3_launcher", "v3_launcher", "v3_rocket"),
        ("YURIPR", "yuri_prime", "super_mind_control", ""),
        // Ordinary scenario spawn is not an original playable/strategic mission claim.
        // The real mission/NotCommandable route is separately tested by actual client60.
        ("PDPLANE", "paradrop_plane", "para_drop_weapon", ""),
    ];
    let contracts = include_str!("ra9_audit/carry38-contracts.tsv");
    let actual: std::collections::BTreeSet<_> = contracts
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<_> = l.split('\t').collect();
            (c[4] == "idle-acquisition").then_some((c[1], c[2], c[3]))
        })
        .collect();
    let mut wanted: std::collections::BTreeSet<_> = cases
        .iter()
        .map(|c| (c.0, "CanPassiveAquire", "no"))
        .collect();
    wanted.insert(("SPY", "CanPassiveAquire", "no"));
    assert_eq!(
        actual, wanted,
        "exact11 literal census; no flag consumption"
    );
    for (section, name, primary, payload) in cases {
        for mode in ["Idle", "Attack", "MoveControl"] {
            let (mut app, source, at, pad) = ac_world(section, name);
            let expected = defs(&app).weapon_kind(primary).unwrap();
            assert_eq!(ac_weapon(&mut app, source).kind, expected);
            let source_hp = ac_hp(&mut app, source);
            let (target, _) = ac_target(&mut app, section, at);
            let target_hp = ac_hp(&mut app, target);
            let command = match mode {
                "Idle" => Command::Stop {
                    units: vec![source],
                },
                "Attack" => Command::Attack {
                    units: vec![source],
                    target,
                },
                _ => Command::Move {
                    units: vec![source],
                    target: CellPos::new(at.x, at.y + 10),
                },
            };
            let mut events = ac_tick(&mut app, vec![issue(P0, command)], Some(target));
            if mode == "MoveControl" {
                for _ in 0..3 {
                    ac_no_fire(&events, source);
                    assert_eq!(ac_weapon(&mut app, source).cooldown, 0);
                    assert_eq!(ac_hp(&mut app, source), source_hp);
                    assert_eq!(ac_hp(&mut app, target), target_hp);
                    events = ac_tick(&mut app, vec![], Some(target));
                }
                ac_no_fire(&events, source);
                continue;
            }
            let mut fired = false;
            let mut effect = false;
            let mut source_destroyed = false;
            for _ in 0..400 {
                fired |= events.iter().any(|e| {
                    matches!(e, SimEvent::WeaponFired {
                    attacker, target: found, weapon
                } if *attacker == source && *found == target && *weapon == expected)
                });
                source_destroyed |= events.iter().any(|e| {
                    matches!(e,
                    SimEvent::UnitDestroyed { unit, .. } if *unit == source)
                });
                if !payload.is_empty() {
                    for event in &events {
                        if let SimEvent::UnitLaunched {
                            player,
                            parent,
                            unit,
                        } = event
                            && *parent == source
                        {
                            assert_eq!(*player, P0);
                            let e = ac_entity(&mut app, *unit);
                            let kind = app.world().get::<Unit>(e).unwrap().kind;
                            assert_eq!(defs(&app).unit(kind).unwrap().id, payload);
                            assert_eq!(
                                app.world().resource::<Grid>().domain_of(*unit),
                                Domain::Air
                            );
                            assert_eq!(
                                app.world().resource::<SpecialState>().spawned.get(unit),
                                Some(&source)
                            );
                            effect = true;
                        }
                    }
                } else if section == "YURIPR" {
                    effect |= ac_owner(&mut app, target) == P0
                        && app
                            .world()
                            .resource::<SpecialState>()
                            .controlled
                            .get(&target)
                            .is_some_and(|(caster, original)| *caster == source && *original == P1);
                } else if matches!(section, "DTRUCK" | "TERROR") {
                    effect |= source_destroyed && ac_hp(&mut app, target) < target_hp;
                } else {
                    effect |= ac_hp(&mut app, target) < target_hp;
                }
                if fired && effect {
                    break;
                }
                events = ac_tick(&mut app, vec![], Some(target));
            }
            assert!(
                fired && effect,
                "{section}/{mode}: actual selected weapon plus launch/control/suicide/damage effect"
            );
            if !matches!(section, "DTRUCK" | "TERROR") {
                assert_eq!(
                    ac_hp(&mut app, source),
                    source_hp,
                    "controlled target caused no damage"
                );
                assert_eq!(ac_owner(&mut app, source), P0);
            }
            if let Some(pad) = pad {
                assert_eq!(
                    app.world().resource::<SpecialState>().docks.get(&source),
                    Some(&(pad, 0))
                );
                assert_eq!(
                    app.world().resource::<SpecialState>().ammo_of(source, 1),
                    0,
                    "actual full-ammo volley was consumed; no ammo setup injection"
                );
            }
        }
    }
}

#[test]
fn ra9_c38_spy_ordinary_idle_never_shoots_but_public_refinery_interact_steals() {
    let mut app = world();
    let spy = put(&mut app, P0, "spy", 10, 10);
    ac_tick(
        &mut app,
        vec![issue(P0, Command::Stop { units: vec![spy] })],
        None,
    );
    let refinery = put(&mut app, P1, "refinery", 11, 10);
    let initial_hp = ac_hp(&mut app, refinery);
    for _ in 0..20 {
        let events = ac_tick(&mut app, vec![], Some(refinery));
        ac_no_fire(&events, spy);
        assert_eq!(ac_weapon(&mut app, spy).cooldown, 0);
        assert!(ac_weapon(&mut app, spy).target.is_none());
        assert_eq!(ac_hp(&mut app, refinery), initial_hp);
        assert_eq!(ac_owner(&mut app, refinery), P1);
    }
    let before = app.world().resource::<Players>().get(P0).unwrap().credits();
    let enemy = app.world().resource::<Players>().get(P1).unwrap().credits();
    assert_eq!(enemy, 10_000);
    ac_tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: spy,
                target: refinery,
            },
        )],
        Some(refinery),
    );
    assert!(
        entity(&mut app, spy).is_none(),
        "real infiltration consumes spy"
    );
    assert_eq!(
        app.world().resource::<Players>().get(P1).unwrap().credits(),
        enemy / 2
    );
    assert_eq!(
        app.world().resource::<Players>().get(P0).unwrap().credits(),
        before + enemy / 2
    );
    assert_eq!(
        ac_hp(&mut app, refinery),
        initial_hp,
        "interaction, not invented damage/role"
    );
    assert_eq!(ac_owner(&mut app, refinery), P1);
}
