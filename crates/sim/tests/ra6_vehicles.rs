//! RA6 vehicle and strategic acceptance on shipped data through commands and the simulation schedule.
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

fn attack(app: &mut App, a: SimId, b: SimId) -> Vec<SimEvent> {
    tick(
        app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![a],
                target: b,
            },
        )],
    )
}
fn charge(app: &mut App, id: SimId) {
    let d = defs(app);
    let e = entity(app, id).unwrap();
    let b = app.world().get::<sim::Building>(e).unwrap();
    let cooldown = d
        .metadata(ProductionItem::Building(b.kind))
        .unwrap()
        .cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(id, cooldown);
}
fn power(app: &mut App) {
    put(app, P0, "nuclear_reactor", 0, 0);
    put(app, P1, "nuclear_reactor", 43, 43);
}
#[test]
fn ra6_drone_enters_then_ticks_and_exits_only_on_its_own_kill() {
    let mut app = world();
    let drone = put(&mut app, P0, "terror_drone", 8, 8);
    let host = put(&mut app, P1, "rhino", 9, 8);
    steps(&mut app, 1);
    attack(&mut app, drone, host);
    assert!(entity(&mut app, drone).is_none());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .contains_key(&host)
    );
    let before = hp(&mut app, host);
    steps(&mut app, 79);
    assert_eq!(hp(&mut app, host), before);
    steps(&mut app, 1);
    assert_eq!(hp(&mut app, host), before - 50);
    set_hp(&mut app, host, 50);
    steps(&mut app, 80);
    assert!(entity(&mut app, host).is_none());
    assert!(entity(&mut app, drone).is_some());
    let mut app = world();
    let drone = put(&mut app, P0, "terror_drone", 8, 8);
    let host = put(&mut app, P1, "rhino", 9, 8);
    steps(&mut app, 1);
    attack(&mut app, drone, host);
    set_hp(&mut app, host, 0);
    steps(&mut app, 1);
    assert!(entity(&mut app, drone).is_none());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .is_empty()
    );
}
#[test]
fn ra6_drone_infantry_bite_and_protected_vehicle_negative() {
    let mut app = world();
    let a = put(&mut app, P0, "terror_drone", 8, 8);
    let b = put(&mut app, P1, "conscript", 9, 8);
    steps(&mut app, 1);
    attack(&mut app, a, b);
    assert!(entity(&mut app, b).is_none());
    assert!(entity(&mut app, a).is_some());
    let b = put(&mut app, P1, "rhino", 9, 8);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .protected
        .insert(b, u64::MAX);
    steps(&mut app, 80);
    attack(&mut app, a, b);
    assert!(entity(&mut app, a).is_some());
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .contains_key(&b)
    );
}
#[test]
fn ra6_ifv_uses_reference_modes_and_restores_empty_missiles() {
    for (passenger, mode) in [
        ("attack_dog", 0),
        ("engineer", 1),
        ("gi", 2),
        ("flak_trooper", 3),
        ("tanya", 4),
        ("sniper", 5),
        ("tesla_trooper", 6),
        ("crazy_ivan", 7),
        ("yuri", 8),
        ("desolator", 9),
        ("chrono_legionnaire", 10),
        ("terrorist", 11),
    ] {
        let mut app = world();
        let ifv = put(&mut app, P0, "ifv", 8, 8);
        let p = put(&mut app, P0, passenger, 9, 8);
        steps(&mut app, 1);
        let events = interact(&mut app, p, ifv);
        assert!(rejected(&events).is_empty(), "{passenger}: {events:?}");
        let d = defs(&app);
        let m = d
            .metadata(ProductionItem::Unit(d.unit_kind("ifv").unwrap()))
            .unwrap();
        let e = entity(&mut app, ifv).unwrap();
        assert_eq!(
            app.world().get::<sim::Weapon>(e).unwrap().kind,
            m.ifv_weapons[mode],
            "{passenger}"
        );
        tick(&mut app, vec![issue(P0, Command::Deploy { unit: ifv })]);
        assert_eq!(
            app.world().get::<sim::Weapon>(e).unwrap().kind,
            m.ifv_weapons[0]
        );
        assert_eq!(owner(&mut app, p), Some(P0));
    }
}
#[test]
fn ra6_reactor_chain_radiation_and_temporal_suppression() {
    let mut app = world();
    let r = put(&mut app, P0, "nuclear_reactor", 8, 8);
    let truck = put(&mut app, P1, "demolition_truck", 11, 8);
    set_hp(&mut app, truck, 1);
    set_hp(&mut app, r, 0);
    steps(&mut app, 1);
    assert!(entity(&mut app, truck).is_none());
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .is_empty()
    );
    let mut app = world();
    let r = put(&mut app, P1, "nuclear_reactor", 8, 8);
    let c = put(&mut app, P0, "chrono_legionnaire", 6, 8);
    steps(&mut app, 1);
    attack(&mut app, c, r);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .erasures
        .get_mut(&r)
        .unwrap()
        .remaining = 1;
    steps(&mut app, 1);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .is_empty()
    );
}
#[test]
fn ra6_chronosphere_square_moves_both_owners_kills_organic_and_preserves_offsets() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "chronosphere", 3, 3);
    let a = put(&mut app, P0, "grizzly", 14, 14);
    let b = put(&mut app, P1, "rhino", 15, 15);
    let c = put(&mut app, P0, "gi", 13, 14);
    let outside = put(&mut app, P0, "grizzly", 16, 14);
    put(&mut app, P0, "spy_satellite", 3, 8);
    steps(&mut app, 2);
    let teleporter = put(&mut app, P0, "chrono_legionnaire", 13, 13);
    charge(&mut app, sw);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sw,
                source: CellPos::new(14, 14),
                target: CellPos::new(30, 30),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(cell(&mut app, a), CellPos::new(30, 30));
    assert_eq!(cell(&mut app, b), CellPos::new(31, 31));
    assert_eq!(cell(&mut app, outside), CellPos::new(16, 14));
    assert!(entity(&mut app, c).is_none());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .disabled
            .contains_key(&a)
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    assert_eq!(cell(&mut app, teleporter), CellPos::new(29, 29));
    steps(&mut app, 79);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .disabled
            .contains_key(&a)
    );
    steps(&mut app, 1);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .disabled
            .contains_key(&a)
    );
}
#[test]
fn ra6_nuclear_missile_delays_200_ticks_and_radiates() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "nuclear_silo", 3, 3);
    let target = put(&mut app, P1, "rhino", 20, 20);
    put(&mut app, P0, "spy_satellite", 3, 8);
    steps(&mut app, 2);
    charge(&mut app, sw);
    let before = hp(&mut app, target);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    assert!(rejected(&events).is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::SuperweaponLaunched { player: P0, .. }))
    );
    assert_eq!(hp(&mut app, target), before);
    steps(&mut app, 199);
    assert_eq!(hp(&mut app, target), before);
    steps(&mut app, 1);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .is_empty()
    );
    assert!(entity(&mut app, target).is_none());
}

