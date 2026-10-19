//! Declared composition traversal through ordinary queued and paid production.
use super::ra8_attrition_tests::{app, building, issue, step};
use super::*;
use bevy_app::App;

fn yard_site_control(kind: Option<&str>, tiny: bool) -> (App, Option<SimId>) {
    let mut app = super::ra8_attrition_tests::app_with_country("america");
    for y in 20..if tiny { 23 } else { 31 } {
        // A yard must retain real water exit cells in its qualified component.
        for x in 18..if tiny { 23 } else { 47 } {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                crate::Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0
                },
            ));
        }
    }
    building(&mut app, 0, "refinery", CellPos::new(4, 14));
    building(
        &mut app,
        1,
        "power_plant",
        CellPos::new(if tiny { 25 } else { 44 }, 17),
    );
    let enemy = kind.map(|kind| {
        let cell = if kind == "attack_dog" {
            CellPos::new(17, 20)
        } else if tiny {
            CellPos::new(20, 43)
        } else {
            CellPos::new(20, 24)
        };
        if tiny {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                cell,
                crate::Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0
                }
            ));
        }
        super::ra8_attrition_tests::unit(&mut app, 1, kind, cell)
    });
    let defs = app.world().resource::<Definitions>();
    let yard = defs.building_kind("naval_shipyard").unwrap();
    let hull = defs.unit_kind("destroyer").unwrap();
    let controller = app
        .world_mut()
        .resource_mut::<AiPlayers>()
        .into_inner()
        .controllers
        .get_mut(&PlayerId(0))
        .unwrap();
    controller.plan.build_order = vec![yard];
    controller.plan.army = vec![hull];
    controller.plan.defences.clear();
    controller.plan.support.clear();
    controller.plan.tier_reserves = [0, 0, 1];
    controller.difficulty.max_army = 1;
    controller.difficulty.first_wave = 100_000;
    controller.next_wave = 100_000;
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = 20_000;
    (app, enemy)
}

