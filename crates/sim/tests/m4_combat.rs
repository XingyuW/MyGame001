//! M4 tests: combat and fog of war (PLAN.md sections 5 and 7, M4; invariants I3, I5, I6, I7, I8).
//!
//! Every scenario runs through the production path: `SimPlugin` installs the resources and
//! `SimSchedule`, the grid and definitions come from fixture files (`units_combat.ron`,
//! `economy_combat.ron`, `combat.ron`, and the economy map), and every order goes through
//! `CommandQueue`. The only direct state changes are scenario setup: `Players::add`,
//! `spawn_building`, `spawn_unit`, and `Grid::set_terrain` for rocks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AttackEnd, AttackOrder, Building, Cell, CellPos, Command, CommandQueue, Definitions,
    HarvestIdle, HarvestState, Harvester, Health, IssuedCommand, Mobile, PlayerId, Players,
    Position, Projectiles, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, SimTick, Unit,
    Vision, Weapon, load_map, load_match_definitions, spawn_building, spawn_unit, state_hash,
};

const HUMAN: PlayerId = PlayerId(0);
const ENEMY: PlayerId = PlayerId(1);

const ROCK: Cell = Cell {
    passable: false,
    buildable: false,
    ore: 0,
};

// --- Scenario helpers -----------------------------------------------------------------------

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn definitions() -> Definitions {
    load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The economy fixture map (grass except for an ore field at x 22..=25, y 6..=9, a sand column
/// at x 18, y 0..=12, and two rock rings around (3, 27) and (28, 27)), the combat definitions,
/// and both players with 1,000 credits.
fn scenario() -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    let grid = load_map(fixture("maps/economy_32x32.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions());
    for player in [HUMAN, ENEMY] {
        app.world_mut()
            .resource_mut::<Players>()
            .add(player, 1_000)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    app
}

fn unit(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn building(app: &mut App, player: PlayerId, kind: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), player, kind, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{kind} at ({x}, {y}): {error}"))
}

fn issue(app: &mut App, player: PlayerId, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand { player, command });
}

/// Runs one tick, checks the invariants, and returns the tick's events.
fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    assert_invariants(app);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn ticks(app: &mut App, n: u32) -> Vec<SimEvent> {
    (0..n).flat_map(|_| tick(app)).collect()
}

/// The index of the tick the next run of the schedule executes.
fn next_tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn hash(app: &mut App) -> u64 {
    state_hash(app.world_mut())
}

/// After every tick: invariant I6 (every unit holds exactly its cell and the cell it steps
/// into, no cell is held twice, no unit holds a cell covered by a building, and the grid's
/// building layer is exactly the buildings' footprints), invariant I7's first half (credits are
/// never negative), and no unit or building with hit points ≤ 0 survives a tick.
fn assert_invariants(app: &mut App) {
    let world = app.world_mut();
    let mut held: BTreeMap<CellPos, SimId> = BTreeMap::new();
    let mut units = world.query::<(&SimId, &Mobile)>();
    for (id, mobile) in units.iter(world) {
        let cells = std::iter::once(mobile.cell).chain(mobile.step.map(|step| step.to));
        for cell in cells {
            assert!(held.insert(cell, *id).is_none(), "{cell:?} held twice");
        }
    }
    let mut covered: BTreeMap<CellPos, SimId> = BTreeMap::new();
    let mut buildings = world.query::<(&SimId, &Building)>();
    for (id, building) in buildings.iter(world) {
        for cell in building.footprint.cells() {
            assert!(
                covered.insert(cell, *id).is_none(),
                "{cell:?} covered twice"
            );
            assert!(!held.contains_key(&cell), "a unit holds {cell:?} of {id:?}");
        }
    }
    let mut healths = world.query::<(&SimId, &Health)>();
    for (id, health) in healths.iter(world) {
        assert!(health.hit_points > 0, "{id:?} survived with {health:?}");
    }
    let grid = world.resource::<sim::Grid>();
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            let cell = CellPos::new(x, y);
            assert_eq!(grid.occupant(cell), held.get(&cell).copied(), "{cell:?}");
            assert_eq!(grid.building(cell), covered.get(&cell).copied(), "{cell:?}");
        }
    }
    for (player, state) in world.resource::<Players>().iter() {
        assert!(state.credits() >= 0, "{player:?}");
    }
}

fn hit_points(app: &mut App, id: SimId) -> Option<i32> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Health)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, health)| health.hit_points)
}

fn weapon(app: &mut App, id: SimId) -> Weapon {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Weapon)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, weapon)| weapon)
        .unwrap_or_else(|| panic!("{id:?} has no weapon"))
}

fn mobile(app: &mut App, id: SimId) -> Mobile {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Mobile)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, mobile)| mobile)
        .unwrap_or_else(|| panic!("{id:?} is not a unit"))
}

fn position(app: &mut App, id: SimId) -> sim::SubPos {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Position)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, position)| position.0)
        .unwrap_or_else(|| panic!("{id:?} has no position"))
}

fn harvester_of(app: &mut App, id: SimId) -> Harvester {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Harvester)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, harvester)| harvester)
        .unwrap_or_else(|| panic!("{id:?} is not a harvester"))
}

fn exists(app: &mut App, id: SimId) -> bool {
    let world = app.world_mut();
    let mut query = world.query::<&SimId>();
    query.iter(world).any(|other| *other == id)
}

fn vision(app: &App, player: PlayerId) -> Vision {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .vision()
        .clone()
}

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

fn shots_by(events: &[SimEvent], attacker: SimId) -> usize {
    events
        .iter()
        .filter(
            |event| matches!(event, SimEvent::WeaponFired { attacker: a, .. } if *a == attacker),
        )
        .count()
}

/// Equation 2 written out independently of the simulation: `floor(base × versus / 100)`, at
/// least 1 if `versus > 0`, and 0 if `versus = 0`.
fn expected_damage(base: i64, versus: i64) -> i64 {
    if versus == 0 {
        0
    } else {
        (base * versus / 100).max(1)
    }
}

/// The ENEMY harvester stops its cycle at once (it would otherwise drive to the ore field).
fn parked_harvester(app: &mut App, player: PlayerId, x: i32, y: i32) -> SimId {
    let id = unit(app, player, "harvester", x, y);
    issue(app, player, Command::Stop { units: vec![id] });
    id
}

// --- Equation 2: every warhead and armour pair ------------------------------------------------

#[test]
fn the_damage_equation_holds_for_every_warhead_and_armour_pair_of_the_fixtures() {
    // Attackers carrying each weapon of the fixture, and targets of each armour class whose hit
    // points exceed one shot's damage (the soft target is a rifleman: 100 hit points against
    // the howitzer's 60 on soft armour), so every shot's damage can be read off.
    let attackers = [("rifleman", false), ("tank", false), ("turret", true)];
    let targets = [("rifleman", false), ("harvester", false), ("power", true)];
    let defs = definitions();
    let weapon_of = |kind: &str, is_building: bool| {
        let terms = if is_building {
            defs.building_combat(defs.building_kind(kind).expect("building"))
        } else {
            defs.unit_combat(defs.unit_kind(kind).expect("unit"))
        };
        terms.expect("combat terms").weapon.expect("armed")
    };
    let armour_of = |kind: &str, is_building: bool| {
        let terms = if is_building {
            defs.building_combat(defs.building_kind(kind).expect("building"))
        } else {
            defs.unit_combat(defs.unit_kind(kind).expect("unit"))
        };
        terms.expect("combat terms").armour
    };
    // The scenarios cover every warhead and every armour class of the fixture.
    let warheads: BTreeSet<_> = attackers
        .iter()
        .map(|&(kind, b)| defs.weapon(weapon_of(kind, b)).expect("weapon").warhead)
        .collect();
    assert_eq!(warheads.len(), defs.warheads().len());
    let classes: BTreeSet<_> = targets
        .iter()
        .map(|&(kind, b)| armour_of(kind, b))
        .collect();
    assert_eq!(classes.len(), defs.armour_classes().len());

    let mut cases = BTreeSet::new();
    for &(attacker_kind, attacker_is_building) in &attackers {
        for &(target_kind, target_is_building) in &targets {
            let mut app = scenario();
            // The turret needs power: 100 produced against its 30.
            building(&mut app, HUMAN, "power", 2, 2);
            let attacker = if attacker_is_building {
                building(&mut app, HUMAN, attacker_kind, 10, 16)
            } else {
                unit(&mut app, HUMAN, attacker_kind, 10, 16)
            };
            // Two cells away: within every weapon's range and the turret's sight of 3 cells.
            let target = if target_is_building {
                building(&mut app, ENEMY, target_kind, 12, 16)
            } else if target_kind == "harvester" {
                parked_harvester(&mut app, ENEMY, 12, 16)
            } else {
                unit(&mut app, ENEMY, target_kind, 12, 16)
            };
            let kind = weapon_of(attacker_kind, attacker_is_building);
            let weapon_def = defs.weapon(kind).expect("weapon").clone();
            let versus = defs
                .versus(
                    weapon_def.warhead,
                    armour_of(target_kind, target_is_building),
                )
                .expect("versus");
            let expected = expected_damage(i64::from(weapon_def.damage), i64::from(versus));
            let full = hit_points(&mut app, target).expect("target");
            // Nothing is visible before the first tick, so the first shot comes in tick 1.
            // Automatic acquisition engages only targets the weapon damages (docs/DECISIONS.md
            // D32), so for the zero case the attacker is ordered to attack; the order is
            // validated in tick 1 and fires in it.
            let mut fired_at = None;
            for _ in 0..5 {
                let at = next_tick(&app);
                if versus == 0 && at == 1 {
                    issue(
                        &mut app,
                        HUMAN,
                        Command::Attack {
                            units: vec![attacker],
                            target,
                        },
                    );
                }
                let events = tick(&mut app);
                if events.contains(&SimEvent::WeaponFired {
                    attacker,
                    target,
                    weapon: kind,
                }) {
                    fired_at = Some(at);
                    break;
                }
            }
            let fired_at =
                fired_at.unwrap_or_else(|| panic!("{attacker_kind} did not fire at {target_kind}"));
            assert_eq!(fired_at, 1, "{attacker_kind} -> {target_kind}");
            // A projectile arrives after ceil(gap / speed) ticks; an instant shot hits at once.
            // Along x, from the centre of (10, 16) (2688) or the turret's right edge (2815) to the
            // centre of (12, 16) (3200) or the plant's left edge (3072).
            let from: u32 = if attacker_is_building { 2815 } else { 2688 };
            let to: u32 = if target_is_building { 3072 } else { 3200 };
            let flight = if weapon_def.projectile_speed == 0 {
                0
            } else {
                (to - from).div_ceil(weapon_def.projectile_speed)
            };
            for _ in 0..flight {
                assert_eq!(
                    hit_points(&mut app, target),
                    Some(full),
                    "before the impact"
                );
                tick(&mut app);
            }
            let left = i64::from(hit_points(&mut app, target).expect("alive"));
            assert_eq!(
                i64::from(full) - left,
                expected,
                "{attacker_kind} ({versus} %) -> {target_kind}"
            );
            cases.insert(match (versus, expected) {
                (0, _) => "zero",
                (_, 1) if i64::from(weapon_def.damage) * i64::from(versus) < 100 => "minimum one",
                _ if i64::from(weapon_def.damage) * i64::from(versus) % 100 != 0 => "floor",
                _ => "exact",
            });
        }
    }
    // The fixture exercises the zero case, the minimum of 1, and rounding down.
    for case in ["zero", "minimum one", "floor", "exact"] {
        assert!(cases.contains(case), "no {case} case in {cases:?}");
    }
}

