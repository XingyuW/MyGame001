//! M7 soak test (PLAN.md section 7, M7: "a soak test of 36,000 ticks (30 minutes of game time) AI
//! against AI"; docs/DECISIONS.md D56).
//!
//! Run from the repository root with
//!
//! ```text
//! cargo run --release -p sim --example m7_soak
//! ```
//!
//! Every configuration plays [`SOAK_TICKS`] ticks of computer players against each other through
//! `SimPlugin` and `CommandQueue`. When a match ends before, the next match of the same
//! configuration starts in a new simulation, as the game's Restart does, so that all of the ticks
//! are played. The configurations:
//!
//! * the shipped data: every shipped map, every pairing of the shipped factions, and each shipped
//!   difficulty for both players;
//! * the fixture data of the simulation's tests on the fixture map: every pairing of the fixture
//!   factions at each fixture difficulty (so that the soak does not depend on the shipped balance
//!   alone, I10).
//!
//! On every tick it checks invariant I6 (each unit holds the cells it stands in and steps into, and
//! none of them is covered by a building), I7 (no player's credits are negative), that every
//! rejected command is explained by a loss in the tick before (D43 item 5), and that the state stays
//! bounded: units, buildings, projectiles, and pending commands below fixed limits. Each
//! configuration is played twice, and the state hash must be the same on every tick (I3). Heap use
//! is bounded: the global allocator refuses allocations beyond [`MEMORY_BUDGET`] (the process then
//! aborts, failing the run), and the peak is reported. Any failure exits with code 1.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use sim::{
    AiDefinitions, Building, CommandQueue, Controller, Definitions, Grid, MatchOutcome, MatchState,
    Mobile, PlayerId, PlayerSetup, Players, Projectiles, RejectReason, SimEvent, SimId, SimPlugin,
    SimSchedule, SkirmishDefinitions, check_ai_factions, load_ai, load_map, load_match_definitions,
    load_skirmish, map_asset_path, setup_match, state_hash,
};

/// Ticks played per configuration: 30 minutes of game time.
const SOAK_TICKS: u64 = 36_000;

/// Limits of the state (far above what a match reaches; `sim::MAX_AI_ARMY` is 200 per player).
const MAX_UNITS: usize = 1_000;
const MAX_BUILDINGS: usize = 300;
const MAX_PROJECTILES: usize = 10_000;
const MAX_PENDING_COMMANDS: usize = 64;

/// Upper limit of heap bytes in use by the process.
const MEMORY_BUDGET: usize = 1 << 30;

static IN_USE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct BudgetAllocator;

fn reserve(size: usize) -> bool {
    let reserved = IN_USE
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |in_use| {
            in_use
                .checked_add(size)
                .filter(|&total| total <= MEMORY_BUDGET)
        })
        .is_ok();
    if reserved {
        PEAK.fetch_max(IN_USE.load(Ordering::SeqCst), Ordering::SeqCst);
    }
    reserved
}

fn release(size: usize) {
    IN_USE.fetch_sub(size, Ordering::SeqCst);
}

