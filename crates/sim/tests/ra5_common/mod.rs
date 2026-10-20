//! The scenario helpers the RA5 tests share (`ra5_*.rs`): a small open world on the RA5 fixture
//! (`tests/fixtures/data/ra5`: the world of the RA4 fixture with the ore and the crates of RA5;
//! invariant I10), driven through `CommandQueue` and `SimSchedule`.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Experience, Grid, Harvester, Health,
    IssuedCommand, Mobile, Owner, PlayerId, Players, SimEvent, SimId, SimPlugin, SimSchedule, Unit,
    load_match_definitions, spawn_building, spawn_unit,
};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

pub fn fixture(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/ra5")
        .join(file)
}

/// The path of the fixture's `file` with each `(from, to)` edit applied (`from` must occur
/// exactly once); the fixture's own file without edits.
pub fn variant(name: &str, file: &str, edits: &[(&str, &str)]) -> PathBuf {
    static NEXT_VARIANT: AtomicU64 = AtomicU64::new(0);
    if edits.is_empty() {
        return fixture(file);
    }
    let mut text = std::fs::read_to_string(fixture(file)).expect("fixture");
    for (from, to) in edits {
        assert_eq!(
            text.matches(from).count(),
            1,
            "{name}: fixture edit {from:?}"
        );
        text = text.replacen(from, to, 1);
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra5");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let sequence = NEXT_VARIANT.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("{name}_{}_{sequence}_{file}", std::process::id()));
    std::fs::write(&path, text).expect("variant");
    path
}

/// The definitions of the RA5 fixture with the economy file `economy` and the rules file
/// `rules`, or the error of the first file that is rejected.
pub fn load(economy: PathBuf, rules: PathBuf) -> Result<Definitions, sim::DataError> {
    load_match_definitions(fixture("units.ron"), economy, fixture("combat.ron"))
        .and_then(|definitions| definitions.with_rules(rules))
        .and_then(|definitions| definitions.with_arsenal(fixture("arsenal.ron")))
}

/// The definitions of the RA5 fixture with the edits `rules` to its rules file.
pub fn definitions_with(name: &str, rules: &[(&str, &str)]) -> Definitions {
    load(fixture("economy.ron"), variant(name, "rules.ron", rules))
        .unwrap_or_else(|error| panic!("{error}"))
}

pub fn definitions() -> Definitions {
    definitions_with("fixture", &[])
}

/// The edits that leave `content` as the only content of a crate (the others get a share of
/// 0).
pub fn only(content: &str) -> Vec<(&'static str, &'static str)> {
    [
        (
            "money",
            r#"(id: "money", share: 20,"#,
            r#"(id: "money", share: 0,"#,
        ),
        (
            "unit",
            r#"(id: "unit", share: 20,"#,
            r#"(id: "unit", share: 0,"#,
        ),
        (
            "veteran",
            r#"(id: "veteran", share: 20,"#,
            r#"(id: "veteran", share: 0,"#,
        ),
        (
            "heal_base",
            r#"(id: "heal_base", share: 10,"#,
            r#"(id: "heal_base", share: 0,"#,
        ),
        (
            "armour",
            r#"(id: "armour", share: 10,"#,
            r#"(id: "armour", share: 0,"#,
        ),
        (
            "firepower",
            r#"(id: "firepower", share: 10,"#,
            r#"(id: "firepower", share: 0,"#,
        ),
        (
            "speed",
            r#"(id: "speed", share: 10,"#,
            r#"(id: "speed", share: 0,"#,
        ),
        (
            "reveal",
            r#"(id: "reveal", share: 10,"#,
            r#"(id: "reveal", share: 0,"#,
        ),
    ]
    .into_iter()
    .filter(|(id, ..)| *id != content)
    .map(|(_, from, to)| (from, to))
    .collect()
}

pub const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};

