//! Ten players on maps that grow with them (docs/DECISIONS.md, "Ten players on maps that grow
//! with them"), with the shipped data on every shipped map.
//!
//! `sim::enlarged_map` has its own tests on small made-up maps (`crates/sim/src/map_scale.rs`).
//! These tests check what the game relies on for the maps it ships: for every map and every
//! player count from 2 to 10 the enlarged map has the announced size, keeps every land route and
//! every stretch of water, stays point-symmetric where the file is and the made starts come in
//! pairs, gives every made start position clear building ground and ore, and sets up as a match
//! with every yard at its start. Then computer players really play on made start positions: all
//! ten build a base and bring in ore on a doubled map, and a ten-player match on the smallest map
//! is played to its end.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use bevy::app::App;
use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use sim::{
    AiDefinitions, AiPlayers, Building, CellPos, Controller, Definitions, EnlargedMap, Grid,
    MatchOutcome, MatchState, Mobile, Owner, PlayerId, PlayerSetup, Players, SimEvent, SimPlugin,
    SimSchedule, SkirmishDefinitions, axis_map, enlarged_map, enlarged_size, load_ai, load_map,
    load_ruleset, load_skirmish, map_asset_path, setup_match, state_hash,
};

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let dir = assets().join("data");
    let definitions = load_ruleset(&dir).unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(dir.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(dir.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

/// The checked map file of map `map`: its terrain and looks.
fn shipped(data: &Data, map: usize) -> (Grid, Vec<u8>) {
    let loaded = load_map(assets().join(map_asset_path(&data.skirmish.maps[map].id)))
        .unwrap_or_else(|error| panic!("{error}"));
    (loaded.grid, loaded.looks)
}

/// Map `map` enlarged for `players` players.
fn grown(data: &Data, map: usize, players: usize) -> EnlargedMap {
    let (grid, looks) = shipped(data, map);
    let def = &data.skirmish.maps[map];
    enlarged_map(&grid, &looks, &def.starts, players)
        .unwrap_or_else(|error| panic!("{}: {players} players: {error}", def.id))
}

fn cells(grid: &Grid) -> impl Iterator<Item = CellPos> + use<> {
    let (width, height) = (grid.width() as i32, grid.height() as i32);
    (0..height).flat_map(move |y| (0..width).map(move |x| CellPos::new(x, y)))
}

/// Whether each cell (row-major) can be reached from `from` over cells that `open` admits, by
/// four-neighbour steps: the cells the pathfinder connects (docs/DECISIONS.md D45).
fn reached(grid: &Grid, from: CellPos, open: impl Fn(&sim::Cell) -> bool) -> Vec<bool> {
    let width = grid.width() as i32;
    let index = |cell: CellPos| (cell.y * width + cell.x) as usize;
    let admits = |cell: CellPos| grid.cell(cell).is_some_and(&open);
    let mut seen = vec![false; (grid.width() * grid.height()) as usize];
    let mut queue = VecDeque::new();
    if admits(from) {
        seen[index(from)] = true;
        queue.push_back(from);
    }
    while let Some(cell) = queue.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = CellPos::new(cell.x + dx, cell.y + dy);
            if admits(next) && !std::mem::replace(&mut seen[index(next)], true) {
                queue.push_back(next);
            }
        }
    }
    seen
}

fn point_symmetric(grid: &Grid, looks: &[u8]) -> bool {
    let (width, height) = (grid.width() as i32, grid.height() as i32);
    let look = |cell: CellPos| looks[(cell.y * width + cell.x) as usize];
    cells(grid).all(|cell| {
        let mirror = CellPos::new(width - 1 - cell.x, height - 1 - cell.y);
        grid.cell(cell) == grid.cell(mirror) && look(cell) == look(mirror)
    })
}

/// The centre of the 3 x 3 yard at `start`.
fn yard_centre(start: CellPos) -> CellPos {
    CellPos::new(
        start.x + sim::YARD_CENTRE_OFFSET,
        start.y + sim::YARD_CENTRE_OFFSET,
    )
}