#[test]
fn ra8_naval_site_preference_preserves_hidden_unit_intents_and_controller_hash() {
    let (mut clear, _) = yard_site_control(None, false);
    let (mut concealed, submarine) = yard_site_control(Some("typhoon"), false);
    for _ in 0..601 {
        step(&mut clear);
        step(&mut concealed);
        assert_eq!(
            clear
                .world()
                .resource::<CommandQueue>()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            concealed
                .world()
                .resource::<CommandQueue>()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            "an unseen source submarine must not alter placement or production intents"
        );
        let a = clear
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        let b = concealed
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        assert_eq!(a, b);
        let mut ha = Fnv1a64::new();
        let mut hb = Fnv1a64::new();
        a.hash_into(&mut ha);
        b.hash_into(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }
    assert!(
        concealed
            .world()
            .resource::<crate::SpecialState>()
            .hidden
            .contains(&(PlayerId(0), submarine.unwrap())),
        "the ordinary source submarine must really remain concealed; no hidden flag is seeded"
    );
    assert!(
        clear
            .world()
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap()
            .naval_target()
            .is_some(),
        "real qualified naval planning must have run in both twins"
    );
}

#[test]
fn ra8_naval_site_preference_retains_source_minimum_target_eligibility_and_exposed_fallback() {
    for (kind, tiny) in [
        ("dreadnought", false),
        ("attack_dog", false),
        ("dreadnought", true),
    ] {
        eprintln!("source site control {kind}/tiny{tiny}");
        let (mut app, enemy) = yard_site_control(Some(kind), tiny);
        let yard = app
            .world()
            .resource::<Definitions>()
            .building_kind("naval_shipyard")
            .unwrap();
        let mut placed = None;
        for _ in 0..3000 {
            for event in step(&mut app) {
                if let crate::SimEvent::BuildingPlaced {
                    player: PlayerId(0),
                    building,
                    kind,
                } = event
                    && kind == yard
                {
                    let world = app.world_mut();
                    placed = Some(
                        world
                            .query::<(&SimId, &crate::Building)>()
                            .iter(world)
                            .find(|(id, _)| **id == building)
                            .unwrap()
                            .1
                            .footprint,
                    );
                }
            }
            if placed.is_some() {
                break;
            }
        }
        let footprint =
            placed.expect("a source-qualified exposed-only yard must remain admissible");
        // Existing placement keeps the observed Dog out of the footprint's
        // one-cell ring, independently of whether its weapon can hurt a yard.
        assert_eq!(
            footprint.origin,
            CellPos::new(if kind == "attack_dog" { 19 } else { 18 }, 20),
            "{kind}/tiny{tiny}"
        );
        let world = app.world_mut();
        let (unit, mobile) = world
            .query::<(&SimId, &crate::Unit, &Mobile)>()
            .iter(world)
            .find_map(|(id, unit, mobile)| (*id == enemy.unwrap()).then_some((*unit, *mobile)))
            .unwrap();
        let defs = world.resource::<Definitions>();
        let terms = defs.unit_combat(unit.kind).unwrap();
        let target_terms = defs.building_combat(yard).unwrap();
        let traits = crate::combat::traits_of(
            world.resource::<Grid>(),
            SimId(u32::MAX),
            target_terms.armour,
            &crate::combat::Body::Building(footprint),
            Some(&target_terms),
        );
        let selected = crate::weapon_against(defs, &terms, terms.weapons(false), &traits);
        if kind == "attack_dog" {
            assert!(
                selected.is_none_or(|(_, percent)| percent <= 1),
                "source Dog cannot damage this naval building"
            );
        } else {
            let weapon = defs.weapon(selected.unwrap().0).unwrap();
            let (dx, dy) = crate::combat::Extent::point(
                mobile.position(world.resource::<Grid>(), enemy.unwrap()),
            )
            .gap(&crate::combat::Extent::footprint(footprint));
            assert_eq!(crate::inside_min_range(dx, dy, weapon.min_range), !tiny);
            assert!(crate::in_range(dx, dy, weapon.range));
        }
    }
}

#[test]
fn ra8_paid_shipyard_uses_an_available_same_sea_site_outside_observed_bombardment() {
    let mut app = super::ra8_attrition_tests::app_with_country("america");
    // The three occupied bombardment cells exhaust this separate pond. This
    // isolates endpoint choice rather than predicting future hostile movement.
    for (xs, ys) in [(18..47, 20..31), (19..22, 43..44)] {
        for y in ys {
            for x in xs.clone() {
                assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                    CellPos::new(x, y),
                    crate::Cell {
                        passable: false,
                        buildable: false,
                        water: true,
                        ore: 0
                    },
                ));
            }
        }
    }
    building(&mut app, 0, "refinery", CellPos::new(4, 14));
    building(&mut app, 0, "tech_outpost", CellPos::new(12, 17));
    building(&mut app, 1, "power_plant", CellPos::new(44, 17));
    building(&mut app, 1, "nuclear_reactor", CellPos::new(2, 36));
    building(&mut app, 1, "spy_satellite", CellPos::new(2, 40));
    let hulls: Vec<_> = [
        CellPos::new(19, 43),
        CellPos::new(20, 43),
        CellPos::new(21, 43),
    ]
    .into_iter()
    .map(|cell| super::ra8_attrition_tests::unit(&mut app, 1, "dreadnought", cell))
    .collect();
    let defs = app.world().resource::<Definitions>();
    let yard = defs.building_kind("naval_shipyard").unwrap();
    let destroyer = defs.unit_kind("destroyer").unwrap();
    let dread = defs.unit_kind("dreadnought").unwrap();
    let weapon = defs
        .weapon(defs.unit_combat(dread).unwrap().weapons(false)[0].unwrap())
        .unwrap()
        .clone();
    let initial = 20_000;
    let yard_cost = defs
        .production(ProductionItem::Building(yard))
        .unwrap()
        .cost;
    let hull_cost = defs
        .production(ProductionItem::Unit(destroyer))
        .unwrap()
        .cost;
    {
        let mut ai = app.world_mut().resource_mut::<AiPlayers>();
        let controller = ai.controllers.get_mut(&PlayerId(0)).unwrap();
        // Declared paid naval-production projection. The actual source refinery,
        // yard prerequisite, prices, placement reach and weapons remain unchanged.
        controller.plan.build_order = vec![yard];
        controller.plan.defences.clear();
        controller.plan.support.clear();
        controller.plan.army = vec![destroyer];
        controller.plan.tier_reserves = [0, 0, 1];
        controller.difficulty.first_wave = 100_000;
        controller.next_wave = 100_000;
        controller.difficulty.max_army = 1;
    }
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = initial;
    step(&mut app);
    let owned: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &crate::Building)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == PlayerId(0))
            .map(|(_, b)| (b.kind, b.footprint))
            .collect()
    };
    assert!(
        check_buildable(
            app.world().resource::<Definitions>(),
            app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
            false,
            &owned.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
            ProductionItem::Building(yard)
        )
        .is_ok()
    );
    assert!(
        check_placement_site(
            app.world().resource::<Grid>(),
            app.world().resource::<Definitions>(),
            app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
            owned.into_iter(),
            yard,
            CellPos::new(42, 20)
        )
        .is_ok(),
        "the safe alternative must pass ordinary water/shore/visibility/ownership-reach geometry"
    );
    let mut placed = None;
    let mut shot = false;
    let mut lost = false;
    let mut produced = false;
    let mut spawned = None;
    for _ in 0..6000 {
        for event in step(&mut app) {
            match event {
                crate::SimEvent::BuildingPlaced {
                    player: PlayerId(0),
                    building,
                    kind,
                } if kind == yard && placed.is_none() => {
                    let world = app.world_mut();
                    let origin = world
                        .query::<(&SimId, &crate::Building)>()
                        .iter(world)
                        .find(|(id, _)| **id == building)
                        .unwrap()
                        .1
                        .footprint
                        .origin;
                    placed = Some((building, origin));
                    issue(
                        &mut app,
                        1,
                        Command::Attack {
                            units: hulls.clone(),
                            target: building,
                        },
                    );
                }
                crate::SimEvent::WeaponFired {
                    attacker, target, ..
                } if hulls.contains(&attacker) && placed.is_some_and(|(id, _)| id == target) => {
                    shot = true
                }
                crate::SimEvent::BuildingDestroyed { building, .. }
                    if placed.is_some_and(|(id, _)| id == building) =>
                {
                    lost = true
                }
                crate::SimEvent::ProductionComplete {
                    player: PlayerId(0),
                    item: ProductionItem::Unit(kind),
                } if kind == destroyer => produced = true,
                crate::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    unit,
                    building,
                } if placed.is_some_and(|(yard, _)| yard == building) => spawned = Some(unit),
                _ => {}
            }
        }
        assert!(crate::stray_claims(app.world_mut()).is_empty());
        if (produced && spawned.is_some()) || lost {
            break;
        }
    }
    let (_, origin) = placed.expect("normal paid yard must actually be placed");
    let footprint = Footprint {
        origin,
        width: 3,
        height: 3,
    };
    let exposed = [
        CellPos::new(19, 43),
        CellPos::new(20, 43),
        CellPos::new(21, 43),
    ]
    .into_iter()
    .any(|cell| {
        let (dx, dy) = crate::combat::Extent::point(crate::SubPos::centre_of(cell))
            .gap(&crate::combat::Extent::footprint(footprint));
        crate::in_range(dx, dy, weapon.range) && !crate::inside_min_range(dx, dy, weapon.min_range)
    });
    eprintln!(
        "paid yard origin{origin:?} exposed{exposed} native_shot{shot} lost{lost} destroyer{produced}"
    );
    assert!(
        !exposed && !shot && !lost && produced && spawned.is_some(),
        "a legal useful safe site must preserve the paid yard and its actual counterforce production"
    );
    let world = app.world_mut();
    assert!(
        world
            .query::<(&SimId, &Owner, &crate::Unit)>()
            .iter(world)
            .any(|(id, owner, unit)| Some(*id) == spawned
                && owner.0 == PlayerId(0)
                && unit.kind == destroyer),
        "the paid completion must become a real living owned Destroyer"
    );
    let (yard_id, _) = placed.unwrap();
    let actual = world
        .query::<(&SimId, &Owner, &crate::Building)>()
        .iter(world)
        .find(|(id, owner, building)| {
            **id == yard_id && owner.0 == PlayerId(0) && building.kind == yard
        })
        .unwrap()
        .2
        .footprint;
    assert!(
        actual
            .cells()
            .all(|cell| world.resource::<Grid>().building(cell) == Some(yard_id))
    );
    assert!(crate::cell_invariant_violations(world).is_empty());
    assert!(crate::stray_claims(world).is_empty());
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        initial - yard_cost - hull_cost
    );
}

