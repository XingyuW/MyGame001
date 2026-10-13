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
//! * the shipped data: every shipped map, every pairing of the two sides (each played by its
//!   countries in turn, docs/DECISIONS.md R5), and each shipped difficulty for both players;
//! * the fixture data of the simulation's tests on the fixture map: every pairing of the fixture
//!   factions at each fixture difficulty (so that the soak does not depend on the shipped balance
//!   alone, I10).
//!
//! On every tick it checks invariant I6' (docs/DECISIONS.md R6, the amendment of I6 in RA3: a
//! surface cell is held by at most one vehicle or at most three infantry and never both, a cell of
//! the air layer by at most one aircraft, no unit of the surface holds a cell covered by a
//! building, and the grid records every unit in the cells it stands in and steps into;
//! `sim::cell_invariant_violations`, whose own test feeds it each kind of violation), every
//! [`STRAY_CHECK_INTERVAL`] ticks that the grid records no unit in a cell it does not hold
//! (`sim::stray_claims`, which looks at every cell of the map), I7 (no player's credits are
//! negative), that every
//! rejected command is explained by a loss in the tick before (D43 item 5), and that the state stays
//! bounded: units, buildings, projectiles, and pending commands below fixed limits. Each
//! configuration is played twice, and the state hash must be the same on every tick (I3). Heap use
//! is bounded: the global allocator refuses allocations beyond [`MEMORY_BUDGET`] (the process then
//! aborts, failing the run), and the peak is reported. Any failure exits with code 1.
//!
//! The configurations are independent simulations and are played on all cores (docs/DECISIONS.md
//! R8); the two runs of a configuration are played one after the other on one thread, results are
//! reported in the order of the configurations, and the heap budget and the reported peak cover
//! all configurations that run at the same time.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use sim::{
    AiDefinitions, Building, CommandQueue, Controller, Definitions, Grid, MatchOutcome, MatchState,
    Mobile, PlayerId, PlayerSetup, Players, Projectiles, RejectReason, SimEvent, SimId, SimPlugin,
    SimSchedule, SkirmishDefinitions, cell_invariant_violations, check_ai_factions, load_ai,
    load_map, load_match_definitions, load_ruleset, load_skirmish, map_asset_path, setup_match,
    state_hash, stray_claims,
};

/// Ticks played per configuration: 30 minutes of game time.
const SOAK_TICKS: u64 = 36_000;

/// Limits of the state (far above what a match reaches; `sim::MAX_AI_ARMY` is 200 per player).
const MAX_UNITS: usize = 1_000;
const MAX_BUILDINGS: usize = 300;
const MAX_PROJECTILES: usize = 10_000;
const MAX_PENDING_COMMANDS: usize = 64;

/// Ticks between two looks at every cell of the map for a holder that holds nothing. A claim
/// that is not released stays, so it is found at the next look.
const STRAY_CHECK_INTERVAL: u64 = 256;

/// Upper limit of heap bytes in use by the process.
const MEMORY_BUDGET: usize = 1 << 30;

static IN_USE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct BudgetAllocator;

/// Bytes a thread takes from the shared counter at a time. The configurations run on all cores
/// and allocate on every tick; updating the shared counter on every allocation made the threads
/// wait for each other's cache line, so that ten threads played no faster than one (docs/
/// DECISIONS.md R8). Each thread therefore keeps a credit of bytes it has already counted.
const CREDIT_CHUNK: usize = 256 << 10;

thread_local! {
    /// Bytes this thread has counted in [`IN_USE`] and not handed out. At most
    /// `2 * CREDIT_CHUNK` plus one allocation, so [`IN_USE`] and [`PEAK`] exceed the bytes really
    /// in use by at most that per thread: the budget is enforced a little early, never late.
    static CREDIT: Cell<usize> = const { Cell::new(0) };
}

