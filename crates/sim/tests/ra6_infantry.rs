//! RA6 infantry acceptance on shipped data through commands and the simulation schedule.
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand, Owner, PlayerId,
    Players, ProductionItem, SimEvent, SimId, SimPlugin, SimSchedule, load_ruleset, spawn_building,
    spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

fn shipped() -> Definitions {
    load_ruleset(data_dir()).unwrap_or_else(|error| panic!("{error}"))
}

const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};

/// A world of 48 x 48 open cells with the shipped data and the players P0 (Great Britain,
/// Allied) and P1 (Russia, Soviet), who are enemies. It has no match, so no crate appears.
fn world() -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("a grid"));
    for (player, country) in [(P0, "great_britain"), (P1, "russia")] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 10_000).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

fn put(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    if defs(app).unit_kind(id).is_some() {
        spawn_unit(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    } else {
        spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    }
}

fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(command);
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("a player")
        .credits()
}

fn rejected(events: &[SimEvent]) -> Vec<&SimEvent> {
    events
        .iter()
        .filter(|event| matches!(event, SimEvent::CommandRejected { .. }))
        .collect()
}

fn entity(app: &mut App, id: SimId) -> Option<bevy_ecs::entity::Entity> {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(entity, _)| entity)
}
fn owner(app: &mut App, id: SimId) -> Option<PlayerId> {
    let entity = entity(app, id)?;
    app.world().get::<Owner>(entity).map(|owner| owner.0)
}
fn hp(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id).expect("object exists");
    app.world()
        .get::<sim::Health>(entity)
        .expect("health")
        .hit_points
}

fn interact(app: &mut App, actor: SimId, target: SimId) -> Vec<SimEvent> {
    tick(
        app,
        vec![issue(
            P0,
            Command::Interact {
                unit: actor,
                target,
            },
        )],
    )
}
fn cell(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).expect("unit");
    app.world().get::<sim::Mobile>(e).expect("mobile").cell
}
fn set_hp(app: &mut App, id: SimId, hp: i32) {
    let e = entity(app, id).expect("actor");
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .expect("health")
        .hit_points = hp;
}
fn steps(app: &mut App, count: usize) {
    for _ in 0..count {
        tick(app, vec![]);
    }
}

#[test]
fn ra6_gi_deploy_persists_uses_para_and_does_not_move_or_gain_armour() {
    let mut app = world();
    let gi = put(&mut app, P0, "gi", 8, 8);
    tick(&mut app, vec![issue(P0, Command::Deploy { unit: gi })]);
    let e = entity(&mut app, gi).unwrap();
    assert_eq!(app.world().get::<sim::Mobile>(e).unwrap().speed, 0);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![gi],
                target: CellPos::new(14, 8),
            },
        )],
    );
    steps(&mut app, 130);
    assert_eq!(cell(&mut app, gi), CellPos::new(8, 8));
    let target = put(&mut app, P1, "construction_yard", 11, 8);
    let mut events = tick(&mut app, vec![]);
    events.extend(tick(&mut app, vec![]));
    let para = defs(&app).weapon_kind("para").unwrap();
    assert!(events.iter().any(|e| matches!(e, SimEvent::WeaponFired { attacker, target: t, weapon } if *attacker==gi && *t==target && *weapon==para)), "{events:?}");
    tick(&mut app, vec![issue(P0, Command::Deploy { unit: gi })]);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![gi],
                target: CellPos::new(5, 8),
            },
        )],
    );
    steps(&mut app, 40);
    assert_ne!(cell(&mut app, gi), CellPos::new(8, 8));
}

#[test]
fn ra6_ivan_normal_elite_and_engineer_defuse_on_due_tick() {
    for elite in [false, true] {
        let mut app = world();
        let ivan = put(&mut app, P0, "crazy_ivan", 8, 8);
        let target = put(&mut app, P0, "construction_yard", 9, 8);
        let engineer = put(&mut app, P0, "engineer", 8, 9);
        tick(
            &mut app,
            vec![issue(
                P0,
                Command::Guard {
                    units: vec![engineer],
                },
            )],
        );
        if elite {
            // IVAN is Trainable=no. Scenario-injected elite state exercises the INI's
            // otherwise unreachable ElitePrimary without changing that training rule.
            assert!(!sim::promote(app.world_mut(), ivan, sim::ELITE));
            let e = entity(&mut app, ivan).unwrap();
            app.world_mut().entity_mut(e).insert(sim::Experience {
                rank: sim::ELITE,
                ..Default::default()
            });
        }
        tick(&mut app, vec![]);
        assert!(rejected(&interact(&mut app, ivan, target)).is_empty());
        let charge = app.world().resource::<sim::SpecialState>().charges[&target].clone();
        assert_eq!(charge.damage, if elite { 600 } else { 400 });
        while app.world().resource::<sim::SimTick>().0 < charge.due {
            tick(&mut app, vec![]);
        }
        let before = hp(&mut app, target);
        assert!(rejected(&interact(&mut app, engineer, target)).is_empty());
        assert_eq!(hp(&mut app, target), before);
        assert!(
            !app.world()
                .resource::<sim::SpecialState>()
                .charges
                .contains_key(&target)
        );
        assert!(entity(&mut app, engineer).is_some());
    }
}