#[test]
fn ra6_drone_blocked_escape_retries_with_identity_hp_and_experience() {
    let mut app = world();
    let drone = put(&mut app, P0, "terror_drone", 8, 8);
    let host = put(&mut app, P1, "rhino", 9, 8);
    steps(&mut app, 1);
    set_hp(&mut app, drone, 77);
    attack(&mut app, drone, host);
    for y in 7..=9 {
        for x in 8..=10 {
            if (x, y) != (9, 8) {
                let id = put(&mut app, P1, "grizzly", x, y);
                let e = entity(&mut app, id).unwrap();
                app.world_mut().entity_mut(e).remove::<sim::Weapon>();
            }
        }
    }
    set_hp(&mut app, host, 50);
    steps(&mut app, 80);
    assert!(entity(&mut app, host).is_none());
    assert!(entity(&mut app, drone).is_none());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .get(&host)
            .unwrap()
            .escape
            .is_some()
    );
    // Once released it must not immediately choose another adjacent enemy host.
    let blockers: Vec<_> = {
        let w = app.world_mut();
        w.query::<(bevy_ecs::entity::Entity, &sim::Owner)>()
            .iter(w)
            .filter(|(_, o)| o.0 == P1)
            .map(|(e, _)| e)
            .collect()
    };
    for e in blockers {
        app.world_mut().entity_mut(e).insert(sim::Owner(P0));
    }
    let blocker = app
        .world()
        .resource::<Grid>()
        .occupant(CellPos::new(8, 7))
        .unwrap();
    set_hp(&mut app, blocker, 0);
    steps(&mut app, 2);
    assert_eq!(hp(&mut app, drone), 77);
    assert_eq!(owner(&mut app, drone), Some(P0));
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .is_empty()
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
#[test]
fn ra6_repair_ifv_reaches_diagonal_but_not_two_cells_and_obeys_reload() {
    let mut app = world();
    let ifv = put(&mut app, P0, "ifv", 8, 8);
    let engineer = put(&mut app, P0, "engineer", 7, 8);
    let near = put(&mut app, P0, "grizzly", 9, 9);
    let far = put(&mut app, P0, "grizzly", 10, 8);
    steps(&mut app, 1);
    interact(&mut app, engineer, ifv);
    set_hp(&mut app, near, 100);
    set_hp(&mut app, far, 100);
    steps(&mut app, 1);
    assert_eq!(hp(&mut app, near), 150);
    assert_eq!(hp(&mut app, far), 100);
    steps(&mut app, 106);
    assert_eq!(hp(&mut app, near), 150);
    steps(&mut app, 1);
    assert_eq!(hp(&mut app, near), 200);
}
#[test]
fn ra6_stationary_mirage_is_a_tree_to_adjacent_nondetectors() {
    let mut app = world();
    let mirage = put(&mut app, P0, "mirage_tank", 8, 8);
    put(&mut app, P1, "conscript", 9, 8);
    steps(&mut app, 1);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P1, mirage))
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .mirage_trees
            .contains_key(&mirage)
    );
    put(&mut app, P1, "attack_dog", 9, 9);
    steps(&mut app, 1);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P1, mirage))
    );
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![mirage],
                target: CellPos::new(14, 8),
            },
        )],
    );
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .mirage_trees
            .contains_key(&mirage)
    );
}
#[test]
fn ra6_curtain_covers_both_owners_kills_infantry_and_expires_at_1000() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "iron_curtain", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let a = put(&mut app, P0, "grizzly", 20, 20);
    let b = put(&mut app, P1, "rhino", 21, 21);
    let infantry = put(&mut app, P1, "conscript", 19, 20);
    let outside = put(&mut app, P0, "grizzly", 22, 20);
    steps(&mut app, 2);
    charge(&mut app, sw);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, infantry).is_none());
    let s = app.world().resource::<sim::SpecialState>();
    assert!(s.protected.contains_key(&a) && s.protected.contains_key(&b));
    assert!(!s.protected.contains_key(&outside));
    steps(&mut app, 999);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .protected
            .contains_key(&a)
    );
    steps(&mut app, 1);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .protected
            .contains_key(&a)
    );
}
fn sea() -> App {
    let mut app = world();
    for y in 0..48 {
        for x in 0..48 {
            app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(x, y),
                Cell {
                    water: true,
                    ..OPEN
                },
            );
        }
    }
    app
}
#[test]
fn ra6_squid_culls_on_the_crossing_hit_and_cannot_cull_through_curtain() {
    for elite in [false, true] {
        let mut app = sea();
        let squid = put(&mut app, P0, "giant_squid", 8, 8);
        let ship = put(&mut app, P1, "dreadnought", 9, 8);
        steps(&mut app, 1);
        if elite {
            sim::promote(app.world_mut(), squid, 2);
        }
        attack(&mut app, squid, ship);
        assert!(entity(&mut app, squid).is_none());
        let d = defs(&app);
        let max = d
            .unit_combat(d.unit_kind("dreadnought").unwrap())
            .unwrap()
            .hit_points;
        let threshold = max * if elite { 50 } else { 25 } / 100;
        set_hp(&mut app, ship, threshold + 1);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .protected
            .insert(ship, u64::MAX);
        steps(&mut app, 132);
        assert_eq!(hp(&mut app, ship), threshold + 1);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .protected
            .remove(&ship);
        steps(&mut app, 1);
        assert!(entity(&mut app, ship).is_none(), "elite={elite}");
        assert!(entity(&mut app, squid).is_some());
    }
}
#[test]
fn ra6_squid_prevents_motion_but_ship_keeps_its_weapon() {
    let mut app = sea();
    let squid = put(&mut app, P0, "giant_squid", 8, 8);
    let ship = put(&mut app, P1, "destroyer", 9, 8);
    steps(&mut app, 1);
    attack(&mut app, squid, ship);
    let before = cell(&mut app, ship);
    let e = entity(&mut app, ship).unwrap();
    assert!(app.world().get::<sim::Weapon>(e).is_some());
    tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![ship],
                target: CellPos::new(15, 8),
            },
        )],
    );
    steps(&mut app, 10);
    assert_eq!(cell(&mut app, ship), before);
    assert!(app.world().get::<sim::Weapon>(e).is_some());
}
#[test]
fn ra6_chronosphere_rejects_unseen_area_and_kills_bad_or_occupied_landings() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "chronosphere", 3, 3);
    let a = put(&mut app, P0, "grizzly", 14, 14);
    steps(&mut app, 2);
    charge(&mut app, sw);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sw,
                source: CellPos::new(14, 14),
                target: CellPos::new(40, 0),
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    assert_eq!(cell(&mut app, a), CellPos::new(14, 14));
    put(&mut app, P0, "spy_satellite", 3, 8);
    let b = put(&mut app, P1, "rhino", 15, 14);
    let blocker = put(&mut app, P1, "rhino", 30, 30);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(31, 30),
        Cell {
            water: true,
            passable: false,
            ..OPEN
        },
    );
    steps(&mut app, 2);
    charge(&mut app, sw);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sw,
                source: CellPos::new(14, 14),
                target: CellPos::new(30, 30),
            },
        )],
    );
    assert!(entity(&mut app, a).is_none());
    assert!(entity(&mut app, b).is_none());
    assert!(entity(&mut app, blocker).is_some());
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
#[test]
fn ra6_public_timers_and_nuclear_marks_do_not_disclose_unseen_sources() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "nuclear_silo", 3, 3);
    put(&mut app, P0, "spy_satellite", 3, 8);
    steps(&mut app, 2);
    charge(&mut app, sw);
    assert!(
        sim::public_timers(app.world())
            .iter()
            .any(|t| t.owner == P0 && t.role == sim::Role::Nuke && t.remaining == 0)
    );
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    assert_eq!(
        sim::nuclear_marks(app.world(), P0),
        vec![CellPos::new(20, 20)]
    );
    assert!(sim::nuclear_marks(app.world(), P1).is_empty());
    put(&mut app, P1, "psychic_sensor", 22, 22);
    steps(&mut app, 1);
    assert_eq!(
        sim::nuclear_marks(app.world(), P1),
        vec![CellPos::new(20, 20)]
    );
}
#[test]
fn ra6_prism_caps_eight_supporters_and_supports_again_at_tick_80() {
    let mut app = world();
    power(&mut app);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let root = put(&mut app, P0, "prism_tower", 10, 10);
    let donors: Vec<_> = (1..=9)
        .map(|n| put(&mut app, P0, "prism_tower", 10 - n, 10))
        .collect();
    let target = put(&mut app, P1, "rhino", 17, 10);
    set_hp(&mut app, target, 10000);
    let e = entity(&mut app, target).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    steps(&mut app, 1);
    let before = hp(&mut app, target);
    steps(&mut app, 1);
    assert_eq!(before - hp(&mut app, target), 120 * 13);
    let s = app.world().resource::<sim::SpecialState>();
    assert_eq!(
        donors
            .iter()
            .filter(|id| s.disabled.contains_key(id))
            .count(),
        8
    );
    assert!(!s.disabled.contains_key(&donors[8]));
    assert!(!s.disabled.contains_key(&root));
    steps(&mut app, 79);
    assert_eq!(hp(&mut app, target), before - 1560);
    steps(&mut app, 1);
    assert_eq!(hp(&mut app, target), before - 3120);
    assert_eq!(
        donors
            .iter()
            .filter(|id| app
                .world()
                .resource::<sim::SpecialState>()
                .disabled
                .contains_key(id))
            .count(),
        8
    );
}
#[test]
fn ra6_storm_warning_exclusivity_duration_and_cloud_separation_are_deterministic() {
    fn setup() -> (App, SimId) {
        let mut a = world();
        power(&mut a);
        let sw = put(&mut a, P0, "weather_control", 3, 3);
        put(&mut a, P0, "spy_satellite", 3, 8);
        put(&mut a, P1, "radar_tower", 39, 39);
        steps(&mut a, 2);
        charge(&mut a, sw);
        (a, sw)
    }
    let (mut a, sw) = setup();
    let (mut b, sw2) = setup();
    for (app, id) in [(&mut a, sw), (&mut b, sw2)] {
        let ev = tick(
            app,
            vec![issue(
                P0,
                Command::Superweapon {
                    building: id,
                    target: CellPos::new(25, 25),
                    units: vec![],
                },
            )],
        );
        assert!(rejected(&ev).is_empty(), "{ev:?}");
    }
    for n in 0..573 {
        let s = a
            .world()
            .resource::<sim::SpecialState>()
            .strategic
            .storm
            .as_ref();
        if n < 333 {
            assert!(s.unwrap().clouds.is_empty());
        }
        if n == 333 {
            assert!(!a.world().resource::<Players>().get(P1).unwrap().radar());
        }
        if let Some(s) = s {
            for (c, _) in &s.clouds {
                assert!((20..30).contains(&c.x) && (20..30).contains(&c.y));
                if *c != s.target {
                    assert!(c.x.abs_diff(s.target.x) + c.y.abs_diff(s.target.y) >= 3);
                }
            }
        }
        assert_eq!(
            sim::state_hash(a.world_mut()),
            sim::state_hash(b.world_mut())
        );
        steps(&mut a, 1);
        steps(&mut b, 1);
    }
    assert!(
        a.world()
            .resource::<sim::SpecialState>()
            .strategic
            .storm
            .is_none()
    );
}
#[test]
fn ra6_airport_paradrops_one_shootable_plane_and_full_side_payload() {
    for (player, count, name, building) in [
        (P0, 6, "gi", "airport"),
        (P1, 9, "conscript", "airport"),
        (P0, 8, "gi", "american_airforce_command"),
    ] {
        let mut app = world();
        let airport = put(&mut app, player, building, 12, 12);
        put(&mut app, player, "nuclear_reactor", 0, 0);
        put(&mut app, player, "spy_satellite", 3, 8);
        steps(&mut app, 2);
        charge(&mut app, airport);
        let ev = tick(
            &mut app,
            vec![issue(
                player,
                Command::Superweapon {
                    building: airport,
                    target: CellPos::new(20, 20),
                    units: vec![],
                },
            )],
        );
        assert!(rejected(&ev).is_empty(), "{ev:?}");
        let plane = *app
            .world()
            .resource::<sim::SpecialState>()
            .strategic
            .flights
            .keys()
            .next()
            .unwrap();
        assert_eq!(hp(&mut app, plane), 400);
        for _ in 0..500 {
            if app
                .world()
                .resource::<sim::SpecialState>()
                .strategic
                .flights
                .is_empty()
            {
                break;
            }
            steps(&mut app, 1);
        }
        assert!(entity(&mut app, plane).is_none());
        let kind = defs(&app).unit_kind(name).unwrap();
        let w = app.world_mut();
        let units: Vec<_> = w
            .query::<(&sim::Unit, &sim::Mobile)>()
            .iter(w)
            .filter(|(u, _)| u.kind == kind)
            .map(|(_, m)| m.cell)
            .collect();
        assert_eq!(units.len(), count);
        assert!(
            units
                .iter()
                .all(|c| c.x.abs_diff(20).pow(2) + c.y.abs_diff(20).pow(2) <= 16)
        );
    }
}