fn distance_squared(a: CellPos, b: CellPos) -> i64 {
    let (dx, dy) = (i64::from(a.x - b.x), i64::from(a.y - b.y));
    dx * dx + dy * dy
}

/// All computer players, one per start position, at `difficulty`: their sides alternating
/// (the first of them playing side `shift`), each side played by its countries in turn (RA1,
/// docs/DECISIONS.md R5; before, the two factions alternated).
fn computers_from(data: &Data, count: usize, difficulty: usize, shift: usize) -> Vec<PlayerSetup> {
    let groups = data.skirmish.pairing_groups();
    (0..count)
        .map(|k| PlayerSetup {
            player: PlayerId(k as u8),
            faction: sim::SkirmishDefinitions::rotating(
                &groups[(k + shift) % groups.len()],
                k / groups.len(),
            )
            .expect("a country"),
            start: k,
            controller: Controller::Ai { difficulty },
        })
        .collect()
}

fn computers(data: &Data, count: usize, difficulty: usize) -> Vec<PlayerSetup> {
    computers_from(data, count, difficulty, 0)
}

/// How many starting units a player of `setup`'s country has with the lobby's default unit
/// count (docs/RA2_PARITY_PLAN.md section 4.9), besides its base unit.
fn starting_units(data: &Data, setup: &PlayerSetup) -> usize {
    let side = data.skirmish.factions[setup.faction]
        .side
        .expect("a country");
    let rules = data.definitions.rules().expect("rules");
    sim::starting_units(
        &data.skirmish,
        &data.definitions,
        side,
        u32::try_from(rules.dialog.unit_count).expect("unit count"),
        &mut sim::SimRng::new(0),
    )
    .len()
}

/// A match of `players` on map `map` enlarged for them, set up as the game sets it up
/// (`crate::skirmish::match_plan`): the enlarged terrain, and the skirmish file with the enlarged
/// start positions.
fn start(data: &Data, map: usize, players: &[PlayerSetup], seed: u64) -> (App, EnlargedMap) {
    let enlarged = grown(data, map, players.len());
    let mut skirmish = data.skirmish.clone();
    skirmish.maps[map].starts = enlarged.starts.clone();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    app.world_mut().insert_resource(enlarged.grid.clone());
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match(app.world_mut(), &skirmish, &data.ai, map, players).unwrap_or_else(|error| {
        panic!(
            "{}: {} players: {error}",
            data.skirmish.maps[map].id,
            players.len()
        )
    });
    (app, enlarged)
}