#[test]
fn ra6_spy_refinery_steals_half_once_and_power_causes_only_blackout() {
    for (building, theft) in [("refinery", true), ("power_plant", false)] {
        let mut app = world();
        let spy = put(&mut app, P0, "spy", 9, 10);
        let target = put(&mut app, P1, building, 10, 10);
        tick(&mut app, vec![]);
        let before = credits(&app, P1);
        assert!(rejected(&interact(&mut app, spy, target)).is_empty());
        assert!(entity(&mut app, spy).is_none());
        assert_eq!(
            credits(&app, P1),
            if theft { before - before / 2 } else { before }
        );
        assert_eq!(
            credits(&app, P0),
            if theft { 10000 + before / 2 } else { 10000 }
        );
        let s = app.world().resource::<sim::SpecialState>();
        assert_eq!(s.outages.contains_key(&P1), !theft);
        if !theft {
            assert_eq!(
                s.outages[&P1] - app.world().resource::<sim::SimTick>().0,
                1332
            );
        }
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .infiltration,
            0
        );
    }
}

#[test]
fn ra6_tanya_c4_is_delayed_and_does_not_kill_on_entry() {
    let mut app = world();
    let tanya = put(&mut app, P0, "tanya", 9, 10);
    let target = put(&mut app, P1, "construction_yard", 10, 10);
    tick(&mut app, vec![]);
    let before = hp(&mut app, target);
    assert!(rejected(&interact(&mut app, tanya, target)).is_empty());
    assert!(entity(&mut app, tanya).is_some());
    assert!(hp(&mut app, target) > 0);
    let due = app.world().resource::<sim::SpecialState>().charges[&target].due;
    assert_eq!(due - app.world().resource::<sim::SimTick>().0, 35);
    while app.world().resource::<sim::SimTick>().0 < due {
        tick(&mut app, vec![]);
    }
    assert!(hp(&mut app, target) <= before);
    tick(&mut app, vec![]);
    assert!(entity(&mut app, target).is_none());
}

#[test]
fn ra6_desolator_emits_on_deploy_and_radiation_spares_buildings_and_immune_infantry() {
    let mut app = world();
    let deso = put(&mut app, P0, "desolator", 10, 10);
    let other = put(&mut app, P0, "desolator", 11, 10);
    let building = put(&mut app, P0, "power_plant", 10, 11);
    let victim = put(&mut app, P0, "gi", 11, 9);
    let immune_hp = hp(&mut app, other);
    let building_hp = hp(&mut app, building);
    tick(&mut app, vec![issue(P0, Command::Deploy { unit: deso })]);
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .len(),
        1
    );
    steps(&mut app, 42);
    assert!(entity(&mut app, victim).is_none());
    assert_eq!(hp(&mut app, other), immune_hp);
    assert_eq!(hp(&mut app, building), building_hp);
}

#[test]
fn ra6_terrorist_death_explodes_once_with_shipped_weapon() {
    let mut app = world();
    let bomber = put(&mut app, P0, "terrorist", 10, 10);
    let victim = put(&mut app, P0, "construction_yard", 11, 10);
    let before = hp(&mut app, victim);
    set_hp(&mut app, bomber, 0);
    tick(&mut app, vec![]);
    assert!(entity(&mut app, bomber).is_none());
    let after = hp(&mut app, victim);
    assert!(after < before);
    tick(&mut app, vec![]);
    assert_eq!(hp(&mut app, victim), after);
}

#[test]
fn ra6_temporal_sums_beams_freezes_and_releases_unharmed() {
    let mut app = world();
    let a = put(&mut app, P0, "chrono_legionnaire", 10, 10);
    let b = put(&mut app, P0, "chrono_legionnaire", 10, 11);
    let target = put(&mut app, P1, "construction_yard", 12, 10);
    let before = hp(&mut app, target);
    steps(&mut app, 2);
    let first = app.world().resource::<sim::SpecialState>().erasures[&target].remaining;
    assert_eq!(
        app.world().resource::<sim::SpecialState>().erasures[&target]
            .beams
            .len(),
        2
    );
    tick(&mut app, vec![]);
    assert_eq!(
        first - app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
        12
    );
    assert_eq!(hp(&mut app, target), before);
    let move_both = Command::Move {
        units: vec![a, b],
        target: CellPos::new(30, 30),
    };
    assert!(rejected(&tick(&mut app, vec![issue(P0, move_both)])).is_empty());
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&target)
    );
    assert_eq!(hp(&mut app, target), before);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .disabled
            .contains_key(&a)
    );
}

#[test]
fn ra6_yuri_releases_previous_and_releases_on_controller_death() {
    let mut app = world();
    let yuri = put(&mut app, P0, "yuri", 10, 10);
    let first = put(&mut app, P1, "conscript", 12, 10);
    steps(&mut app, 2);
    assert_eq!(owner(&mut app, first), Some(P0));
    let second = put(&mut app, P1, "conscript", 12, 11);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![yuri],
                target: second,
            },
        )],
    );
    steps(&mut app, 300);
    assert_eq!(owner(&mut app, second), Some(P0));
    assert_eq!(owner(&mut app, first), Some(P1));
    set_hp(&mut app, yuri, 0);
    tick(&mut app, vec![]);
    assert_eq!(owner(&mut app, second), Some(P1));
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .controlled
            .is_empty()
    );
}