// --- Equation 1: the range boundary ----------------------------------------------------------

#[test]
fn a_target_is_in_range_at_exactly_the_range_and_not_one_subcell_beyond() {
    // An idle rifleman (range 768 subcells) and an enemy crawler that walks toward it at one
    // subcell per tick along the x axis: the first shot comes exactly in the tick in which the
    // distance falls from 769 to 768 subcells.
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 14, 16);
    issue(
        &mut app,
        ENEMY,
        Command::Move {
            units: vec![crawler],
            target: CellPos::new(11, 16),
        },
    );
    let shooter = position(&mut app, rifleman);
    let mut previous = None;
    for _ in 0..600 {
        let events = tick(&mut app);
        let target = position(&mut app, crawler);
        assert_eq!(target.y, shooter.y);
        let distance = target.x - shooter.x;
        if shots_by(&events, rifleman) > 0 {
            assert_eq!(distance, 768, "the first shot is at exactly the range");
            assert_eq!(previous, Some(769), "one subcell farther it did not fire");
            return;
        }
        assert!(distance > 768, "in range at {distance} without firing");
        previous = Some(distance);
    }
    panic!("the rifleman never fired");
}

#[test]
fn a_building_is_in_range_when_its_nearest_edge_is() {
    // The rifleman at the centre of (10, 16) (x = 2688); a power plant's footprint starting at
    // x = 13 begins at 3328, 640 subcells away (in range 768); at x = 14, 896 (out of range,
    // though still visible within the rifleman's sight of 5 cells). The bullet's 0 % against
    // concrete is raised to 1 %, as automatic acquisition engages only targets the weapon
    // damages (docs/DECISIONS.md D32).
    let bullet_damages_concrete = combat_variant("bullet_versus_concrete_1", &[BULLET_CONCRETE]);
    for (origin, fires) in [(13, true), (14, false)] {
        let mut app = scenario();
        app.world_mut()
            .insert_resource(bullet_damages_concrete.clone());
        let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
        let plant = building(&mut app, ENEMY, "power", origin, 16);
        let events = ticks(&mut app, 3);
        assert_eq!(shots_by(&events, rifleman) > 0, fires, "origin x {origin}");
        assert!(vision(&app, HUMAN).is_visible(CellPos::new(origin, 16)));
        assert_eq!(weapon(&mut app, rifleman).target.is_some(), fires);
        let _ = plant;
    }
}

/// The edit of the fixture's combat file that raises the bullet's 0 % against concrete to 1 %.
const BULLET_CONCRETE: (&str, &str) = (
    "(armour: \"concrete\", percent: 0)",
    "(armour: \"concrete\", percent: 1)",
);

/// The fixture with the rifle at the shortest range and reload the format admits, and the tank's
/// cannon at the longest range with the slowest projectile and the widest sight. The bullet deals
/// 1 % instead of 0 % against concrete, so that idle riflemen engage buildings (automatic
/// acquisition engages only targets the weapon damages, docs/DECISIONS.md D32).
fn extreme_scenario() -> App {
    let text = std::fs::read_to_string(fixture("data/combat.ron")).expect("fixture");
    assert!(text.contains(BULLET_CONCRETE.0));
    let text = text
        .replacen(BULLET_CONCRETE.0, BULLET_CONCRETE.1, 1)
        .replacen(
            "range: 768,\n            reload: 10,",
            "range: 384,\n            reload: 1,",
            1,
        )
        .replacen("range: 1024,", "range: 4096,", 1)
        .replacen(
            "armour: \"plate\", sight: 6,",
            "armour: \"plate\", sight: 16,",
            1,
        );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m4_combat");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join("extreme_combat.ron");
    std::fs::write(&path, text).expect("variant");
    let definitions = sim::load_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
    )
    .and_then(|definitions| definitions.with_combat(&path))
    .unwrap_or_else(|error| panic!("{error}"));
    let rifle = definitions
        .weapon(definitions.weapon_kind("rifle").expect("rifle"))
        .expect("def");
    assert_eq!((rifle.range, rifle.reload), (sim::MIN_WEAPON_RANGE, 1));
    let cannon = definitions
        .weapon(definitions.weapon_kind("cannon").expect("cannon"))
        .expect("def");
    assert_eq!(
        (cannon.range, cannon.projectile_speed),
        (sim::MAX_WEAPON_RANGE, sim::MIN_PROJECTILE_SPEED)
    );
    let mut app = scenario();
    app.world_mut().insert_resource(definitions);
    app
}

#[test]
fn the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building() {
    // MIN_WEAPON_RANGE (384 subcells) covers a diagonal neighbour's centre (362 away) and a
    // building from the corner of its ring (181 to the footprint's corner); with reload 1 the
    // rifle fires every tick.
    let mut app = extreme_scenario();
    let diagonal = unit(&mut app, HUMAN, "rifleman", 10, 16);
    let target = parked_harvester(&mut app, ENEMY, 11, 17);
    let corner = unit(&mut app, HUMAN, "rifleman", 20, 16);
    let plant = building(&mut app, ENEMY, "power", 21, 17);
    tick(&mut app);
    for _ in 0..5 {
        let events = tick(&mut app);
        assert!(events.contains(&SimEvent::WeaponFired {
            attacker: diagonal,
            target,
            weapon: sim::WeaponKind(0),
        }));
        assert!(events.contains(&SimEvent::WeaponFired {
            attacker: corner,
            target: plant,
            weapon: sim::WeaponKind(0),
        }));
    }
    // Two cells away (512) it is out of range: an attack order walks up to a neighbour first.
    let mut app = extreme_scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 12, 16);
    tick(&mut app);
    assert_eq!(shots_by(&tick(&mut app), rifleman), 0);
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![rifleman],
            target: crawler,
        },
    );
    let events = ticks(&mut app, 40);
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: crawler
    }));
    assert_eq!(mobile(&mut app, rifleman).cell, CellPos::new(11, 16));
}

#[test]
fn the_longest_range_with_the_slowest_projectile_flies_32_ticks() {
    // A tank seeing 16 cells fires at a target exactly MAX_WEAPON_RANGE (16 cells) away; the shell
    // at MIN_PROJECTILE_SPEED arrives MAX_WEAPON_RANGE / MIN_PROJECTILE_SPEED = 32 ticks later.
    let mut app = extreme_scenario();
    let tank = unit(&mut app, HUMAN, "tank", 4, 16);
    let target = parked_harvester(&mut app, ENEMY, 20, 16);
    tick(&mut app);
    let events = tick(&mut app);
    assert_eq!(shots_by(&events, tank), 1);
    let shell = *app
        .world()
        .resource::<Projectiles>()
        .iter()
        .next()
        .expect("shell");
    assert_eq!(
        shell.flight,
        sim::MAX_WEAPON_RANGE / sim::MIN_PROJECTILE_SPEED
    );
    assert_eq!(shell.flight, 32);
    for _ in 0..31 {
        tick(&mut app);
        assert_eq!(hit_points(&mut app, target), Some(200));
    }
    tick(&mut app);
    assert_eq!(
        hit_points(&mut app, target),
        Some(175),
        "25 at 100 % against plate"
    );
    // One subcell farther would be out of range: 17 cells away it does not fire at all.
    let mut far = extreme_scenario();
    let tank = unit(&mut far, HUMAN, "tank", 4, 16);
    parked_harvester(&mut far, ENEMY, 21, 16);
    assert_eq!(shots_by(&ticks(&mut far, 30), tank), 0);
}

