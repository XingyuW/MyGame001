//! Actual command/schedule production and specialist paths, using the legacy ruleset fixture (the
//! data shipped before the RA2 parity programme, `tests/fixtures/data/legacy`; docs/DECISIONS.md
//! R15).
use bevy_app::App;
use bevy_ecs::{entity::Entity, message::Messages};
use sim::*;
use std::path::PathBuf;
const P: PlayerId = PlayerId(0);
const E: PlayerId = PlayerId(1);
fn defs() -> Definitions {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
    load_match_definitions(
        p.join("units.ron"),
        p.join("economy.ron"),
        p.join("combat.ron"),
    )
    .unwrap()
    .with_arsenal(p.join("arsenal.ron"))
    .unwrap()
}
fn app() -> App {
    let mut a = App::new();
    a.add_plugins(SimPlugin { seed: 5 });
    let mut grid =
        load_map(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo/demo.tmx"))
            .unwrap()
            .grid;
    for y in 0..64 {
        for x in 0..64 {
            grid.set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: x < 48,
                    buildable: x < 48,
                    water: x >= 48,
                    ore: 0,
                },
            );
        }
    }
    a.world_mut().insert_resource(grid);
    a.world_mut().insert_resource(defs());
    for p in [P, E] {
        a.world_mut()
            .resource_mut::<Players>()
            .add(p, 1_000_000)
            .unwrap();
    }
    a
}
fn tick(a: &mut App, n: u32) {
    for _ in 0..n {
        a.world_mut().run_schedule(SimSchedule);
    }
}
fn cmd(a: &mut App, c: Command) {
    a.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand {
            player: P,
            command: c,
        });
    tick(a, 1);
}
fn b(a: &mut App, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(a.world_mut(), P, id, CellPos::new(x, y)).unwrap()
}
fn u(a: &mut App, p: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_unit(a.world_mut(), p, id, CellPos::new(x, y)).unwrap()
}
fn entity(a: &mut App, id: SimId) -> Entity {
    a.world_mut()
        .query::<(Entity, &SimId)>()
        .iter(a.world())
        .find(|(_, i)| **i == id)
        .unwrap()
        .0
}
fn cell(a: &mut App, id: SimId) -> CellPos {
    let e = entity(a, id);
    a.world().get::<Mobile>(e).unwrap().cell
}
fn hp(a: &mut App, id: SimId) -> i32 {
    let e = entity(a, id);
    a.world().get::<Health>(e).unwrap().hit_points
}
fn rejected(a: &App) -> bool {
    a.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
}
fn infrastructure(a: &mut App) {
    for (i, id) in [
        "construction_yard",
        "power_plant",
        "refinery",
        "barracks",
        "war_factory",
        "radar_tower",
        "tech_center",
        "airforce_command",
        "service_depot",
        "nuclear_reactor",
        "spy_satellite",
    ]
    .iter()
    .enumerate()
    {
        b(a, id, 2 + (i % 5) as i32 * 8, 2 + (i / 5) as i32 * 8);
    }
    b(a, "naval_shipyard", 48, 3);
    tick(a, 2);
}
fn item(a: &App, id: &str) -> ProductionItem {
    let d = a.world().resource::<Definitions>();
    d.building_kind(id)
        .map(ProductionItem::Building)
        .or_else(|| d.unit_kind(id).map(ProductionItem::Unit))
        .unwrap()
}
fn credits(a: &App) -> i64 {
    a.world().resource::<Players>().get(P).unwrap().credits()
}

#[test]
fn exact_unified_roster_and_categories() {
    let d = defs();
    let mut counts = [0; 4];
    for (i, m) in &d.arsenal().0 {
        counts[m.category as usize] += 1;
        let p = d.production(*i).unwrap();
        assert!(p.cost > 0 && p.build_time > 0);
        assert!(d.building(p.producer).is_some());
    }
    assert_eq!(counts, [14, 15, 15, 28]);
    for id in [
        "apocalypse",
        "chrono_legionnaire",
        "prism_tank",
        "aircraft_carrier",
        "nuclear_silo",
        "chronosphere",
    ] {
        let i = d
            .unit_kind(id)
            .map(ProductionItem::Unit)
            .or_else(|| d.building_kind(id).map(ProductionItem::Building))
            .unwrap();
        assert!(
            d.production(i)
                .unwrap()
                .prerequisites
                .contains(&d.building_kind("tech_center").unwrap())
        );
    }
    assert!(d.unit_kind("yuri").is_some());
    assert!(d.unit_kind("magnetron").is_none());
}