fn composition(army: &[&str], opted_in: bool, reserves: [u32; 3], budget: i64, cap: u32) -> App {
    let mut app = app();
    building(&mut app, 0, "barracks", CellPos::new(2, 14));
    building(&mut app, 0, "war_factory", CellPos::new(6, 14));
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    building(&mut app, 0, "tech_center", CellPos::new(12, 10));
    building(&mut app, 0, "airforce_command", CellPos::new(18, 4));
    let kinds: Vec<_> = army
        .iter()
        .map(|id| app.world().resource::<Definitions>().unit_kind(id).unwrap())
        .collect();
    let controller = app
        .world_mut()
        .resource_mut::<AiPlayers>()
        .into_inner()
        .controllers
        .get_mut(&PlayerId(0))
        .unwrap();
    // Declared isolated policy fixture: no unrelated construction/support costs;
    // normal source owners, prerequisites, producers, prices and command checks.
    controller.plan.army = kinds;
    controller.plan.rules_difficulty = opted_in;
    controller.plan.tier_reserves = reserves;
    controller.plan.build_order.clear();
    controller.plan.defences.clear();
    controller.plan.support.clear();
    controller.difficulty.max_army = cap;
    controller.difficulty.first_wave = 10_000;
    controller.next_wave = 10_000;
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = budget;
    app
}