#[test]
fn ra6_yuri_psi_pulse_and_undeploy_boundary() {
    let mut app = world();
    let yuri = put(&mut app, P0, "yuri", 10, 10);
    let victim = put(&mut app, P1, "gi", 11, 10);
    let immune = put(&mut app, P0, "yuri", 10, 11);
    let before = hp(&mut app, immune);
    tick(&mut app, vec![issue(P0, Command::Deploy { unit: yuri })]);
    assert!(entity(&mut app, victim).is_none());
    assert_eq!(hp(&mut app, immune), before);
    let until = app.world().resource::<sim::SpecialState>().deployed[&yuri];
    assert_eq!(until - app.world().resource::<sim::SimTick>().0, 199);
    while app.world().resource::<sim::SimTick>().0 < until {
        tick(&mut app, vec![]);
    }
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .deployed
            .contains_key(&yuri)
    );
    tick(&mut app, vec![]);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .deployed
            .contains_key(&yuri)
    );
}

#[test]
fn ra6_spy_disguise_default_copy_and_dog_detection() {
    let mut app = world();
    let spy = put(&mut app, P0, "spy", 10, 10);
    let infantry = put(&mut app, P1, "conscript", 11, 10);
    steps(&mut app, 2);
    let gi = defs(&app).unit_kind("gi").unwrap();
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .disguise_for(P1, spy),
        Some(gi)
    );
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .disguise_for(P0, spy),
        None
    );
    assert!(rejected(&interact(&mut app, spy, infantry)).is_empty());
    let conscript = defs(&app).unit_kind("conscript").unwrap();
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .disguise_for(P1, spy),
        Some(conscript)
    );
    // Public movement preserves the copied disguise; it does not reset it to the
    // default GI or clear it. Observe real cell arrival before adding the detector.
    let before_move = cell(&mut app, spy);
    let destination = CellPos::new(10, 12);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![spy],
                target: destination,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..400 {
        assert_eq!(
            app.world()
                .resource::<sim::SpecialState>()
                .disguise_for(P1, spy),
            Some(conscript)
        );
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
        assert!(
            app.world()
                .resource::<Players>()
                .iter()
                .all(|(_, p)| p.credits() >= 0)
        );
        let e = entity(&mut app, spy).unwrap();
        let mobile = app.world().get::<sim::Mobile>(e).unwrap();
        if mobile.cell == destination && mobile.step.is_none() {
            break;
        }
        let events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty(), "{events:?}");
    }
    assert_ne!(cell(&mut app, spy), before_move);
    assert_eq!(cell(&mut app, spy), destination);
    let e = entity(&mut app, spy).unwrap();
    assert!(app.world().get::<sim::Mobile>(e).unwrap().step.is_none());
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .disguise_for(P1, spy),
        Some(conscript)
    );
    let _dog = put(&mut app, P1, "attack_dog", 14, 10);
    tick(&mut app, vec![]);
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .disguise_for(P1, spy),
        None
    );
}

#[test]
fn ra6_dog_bite_kills_infantry_even_above_nominal_weapon_damage() {
    let mut app = world();
    let _dog = put(&mut app, P0, "attack_dog", 10, 10);
    let victim = put(&mut app, P1, "tesla_trooper", 11, 10);
    set_hp(&mut app, victim, 1000);
    steps(&mut app, 3);
    assert!(entity(&mut app, victim).is_none());
}

#[test]
fn ra6_each_spy_factory_and_lab_entry_sets_only_its_own_unlock() {
    for (building, flags) in [
        ("barracks", 1),
        ("war_factory", 2),
        ("tech_center", 4),
        ("soviet_battle_lab", 8),
    ] {
        let mut app = world();
        let target = put(&mut app, P1, building, 10, 10);
        let spy = put(&mut app, P0, "spy", 9, 10);
        tick(&mut app, vec![]);
        assert!(
            rejected(&interact(&mut app, spy, target)).is_empty(),
            "{building}"
        );
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .infiltration,
            flags,
            "{building}"
        );
        assert_eq!(credits(&app, P0), 10000);
        assert_eq!(credits(&app, P1), 10000);
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .outages
                .is_empty()
        );
        assert!(entity(&mut app, spy).is_none());
    }
}

#[test]
fn ra6_tesla_charge_two_low_power_three_overpowered_one_with_power() {
    for (powered, count, expected) in [
        (false, 1, None),
        (false, 2, Some("coil_bolt")),
        (false, 3, Some("op_coil_bolt")),
        (true, 1, Some("op_coil_bolt")),
    ] {
        let mut app = world();
        let coil = put(&mut app, P0, "tesla_coil", 10, 10);
        if powered {
            put(&mut app, P0, "nuclear_reactor", 2, 2);
        }
        for i in 0..count {
            put(&mut app, P0, "tesla_trooper", 9, 9 + i);
        }
        put(&mut app, P1, "construction_yard", 15, 10);
        let mut events = Vec::new();
        for _ in 0..3 {
            events.extend(tick(&mut app, vec![]));
        }
        let fired: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let SimEvent::WeaponFired {
                    attacker, weapon, ..
                } = e
                {
                    (*attacker == coil).then_some(*weapon)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            fired.first().copied(),
            expected.and_then(|id| defs(&app).weapon_kind(id)),
            "power {powered}, troops {count}: {events:?}"
        );
    }
}