// --- Projectiles ---------------------------------------------------------------------------

#[test]
fn a_projectile_hits_after_its_flight_time() {
    // The tank's cannon (speed 128) fires at a crawler three cells away (768 subcells), so its
    // shell flies ceil(768 / 128) = 6 ticks and deals floor(25 × 30 / 100) = 7.
    let mut app = scenario();
    let tank = unit(&mut app, HUMAN, "tank", 10, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 13, 16);
    tick(&mut app);
    let events = tick(&mut app);
    assert_eq!(shots_by(&events, tank), 1);
    assert_eq!(app.world().resource::<Projectiles>().len(), 1);
    let shell = *app
        .world()
        .resource::<Projectiles>()
        .iter()
        .next()
        .expect("shell");
    assert_eq!((shell.attacker, shell.target), (tank, crawler));
    assert_eq!((shell.flight, shell.remaining), (6, 6));
    for _ in 0..5 {
        tick(&mut app);
        assert_eq!(hit_points(&mut app, crawler), Some(50));
    }
    tick(&mut app);
    assert_eq!(hit_points(&mut app, crawler), Some(43));
    assert!(app.world().resource::<Projectiles>().is_empty());
}

#[test]
fn a_target_that_dies_while_a_projectile_is_in_flight_makes_it_expire() {
    // The tank's shell flies six ticks; three riflemen kill the crawler (50 hit points, 3 × 20)
    // in the tick in which they all fire, so the shell finds no target.
    let mut app = scenario();
    let tank = unit(&mut app, HUMAN, "tank", 10, 16);
    let riflemen = [
        unit(&mut app, HUMAN, "rifleman", 11, 15),
        unit(&mut app, HUMAN, "rifleman", 11, 17),
        unit(&mut app, HUMAN, "rifleman", 12, 15),
    ];
    let crawler = unit(&mut app, ENEMY, "crawler", 13, 16);
    tick(&mut app);
    let events = tick(&mut app);
    assert_eq!(shots_by(&events, tank), 1);
    for rifleman in riflemen {
        assert_eq!(shots_by(&events, rifleman), 1);
    }
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: crawler
    }));
    assert!(!exists(&mut app, crawler));
    assert_eq!(app.world().resource::<Projectiles>().len(), 1);
    // The shooters' targets are gone; idle units simply stop aiming at it.
    let mut expired = Vec::new();
    for _ in 0..6 {
        let events = tick(&mut app);
        expired.extend(
            events
                .into_iter()
                .filter(|event| matches!(event, SimEvent::ProjectileExpired { .. })),
        );
    }
    let cannon = definitions().weapon_kind("cannon").expect("cannon");
    assert_eq!(
        expired,
        [SimEvent::ProjectileExpired {
            attacker: tank,
            target: crawler,
            weapon: cannon,
        }]
    );
    assert!(app.world().resource::<Projectiles>().is_empty());
    for rifleman in riflemen {
        assert_eq!(hit_points(&mut app, rifleman), Some(100));
        assert_eq!(weapon(&mut app, rifleman).target, None);
    }
}

// --- Command validation --------------------------------------------------------------------

#[test]
fn attacks_on_own_dead_and_unseen_units_are_rejected_without_changing_state() {
    let build = || {
        let mut app = scenario();
        let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
        let own_crawler = unit(&mut app, HUMAN, "crawler", 10, 18);
        let own_plant = building(&mut app, HUMAN, "power", 2, 14);
        // In range and visible: the rifleman kills it with three shots (ticks 1, 11, 21).
        let doomed = unit(&mut app, ENEMY, "crawler", 12, 16);
        // Far beyond every sight of the human player.
        let unseen = unit(&mut app, ENEMY, "crawler", 28, 20);
        let unseen_plant = building(&mut app, ENEMY, "power", 26, 14);
        (
            app,
            [
                rifleman,
                own_crawler,
                own_plant,
                doomed,
                unseen,
                unseen_plant,
            ],
        )
    };
    let (
        mut app,
        [
            rifleman,
            own_crawler,
            own_plant,
            doomed,
            unseen,
            unseen_plant,
        ],
    ) = build();
    let (mut control, _) = build();
    let attack = |target| Command::Attack {
        units: vec![rifleman],
        target,
    };
    // Before the first tick nothing is visible, so even the visible target is unknown.
    issue(&mut app, HUMAN, attack(doomed));
    let events = tick(&mut control);
    assert_eq!(rejections(&events), vec![]);
    let events = tick(&mut app);
    assert_eq!(rejections(&events), [RejectReason::UnknownTarget(doomed)]);
    assert_eq!(hash(&mut app), hash(&mut control));

    let cases = [
        (
            HUMAN,
            attack(own_crawler),
            RejectReason::OwnTarget(own_crawler),
        ),
        (HUMAN, attack(own_plant), RejectReason::OwnTarget(own_plant)),
        (HUMAN, attack(rifleman), RejectReason::OwnTarget(rifleman)),
        (HUMAN, attack(unseen), RejectReason::UnknownTarget(unseen)),
        (
            HUMAN,
            attack(unseen_plant),
            RejectReason::UnknownTarget(unseen_plant),
        ),
        (
            HUMAN,
            attack(SimId(9_999)),
            RejectReason::UnknownTarget(SimId(9_999)),
        ),
        (
            HUMAN,
            Command::Attack {
                units: vec![own_crawler],
                target: doomed,
            },
            RejectReason::NotArmed(own_crawler),
        ),
        (
            HUMAN,
            Command::Attack {
                units: vec![doomed],
                target: rifleman,
            },
            RejectReason::NotOwned(doomed),
        ),
        (
            HUMAN,
            Command::Attack {
                units: vec![],
                target: doomed,
            },
            RejectReason::NoUnits,
        ),
        (
            PlayerId(7),
            attack(doomed),
            RejectReason::UnknownPlayer(PlayerId(7)),
        ),
        (
            HUMAN,
            Command::AttackMove {
                units: vec![rifleman],
                target: CellPos::new(32, 5),
            },
            RejectReason::OutsideMap(CellPos::new(32, 5)),
        ),
        (
            HUMAN,
            Command::AttackMove {
                units: vec![rifleman],
                target: CellPos::new(i32::MIN, i32::MAX),
            },
            RejectReason::OutsideMap(CellPos::new(i32::MIN, i32::MAX)),
        ),
        (
            PlayerId(7),
            Command::AttackMove {
                units: vec![rifleman],
                target: CellPos::new(5, 5),
            },
            RejectReason::UnknownPlayer(PlayerId(7)),
        ),
    ];
    for (player, command, _) in &cases {
        issue(&mut app, *player, command.clone());
    }
    let events = tick(&mut app);
    tick(&mut control);
    let expected: Vec<RejectReason> = cases.iter().map(|(.., reason)| reason.clone()).collect();
    assert_eq!(rejections(&events), expected);
    assert_eq!(
        hash(&mut app),
        hash(&mut control),
        "rejections change nothing"
    );

    // The idle rifleman kills the visible crawler by itself; afterwards it is dead and unknown.
    let mut destroyed = false;
    for _ in 0..40 {
        let a = tick(&mut app);
        tick(&mut control);
        destroyed |= a.contains(&SimEvent::UnitDestroyed {
            player: ENEMY,
            unit: doomed,
        });
        if destroyed {
            break;
        }
    }
    assert!(destroyed);
    issue(&mut app, HUMAN, attack(doomed));
    let events = tick(&mut app);
    tick(&mut control);
    assert_eq!(rejections(&events), [RejectReason::UnknownTarget(doomed)]);
    assert_eq!(hash(&mut app), hash(&mut control));

    // Once the rifleman sees the unseen crawler, the same command is accepted.
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![rifleman],
            target: CellPos::new(24, 20),
        },
    );
    let mut seen = false;
    for _ in 0..400 {
        tick(&mut app);
        if vision(&app, HUMAN).is_visible(CellPos::new(28, 20)) {
            seen = true;
            break;
        }
    }
    assert!(seen, "the rifleman never saw the crawler");
    issue(&mut app, HUMAN, attack(unseen));
    let events = tick(&mut app);
    assert_eq!(rejections(&events), vec![]);
    assert_eq!(
        weapon(&mut app, rifleman).order,
        Some(AttackOrder::Attack {
            target: unseen,
            chases: 1
        }),
        "out of range, it set out toward the crawler"
    );
}

