//! Historical preparation note: UNVALIDATED / PENDING. Draft native test helpers; final RA8 input refresh required.
#![allow(dead_code)]
//! RA6 vehicle and strategic acceptance on shipped data through commands and the simulation schedule.
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand, Owner, PlayerId,
    Players, ProductionItem, SimEvent, SimId, SimPlugin, SimSchedule, load_ruleset, spawn_building,
    spawn_unit,
};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

pub fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

pub fn shipped() -> Definitions {
    load_ruleset(data_dir()).unwrap_or_else(|error| panic!("{error}"))
}

pub const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};

/// A world of 48 x 48 open cells with the shipped data and the players P0 (Great Britain,
/// Allied) and P1 (Russia, Soviet), who are enemies. It has no match, so no crate appears.
pub fn world() -> App {
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

pub fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

pub fn put(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    if defs(app).unit_kind(id).is_some() {
        spawn_unit(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    } else {
        spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    }
}

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

pub fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

pub fn rejected(events: &[SimEvent]) -> Vec<&SimEvent> {
    events
        .iter()
        .filter(|event| matches!(event, SimEvent::CommandRejected { .. }))
        .collect()
}

pub fn entity(app: &mut App, id: SimId) -> Option<bevy_ecs::entity::Entity> {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(entity, _)| entity)
}
pub fn owner(app: &mut App, id: SimId) -> Option<PlayerId> {
    let entity = entity(app, id)?;
    app.world().get::<Owner>(entity).map(|owner| owner.0)
}
pub fn hp(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id).expect("object exists");
    app.world()
        .get::<sim::Health>(entity)
        .expect("health")
        .hit_points
}

pub fn interact(app: &mut App, actor: SimId, target: SimId) -> Vec<SimEvent> {
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
pub fn cell(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).expect("unit");
    app.world().get::<sim::Mobile>(e).expect("mobile").cell
}
pub fn set_hp(app: &mut App, id: SimId, hp: i32) {
    let e = entity(app, id).expect("actor");
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .expect("health")
        .hit_points = hp;
}
pub fn steps(app: &mut App, count: usize) {
    for _ in 0..count {
        tick(app, vec![]);
    }
}

pub fn attack(app: &mut App, a: SimId, b: SimId) -> Vec<SimEvent> {
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
pub fn charge(app: &mut App, id: SimId) {
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
pub fn power(app: &mut App) {
    put(app, P0, "nuclear_reactor", 0, 0);
    put(app, P1, "nuclear_reactor", 43, 43);
}

pub fn cells(app: &mut App) {
    let problems = sim::cell_invariant_violations(app.world_mut());
    assert!(problems.is_empty(), "{problems:?}");
    let stray = sim::stray_claims(app.world_mut());
    assert!(stray.is_empty(), "{stray:?}");
    for (_, player) in app.world().resource::<Players>().iter() {
        assert!(player.credits() >= 0);
    }
}
pub fn cargo(app: &App, host: SimId) -> Vec<SimId> {
    app.world()
        .resource::<sim::SpecialState>()
        .cargo
        .get(&host)
        .map_or_else(Vec::new, |passengers| {
            passengers.iter().map(|p| p.id).collect()
        })
}