#[test]
fn ra6_specialists_embark_and_unload_preserving_identity_health_and_rank() {
    for name in [
        "gi",
        "engineer",
        "spy",
        "tanya",
        "crazy_ivan",
        "chrono_legionnaire",
        "yuri",
        "desolator",
        "terrorist",
    ] {
        let mut app = world();
        let carrier = put(&mut app, P0, "amphibious_transport", 10, 10);
        let passenger = put(&mut app, P0, name, 9, 10);
        tick(&mut app, vec![]);
        set_hp(&mut app, passenger, 42);
        let promoted = sim::promote(app.world_mut(), passenger, sim::VETERAN);
        let before = hp(&mut app, passenger);
        let events = interact(&mut app, passenger, carrier);
        assert!(rejected(&events).is_empty(), "{name}: {events:?}");
        assert!(entity(&mut app, passenger).is_none(), "{name}");
        assert_eq!(
            app.world().resource::<sim::SpecialState>().cargo[&carrier][0].id,
            passenger
        );
        let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: carrier })]);
        assert!(rejected(&events).is_empty(), "{name}: {events:?}");
        assert_eq!(hp(&mut app, passenger), before, "{name}");
        if promoted {
            let e = entity(&mut app, passenger).unwrap();
            assert_eq!(
                app.world().get::<sim::Experience>(e).unwrap().rank,
                sim::VETERAN
            );
        }
    }
}

#[test]
fn ra6_radiation_diagonal_decay_edge_expiry_and_reirradiation() {
    let mut app = world();
    let deso = put(&mut app, P0, "desolator", 10, 10);
    tick(&mut app, vec![issue(P0, Command::Deploy { unit: deso })]);
    let site = app.world().resource::<sim::SpecialState>().radiation_sites[0].clone();
    let diagonal = CellPos::new(11, 11);
    assert_eq!(site.cell_level(site.born, 120, diagonal), 429);
    assert_eq!(site.cell_level(site.born + 119, 120, diagonal), 429);
    assert_eq!(site.cell_level(site.born + 120, 120, diagonal), 344);
    assert_eq!(site.cell_level(site.born + 240, 120, diagonal), 259);
    assert_eq!(site.cell_level(site.born, 120, CellPos::new(20, 10)), 0);
    assert_eq!(site.cell_level(site.born, 120, CellPos::new(21, 10)), 0);
    assert_eq!(site.cell_level(site.born + site.duration, 120, diagonal), 0);
    assert_eq!(site.duration, 667);
    while app.world().resource::<sim::SimTick>().0 <= site.born + 480 {
        tick(&mut app, vec![]);
    }
    let current = &app.world().resource::<sim::SpecialState>().radiation_sites[0];
    assert!(current.born > site.born, "redeploy below one third");
    assert_eq!(current.level, 500, "re-irradiation is capped");
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .len(),
        1,
        "same-spread site is reused"
    );
}

#[test]
fn ra6_stolen_units_require_lab_flags_and_proper_barracks_and_prime_limit_is_same_tick() {
    for (unit, lab, barracks) in [
        ("chrono_commando", "tech_center", "barracks"),
        ("psi_commando", "soviet_battle_lab", "barracks"),
        ("chrono_ivan", "tech_center", "soviet_barracks"),
        ("yuri_prime", "soviet_battle_lab", "soviet_barracks"),
    ] {
        let mut app = world();
        put(&mut app, P0, barracks, 2, 2);
        let target = put(&mut app, P1, lab, 10, 10);
        let spy = put(&mut app, P0, "spy", 9, 10);
        let kind = defs(&app).unit_kind(unit).unwrap();
        let queue = issue(
            P0,
            Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            },
        );
        let events = tick(&mut app, vec![queue.clone()]);
        assert_eq!(rejected(&events).len(), 1, "{unit} needs stolen tech");
        assert!(rejected(&interact(&mut app, spy, target)).is_empty());
        let events = tick(&mut app, vec![queue.clone(), queue]);
        assert_eq!(
            rejected(&events).len(),
            usize::from(unit == "yuri_prime"),
            "{unit}: {events:?}"
        );
        if unit == "yuri_prime" {
            assert!(events.iter().any(|e| matches!(
                e,
                SimEvent::CommandRejected {
                    reason: sim::RejectReason::BuildLimitReached { .. },
                    ..
                }
            )));
        }
        let producer = defs(&app)
            .production(ProductionItem::Unit(kind))
            .unwrap()
            .producer;
        assert_eq!(defs(&app).building(producer).unwrap().id, barracks);
    }
}

#[test]
fn ra6_control_chain_returns_to_original_owner_after_second_controller_dies() {
    let mut app = world();
    let p2 = PlayerId(2);
    app.world_mut()
        .resource_mut::<Players>()
        .add(p2, 10000)
        .unwrap();
    let first = put(&mut app, P1, "yuri", 10, 10);
    let target = put(&mut app, P0, "engineer", 12, 10);
    steps(&mut app, 2);
    assert_eq!(owner(&mut app, target), Some(P1));
    let second = put(&mut app, p2, "yuri", 14, 10);
    steps(&mut app, 2);
    assert_eq!(owner(&mut app, target), Some(p2));
    assert_eq!(
        app.world().resource::<sim::SpecialState>().controlled[&target],
        (second, P0)
    );
    set_hp(&mut app, first, 0);
    set_hp(&mut app, second, 0);
    tick(&mut app, vec![]);
    assert_eq!(owner(&mut app, target), Some(P0));
}

