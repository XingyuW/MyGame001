//! Historical preparation note: UNCOMPILED / UNRUN. Proposed crates/sim/tests/ra9_guard75_readiness.rs.
//! Four bodies, three missing routes. Install only after root releases RA8 and refreshes pins.
//! Only shipped definitions, public commands and SimSchedule. promote is controlled elite setup.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    CellPos, Command, Grid, Health, Mobile, Owner, Players, Position, ProductionItem, Projectiles,
    Role, SimEvent, SimId, SimTick, SpecialState, SubPos, Unit, Weapon,
};

fn ok(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}
fn refusal(events: &[SimEvent]) {
    let reasons = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::CommandRejected { reason, .. } => Some(reason.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reasons.len(), 1, "{events:?}");
    assert!(
        reasons[0].contains("Ability requires power and a full charge"),
        "{reasons:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::SuperweaponLaunched { .. }))
    );
}
fn hp_opt(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<Health>(e).map(|h| h.hit_points)
}
fn hp(app: &mut App, id: SimId) -> i32 {
    hp_opt(app, id).expect("actual live actor")
}
fn pos(app: &mut App, id: SimId) -> SubPos {
    let e = entity(app, id).expect("actual live actor");
    app.world()
        .get::<Position>(e)
        .expect("unit fine position")
        .0
}
fn charge(app: &App, id: SimId) -> u32 {
    app.world()
        .resource::<SpecialState>()
        .charge
        .get(&id)
        .copied()
        .unwrap_or(0)
}
fn low(app: &App) -> bool {
    app.world()
        .resource::<Players>()
        .get(P0)
        .expect("player")
        .low_power()
}
fn launch(id: SimId, target: CellPos) -> sim::IssuedCommand {
    issue(
        P0,
        Command::Superweapon {
            building: id,
            target,
            units: vec![],
        },
    )
}
fn visible(app: &App, c: CellPos) {
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("player")
            .vision()
            .is_visible(c),
        "real scheduled sight must precede command"
    );
}
fn ready_event(events: &[SimEvent], role: Role) -> usize {
    events
        .iter()
        .filter(|e| matches!(e,SimEvent::SuperweaponReady{player:P0,role:r} if *r==role))
        .count()
}
fn launch_receipt(events: &[SimEvent], id: SimId, target: CellPos, role: Role) {
    ok(events);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,
        SimEvent::SuperweaponLaunched{player:P0,role:r} if *r==role))
            .count(),
        1,
        "{events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,
        SimEvent::StrategicActivated{player:P0,building,target:c,role:r,..}
        if *building==id&&*c==target&&*r==role))
            .count(),
        1,
        "{events:?}"
    );
}
/// Called after the first ordinary tick has registered/announced this source at charge zero.
/// No charge, clock, ready event or outage injection. The first attempted launch is genuinely
/// early with actual sight and power (or unpowered-ability exemption) already established.
fn natural_ready(app: &mut App, id: SimId, role: Role, cooldown: u32, target: CellPos) {
    assert_eq!(charge(app, id), 0);
    visible(app, target);
    let started = app.world().resource::<SimTick>().0;
    let mut ready = 0;
    for n in 1..=cooldown {
        let events = tick(
            app,
            if n == 1 {
                vec![launch(id, target)]
            } else {
                vec![]
            },
        );
        if n == 1 {
            refusal(&events);
        } else {
            ok(&events);
        }
        assert_eq!(charge(app, id), n, "one genuine charging tick");
        assert_eq!(
            app.world().resource::<SimTick>().0,
            started
                .checked_add(u64::from(n))
                .expect("bounded native readiness")
        );
        let count = ready_event(&events, role);
        assert_eq!(
            count,
            usize::from(n == cooldown),
            "edge-triggered readiness at full charge"
        );
        ready += count;
        assert!(
            app.world()
                .resource::<SpecialState>()
                .strategic
                .missiles
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<SpecialState>()
                .strategic
                .flights
                .is_empty()
        );
    }
    assert_eq!(ready, 1);
}
fn unit_ids(app: &mut App, name: &str) -> Vec<SimId> {
    let kind = defs(app).unit_kind(name).expect("kind");
    let w = app.world_mut();
    let mut ids = w
        .query::<(&SimId, &Unit, &Owner)>()
        .iter(w)
        .filter(|(_, u, o)| u.kind == kind && o.0 == P0)
        .map(|(id, _, _)| *id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}
fn america(powered: bool) {
    let mut app = world();
    assert_eq!(app.world().resource::<SimTick>().0, 0);
    let country = defs(&app)
        .rules()
        .expect("rules")
        .country_id("america")
        .expect("America");
    assert!(
        app.world_mut()
            .resource_mut::<Players>()
            .set_country(P0, Some(country))
    );
    if powered {
        put(&mut app, P0, "nuclear_reactor", 0, 0);
    }
    let source = put(&mut app, P0, "american_airforce_command", 12, 12);
    // Engineer gives real Sight4, without adding a GI to the payload count.
    put(&mut app, P0, "engineer", 20, 24);
    let target = CellPos::new(20, 20);
    let d = defs(&app);
    let k = d.building_kind("american_airforce_command").expect("kind");
    let m = d.metadata(ProductionItem::Building(k)).expect("metadata");
    assert_eq!(m.role, Role::Paradrop);
    assert!(m.strategic);
    assert!(!m.is_powered);
    assert_eq!(m.cooldown, 4800);
    assert_eq!(
        d.rules()
            .expect("rules")
            .superweapons
            .american_paradrop_count,
        8
    );
    assert!(unit_ids(&mut app, "gi").is_empty());
    let events = tick(&mut app, vec![]);
    ok(&events);
    assert_eq!(
        charge(&app, source),
        0,
        "normal initial source registration"
    );
    assert_eq!(low(&app), !powered, "actual economy power calculation");
    visible(&app, target);
    natural_ready(&mut app, source, Role::Paradrop, 4800, target);
    assert_eq!(low(&app), !powered);
    let events = tick(&mut app, vec![launch(source, target)]);
    launch_receipt(&events, source, target, Role::Paradrop);
    let state = &app.world().resource::<SpecialState>().strategic;
    assert_eq!(state.flights.len(), 1);
    let (&plane, f) = state.flights.iter().next().expect("real spawned plane");
    assert_eq!(f.owner, P0);
    assert_eq!(f.target, target);
    assert_eq!(f.count, 8);
    assert_eq!(f.infantry, d.unit_kind("gi").expect("GI"));
    assert_eq!(hp(&mut app, plane), 400);
    let start = pos(&mut app, plane);
    assert!(unit_ids(&mut app, "gi").is_empty());
    let mut travelled = false;
    for _ in 0..500 {
        if let Some(e) = entity(&mut app, plane) {
            let p = app.world().get::<Position>(e).expect("plane position").0;
            travelled |= p != start;
        }
        if app
            .world()
            .resource::<SpecialState>()
            .strategic
            .flights
            .is_empty()
        {
            break;
        }
        ok(&tick(&mut app, vec![]));
    }
    assert!(
        travelled,
        "actual scheduled moving plane, not direct infantry spawn"
    );
    assert!(entity(&mut app, plane).is_none());
    assert!(
        app.world()
            .resource::<SpecialState>()
            .strategic
            .flights
            .is_empty()
    );
    let ids = unit_ids(&mut app, "gi");
    assert_eq!(ids.len(), 8, "actual landed GI actors");
    for id in ids {
        let e = entity(&mut app, id).expect("landed actor");
        let c = app
            .world()
            .get::<Mobile>(e)
            .expect("ground registration")
            .cell;
        assert!(c.x.abs_diff(20).pow(2) + c.y.abs_diff(20).pow(2) <= 16);
        assert!(hp(&mut app, id) > 0);
        assert!(
            app.world()
                .resource::<Grid>()
                .holders(c)
                .into_iter()
                .flatten()
                .any(|held| held == id)
        );
    }
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
#[test]
fn ra9_guard75_america_natural_readiness_real_eight_gi_plane_in_power_and_low_power() {
    america(true);
    america(false);
}

fn aegis(elite: bool) {
    let mut app = world();
    assert_eq!(app.world().resource::<SimTick>().0, 0);
    for y in 0..48 {
        for x in 0..48 {
            app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                sim::Cell {
                    water: true,
                    ..OPEN
                },
            );
        }
    }
    let source = put(&mut app, P0, "aegis_cruiser", 8, 8);
    let target = put(&mut app, P1, "kirov", 15, 8);
    if elite {
        assert!(
            sim::promote(app.world_mut(), source, sim::ELITE),
            "public promote between ticks: controlled rank setup, not natural XP proof"
        );
    }
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Guard {
                    units: vec![source],
                },
            ),
            issue(
                P1,
                Command::Guard {
                    units: vec![target],
                },
            ),
        ],
    );
    ok(&events);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..} if *attacker==source))
    );
    assert!(app.world().resource::<Projectiles>().is_empty());
    assert_eq!(hp(&mut app, target), 2000);
    let from = pos(&mut app, source);
    let to = pos(&mut app, target);
    assert_eq!(from, SubPos::centre_of(CellPos::new(8, 8)));
    assert_eq!(to, SubPos::centre_of(CellPos::new(15, 8)));
    visible(&app, CellPos::new(15, 8));
    let d = defs(&app);
    let name = if elite { "medusa_e" } else { "medusa" };
    let wk = d.weapon_kind(name).expect("selected weapon");
    let w = d.weapon(wk).expect("weapon");
    assert_eq!(w.projectile_speed, 191);
    assert_eq!(w.damage, 100);
    assert!(w.anti_air);
    assert!(!w.anti_ground);
    assert_eq!(w.burst, if elite { 2 } else { 1 });
    assert_eq!(w.warhead, d.warhead_kind("samwh").expect("SAMWH"));
    let e = entity(&mut app, source).expect("source");
    assert_eq!(app.world().get::<Weapon>(e).expect("arms").kind, wk);
    let firepower = if elite { 1100 } else { 1000 };
    assert_eq!(
        app.world()
            .get::<sim::Experience>(e)
            .expect("rank terms")
            .firepower_pm(),
        firepower
    );
    assert_eq!(w.reload, if elite { 7 } else { 20 });
    let rank_reload = app
        .world()
        .get::<sim::Experience>(e)
        .expect("rank terms")
        .reload_pm();
    assert_eq!(rank_reload, if elite { 600 } else { 1000 });
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("player")
            .ai_reload_pm(),
        1000
    );
    // Independent fixed timeline: floor(7*600/1000)=4 after the second elite shot,
    // not5. First volley shots0/+5, next volley starts+9; the first impact is+10.
    let reload = u32::try_from(u64::from(w.reload) * u64::from(rank_reload) / 1000)
        .expect("small reload")
        .max(1);
    assert_eq!(reload, if elite { 4 } else { 20 });
    assert_eq!(sim::BURST_GAP, 5);
    let fired_at = app.world().resource::<SimTick>().0;
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![source],
                target,
            },
        )],
    );
    ok(&events);
    assert_eq!(
        events
            .iter()
            .filter(
                |e| matches!(e,SimEvent::WeaponFired{attacker,target:t,weapon}
        if *attacker==source&&*t==target&&*weapon==wk)
            )
            .count(),
        1,
        "{events:?}"
    );
    let first = app
        .world()
        .resource::<Projectiles>()
        .iter()
        .filter(|p| p.attacker == source && p.target == target)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(first.len(), 1);
    let first = first[0];
    assert_eq!(first.weapon, wk);
    assert!(first.air);
    assert!(!first.scattered);
    assert_eq!(first.origin, from);
    assert_eq!(first.aim, to);
    assert_eq!(first.firepower_pm, firepower);
    // For two real point extents, actual fine positions define the same gap used in Combat.
    let dx = i64::from(to.x) - i64::from(from.x);
    let dy = i64::from(to.y) - i64::from(from.y);
    let distance = (dx.unsigned_abs().pow(2) + dy.unsigned_abs().pow(2)).isqrt();
    assert_eq!(distance, 1792);
    let flight = u32::try_from(distance.div_ceil(191))
        .expect("small distance")
        .max(1);
    assert_eq!(flight, 10);
    assert_eq!((first.flight, first.remaining), (flight, flight));
    assert_eq!(hp(&mut app, target), 2000, "no launch-time damage");
    let mut second_salvo_launches = 0;
    for elapsed in 1..=flight {
        let events = tick(&mut app, vec![]);
        ok(&events);
        let launches = events
            .iter()
            .filter(|e| {
                matches!(e,SimEvent::WeaponFired{attacker,target:t,weapon}
            if *attacker==source&&*t==target&&*weapon==wk)
            })
            .count();
        assert_eq!(
            launches,
            usize::from(elite && (elapsed == 5 || elapsed == 9)),
            "exact real burst/next-volley timeline; no unasserted extra launch at another tick"
        );
        if elite && elapsed == 5 {
            assert_eq!(launches, 1, "second real elite burst shot at five ticks");
        }
        if !elite {
            assert_eq!(launches, 0, "base reload20 exceeds first flight");
        }
        let arms = app.world().get::<Weapon>(e).expect("actual firing weapon");
        let (cooldown, burst_left) = if !elite {
            (20 - elapsed, 0)
        } else if elapsed < 5 {
            (5 - elapsed, 1)
        } else if elapsed < 9 {
            (9 - elapsed, 0)
        } else {
            (14 - elapsed, 1)
        };
        assert_eq!(
            (arms.cooldown, arms.burst_left),
            (cooldown, burst_left),
            "actual per-tick cooldown/burst state accompanies the public shots"
        );
        let mut remaining = app
            .world()
            .resource::<Projectiles>()
            .iter()
            .filter(|p| p.attacker == source && p.target == target && p.weapon == wk)
            .map(|p| {
                assert_eq!(
                    (p.flight, p.origin, p.aim, p.firepower_pm),
                    (flight, from, to, firepower)
                );
                p.remaining
            })
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        if elapsed < 10 {
            expected.push(10 - elapsed);
        }
        if elite && elapsed >= 5 {
            expected.push(15 - elapsed);
        }
        if elite && elapsed >= 9 {
            expected.push(19 - elapsed);
        }
        remaining.sort_unstable();
        expected.sort_unstable();
        assert_eq!(
            remaining, expected,
            "every actually launched missile keeps its exact countdown"
        );
        second_salvo_launches += launches;
        assert_eq!(pos(&mut app, source), from);
        assert_eq!(pos(&mut app, target), to);
        assert_eq!(
            app.world().resource::<SimTick>().0,
            fired_at
                .checked_add(1 + u64::from(elapsed))
                .expect("bounded shot clock")
        );
        if elapsed < flight {
            assert_eq!(hp(&mut app, target), 2000, "no early impact");
            assert!(
                app.world()
                    .resource::<Projectiles>()
                    .iter()
                    .any(|p| p.attacker == source
                        && p.target == target
                        && p.weapon == wk
                        && p.remaining == flight - elapsed
                        && p.flight == flight
                        && p.origin == from
                        && p.aim == to)
            );
        } else {
            assert_eq!(
                hp(&mut app, target),
                if elite { 1890 } else { 1900 },
                "independent first selected100 damage, SAMWH light100%, rank1000/1100"
            );
            assert!(
                !app.world()
                    .resource::<Projectiles>()
                    .iter()
                    .any(|p| p.attacker == source && p.target == target && p.remaining == 0)
            );
        }
    }
    assert_eq!(
        second_salvo_launches,
        if elite { 2 } else { 0 },
        "elite adds the +5 burst shot and +9 next-volley shot during the first10-tick flight"
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
#[test]
fn ra9_guard75_aegis_base_medusa_191_actual_position_flight_countdown_and_impact() {
    aegis(false);
}
#[test]
fn ra9_guard75_aegis_elite_medusa_e_191_actual_burst_flight_and_impact() {
    aegis(true);
}

#[test]
fn ra9_guard75_stock_nuke_natural_power_readiness_and_actual_600_surviving_hit() {
    let mut app = world();
    assert_eq!(app.world().resource::<SimTick>().0, 0);
    let source = put(&mut app, P0, "nuclear_silo", 3, 3);
    let sight = CellPos::new(6, 6);
    let d = defs(&app);
    let sk = d.building_kind("nuclear_silo").expect("silo");
    let m = d
        .metadata(ProductionItem::Building(sk))
        .expect("silo metadata");
    assert!(m.strategic && m.is_powered);
    assert_eq!(m.role, Role::Nuke);
    assert_eq!(m.cooldown, 12000);
    assert_eq!(d.rules().expect("rules").superweapons.nuclear_flight, 200);
    let payload = d
        .weapon(d.weapon_kind("nuke_payload").expect("payload"))
        .expect("weapon");
    assert_eq!(payload.damage, 600);
    assert_eq!(payload.rad_level, 500);
    assert_eq!(payload.warhead, d.warhead_kind("nuke").expect("warhead"));
    ok(&tick(&mut app, vec![]));
    assert!(low(&app));
    visible(&app, sight);
    assert_eq!(charge(&app, source), 0);
    // Genuine low-power stall. This is not yet an isolated full-charge launch refusal.
    for _ in 0..23 {
        let events = tick(&mut app, vec![]);
        ok(&events);
        assert!(low(&app));
        assert_eq!(charge(&app, source), 0);
        assert_eq!(ready_event(&events, Role::Nuke), 0);
    }
    let reactor = put(&mut app, P0, "nuclear_reactor", 0, 0);
    ok(&tick(&mut app, vec![]));
    assert!(!low(&app));
    // The actual restoration tick legitimately charges one; do not erase it.
    assert_eq!(charge(&app, source), 1);
    let events = tick(&mut app, vec![launch(source, sight)]);
    refusal(&events);
    assert_eq!(charge(&app, source), 2);
    let start = app.world().resource::<SimTick>().0;
    for n in 3..=12000 {
        let events = tick(&mut app, vec![]);
        ok(&events);
        assert_eq!(charge(&app, source), n);
        assert_eq!(ready_event(&events, Role::Nuke), usize::from(n == 12000));
    }
    assert_eq!(app.world().resource::<SimTick>().0, start + 11998);
    // Full charge plus visible target, but public sale removes the real power producer.
    let sale = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: reactor })],
    );
    ok(&sale);
    assert!(
        sale.iter()
            .any(|event| matches!(event,SimEvent::BuildingSold{building,..}if *building==reactor))
    );
    assert!(
        sale.iter()
            .any(|event| matches!(event,SimEvent::CrewEscaped{from,..}if *from==reactor)),
        "actual sold crew remains in the old footprint; do not delete it for a setup placement"
    );
    assert!(entity(&mut app, reactor).is_none());
    assert!(low(&app));
    assert_eq!(charge(&app, source), 12000);
    visible(&app, sight);
    let events = tick(&mut app, vec![launch(source, sight)]);
    refusal(&events);
    assert_eq!(charge(&app, source), 12000);
    assert!(
        app.world()
            .resource::<SpecialState>()
            .strategic
            .missiles
            .is_empty()
    );
    // The sale legitimately leaves crew at(0,0). Restore power with another legal
    // reactor footprint; do not overwrite those actors or bypass occupancy.
    put(&mut app, P0, "nuclear_reactor", 0, 5);
    put(&mut app, P0, "spy_satellite", 3, 8);
    // Stock800 HP / heavy100% target survives600. Legacy General.AtomDamage1000 would kill.
    // Full stock Health, armour, position and self-heal; no direct HP/weapon edits or fixture.
    // Stock600 NUKE has wall=false. These stock300HP walls are measured for eligibility,
    // not destruction. No high-HP/retuned NUKE fixture is substituted for this shipped gate.
    let near_wall = put(&mut app, P1, "fortress_walls", 21, 20);
    let far_wall = put(&mut app, P1, "fortress_walls", 31, 20);
    let wood = put(&mut app, P1, "civilian_house", 23, 20);
    let air = put(&mut app, P1, "kirov", 20, 20);
    let nuke = d.warhead(payload.warhead).expect("NUKE");
    assert_eq!((nuke.cell_spread, nuke.percent_at_max), (2560, 2));
    assert!(!nuke.wall && nuke.wood);
    let wall_terms = d
        .building_combat(d.building_kind("fortress_walls").expect("wall"))
        .expect("terms");
    assert!(wall_terms.wall);
    assert_eq!(wall_terms.hit_points, 300);
    // Concrete8% gives at most48 bounded damage per cell even if wall eligibility changed.
    let bounded_wall_damage = payload.damage * 8 / 100;
    assert_eq!(bounded_wall_damage, 48);
    assert!(wall_terms.hit_points > bounded_wall_damage);
    let tree_cell = CellPos::new(24, 20);
    assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
        tree_cell,
        sim::Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        }
    ));
    let mut looks = vec![0; 48 * 48];
    looks[20 * 48 + 24] = 4 << 4;
    let props = sim::Props::from_looks(
        app.world().resource::<Grid>(),
        &looks,
        d.armour_class("wood").expect("wood"),
    );
    let tree = props
        .id_at(tree_cell)
        .expect("real original primitive tree");
    assert_eq!(props.get(tree).expect("tree").1.hit_points, 60);
    app.world_mut().insert_resource(props);
    let target = put(&mut app, P1, "apocalypse", 20, 20);
    let outside = put(&mut app, P1, "apocalypse", 35, 35);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Guard {
                units: vec![target, outside],
            },
        )],
    );
    ok(&events);
    assert!(!low(&app));
    assert_eq!(charge(&app, source), 12000);
    let tc = d
        .unit_combat(d.unit_kind("apocalypse").expect("APOC"))
        .expect("terms");
    assert_eq!(tc.hit_points, 800);
    assert_eq!(tc.armour, d.armour_class("heavy").expect("heavy"));
    assert_eq!(
        sim::versus_against(
            d.warhead(payload.warhead).expect("NUKE"),
            &sim::TargetTraits {
                armour: tc.armour,
                ..Default::default()
            }
        ),
        100
    );
    assert_eq!(hp(&mut app, target), 800);
    assert_eq!(hp(&mut app, outside), 800);
    let target_cell = CellPos::new(20, 20);
    let point = SubPos::centre_of(target_cell);
    assert_eq!(pos(&mut app, target), point);
    visible(&app, target_cell);
    let launched = app.world().resource::<SimTick>().0;
    let events = tick(&mut app, vec![launch(source, target_cell)]);
    launch_receipt(&events, source, target_cell, Role::Nuke);
    assert_eq!(hp(&mut app, target), 800);
    let missiles = &app.world().resource::<SpecialState>().strategic.missiles;
    assert_eq!(missiles.len(), 1);
    assert_eq!(
        (missiles[0].source, missiles[0].owner, missiles[0].target),
        (source, P0, target_cell)
    );
    let due = launched.checked_add(200).expect("small flight");
    assert_eq!(missiles[0].due, due);
    assert!(
        app.world()
            .resource::<SpecialState>()
            .radiation_sites
            .is_empty()
    );
    while app.world().resource::<SimTick>().0 < due {
        let events = tick(&mut app, vec![]);
        ok(&events);
        assert_eq!(
            hp(&mut app, target),
            800,
            "no payload damage before due special stage"
        );
        assert_eq!(hp(&mut app, outside), 800);
        assert_eq!(pos(&mut app, target), point);
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .strategic
                .missiles
                .len(),
            1
        );
    }
    assert_eq!(app.world().resource::<SimTick>().0, due);
    let events = tick(&mut app, vec![]);
    ok(&events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::Impact{warhead,at,air}
        if *warhead==payload.warhead&&*at==point&&!*air)),
        "actual payload impact: {events:?}"
    );
    assert_eq!(hp(&mut app, outside), 800, "outside10-cell blast control");
    assert!(
        app.world()
            .resource::<SpecialState>()
            .strategic
            .missiles
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .radiation_sites
            .iter()
            .any(|s| s.cell == target_cell
                && s.level == 500
                && s.born == due
                && s.source == Some(source))
    );
    assert_eq!(
        hp_opt(&mut app, target),
        Some(200),
        "independent stock800 minus actual600 at centre/heavy100%; legacy1000 kills"
    );
    assert_eq!(
        (hp(&mut app, near_wall), hp(&mut app, far_wall)),
        (300, 300),
        "stock NUKE Wall=false excludes walls, regardless of absolute-destroyer source literal"
    );
    // Wood60%, distance3*256, radius2560, edge2%: floor423.6*0.6 =254,
    // evaluated in one exact integer division, NOT by rounding an intermediate damage.
    let wood_damage = 600_i64 * (100 * 2560 - 98 * 768) * 60 / (100 * 2560 * 100);
    assert_eq!(wood_damage, 254);
    assert_eq!(hp(&mut app, wood), 300 - 254);
    assert_eq!(
        hp(&mut app, air),
        2000,
        "ground nuclear payload excludes actual air-layer actor"
    );
    assert!(
        app.world().resource::<sim::Props>().get(tree).is_none(),
        "NUKE Wood=true reaches real tree"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::PropDestroyed{prop,..}if *prop==tree))
    );
    // Radiation advance precedes strategic payload in this special stage. Stop here: a later
    // radiation application or real self-heal must not be mislabeled part of this600 impact.
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
