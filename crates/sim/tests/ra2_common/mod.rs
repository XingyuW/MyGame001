//! The scenario helpers the RA2 tests share (`ra2_*.rs`): a small open world on the RA2 fixture
//! (`tests/fixtures/data/ra2`: the two sided world of the RA1 fixture with the traits and rules
//! of RA2, in values that differ from the shipped ones; invariant I10), driven through
//! `CommandQueue` and `SimSchedule`.
#![allow(dead_code)]

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Definitions, Grid, Health,
    IssuedCommand, Lane, Mobile, Owner, PlayerId, Players, ProductionItem, QueueItem, QueueKey,
    RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, Unit, load_match_definitions,
    spawn_building, spawn_unit,
};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

/// No rejection.
pub const NONE: [RejectReason; 0] = [];

pub fn fixture(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/ra2")
        .join(file)
}

/// The definitions of the RA2 fixture: units, economy, combat, rules and arsenal.
pub fn definitions() -> Definitions {
    load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(fixture("rules.ron")))
    .and_then(|definitions| definitions.with_arsenal(fixture("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"))
}

/// A world of 40 x 40 open cells with the players P0 (country alpha, Allied) and P1 (gamma,
/// Soviet), who are enemies, each with `credits` credits.
pub fn world(credits: i64) -> App {
    let definitions = definitions();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 7 });
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
        players.add(player, credits).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

pub fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

pub fn kind(app: &App, id: &str) -> BuildingKind {
    defs(app)
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

pub fn building(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Building(kind(app, id))
}

pub fn unit(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Unit(
        defs(app)
            .unit_kind(id)
            .unwrap_or_else(|| panic!("unit {id}")),
    )
}

/// A building of type `id` of `owner` with its footprint's origin at (`x`, `y`), as scenario
/// setup (no placement distance, no free units).
pub fn place(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// A unit of type `id` of `owner` in the cell (`x`, `y`), as scenario setup.
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

/// Runs `count` ticks without commands and returns all their events.
pub fn ticks(app: &mut App, count: u32) -> Vec<SimEvent> {
    (0..count).flat_map(|_| tick(app, Vec::new())).collect()
}

pub fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

pub fn queue(player: PlayerId, item: ProductionItem) -> IssuedCommand {
    issue(player, Command::QueueProduction { item })
}

pub fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

pub fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .credits()
}

/// The queue of `player` in which `item` is produced.
pub fn lane_of(app: &App, player: PlayerId, item: ProductionItem) -> Vec<QueueItem> {
    let key = defs(app).queue_of(item).expect("an item with a queue");
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .lane(key)
        .to_vec()
}

/// The queue key of the yards of the fixture in `lane`.
pub fn yard_lane(app: &App, lane: Lane) -> QueueKey {
    QueueKey {
        producer: defs(app).queue_key(kind(app, "allied_yard")),
        lane,
    }
}

pub fn entity(app: &mut App, id: SimId) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &SimId)>();
    query
        .iter(app.world())
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no entity {}", id.0))
}

pub fn exists(app: &mut App, id: SimId) -> bool {
    let mut query = app.world_mut().query::<&SimId>();
    query.iter(app.world()).any(|other| *other == id)
}

pub fn hit_points(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id);
    app.world()
        .get::<Health>(entity)
        .expect("health")
        .hit_points
}

pub fn set_hit_points(app: &mut App, id: SimId, value: i32) {
    let entity = entity(app, id);
    app.world_mut()
        .get_mut::<Health>(entity)
        .expect("health")
        .hit_points = value;
}

pub fn cell_of(app: &mut App, id: SimId) -> CellPos {
    let entity = entity(app, id);
    app.world().get::<Mobile>(entity).expect("a unit").cell
}

/// The types of the buildings of `player`, in id order.
pub fn owned_kinds(app: &mut App, player: PlayerId) -> Vec<BuildingKind> {
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
    let mut found: Vec<(SimId, BuildingKind)> = query
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == player)
        .map(|(id, _, building)| (*id, building.kind))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, kind)| kind).collect()
}

/// The units of type `id` of `player`.
pub fn units_of(app: &mut App, player: PlayerId, id: &str) -> usize {
    let kind = defs(app).unit_kind(id).expect("unit");
    let mut query = app.world_mut().query::<(&Owner, &Unit)>();
    query
        .iter(app.world())
        .filter(|(owner, unit)| owner.0 == player && unit.kind == kind)
        .count()
}