#[test]
fn every_item_executes_queue_cost_completion_and_placement_or_spawn() {
    verify_catalogue(false);
}
#[test]
fn super_me_every_item_costs_one_and_finishes_in_tenth_time() {
    verify_catalogue(true);
}
fn verify_catalogue(super_me: bool) {
    let mut a = app();
    if super_me {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
        let d = a.world().resource::<Definitions>().clone();
        let mut sk = load_skirmish(root.join("skirmish.ron"), &d).unwrap();
        let ai = load_ai(root.join("ai.ron"), &d).unwrap();
        for faction in &mut sk.factions {
            faction.buildings.clear();
            faction.units.clear();
        }
        sk.maps[0].starts = vec![CellPos::new(3, 3), CellPos::new(35, 35)];
        sk.starting_credits = 1_000_000;
        a.world_mut().insert_resource(Players::default());
        setup_match_with_super_me(
            a.world_mut(),
            &sk,
            &ai,
            0,
            &[
                PlayerSetup {
                    player: P,
                    faction: 0,
                    start: 0,
                    controller: Controller::Human,
                },
                PlayerSetup {
                    player: E,
                    faction: 0,
                    start: 1,
                    controller: Controller::Human,
                },
            ],
            &[P],
        )
        .unwrap();
        assert!(!a.world().resource::<Players>().get(E).unwrap().super_me());
    }
    infrastructure(&mut a);
    let d = a.world().resource::<Definitions>().clone();
    let list: Vec<_> = d.arsenal().0.keys().copied().collect();
    for i in list {
        let terms = d.production(i).unwrap();
        let before = credits(&a);
        cmd(&mut a, Command::QueueProduction { item: i });
        assert!(!rejected(&a), "queue {i:?}");
        let duration = terms.build_time.div_ceil(if super_me { 10 } else { 1 });
        if duration > 1 {
            tick(&mut a, duration - 2);
            assert!(
                !a.world()
                    .resource::<Players>()
                    .get(P)
                    .unwrap()
                    .queue(terms.producer)
                    .is_empty(),
                "premature completion {i:?}"
            );
            tick(&mut a, 1);
        }
        assert_eq!(
            before - credits(&a),
            if super_me { 1 } else { terms.cost },
            "cost {i:?}"
        );
        assert_eq!(
            a.world()
                .resource::<Definitions>()
                .production(i)
                .unwrap()
                .cost,
            terms.cost
        );
        assert_eq!(
            a.world()
                .resource::<Players>()
                .get(E)
                .unwrap()
                .production_cost(terms.cost),
            terms.cost
        );
        match i {
            ProductionItem::Unit(k) => {
                assert!(
                    a.world_mut()
                        .query::<&Unit>()
                        .iter(a.world())
                        .any(|u| u.kind == k),
                    "spawn {}",
                    d.unit(k).unwrap().id
                );
                assert!(
                    a.world()
                        .resource::<Players>()
                        .get(P)
                        .unwrap()
                        .queue(terms.producer)
                        .is_empty()
                );
                let ids: Vec<_> = a
                    .world_mut()
                    .query::<(&SimId, &Unit)>()
                    .iter(a.world())
                    .filter(|(_, u)| u.kind == k)
                    .map(|(id, _)| *id)
                    .collect();
                let target = if d.domain(k) == Domain::Naval || d.domain(k) == Domain::Amphibious {
                    CellPos::new(
                        54 + (i32::from(k.0) % 3) * 3,
                        20 + (i32::from(k.0) % 10) * 3,
                    )
                } else {
                    CellPos::new(
                        4 + (i32::from(k.0) % 12) * 3,
                        35 + (i32::from(k.0) / 12) * 4,
                    )
                };
                cmd(&mut a, Command::Move { units: ids, target });
            }
            ProductionItem::Building(k) => {
                let owned: Vec<_> = a
                    .world_mut()
                    .query::<(&Owner, &Building)>()
                    .iter(a.world())
                    .filter(|(o, _)| o.0 == P)
                    .map(|(_, b)| (b.kind, b.footprint))
                    .collect();
                let g = a.world().resource::<Grid>();
                let state = a.world().resource::<Players>().get(P).unwrap();
                let pos = (0..64)
                    .flat_map(|y| (0..64).map(move |x| CellPos::new(x, y)))
                    .find(|&c| check_placement(g, &d, state, owned.iter().copied(), k, c).is_ok())
                    .unwrap_or_else(|| panic!("no placement for {}", d.building(k).unwrap().id));
                cmd(
                    &mut a,
                    Command::PlaceBuilding {
                        building: k,
                        origin: pos,
                    },
                );
                assert!(!rejected(&a), "place {i:?}");
                let id = a.world().resource::<Grid>().building(pos).unwrap();
                cmd(&mut a, Command::SellBuilding { building: id });
                assert!(!rejected(&a));
            }
        }
    }
}
#[test]
fn prerequisites_refunds_ready_cancellation_and_low_power() {
    let mut a = app();
    b(&mut a, "construction_yard", 2, 2);
    tick(&mut a, 1);
    let advanced = item(&a, "apocalypse");
    cmd(&mut a, Command::QueueProduction { item: advanced });
    assert!(rejected(&a));
    assert_eq!(credits(&a), 1_000_000);
    let power = item(&a, "power_plant");
    cmd(&mut a, Command::QueueProduction { item: power });
    tick(&mut a, 49);
    assert!(credits(&a) < 1_000_000);
    cmd(&mut a, Command::CancelProduction { item: power });
    assert_eq!(credits(&a), 1_000_000);
    cmd(&mut a, Command::CancelProduction { item: power });
    assert!(rejected(&a));
    assert_eq!(credits(&a), 1_000_000);
    cmd(&mut a, Command::QueueProduction { item: power });
    tick(&mut a, 199);
    cmd(&mut a, Command::CancelProduction { item: power });
    assert_eq!(credits(&a), 1_000_000);
    b(&mut a, "barracks", 8, 2);
    b(&mut a, "war_factory", 14, 2);
    tick(&mut a, 1);
    assert!(a.world().resource::<Players>().get(P).unwrap().low_power());
    cmd(&mut a, Command::QueueProduction { item: power });
    tick(&mut a, 99);
    let d = a.world().resource::<Definitions>();
    let p = d.production(power).unwrap();
    assert_eq!(
        a.world()
            .resource::<Players>()
            .get(P)
            .unwrap()
            .queue(p.producer)[0]
            .progress,
        50
    );
}
#[test]
fn shoreline_and_domain_navigation_are_enforced() {
    let mut a = app();
    assert!(spawn_building(a.world_mut(), P, "naval_shipyard", CellPos::new(10, 10)).is_err());
    assert!(spawn_building(a.world_mut(), P, "naval_shipyard", CellPos::new(52, 10)).is_err());
    b(&mut a, "naval_shipyard", 48, 3);
    assert!(spawn_unit(a.world_mut(), P, "destroyer", CellPos::new(5, 5)).is_err());
    let ship = u(&mut a, P, "destroyer", 52, 8);
    let plane = u(&mut a, P, "harrier", 4, 8);
    let tank = u(&mut a, P, "grizzly", 4, 12);
    tick(&mut a, 1);
    cmd(
        &mut a,
        Command::Move {
            units: vec![ship],
            target: CellPos::new(52, 18),
        },
    );
    cmd(
        &mut a,
        Command::Move {
            units: vec![plane],
            target: CellPos::new(54, 12),
        },
    );
    cmd(
        &mut a,
        Command::Move {
            units: vec![tank],
            target: CellPos::new(52, 12),
        },
    );
    tick(&mut a, 400);
    assert_eq!(cell(&mut a, ship), CellPos::new(52, 18));
    assert_eq!(cell(&mut a, plane), CellPos::new(54, 12));
    assert!(cell(&mut a, tank).x < 48);
    cmd(
        &mut a,
        Command::Move {
            units: vec![ship],
            target: CellPos::new(10, 18),
        },
    );
    tick(&mut a, 400);
    assert!(cell(&mut a, ship).x >= 48);
}
#[test]
fn mcv_deploys_and_new_yard_produces() {
    let mut a = app();
    let m = u(&mut a, P, "mcv", 10, 10);
    tick(&mut a, 2);
    cmd(&mut a, Command::Deploy { unit: m });
    assert!(!rejected(&a));
    let e = entity(&mut a, m);
    assert!(a.world().get::<Unit>(e).is_none());
    assert!(a.world().get::<Building>(e).is_some());
    let power = item(&a, "power_plant");
    cmd(&mut a, Command::QueueProduction { item: power });
    tick(&mut a, 199);
    assert_eq!(credits(&a), 999700);
}
#[test]
fn collectors_deliver_and_chrono_miner_teleports() {
    let mut a = app();
    b(&mut a, "refinery", 3, 3);
    b(&mut a, "nuclear_reactor", 8, 3);
    let miner = u(&mut a, P, "chrono_miner", 25, 25);
    let war = u(&mut a, P, "war_miner", 27, 25);
    for x in 25..=27 {
        a.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(x, 25),
            Cell {
                passable: true,
                buildable: false,
                water: false,
                ore: 3000,
            },
        );
    }
    tick(&mut a, 100);
    assert!(cell(&mut a, miner).x < 10);
    tick(&mut a, 1200);
    assert!(credits(&a) > 1_000_000);
    let e = entity(&mut a, war);
    assert!(a.world().get::<Weapon>(e).is_some());
}
#[test]
fn anti_air_defence_ignores_ground_and_stops_without_power() {
    let mut a = app();
    b(&mut a, "power_plant", 2, 2);
    b(&mut a, "patriot", 10, 10);
    let ground = u(&mut a, E, "rhino", 12, 10);
    let air = u(&mut a, E, "kirov", 12, 12);
    tick(&mut a, 30);
    assert_eq!(hp(&mut a, ground), 500);
    assert!(hp(&mut a, air) < 2000);
}
#[test]
fn transport_ifv_and_engineer_commands_execute() {
    let mut a = app();
    let ifv = u(&mut a, P, "ifv", 10, 10);
    let gi = u(&mut a, P, "gi", 11, 10);
    let eng = u(&mut a, P, "engineer", 20, 20);
    let enemy = spawn_building(a.world_mut(), E, "power_plant", CellPos::new(21, 20)).unwrap();
    tick(&mut a, 2);
    cmd(
        &mut a,
        Command::Interact {
            unit: gi,
            target: ifv,
        },
    );
    assert!(!rejected(&a));
    assert_eq!(a.world().resource::<SpecialState>().cargo[&ifv].len(), 1);
    cmd(&mut a, Command::Deploy { unit: ifv });
    assert!(!rejected(&a));
    assert!(a.world().resource::<SpecialState>().cargo[&ifv].is_empty());
    assert_eq!(hp(&mut a, gi), 125);
    cmd(
        &mut a,
        Command::Interact {
            unit: eng,
            target: enemy,
        },
    );
    assert!(!rejected(&a));
    let e = entity(&mut a, enemy);
    assert_eq!(a.world().get::<Owner>(e).unwrap().0, P);
}
#[test]
fn superweapons_charge_activate_and_reset_without_resource_duplication() {
    let mut a = app();
    infrastructure(&mut a);
    let mut sw = Vec::new();
    for (i, name) in [
        "chronosphere",
        "weather_control",
        "iron_curtain",
        "nuclear_silo",
    ]
    .iter()
    .enumerate()
    {
        sw.push(b(&mut a, name, 3 + i as i32 * 7, 27));
    }
    let tank = u(&mut a, P, "grizzly", 20, 40);
    let enemy = u(&mut a, E, "rhino", 35, 40);
    tick(&mut a, 2);
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[3],
            target: CellPos::new(35, 40),
            units: vec![],
        },
    );
    assert!(rejected(&a));
    tick(&mut a, 6000);
    let before = credits(&a);
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[0],
            target: CellPos::new(25, 40),
            units: vec![tank],
        },
    );
    assert!(!rejected(&a));
    assert_eq!(cell(&mut a, tank), CellPos::new(25, 40));
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[2],
            target: CellPos::new(25, 40),
            units: vec![],
        },
    );
    assert!(!rejected(&a));
    assert!(
        a.world()
            .resource::<SpecialState>()
            .protected
            .contains_key(&tank)
    );
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[1],
            target: CellPos::new(40, 40),
            units: vec![],
        },
    );
    assert!(!rejected(&a));
    assert!(!a.world().resource::<SpecialState>().zones.is_empty());
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[3],
            target: CellPos::new(35, 40),
            units: vec![],
        },
    );
    assert!(!rejected(&a));
    assert!(
        !a.world_mut()
            .query::<&SimId>()
            .iter(a.world())
            .any(|id| *id == enemy)
    );
    assert_eq!(credits(&a), before);
    assert!(a.world().resource::<SpecialState>().charge[&sw[3]] < 10);
    cmd(
        &mut a,
        Command::Superweapon {
            building: sw[3],
            target: CellPos::new(35, 40),
            units: vec![],
        },
    );
    assert!(rejected(&a));
}

