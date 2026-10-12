//! The scenario helpers the RA3 tests share (`ra3_*.rs`): a small open world on the RA3 fixture
//! (`tests/fixtures/data/ra3`: the world of the RA2 fixture with an aircraft, ships, specialists
//! and combat terms that use every field of the combat file's schema version 2; invariant I10),
//! driven through `CommandQueue` and `SimSchedule`.
#![allow(dead_code)]

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Grid, Health, IssuedCommand, Mobile,
    PlayerId, Players, Position, SimEvent, SimId, SimPlugin, SimSchedule, SubPos, Weapon,
    WeaponKind, load_match_definitions, spawn_building, spawn_unit,
};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

pub fn fixture(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/ra3")
        .join(file)
}

/// The definitions of the RA3 fixture, its combat file with each `(from, to)` edit applied
/// (`from` must occur exactly once).
pub fn definitions_with(name: &str, edits: &[(&str, &str)]) -> Definitions {
    let combat = if edits.is_empty() {
        fixture("combat.ron")
    } else {
        let mut text = std::fs::read_to_string(fixture("combat.ron")).expect("fixture");
        for (from, to) in edits {
            assert_eq!(
                text.matches(from).count(),
                1,
                "{name}: fixture edit {from:?}"
            );
            text = text.replacen(from, to, 1);
        }
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra3");
        std::fs::create_dir_all(&dir).expect("temporary directory");
        let path = dir.join(format!("{name}_{}.ron", std::process::id()));
        std::fs::write(&path, text).expect("variant");
        path
    };
    load_match_definitions(fixture("units.ron"), fixture("economy.ron"), combat)
        .and_then(|definitions| definitions.with_rules(fixture("rules.ron")))
        .and_then(|definitions| definitions.with_arsenal(fixture("arsenal.ron")))
        .unwrap_or_else(|error| panic!("{error}"))
}

pub fn definitions() -> Definitions {
    definitions_with("fixture", &[])
}

/// A world of 40 x 40 open cells with the players P0 (country alpha, Allied) and P1 (gamma,
/// Soviet), who are enemies, with `definitions` and the seed `seed`.
pub fn world_of(definitions: Definitions, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    let cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(40, 40, vec![cell; 1600]).expect("a grid"));
    for (player, country) in [(P0, "alpha"), (P1, "gamma")] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 5_000).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

pub fn world() -> App {
    world_of(definitions(), 7)
}

/// The world with the fixture's combat file edited.
pub fn world_with(name: &str, edits: &[(&str, &str)]) -> App {
    world_of(definitions_with(name, edits), 7)
}

pub fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

/// Makes the cells (`x0`..=`x1`, `y0`..=`y1`) water.
pub fn flood(app: &mut App, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    let mut grid = app.world_mut().resource_mut::<Grid>();
    for y in y0..=y1 {
        for x in x0..=x1 {
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

pub fn place(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

pub fn spawn(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_unit(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// Runs one tick with `commands` and returns its events.
pub fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
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

pub fn ticks(app: &mut App, count: u32) -> Vec<SimEvent> {
    (0..count).flat_map(|_| tick(app, Vec::new())).collect()
}

pub fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

pub fn attack(player: PlayerId, unit: SimId, target: SimId) -> IssuedCommand {
    issue(
        player,
        Command::Attack {
            units: vec![unit],
            target,
        },
    )
}

/// The hit points of the unit or building `id`, or `None` once it is gone.
pub fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Health)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, health)| health.hit_points)
}

pub fn set_hp(app: &mut App, id: SimId, hit_points: i32) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut Health)>();
    for (other, mut health) in query.iter_mut(world) {
        if *other == id {
            health.hit_points = hit_points;
        }
    }
}

pub fn weapon(app: &mut App, id: SimId) -> Weapon {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Weapon)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, weapon)| weapon)
        .unwrap_or_else(|| panic!("{id:?} has no weapon"))
}

pub fn mobile(app: &mut App, id: SimId) -> Mobile {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Mobile)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, mobile)| mobile)
        .unwrap_or_else(|| panic!("{id:?} is not a unit"))
}

pub fn position(app: &mut App, id: SimId) -> SubPos {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Position)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, position)| position.0)
        .unwrap_or_else(|| panic!("{id:?} is not a unit"))
}

pub fn weapon_kind(app: &App, id: &str) -> WeaponKind {
    defs(app)
        .weapon_kind(id)
        .unwrap_or_else(|| panic!("weapon {id}"))
}

/// The weapons `attacker` fired in `events`, in order, each with its target.
pub fn shots(events: &[SimEvent], attacker: SimId) -> Vec<(SimId, WeaponKind)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::WeaponFired {
                attacker: by,
                target,
                weapon,
            } if *by == attacker => Some((*target, *weapon)),
            _ => None,
        })
        .collect()
}

/// The points of the impacts reported in `events`.
pub fn impacts(events: &[SimEvent]) -> Vec<(SubPos, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::Impact { at, air, .. } => Some((*at, *air)),
            _ => None,
        })
        .collect()
}