#[test]
fn ra6_veteran_infiltration_applies_to_paid_production_not_free_spawns() {
    for (unit, factory) in [("gi", "barracks"), ("grizzly", "war_factory")] {
        let mut app = world();
        put(&mut app, P0, "construction_yard", 1, 1);
        put(&mut app, P0, "power_plant", 6, 1);
        put(&mut app, P0, "refinery", 10, 1);
        put(&mut app, P0, "barracks", 15, 1);
        put(&mut app, P0, "war_factory", 19, 1);
        let target = put(&mut app, P1, factory, 10, 10);
        let spy = put(&mut app, P0, "spy", 9, 10);
        tick(&mut app, vec![]);
        assert!(rejected(&interact(&mut app, spy, target)).is_empty());
        let free = put(&mut app, P0, unit, 30, 30);
        tick(&mut app, vec![]);
        let e = entity(&mut app, free).unwrap();
        assert_eq!(
            app.world().get::<sim::Experience>(e).unwrap().rank,
            0,
            "scenario/crate-style spawn stays rookie"
        );
        let kind = defs(&app).unit_kind(unit).unwrap();
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::QueueProduction {
                    item: ProductionItem::Unit(kind),
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{unit}: {events:?}");
        let mut produced = None;
        for _ in 0..4000 {
            let events = tick(&mut app, vec![]);
            if let Some(id) = events.iter().find_map(|e| {
                if let SimEvent::UnitSpawned { player, unit, .. } = e {
                    (*player == P0).then_some(*unit)
                } else {
                    None
                }
            }) {
                produced = Some(id);
                break;
            }
        }
        let produced = produced.expect("paid unit completes");
        let e = entity(&mut app, produced).unwrap();
        assert_eq!(
            app.world().get::<sim::Experience>(e).unwrap().rank,
            sim::VETERAN,
            "{unit}"
        );
    }
}

#[test]
fn ra6_spy_superweapon_resets_charge_and_radar_resets_only_old_shroud() {
    let mut app = world();
    put(&mut app, P1, "nuclear_reactor", 2, 2);
    let target = put(&mut app, P1, "nuclear_silo", 10, 10);
    let spy = put(&mut app, P0, "spy", 9, 10);
    tick(&mut app, vec![]);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(target, 300);
    assert!(rejected(&interact(&mut app, spy, target)).is_empty());
    assert_eq!(
        app.world().resource::<sim::SpecialState>().charge[&target],
        1,
        "reset in Commands, one Economy charging tick"
    );
    assert_eq!(credits(&app, P0), 10000);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .outages
            .is_empty()
    );

    for satellite in [false, true] {
        let mut app = world();
        put(&mut app, P1, "nuclear_reactor", 2, 2);
        if satellite {
            put(&mut app, P1, "spy_satellite", 5, 2);
        }
        let scout = put(&mut app, P1, "engineer", 35, 35);
        let radar = put(&mut app, P1, "radar_tower", 10, 10);
        let spy = put(&mut app, P0, "spy", 9, 10);
        tick(&mut app, vec![]);
        set_hp(&mut app, scout, 0);
        tick(&mut app, vec![]);
        assert!(
            app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_explored(CellPos::new(35, 35))
        );
        assert!(rejected(&interact(&mut app, spy, radar)).is_empty());
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_explored(CellPos::new(35, 35)),
            satellite
        );
        assert!(
            app.world()
                .resource::<Players>()
                .get(P1)
                .unwrap()
                .vision()
                .is_explored(CellPos::new(10, 10))
        );
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .infiltration,
            0
        );
    }
}

fn ra6_bridge_world(destructible: bool) -> (App, SimId, SimId) {
    let mut app = world();
    let mut looks = vec![0; 48 * 48];
    {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 0..48 {
            for x in 22..=24 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
        for x in 22..=24 {
            grid.set_terrain(
                CellPos::new(x, 24),
                Cell {
                    buildable: false,
                    ..OPEN
                },
            );
            looks[24 * 48 + x as usize] = 8 << 4;
        }
        for (x, y) in [(21, 22), (25, 22)] {
            looks[y * 48 + x] = 15 << 4;
        }
        grid.mark_world_features(&looks);
    }
    sim::install_world_objects(app.world_mut(), destructible);
    let bridge = *app
        .world()
        .resource::<sim::Bridges>()
        .bridges
        .keys()
        .next()
        .expect("bridge");
    let hut = app
        .world()
        .resource::<Grid>()
        .building(CellPos::new(21, 22))
        .expect("west hut");
    (app, bridge, hut)
}

#[test]
fn ra6_tanya_c4_hut_collapses_bridge_only_when_enabled() {
    for enabled in [true, false] {
        let (mut app, bridge, hut) = ra6_bridge_world(enabled);
        let mut rules = app.world().resource::<sim::MatchState>().rules();
        rules.bridge_destruction = enabled;
        app.world_mut()
            .resource_mut::<sim::MatchState>()
            .set_rules(rules);
        let tanya = put(&mut app, P0, "tanya", 20, 22);
        tick(&mut app, vec![]);
        let events = interact(&mut app, tanya, hut);
        assert_eq!(rejected(&events).len(), usize::from(!enabled), "{events:?}");
        steps(&mut app, 40);
        assert_eq!(
            app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed,
            enabled
        );
        assert!(entity(&mut app, hut).is_some(), "immune hut survives");
    }
}

#[test]
fn ra6_tanya_swims_and_sapper_is_exclusive_to_naval_targets() {
    let mut app = world();
    {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 0..48 {
            for x in 0..48 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
    }
    let tanya = put(&mut app, P0, "tanya", 10, 10);
    let ship = put(&mut app, P1, "amphibious_transport", 11, 10);
    let mut events = tick(&mut app, vec![]);
    events.extend(tick(&mut app, vec![]));
    let sapper = defs(&app).weapon_kind("sapper").unwrap();
    assert!(events.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,target,weapon}if *attacker==tanya&&*target==ship&&*weapon==sapper)));
    assert!(entity(&mut app, ship).is_none());
    assert!(
        rejected(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![tanya],
                    target: CellPos::new(13, 10)
                }
            )]
        ))
        .is_empty()
    );
    steps(&mut app, 80);
    assert_eq!(cell(&mut app, tanya), CellPos::new(13, 10));
    let definitions = defs(&app);
    let k = definitions.unit_kind("tanya").unwrap();
    let terms = definitions.unit_combat(k).unwrap();
    let traits = sim::TargetTraits {
        armour: sim::ArmourClass(
            definitions
                .armour_classes()
                .iter()
                .position(|s| s == "heavy")
                .unwrap() as u16,
        ),
        ..Default::default()
    };
    assert_ne!(
        sim::weapon_against(&definitions, &terms, terms.weapons(false), &traits).map(|(k, _)| k),
        Some(sapper)
    );
}