#[test]
fn cloning_produces_one_extra_infantry_without_recursive_clones() {
    let mut a = app();
    infrastructure(&mut a);
    b(&mut a, "cloning_vats", 3, 27);
    let gi = item(&a, "gi");
    let before = credits(&a);
    cmd(&mut a, Command::QueueProduction { item: gi });
    tick(&mut a, 100);
    let k = a.world().resource::<Definitions>().unit_kind("gi").unwrap();
    assert_eq!(
        a.world_mut()
            .query::<&Unit>()
            .iter(a.world())
            .filter(|u| u.kind == k)
            .count(),
        2
    );
    assert_eq!(before - credits(&a), 200);
    tick(&mut a, 200);
    assert_eq!(
        a.world_mut()
            .query::<&Unit>()
            .iter(a.world())
            .filter(|u| u.kind == k)
            .count(),
        2
    );
}
/// The cloning vats take a rally point like the barracks: the copy walks to it, while the
/// soldier trained at the barracks (which has none here) stays beside the barracks.
#[test]
fn a_clone_goes_to_the_rally_point_of_the_cloning_vats() {
    let mut a = app();
    infrastructure(&mut a);
    let vats = b(&mut a, "cloning_vats", 3, 27);
    let rally = CellPos::new(20, 40);
    cmd(
        &mut a,
        Command::SetRallyPoint {
            building: vats,
            cell: rally,
        },
    );
    assert!(!rejected(&a), "the cloning vats accept a rally point");
    let gi = item(&a, "gi");
    cmd(&mut a, Command::QueueProduction { item: gi });
    tick(&mut a, 700);
    let k = a.world().resource::<Definitions>().unit_kind("gi").unwrap();
    let cells: Vec<CellPos> = a
        .world_mut()
        .query::<(&Unit, &Mobile)>()
        .iter(a.world())
        .filter(|(u, _)| u.kind == k)
        .map(|(_, m)| m.cell)
        .collect();
    assert_eq!(cells.len(), 2);
    let near = |c: &CellPos| (c.x - rally.x).abs() <= 1 && (c.y - rally.y).abs() <= 1;
    assert_eq!(cells.iter().filter(|c| near(c)).count(), 1, "{cells:?}");
}
#[test]
fn aircraft_strike_returns_to_air_facility_and_rearms() {
    let mut a = app();
    b(&mut a, "airforce_command", 4, 4);
    b(&mut a, "nuclear_reactor", 8, 4);
    let plane = u(&mut a, P, "harrier", 20, 20);
    let target =
        spawn_building(a.world_mut(), E, "construction_yard", CellPos::new(22, 20)).unwrap();
    tick(&mut a, 2);
    cmd(
        &mut a,
        Command::Attack {
            units: vec![plane],
            target,
        },
    );
    assert!(hp(&mut a, target) < 1100);
    assert!(
        a.world()
            .resource::<SpecialState>()
            .rearming
            .contains_key(&plane)
    );
    tick(&mut a, 300);
    assert!(cell(&mut a, plane).x < 10);
    assert!(
        !a.world()
            .resource::<SpecialState>()
            .rearming
            .contains_key(&plane)
    );
}
#[test]
fn mind_control_reverts_when_controller_is_destroyed() {
    let mut a = app();
    let y = u(&mut a, P, "yuri", 10, 10);
    let victim = u(&mut a, E, "rhino", 13, 10);
    tick(&mut a, 2);
    let e = entity(&mut a, victim);
    assert_eq!(a.world().get::<Owner>(e).unwrap().0, P);
    let enemy = u(&mut a, E, "sniper", 10, 12);
    tick(&mut a, 110);
    assert!(
        !a.world_mut()
            .query::<&SimId>()
            .iter(a.world())
            .any(|i| *i == y)
    );
    assert_eq!(a.world().get::<Owner>(e).unwrap().0, E);
    assert!(hp(&mut a, enemy) > 0);
}
#[test]
fn spy_infiltration_steals_refinery_credits_and_dogs_reveal_spies() {
    let mut a = app();
    let spy = u(&mut a, P, "spy", 10, 10);
    let refi = spawn_building(a.world_mut(), E, "refinery", CellPos::new(11, 10)).unwrap();
    let hidden_spy = u(&mut a, E, "spy", 22, 20);
    u(&mut a, P, "gi", 17, 20);
    tick(&mut a, 2);
    assert!(
        a.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P, hidden_spy))
    );
    u(&mut a, P, "attack_dog", 18, 20);
    tick(&mut a, 1);
    assert!(
        !a.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P, hidden_spy))
    );
    cmd(
        &mut a,
        Command::Interact {
            unit: spy,
            target: refi,
        },
    );
    assert!(!rejected(&a));
    assert_eq!(credits(&a), 1_500_000);
    assert_eq!(
        a.world().resource::<Players>().get(E).unwrap().credits(),
        500_000
    );
}
#[test]
fn timed_explosives_radiation_and_vehicle_disable_have_effects() {
    let mut a = app();
    let ivan = u(&mut a, P, "crazy_ivan", 10, 10);
    let target =
        spawn_building(a.world_mut(), E, "construction_yard", CellPos::new(11, 10)).unwrap();
    let drone = u(&mut a, P, "terror_drone", 20, 20);
    let tank = u(&mut a, E, "rhino", 21, 20);
    let des = u(&mut a, P, "desolator", 30, 30);
    let before = hp(&mut a, target);
    tick(&mut a, 2);
    assert!(
        a.world()
            .resource::<SpecialState>()
            .bombs
            .contains_key(&target)
    );
    assert!(
        a.world()
            .resource::<SpecialState>()
            .disabled
            .contains_key(&tank)
    );
    cmd(&mut a, Command::Deploy { unit: des });
    assert!(!rejected(&a));
    assert!(!a.world().resource::<SpecialState>().zones.is_empty());
    tick(&mut a, 100);
    assert!(hp(&mut a, target) < before - 100);
    let _ = (ivan, drone);
}
#[test]
fn lost_prerequisites_pause_queued_progress_and_refund_exactly() {
    let mut a = app();
    infrastructure(&mut a);
    let lab = a
        .world_mut()
        .query::<(&SimId, &Building)>()
        .iter(a.world())
        .find(|(_, b)| {
            b.kind
                == a.world()
                    .resource::<Definitions>()
                    .building_kind("tech_center")
                    .unwrap()
        })
        .unwrap()
        .0
        .to_owned();
    let i = item(&a, "apocalypse");
    let before = credits(&a);
    cmd(&mut a, Command::QueueProduction { item: i });
    tick(&mut a, 20);
    cmd(&mut a, Command::SellBuilding { building: lab });
    let paid = credits(&a);
    tick(&mut a, 100);
    assert_eq!(credits(&a), paid);
    cmd(&mut a, Command::CancelProduction { item: i });
    assert_eq!(credits(&a), before + 750);
}
#[test]
fn deterministic_extended_state_includes_abilities() {
    fn run() -> u64 {
        let mut a = app();
        infrastructure(&mut a);
        let des = u(&mut a, P, "desolator", 30, 30);
        tick(&mut a, 1);
        cmd(&mut a, Command::Deploy { unit: des });
        tick(&mut a, 30);
        state_hash(a.world_mut())
    }
    assert_eq!(run(), run());
}