#[test]
fn every_shipped_map_grows_with_every_player_count() {
    let data = data();
    let mut made = 0;
    for (map, def) in data.skirmish.maps.iter().enumerate() {
        let (grid, looks) = shipped(&data, map);
        let (width, height) = (grid.width(), grid.height());
        let symmetric_file = point_symmetric(&grid, &looks);
        // Two players: the map file itself, cell for cell.
        let two = grown(&data, map, 2);
        assert_eq!(two.grid, grid, "{}", def.id);
        assert_eq!(two.looks, looks, "{}", def.id);
        assert_eq!(two.starts, def.starts, "{}", def.id);
        for players in 2..=sim::MAX_MATCH_PLAYERS {
            let name = format!("{}: {players} players", def.id);
            let enlarged = grown(&data, map, players);
            // Deterministic: the same map twice.
            assert_eq!(enlarged, grown(&data, map, players), "{name}");
            let size = enlarged_size(width, height, players);
            assert_eq!(
                (enlarged.grid.width(), enlarged.grid.height()),
                size,
                "{name}"
            );
            assert_eq!(
                enlarged.looks.len(),
                (size.0 * size.1) as usize,
                "{name}: one look per cell"
            );
            // Each player beyond the second adds an eighth (rounded up to whole cells on both
            // sides of the middle), and the map keeps its proportions. A map that would pass
            // the largest size stops at the last eighth that fits: the largest map files are
            // played as they are.
            let fits = |eighths: u32| {
                [(width, size.0), (height, size.1)]
                    .iter()
                    .all(|&(side, grown)| {
                        grown * 8 >= side * eighths && grown * 8 < side * eighths + 16
                    })
            };
            let eighths = (8..=players as u32 + 6)
                .rev()
                .find(|&eighths| fits(eighths))
                .unwrap_or_else(|| panic!("{name}: {width} x {height} -> {size:?}"));
            if eighths < players as u32 + 6 {
                // The next eighth would not fit on one side or the other.
                let next = eighths + 1;
                assert!(
                    width.max(height) * next > sim::MAX_MAP_SIDE * 8,
                    "{name}: {width} x {height} -> {size:?}"
                );
            }
            for (side, grown) in [(width, size.0), (height, size.1)] {
                assert_eq!((grown - side) % 2, 0, "{name}");
                assert!(grown <= sim::MAX_MAP_SIDE, "{name}");
            }

            // The terrain is the file's, column by column and row by row; only made start
            // positions differ, and only by cells that became passable land or ore.
            let (columns, rows) = (axis_map(width, size.0), axis_map(height, size.1));
            let mut changed = BTreeSet::new();
            for cell in cells(&enlarged.grid) {
                let from = CellPos::new(
                    columns[cell.x as usize] as i32,
                    rows[cell.y as usize] as i32,
                );
                let (before, after) = (grid.cell(from).unwrap(), enlarged.grid.cell(cell).unwrap());
                let look_before = looks[(from.y * width as i32 + from.x) as usize];
                let look_after = enlarged.looks[(cell.y * size.0 as i32 + cell.x) as usize];
                if (before, look_before) == (after, look_after) {
                    continue;
                }
                changed.insert(cell);
                assert!(after.passable && !after.water, "{name}: {cell:?} {after:?}");
                assert_eq!(
                    before.water, after.water,
                    "{name}: {cell:?}: no water is filled in"
                );
                assert!(
                    (after.buildable && after.ore == 0) || (!after.buildable && after.ore > 0),
                    "{name}: {cell:?} {after:?}: open building ground or ore"
                );
                // The ground look stays; the feature goes, or is ore.
                assert_eq!(look_after & 15, look_before & 15, "{name}: {cell:?}");
                assert_eq!(look_after >> 4, u8::from(after.ore > 0), "{name}: {cell:?}");
            }

            // The file's starts first, moved with their terrain; then the made ones.
            let authored = def.starts.len();
            assert_eq!(enlarged.starts.len(), authored.max(players), "{name}");
            let centres: Vec<CellPos> = enlarged.starts.iter().map(|&s| yard_centre(s)).collect();
            for (k, &start) in def.starts.iter().enumerate() {
                let centre = centres[k];
                assert_eq!(
                    (columns[centre.x as usize], rows[centre.y as usize]),
                    (
                        (start.x + sim::YARD_CENTRE_OFFSET) as u32,
                        (start.y + sim::YARD_CENTRE_OFFSET) as u32
                    ),
                    "{name}: start {k} stays on its ground"
                );
            }
            if players <= authored {
                assert!(changed.is_empty(), "{name}: nothing is made");
                assert_eq!(
                    point_symmetric(&enlarged.grid, &enlarged.looks),
                    symmetric_file,
                    "{name}"
                );
            }
            // Mirrored starts of the file stay mirrored (pairs 0 and 1, 2 and 3, ...).
            if symmetric_file {
                for pair in enlarged.starts[..authored].chunks(2) {
                    assert_eq!(
                        (pair[0].x + pair[1].x, pair[0].y + pair[1].y),
                        (size.0 as i32 - 3, size.1 as i32 - 3),
                        "{name}"
                    );
                }
            }

            // Every start is a land walk away from every other.
            let land = reached(&enlarged.grid, enlarged.starts[0], |cell| cell.passable);
            for (k, start) in enlarged.starts.iter().enumerate() {
                assert!(
                    land[(start.y * size.0 as i32 + start.x) as usize],
                    "{name}: start {k} at {start:?} is cut off"
                );
            }
            // Every cell of land or water that was reachable in the file still is: the enlarged
            // map has as many separate pieces of land, and of sea, as the file.
            for open in [
                (|cell: &sim::Cell| cell.passable) as fn(&sim::Cell) -> bool,
                |cell: &sim::Cell| cell.water,
            ] {
                let pieces = |grid: &Grid| {
                    let mut seen = vec![false; (grid.width() * grid.height()) as usize];
                    let mut pieces = 0;
                    for cell in cells(grid) {
                        let index = (cell.y * grid.width() as i32 + cell.x) as usize;
                        if seen[index] || !grid.cell(cell).is_some_and(open) {
                            continue;
                        }
                        pieces += 1;
                        for (index, reached) in reached(grid, cell, open).into_iter().enumerate() {
                            seen[index] |= reached;
                        }
                    }
                    pieces
                };
                assert!(pieces(&enlarged.grid) <= pieces(&grid), "{name}");
            }

            // Made start positions: far from every other start and from the edge, on clear
            // building ground, with ore a harvester can walk to.
            let least = i64::from(sim::MIN_START_DISTANCE).pow(2);
            for k in authored..players.max(authored) {
                made += 1;
                let centre = centres[k];
                for (other, &centre_other) in centres.iter().enumerate() {
                    assert!(
                        other == k || distance_squared(centre, centre_other) >= least,
                        "{name}: starts {k} and {other} at {centre:?} and {centre_other:?}"
                    );
                }
                let margin = sim::EDGE_MARGIN;
                assert!(
                    (margin..size.0 as i32 - margin).contains(&centre.x)
                        && (margin..size.1 as i32 - margin).contains(&centre.y),
                    "{name}: start {k} at {centre:?}"
                );
                let core = sim::CORE_RADIUS;
                for dy in -core..=core {
                    for dx in -core..=core {
                        let cell = CellPos::new(centre.x + dx, centre.y + dy);
                        let terrain = enlarged.grid.cell(cell).unwrap();
                        assert!(
                            terrain.passable && terrain.buildable && terrain.ore == 0,
                            "{name}: start {k}: {cell:?} {terrain:?}"
                        );
                    }
                }
                let reach = sim::ORE_DISTANCE + sim::ORE_RADIUS + 2;
                let ore = cells(&enlarged.grid)
                    .filter(|cell| {
                        (cell.x - centre.x).abs() <= reach
                            && (cell.y - centre.y).abs() <= reach
                            && enlarged.grid.cell(*cell).is_some_and(|c| c.ore > 0)
                            && land[(cell.y * size.0 as i32 + cell.x) as usize]
                    })
                    .count();
                assert!(
                    ore >= sim::MIN_ORE_CELLS,
                    "{name}: start {k}: {ore} ore cells"
                );
                // No ore on any yard's apron.
                for &any in &centres {
                    let apron = sim::APRON_RADIUS;
                    for cell in &changed {
                        let on_apron =
                            (cell.x - any.x).abs() <= apron && (cell.y - any.y).abs() <= apron;
                        assert!(
                            !on_apron || enlarged.grid.cell(*cell).unwrap().ore == 0,
                            "{name}: ore on the apron of {any:?} at {cell:?}"
                        );
                    }
                }
            }
            // A symmetric file with its missing starts made in pairs is a symmetric map.
            if symmetric_file && (players.saturating_sub(authored)).is_multiple_of(2) {
                assert!(
                    point_symmetric(&enlarged.grid, &enlarged.looks),
                    "{name}: point symmetry"
                );
                for pair in enlarged.starts.chunks(2) {
                    assert_eq!(
                        (pair[0].x + pair[1].x, pair[0].y + pair[1].y),
                        (size.0 as i32 - 3, size.1 as i32 - 3),
                        "{name}: {pair:?}"
                    );
                }
            }

            // The match sets up with every base unit at its start, for either order of the
            // sides, and every computer player deploys it there in its first turns (RA1; before,
            // the match was set up with the yards): every yard stands at its start.
            for shift in 0..data.skirmish.pairing_groups().len() {
                let players = computers_from(&data, players, 1, shift);
                let (mut app, enlarged) = start(&data, map, &players, 0);
                let world = app.world_mut();
                let bases: BTreeMap<PlayerId, CellPos> = world
                    .query::<(&Owner, &sim::Unit, &Mobile)>()
                    .iter(world)
                    .filter(|(_, unit, _)| {
                        data.definitions
                            .rules()
                            .is_some_and(|rules| rules.is_base_unit(unit.kind))
                    })
                    .map(|(owner, _, mobile)| (owner.0, mobile.cell))
                    .collect();
                for (k, setup) in players.iter().enumerate() {
                    assert_eq!(
                        bases.get(&setup.player),
                        Some(&enlarged.starts[k]),
                        "{name}"
                    );
                }
                let mut yards: BTreeMap<PlayerId, CellPos> = BTreeMap::new();
                for _ in 0..100 {
                    world.run_schedule(SimSchedule);
                    yards = world
                        .query::<(&Owner, &Building)>()
                        .iter(world)
                        .filter(|(_, building)| {
                            data.definitions.rules().is_some_and(|rules| {
                                rules.side_of_construction_yard(building.kind).is_some()
                            })
                        })
                        .map(|(owner, building)| (owner.0, building.footprint.origin))
                        .collect();
                    if yards.len() == players.len() {
                        break;
                    }
                }
                for (k, setup) in players.iter().enumerate() {
                    assert_eq!(
                        yards.get(&setup.player),
                        Some(&enlarged.starts[k]),
                        "{name}"
                    );
                }
                let expected: usize = players
                    .iter()
                    .map(|setup| starting_units(&data, setup))
                    .sum();
                assert_eq!(
                    world.query::<&Mobile>().iter(world).count(),
                    expected,
                    "{name}"
                );
            }
        }
    }
    assert!(made > 0, "some matches have made start positions");
}