#[test]
fn ra6_radiation_overlap_caps_damage_and_empty_sites_expire() {
    let mut app = world();
    let left = put(&mut app, P0, "desolator", 10, 10);
    let right = put(&mut app, P0, "desolator", 18, 10);
    let tank = put(&mut app, P0, "grizzly", 14, 10);
    let before = hp(&mut app, tank);
    tick(
        &mut app,
        vec![
            issue(P0, Command::Deploy { unit: left }),
            issue(P0, Command::Deploy { unit: right }),
        ],
    );
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .len(),
        2
    );
    assert_eq!(
        before - hp(&mut app, tank),
        10,
        "summed level capped500, factor0.2, heavy10%"
    );
    tick(
        &mut app,
        vec![
            issue(P0, Command::Deploy { unit: left }),
            issue(P0, Command::Deploy { unit: right }),
        ],
    );
    steps(&mut app, 700);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .is_empty()
    );
}

#[test]
fn ra6_temporal_removal_has_no_terrorist_death_blast_and_frozen_actor_cannot_move() {
    let mut app = world();
    let chrono = put(&mut app, P0, "chrono_legionnaire", 10, 10);
    let terrorist = put(&mut app, P1, "terrorist", 12, 10);
    let observer = put(&mut app, P1, "construction_yard", 13, 10);
    let before = hp(&mut app, observer);
    steps(&mut app, 2);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&terrorist)
    );
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![terrorist],
                target: CellPos::new(20, 20),
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    for _ in 0..400 {
        if entity(&mut app, terrorist).is_none() {
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(entity(&mut app, terrorist).is_none());
    assert_eq!(hp(&mut app, observer), before);
    assert!(entity(&mut app, chrono).is_some());
}

#[test]
fn ra6_specialist_replay_and_each_new_runtime_state_are_hashed() {
    fn replay() -> Vec<u64> {
        let mut app = world();
        let gi = put(&mut app, P0, "gi", 8, 8);
        let deso = put(&mut app, P0, "desolator", 25, 25);
        let spy = put(&mut app, P0, "spy", 9, 15);
        let lab = put(&mut app, P1, "tech_center", 10, 15);
        tick(&mut app, vec![]);
        tick(
            &mut app,
            vec![
                issue(P0, Command::Deploy { unit: gi }),
                issue(P0, Command::Deploy { unit: deso }),
                issue(
                    P0,
                    Command::Interact {
                        unit: spy,
                        target: lab,
                    },
                ),
            ],
        );
        (0..80)
            .map(|_| {
                tick(&mut app, vec![]);
                sim::state_hash(app.world_mut())
            })
            .collect()
    }
    assert_eq!(replay(), replay());
    let mut app = world();
    let id = put(&mut app, P0, "gi", 8, 8);
    tick(&mut app, vec![]);
    let baseline = app.world().resource::<sim::SpecialState>().clone();
    let hash = sim::state_hash(app.world_mut());
    for mutation in 0..5 {
        app.world_mut().insert_resource(baseline.clone());
        {
            let mut s = app.world_mut().resource_mut::<sim::SpecialState>();
            match mutation {
                0 => {
                    s.deployed.insert(id, 1);
                }
                1 => {
                    s.coil_charge.insert(id, 1);
                }
                2 => {
                    s.temporal_removed.insert(id);
                }
                3 => {
                    s.death_detonated.insert(id);
                }
                _ => {
                    s.eruption_next.insert(id, 1);
                }
            }
        }
        assert_ne!(sim::state_hash(app.world_mut()), hash, "state{mutation}");
    }
}

#[test]
fn ra6_yuri_prime_limit_counts_cargo_and_releases_on_carrier_death() {
    let mut app = world();
    put(&mut app, P0, "soviet_barracks", 2, 2);
    let lab = put(&mut app, P1, "soviet_battle_lab", 10, 10);
    let spy = put(&mut app, P0, "spy", 9, 10);
    tick(&mut app, vec![]);
    assert!(rejected(&interact(&mut app, spy, lab)).is_empty());
    let prime = put(&mut app, P0, "yuri_prime", 19, 20);
    let carrier = put(&mut app, P0, "amphibious_transport", 20, 20);
    let kind = defs(&app).unit_kind("yuri_prime").unwrap();
    let queue = issue(
        P0,
        Command::QueueProduction {
            item: ProductionItem::Unit(kind),
        },
    );
    assert_eq!(rejected(&tick(&mut app, vec![queue.clone()])).len(), 1);
    assert!(rejected(&interact(&mut app, prime, carrier)).is_empty());
    assert_eq!(
        rejected(&tick(&mut app, vec![queue.clone()])).len(),
        1,
        "cargo counts"
    );
    set_hp(&mut app, carrier, 0);
    assert!(
        rejected(&tick(&mut app, vec![queue])).is_empty(),
        "dead carrier and passengers no longer count"
    );
}

#[test]
fn ra6_factory_training_never_promotes_a_paid_ship() {
    let mut app = world();
    {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 20..48 {
            for x in 20..48 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
    }
    put(&mut app, P0, "construction_yard", 1, 1);
    put(&mut app, P0, "power_plant", 6, 1);
    put(&mut app, P0, "refinery", 10, 1);
    put(&mut app, P0, "naval_shipyard", 25, 20);
    let factory = put(&mut app, P1, "war_factory", 10, 10);
    let spy = put(&mut app, P0, "spy", 9, 10);
    tick(&mut app, vec![]);
    assert!(rejected(&interact(&mut app, spy, factory)).is_empty());
    let kind = defs(&app).unit_kind("destroyer").unwrap();
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let mut produced = None;
    for _ in 0..4000 {
        let events = tick(&mut app, vec![]);
        if let Some(id) = events.iter().find_map(|e| {
            if let SimEvent::UnitSpawned { player, unit, .. } = e {
                (*player == P0).then_some(*unit)
            } else {
                None
            }
        }) {
            produced = Some(id);
            break;
        }
    }
    let e = entity(&mut app, produced.expect("ship completes")).unwrap();
    assert_eq!(app.world().get::<sim::Experience>(e).unwrap().rank, 0);
}

#[test]
fn ra6_repeat_psi_deploy_cannot_bypass_weapon_reload() {
    let mut app = world();
    let yuri = put(&mut app, P0, "yuri", 10, 10);
    let target = put(&mut app, P1, "engineer", 11, 10);
    set_hp(&mut app, target, 1000);
    let events = tick(
        &mut app,
        vec![
            issue(P0, Command::Deploy { unit: yuri }),
            issue(P0, Command::Deploy { unit: yuri }),
            issue(P0, Command::Deploy { unit: yuri }),
        ],
    );
    assert_eq!(rejected(&events).len(), 1, "{events:?}");
    assert!(hp(&mut app, target) >= 750);
}
#[test]
fn ra6_specialists_enter_friendly_hospital_instead_of_infiltrating_or_bombing() {
    for kind in ["tanya", "spy", "crazy_ivan", "engineer"] {
        let mut app = world();
        let hospital = put(&mut app, P0, "hospital", 10, 10);
        let unit = put(&mut app, P0, kind, 9, 10);
        set_hp(&mut app, unit, 20);
        tick(&mut app, vec![]);
        let events = interact(&mut app, unit, hospital);
        assert!(rejected(&events).is_empty(), "{kind}: {events:?}");
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .cargo
                .get(&hospital)
                .is_some_and(|p| p.iter().any(|p| p.id == unit)),
            "{kind}"
        );
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .charges
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .infiltration,
            0
        );
    }
}

