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
/// A single-field Rules-present parser fixture: a genuinely unannounced Chronosphere.
/// All other arsenal bytes and the shipped rules/combat/economy definitions are retained.
fn chronosphere_without_public_reveal(definitions: Definitions) -> Definitions {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let original = std::fs::read_to_string(data_dir().join("arsenal.ron")).unwrap();
    let rows: Vec<_> = original
        .lines()
        .filter(|line| line.contains(r#"id: "chronosphere""#))
        .collect();
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(row.matches("reveal_to_all: 1").count(), 1);
    let replacement = row.replacen("reveal_to_all: 1", "reveal_to_all: 0", 1);
    let altered = original.replacen(row, &replacement, 1);
    assert_eq!(altered.replacen(&replacement, row, 1), original);
    let path = std::env::temp_dir().join(format!(
        "ra6-hidden-chronosphere-{}-{}.ron",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("exclusive per-invocation parser fixture");
        file.write_all(altered.as_bytes()).unwrap();
    }
    let result = definitions.with_arsenal(&path);
    std::fs::remove_file(path).expect("remove before load-result assertion");
    result.unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn ra6_hidden_strategic_sources_do_not_disclose_type_through_legacy_unit_arguments() {
    // Stock RevealToAll makes this source genuinely known after Visibility. Keep that
    // positive distinct from the hidden-source oracle instead of treating it as hidden.
    let mut public = world();
    let kind = defs(&public).building_kind("chronosphere").unwrap();
    assert!(defs(&public).rules().is_some());
    assert!(
        defs(&public)
            .metadata(ProductionItem::Building(kind))
            .unwrap()
            .reveal_to_all
    );
    let source = put(&mut public, P1, "chronosphere", 35, 35);
    put(&mut public, P0, "gi", 3, 3);
    let e = entity(&mut public, source).unwrap();
    let footprint = public.world().get::<sim::Building>(e).unwrap().footprint;
    let observed = tick(&mut public, vec![]);
    assert_eq!(owner(&mut public, source), Some(P1));
    // These are the actual special::known_to premises, not a fabricated Vision.
    assert!(
        public
            .world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .sees_footprint(footprint)
    );
    assert!(
        !public
            .world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, source))
    );
    assert_eq!(
        public
            .world()
            .resource::<sim::SpecialState>()
            .revealed_owners
            .get(&source),
        Some(&P1)
    );
    assert!(observed.iter().any(|event| matches!(
        event,
        SimEvent::BuildingRevealed { player, building, kind: seen_kind, origin }
            if *player == P1 && *building == source && *seen_kind == kind
                && *origin == footprint.origin
    )));
    let ev = tick(
        &mut public,
        vec![issue(
            P0,
            Command::Superweapon {
                building: source,
                target: CellPos::new(3, 3),
                units: vec![SimId(99998)],
            },
        )],
    );
    let denied = rejected(&ev);
    assert_eq!(denied.len(), 1);
    assert!(
        matches!(denied[0], SimEvent::CommandRejected {
            reason: sim::RejectReason::NotOwned(id), ..
        } if *id == source),
        "{ev:?}"
    );

    // Change only this source's announcement flag through the real parser. Retain the
    // exact hidden/missing UnknownUnit oracle with the same malformed legacy units arg.
    let mut a = world();
    let definitions = chronosphere_without_public_reveal(defs(&a));
    assert!(definitions.rules().is_some());
    assert!(
        !definitions
            .metadata(ProductionItem::Building(kind))
            .unwrap()
            .reveal_to_all
    );
    a.world_mut().insert_resource(definitions);
    let hidden = put(&mut a, P1, "chronosphere", 35, 35);
    put(&mut a, P0, "gi", 3, 3);
    let e = entity(&mut a, hidden).unwrap();
    let footprint = a.world().get::<sim::Building>(e).unwrap().footprint;
    let observed = tick(&mut a, vec![]);
    assert!(!observed.iter().any(|event| matches!(
        event, SimEvent::BuildingRevealed { building, .. } if *building == hidden
    )));
    for source in [hidden, SimId(99999)] {
        assert_eq!(owner(&mut a, hidden), Some(P1));
        assert!(
            !a.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .vision()
                .sees_footprint(footprint)
        );
        assert!(
            !a.world()
                .resource::<sim::SpecialState>()
                .hidden
                .contains(&(P0, hidden))
        );
        assert!(
            !a.world()
                .resource::<sim::SpecialState>()
                .revealed_owners
                .contains_key(&hidden)
        );
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

// UNCOMPILED/UNRUN. Append to existing ra6_vehicles.rs test module/file; reuse its real
// world/sea/defs/put/entity/cell/hp/tick/issue/steps/rejected/attack helpers without editing them.
// These are current-model receipts, not original-engine fidelity or source flag readers.
#[test]
fn ra9_behavior87_uniform_attackmove_admission_reaches_coordinate_for_exact_stock_types() {
    for (section, name) in [
        ("BEAG", "black_eagle"),
        ("CCOMAND", "chrono_commando"),
        ("CLEG", "chrono_legionnaire"),
        ("ENGINEER", "engineer"),
        ("ORCA", "harrier"),
        ("SENGINEER", "engineer"),
        ("SHAD", "nighthawk"),
        ("SNIPE", "sniper"),
        ("SPY", "spy"),
    ] {
        let mut app = world();
        let u = put(&mut app, P0, name, 10, 10);
        if defs(&app)
            .metadata(ProductionItem::Unit(defs(&app).unit_kind(name).unwrap()))
            .unwrap()
            .airport_bound
        {
            put(&mut app, P0, "airforce_command", 8, 8);
        }
        steps(&mut app, 1);
        let target = CellPos::new(25, 25);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::AttackMove {
                    units: vec![u],
                    target,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{section}: {events:?}");
        let mut arrived = cell(&mut app, u) == target;
        for _ in 0..1000 {
            if arrived {
                break;
            }
            tick(&mut app, vec![]);
            arrived = cell(&mut app, u) == target;
        }
        assert!(
            arrived,
            "{section}: actual admitted movement must reach the coordinate"
        );
        // ENGINEER/SENGINEER share the game's one physical kind; alias proof is separately mapped.
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
}
#[test]
fn ra9_behavior87_coordinate_move_admits_genuine_unexplored_destination() {
    for name in [
        "chrono_commando",
        "chrono_ivan",
        "chrono_legionnaire",
        "black_eagle",
        "harrier",
        "amphibious_transport",
        "soviet_amphibious_transport",
        "kirov",
    ] {
        let mut app = world();
        let u = put(&mut app, P0, name, 8, 8);
        if defs(&app)
            .metadata(ProductionItem::Unit(defs(&app).unit_kind(name).unwrap()))
            .unwrap()
            .airport_bound
        {
            put(&mut app, P0, "airforce_command", 5, 5);
        }
        steps(&mut app, 1);
        let target = CellPos::new(18, 18);
        assert!(
            !app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .vision()
                .is_explored(target),
            "{name}: real unexplored premise"
        );
        // Ten diagonal steps remain beyond actual sight and fit the original1000-tick bound
        // even for loaded Kirov speed4; no runtime speed or deadline is retuned.
        let loaded_speed = defs(&app)
            .unit(defs(&app).unit_kind(name).unwrap())
            .unwrap()
            .speed;
        assert!(u32::from(sim::DIAGONAL_STEP) * 10 / u32::from(loaded_speed) + 1 < 1000);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![u],
                    target,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{name}: {events:?}");
        let mut arrived = cell(&mut app, u) == target;
        for _ in 0..1000 {
            if arrived {
                break;
            }
            tick(&mut app, vec![]);
            arrived = cell(&mut app, u) == target;
        }
        assert!(arrived, "{name}: actual coordinate movement/chrono delay");
        assert!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .vision()
                .is_explored(target)
        );
    }
    // ASW/HORNET/DMISL/V3ROCKET/PDPLANE require genuine launch/internal mission bodies and remain
    // explicit route obligations. Directly spawning one would not prove a real mission.
}
#[test]
fn ra9_behavior87_real_base_elite_cloakable_shots_use_r51_surface_delay() {
    for (name, base, elite) in [
        ("dolphin", "sonic_zap", "sonic_zap_e"),
        ("typhoon", "sub_torpedo", "sub_torpedo_e"),
        ("giant_squid", "squid_grab", "squid_grab_e"),
    ] {
        for is_elite in [false, true] {
            let mut app = sea();
            let source = put(&mut app, P0, name, 10, 10);
            let target = put(&mut app, P1, "dreadnought", 11, 10);
            // Controlled initial host protection/disable isolates the firing hook and keeps Squid
            // in the world for a live observer check; it is NOT native production of those effects.
            app.world_mut()
                .resource_mut::<sim::SpecialState>()
                .protected
                .insert(target, u64::MAX);
            app.world_mut()
                .resource_mut::<sim::SpecialState>()
                .disabled
                .insert(target, u64::MAX);
            if is_elite {
                assert!(sim::promote(app.world_mut(), source, 2), "real elite setup");
            }
            steps(&mut app, 1);
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .hidden
                    .contains(&(P1, source)),
                "sensor-free observer before actual shot"
            );
            let firing_tick = app.world().resource::<sim::SimTick>().0;
            let events = attack(&mut app, source, target);
            assert!(rejected(&events).is_empty(), "{events:?}");
            let expected = defs(&app)
                .weapon_kind(if is_elite { elite } else { base })
                .unwrap();
            assert!(events.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,weapon,..}if *attacker==source&&*weapon==expected)),"{name}/{is_elite}: {events:?}");
            assert!(
                entity(&mut app, source).is_some(),
                "protected target keeps shooter available; normal attachment is separate"
            );
            assert_eq!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .surfaced
                    .get(&source),
                Some(&(firing_tick + 24))
            );
            assert!(
                !app.world()
                    .resource::<sim::SpecialState>()
                    .hidden
                    .contains(&(P1, source))
            );
            for offset in 1..=24 {
                // Public movement cancels firing; elite reloads can legitimately be below24 ticks.
                let ev = tick(
                    &mut app,
                    if offset == 1 {
                        vec![issue(
                            P0,
                            Command::Move {
                                units: vec![source],
                                target: CellPos::new(2, 10),
                            },
                        )]
                    } else {
                        vec![]
                    },
                );
                assert!(
                    rejected(&ev).is_empty(),
                    "actual post-shot Move must be admitted: {ev:?}"
                );
                assert!(
                    !ev.iter().any(
                        |e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==source)
                    ),
                    "public Move isolates last-shot24-tick surface interval"
                );
                assert_eq!(
                    app.world()
                        .resource::<sim::SpecialState>()
                        .hidden
                        .contains(&(P1, source)),
                    offset >= 24,
                    "{name}/{is_elite} offset{offset}"
                );
            }
        }
    }
    // Actual detector control, normal Squid attachment/despawn/reappearance and all old
    // R51/RA6 assertions remain additional route obligations, not credited by this fixture.
}
#[test]
fn ra9_behavior87_stock_parasite_targets_keep_actual_selected_weapon_and_layer_controls() {
    for elite in [false, true] {
        // A stock Drone's zero-versus same-kind victim is distinct from S's HP2000/light fixture.
        let mut app = world();
        let a = put(&mut app, P0, "terror_drone", 10, 10);
        let b = put(&mut app, P1, "terror_drone", 11, 10);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .disabled
            .insert(b, u64::MAX);
        if elite {
            assert!(
                !sim::promote(app.world_mut(), a, 2),
                "stock Drone is untrainable; no elite-Drone claim"
            );
        }
        steps(&mut app, 1);
        let before = hp(&mut app, b);
        let events = attack(&mut app, a, b);
        assert!(rejected(&events).is_empty(), "{events:?}");
        let expected = defs(&app).weapon_kind("drone_jump").unwrap();
        assert!(events.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,weapon,..}if *attacker==a&&*weapon==expected)),"explicit zero-versus shot: {events:?}");
        assert_eq!(
            hp(&mut app, b),
            before,
            "zero-versus shot is not damage evidence"
        );
        assert_eq!(
            app.world()
                .resource::<sim::SpecialState>()
                .parasites
                .get(&b)
                .map(|p| p.passenger.id),
            Some(a),
            "prospective current explicit-shot attachment, UNRUN"
        );
        // Stock Squid chooses punch for an Organic target on water, not grab.
        let mut app = sea();
        let a = put(&mut app, P0, "giant_squid", 10, 10);
        let b = put(&mut app, P1, "giant_squid", 11, 10);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .disabled
            .insert(b, u64::MAX);
        if elite {
            assert!(sim::promote(app.world_mut(), a, 2), "real elite Squid");
        }
        steps(&mut app, 1);
        let events = attack(&mut app, a, b);
        assert!(rejected(&events).is_empty(), "{events:?}");
        let expected = defs(&app)
            .weapon_kind(if elite {
                "squid_punch_e"
            } else {
                "squid_punch"
            })
            .unwrap();
        assert!(events.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,weapon,..}if *attacker==a&&*weapon==expected)),"{events:?}");
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .parasites
                .is_empty()
        );
        // Air ZEP is not a valid layer for Drone's non-AA parasite weapon.
        let mut app = world();
        let a = put(&mut app, P0, "terror_drone", 10, 10);
        let b = put(&mut app, P1, "kirov", 11, 10);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .disabled
            .insert(b, u64::MAX);
        if elite {
            assert!(
                !sim::promote(app.world_mut(), a, 2),
                "stock Drone is untrainable"
            );
        }
        steps(&mut app, 1);
        let before = hp(&mut app, b);
        let events = attack(&mut app, a, b);
        assert!(
            rejected(&events).is_empty(),
            "target ownership/vision admission is not weapon-layer admission"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==a))
        );
        assert_eq!(hp(&mut app, b), before);
        assert!(
            app.world()
                .resource::<sim::SpecialState>()
                .parasites
                .is_empty()
        );
    }
    // Normal tank/vessel positive attachment and host-can-still-fire/depot/release/death are
    // existing actual native routes that still must run; these negatives do not waive them.
}
#[test]
fn ra9_behavior87_direct_hospital_and_hut_targets_keep_independent_damage_immunity() {
    for (name, immune) in [("hospital", false), ("bridge_hut", true)] {
        let mut app = world();
        let source = put(&mut app, P0, "grizzly", 10, 10);
        let target = put(&mut app, P1, name, 13, 10);
        steps(&mut app, 1);
        let before = hp(&mut app, target);
        let mut events = attack(&mut app, source, target);
        assert!(rejected(&events).is_empty(), "{name}: {events:?}");
        let mut shot=events.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,target:t,..}if *attacker==source&&*t==target));
        for _ in 0..80 {
            if shot {
                break;
            }
            let more = tick(&mut app, vec![]);
            shot|=more.iter().any(|e|matches!(e,SimEvent::WeaponFired{attacker,target:t,..}if *attacker==source&&*t==target));
            events.extend(more);
        }
        assert!(shot, "{name}: actual targeted shot");
        steps(&mut app, 40);
        if immune {
            assert_eq!(hp(&mut app, target), before);
        } else {
            assert!(
                hp(&mut app, target) < before,
                "hospital LegalTarget=no is not a current admission flag"
            );
        }
    }
    // ForceFire/collateral, raw flag mutation and known/hidden/dead/owner controls remain explicit
    // routes; no claim that Immune parsed LegalTarget or that direct fire proves collateral.
}