fn paid_sequence(app: &mut App, count: usize) -> Vec<UnitKind> {
    let initial_units = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &Unit)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .count()
    };
    let initial = app
        .world()
        .resource::<Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    let mut result = Vec::new();
    for _ in 0..10_000 {
        for event in step(app) {
            if let crate::SimEvent::ProductionComplete {
                player: PlayerId(0),
                item: ProductionItem::Unit(kind),
            } = event
            {
                result.push(kind);
            }
        }
        let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
        for (_, queue) in state.queues() {
            assert!(
                queue.len() <= AI_UNIT_QUEUE,
                "bounded ordinary producer admissions"
            );
        }
        if result.len() == count {
            let cost: i64 = result
                .iter()
                .map(|&k| {
                    app.world()
                        .resource::<Definitions>()
                        .production(ProductionItem::Unit(k))
                        .unwrap()
                        .cost
                })
                .sum();
            assert_eq!(state.credits(), initial - cost);
            let world = app.world_mut();
            assert_eq!(
                world
                    .query::<(&Owner, &Unit)>()
                    .iter(world)
                    .filter(|(o, _)| o.0 == PlayerId(0))
                    .count(),
                initial_units + count,
                "paid completions must actually spawn their source units"
            );
            return result;
        }
    }
    panic!("ordinary paid sequence incomplete: {result:?}");
}

#[test]
fn ra8_germany_pays_its_declared_siege_pair_and_national_unit_again_after_real_casualties() {
    let source = app();
    let defs = source.world().resource::<Definitions>();
    // Declared production fixture: project the shipped Germany cycle to its
    // direct-fire ground vehicles. Infantry, aircraft, ships and reactive AA
    // use other source lanes/policies and remain covered by default matches.
    let army: Vec<_> = source
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .plan()
        .army
        .iter()
        .filter_map(|&kind| {
            let unit = defs.unit(kind)?;
            let meta = defs.metadata(ProductionItem::Unit(kind))?;
            (meta.domain == Domain::Land && !meta.infantry && meta.role != Role::Ifv)
                .then_some(unit.id.as_str())
        })
        .collect();
    assert_eq!(
        army,
        [
            "grizzly",
            "grizzly",
            "prism_tank",
            "prism_tank",
            "tank_destroyer",
            "mirage_tank"
        ]
    );
    let mut app = composition(&army, true, [0, 2, 0], 25_000, 9);
    {
        let mut players = app.world_mut().resource_mut::<AiPlayers>();
        let controller = players.controllers.get_mut(&PlayerId(0)).unwrap();
        // Isolate actual paid production and casualties, without offensive
        // travel changing the selected cohort during this capability fixture.
        controller.difficulty.first_wave = 100_000;
        controller.next_wave = 100_000;
    }
    let expected = kinds(
        &app,
        &[
            "prism_tank",
            "prism_tank",
            "grizzly",
            "grizzly",
            "prism_tank",
            "prism_tank",
            "tank_destroyer",
            "mirage_tank",
            "grizzly",
        ],
    );
    assert_eq!(paid_sequence(&mut app, 9), expected);
    let prism = kinds(&app, &["prism_tank"])[0];
    let mirage = kinds(&app, &["mirage_tank"])[0];
    let national = kinds(&app, &["tank_destroyer"])[0];
    let mut owned = {
        let world = app.world_mut();
        world
            .query::<(&SimId, &Owner, &Unit)>()
            .iter(world)
            .filter(|(_, o, u)| o.0 == PlayerId(0) && [prism, mirage, national].contains(&u.kind))
            .map(|(id, _, u)| (*id, u.kind))
            .collect::<Vec<_>>()
    };
    owned.sort();
    let actor = owned.iter().find(|(_, k)| *k == prism).unwrap().0;
    let controllers = std::mem::take(app.world_mut().resource_mut::<AiPlayers>().into_inner());
    for (victim, kind) in owned.into_iter().filter(|(id, _)| *id != actor) {
        assert!(
            !app.world()
                .resource::<Definitions>()
                .metadata(ProductionItem::Unit(kind))
                .unwrap()
                .crewed
        );
        issue(
            &mut app,
            0,
            Command::ForceFire {
                units: vec![actor],
                target: crate::ForceTarget::Entity(victim),
            },
        );
        let mut destroyed = false;
        for _ in 0..2000 {
            if step(&mut app).iter().any(|event| matches!(event, crate::SimEvent::UnitDestroyed { unit, .. } if *unit == victim)) {
                destroyed = true;
                break;
            }
        }
        assert!(
            destroyed,
            "ordinary source weapon must produce the real casualty"
        );
    }
    issue(&mut app, 0, Command::Stop { units: vec![actor] });
    step(&mut app);
    app.world_mut().insert_resource(controllers);
    // The surviving Prism fills one advanced slot. Ordinary refill must pay
    // the other first, then traverse the retained cycle through the siege pair
    // and national replacement (Tank Destroyer's actual source tech is two).
    let replacement = kinds(
        &app,
        &[
            "prism_tank",
            "grizzly",
            "prism_tank",
            "prism_tank",
            "tank_destroyer",
        ],
    );
    assert_eq!(paid_sequence(&mut app, 5), replacement);
}