fn map_index(data: &Data, id: &str) -> usize {
    data.skirmish
        .maps
        .iter()
        .position(|map| map.id == id)
        .unwrap_or_else(|| panic!("map {id}"))
}

/// What the players of a match did while it ran.
#[derive(Default)]
struct Course {
    /// Buildings each player placed.
    placed: BTreeMap<PlayerId, usize>,
    /// Credits each player's harvesters brought in.
    earned: BTreeMap<PlayerId, i64>,
    /// The tick each defeated player was defeated in.
    defeated: BTreeMap<PlayerId, u64>,
    outcome: Option<MatchOutcome>,
    ticks: u64,
}

/// Runs the match for up to `limit` ticks, or until it ends.
fn run(world: &mut World, limit: u64) -> Course {
    let mut course = Course::default();
    for tick in 0..limit {
        world.run_schedule(SimSchedule);
        course.ticks = tick + 1;
        for event in world
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
        {
            match event {
                SimEvent::BuildingPlaced { player, .. } => {
                    *course.placed.entry(*player).or_default() += 1;
                }
                SimEvent::OreDelivered {
                    player, credits, ..
                } => *course.earned.entry(*player).or_default() += credits,
                SimEvent::PlayerDefeated { player } => {
                    course.defeated.insert(*player, tick);
                }
                _ => {}
            }
        }
        course.outcome = world.resource::<MatchState>().outcome();
        if course.outcome.is_some() {
            break;
        }
    }
    course
}