// UNCOMPILED / UNRUN / UNACCEPTED. Append to existing sim/tests/ra6_vehicles.rs.
// Reuse unchanged real world/sea/put/defs/entity/cell/hp/issue/tick/attack/steps helpers.
// No S provenance fixture is imported as stock targeting proof.
#[test]
fn ra9_b87_infested_stock_destroyer_really_fires_and_damages_after_move_refusal() {
    let mut app = sea();
    let squid = put(&mut app, P0, "giant_squid", 8, 8);
    let host = put(&mut app, P1, "destroyer", 9, 8);
    let target = put(&mut app, P0, "amphibious_transport", 15, 8);
    for p in [P0, P1] {
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(p)
                .unwrap()
                .vision()
                .visible_cells(),
            0
        );
    }
    let primed = tick(
        &mut app,
        vec![
            issue(P0, Command::Guard { units: vec![squid] }),
            issue(P1, Command::Guard { units: vec![host] }),
        ],
    );
    assert!(rejected(&primed).is_empty(), "{primed:?}");
    assert!(
        !primed
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,..}if *attacker==squid))
    );
    assert!(entity(&mut app, squid).is_some());
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .is_empty()
    );
    let events = attack(&mut app, squid, host);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired{attacker,target:t,..}
        if *attacker==squid&&*t==host))
    );
    assert!(entity(&mut app, squid).is_none());
    assert_eq!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .get(&host)
            .map(|p| p.passenger.id),
        Some(squid)
    );
    let before = cell(&mut app, host);
    let host_entity = entity(&mut app, host).unwrap();
    let before_point = app
        .world()
        .get::<sim::Mobile>(host_entity)
        .unwrap()
        .sub_position();
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![host],
                target: CellPos::new(15, 12),
            },
        )],
    );
    assert!(
        rejected(&events).is_empty(),
        "immobilization is not public NotCommandable"
    );
    steps(&mut app, 10);
    assert_eq!(cell(&mut app, host), before);
    let victim_hp = hp(&mut app, target);
    let primary = defs(&app).weapon_kind("gun_155mm").unwrap();
    let mut events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![host],
                target,
            },
        )],
    );
    assert!(
        rejected(&events).is_empty(),
        "real host owner observes surface victim at distance6"
    );
    let mut fired = false;
    let mut damaged = false;
    for _ in 0..240 {
        fired |= events.iter().any(|e| {
            matches!(e,SimEvent::WeaponFired{attacker,target:t,weapon}
            if *attacker==host&&*t==target&&*weapon==primary)
        });
        damaged = hp(&mut app, target) < victim_hp;
        if fired && damaged {
            break;
        }
        events = tick(&mut app, vec![]);
    }
    assert!(
        fired && damaged,
        "actual infested-host primary shot and independent target HP decrease"
    );
    assert_eq!(
        cell(&mut app, host),
        before,
        "the host fired without escaping immobilization"
    );
    assert_eq!(
        app.world()
            .get::<sim::Mobile>(host_entity)
            .unwrap()
            .sub_position(),
        before_point
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .parasites
            .contains_key(&host)
    );
    assert!(entity(&mut app, squid).is_none());
    assert_eq!(owner(&mut app, host), Some(P1));
    assert!(hp(&mut app, host) > 0);
}