fn reserve_shared(size: usize) -> bool {
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

fn reserve(size: usize) -> bool {
    CREDIT
        .try_with(|credit| {
            let have = credit.get();
            if have >= size {
                credit.set(have - size);
                return true;
            }
            // Take a chunk more than is missing; near the budget, only what is missing.
            let missing = size - have;
            let Some(wanted) = missing.checked_add(CREDIT_CHUNK) else {
                return false;
            };
            if reserve_shared(wanted) {
                credit.set(CREDIT_CHUNK);
                true
            } else if reserve_shared(missing) {
                credit.set(0);
                true
            } else {
                false
            }
        })
        // A thread that is ending has no credit any more.
        .unwrap_or_else(|_| reserve_shared(size))
}

fn release(size: usize) {
    let kept = CREDIT.try_with(|credit| {
        let have = credit.get().saturating_add(size);
        if have > 2 * CREDIT_CHUNK {
            IN_USE.fetch_sub(have - CREDIT_CHUNK, Ordering::SeqCst);
            credit.set(CREDIT_CHUNK);
        } else {
            credit.set(have);
        }
    });
    if kept.is_err() {
        IN_USE.fetch_sub(size, Ordering::SeqCst);
    }
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
    // The shipped ruleset has a rules file and an arsenal (RA1); the fixture has neither.
    let definitions = if name == "shipped" {
        load_ruleset(&data)
    } else {
        load_match_definitions(
            data.join(files[0]),
            data.join(files[1]),
            data.join(files[2]),
        )
    }
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

/// The invariants and limits of the state after tick `at`; the problems found.
fn check_state(world: &mut World, at: u64) -> Vec<String> {
    // I6' (docs/DECISIONS.md R6): who holds which cell, from the units and from the grid.
    let mut problems = cell_invariant_violations(world);
    if at.is_multiple_of(STRAY_CHECK_INTERVAL) || at + 1 == SOAK_TICKS {
        problems.extend(stray_claims(world));
    }
    let units = world.query::<&Mobile>().iter(world).count();
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
    if units > MAX_UNITS
        || buildings > MAX_BUILDINGS
        || projectiles > MAX_PROJECTILES
        || pending > MAX_PENDING_COMMANDS
    {
        problems.push(format!(
            "state beyond its limits: {units} units, {buildings} buildings, {projectiles} \
             projectiles, {pending} pending commands"
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
                        // A unit of another player that the placing player could not see
                        // beside the footprint stepped into it (docs/DECISIONS.md D43 and
                        // "Maps at the engine's largest size", item 8): on the large maps
                        // armies reach a base that is still building, as the end to end tests
                        // of the game crate already accept.
                        RejectReason::Placement(sim::PlacementError::Occupied { unit, .. }) => {
                            let world = app.world_mut();
                            let mut owners = world.query::<(&SimId, &sim::Owner)>();
                            lost.is_some_and(|lost| !lost.is_empty())
                                || owners
                                    .iter(world)
                                    .any(|(id, owner)| *id == unit && owner.0 != player)
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
        for problem in check_state(app.world_mut(), at) {
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
    let mut sets = Vec::new();
    for set in [shipped, fixture] {
        match set {
            Ok(set) => sets.push(set),
            Err(error) => failures.push(error),
        }
    }
    // The configurations in the order they are reported in.
    let mut jobs = Vec::new();
    for (index, set) in sets.iter().enumerate() {
        // Every pairing of the sides, each side played by its countries in turn, or without
        // countries every pairing of the factions (docs/DECISIONS.md R5).
        let groups = set.skirmish.pairing_groups();
        for map in 0..set.skirmish.maps.len() {
            for (pairing, (first, second)) in (0..groups.len())
                .flat_map(|first| (0..groups.len()).map(move |second| (first, second)))
                .enumerate()
            {
                let turn = map + pairing;
                let (Some(first), Some(second)) = (
                    SkirmishDefinitions::rotating(&groups[first], turn),
                    SkirmishDefinitions::rotating(&groups[second], turn + 1),
                ) else {
                    // A side without a country would be a configuration that is silently not
                    // soaked (`pairing_groups` leaves empty groups out).
                    panic!("a side of the shipped skirmish file has no country");
                };
                {
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
                        jobs.push((index, config, name));
                    }
                }
            }
        }
    }
    // Configurations are independent simulations, so they are played on all cores (docs/
    // DECISIONS.md R8); both runs of a configuration stay on one thread, and the heap budget
    // covers everything that runs at once.
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<(Played, Played, f64)>>> =
        Mutex::new((0..jobs.len()).map(|_| None).collect());
    let workers = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some((set, config, name)) = jobs.get(index) else {
                        break;
                    };
                    let clock = Instant::now();
                    let played = soak(&sets[*set], *config);
                    let again = soak(&sets[*set], *config);
                    let seconds = clock.elapsed().as_secs_f64();
                    // Progress, in the order the configurations finish; the report follows in
                    // their own order once all are played.
                    eprintln!("played {} of {}: {name}", index + 1, jobs.len());
                    let mut results = results.lock().unwrap_or_else(|e| e.into_inner());
                    results[index] = Some((played, again, seconds));
                }
            });
        }
    });
    let results = results.into_inner().unwrap_or_else(|e| e.into_inner());
    for ((_, _, name), result) in jobs.iter().zip(results) {
        let Some((played, again, seconds)) = result else {
            failures.push(format!("{name}: not played"));
            continue;
        };
        total_ticks += played.hashes.len() as u64 + again.hashes.len() as u64;
        let ended = played.outcomes.iter().filter(|o| o.is_some()).count();
        println!(
            "{name}: {} ticks, {} matches ({ended} ended, the last {}), {seconds:.2} s for both \
             runs",
            played.hashes.len(),
            played.outcomes.len(),
            if played.outcomes.last().is_some_and(Option::is_none) {
                "still running"
            } else {
                "ended"
            },
        );
        for failure in &played.failures {
            failures.push(format!("{name}: {failure}"));
        }
        if played.hashes.len() != SOAK_TICKS as usize && played.failures.is_empty() {
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
                "{name}: the two runs differ (first different tick: {first_difference:?})"
            ));
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