/// A world of 40 x 40 open cells with the players P0 (country alpha, Allied) and P1 (gamma,
/// Soviet), who are enemies, with `definitions` and the seed `seed`. It has no match: no crate
/// appears by itself.
pub fn world_of(definitions: Definitions, seed: u64) -> App {
    world_with_credits(definitions, seed, [5_000, 5_000])
}

/// [`world_of`] in which P0 and P1 start with `credits`.
pub fn world_with_credits(definitions: Definitions, seed: u64, credits: [i64; 2]) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    app.world_mut()
        .insert_resource(Grid::from_cells(40, 40, vec![OPEN; 1600]).expect("a grid"));
    for ((player, country), credits) in [(P0, "alpha"), (P1, "gamma")].into_iter().zip(credits) {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, credits).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

pub fn world() -> App {
    world_of(definitions(), 7)
}

pub fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

/// Gives the cell (`x`, `y`) the terrain `cell`.
pub fn terrain(app: &mut App, x: i32, y: i32, cell: Cell) {
    assert!(
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(CellPos::new(x, y), cell)
    );
}

/// Puts `bails` bails of ore into the cell (`x`, `y`): ore ground, which is no building ground.
pub fn ore(app: &mut App, x: i32, y: i32, bails: u32) {
    terrain(
        app,
        x,
        y,
        Cell {
            buildable: false,
            ore: bails,
            ..OPEN
        },
    );
}

/// Puts `bails` bails of gems into the cell (`x`, `y`).
pub fn gems(app: &mut App, x: i32, y: i32, bails: u32) {
    ore(app, x, y, bails);
    assert!(
        app.world_mut()
            .resource_mut::<Grid>()
            .set_gems(CellPos::new(x, y), true)
    );
}

/// The bails in the cell (`x`, `y`).
pub fn bails(app: &App, x: i32, y: i32) -> u32 {
    app.world()
        .resource::<Grid>()
        .cell(CellPos::new(x, y))
        .expect("a cell")
        .ore
}

/// Every cell with ore or gems and its bails, in row-major order.
pub fn field(app: &App) -> Vec<(CellPos, u32)> {
    let grid = app.world().resource::<Grid>();
    let mut cells = Vec::new();
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            let cell = CellPos::new(x, y);
            let ore = grid.cell(cell).expect("a cell").ore;
            if ore > 0 {
                cells.push((cell, ore));
            }
        }
    }
    cells
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

pub fn move_to(player: PlayerId, unit: SimId, x: i32, y: i32) -> IssuedCommand {
    issue(
        player,
        Command::Move {
            units: vec![unit],
            target: CellPos::new(x, y),
        },
    )
}

pub fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("a player")
        .credits()
}

/// The rejections among `events`.
pub fn rejections(events: &[SimEvent]) -> Vec<sim::RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

/// The component `T` of the unit or building `id`.
pub fn component<T: bevy_ecs::component::Component + Clone>(app: &mut App, id: SimId) -> Option<T> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &T)>();
    query
        .iter(world)
        .find(|(found, _)| **found == id)
        .map(|(_, component)| component.clone())
}

pub fn hit_points(app: &mut App, id: SimId) -> i32 {
    component::<Health>(app, id).expect("health").hit_points
}

/// Sets the hit points of the unit or building `id` (scenario setup).
pub fn set_hit_points(app: &mut App, id: SimId, hit_points: i32) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut Health)>();
    let (_, mut health) = query
        .iter_mut(world)
        .find(|(found, _)| **found == id)
        .expect("health");
    health.hit_points = hit_points;
}

pub fn harvester(app: &mut App, id: SimId) -> Harvester {
    component::<Harvester>(app, id).expect("a harvester")
}

pub fn experience(app: &mut App, id: SimId) -> Experience {
    component::<Experience>(app, id).unwrap_or_default()
}

pub fn cell_of(app: &mut App, id: SimId) -> CellPos {
    component::<Mobile>(app, id).expect("a unit").cell
}