#[test]
fn ra9_b87_real_v3_neighbor_impact_damages_hospital_but_not_independent_immune_hut() {
    let mut app = world();
    let launcher = put(&mut app, P0, "v3_launcher", 10, 10);
    put(&mut app, P0, "engineer", 18, 12);
    // Both one-cell stock buildings are one cell from the unoccupied impact coordinate.
    // Neither building is the direct target; no fake Blast/Impact/Health mutation is used.
    let hospital = put(&mut app, P1, "hospital", 20, 11);
    let hut = put(&mut app, P1, "bridge_hut", 20, 9);
    let impact_cell = CellPos::new(20, 10);
    steps(&mut app, 1);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(impact_cell)
    );
    let d = defs(&app);
    let wh = d.warhead_kind("v3_wh").unwrap();
    assert_eq!(d.warhead(wh).unwrap().cell_spread, 256);
    let hospital_before = hp(&mut app, hospital);
    let hut_before = hp(&mut app, hut);
    let mut events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::ForceFire {
                units: vec![launcher],
                target: sim::ForceTarget::Cell(impact_cell),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let mut missile = None;
    let mut detonated = false;
    let mut at = None;
    let mut impact = false;
    for _ in 0..240 {
        for event in &events {
            match event {
                SimEvent::WeaponFired {
                    attacker, target, ..
                } if *attacker == launcher => {
                    assert_ne!(*target, hospital, "Hospital never named as direct target")
                }
                SimEvent::UnitLaunched {
                    player,
                    parent,
                    unit,
                } if *player == P0 && *parent == launcher => {
                    if missile.is_none() {
                        missile = Some(*unit);
                    }
                }
                SimEvent::MissileDetonated {
                    parent,
                    unit,
                    at: point,
                    ..
                } if *parent == launcher && Some(*unit) == missile => {
                    detonated = true;
                    at = Some(*point);
                }
                SimEvent::Impact {
                    warhead, at: point, ..
                } if *warhead == wh && *point == sim::SubPos::centre_of(impact_cell) => {
                    impact = true
                }
                _ => {}
            }
        }
        if detonated && impact {
            break;
        }
        events = tick(&mut app, vec![]);
    }
    assert!(
        missile.is_some() && detonated && impact,
        "real public launcher, flight and detonation"
    );
    assert_eq!(at, Some(sim::SubPos::centre_of(impact_cell)));
    assert!(
        hp(&mut app, hospital) < hospital_before,
        "positive collateral at neighboring coordinate"
    );
    assert!(
        hp(&mut app, hospital) > 0,
        "stock Hospital survives this one neighboring impact"
    );
    assert_eq!(
        hp(&mut app, hut),
        hut_before,
        "independent stock Immune hut at same distance"
    );
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
}

