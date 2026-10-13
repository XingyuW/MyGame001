//! The scenario helpers the RA4 tests share (`ra4_*.rs`): a small open world on the RA4 fixture
//! (`tests/fixtures/data/ra4`: the world of the RA3 fixture with trainable, crewed, docked,
//! spawning and cloakable types, an arsenal that uses every trait of its schema version 5;
//! invariant I10), driven through `CommandQueue` and `SimSchedule`. The helpers are those of
//! `ra3_common`, on this fixture.
#![allow(dead_code)]

use std::path::PathBuf;

use std::collections::BTreeMap;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Domain, Grid, Health, IssuedCommand, Mobile,
    PlayerId, Players, Position, SPOT_OFFSETS, SPOTS_PER_CELL, SimEvent, SimId, SimPlugin,
    SimSchedule, SubPos, Unit, Weapon, WeaponKind, load_match_definitions, spawn_building,
    spawn_unit,
};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);

pub fn fixture(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/ra4")
        .join(file)
}

/// The definitions of the RA4 fixture, its combat file with each `(from, to)` edit applied
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
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra4");
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

pub fn move_to(player: PlayerId, units: &[SimId], x: i32, y: i32) -> IssuedCommand {
    issue(
        player,
        Command::Move {
            units: units.to_vec(),
            target: CellPos::new(x, y),
        },
    )
}

/// Invariant I6' (docs/DECISIONS.md R6) on the state after a tick: the simulation's own check
/// finds nothing ([`sim::cell_invariant_violations`], [`sim::stray_claims`]), and, worked out
/// here from the units' components alone and from the grid:
///
/// * no surface cell is held by two vehicles, by a vehicle and infantry, or by more than three
///   infantry, and no cell of the air layer by two aircraft (a unit holds the cell it stands in
///   and, while it steps, the cell it enters);
/// * the infantry in a cell hold different spots, and the grid has each unit in every cell it
///   holds;
/// * no unit of the surface holds a cell covered by a building or enters an impassable one;
/// * a unit that stands is at the centre of its cell, a unit on foot at its spot; a unit in a
///   step is at the step's share of the straight way from there to the centre, or to its spot,
///   in the next cell;
/// * no two units that stand are at the same point.
pub fn assert_cells(app: &mut App, context: &str) {
    let problems = sim::cell_invariant_violations(app.world_mut());
    assert!(problems.is_empty(), "{context}: {problems:?}");
    let stray = sim::stray_claims(app.world_mut());
    assert!(stray.is_empty(), "{context}: {stray:?}");
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Unit, &Mobile, &Position)>();
    let units: Vec<(SimId, Unit, Mobile, SubPos)> = query
        .iter(world)
        .map(|(id, unit, mobile, position)| (*id, *unit, *mobile, position.0))
        .collect();
    let grid = world.resource::<Grid>();
    // Per layer and cell: the vehicles (or aircraft) and the infantry on foot with their spots.
    type Holders = (Vec<SimId>, Vec<(SimId, u8)>);
    let mut held: BTreeMap<(bool, CellPos), Holders> = BTreeMap::new();
    let mut standing: BTreeMap<(bool, SubPos), SimId> = BTreeMap::new();
    for &(id, unit, mobile, position) in &units {
        let domain = definitions.domain(unit.kind);
        let air = domain == Domain::Air;
        let on_foot = definitions.on_foot(unit.kind);
        let mut points = Vec::new();
        for cell in std::iter::once(mobile.cell).chain(mobile.step.map(|step| step.to)) {
            assert!(
                grid.holders_for(cell, domain).contains(&Some(id)),
                "{context}: the grid does not have {id:?} in {cell:?}"
            );
            let entry = held.entry((air, cell)).or_default();
            if on_foot {
                let spot = grid
                    .spot(cell, id)
                    .unwrap_or_else(|| panic!("{context}: {id:?} has no spot in {cell:?}"));
                entry.1.push((id, spot));
                let (dx, dy) = SPOT_OFFSETS[usize::from(spot)];
                let centre = SubPos::centre_of(cell);
                points.push(SubPos::new(centre.x + dx, centre.y + dy));
            } else {
                entry.0.push(id);
                points.push(SubPos::centre_of(cell));
            }
            if !air {
                assert_eq!(
                    grid.building(cell),
                    None,
                    "{context}: {id:?} holds {cell:?} under a building"
                );
            }
        }
        let expected = match mobile.step {
            None => points[0],
            Some(step) => {
                assert!(
                    grid.passable_for(step.to, domain),
                    "{context}: {id:?} enters an impassable cell"
                );
                let (from, to) = (points[0], points[1]);
                let (progress, length) = (i32::from(step.progress), i32::from(step.length));
                assert!(
                    progress <= length && length > 0,
                    "{context}: {id:?} {step:?}"
                );
                if on_foot {
                    SubPos::new(
                        from.x + (to.x - from.x) * progress / length,
                        from.y + (to.y - from.y) * progress / length,
                    )
                } else {
                    // Between centres: 256 subcells along each axis that changes.
                    let offset = 256 * progress / length;
                    SubPos::new(
                        from.x + (to.x - from.x).signum() * offset,
                        from.y + (to.y - from.y).signum() * offset,
                    )
                }
            }
        };
        assert_eq!(position, expected, "{context}: {id:?} with {mobile:?}");
        if mobile.step.is_none()
            && let Some(other) = standing.insert((air, position), id)
        {
            panic!("{context}: {other:?} and {id:?} both stand at {position:?}");
        }
    }
    for ((air, cell), (whole, foot)) in &held {
        assert!(
            whole.len() <= 1,
            "{context}: {cell:?} (air: {air}) is held by {whole:?}"
        );
        assert!(
            foot.len() <= SPOTS_PER_CELL,
            "{context}: {cell:?} is held by the infantry {foot:?}"
        );
        assert!(
            whole.is_empty() || foot.is_empty(),
            "{context}: {cell:?} is held by {whole:?} and the infantry {foot:?}"
        );
        assert!(
            !air || foot.is_empty(),
            "{context}: infantry on foot in the air"
        );
        let mut spots: Vec<u8> = foot.iter().map(|&(_, spot)| spot).collect();
        spots.sort_unstable();
        spots.dedup();
        assert_eq!(
            spots.len(),
            foot.len(),
            "{context}: {cell:?}: two soldiers in one spot, {foot:?}"
        );
    }
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