/// The units of `owner`: id and type id, in id order.
pub fn units_of(app: &mut App, owner: PlayerId) -> Vec<(SimId, String)> {
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Owner, &Unit)>();
    let mut units: Vec<(SimId, String)> = query
        .iter(world)
        .filter(|(_, of, _)| of.0 == owner)
        .map(|(id, _, unit)| {
            (
                *id,
                definitions.unit(unit.kind).expect("a unit type").id.clone(),
            )
        })
        .collect();
    units.sort();
    units
}

/// Invariant I6' on the state after a tick: the simulation's own checks find nothing.
pub fn assert_cells(app: &mut App, context: &str) {
    let problems = sim::cell_invariant_violations(app.world_mut());
    assert!(problems.is_empty(), "{context}: {problems:?}");
    let stray = sim::stray_claims(app.world_mut());
    assert!(stray.is_empty(), "{context}: {stray:?}");
}

/// The fixture's definitions with its skirmish and AI files.
pub struct Data {
    pub definitions: Definitions,
    pub skirmish: sim::SkirmishDefinitions,
    pub ai: sim::AiDefinitions,
}

/// The fixture with the edits `rules` to its rules file.
pub fn data_with(name: &str, rules: &[(&str, &str)]) -> Data {
    let definitions = definitions_with(name, rules);
    let skirmish = sim::load_skirmish(fixture("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai =
        sim::load_ai(fixture("ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    sim::check_ai_factions(fixture("ai.ron"), &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

pub fn data() -> Data {
    data_with("fixture", &[])
}

/// The factions of the fixture's skirmish file: two Allied countries and a Soviet one.
pub const ALPHA: usize = 0;
pub const GAMMA: usize = 2;

pub fn human(player: PlayerId, faction: usize, start: usize) -> sim::PlayerSetup {
    sim::PlayerSetup {
        player,
        faction,
        start,
        controller: sim::Controller::Human,
    }
}

pub fn computer(
    player: PlayerId,
    faction: usize,
    start: usize,
    difficulty: usize,
) -> sim::PlayerSetup {
    sim::PlayerSetup {
        player,
        faction,
        start,
        controller: sim::Controller::Ai { difficulty },
    }
}

/// The fixture map `maps/skirmish_40x40.tmx`: starts at (5, 5) and (32, 32), each with a field
/// of ore of 100 a cell.
pub fn fixture_map() -> Grid {
    let map =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps/skirmish_40x40.tmx");
    sim::load_map(map)
        .unwrap_or_else(|error| panic!("{error}"))
        .grid
}

/// A match on `grid` (a variant of the fixture map) with `players` and `options`, set up
/// through `setup_match_configured`, before its first tick.
pub fn match_on(
    data: &Data,
    grid: Grid,
    players: &[sim::PlayerSetup],
    options: &sim::MatchOptions,
    seed: u64,
) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    sim::setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        players,
        options,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    app
}

/// Puts a crate into the cell (`x`, `y`), which must take one (scenario setup).
pub fn crate_at(app: &mut App, x: i32, y: i32) {
    let grid = app.world().resource::<Grid>().clone();
    assert!(
        app.world_mut()
            .resource_mut::<sim::Crates>()
            .place(&grid, CellPos::new(x, y)),
        "a crate at ({x}, {y})"
    );
}

/// The cells with a crate.
pub fn crates(app: &App) -> Vec<CellPos> {
    app.world().resource::<sim::Crates>().cells().collect()
}

/// What the crates taken among `events` held: collector, content, credits and new unit.
pub fn taken(events: &[SimEvent]) -> Vec<(SimId, sim::CrateContent, i64, Option<SimId>)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrateTaken {
                unit,
                content,
                credits,
                spawned,
                ..
            } => Some((*unit, *content, *credits, *spawned)),
            _ => None,
        })
        .collect()
}