#[test]
fn commands_that_name_an_unseen_enemy_report_it_as_unknown() {
    // Under fog of war a command that names another player's unit or building as its own is
    // rejected as `NotOwned` only if the player sees it; an unseen one is reported exactly like
    // one that does not exist.
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    let seen_unit = unit(&mut app, ENEMY, "crawler", 13, 12);
    let unseen_unit = unit(&mut app, ENEMY, "crawler", 28, 20);
    let seen_barracks = building(&mut app, ENEMY, "barracks", 12, 19);
    let unseen_barracks = building(&mut app, ENEMY, "barracks", 26, 14);
    let mut control = scenario();
    for (player, kind, x, y) in [
        (HUMAN, "rifleman", 10, 16),
        (ENEMY, "crawler", 13, 12),
        (ENEMY, "crawler", 28, 20),
    ] {
        unit(&mut control, player, kind, x, y);
    }
    building(&mut control, ENEMY, "barracks", 12, 19);
    building(&mut control, ENEMY, "barracks", 26, 14);
    tick(&mut app);
    tick(&mut control);
    let view = vision(&app, HUMAN);
    assert!(view.is_visible(CellPos::new(13, 12)) && view.is_visible(CellPos::new(12, 19)));
    assert!(!view.is_visible(CellPos::new(28, 20)) && !view.is_visible(CellPos::new(26, 14)));
    let cases = [
        (
            Command::Move {
                units: vec![rifleman, seen_unit],
                target: CellPos::new(5, 5),
            },
            RejectReason::NotOwned(seen_unit),
        ),
        (
            Command::Move {
                units: vec![rifleman, unseen_unit],
                target: CellPos::new(5, 5),
            },
            RejectReason::UnknownUnit(unseen_unit),
        ),
        (
            Command::Stop {
                units: vec![unseen_unit],
            },
            RejectReason::UnknownUnit(unseen_unit),
        ),
        (
            Command::AttackMove {
                units: vec![unseen_unit],
                target: CellPos::new(5, 5),
            },
            RejectReason::UnknownUnit(unseen_unit),
        ),
        (
            Command::SellBuilding {
                building: seen_barracks,
            },
            RejectReason::NotOwned(seen_barracks),
        ),
        (
            Command::SellBuilding {
                building: unseen_barracks,
            },
            RejectReason::UnknownBuilding(unseen_barracks),
        ),
        (
            Command::SetRallyPoint {
                building: unseen_barracks,
                cell: CellPos::new(5, 5),
            },
            RejectReason::UnknownBuilding(unseen_barracks),
        ),
        (
            Command::SetRallyPoint {
                building: seen_barracks,
                cell: CellPos::new(5, 5),
            },
            RejectReason::NotOwned(seen_barracks),
        ),
    ];
    for (command, _) in &cases {
        issue(&mut app, HUMAN, command.clone());
    }
    let events = tick(&mut app);
    tick(&mut control);
    let expected: Vec<RejectReason> = cases.iter().map(|(_, reason)| reason.clone()).collect();
    assert_eq!(rejections(&events), expected);
    assert_eq!(hash(&mut app), hash(&mut control));
    // A player who is not a player of the match sees nothing.
    issue(
        &mut app,
        PlayerId(9),
        Command::Stop {
            units: vec![seen_unit],
        },
    );
    let events = tick(&mut app);
    assert_eq!(rejections(&events), [RejectReason::UnknownUnit(seen_unit)]);
}

// --- Death and cleanup -----------------------------------------------------------------------

#[test]
fn a_destroyed_unit_releases_both_cells_of_its_step() {
    // A crawler (speed 1) needs 256 ticks for a step, so it dies between two cells.
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 12, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 14, 16);
    issue(
        &mut app,
        ENEMY,
        Command::Move {
            units: vec![crawler],
            target: CellPos::new(20, 16),
        },
    );
    let mut died = false;
    for _ in 0..100 {
        let before = mobile(&mut app, crawler);
        let events = tick(&mut app);
        if events.contains(&SimEvent::UnitDestroyed {
            player: ENEMY,
            unit: crawler,
        }) {
            let step = before.step.expect("it died between two cells");
            let grid = app.world().resource::<sim::Grid>();
            assert_eq!(grid.occupant(before.cell), None);
            assert_eq!(grid.occupant(step.to), None);
            died = true;
            break;
        }
    }
    assert!(died);
    assert!(!exists(&mut app, crawler));
    // The freed cells can be entered.
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![rifleman],
            target: CellPos::new(15, 16),
        },
    );
    let events = ticks(&mut app, 200);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: rifleman,
        retargeted: false
    }));
    assert_eq!(mobile(&mut app, rifleman).cell, CellPos::new(15, 16));
}

#[test]
fn a_destroyed_building_frees_its_cells() {
    // Five tanks (12 damage per shot against concrete) destroy a power plant (200 hit points).
    let mut app = scenario();
    let tanks: Vec<SimId> = [(12, 20), (12, 21), (13, 19), (16, 19), (16, 22)]
        .into_iter()
        .map(|(x, y)| unit(&mut app, HUMAN, "tank", x, y))
        .collect();
    let plant = building(&mut app, ENEMY, "power", 14, 20);
    let power = definitions().building_kind("power").expect("power");
    let mut destroyed = None;
    for _ in 0..400 {
        let at = next_tick(&app);
        let events = tick(&mut app);
        if events.contains(&SimEvent::BuildingDestroyed {
            player: ENEMY,
            building: plant,
            kind: power,
            lost_units: vec![],
        }) {
            destroyed = Some(at);
            break;
        }
    }
    assert!(destroyed.is_some(), "the plant survived");
    let grid = app.world().resource::<sim::Grid>().clone();
    for (x, y) in [(14, 20), (15, 20), (14, 21), (15, 21)] {
        let cell = CellPos::new(x, y);
        assert_eq!(grid.building(cell), None);
        assert!(grid.is_passable(cell));
    }
    // The ENEMY's power is recomputed without it.
    tick(&mut app);
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(ENEMY)
            .expect("enemy")
            .power_produced(),
        0
    );
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![tanks[0]],
            target: CellPos::new(15, 21),
        },
    );
    let events = ticks(&mut app, 200);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: tanks[0],
        retargeted: false
    }));
}

#[test]
fn a_destroyed_building_reports_the_free_units_lost_with_it() {
    // A refinery placed while every cell of its ring is held keeps its free harvester pending;
    // destroyed, it reports the harvester as lost (no silent loss, cf. M3 review 1 P2-1).
    let mut app = scenario();
    building(&mut app, HUMAN, "yard", 19, 14);
    building(&mut app, HUMAN, "power", 23, 14);
    let refinery_kind = definitions().building_kind("refinery").expect("refinery");
    let harvester_kind = definitions().unit_kind("harvester").expect("harvester");
    // The refinery's footprint will be x 19..=21, y 18..=19; its ring is held by crawlers.
    for y in 17..=20 {
        for x in 18..=22 {
            if !((19..=21).contains(&x) && (18..=19).contains(&y)) {
                unit(&mut app, HUMAN, "crawler", x, y);
            }
        }
    }
    let tanks: Vec<SimId> = (17..=23)
        .map(|x| unit(&mut app, ENEMY, "tank", x, 25))
        .collect();
    issue(
        &mut app,
        HUMAN,
        Command::QueueProduction {
            item: sim::ProductionItem::Building(refinery_kind),
        },
    );
    let mut refinery = None;
    for _ in 0..40 {
        let events = tick(&mut app);
        assert_eq!(rejections(&events), vec![]);
        if events
            .iter()
            .any(|event| matches!(event, SimEvent::ProductionComplete { .. }))
        {
            issue(
                &mut app,
                HUMAN,
                Command::PlaceBuilding {
                    building: refinery_kind,
                    origin: CellPos::new(19, 18),
                },
            );
            let events = tick(&mut app);
            assert_eq!(rejections(&events), vec![]);
            refinery = events.iter().find_map(|event| match event {
                SimEvent::BuildingPlaced { building, .. } => Some(*building),
                _ => None,
            });
            break;
        }
    }
    let refinery = refinery.expect("placed");
    tick(&mut app);
    issue(
        &mut app,
        ENEMY,
        Command::Attack {
            units: tanks.clone(),
            target: refinery,
        },
    );
    let mut destroyed = false;
    for _ in 0..600 {
        let events = tick(&mut app);
        assert_eq!(rejections(&events), vec![]);
        if events.contains(&SimEvent::BuildingDestroyed {
            player: HUMAN,
            building: refinery,
            kind: refinery_kind,
            lost_units: vec![harvester_kind],
        }) {
            destroyed = true;
            // Each tank's order ends in the next tick, and says why.
            let next = tick(&mut app);
            for tank in &tanks {
                assert!(next.contains(&SimEvent::AttackEnded {
                    unit: *tank,
                    target: refinery,
                    reason: AttackEnd::TargetGone,
                }));
            }
            break;
        }
    }
    assert!(destroyed);
}

// --- Visibility ------------------------------------------------------------------------------

/// The cells around `(cx, cy)` within `sight` by equation 6, written out independently, inside
/// a `width` × `height` map.
fn cells_in_sight(cx: i32, cy: i32, sight: i32, width: i32, height: i32) -> BTreeSet<CellPos> {
    let mut cells = BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = (x - cx, y - cy);
            if dx * dx + dy * dy <= sight * sight {
                cells.insert(CellPos::new(x, y));
            }
        }
    }
    cells
}

