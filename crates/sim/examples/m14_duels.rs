//! Exact diagnostics may set `RA2_DUEL_SEED=0` and
//! `RA2_DUEL_PAIR=libya:brutal,iraq:medium` with `full coral_archipelago trace`.
//! Without these selectors, the required full sweep remains unchanged.
//! Two computer players on the shipped maps, played to the end: how long the matches of the
//! large maps take (docs/DECISIONS.md, "Maps at the engine's largest size").
//!
//! Run from the repository root with
//!
//! ```text
//! cargo run --release -p sim --example m14_duels -- [full] [doubled] [trace] [map id ...]
//! ```
//!
//! For each named map of the skirmish file (all of them if none is named), computer players of
//! the shipped data play one against one from the first two start positions, to the end or to
//! [`END_LIMIT`] ticks, and the run reports the tick each match ended in. Without `full` four
//! pairings of sides and difficulties are played per map, with `full` every one of them (as
//! `crates/game/tests/m6_production_matches.rs` does). The country of each side rotates with the
//! map and the pairing (docs/DECISIONS.md R5), so that every country is played on several maps. `doubled` plays on the map as a match of
//! ten players enlarges it (`sim::enlarged_map`), still with two players at the first two
//! starts. `trace` prints, every 3,000 ticks, where each player's units are and what it owns,
//! which shows a stalled match for what it is.
//!
//! The matches are independent of each other and run on all cores. The run fails (exit code 1)
//! if a match does not end.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use bevy_app::App;
use sim::{
    Building, CellPos, Controller, Grid, MAX_MATCH_PLAYERS, MatchOutcome, MatchState, Mobile,
    Owner, PlayerId, PlayerSetup, SimPlugin, SimSchedule, SkirmishDefinitions, Unit, enlarged_map,
    load_ai, load_map, load_ruleset, load_skirmish, map_asset_path, setup_match,
};

/// Ticks a match may take before the run gives up on it.
const END_LIMIT: u64 = 300_000;

/// Ticks between two lines of a traced match.
const TRACE_EVERY: u64 = 3_000;

fn winning_tick(tick: u64, outcome: MatchOutcome) -> Option<u64> {
    matches!(outcome, MatchOutcome::Won { .. }).then_some(tick)
}