// SAFETY: every call is forwarded to `System` unchanged; the counter only decides whether to
// forward an allocation at all, and failing an allocation by returning null is permitted.
unsafe impl GlobalAlloc for BudgetAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !reserve(layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            release(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !reserve(layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if pointer.is_null() {
            release(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.dealloc(pointer, layout) };
        release(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        if new_size > old_size && !reserve(new_size - old_size) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if new_pointer.is_null() {
            if new_size > old_size {
                release(new_size - old_size);
            }
        } else if new_size < old_size {
            release(old_size - new_size);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: BudgetAllocator = BudgetAllocator;

/// The files of a data set and its maps.
struct DataSet {
    name: &'static str,
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
    grids: Vec<Grid>,
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(
    name: &'static str,
    data: PathBuf,
    files: [&str; 5],
    maps: impl Fn(&str) -> PathBuf,
) -> Result<DataSet, String> {
    let definitions = load_match_definitions(
        data.join(files[0]),
        data.join(files[1]),
        data.join(files[2]),
    )
    .map_err(|error| error.to_string())?;
    let skirmish =
        load_skirmish(data.join(files[3]), &definitions).map_err(|error| error.to_string())?;
    let ai = load_ai(data.join(files[4]), &definitions).map_err(|error| error.to_string())?;
    check_ai_factions(data.join(files[4]), &ai, &skirmish, &definitions)
        .map_err(|error| error.to_string())?;
    let mut grids = Vec::new();
    for map in &skirmish.maps {
        grids.push(
            load_map(maps(&map.id))
                .map_err(|error| error.to_string())?
                .grid,
        );
    }
    Ok(DataSet {
        name,
        definitions,
        skirmish,
        ai,
        grids,
    })
}

/// One configuration: a map, the two players' factions, and the difficulty of both.
#[derive(Clone, Copy)]
struct Config {
    map: usize,
    factions: (usize, usize),
    difficulty: usize,
}

/// What one soak of a configuration played.
#[derive(Debug, PartialEq, Eq)]
struct Played {
    /// The state hash after every tick.
    hashes: Vec<u64>,
    outcomes: Vec<Option<MatchOutcome>>,
    failures: Vec<String>,
}

fn new_match(set: &DataSet, config: Config) -> Result<App, String> {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    app.world_mut()
        .insert_resource(set.grids[config.map].clone());
    app.world_mut().insert_resource(set.definitions.clone());
    let players = [
        PlayerSetup {
            player: PlayerId(0),
            faction: config.factions.0,
            start: 0,
            controller: Controller::Ai {
                difficulty: config.difficulty,
            },
        },
        PlayerSetup {
            player: PlayerId(1),
            faction: config.factions.1,
            start: 1,
            controller: Controller::Ai {
                difficulty: config.difficulty,
            },
        },
    ];
    setup_match(
        app.world_mut(),
        &set.skirmish,
        &set.ai,
        config.map,
        &players,
    )
    .map_err(|error| error.to_string())?;
    Ok(app)
}

/// The invariants and limits of the state after a tick; the problems found.
fn check_state(world: &mut World) -> Vec<String> {
    let mut problems = Vec::new();
    let mut units = world.query::<(&SimId, &Mobile)>();
    let held: Vec<(SimId, Mobile)> = units.iter(world).map(|(id, m)| (*id, *m)).collect();
    let grid = world.resource::<Grid>();
    for (id, mobile) in &held {
        for cell in [mobile.cell, mobile.next_centre()] {
            if grid.occupant(cell) != Some(*id) {
                problems.push(format!("I6: unit {} does not hold its cell {cell:?}", id.0));
            }
            if grid.building(cell).is_some() {
                problems.push(format!(
                    "I6: unit {} stands on a building at {cell:?}",
                    id.0
                ));
            }
        }
    }
    for (player, state) in world.resource::<Players>().iter() {
        if state.credits() < 0 {
            problems.push(format!(
                "I7: player {} has {} credits",
                player.0,
                state.credits()
            ));
        }
    }
    let buildings = world.query::<&Building>().iter(world).count();
    let projectiles = world.resource::<Projectiles>().iter().count();
    let pending = world.resource::<CommandQueue>().len();
    if held.len() > MAX_UNITS
        || buildings > MAX_BUILDINGS
        || projectiles > MAX_PROJECTILES
        || pending > MAX_PENDING_COMMANDS
    {
        problems.push(format!(
            "state beyond its limits: {} units, {buildings} buildings, {projectiles} projectiles, \
             {pending} pending commands",
            held.len()
        ));
    }
    problems
}

fn soak(set: &DataSet, config: Config) -> Played {
    let mut played = Played {
        hashes: Vec::with_capacity(SOAK_TICKS as usize),
        outcomes: Vec::new(),
        failures: Vec::new(),
    };
    let mut app = match new_match(set, config) {
        Ok(app) => app,
        Err(error) => {
            played.failures.push(error);
            return played;
        }
    };
    let mut losses: BTreeMap<PlayerId, BTreeSet<SimId>> = BTreeMap::new();
    for at in 0..SOAK_TICKS {
        app.world_mut().run_schedule(SimSchedule);
        let events: Vec<SimEvent> = app
            .world()
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        let before = std::mem::take(&mut losses);
        for event in events {
            match event {
                SimEvent::CommandRejected {
                    player,
                    command,
                    reason,
                } => {
                    let lost = before.get(&player);
                    let explained = match reason {
                        RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                            lost.is_some_and(|lost| lost.contains(&id))
                        }
                        RejectReason::MissingPrerequisite { .. }
                        | RejectReason::NoProducer { .. }
                        | RejectReason::Placement(_) => lost.is_some_and(|lost| !lost.is_empty()),
                        _ => false,
                    };
                    if !explained {
                        played.failures.push(format!(
                            "tick {at}: {command:?} of player {} rejected ({reason}) without a \
                             loss",
                            player.0
                        ));
                    }
                }
                SimEvent::UnitDestroyed { player, unit } => {
                    losses.entry(player).or_default().insert(unit);
                }
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => {
                    losses.entry(player).or_default().insert(building);
                }
                _ => {}
            }
        }
        for problem in check_state(app.world_mut()) {
            played.failures.push(format!("tick {at}: {problem}"));
        }
        if played.failures.len() > 20 {
            played
                .failures
                .push("too many failures; the soak stops".into());
            return played;
        }
        played.hashes.push(state_hash(app.world_mut()));
        if let Some(outcome) = app.world().resource::<MatchState>().outcome() {
            // The next match of the configuration, as the game's Restart starts it.
            played.outcomes.push(Some(outcome));
            losses.clear();
            match new_match(set, config) {
                Ok(next) => app = next,
                Err(error) => {
                    played.failures.push(error);
                    return played;
                }
            }
        }
    }
    // The match in progress at the end.
    if app.world().resource::<MatchState>().outcome().is_none() {
        played.outcomes.push(None);
    }
    played
}

fn main() -> ExitCode {
    let repository = repository();
    let assets = repository.join("assets");
    let fixtures = repository.join("crates/sim/tests/fixtures");
    let shipped = load(
        "shipped",
        assets.join("data"),
        [
            "units.ron",
            "economy.ron",
            "combat.ron",
            "skirmish.ron",
            "ai.ron",
        ],
        |id| assets.join(map_asset_path(id)),
    );
    let fixture = load(
        "fixture",
        fixtures.join("data"),
        [
            "units_combat.ron",
            "economy_combat.ron",
            "combat.ron",
            "skirmish.ron",
            "ai.ron",
        ],
        // The fixture skirmish file's map lies among the fixture maps.
        |id| fixtures.join("maps").join(format!("{id}.tmx")),
    );
    let mut failures = Vec::new();
    let mut total_ticks = 0u64;
    let started = Instant::now();
    for set in [shipped, fixture] {
        let set = match set {
            Ok(set) => set,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let factions = set.skirmish.factions.len();
        for map in 0..set.skirmish.maps.len() {
            for first in 0..factions {
                for second in 0..factions {
                    for difficulty in 0..set.ai.difficulties.len() {
                        let config = Config {
                            map,
                            factions: (first, second),
                            difficulty,
                        };
                        let name = format!(
                            "{} {}: {} against {}, both {}",
                            set.name,
                            set.skirmish.maps[map].id,
                            set.skirmish.factions[first].id,
                            set.skirmish.factions[second].id,
                            set.ai.difficulties[difficulty].id
                        );
                        let clock = Instant::now();
                        let played = soak(&set, config);
                        let again = soak(&set, config);
                        total_ticks += played.hashes.len() as u64 + again.hashes.len() as u64;
                        let ended = played.outcomes.iter().filter(|o| o.is_some()).count();
                        println!(
                            "{name}: {} ticks, {} matches ({ended} ended, the last {}), {:.2} s \
                             for both runs",
                            played.hashes.len(),
                            played.outcomes.len(),
                            if played.outcomes.last().is_some_and(Option::is_none) {
                                "still running"
                            } else {
                                "ended"
                            },
                            clock.elapsed().as_secs_f64()
                        );
                        for failure in &played.failures {
                            failures.push(format!("{name}: {failure}"));
                        }
                        if played.hashes.len() != SOAK_TICKS as usize && played.failures.is_empty()
                        {
                            failures.push(format!(
                                "{name}: {} of {SOAK_TICKS} ticks played",
                                played.hashes.len()
                            ));
                        }
                        if played != again {
                            let first_difference = played
                                .hashes
                                .iter()
                                .zip(&again.hashes)
                                .position(|(a, b)| a != b);
                            failures.push(format!(
                                "{name}: the two runs differ (first different tick: \
                                 {first_difference:?})"
                            ));
                        }
                    }
                }
            }
        }
    }
    println!(
        "m7 soak: {total_ticks} ticks in {:.1} s; peak heap in use {:.1} MiB (budget {} MiB)",
        started.elapsed().as_secs_f64(),
        PEAK.load(Ordering::SeqCst) as f64 / f64::from(1u32 << 20),
        MEMORY_BUDGET >> 20
    );
    if failures.is_empty() {
        println!("m7 soak: passed");
        ExitCode::SUCCESS
    } else {
        for failure in &failures {
            eprintln!("m7 soak failed: {failure}");
        }
        ExitCode::FAILURE
    }
}