// --- RA4 ------------------------------------------------------------------------------------------

/// The experience of the unit `id`, or `None` for a unit without any (untrainable, or gone).
pub fn experience(app: &mut App, id: SimId) -> Option<sim::Experience> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Experience)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, experience)| *experience)
}

/// The rank of the unit `id` (0 without experience).
pub fn rank(app: &mut App, id: SimId) -> u8 {
    experience(app, id).map_or(0, |experience| experience.rank)
}

/// Whether the unit or building `id` exists.
pub fn alive(app: &mut App, id: SimId) -> bool {
    hp(app, id).is_some()
}

/// The ids of the units of `owner` of the type `kind`, in id order.
pub fn units_of(app: &mut App, owner: PlayerId, kind: &str) -> Vec<SimId> {
    let wanted = defs(app)
        .unit_kind(kind)
        .unwrap_or_else(|| panic!("{kind}"));
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Owner, &Unit)>();
    let mut found: Vec<SimId> = query
        .iter(world)
        .filter(|(_, by, unit)| by.0 == owner && unit.kind == wanted)
        .map(|(id, ..)| *id)
        .collect();
    found.sort_unstable();
    found
}

/// The commands rejected in `events`, with their reasons.
pub fn rejections(events: &[SimEvent]) -> Vec<sim::RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

/// Runs ticks until `done` holds for the events of a tick, at most `limit`; the events of all
/// those ticks, and the number of ticks run.
pub fn run_until(
    app: &mut App,
    limit: u32,
    mut done: impl FnMut(&mut App, &[SimEvent]) -> bool,
) -> (Vec<SimEvent>, u32) {
    let mut all = Vec::new();
    for ran in 1..=limit {
        let events = tick(app, Vec::new());
        let stop = done(app, &events);
        all.extend(events);
        if stop {
            return (all, ran);
        }
    }
    panic!("the condition did not hold within {limit} ticks");
}

/// The state hash of the world.
pub fn hash(app: &mut App) -> u64 {
    sim::state_hash(app.world_mut())
}

/// The definitions of the fixture with each `(from, to)` replaced in the arsenal's line of
/// the entry `id` (`from` must occur exactly once in it).
pub fn definitions_with_arsenal(name: &str, id: &str, edits: &[(&str, &str)]) -> Definitions {
    let text = std::fs::read_to_string(fixture("arsenal.ron")).expect("the fixture");
    let marker = format!("(id: {id:?},");
    let mut found = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut line = line.to_owned();
            if line.trim_start().starts_with("(id: ") && line.contains(&marker) {
                for (from, to) in edits {
                    assert_eq!(line.matches(from).count(), 1, "{name}: {from:?} in {id}");
                    line = line.replace(from, to);
                }
                found = true;
            }
            line
        })
        .collect();
    assert!(found, "{name}: no entry {id}");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra4");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(format!("arsenal_{name}_{}.ron", std::process::id()));
    std::fs::write(&path, lines.join("\n") + "\n").expect("variant");
    load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(fixture("rules.ron")))
    .and_then(|definitions| definitions.with_arsenal(&path))
    .unwrap_or_else(|error| panic!("{error}"))
}