/// Where every player's armed units and buildings are, for a match that did not end.
fn report(world: &mut World) -> String {
    let mut lines = Vec::new();
    let players: Vec<(PlayerId, i64)> = world
        .resource::<Players>()
        .iter()
        .map(|(id, state)| (id, state.credits()))
        .collect();
    for (id, credits) in players {
        let buildings: Vec<CellPos> = world
            .query::<(&Owner, &Building)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == id)
            .map(|(_, building)| building.footprint.origin)
            .collect();
        let units: Vec<CellPos> = world
            .query::<(&Owner, &Mobile)>()
            .iter(world)
            .filter(|(owner, _)| owner.0 == id)
            .map(|(_, mobile)| mobile.cell)
            .collect();
        let ai = world.resource::<AiPlayers>();
        let wave = ai.controller(id).map(|c| (c.wave().len(), c.target()));
        lines.push(format!(
            "{id:?}: {credits} credits, {} buildings {buildings:?}, {} units {units:?}, wave {wave:?}",
            buildings.len(),
            units.len()
        ));
    }
    lines.join("\n")
}

#[test]
fn ten_computer_players_build_bases_and_bring_in_ore_on_enlarged_maps() {
    // Five minutes of game time on a map of the largest size with four made start positions
    // (its file lists six), and on the map with the fewest authored ones (four, so six are
    // made): every player, at a made start or not, places buildings and brings in ore, so the
    // made bases work.
    let data = data();
    for id in ["iron_isthmus", "riverlands"] {
        let map = map_index(&data, id);
        let players = computers(&data, sim::MAX_MATCH_PLAYERS, 1);
        let (mut app, enlarged) = start(&data, map, &players, 3);
        let authored = data.skirmish.maps[map].starts.len();
        assert!(authored < players.len(), "{id}: some starts are made");
        let world = app.world_mut();
        assert_eq!(world.resource::<AiPlayers>().players().count(), 10);
        let before = state_hash(world);
        let course = run(world, 6_000);
        for (k, setup) in players.iter().enumerate() {
            let made = if k >= authored { "made" } else { "authored" };
            let name = format!(
                "{id}: player {k} at the {made} start {:?}",
                enlarged.starts[k]
            );
            let placed = course.placed.get(&setup.player).copied().unwrap_or(0);
            let earned = course.earned.get(&setup.player).copied().unwrap_or(0);
            // The yard its base unit deployed into (RA1), a power plant and a refinery at least,
            // and ore from the field beside the base.
            assert!(
                placed >= 3,
                "{name}: {placed} buildings placed\n{}",
                report(world)
            );
            assert!(
                earned > 0,
                "{name}: {earned} credits earned\n{}",
                report(world)
            );
        }
        assert_ne!(state_hash(world), before);
        // The same match again: the same course, to the same state.
        let (mut again, _) = start(&data, map, &players, 3);
        run(again.world_mut(), 6_000);
        assert_eq!(state_hash(again.world_mut()), state_hash(world), "{id}");
    }
}