#[test]
fn ra6_chrono_infantry_distance_delay_and_stolen_abilities_are_independent() {
    for kind in ["chrono_legionnaire", "chrono_commando", "chrono_ivan"] {
        for (distance, delay) in [(1, 21), (12, 85)] {
            let mut app = world();
            let unit = put(&mut app, P0, kind, 10, 10);
            let e = entity(&mut app, unit).unwrap();
            app.world_mut().get_mut::<sim::Position>(e).unwrap().0 =
                sim::SubPos::centre_of(CellPos::new(10, 10));
            let before = app.world().resource::<sim::SimTick>().0;
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Move {
                        units: vec![unit],
                        target: CellPos::new(10 + distance, 10),
                    },
                )],
            );
            assert!(rejected(&events).is_empty(), "{kind}: {events:?}");
            assert_eq!(cell(&mut app, unit), CellPos::new(10 + distance, 10));
            assert_eq!(
                app.world().resource::<sim::SpecialState>().disabled[&unit],
                before + delay,
                "{kind}"
            );
        }
    }
    for (kind, planted) in [
        ("chrono_commando", false),
        ("psi_commando", true),
        ("chrono_ivan", true),
    ] {
        let mut app = world();
        let unit = put(&mut app, P0, kind, 9, 10);
        let target = put(&mut app, P1, "construction_yard", 10, 10);
        tick(&mut app, vec![]);
        interact(&mut app, unit, target);
        let charge = app
            .world()
            .resource::<sim::SpecialState>()
            .charges
            .get(&target);
        assert_eq!(charge.is_some(), planted, "{kind}");
        if let Some(charge) = charge {
            assert_eq!(charge.c4, kind == "psi_commando");
        }
    }
}

#[test]
fn ra6_elite_gi_uses_elite_para_and_elite_temporal_has_double_rate() {
    let mut app = world();
    let gi = put(&mut app, P0, "gi", 10, 10);
    assert!(sim::promote(app.world_mut(), gi, sim::ELITE));
    let target = put(&mut app, P1, "construction_yard", 12, 10);
    let mut events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: gi })]);
    events.extend(tick(&mut app, vec![]));
    let elite = defs(&app).weapon_kind("para_e").unwrap();
    assert!(events.iter().any(|event| matches!(event, SimEvent::WeaponFired {attacker, target: t, weapon} if *attacker == gi && *t == target && *weapon == elite)));
    let mut app = world();
    let chrono = put(&mut app, P0, "chrono_legionnaire", 10, 10);
    assert!(sim::promote(app.world_mut(), chrono, sim::ELITE));
    let target = put(&mut app, P1, "construction_yard", 12, 10);
    steps(&mut app, 2);
    let remaining = app.world().resource::<sim::SpecialState>().erasures[&target].remaining;
    tick(&mut app, vec![]);
    assert_eq!(
        remaining - app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
        12
    );
}