#[test]
fn ra8_duel_completion_requires_a_winner() {
    assert_eq!(winning_tick(1, MatchOutcome::Draw { tick: 1 }), None);
    assert_eq!(
        winning_tick(
            1,
            MatchOutcome::Won {
                winner: PlayerId(0),
                tick: 1
            }
        ),
        Some(1)
    );
    assert_eq!(END_LIMIT, 300_000);
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Job {
    map: usize,
    id: String,
    grid: Grid,
    starts: Vec<CellPos>,
    /// Faction and difficulty of the two players.
    sides: [(usize, usize); 2],
}

fn main() -> ExitCode {
    // Optional diagnostic replay selectors; the full gate retains seed14 and every pairing.
    let seed = match std::env::var("RA2_DUEL_SEED") {
        Ok(value) => match value.parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => {
                eprintln!("RA2_DUEL_SEED must be an unsigned integer");
                return ExitCode::FAILURE;
            }
        },
        Err(_) => 14,
    };
    let pair_filter = std::env::var("RA2_DUEL_PAIR").ok();
    let flags = ["full", "doubled", "trace"];
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| arguments.iter().any(|argument| argument == flag);
    let (full, doubled, trace) = (has("full"), has("doubled"), has("trace"));
    let named: Vec<&String> = arguments
        .iter()
        .filter(|argument| !flags.contains(&argument.as_str()))
        .collect();
    let data = repository().join("assets/data");
    let loaded = load_ruleset(&data).and_then(|definitions| {
        let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions)?;
        let ai = load_ai(data.join("ai.ron"), &definitions)?;
        Ok((definitions, skirmish, ai))
    });
    let (definitions, skirmish, ai) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    // The sides, each with its countries (R5); a pairing names a side and a difficulty per
    // player, and the country follows from the map and the pairing.
    let groups = skirmish.pairing_groups();
    let (factions, difficulties) = (groups.len(), ai.difficulties.len());
    let mut jobs = Vec::new();
    for (map, def) in skirmish.maps.iter().enumerate() {
        if !named.is_empty() && !named.iter().any(|id| **id == def.id) {
            continue;
        }
        let file = match load_map(repository().join("assets").join(map_asset_path(&def.id))) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        let (grid, starts) = if doubled {
            match enlarged_map(&file.grid, &file.looks, &def.starts, MAX_MATCH_PLAYERS) {
                Ok(enlarged) => (enlarged.grid, enlarged.starts),
                Err(error) => {
                    eprintln!("{}: {error}", def.id);
                    return ExitCode::FAILURE;
                }
            }
        } else {
            (file.grid, def.starts.clone())
        };
        let mut pairings = Vec::new();
        if full {
            for first in 0..factions * difficulties {
                for second in 0..factions * difficulties {
                    pairings.push([
                        (first / difficulties, first % difficulties),
                        (second / difficulties, second % difficulties),
                    ]);
                }
            }
        } else {
            let (last, middle) = (difficulties - 1, difficulties / 2);
            pairings.extend([
                [(0, middle), (factions - 1, middle)],
                [(factions - 1, last), (0, last)],
                [(0, 0), (0, 0)],
                [(factions - 1, 0), (factions - 1, last)],
            ]);
        }
        for sides in pairings {
            // The number of the pairing among all pairings of sides and difficulties, as
            // `m6_production_matches.rs` counts them, so that both play the same matches.
            let pairing = ((sides[0].0 * factions + sides[1].0) * difficulties + sides[0].1)
                * difficulties
                + sides[1].1;
            let country = |slot: usize| {
                // A side without a country would be a duel that is not played: an error, not
                // a match of country 0 (`pairing_groups` leaves empty groups out).
                SkirmishDefinitions::rotating(&groups[sides[slot].0], map + pairing + slot)
                    .expect("a country of the side")
            };
            let pair = format!(
                "{}:{},{}:{}",
                skirmish.factions[country(0)].id,
                ai.difficulties[sides[0].1].id,
                skirmish.factions[country(1)].id,
                ai.difficulties[sides[1].1].id
            );
            if pair_filter.as_ref().is_some_and(|wanted| wanted != &pair) {
                continue;
            }
            jobs.push(Job {
                map,
                id: def.id.clone(),
                grid: grid.clone(),
                starts: starts.clone(),
                sides: [(country(0), sides[0].1), (country(1), sides[1].1)],
            });
        }
    }
    if jobs.is_empty() {
        eprintln!("no such map in the skirmish file");
        return ExitCode::FAILURE;
    }
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, String, Option<u64>)>> = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else {
                        break;
                    };
                    let name = format!(
                        "{} {}x{}: {} ({}) against {} ({})",
                        job.id,
                        job.grid.width(),
                        job.grid.height(),
                        skirmish.factions[job.sides[0].0].id,
                        ai.difficulties[job.sides[0].1].id,
                        skirmish.factions[job.sides[1].0].id,
                        ai.difficulties[job.sides[1].1].id,
                    );
                    let mut starts = skirmish.clone();
                    starts.maps[job.map].starts.clone_from(&job.starts);
                    let players: Vec<PlayerSetup> = job
                        .sides
                        .iter()
                        .enumerate()
                        .map(|(k, &(faction, difficulty))| PlayerSetup {
                            player: PlayerId(k as u8),
                            faction,
                            start: k,
                            controller: Controller::Ai { difficulty },
                        })
                        .collect();
                    let mut app = App::new();
                    app.add_plugins(SimPlugin { seed });
                    app.world_mut().insert_resource(job.grid.clone());
                    app.world_mut().insert_resource(definitions.clone());
                    if let Err(error) =
                        setup_match(app.world_mut(), &starts, &ai, job.map, &players)
                    {
                        eprintln!("{name}: {error}");
                        results.lock().unwrap().push((index, name, None));
                        continue;
                    }
                    let world = app.world_mut();
                    let started = Instant::now();
                    let mut ended = None;
                    for tick in 0..END_LIMIT {
                        world.run_schedule(SimSchedule);
                        if trace && tick % TRACE_EVERY == 0 {
                            println!("{name}, tick {tick}: {}", situation(world));
                        }
                        if let Some(outcome) = world.resource::<MatchState>().outcome() {
                            ended = winning_tick(tick, outcome);
                            println!(
                                "{name}: ended in tick {tick} ({outcome:?}) after {:.1?}",
                                started.elapsed()
                            );
                            break;
                        }
                    }
                    if ended.is_none() {
                        println!(
                            "{name}: NO WINNER within {END_LIMIT} ticks ({:.1?}): {}",
                            started.elapsed(),
                            situation(world)
                        );
                    }
                    results.lock().unwrap().push((index, name, ended));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort();
    let slowest = results.iter().filter_map(|(_, _, ended)| *ended).max();
    let open: Vec<&String> = results
        .iter()
        .filter(|(_, _, ended)| ended.is_none())
        .map(|(_, name, _)| name)
        .collect();
    println!(
        "{} matches, {} ended, the slowest in tick {}",
        results.len(),
        results.len() - open.len(),
        slowest.map_or("-".to_owned(), |tick| tick.to_string())
    );
    if open.is_empty() {
        ExitCode::SUCCESS
    } else {
        for name in open {
            eprintln!("not ended: {name}");
        }
        ExitCode::FAILURE
    }
}

/// What each player owns and where its units are, on average.
fn situation(world: &mut bevy_ecs::world::World) -> String {
    let definitions = world.resource::<sim::Definitions>().clone();
    let mut text = String::new();
    for player in 0..2_u8 {
        let mut units = 0_i64;
        let (mut x, mut y) = (0_i64, 0_i64);
        // The units by type, and what the harvesters among them do.
        let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
        let mut harvest = Vec::new();
        let mut query = world.query::<(&Unit, &Owner, &Mobile, Option<&sim::Harvester>)>();
        for (unit, owner, mobile, harvester) in query.iter(world) {
            if owner.0 == PlayerId(player) {
                units += 1;
                x += i64::from(mobile.cell.x);
                y += i64::from(mobile.cell.y);
                let id = definitions
                    .unit(unit.kind)
                    .map_or_else(String::new, |def| def.id.clone());
                *kinds.entry(id).or_default() += 1;
                if let Some(harvester) = harvester {
                    harvest.push(format!("{:?}", harvester.state));
                }
            }
        }
        let mut owned: std::collections::BTreeMap<String, usize> = Default::default();
        let mut query = world.query::<(&Building, &Owner)>();
        for (building, owner) in query.iter(world) {
            if owner.0 == PlayerId(player) {
                let id = definitions
                    .building(building.kind)
                    .map_or_else(String::new, |def| def.id.clone());
                *owned.entry(id).or_default() += 1;
            }
        }
        let buildings: usize = owned.values().sum();
        let centre = if units > 0 {
            format!("around ({}, {})", x / units, y / units)
        } else {
            "-".to_owned()
        };
        let credits = world
            .resource::<sim::Players>()
            .get(PlayerId(player))
            .map_or(0, |state| state.credits());
        text.push_str(&format!(
            "player {player}: {buildings} buildings {owned:?}, {units} units {centre} \
             {kinds:?}, harvesters {harvest:?}, {credits} credits; "
        ));
    }
    text
}