fn visible_set(vision: &Vision, width: i32, height: i32) -> BTreeSet<CellPos> {
    let mut cells = BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            if vision.is_visible(CellPos::new(x, y)) {
                cells.insert(CellPos::new(x, y));
            }
        }
    }
    cells
}

#[test]
fn a_unit_sees_exactly_the_cells_within_its_sight_radius() {
    // A rifleman (sight 5) in the open, and one in the map's corner.
    for (x, y) in [(10, 16), (0, 0), (31, 31)] {
        let mut app = scenario();
        unit(&mut app, HUMAN, "rifleman", x, y);
        assert_eq!(
            vision(&app, HUMAN).visible_cells(),
            0,
            "before the first tick"
        );
        tick(&mut app);
        let view = vision(&app, HUMAN);
        assert_eq!(visible_set(&view, 32, 32), cells_in_sight(x, y, 5, 32, 32));
    }
    // The boundary itself, at (10, 16).
    let mut app = scenario();
    unit(&mut app, HUMAN, "rifleman", 10, 16);
    tick(&mut app);
    let view = vision(&app, HUMAN);
    for (cell, visible) in [
        ((15, 16), true),
        ((16, 16), false),
        ((5, 16), true),
        ((4, 16), false),
        ((10, 21), true),
        ((10, 22), false),
        ((13, 20), true), // 3² + 4² = 5²
        ((14, 20), false),
    ] {
        assert_eq!(
            view.is_visible(CellPos::new(cell.0, cell.1)),
            visible,
            "{cell:?}"
        );
    }
    assert!(!view.is_visible(CellPos::new(-1, 16)));
    assert!(!view.is_visible(CellPos::new(i32::MAX, i32::MIN)));
}

#[test]
fn a_building_sees_from_the_nearest_cell_of_its_footprint() {
    // The yard (sight 4) covers x 20..=22, y 20..=22.
    let mut app = scenario();
    building(&mut app, ENEMY, "yard", 20, 20);
    tick(&mut app);
    let view = vision(&app, ENEMY);
    let mut expected = BTreeSet::new();
    for y in 0..32 {
        for x in 0..32 {
            let dx = (20 - x).max(x - 22).max(0);
            let dy = (20 - y).max(y - 22).max(0);
            if dx * dx + dy * dy <= 16 {
                expected.insert(CellPos::new(x, y));
            }
        }
    }
    assert_eq!(visible_set(&view, 32, 32), expected);
    assert!(view.is_visible(CellPos::new(26, 21)));
    assert!(!view.is_visible(CellPos::new(27, 21)));
    assert!(!view.is_visible(CellPos::new(25, 25)), "3² + 3² > 4²");
    assert!(view.is_visible(CellPos::new(24, 25)), "2² + 3² ≤ 4²");
    assert_eq!(
        vision(&app, HUMAN).visible_cells(),
        0,
        "visions are per player"
    );
}

#[test]
fn enemies_are_visible_exactly_within_sight_and_explored_cells_stay_explored() {
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    // At the sight radius (5 cells) and one cell beyond it.
    let near = unit(&mut app, ENEMY, "crawler", 15, 12);
    let far = unit(&mut app, ENEMY, "crawler", 16, 12);
    tick(&mut app);
    // (15, 12) is 5 cells along x and 4 along y away: 41 > 25, so neither is visible; move the
    // rifleman to (10, 12).
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![rifleman],
            target: CellPos::new(10, 12),
        },
    );
    let events = ticks(&mut app, 60);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: rifleman,
        retargeted: false
    }));
    let view = vision(&app, HUMAN);
    assert!(view.is_visible(CellPos::new(15, 12)));
    assert!(!view.is_visible(CellPos::new(16, 12)));
    // The rifleman cannot attack either (both out of its range), but an attack on the visible
    // one is accepted and one on the hidden one is rejected as unknown.
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![rifleman],
            target: far,
        },
    );
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![rifleman],
            target: near,
        },
    );
    let events = tick(&mut app);
    assert_eq!(rejections(&events), [RejectReason::UnknownTarget(far)]);
    // Moving away: cells seen before stay explored but are no longer visible (fog), and cells
    // never seen stay unexplored (shroud).
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![rifleman],
            target: CellPos::new(2, 25),
        },
    );
    let events = ticks(&mut app, 400);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: rifleman,
        retargeted: false
    }));
    let view = vision(&app, HUMAN);
    assert!(view.is_explored(CellPos::new(15, 12)));
    assert!(!view.is_visible(CellPos::new(15, 12)));
    assert!(!view.is_explored(CellPos::new(30, 2)));
    assert!(view.visible_cells() > 0);
    assert!(view.explored_cells() > view.visible_cells());
}

#[test]
fn a_destroyed_unit_stops_seeing_in_the_tick_it_dies() {
    // Five enemy riflemen kill the human rifleman (100 hit points) with one volley; the human
    // player's vision, recomputed after the cleanup of that tick, is empty.
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    for (x, y) in [(12, 15), (12, 16), (12, 17), (11, 14), (11, 18)] {
        unit(&mut app, ENEMY, "rifleman", x, y);
    }
    tick(&mut app);
    assert!(vision(&app, HUMAN).visible_cells() > 0);
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: HUMAN,
        unit: rifleman
    }));
    assert_eq!(vision(&app, HUMAN).visible_cells(), 0);
    assert!(vision(&app, HUMAN).explored_cells() > 0);
}

// --- Target acquisition and orders -----------------------------------------------------------

#[test]
fn a_unit_on_a_plain_move_does_not_fire_and_an_attack_move_fights_on_the_way() {
    // A crawler stands two cells beside the rifleman's route along y = 16.
    let run = |command: fn(Vec<SimId>, CellPos) -> Command| {
        let mut app = scenario();
        let rifleman = unit(&mut app, HUMAN, "rifleman", 4, 16);
        let crawler = unit(&mut app, ENEMY, "crawler", 12, 18);
        tick(&mut app);
        issue(
            &mut app,
            HUMAN,
            command(vec![rifleman], CellPos::new(20, 16)),
        );
        let mut log = Vec::new();
        for _ in 0..600 {
            let events = tick(&mut app);
            let done = events.iter().any(
                |event| matches!(event, SimEvent::MoveCompleted { unit, .. } if *unit == rifleman),
            );
            log.extend(events);
            if done {
                break;
            }
        }
        let alive = exists(&mut app, crawler);
        let at = mobile(&mut app, rifleman).cell;
        let order = weapon(&mut app, rifleman).order;
        (log, rifleman, crawler, alive, at, order)
    };

    let (log, rifleman, _, alive, at, order) = run(|units, target| Command::Move { units, target });
    assert_eq!(shots_by(&log, rifleman), 0, "a plain move does not fire");
    assert!(alive);
    assert_eq!((at, order), (CellPos::new(20, 16), None));

    let (log, rifleman, crawler, alive, at, order) =
        run(|units, target| Command::AttackMove { units, target });
    assert!(!alive, "the attack move killed the crawler");
    assert_eq!(
        (at, order),
        (CellPos::new(20, 16), None),
        "and then arrived"
    );
    let position = |wanted: &SimEvent| log.iter().position(|event| event == wanted);
    let first_shot = log
        .iter()
        .position(|event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == rifleman))
        .expect("fired");
    let killed = position(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: crawler,
    })
    .expect("killed");
    let arrived = position(&SimEvent::MoveCompleted {
        unit: rifleman,
        retargeted: false,
    })
    .expect("arrived");
    assert!(first_shot < killed && killed < arrived);
    assert_eq!(shots_by(&log, rifleman), 3, "3 × 20 against 50 hit points");
}

#[test]
fn unarmed_units_follow_an_attack_move_as_a_move() {
    let mut app = scenario();
    let harvester = unit(&mut app, HUMAN, "harvester", 4, 16);
    tick(&mut app);
    issue(
        &mut app,
        HUMAN,
        Command::AttackMove {
            units: vec![harvester],
            target: CellPos::new(12, 20),
        },
    );
    let events = ticks(&mut app, 200);
    assert_eq!(rejections(&events), vec![]);
    assert!(events.contains(&SimEvent::MoveCompleted {
        unit: harvester,
        retargeted: false
    }));
    assert_eq!(harvester_state(&mut app, harvester), HarvestState::Idle);
}

fn harvester_state(app: &mut App, id: SimId) -> HarvestState {
    harvester_of(app, id).state
}

#[test]
fn an_attack_order_closes_in_fires_and_ends_when_the_target_is_gone() {
    // The tank sees the crawler five cells away (sight 6) but its range is four cells.
    let mut app = scenario();
    let tank = unit(&mut app, HUMAN, "tank", 3, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 8, 16);
    tick(&mut app);
    assert_eq!(
        shots_by(&tick(&mut app), tank),
        0,
        "out of range while idle"
    );
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![tank],
            target: crawler,
        },
    );
    let start = position(&mut app, tank);
    let mut log = Vec::new();
    for _ in 0..600 {
        let events = tick(&mut app);
        let ended = events
            .iter()
            .any(|event| matches!(event, SimEvent::AttackEnded { .. }));
        if shots_by(&events, tank) > 0
            && !log
                .iter()
                .any(|e| matches!(e, SimEvent::WeaponFired { .. }))
        {
            let at = position(&mut app, tank);
            assert!(at.x > start.x, "it moved toward the target first");
            let gap = i64::from(position(&mut app, crawler).x - at.x);
            assert!(
                sim::in_range(gap, 0, 1024),
                "it fired from within its range"
            );
        }
        log.extend(events);
        if ended {
            break;
        }
    }
    assert!(log.contains(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: crawler
    }));
    assert!(log.contains(&SimEvent::AttackEnded {
        unit: tank,
        target: crawler,
        reason: AttackEnd::TargetGone,
    }));
    assert_eq!(weapon(&mut app, tank).order, None);
    // It stays where it stopped.
    let stopped = mobile(&mut app, tank);
    ticks(&mut app, 50);
    assert_eq!(mobile(&mut app, tank).cell, stopped.next_centre());
}