fn kinds(app: &App, ids: &[&str]) -> Vec<UnitKind> {
    ids.iter()
        .map(|id| app.world().resource::<Definitions>().unit_kind(id).unwrap())
        .collect()
}

#[test]
fn ra8_opted_in_composition_pays_and_spawns_every_declared_entry_in_order_and_wraps() {
    let mut app = composition(
        &["grizzly", "prism_tank", "mirage_tank"],
        true,
        [0; 3],
        20_000,
        6,
    );
    let expected = kinds(
        &app,
        &[
            "grizzly",
            "prism_tank",
            "mirage_tank",
            "grizzly",
            "prism_tank",
            "mirage_tank",
        ],
    );
    let actual = paid_sequence(&mut app, 6);
    eprintln!("actual paid traversal {actual:?}; expected {expected:?}");
    assert_eq!(actual, expected);
}

#[test]
fn ra8_legacy_composition_keeps_its_existing_paid_traversal() {
    let mut app = composition(
        &["grizzly", "prism_tank", "mirage_tank"],
        false,
        [0; 3],
        20_000,
        3,
    );
    let expected = kinds(&app, &["grizzly", "mirage_tank", "grizzly"]);
    assert_eq!(paid_sequence(&mut app, 3), expected);
}

#[test]
fn ra8_opted_in_composition_skips_a_funded_locked_entry_and_pays_later_legal_units() {
    let mut app = composition(
        &["grizzly", "chrono_commando", "mirage_tank"],
        true,
        [0; 3],
        5_000,
        2,
    );
    let defs = app.world().resource::<Definitions>();
    let locked = ProductionItem::Unit(defs.unit_kind("chrono_commando").unwrap());
    assert!(defs.production(locked).unwrap().cost <= 5_000);
    let own: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &Building)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, b)| b.kind)
            .collect()
    };
    assert!(matches!(
        crate::check_buildable(
            app.world().resource::<Definitions>(),
            app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
            true,
            &own,
            locked,
        ),
        Err(crate::RejectReason::AbilityUnavailable(_))
    ));
    let expected = kinds(&app, &["grizzly", "mirage_tank"]);
    assert_eq!(paid_sequence(&mut app, 2), expected);
}

#[test]
fn ra8_opted_in_composition_skips_locked_and_unaffordable_entries_without_losing_legal_work() {
    let mut app = composition(
        &["grizzly", "chrono_commando", "prism_tank", "gi"],
        true,
        [0; 3],
        900,
        2,
    );
    let expected = kinds(&app, &["gi", "grizzly"]);
    let mut actual = paid_sequence(&mut app, 2);
    // Different real producers finish at their own source speed; admission order
    // need not be completion order across lanes.
    actual.sort();
    let mut expected = expected;
    expected.sort();
    assert_eq!(actual, expected);
    assert!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .all(|(_, q)| q.is_empty())
    );
}