#[test]
fn power_loss_disables_defence_and_pauses_superweapon_charging() {
    let mut a = app();
    let power = b(&mut a, "power_plant", 2, 2);
    b(&mut a, "patriot", 10, 10);
    let air = u(&mut a, E, "kirov", 14, 10);
    tick(&mut a, 5);
    cmd(&mut a, Command::SellBuilding { building: power });
    tick(&mut a, 25);
    let before = hp(&mut a, air);
    tick(&mut a, 20);
    assert_eq!(hp(&mut a, air), before);
    let mut a = app();
    let weapon = b(&mut a, "chronosphere", 10, 10);
    tick(&mut a, 100);
    assert_eq!(
        a.world()
            .resource::<SpecialState>()
            .charge
            .get(&weapon)
            .copied()
            .unwrap_or(0),
        0
    );
    let tank = u(&mut a, P, "grizzly", 13, 13);
    cmd(
        &mut a,
        Command::Superweapon {
            building: weapon,
            target: CellPos::new(13, 14),
            units: vec![tank],
        },
    );
    assert!(rejected(&a));
}
#[test]
fn gap_satellite_and_direct_vision_use_the_existing_fog_grid() {
    let mut a = app();
    b(&mut a, "nuclear_reactor", 2, 2);
    b(&mut a, "spy_satellite", 5, 5);
    spawn_building(a.world_mut(), E, "nuclear_reactor", CellPos::new(40, 5)).unwrap();
    spawn_building(a.world_mut(), E, "gap_generator", CellPos::new(30, 30)).unwrap();
    tick(&mut a, 2);
    assert!(
        a.world()
            .resource::<Players>()
            .get(P)
            .unwrap()
            .vision()
            .is_visible(CellPos::new(40, 40))
    );
    assert!(
        !a.world()
            .resource::<Players>()
            .get(P)
            .unwrap()
            .vision()
            .is_visible(CellPos::new(30, 30))
    );
    u(&mut a, P, "gi", 29, 30);
    tick(&mut a, 1);
    assert!(
        a.world()
            .resource::<Players>()
            .get(P)
            .unwrap()
            .vision()
            .is_visible(CellPos::new(30, 30))
    );
}
#[test]
fn purifier_adds_income_and_radiation_spares_air_and_its_operator() {
    let mut a = app();
    b(&mut a, "nuclear_reactor", 2, 2);
    b(&mut a, "refinery", 6, 2);
    b(&mut a, "ore_purifier", 10, 2);
    let miner = u(&mut a, P, "chrono_miner", 20, 20);
    a.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(20, 20),
        Cell {
            water: false,
            passable: true,
            buildable: false,
            ore: 500,
        },
    );
    tick(&mut a, 100);
    assert_eq!(credits(&a), 1_000_625);
    let _ = miner;
    let des = u(&mut a, P, "desolator", 30, 30);
    let air = u(&mut a, P, "rocketeer", 31, 30);
    let before = hp(&mut a, des);
    cmd(&mut a, Command::Deploy { unit: des });
    tick(&mut a, 100);
    assert_eq!(hp(&mut a, des), before);
    assert_eq!(hp(&mut a, air), 180);
}