#[test]
fn an_attack_order_ends_when_the_target_leaves_sight() {
    // The harvester (speed 64) drives away from the rifleman (speed 32) chasing it.
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 5, 16);
    let target = parked_harvester(&mut app, ENEMY, 9, 16);
    tick(&mut app);
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![rifleman],
            target,
        },
    );
    issue(
        &mut app,
        ENEMY,
        Command::Move {
            units: vec![target],
            target: CellPos::new(30, 16),
        },
    );
    let events = ticks(&mut app, 300);
    assert_eq!(rejections(&events), vec![]);
    assert!(events.contains(&SimEvent::AttackEnded {
        unit: rifleman,
        target,
        reason: AttackEnd::TargetHidden,
    }));
    assert_eq!(shots_by(&events, rifleman), 0);
    assert_eq!(weapon(&mut app, rifleman).order, None);
}

#[test]
fn an_attack_order_on_a_target_out_of_reach_ends_at_once() {
    // The crawler sits in a pocket inside a rock square four cells thick: from every reachable
    // cell it is at least five cells away, beyond the rifle's three, though within its sight.
    let mut app = scenario();
    for y in 16..=24 {
        for x in 12..=20 {
            if (x, y) != (16, 20) {
                app.world_mut()
                    .resource_mut::<sim::Grid>()
                    .set_terrain(CellPos::new(x, y), ROCK);
            }
        }
    }
    let crawler = unit(&mut app, ENEMY, "crawler", 16, 20);
    let rifleman = unit(&mut app, HUMAN, "rifleman", 16, 25);
    tick(&mut app);
    assert!(vision(&app, HUMAN).is_visible(CellPos::new(16, 20)));
    issue(
        &mut app,
        HUMAN,
        Command::Attack {
            units: vec![rifleman],
            target: crawler,
        },
    );
    let events = tick(&mut app);
    assert_eq!(rejections(&events), vec![]);
    assert!(events.contains(&SimEvent::AttackEnded {
        unit: rifleman,
        target: crawler,
        reason: AttackEnd::OutOfReach,
    }));
    assert_eq!(weapon(&mut app, rifleman).order, None);
}

#[test]
fn defensive_structures_hold_fire_under_low_power() {
    // The turret (30 power) fires while the plant (100) runs, stops when it is sold (the yard
    // alone produces 10), and fires again once a new plant is placed.
    let mut app = scenario();
    building(&mut app, HUMAN, "yard", 2, 13);
    let plant = building(&mut app, HUMAN, "power", 6, 16);
    let turret = building(&mut app, HUMAN, "turret", 10, 16);
    // A harvester (200 hit points, 16 per shot) two cells away.
    let target = parked_harvester(&mut app, ENEMY, 12, 16);
    let events = ticks(&mut app, 40);
    assert!(shots_by(&events, turret) >= 1);
    issue(&mut app, HUMAN, Command::SellBuilding { building: plant });
    tick(&mut app);
    let state = app
        .world()
        .resource::<Players>()
        .get(HUMAN)
        .expect("human")
        .clone();
    assert!(state.low_power());
    let events = ticks(&mut app, 120);
    assert_eq!(shots_by(&events, turret), 0, "no fire under low power");
    // A projectile in flight when the power failed has landed by now; nothing hits after it.
    let after = hit_points(&mut app, target);
    assert!(after.is_some());
    let events = ticks(&mut app, 60);
    assert_eq!(shots_by(&events, turret), 0);
    assert_eq!(hit_points(&mut app, target), after);
    // A new plant restores the power.
    let power = definitions().building_kind("power").expect("power");
    issue(
        &mut app,
        HUMAN,
        Command::QueueProduction {
            item: sim::ProductionItem::Building(power),
        },
    );
    // Under low power production progresses on even ticks only: 10 ticks of progress take 19
    // or 20 ticks.
    let events = ticks(&mut app, 21);
    assert_eq!(rejections(&events), vec![]);
    assert!(events.contains(&SimEvent::ProductionComplete {
        player: HUMAN,
        item: sim::ProductionItem::Building(power),
    }));
    issue(
        &mut app,
        HUMAN,
        Command::PlaceBuilding {
            building: power,
            origin: CellPos::new(6, 13),
        },
    );
    let events = ticks(&mut app, 40);
    assert_eq!(rejections(&events), vec![]);
    assert!(shots_by(&events, turret) >= 1, "it fires again");
}

#[test]
fn a_turret_fires_beyond_its_own_sight_only_at_enemies_its_owner_sees() {
    // The turret sees three cells but reaches six: a crawler five cells away is in range but
    // unseen until a rifleman of the same player walks up and sees it.
    let mut app = scenario();
    building(&mut app, HUMAN, "power", 2, 2);
    let turret = building(&mut app, HUMAN, "turret", 10, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 15, 16);
    let rifleman = unit(&mut app, HUMAN, "rifleman", 2, 24);
    let events = ticks(&mut app, 60);
    assert_eq!(
        shots_by(&events, turret),
        0,
        "unseen enemies are not attacked"
    );
    assert!(!vision(&app, HUMAN).is_visible(CellPos::new(15, 16)));
    // From (12, 18) the rifleman sees the crawler (3² + 2² ≤ 5²), out of its own range.
    issue(
        &mut app,
        HUMAN,
        Command::Move {
            units: vec![rifleman],
            target: CellPos::new(12, 18),
        },
    );
    let mut fired = false;
    for _ in 0..200 {
        let events = tick(&mut app);
        if events.contains(&SimEvent::WeaponFired {
            attacker: turret,
            target: crawler,
            weapon: definitions().weapon_kind("howitzer").expect("howitzer"),
        }) {
            assert!(vision(&app, HUMAN).is_visible(CellPos::new(15, 16)));
            fired = true;
            break;
        }
    }
    assert!(fired);
}

// --- Harvesters and combat -------------------------------------------------------------------

#[test]
fn a_refinery_destroyed_during_the_return_trip_leaves_the_harvester_idle_with_its_load() {
    // M3 tested a refinery sold during the return trip; from M4 refineries are also destroyed.
    // The refinery covers x 8..=10, y 24..=25, far from the ore field (x 22..=25, y 6..=9), so
    // the return trip is long. The tanks wait on the bottom row, 1,409 subcells from its
    // footprint (out of their range of 1,024) and six rows from the harvester's route, and the
    // tanks below it see it (sight 6).
    let mut app = scenario();
    let refinery = building(&mut app, HUMAN, "refinery", 8, 24);
    let harvester = unit(&mut app, HUMAN, "harvester", 21, 10);
    let tanks: Vec<SimId> = (3..=15)
        .map(|x| unit(&mut app, ENEMY, "tank", x, 31))
        .collect();
    let mut set_out = false;
    for _ in 0..400 {
        tick(&mut app);
        if matches!(
            harvester_state(&mut app, harvester),
            HarvestState::ToRefinery { .. }
        ) {
            set_out = true;
            break;
        }
    }
    assert!(set_out, "the harvester never filled up");
    let load = harvester_of(&mut app, harvester).load;
    assert_eq!(load, 20);
    issue(
        &mut app,
        ENEMY,
        Command::Attack {
            units: tanks,
            target: refinery,
        },
    );
    let mut log = Vec::new();
    for _ in 0..400 {
        let events = tick(&mut app);
        let idle = events.contains(&SimEvent::HarvesterIdle {
            unit: harvester,
            reason: HarvestIdle::NoRefinery,
        });
        log.extend(events);
        if idle {
            break;
        }
    }
    assert_eq!(rejections(&log), vec![]);
    let destroyed = log
        .iter()
        .position(|event| matches!(event, SimEvent::BuildingDestroyed { building, .. } if *building == refinery))
        .expect("destroyed");
    let idle = log
        .iter()
        .position(|event| {
            *event
                == SimEvent::HarvesterIdle {
                    unit: harvester,
                    reason: HarvestIdle::NoRefinery,
                }
        })
        .expect("idle");
    assert!(destroyed < idle);
    assert!(
        !log.iter()
            .any(|event| matches!(event, SimEvent::OreDelivered { .. })),
        "nothing was delivered"
    );
    let state = harvester_of(&mut app, harvester);
    assert_eq!((state.state, state.load), (HarvestState::Idle, load));
}

// --- Determinism and the hash ----------------------------------------------------------------