#[test]
fn ra8_opted_in_tier_reserves_share_real_queue_bounds_and_paid_capacity() {
    let mut app = composition(
        &["grizzly", "prism_tank", "mirage_tank", "harrier"],
        true,
        [1, 2, 0],
        20_000,
        3,
    );
    step(&mut app);
    let emitted: Vec<_> = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .filter_map(|c| {
            if let Command::QueueProduction {
                item: ProductionItem::Unit(k),
            } = c.command
            {
                Some(k)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        emitted,
        kinds(&app, &["harrier", "prism_tank", "mirage_tank"])
    );
    // Include the first decision in the ordinary paid sequence, not injected queue state.
    let mut actual = paid_sequence(&mut app, 3);
    actual.sort();
    let mut expected = kinds(&app, &["prism_tank", "mirage_tank", "harrier"]);
    expected.sort();
    assert_eq!(actual, expected);
}

fn libya_land_cycle(tech: bool) -> (App, AiPlan) {
    let mut app = super::ra8_attrition_tests::app_with_country("libya");
    building(&mut app, 0, "soviet_barracks", CellPos::new(2, 14));
    building(&mut app, 0, "soviet_war_factory", CellPos::new(6, 14));
    if tech {
        building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
        building(&mut app, 0, "soviet_battle_lab", CellPos::new(12, 10));
    }
    let definitions = app.world().resource::<Definitions>().clone();
    let source_plan = app
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .plan()
        .clone();
    let army: Vec<_> = source_plan
        .army
        .iter()
        .copied()
        .filter(|&k| definitions.domain(k) == Domain::Land)
        .collect();
    let controller = app
        .world_mut()
        .resource_mut::<AiPlayers>()
        .into_inner()
        .controllers
        .get_mut(&PlayerId(0))
        .unwrap();
    // Declared mixed-producer projection of the actual Libya cycle. Only
    // unrelated air/naval quotas, building/support spending and combat are
    // isolated; the sourced land prices/prerequisites and advanced reserve stay.
    controller.plan.army = army;
    controller.plan.tier_reserves = [0, source_plan.tier_reserves[1], 0];
    controller.plan.build_order.clear();
    controller.plan.defences.clear();
    controller.plan.support.clear();
    controller.difficulty.max_army = 16;
    controller.difficulty.first_wave = 10_000;
    controller.next_wave = 10_000;
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = 40_000;
    (app, source_plan)
}

#[test]
fn ra8_per_queue_composition_cursor_hash_covers_keys_indices_and_ordered_values() {
    let app = app();
    let base = app
        .world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
        .clone();
    let hash = |controller: &AiController| {
        let mut h = Fnv1a64::new();
        controller.hash_into(&mut h);
        h.finish()
    };
    let factory = QueueKey {
        producer: BuildingKind(4),
        lane: Lane::Main,
    };
    let barracks = QueueKey {
        producer: BuildingKind(3),
        lane: Lane::Main,
    };
    let mut a = base.clone();
    a.army_cursors.insert(factory, 0);
    assert_ne!(
        hash(&base),
        hash(&a),
        "even a zero cursor is recorded queue state"
    );
    for (key, value) in [
        (
            QueueKey {
                producer: BuildingKind(5),
                ..factory
            },
            0,
        ),
        (
            QueueKey {
                lane: Lane::Defences,
                ..factory
            },
            0,
        ),
        (factory, 1),
    ] {
        let mut changed = base.clone();
        changed.army_cursors.insert(key, value);
        assert_ne!(
            hash(&a),
            hash(&changed),
            "producer, lane and index independently affect state"
        );
    }
    a.army_cursors.insert(barracks, 2);
    let mut b = base.clone();
    b.army_cursors.insert(barracks, 2);
    b.army_cursors.insert(factory, 0);
    assert_eq!(a, b);
    assert_eq!(
        hash(&a),
        hash(&b),
        "insertion order cannot affect ordered state"
    );
    b.army_cursors.clear();
    assert_eq!(
        hash(&base),
        hash(&b),
        "empty legacy state retains its original hash encoding"
    );
}

#[test]
fn ra8_mixed_producer_admissions_cannot_skip_the_busy_factorys_declared_cycle() {
    let (mut app, _) = libya_land_cycle(true);
    let definitions = app.world().resource::<Definitions>().clone();
    let declared = kinds(
        &app,
        &["v3_launcher", "rhino", "conscript", "yuri", "kirov"],
    );
    let expected = kinds(&app, &["v3_launcher", "rhino", "kirov"]).repeat(2);
    let factory = definitions
        .production(ProductionItem::Unit(expected[0]))
        .unwrap()
        .producer;
    let owned: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &Building)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, b)| b.kind)
            .collect()
    };
    for &kind in &declared {
        assert!(
            crate::check_buildable(
                &definitions,
                app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
                true,
                &owned,
                ProductionItem::Unit(kind)
            )
            .is_ok()
        );
    }
    {
        let controller = app
            .world_mut()
            .resource_mut::<AiPlayers>()
            .into_inner()
            .controllers
            .get_mut(&PlayerId(0))
            .unwrap();
        // Project the real legal Libya mixed lanes, isolating tier refills and
        // combat. Source prices/durations, FIFO bounds and ordinary AI remain.
        controller.plan.army = declared;
        controller.plan.tier_reserves = [0; 3];
        controller.difficulty.max_army = 42;
        controller.difficulty.first_wave = 20_000;
        controller.next_wave = 20_000;
    }
    let mut spawned = Vec::new();
    let mut infantry = 0;
    for _ in 0..10_000 {
        let events = step(&mut app);
        assert!(
            app.world()
                .resource::<Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .all(|(_, q)| q.len() <= AI_UNIT_QUEUE)
        );
        let at = app.world().resource::<SimTick>().0;
        for event in events {
            if let crate::SimEvent::UnitSpawned {
                player: PlayerId(0),
                unit,
                ..
            } = event
            {
                let world = app.world_mut();
                let kind = world
                    .query::<(&SimId, &Unit)>()
                    .iter(world)
                    .find(|(id, _)| **id == unit)
                    .unwrap()
                    .1
                    .kind;
                if definitions
                    .production(ProductionItem::Unit(kind))
                    .unwrap()
                    .producer
                    == factory
                {
                    spawned.push(kind);
                } else {
                    infantry += 1;
                }
            }
        }
        if at % 20 == 1 {
            let intents: Vec<_> = app
                .world()
                .resource::<crate::CommandQueue>()
                .iter()
                .filter(|i| {
                    i.player == PlayerId(0) && matches!(i.command, Command::QueueProduction { .. })
                })
                .collect();
            if !intents.is_empty() {
                let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
                eprintln!(
                    "mixed cycle tick{at} queues{:?} intents{intents:?}",
                    state.queues().collect::<Vec<_>>()
                );
            }
        }
        if spawned.len() >= expected.len() {
            break;
        }
    }
    eprintln!("mixed factory actual paid spawns {spawned:?}, infantry spawns {infantry}");
    assert!(
        infantry > 0,
        "the other lane must really run while the factory is busy"
    );
    assert_eq!(
        spawned, expected,
        "a barracks admission cannot consume a factory entry that has not been admitted"
    );
}