#[test]
fn carrier_launches_and_recovers_real_aircraft_without_airbase() {
    let mut a = app();
    b(&mut a, "nuclear_reactor", 2, 2);
    b(&mut a, "spy_satellite", 8, 2);
    let carrier = u(&mut a, P, "aircraft_carrier", 52, 20);
    let target =
        spawn_building(a.world_mut(), E, "construction_yard", CellPos::new(40, 20)).unwrap();
    tick(&mut a, 2);
    let initial = hp(&mut a, target);
    cmd(
        &mut a,
        Command::Attack {
            units: vec![carrier],
            target,
        },
    );
    let flights = a.world().resource::<SpecialState>().carrier_flights.clone();
    assert_eq!(flights.len(), 2);
    assert_eq!(
        hp(&mut a, target),
        initial,
        "carrier launch itself must not deal damage"
    );
    for id in flights.keys() {
        assert_eq!(a.world().resource::<Grid>().domain_of(*id), Domain::Air);
    }
    tick(&mut a, 150);
    assert!(
        hp(&mut a, target) < initial,
        "aircraft must reach and bomb the target"
    );
    assert!(
        flights.keys().all(|id| !a
            .world()
            .resource::<SpecialState>()
            .carrier_flights
            .contains_key(id)),
        "original wing must recover at its carrier"
    );
}