/// Twenty units a side under fog: each side attack-moves to where it last saw the enemy, or to
/// the far side of the map, every 150 ticks, until one side has no unit left.
fn battle() -> (Vec<u64>, BTreeMap<PlayerId, usize>, u64, usize, usize) {
    let mut app = scenario();
    let mut armies: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
    // The human side: 12 tanks and 8 riflemen on x 2..=5; the enemy: 8 tanks and 12 riflemen on
    // x 26..=29; both on the grass rows y 14..=18.
    for (player, x0, tanks) in [(HUMAN, 2, 12), (ENEMY, 26, 8)] {
        let placed = armies.entry(player).or_default();
        for y in 14..=18 {
            for x in x0..x0 + 4 {
                let kind = if placed.len() < tanks {
                    "tank"
                } else {
                    "rifleman"
                };
                placed.push(unit(&mut app, player, kind, x, y));
            }
        }
    }
    for army in armies.values() {
        assert_eq!(army.len(), 20);
    }
    let far = BTreeMap::from([(HUMAN, CellPos::new(28, 18)), (ENEMY, CellPos::new(3, 18))]);
    let mut trace = Vec::new();
    let mut fired = 0;
    let mut destroyed = 0;
    for _ in 0..8_000 {
        let at = next_tick(&app);
        if at.is_multiple_of(150) {
            for (&player, army) in &armies {
                let alive: Vec<SimId> = army
                    .iter()
                    .copied()
                    .filter(|&id| exists(&mut app, id))
                    .collect();
                if alive.is_empty() {
                    continue;
                }
                // The nearest enemy the player sees, else the far side.
                let view = vision(&app, player);
                let enemy = if player == HUMAN { ENEMY } else { HUMAN };
                let mut seen = Vec::new();
                for &id in &armies[&enemy] {
                    if exists(&mut app, id) {
                        let cell = mobile(&mut app, id).cell;
                        if view.is_visible(cell) {
                            seen.push(cell);
                        }
                    }
                }
                let target = seen.first().copied().unwrap_or(far[&player]);
                issue(
                    &mut app,
                    player,
                    Command::AttackMove {
                        units: alive,
                        target,
                    },
                );
            }
        }
        let events = tick(&mut app);
        assert_eq!(rejections(&events), vec![]);
        fired += events
            .iter()
            .filter(|event| matches!(event, SimEvent::WeaponFired { .. }))
            .count();
        destroyed += events
            .iter()
            .filter(|event| matches!(event, SimEvent::UnitDestroyed { .. }))
            .count();
        trace.push(hash(&mut app));
        let survivors: BTreeMap<PlayerId, usize> = armies
            .iter()
            .map(|(&player, army)| {
                (
                    player,
                    army.iter().filter(|&&id| exists(&mut app, id)).count(),
                )
            })
            .collect();
        if survivors.values().any(|&count| count == 0) {
            return (trace, survivors, next_tick(&app), fired, destroyed);
        }
    }
    panic!("no side was destroyed within 8,000 ticks");
}

#[test]
fn twenty_against_twenty_fight_to_a_conclusion_under_fog_with_a_stable_hash() {
    let (first, survivors, ended, fired, destroyed) = battle();
    println!(
        "battle: ended at tick {ended}, survivors {survivors:?}, {fired} shots, {destroyed} \
         units destroyed, final hash {:016x}",
        first.last().copied().unwrap_or_default()
    );
    let (second, survivors_again, ended_again, ..) = battle();
    assert_eq!(first, second, "the same hash at every tick");
    assert_eq!((survivors.clone(), ended), (survivors_again, ended_again));
    // One side is destroyed; the stronger human side (12 tanks) wins.
    assert_eq!(survivors[&ENEMY], 0, "{survivors:?} at tick {ended}");
    assert!(survivors[&HUMAN] > 0);
    assert!(destroyed >= 20);
    assert!(fired > destroyed);
}

#[test]
fn the_hash_covers_hit_points_weapons_and_vision() {
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    tick(&mut app);
    let base = hash(&mut app);
    let entity = {
        let world = app.world_mut();
        let mut query = world.query::<(bevy_ecs::entity::Entity, &SimId)>();
        query
            .iter(world)
            .find(|(_, id)| **id == rifleman)
            .map(|(entity, _)| entity)
            .expect("rifleman")
    };
    type Change = Box<dyn Fn(&mut App)>;
    let mut changes: Vec<Change> = vec![
        Box::new(move |app| {
            app.world_mut()
                .get_mut::<Health>(entity)
                .expect("health")
                .hit_points -= 1;
        }),
        Box::new(move |app| {
            app.world_mut()
                .get_mut::<Weapon>(entity)
                .expect("weapon")
                .cooldown += 1;
        }),
        Box::new(move |app| {
            app.world_mut()
                .get_mut::<Weapon>(entity)
                .expect("weapon")
                .target = Some(SimId(77));
        }),
        Box::new(move |app| {
            app.world_mut()
                .get_mut::<Weapon>(entity)
                .expect("weapon")
                .order = Some(AttackOrder::AttackMove {
                destination: CellPos::new(1, 1),
                moving: false,
            });
        }),
    ];
    for (index, change) in changes.drain(..).enumerate() {
        let mut copy = scenario();
        let id = unit(&mut copy, HUMAN, "rifleman", 10, 16);
        assert_eq!(id, rifleman);
        tick(&mut copy);
        assert_eq!(hash(&mut copy), base);
        change(&mut copy);
        assert_ne!(hash(&mut copy), base, "change {index}");
    }
    // Projectiles and vision: a tank that fired and an identical one whose shot was not yet
    // fired differ; so do two players' visions that differ in what they explored (both are
    // checked through the hash of whole scenarios that differ only there).
    let shoot = |fire: bool| {
        let mut app = scenario();
        unit(&mut app, HUMAN, "tank", 10, 16);
        unit(&mut app, ENEMY, "crawler", 13, 16);
        tick(&mut app);
        if fire {
            tick(&mut app);
        }
        app
    };
    let mut fired = shoot(true);
    assert_eq!(fired.world().resource::<Projectiles>().len(), 1);
    let mut idle = shoot(false);
    assert_ne!(hash(&mut fired), hash(&mut idle));
    let mut units = app.world_mut().query::<&Unit>();
    assert_eq!(units.iter(app.world()).count(), 1);
}

// --- M4 remediation round 1: regression tests -------------------------------------------------

/// The fixture definitions with `edits` applied to the text of `combat.ron`, each of which must
/// change it, written to a file of its own under `name`.
fn combat_variant(name: &str, edits: &[(&str, &str)]) -> Definitions {
    let mut text = std::fs::read_to_string(fixture("data/combat.ron")).expect("fixture");
    for (from, to) in edits {
        let edited = text.replacen(from, to, 1);
        assert_ne!(edited, text, "{from:?} is not in the fixture");
        text = edited;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m4_combat");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(format!("{name}.ron"));
    std::fs::write(&path, text).expect("variant");
    sim::load_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
    )
    .and_then(|definitions| definitions.with_combat(&path))
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The fixture's only pair of equation 2 with a percentage of 0: the rifle's bullet against the
/// concrete of every building.
fn assert_bullet_cannot_damage_concrete() {
    let defs = definitions();
    let rifle = defs
        .weapon(defs.weapon_kind("rifle").expect("rifle"))
        .expect("def");
    let fence = defs
        .building_combat(defs.building_kind("fence").expect("fence"))
        .expect("terms");
    assert_eq!(defs.versus(rifle.warhead, fence.armour), Some(0));
}

#[test]
fn an_attack_move_past_an_enemy_it_cannot_damage_ends_when_its_move_ends() {
    // M4 review 1 P1-1: a fence (concrete, 0 % from the bullet) stands two cells beside the
    // rifleman's route along y = 16, in its range and sight. It is no target, so the attack move
    // does not stop for it and ends with its move.
    assert_bullet_cannot_damage_concrete();
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 4, 16);
    let fence = building(&mut app, ENEMY, "fence", 12, 18);
    tick(&mut app);
    issue(
        &mut app,
        HUMAN,
        Command::AttackMove {
            units: vec![rifleman],
            target: CellPos::new(20, 16),
        },
    );
    let mut log = Vec::new();
    let mut passed_in_range = false;
    for _ in 0..600 {
        let events = tick(&mut app);
        let at = position(&mut app, rifleman);
        // The fence's footprint begins at y = 18 × 256; the rifle reaches 768 subcells.
        passed_in_range |= vision(&app, HUMAN).is_visible(CellPos::new(12, 18))
            && sim::in_range(
                i64::from(3072 - at.x).max(i64::from(at.x - 3327)).max(0),
                i64::from(4608 - at.y),
                768,
            );
        let done = events.iter().any(
            |event| matches!(event, SimEvent::MoveCompleted { unit, .. } if *unit == rifleman),
        );
        log.extend(events);
        if done {
            break;
        }
    }
    assert!(passed_in_range, "the fence was a visible enemy in range");
    assert!(
        log.contains(&SimEvent::MoveCompleted {
            unit: rifleman,
            retargeted: false
        }),
        "the attack move ended"
    );
    assert_eq!(shots_by(&log, rifleman), 0);
    assert_eq!(hit_points(&mut app, fence), Some(50));
    assert_eq!(mobile(&mut app, rifleman).cell, CellPos::new(20, 16));
    let weapon = weapon(&mut app, rifleman);
    assert_eq!((weapon.order, weapon.target), (None, None));
}