/// Controlled readiness is setup; launch/sale/flight/payload are genuine public/native paths.
/// Stock major superweapons are uncapturable: this is a sale twin, never a forged capture.
#[test]
fn ra9_nuclear_public_sale_retains_launch_owner_and_exact_delayed_payload() {
    let mut results = Vec::new();
    for sell in [false, true] {
        let mut app = world();
        power(&mut app);
        let source = put(&mut app, P0, "nuclear_silo", 3, 3);
        put(&mut app, P0, "spy_satellite", 3, 8);
        let witness = put(&mut app, P1, "apocalypse", 23, 20);
        let e = entity(&mut app, witness).unwrap();
        // The measuring target is explicitly passive, not a modified stock source.
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        steps(&mut app, 2);
        charge(&mut app, source);
        let definitions = defs(&app);
        let silo = definitions.building_kind("nuclear_silo").unwrap();
        let weapon = definitions
            .weapon(definitions.weapon_kind("nuke_payload").unwrap())
            .unwrap();
        let warhead = definitions.warhead(weapon.warhead).unwrap();
        let armour = definitions
            .unit_combat(definitions.unit_kind("apocalypse").unwrap())
            .unwrap()
            .armour;
        let versus = warhead.versus[armour.0 as usize];
        assert_eq!(
            (
                weapon.damage,
                warhead.cell_spread,
                warhead.percent_at_max,
                versus
            ),
            (600, 2560, 2, 100)
        );
        assert_eq!(
            definitions.rules().unwrap().superweapons.nuclear_flight,
            200
        );
        // Independent integer attenuation at an actual vehicle centre three cells away.
        let reach = i64::from(warhead.cell_spread);
        let expected = i64::from(weapon.damage)
            * i64::from(versus)
            * (100 * reach - i64::from(100 - warhead.percent_at_max) * 768)
            / (100 * 100 * reach);
        assert_eq!(expected, 423);
        let before_hp = hp(&mut app, witness);
        let initial_credit = app.world().resource::<Players>().get(P0).unwrap().credits();
        let launch_tick = app.world().resource::<sim::SimTick>().0;
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Superweapon {
                    building: source,
                    target: CellPos::new(20, 20),
                    units: vec![],
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        assert!(events.iter().any(|e| matches!(e,
            SimEvent::StrategicActivated { player, building, role: sim::Role::Nuke, .. }
                if *player == P0 && *building == source)));
        let missiles = &app
            .world()
            .resource::<sim::SpecialState>()
            .strategic
            .missiles;
        assert_eq!(missiles.len(), 1);
        let due = missiles[0].due;
        assert_eq!(due - launch_tick, 200);
        let refund = definitions.rule_refund(silo);
        for elapsed in 1..=200 {
            let events = tick(
                &mut app,
                if sell && elapsed == 1 {
                    vec![issue(P0, Command::SellBuilding { building: source })]
                } else {
                    vec![]
                },
            );
            assert!(rejected(&events).is_empty(), "{events:?}");
            if sell && elapsed == 1 {
                assert!(events.iter().any(|e| matches!(e,
                    SimEvent::BuildingSold { player, building, refund: paid }
                        if *player == P0 && *building == source && *paid == refund)));
            }
            assert_eq!(entity(&mut app, source).is_some(), !sell);
            assert_eq!(
                app.world().resource::<Players>().get(P0).unwrap().credits(),
                initial_credit + if sell { refund } else { 0 }
            );
            if elapsed < 200 {
                assert_eq!(
                    hp(&mut app, witness),
                    before_hp,
                    "no early payload at {elapsed}"
                );
                let missiles = &app
                    .world()
                    .resource::<sim::SpecialState>()
                    .strategic
                    .missiles;
                assert_eq!(missiles.len(), 1);
                assert_eq!(
                    (
                        missiles[0].owner,
                        missiles[0].source,
                        missiles[0].target,
                        missiles[0].due
                    ),
                    (P0, source, CellPos::new(20, 20), due)
                );
            } else {
                assert!(
                    app.world()
                        .resource::<sim::SpecialState>()
                        .strategic
                        .missiles
                        .is_empty()
                );
                assert!(events.iter().any(|e| matches!(e,
                    SimEvent::Impact { warhead: kind, at, .. }
                        if *kind == weapon.warhead && *at == sim::SubPos::centre_of(CellPos::new(20, 20)))));
                assert_eq!(i64::from(before_hp - hp(&mut app, witness)), expected);
                assert!(
                    app.world()
                        .resource::<sim::SpecialState>()
                        .radiation_sites
                        .iter()
                        .any(|site| site.source == Some(source))
                );
            }
            assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
            assert!(
                app.world()
                    .resource::<Players>()
                    .iter()
                    .all(|(_, p)| p.credits() >= 0)
            );
        }
        results.push(before_hp - hp(&mut app, witness));
    }
    assert_eq!(
        results,
        vec![423, 423],
        "source existence cannot cancel/scale/reassign payload"
    );
}

/// Current generic Attack chase, not a parsed CanApproachTarget permission.
#[test]
fn ra9_mirage_public_attack_approaches_visible_out_of_range_target_and_breaks_tree() {
    let mut app = world();
    let source = put(&mut app, P0, "mirage_tank", 16, 16);
    tick(&mut app, vec![]);
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    assert!(
        app.world()
            .resource::<Players>()
            .iter()
            .all(|(_, p)| p.credits() >= 0)
    );
    let definitions = defs(&app);
    let kind = definitions.unit_kind("mirage_tank").unwrap();
    let terms = definitions.unit_combat(kind).unwrap();
    let primary = terms.weapon.unwrap();
    let gun = definitions.weapon(primary).unwrap();
    assert_eq!(primary, definitions.weapon_kind("mirage_gun").unwrap());
    let distance = gun.range / 256 + 1;
    assert!(
        distance <= terms.sight,
        "actual loaded range must leave a visible chase interval"
    );
    assert!(distance < 24);
    let target_cell = CellPos::new(16 + distance as i32, 16);
    // MCV is naturally unarmed; no HP/Owner/Orders/Position/source definition is injected.
    let target = put(&mut app, P1, "mcv", target_cell.x, target_cell.y);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .vision()
            .is_visible(target_cell)
    );
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .mirage_trees
            .contains_key(&source)
    );
    let e = entity(&mut app, source).unwrap();
    let before = app.world().get::<sim::Mobile>(e).unwrap().sub_position();
    let target_e = entity(&mut app, target).unwrap();
    let target_point = app
        .world()
        .get::<sim::Mobile>(target_e)
        .unwrap()
        .sub_position();
    assert!(!sim::in_range(
        i64::from(target_point.x) - i64::from(before.x),
        i64::from(target_point.y) - i64::from(before.y),
        gun.range
    ));
    let before_hp = hp(&mut app, target);
    let mut events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![source],
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .mirage_trees
            .contains_key(&source),
        "real attack movement order removes stationary tree appearance"
    );
    let mut approached = false;
    let mut fired = false;
    for _ in 0..400 {
        let e = entity(&mut app, source).unwrap();
        let mobile = app.world().get::<sim::Mobile>(e).unwrap();
        approached |= mobile.step.is_some() && mobile.sub_position() != before;
        fired |= events.iter().any(|e| {
            matches!(e,
            SimEvent::WeaponFired { attacker, target: t, weapon }
                if *attacker == source && *t == target && *weapon == primary)
        });
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
        assert!(
            app.world()
                .resource::<Players>()
                .iter()
                .all(|(_, p)| p.credits() >= 0)
        );
        if fired && hp(&mut app, target) < before_hp {
            break;
        }
        events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty(), "{events:?}");
    }
    assert!(
        approached,
        "actual native position/step must approach before the selected shot"
    );
    assert!(fired, "real public Attack must fire the loaded primary");
    assert!(hp(&mut app, target) < before_hp);
    assert_eq!(owner(&mut app, source), Some(P0));
    assert_eq!(owner(&mut app, target), Some(P1));
}