#[test]
fn ore_attack_orders_repeat_deliveries_remember_field_and_stop_on_override() {
    for name in ["harvester", "war_miner", "chrono_miner"] {
        let mut a = app();
        b(&mut a, "refinery", 6, 9);
        let field = CellPos::new(25, 25);
        a.world_mut().resource_mut::<Grid>().set_terrain(
            field,
            Cell {
                passable: true,
                buildable: false,
                water: false,
                ore: 5000,
            },
        );
        let nearby = CellPos::new(8, 14);
        a.world_mut().resource_mut::<Grid>().set_terrain(
            nearby,
            Cell {
                passable: true,
                buildable: false,
                water: false,
                ore: 5000,
            },
        );
        let miner = u(&mut a, P, name, 24, 25);
        let e = entity(&mut a, miner);
        cmd(
            &mut a,
            Command::AttackMove {
                units: vec![miner],
                target: field,
            },
        );
        assert_eq!(
            a.world().get::<Harvester>(e).unwrap().field,
            Some(field),
            "{name}"
        );
        let mut deliveries = 0;
        for _ in 0..4000 {
            tick(&mut a, 1);
            deliveries+=a.world().resource::<Messages<SimEvent>>().iter_current_update_messages()
                .filter(|event|matches!(event,SimEvent::OreDelivered{harvester,player,..} if *harvester==miner && *player==P)).count();
            if deliveries >= 2 {
                break;
            }
        }
        assert!(
            deliveries >= 2,
            "{name} must complete two unattended cycles"
        );
        assert_eq!(
            a.world().resource::<Grid>().cell(nearby).unwrap().ore,
            5000,
            "{name} must return to assigned field"
        );
        assert_eq!(
            a.world().resource::<Players>().get(E).unwrap().credits(),
            1_000_000
        );
        cmd(
            &mut a,
            Command::Move {
                units: vec![miner],
                target: CellPos::new(16, 17),
            },
        );
        assert_eq!(
            a.world().get::<Harvester>(e).unwrap().state,
            HarvestState::Idle
        );
        assert_eq!(a.world().get::<Harvester>(e).unwrap().field, None);
    }
}