#[test]
fn an_idle_unit_fires_at_an_enemy_it_can_damage_and_not_at_a_nearer_one_it_cannot() {
    // M4 review 1 P1-1: an enemy fence stands next to the idle rifleman and an enemy rifleman a
    // little farther; both are in range. The rifleman returns fire at the rifleman.
    assert_bullet_cannot_damage_concrete();
    let mut app = scenario();
    let rifleman = unit(&mut app, HUMAN, "rifleman", 10, 16);
    let fence = building(&mut app, ENEMY, "fence", 11, 16);
    let enemy = unit(&mut app, ENEMY, "rifleman", 12, 17);
    let rifle = definitions().weapon_kind("rifle").expect("rifle");
    tick(&mut app);
    let events = tick(&mut app);
    assert!(events.contains(&SimEvent::WeaponFired {
        attacker: rifleman,
        target: enemy,
        weapon: rifle,
    }));
    let events = [events, ticks(&mut app, 60)].concat();
    let at_fence = events
        .iter()
        .filter(|event| {
            matches!(event, SimEvent::WeaponFired { attacker, target, .. }
                if *attacker == rifleman && *target == fence)
        })
        .count();
    assert_eq!(at_fence, 0, "no shot at the fence");
    // Five shots of 20 each way: both riflemen fall in the same tick.
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: enemy
    }));
    assert_eq!(hit_points(&mut app, fence), Some(50));
}

#[test]
fn a_defensive_structure_fires_at_an_enemy_it_can_damage_and_not_at_a_nearer_one_it_cannot() {
    // M4 review 1 P1-1 for buildings: with the howitzer's warhead at 0 % against concrete, an
    // enemy fence next to the turret is no target; a crawler three cells away is.
    let mut app = scenario();
    app.world_mut().insert_resource(combat_variant(
        "he_versus_concrete_0",
        &[(
            "(armour: \"concrete\", percent: 75)",
            "(armour: \"concrete\", percent: 0)",
        )],
    ));
    building(&mut app, HUMAN, "power", 2, 2);
    let turret = building(&mut app, HUMAN, "turret", 10, 16);
    let fence = building(&mut app, ENEMY, "fence", 11, 16);
    let crawler = unit(&mut app, ENEMY, "crawler", 13, 16);
    let howitzer = definitions().weapon_kind("howitzer").expect("howitzer");
    let events = ticks(&mut app, 120);
    assert!(events.contains(&SimEvent::WeaponFired {
        attacker: turret,
        target: crawler,
        weapon: howitzer,
    }));
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: ENEMY,
        unit: crawler
    }));
    assert_eq!(
        shots_by(&events, turret),
        1,
        "one shot (60 against 50) and no more"
    );
    assert_eq!(hit_points(&mut app, fence), Some(50));
    assert_eq!(weapon(&mut app, turret).target, None);
}

#[test]
fn an_attack_order_on_a_target_it_cannot_damage_ends_after_one_shot() {
    // The class of M4 review 1 P1-1 for explicit orders: the rifleman fires once at the fence, as
    // ordered (equation 2 gives 0), and the order ends with `NoDamage` instead of lasting for as
    // long as the fence stands. Out of range (from x 8: 896 subcells, though in sight), it first
    // moves into range.
    assert_bullet_cannot_damage_concrete();
    let rifle = definitions().weapon_kind("rifle").expect("rifle");
    for (start, fence_x) in [(10, 12), (8, 12)] {
        let mut app = scenario();
        let rifleman = unit(&mut app, HUMAN, "rifleman", start, 16);
        let fence = building(&mut app, ENEMY, "fence", fence_x, 16);
        tick(&mut app);
        issue(
            &mut app,
            HUMAN,
            Command::Attack {
                units: vec![rifleman],
                target: fence,
            },
        );
        let mut log = Vec::new();
        let mut ended_at = None;
        for _ in 0..600 {
            let events = tick(&mut app);
            let ended = events.contains(&SimEvent::AttackEnded {
                unit: rifleman,
                target: fence,
                reason: AttackEnd::NoDamage,
            });
            log.extend(events);
            if ended {
                ended_at = Some(log.len());
                break;
            }
        }
        assert!(ended_at.is_some(), "from x {start}: the order never ended");
        assert_eq!(rejections(&log), vec![]);
        let shots: Vec<&SimEvent> = log
            .iter()
            .filter(|event| matches!(event, SimEvent::WeaponFired { .. }))
            .collect();
        assert_eq!(
            shots,
            [&SimEvent::WeaponFired {
                attacker: rifleman,
                target: fence,
                weapon: rifle,
            }]
        );
        assert_eq!(hit_points(&mut app, fence), Some(50));
        let weapon_now = weapon(&mut app, rifleman);
        assert_eq!((weapon_now.order, weapon_now.target), (None, None));
        // Idle afterwards, it does not take the fence as a target either.
        let events = ticks(&mut app, 100);
        assert_eq!(shots_by(&events, rifleman), 0);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SimEvent::AttackEnded { .. }))
        );
    }
}

/// The human player's yard at (10, 10) (3 × 3, sight 4) with a ready power plant (2 × 2), and
/// `enemy` (a unit or building type, if any) at (16, 14) for the ENEMY player.
fn placement_scenario(enemy: Option<(&str, bool)>) -> (App, Option<SimId>) {
    let mut app = scenario();
    building(&mut app, HUMAN, "yard", 10, 10);
    let blocker = enemy.map(|(kind, is_building)| {
        if is_building {
            building(&mut app, ENEMY, kind, 16, 14)
        } else {
            unit(&mut app, ENEMY, kind, 16, 14)
        }
    });
    let power = definitions().building_kind("power").expect("power");
    issue(
        &mut app,
        HUMAN,
        Command::QueueProduction {
            item: sim::ProductionItem::Building(power),
        },
    );
    let mut ready = false;
    for _ in 0..40 {
        let events = tick(&mut app);
        assert_eq!(rejections(&events), vec![]);
        ready |= events.contains(&SimEvent::ProductionComplete {
            player: HUMAN,
            item: sim::ProductionItem::Building(power),
        });
        if ready {
            break;
        }
    }
    assert!(ready);
    (app, blocker)
}

fn place_power(app: &mut App, x: i32, y: i32) -> Vec<SimEvent> {
    let power = definitions().building_kind("power").expect("power");
    issue(
        app,
        HUMAN,
        Command::PlaceBuilding {
            building: power,
            origin: CellPos::new(x, y),
        },
    );
    tick(app)
}

#[test]
fn a_placement_over_cells_the_player_does_not_see_reveals_nothing_about_them() {
    // M4 review 1 P1-2: the footprint x 15..=16, y 13..=14 lies within the placement distance (3)
    // of the yard, but (16, 13) and (16, 14) lie beyond the yard's sight. Whether an enemy unit,
    // an enemy building, or nothing stands at (16, 14), the placement is rejected for the first
    // unseen cell, the same reason, which names no unit or building.
    let mut reasons = Vec::new();
    for enemy in [None, Some(("crawler", false)), Some(("beacon", true))] {
        let (mut app, blocker) = placement_scenario(enemy);
        let view = vision(&app, HUMAN);
        assert!(view.is_visible(CellPos::new(15, 13)) && view.is_visible(CellPos::new(15, 14)));
        assert!(!view.is_visible(CellPos::new(16, 13)) && !view.is_visible(CellPos::new(16, 14)));
        let (mut control, _) = placement_scenario(enemy);
        let events = place_power(&mut app, 15, 13);
        let found = rejections(&events);
        assert_eq!(
            found,
            [RejectReason::Placement(sim::PlacementError::NotVisible(
                CellPos::new(16, 13)
            ))],
            "{enemy:?}"
        );
        tick(&mut control);
        assert_eq!(
            hash(&mut app),
            hash(&mut control),
            "the rejection changed nothing"
        );
        if let Some(blocker) = blocker {
            assert!(!format!("{found:?}").contains(&format!("SimId({})", blocker.0)));
        }
        reasons.push(found);
        // The building is still ready and is placed where the player sees every cell (x 14..=15,
        // y 11..=12).
        let events = place_power(&mut app, 14, 11);
        assert_eq!(rejections(&events), vec![]);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, SimEvent::BuildingPlaced { player: HUMAN, .. }))
        );
    }
    assert!(reasons.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn a_placement_over_a_visible_enemy_still_names_it() {
    // Hiding applies to unseen cells only: an enemy crawler the player sees at (15, 12) blocks
    // the footprint x 14..=15, y 11..=12 and is named, as in M3.
    let mut app = scenario();
    building(&mut app, HUMAN, "yard", 10, 10);
    let crawler = unit(&mut app, ENEMY, "crawler", 15, 12);
    let power = definitions().building_kind("power").expect("power");
    issue(
        &mut app,
        HUMAN,
        Command::QueueProduction {
            item: sim::ProductionItem::Building(power),
        },
    );
    ticks(&mut app, 12);
    assert!(vision(&app, HUMAN).is_visible(CellPos::new(15, 12)));
    let events = place_power(&mut app, 14, 11);
    assert_eq!(
        rejections(&events),
        [RejectReason::Placement(sim::PlacementError::Occupied {
            cell: CellPos::new(15, 12),
            unit: crawler,
        })]
    );
}