#[test]
fn ra6_ifv_mode_twelve_uses_cow_shot_with_an_independent_mode_fixture() {
    let mut app = world();
    let d = defs(&app);
    let text = std::fs::read_to_string(data_dir().join("arsenal.ron")).unwrap();
    let line = text.lines().find(|l| l.contains("id: \"gi\"")).unwrap();
    let modified = text.replacen(line, &line.replace("ifv_mode: 2", "ifv_mode: 12"), 1);
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ra6-cow-mode-{}.ron", std::process::id()));
    std::fs::write(&path, modified).unwrap();
    app.world_mut()
        .insert_resource(d.with_arsenal(&path).unwrap());
    std::fs::remove_file(path).unwrap();
    let ifv = put(&mut app, P0, "ifv", 8, 8);
    let cow_mode = put(&mut app, P0, "gi", 9, 8);
    steps(&mut app, 1);
    assert!(rejected(&interact(&mut app, cow_mode, ifv)).is_empty());
    let e = entity(&mut app, ifv).unwrap();
    assert_eq!(
        app.world().get::<sim::Weapon>(e).unwrap().kind,
        defs(&app).weapon_kind("cow_shot").unwrap()
    );
}
#[test]
fn ra6_dolphin_sonic_hit_releases_squid_without_killing_it() {
    let mut app = sea();
    let squid = put(&mut app, P0, "giant_squid", 8, 8);
    let host = put(&mut app, P1, "dreadnought", 9, 8);
    steps(&mut app, 1);
    attack(&mut app, squid, host);
    let dolphin = put(&mut app, P0, "dolphin", 12, 8);
    steps(&mut app, 1);
    attack(&mut app, dolphin, host);
    assert!(entity(&mut app, squid).is_some());
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .contains_key(&host)
    );
    assert!(entity(&mut app, host).is_some());
}
#[test]
fn ra6_shipped_curtain_death_prevents_later_same_tick_infantry_command() {
    let mut app = world();
    power(&mut app);
    let sw = put(&mut app, P0, "iron_curtain", 3, 3);
    let victim = put(&mut app, P0, "gi", 12, 12);
    steps(&mut app, 2);
    charge(&mut app, sw);
    let ev = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Superweapon {
                    building: sw,
                    target: CellPos::new(12, 12),
                    units: vec![],
                },
            ),
            issue(
                P0,
                Command::Move {
                    units: vec![victim],
                    target: CellPos::new(15, 15),
                },
            ),
        ],
    );
    assert_eq!(rejected(&ev).len(), 1);
    assert!(entity(&mut app, victim).is_none());
}
#[test]
fn ra6_paradrop_blocked_landing_retains_payload_until_space_opens() {
    let mut app = world();
    let sw = put(&mut app, P0, "airport", 12, 12);
    power(&mut app);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let mut blockers = vec![];
    for y in 16..=24 {
        for x in 16..=24 {
            if (x - 20i32).pow(2) + (y - 20i32).pow(2) <= 16 {
                blockers.push(put(&mut app, P0, "terror_drone", x, y));
            }
        }
    }
    steps(&mut app, 2);
    charge(&mut app, sw);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    steps(&mut app, 500);
    let state = app.world().resource::<sim::SpecialState>();
    assert_eq!(state.strategic.flights.len(), 1);
    assert_eq!(state.strategic.flights.values().next().unwrap().count, 6);
    for id in blockers {
        set_hp(&mut app, id, 0);
    }
    steps(&mut app, 2);
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .strategic
            .flights
            .is_empty()
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}
#[test]
fn ra6_anti_air_destroys_paradrop_plane_before_it_drops() {
    let mut app = world();
    let sw = put(&mut app, P0, "airport", 12, 12);
    power(&mut app);
    put(&mut app, P0, "spy_satellite", 3, 8);
    steps(&mut app, 2);
    charge(&mut app, sw);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(20, 20),
                units: vec![],
            },
        )],
    );
    let plane = *app
        .world()
        .resource::<sim::SpecialState>()
        .strategic
        .flights
        .keys()
        .next()
        .unwrap();
    let c = cell(&mut app, plane);
    put(&mut app, P1, "flak_cannon", c.x + 2, c.y + 2);
    set_hp(&mut app, plane, 1);
    steps(&mut app, 150);
    assert!(entity(&mut app, plane).is_none());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .strategic
            .flights
            .is_empty()
    );
    let kind = defs(&app).unit_kind("gi").unwrap();
    let w = app.world_mut();
    assert_eq!(
        w.query::<&sim::Unit>()
            .iter(w)
            .filter(|u| u.kind == kind)
            .count(),
        0
    );
}
#[test]
fn ra6_storm_rejects_a_second_launch_and_preserves_its_charge() {
    let mut a = world();
    power(&mut a);
    let one = put(&mut a, P0, "weather_control", 3, 3);
    let two = put(&mut a, P0, "weather_control", 8, 3);
    put(&mut a, P0, "spy_satellite", 3, 8);
    steps(&mut a, 2);
    charge(&mut a, one);
    charge(&mut a, two);
    let ev = tick(
        &mut a,
        vec![
            issue(
                P0,
                Command::Superweapon {
                    building: one,
                    target: CellPos::new(25, 25),
                    units: vec![],
                },
            ),
            issue(
                P0,
                Command::Superweapon {
                    building: two,
                    target: CellPos::new(30, 30),
                    units: vec![],
                },
            ),
        ],
    );
    assert_eq!(rejected(&ev).len(), 1);
    assert_eq!(
        a.world().resource::<sim::SpecialState>().charge[&two],
        12000
    );
    assert_eq!(
        a.world()
            .resource::<sim::SpecialState>()
            .strategic
            .storm
            .as_ref()
            .unwrap()
            .target,
        CellPos::new(25, 25)
    );
}
#[test]
fn ra6_strategic_recharges_power_option_and_readiness_edges() {
    for (name, recharge) in [
        ("nuclear_silo", 12000),
        ("weather_control", 12000),
        ("iron_curtain", 6000),
        ("chronosphere", 8400),
        ("airport", 4800),
        ("american_airforce_command", 4800),
    ] {
        let mut a = world();
        power(&mut a);
        let sw = put(&mut a, P0, name, 5, 5);
        steps(&mut a, 2);
        let d = defs(&a);
        let k = d.building_kind(name).unwrap();
        assert_eq!(
            d.metadata(ProductionItem::Building(k)).unwrap().cooldown,
            recharge
        );
        a.world_mut()
            .resource_mut::<sim::SpecialState>()
            .charge
            .insert(sw, recharge - 1);
        let ev = tick(&mut a, vec![]);
        assert_eq!(
            a.world().resource::<sim::SpecialState>().charge[&sw],
            recharge
        );
        assert_eq!(
            ev.iter()
                .filter(|e| matches!(e, SimEvent::SuperweaponReady { player: P0, .. }))
                .count(),
            1
        );
        assert!(
            !tick(&mut a, vec![])
                .iter()
                .any(|e| matches!(e, SimEvent::SuperweaponReady { .. }))
        );
        if d.metadata(ProductionItem::Building(k)).unwrap().is_powered {
            a.world_mut()
                .resource_mut::<sim::SpecialState>()
                .outages
                .insert(P0, u64::MAX);
            steps(&mut a, 1);
            a.world_mut()
                .resource_mut::<sim::SpecialState>()
                .charge
                .insert(sw, 0);
            steps(&mut a, 5);
            assert_eq!(a.world().resource::<sim::SpecialState>().charge[&sw], 0);
            charge(&mut a, sw);
            let ev = tick(
                &mut a,
                vec![issue(
                    P0,
                    Command::Superweapon {
                        building: sw,
                        target: CellPos::new(5, 5),
                        units: vec![],
                    },
                )],
            );
            assert_eq!(rejected(&ev).len(), 1);
        }
    }
    let mut a = world();
    power(&mut a);
    let sw = put(&mut a, P0, "iron_curtain", 5, 5);
    steps(&mut a, 2);
    charge(&mut a, sw);
    let mut rules = a.world().resource::<sim::MatchState>().rules();
    rules.superweapons = false;
    a.world_mut()
        .resource_mut::<sim::MatchState>()
        .set_rules(rules);
    assert_eq!(
        rejected(&tick(
            &mut a,
            vec![issue(
                P0,
                Command::Superweapon {
                    building: sw,
                    target: CellPos::new(5, 5),
                    units: vec![]
                }
            )]
        ))
        .len(),
        1
    );
}
#[test]
fn ra6_ivan_ifv_death_uses_crnuke_and_loses_its_passenger() {
    let mut a = world();
    let ifv = put(&mut a, P0, "ifv", 8, 8);
    let ivan = put(&mut a, P0, "crazy_ivan", 9, 8);
    steps(&mut a, 1);
    interact(&mut a, ivan, ifv);
    let victim = put(&mut a, P0, "grizzly", 9, 8);
    let before = hp(&mut a, victim);
    set_hp(&mut a, ifv, 0);
    steps(&mut a, 1);
    assert!(entity(&mut a, ivan).is_none());
    assert!(
        !a.world()
            .resource::<sim::SpecialState>()
            .cargo
            .contains_key(&ifv)
    );
    assert!(hp(&mut a, victim) < before);
    assert!(
        !a.world()
            .resource::<sim::SpecialState>()
            .radiation_sites
            .is_empty()
    );
}
#[test]
fn ra6_kirov_crash_and_demo_death_damage_nearby_allies() {
    for name in ["kirov", "demolition_truck"] {
        let mut a = world();
        let payload = put(&mut a, P0, name, 8, 8);
        let victim = put(&mut a, P0, "grizzly", 9, 8);
        set_hp(&mut a, victim, 10000);
        let before = hp(&mut a, victim);
        set_hp(&mut a, payload, 0);
        steps(&mut a, 1);
        assert!(hp(&mut a, victim) < before, "{name}");
        assert!(entity(&mut a, payload).is_none());
    }
}
#[test]
fn ra6_pending_parasite_and_strategic_state_changes_affect_replay_hash() {
    let mut a = world();
    power(&mut a);
    let sw = put(&mut a, P0, "nuclear_silo", 3, 3);
    put(&mut a, P0, "spy_satellite", 3, 8);
    let drone = put(&mut a, P0, "terror_drone", 8, 8);
    let host = put(&mut a, P1, "rhino", 9, 8);
    steps(&mut a, 2);
    attack(&mut a, drone, host);
    charge(&mut a, sw);
    tick(
        &mut a,
        vec![issue(
            P0,
            Command::Superweapon {
                building: sw,
                target: CellPos::new(25, 25),
                units: vec![],
            },
        )],
    );
    let baseline = a.world().resource::<sim::SpecialState>().clone();
    let hash = sim::state_hash(a.world_mut());
    for mutation in 0..9 {
        a.world_mut().insert_resource(baseline.clone());
        {
            let mut state = a.world_mut().resource_mut::<sim::SpecialState>();
            match mutation {
                0 => state.parasites.get_mut(&host).unwrap().escape = Some(CellPos::new(9, 8)),
                1 => state.parasites.get_mut(&host).unwrap().due += 1,
                2 => state.parasites.get_mut(&host).unwrap().owner = P1,
                3 => state.parasites.get_mut(&host).unwrap().passenger.hp -= 1,
                4 => state.strategic.missiles[0].due += 1,
                5 => state.strategic.missiles[0].target.x += 1,
                6 => state.strategic.missiles[0].source = SimId(9999),
                7 => state.strategic.missiles[0].owner = P1,
                _ => {
                    state.mirage_trees.insert(drone, 2);
                }
            }
        }
        assert_ne!(sim::state_hash(a.world_mut()), hash, "mutation {mutation}");
    }
}
#[test]
fn ra6_depot_and_repair_ifv_remove_drone_before_its_first_damage_tick() {
    for depot in [true, false] {
        let mut a = world();
        let host = put(&mut a, P0, "grizzly", 9, 8);
        let repair = put(
            &mut a,
            P0,
            if depot { "service_depot" } else { "ifv" },
            10,
            8,
        );
        let engineer = if depot {
            None
        } else {
            Some(put(&mut a, P0, "engineer", 11, 8))
        };
        steps(&mut a, 1);
        if let Some(e) = engineer {
            interact(&mut a, e, repair);
        }
        let drone = put(&mut a, P1, "terror_drone", 8, 8);
        steps(&mut a, 1);
        let before = hp(&mut a, host);
        tick(
            &mut a,
            vec![issue(
                P1,
                Command::Attack {
                    units: vec![drone],
                    target: host,
                },
            )],
        );
        assert!(
            a.world()
                .resource::<sim::SpecialState>()
                .parasites
                .contains_key(&host)
        );
        if depot {
            assert!(rejected(&interact(&mut a, host, repair)).is_empty());
        } else {
            steps(&mut a, 1);
        }
        assert!(
            !a.world()
                .resource::<sim::SpecialState>()
                .parasites
                .contains_key(&host)
        );
        assert!(entity(&mut a, drone).is_none());
        assert_eq!(hp(&mut a, host), before);
    }
}
#[test]
fn ra6_hidden_strategic_sources_do_not_disclose_type_through_legacy_unit_arguments() {
    let mut a = world();
    let hidden = put(&mut a, P1, "chronosphere", 35, 35);
    put(&mut a, P0, "gi", 3, 3);
    steps(&mut a, 1);
    for source in [hidden, SimId(99999)] {
        let ev = tick(
            &mut a,
            vec![issue(
                P0,
                Command::Superweapon {
                    building: source,
                    target: CellPos::new(3, 3),
                    units: vec![SimId(99998)],
                },
            )],
        );
        let rejected = rejected(&ev);
        assert_eq!(rejected.len(), 1);
        assert!(
            matches!(rejected[0],SimEvent::CommandRejected{reason:sim::RejectReason::UnknownUnit(id),..} if *id==source),
            "{ev:?}"
        );
    }
}