#[test]
fn ra8_libya_shipped_land_cycle_funds_and_spawns_siege_alongside_its_national_reserve() {
    let (mut app, source_plan) = libya_land_cycle(true);
    let definitions = app.world().resource::<Definitions>().clone();
    let expected = kinds(&app, &["v3_launcher", "demolition_truck"]);
    let owned: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &Building)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == PlayerId(0))
            .map(|(_, b)| b.kind)
            .collect()
    };
    for (&kind, cost) in expected.iter().zip([800, 1_500]) {
        let item = ProductionItem::Unit(kind);
        assert!(source_plan.army.contains(&kind));
        assert_eq!(definitions.production(item).unwrap().cost, cost);
        assert!(
            crate::check_buildable(
                &definitions,
                app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
                true,
                &owned,
                item
            )
            .is_ok(),
            "source country/prerequisite legality must hold before the paid control"
        );
        assert!(
            owned
                .iter()
                .any(|&producer| produces(&definitions, producer, item)),
            "actual required producer is present"
        );
    }
    let paid = paid_sequence(&mut app, 16);
    eprintln!("Libya actual mixed-lane paid cycle {paid:?}");
    assert!(
        paid.contains(&expected[0]),
        "legally affordable source siege must actually be funded and spawned through the mixed-producer plan"
    );
    assert!(
        paid.contains(&expected[1]),
        "the real national reserve must remain reachable and paid"
    );
}

#[test]
fn ra8_libya_siege_priority_keeps_the_normal_locked_tech_tank_opening() {
    let (mut app, _) = libya_land_cycle(false);
    let definitions = app.world().resource::<Definitions>().clone();
    let owned: Vec<_> = {
        let world = app.world_mut();
        world
            .query::<(&Owner, &Building)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, b)| b.kind)
            .collect()
    };
    for id in ["v3_launcher", "demolition_truck"] {
        let item = ProductionItem::Unit(definitions.unit_kind(id).unwrap());
        assert!(matches!(
            crate::check_buildable(
                &definitions,
                app.world().resource::<Players>().get(PlayerId(0)).unwrap(),
                true,
                &owned,
                item
            ),
            Err(crate::RejectReason::MissingPrerequisite { .. })
        ));
    }
    let tank = definitions.unit_kind("rhino").unwrap();
    let mut paid = Vec::new();
    for _ in 0..3000 {
        for e in step(&mut app) {
            if let crate::SimEvent::ProductionComplete {
                player: PlayerId(0),
                item: ProductionItem::Unit(k),
            } = e
                && definitions.domain(k) == Domain::Land
                && !definitions
                    .metadata(ProductionItem::Unit(k))
                    .unwrap()
                    .infantry
            {
                paid.push(k);
            }
        }
        if paid.len() == 2 {
            break;
        }
    }
    assert_eq!(
        paid,
        vec![tank, tank],
        "locked siege/national entries must retain the two real paid opening tanks"
    );
}