#[test]
fn aircraft_fly_over_ground_units_and_ground_units_drive_under_them() {
    let mut a = app();
    // A harrier boxed in by eight tanks, and a tank parked on the plane's destination.
    let plane = u(&mut a, P, "harrier", 20, 20);
    for (dx, dy) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        u(&mut a, P, "grizzly", 20 + dx, 20 + dy);
    }
    let parked = u(&mut a, P, "rhino", 30, 20);
    tick(&mut a, 1);
    cmd(
        &mut a,
        Command::Move {
            units: vec![plane],
            target: CellPos::new(30, 20),
        },
    );
    tick(&mut a, 300);
    assert_eq!(cell(&mut a, plane), CellPos::new(30, 20));
    assert_eq!(cell(&mut a, parked), CellPos::new(30, 20));
    // A tank ordered onto the cell under the hovering plane drives there.
    let tank = u(&mut a, P, "grizzly", 30, 26);
    cmd(
        &mut a,
        Command::Move {
            units: vec![tank],
            target: CellPos::new(30, 21),
        },
    );
    tick(&mut a, 300);
    assert_eq!(cell(&mut a, tank), CellPos::new(30, 21));
    let grid = a.world().resource::<Grid>();
    assert_eq!(
        grid.occupant_for(CellPos::new(30, 20), Domain::Air),
        Some(plane)
    );
    assert_eq!(grid.occupant(CellPos::new(30, 20)), Some(parked));
}