/// Exact HARV/CMIN current ordinary damage and Ivan eligibility. Original manual
/// berserk/death/time-bomb controls are absent; Tanya has no land-unit C4 action here.
#[test]
fn ra9_stock_miners_take_real_shell_and_ivan_damage_but_refuse_land_c4_interact() {
    for name in ["war_miner", "chrono_miner"] {
        for route in ["shot", "ivan", "tanya"] {
            let mut app = world();
            let actor_name = match route {
                "shot" => "grizzly",
                "ivan" => "crazy_ivan",
                _ => "tanya",
            };
            let actor = put(&mut app, P0, actor_name, 8, 8);
            tick(&mut app, vec![]); // source alone naturally observes the later target cell
            assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
            assert!(
                app.world()
                    .resource::<Players>()
                    .iter()
                    .all(|(_, p)| p.credits() >= 0)
            );
            let target = put(&mut app, P1, name, 9, 8);
            let e = entity(&mut app, target).unwrap();
            // Only the measuring target is passive; actual source definitions/state stay stock.
            app.world_mut().entity_mut(e).remove::<sim::Weapon>();
            let definitions = defs(&app);
            let kind = definitions.unit_kind(name).unwrap();
            let terms = definitions.unit_combat(kind).unwrap();
            let meta = definitions.metadata(ProductionItem::Unit(kind)).unwrap();
            assert_eq!(terms.hit_points, 1000);
            assert_eq!(terms.armour, definitions.armour_class("medium").unwrap());
            assert!(meta.bombable && !meta.immune && meta.self_healing);
            let before = hp(&mut app, target);
            let command_tick = app.world().resource::<sim::SimTick>().0;
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    if route == "shot" {
                        Command::Attack {
                            units: vec![actor],
                            target,
                        }
                    } else {
                        Command::Interact {
                            unit: actor,
                            target,
                        }
                    },
                )],
            );
            assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
            assert!(
                app.world()
                    .resource::<Players>()
                    .iter()
                    .all(|(_, p)| p.credits() >= 0)
            );
            if route == "tanya" {
                assert_eq!(rejected(&events).len(), 1, "{name}: {events:?}");
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .charges
                        .contains_key(&target)
                );
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .cargo
                        .values()
                        .flatten()
                        .any(|p| p.id == actor || p.id == target)
                );
                assert_eq!(owner(&mut app, actor), Some(P0));
                assert_eq!(owner(&mut app, target), Some(P1));
            } else if route == "shot" {
                assert!(rejected(&events).is_empty(), "{name}: {events:?}");
                let primary = definitions
                    .unit_combat(definitions.unit_kind("grizzly").unwrap())
                    .unwrap()
                    .weapon
                    .unwrap();
                let gun = definitions.weapon(primary).unwrap();
                let warhead = definitions.warhead(gun.warhead).unwrap();
                assert_eq!(
                    (gun.damage, warhead.versus[terms.armour.0 as usize]),
                    (65, 100)
                );
                assert!(events.iter().any(|e| matches!(e,
                    SimEvent::WeaponFired { attacker, target: t, weapon }
                        if *attacker == actor && *t == target && *weapon == primary)));
                let mut hit = hp(&mut app, target) < before;
                for _ in 0..20 {
                    if hit {
                        break;
                    }
                    tick(&mut app, vec![]);
                    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
                    assert!(
                        app.world()
                            .resource::<Players>()
                            .iter()
                            .all(|(_, p)| p.credits() >= 0)
                    );
                    hit = hp(&mut app, target) < before;
                }
                assert!(hit, "{name}: real in-range shell must arrive");
                assert_eq!(before - hp(&mut app, target), 65);
            } else {
                assert!(rejected(&events).is_empty(), "{name}: {events:?}");
                let charge = app.world().resource::<sim::SpecialState>().charges[&target].clone();
                assert!(!charge.c4);
                assert_eq!((charge.source, charge.damage), (actor, 400));
                let rules = definitions.rules().unwrap();
                assert_eq!(
                    (rules.combat.ivan_damage, rules.combat.ivan_delay),
                    (400, 600)
                );
                assert_eq!(charge.due - command_tick, 600);
                let effect = definitions.effect(sim::Effect::IvanBomb).unwrap();
                let warhead = definitions.warhead(effect.warhead).unwrap();
                assert_eq!(warhead.versus[terms.armour.0 as usize], 100);
                while app.world().resource::<sim::SimTick>().0 < charge.due {
                    assert_eq!(hp(&mut app, target), before, "no early charge at {name}");
                    tick(&mut app, vec![]);
                    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
                    assert!(
                        app.world()
                            .resource::<Players>()
                            .iter()
                            .all(|(_, p)| p.credits() >= 0)
                    );
                }
                assert_eq!(hp(&mut app, target), before);
                let events = tick(&mut app, vec![]);
                assert!(events.iter().any(|e| matches!(e,
                    SimEvent::Impact { warhead: h, at, .. }
                        if *h == effect.warhead && *at == sim::SubPos::centre_of(CellPos::new(9, 8)))));
                assert_eq!(before - hp(&mut app, target), 400);
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .charges
                        .contains_key(&target)
                );
                assert_eq!(owner(&mut app, target), Some(P1));
            }
            assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
            assert!(
                app.world()
                    .resource::<Players>()
                    .iter()
                    .all(|(_, p)| p.credits() >= 0)
            );
        }
    }
}