#[test]
fn ra6_ifv_psi_pulse_attacks_from_vehicle_with_immunity_and_reload() {
    let mut app = world();
    let ifv = put(&mut app, P0, "ifv", 8, 8);
    let passenger = put(&mut app, P0, "yuri", 9, 8);
    steps(&mut app, 1);
    assert!(rejected(&interact(&mut app, passenger, ifv)).is_empty());
    // Friendly infantry alone must not trigger autonomous fire.
    let friend = put(&mut app, P0, "gi", 7, 8);
    let e = entity(&mut app, friend).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    steps(&mut app, 2);
    let before = hp(&mut app, friend);
    assert_eq!(before, 125);
    let outside_range = put(&mut app, P1, "conscript", 6, 8);
    let e = entity(&mut app, outside_range).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    set_hp(&mut app, outside_range, 2000);
    let events = tick(&mut app, vec![]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::WeaponFired { attacker, .. } if *attacker == ifv))
    );
    assert_eq!(hp(&mut app, outside_range), 2000);
    let target = put(&mut app, P1, "conscript", 9, 8);
    let immune = put(&mut app, P1, "yuri", 8, 9);
    // Remove return fire in this timing fixture; all weapon and armour definitions
    // remain the shipped definitions.
    for id in [friend, target, immune] {
        let e = entity(&mut app, id).unwrap();
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    }
    // High health isolates reload timing from normal lethal infantry damage.
    set_hp(&mut app, target, 2000);
    let events = attack(&mut app, ifv, target);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::WeaponFired { attacker, .. } if *attacker == ifv))
    );
    assert!(hp(&mut app, target) < 2000);
    assert!(hp(&mut app, outside_range) < 2000);
    assert_eq!(hp(&mut app, immune), 100);
    // The opposite side of the IFV is inside the pulse too, proving vehicle origin.
    assert!(entity(&mut app, friend).is_none() || hp(&mut app, friend) < before);
    let after = hp(&mut app, target);
    steps(&mut app, 12);
    assert_eq!(hp(&mut app, target), after);
    steps(&mut app, 1);
    assert!(hp(&mut app, target) < after);
    set_hp(&mut app, target, 1);
    steps(&mut app, 13);
    assert!(entity(&mut app, target).is_none());
    let e = entity(&mut app, ifv).unwrap();
    assert!(app.world().get::<sim::Experience>(e).unwrap().value >= 100);
}