#[test]
fn a_ten_player_match_on_the_smallest_map_is_played_to_its_end() {
    // Crossroads, 56 x 56 cells in its file and 112 x 112 for ten players, four of them at made
    // start positions: all against all until one is left.
    let data = data();
    let map = map_index(&data, "crossroads");
    let players = computers(&data, sim::MAX_MATCH_PLAYERS, 1);
    let (mut app, _) = start(&data, map, &players, 7);
    let world = app.world_mut();
    let course = run(world, MATCH_TICK_LIMIT);
    assert!(
        matches!(course.outcome, Some(MatchOutcome::Won { .. })),
        "{:?} after {} ticks, defeated {:?}\n{}",
        course.outcome,
        course.ticks,
        course.defeated,
        report(world)
    );
    assert_eq!(course.defeated.len(), 9, "{:?}", course.defeated);
    // Every player at a made start position built a base before it fell or won.
    // (the yard its base unit deployed into, and two buildings more, as before RA1)
    for setup in &players[data.skirmish.maps[map].starts.len()..] {
        assert!(
            course.placed.get(&setup.player).copied().unwrap_or(0) >= 3,
            "{:?}: {:?}",
            setup.player,
            course.placed
        );
    }
}

/// Ticks within which the ten-player match on the smallest map must end: an hour of game time,
/// the limit of the free-for-all matches of `m10_unlocks_and_enemies.rs`. It ends in tick 15,577.
const MATCH_TICK_LIMIT: u64 = 72_000;