#[test]
fn ra8_libya_replaces_its_national_with_paid_production_after_real_enemy_capture() {
    let (mut app, _) = libya_land_cycle(true);
    let national = app
        .world()
        .resource::<Definitions>()
        .unit_kind("demolition_truck")
        .unwrap();
    // Capture the first real paid national, still within the original first16
    // admissions. Waiting for all16 may now produce a second national, which
    // does not exercise loss of the country's sole representative.
    let mut first_paid = Vec::new();
    for _ in 0..10_000 {
        for event in step(&mut app) {
            if let crate::SimEvent::ProductionComplete {
                player: PlayerId(0),
                item: ProductionItem::Unit(kind),
            } = event
            {
                first_paid.push(kind);
            }
        }
        if first_paid.contains(&national) {
            break;
        }
    }
    assert!(
        first_paid.len() <= 16 && first_paid.contains(&national),
        "the national must still be paid within the original16-admission bound"
    );
    {
        let world = app.world_mut();
        let own: Vec<_> = world
            .query::<(&Owner, &Unit)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, u)| u.kind)
            .collect();
        let controller = world
            .resource::<AiPlayers>()
            .controller(PlayerId(0))
            .unwrap();
        eprintln!(
            "national capture setup owned{own:?} cap{} cursors{:?}",
            controller.difficulty.max_army, controller.army_cursors
        );
        assert_eq!(own.iter().filter(|&&kind| kind == national).count(), 1);
    }
    let victim = {
        let world = app.world_mut();
        world
            .query::<(&SimId, &Owner, &Unit)>()
            .iter(world)
            .find(|(_, o, u)| o.0 == PlayerId(0) && u.kind == national)
            .unwrap()
            .0
            .to_owned()
    };
    let controllers = std::mem::take(app.world_mut().resource_mut::<AiPlayers>().into_inner());
    let destination = CellPos::new(28, 8);
    issue(
        &mut app,
        0,
        Command::Move {
            units: vec![victim],
            target: destination,
        },
    );
    let mut arrived = false;
    for _ in 0..3000 {
        step(&mut app);
        let world = app.world_mut();
        arrived = world
            .query::<(&SimId, &Mobile)>()
            .iter(world)
            .any(|(id, m)| *id == victim && m.cell == destination && m.step.is_none());
        if arrived {
            break;
        }
    }
    assert!(arrived);
    // Ordinary Visibility must run before the newly spawned human actor can
    // legally name a target; start outside its weapon range but within sight.
    let captor = super::ra8_attrition_tests::unit(&mut app, 1, "yuri", CellPos::new(37, 8));
    step(&mut app);
    {
        let world = app.world_mut();
        let (owner, mobile) = world
            .query::<(&SimId, &Owner, &Mobile)>()
            .iter(world)
            .find(|(id, _, _)| **id == victim)
            .map(|(_, o, m)| (*o, *m))
            .unwrap();
        assert_eq!(owner.0, PlayerId(0));
        assert!(
            world
                .resource::<Players>()
                .get(PlayerId(1))
                .unwrap()
                .vision()
                .sees_unit(&mobile)
        );
    }
    issue(
        &mut app,
        1,
        Command::Attack {
            units: vec![captor],
            target: victim,
        },
    );
    let mut captured = false;
    for _ in 0..1000 {
        step(&mut app);
        let world = app.world_mut();
        captured = world
            .query::<(&SimId, &Owner)>()
            .iter(world)
            .any(|(id, o)| *id == victim && o.0 == PlayerId(1));
        if captured {
            break;
        }
    }
    assert!(
        captured,
        "actual source mind control must remove national ownership without fabricated death or queue mutation"
    );
    {
        let world = app.world_mut();
        let own: Vec<_> = world
            .query::<(&Owner, &Unit)>()
            .iter(world)
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, u)| u.kind)
            .collect();
        eprintln!("national after actual capture owned{own:?}");
        assert!(
            !own.contains(&national),
            "actual capture must remove the last owned national"
        );
    }
    app.world_mut().insert_resource(controllers);
    let mut paid = false;
    for _ in 0..5000 {
        for e in step(&mut app) {
            paid |= matches!(e,crate::SimEvent::ProductionComplete {player:PlayerId(0),item:ProductionItem::Unit(k)} if k==national);
        }
        assert!(
            app.world()
                .resource::<Players>()
                .get(PlayerId(0))
                .unwrap()
                .queues()
                .all(|(_, q)| q.len() <= AI_UNIT_QUEUE)
        );
        if paid {
            break;
        }
    }
    assert!(
        paid,
        "source national replacement must actually be paid after the siege-priority retune"
    );
    let world = app.world_mut();
    assert!(
        world
            .query::<(&SimId, &Owner, &Unit)>()
            .iter(world)
            .any(|(id, o, u)| *id != victim && o.0 == PlayerId(0) && u.kind == national)
    );
}