#[test]
fn ra6_ifv_temporal_and_suicide_modes_execute_on_attack() {
    for passenger in ["chrono_legionnaire", "crazy_ivan", "terrorist"] {
        let mut app = world();
        let ifv = put(&mut app, P0, "ifv", 8, 8);
        let p = put(&mut app, P0, passenger, 9, 8);
        steps(&mut app, 1);
        assert!(rejected(&interact(&mut app, p, ifv)).is_empty());
        let target = put(&mut app, P1, "rhino", 9, 8);
        let before = hp(&mut app, target);
        let events = attack(&mut app, ifv, target);
        assert!(rejected(&events).is_empty(), "{passenger}: {events:?}");
        if passenger == "chrono_legionnaire" {
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .erasures
                    .contains_key(&target)
            );
            // Rhino Strength400 * 10 / floor(CRNeutronRifle10 * 3/4): 572 ticks.
            steps(&mut app, 570);
            assert!(entity(&mut app, target).is_some());
            steps(&mut app, 1);
            assert!(entity(&mut app, target).is_none());
            assert!(entity(&mut app, ifv).is_some());
        } else {
            assert!(entity(&mut app, ifv).is_none(), "{passenger}");
            assert!(
                entity(&mut app, target).is_none() || hp(&mut app, target) < before,
                "{passenger}"
            );
        }
    }
}

#[test]
fn ra6_gunner_area_fire_does_not_enable_yuri_automatic_deploy_pulse() {
    let mut app = world();
    let yuri = put(&mut app, P0, "yuri", 8, 8);
    let immune = put(&mut app, P1, "yuri", 9, 8);
    let e = entity(&mut app, immune).unwrap();
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    for _ in 0..80 {
        let events = tick(&mut app, vec![]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::WeaponFired { attacker, .. } if *attacker == yuri))
        );
    }
    assert_eq!(hp(&mut app, immune), 100);
}
