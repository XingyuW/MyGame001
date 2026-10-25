//! M7 benchmark (PLAN.md section 7, M7: "a headless benchmark with 200 moving and fighting units
//! reports the mean and maximum tick time against the budget recorded in docs/DECISIONS.md";
//! docs/DECISIONS.md D55).
//!
//! Run from the repository root with
//!
//! ```text
//! cargo run --release -p sim --example m7_benchmark
//! ```
//!
//! Two scenarios, each with 200 armed units, 100 per player, that attack-move across the map into
//! each other through `CommandQueue` and fight to the end of the measured window:
//!
//! 1. `fixture`: the fixture data and the 40 x 40 skirmish fixture map of the simulation's tests
//!    (riflemen and tanks), so that the result does not depend on the shipped balance (I10);
//! 2. `shipped`: the shipped data on the shipped demo map (64 x 64), each player with the armed
//!    ground units of one side (the own tech tree of its first country), so that the result is
//!    the shipped game's.
//!
//! After one warm-up tick (which builds the schedule), [`MEASURED_TICKS`] ticks are timed one by
//! one with the clock (the harness reads the clock; the simulation never does, I4). The run fails
//! (exit code 1) if a scenario's mean tick time exceeds [`MEAN_BUDGET`] or its longest tick
//! [`MAX_BUDGET`], or if it did not measure what it claims: 200 units spawned, units moved, shots
//! were fired, and units were destroyed. The mean over the busy part of the battle (the ticks that
//! start with at least [`BUSY_UNITS`] units alive) is reported and held to the same budget. Heap
//! use is bounded: the global allocator refuses
//! allocations beyond [`MEMORY_BUDGET`] (the process then aborts, failing the run), and the peak
//! is reported.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand, PlayerId, Players, Position,
    ProductionItem, SimEvent, SimId, SimPlugin, SimSchedule, SubPos, Unit, load_map,
    load_match_definitions, load_skirmish, plan_starting_units, spawn_unit,
};

/// The budget for the mean tick time (PLAN.md section 7, M7: "initial target: mean ≤ 5 ms in
/// release mode on the development machine").
const MEAN_BUDGET: Duration = Duration::from_millis(5);

/// The budget for the longest tick: one tick period at 20 ticks per second. A longer tick would
/// make the game fall behind real time.
const MAX_BUDGET: Duration = Duration::from_millis(50);

/// Ticks measured per scenario (100 s of game time).
const MEASURED_TICKS: u64 = 2_000;

/// Armed units per player.
const UNITS_PER_PLAYER: usize = 100;

/// Ticks that start with at least this many units alive count as the busy part of the battle,
/// whose mean is reported and checked against the budget as well.
const BUSY_UNITS: usize = 150;

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

/// One side of a scenario: its player, the unit types it fields (in turn), where its units start,
/// and where they attack-move to.
struct Side {
    player: PlayerId,
    units: Vec<String>,
    anchor: CellPos,
    target: CellPos,
}

/// Ordered monotonic witnesses for the ORIGINAL commanded actors, never survivor counts.
struct MovementWitness {
    start: BTreeMap<SimId, SubPos>,
    moved: BTreeSet<SimId>,
}
impl MovementWitness {
    fn new(start: BTreeMap<SimId, SubPos>) -> Self {
        Self {
            start,
            moved: BTreeSet::new(),
        }
    }
    fn observe(&mut self, current: &BTreeMap<SimId, SubPos>) {
        for (&id, position) in &self.start {
            if current.get(&id).is_some_and(|now| now != position) {
                self.moved.insert(id);
            }
        }
    }
    fn complete(&self) -> bool {
        self.moved.len() == self.start.len()
    }
}

/// What a scenario measured.
struct Report {
    name: &'static str,
    spawned: usize,
    mean: Duration,
    max: Duration,
    /// The mean over the ticks that started with at least [`BUSY_UNITS`] units alive, and their
    /// number.
    busy_mean: Duration,
    busy_ticks: u64,
    /// Units alive every 500 ticks, from the first measured tick on.
    alive: Vec<usize>,
    moved: usize,
    shots: u64,
    destroyed: u64,
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn units(app: &mut App) -> BTreeMap<SimId, SubPos> {
    let mut query = app.world_mut().query::<(&SimId, &Position, &Unit)>();
    query
        .iter(app.world())
        .map(|(id, position, _)| (*id, position.0))
        .collect()
}

fn run(
    name: &'static str,
    definitions: &Definitions,
    grid: Grid,
    sides: &[Side],
) -> Result<Report, String> {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 7 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions.clone());
    for side in sides {
        // Players of the match, so that they see and their units acquire targets (D30).
        app.world_mut()
            .resource_mut::<Players>()
            .add(side.player, 0)
            .map_err(|error| format!("{name}: {error}"))?;
    }
    // Scenario setup (as match setup, D40): every unit on the free cells nearest to its anchor.
    let groups: Vec<(PlayerId, &str, (i32, i32), usize)> = sides
        .iter()
        .flat_map(|side| {
            side.units.iter().enumerate().map(move |(index, unit)| {
                let count = UNITS_PER_PLAYER / side.units.len()
                    + usize::from(index < UNITS_PER_PLAYER % side.units.len());
                (
                    side.player,
                    unit.as_str(),
                    (side.anchor.x, side.anchor.y),
                    count,
                )
            })
        })
        .collect();
    let planned = plan_starting_units(app.world().resource::<Grid>(), &groups)
        .map_err(|error| format!("{name}: {error}"))?;
    let mut ids: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
    for (player, unit, cell) in planned {
        let id = spawn_unit(app.world_mut(), player, unit, cell)
            .map_err(|error| format!("{name}: {error}"))?;
        ids.entry(player).or_default().push(id);
    }
    let spawned = ids.values().map(Vec::len).sum();
    let originals: BTreeSet<_> = ids.values().flatten().copied().collect();
    let initial = units(&mut app);
    if initial.keys().copied().collect::<BTreeSet<_>>() != originals || initial.len() != spawned {
        return Err(format!(
            "{name}: original participant snapshot differs from commanded ids"
        ));
    }
    let mut motion = MovementWitness::new(initial);
    // The warm-up tick builds the schedule and computes the first vision.
    app.world_mut().run_schedule(SimSchedule);
    for side in sides {
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: side.player,
                command: Command::AttackMove {
                    units: ids.get(&side.player).cloned().unwrap_or_default(),
                    target: side.target,
                },
            });
    }
    motion.observe(&units(&mut app));
    let (mut total, mut max) = (Duration::ZERO, Duration::ZERO);
    let (mut busy_total, mut busy_ticks) = (Duration::ZERO, 0u64);
    let (mut shots, mut destroyed) = (0, 0);
    let mut alive = Vec::new();
    for at in 0..MEASURED_TICKS {
        let current = units(&mut app);
        let count = current.len();
        motion.observe(&current);
        if at % 500 == 0 {
            alive.push(count);
        }
        let clock = Instant::now();
        app.world_mut().run_schedule(SimSchedule);
        let elapsed = clock.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        if count >= BUSY_UNITS {
            busy_total += elapsed;
            busy_ticks += 1;
        }
        for event in app
            .world()
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
        {
            match event {
                SimEvent::WeaponFired { .. } => shots += 1,
                SimEvent::UnitDestroyed { .. } => destroyed += 1,
                SimEvent::CommandRejected { reason, .. } => {
                    return Err(format!("{name}: a command was rejected: {reason}"));
                }
                _ => {}
            }
        }
    }
    alive.push(units(&mut app).len());
    let end = units(&mut app);
    motion.observe(&end);
    let moved = motion.moved.len();
    if !motion.complete() {
        let unmoved: Vec<_> = motion
            .start
            .keys()
            .filter(|id| !motion.moved.contains(*id))
            .collect();
        return Err(format!(
            "{name}: original participants without observed movement: {unmoved:?}"
        ));
    }
    Ok(Report {
        name,
        spawned,
        mean: total / u32::try_from(MEASURED_TICKS).unwrap_or(u32::MAX),
        max,
        busy_mean: busy_total / u32::try_from(busy_ticks.max(1)).unwrap_or(u32::MAX),
        busy_ticks,
        alive,
        moved,
        shots,
        destroyed,
    })
}

fn fixture_scenario() -> Result<Report, String> {
    let fixtures = repository().join("crates/sim/tests/fixtures");
    let definitions = load_match_definitions(
        fixtures.join("data/units_combat.ron"),
        fixtures.join("data/economy_combat.ron"),
        fixtures.join("data/combat.ron"),
    )
    .map_err(|error| error.to_string())?;
    let grid = load_map(fixtures.join("maps/skirmish_40x40.tmx"))
        .map_err(|error| error.to_string())?
        .grid;
    let army = vec!["rifleman".to_string(), "tank".to_string()];
    run(
        "fixture",
        &definitions,
        grid,
        &[
            Side {
                player: PlayerId(0),
                units: army.clone(),
                anchor: CellPos::new(8, 8),
                target: CellPos::new(31, 31),
            },
            Side {
                player: PlayerId(1),
                units: army,
                anchor: CellPos::new(31, 31),
                target: CellPos::new(8, 8),
            },
        ],
    )
}

fn shipped_scenario() -> Result<Report, String> {
    let data = repository().join("assets/data");
    let definitions = sim::load_ruleset(&data).map_err(|error| error.to_string())?;
    let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions)
        .map_err(|error| error.to_string())?;
    let demo = skirmish
        .maps
        .iter()
        .find(|map| map.id == "demo")
        .ok_or("the shipped skirmish file has no demo map")?;
    let grid = load_map(
        repository()
            .join("assets")
            .join(sim::map_asset_path(&demo.id)),
    )
    .map_err(|error| error.to_string())?
    .grid;
    // Each player fields the armed ground units of one side: those of the own tech tree of its
    // first country (RA1; ships and aircraft do not stand on the ground the scenario uses).
    let groups = skirmish.pairing_groups();
    let country = |side: usize| groups.get(side).and_then(|group| group.first()).copied();
    let (Some(allied), Some(soviet)) = (country(0), country(1)) else {
        return Err("the shipped skirmish file has no two sides".into());
    };
    let armed = |faction: usize| -> Vec<String> {
        skirmish
            .factions
            .get(faction)
            .map_or_else(Vec::new, |faction| {
                faction
                    .roster
                    .iter()
                    .filter_map(|item| match item {
                        ProductionItem::Unit(kind) => Some(*kind),
                        ProductionItem::Building(_) => None,
                    })
                    .filter(|&kind| {
                        definitions.domain(kind) == sim::Domain::Land
                            && definitions
                                .unit_combat(kind)
                                .is_some_and(|terms| terms.weapon.is_some())
                    })
                    .filter_map(|kind| definitions.unit(kind).map(|def| def.id.clone()))
                    .collect()
            })
    };
    let (first, second) = (armed(allied), armed(soviet));
    if first.is_empty() || second.is_empty() {
        return Err("a shipped faction has no armed unit type".into());
    }
    let (a, b) = (demo.starts[0], demo.starts[1]);
    run(
        "shipped",
        &definitions,
        grid,
        &[
            Side {
                player: PlayerId(0),
                units: first,
                anchor: a,
                target: b,
            },
            Side {
                player: PlayerId(1),
                units: second,
                anchor: b,
                target: a,
            },
        ],
    )
}

fn main() -> ExitCode {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "m7 benchmark ({profile} build): {} armed units per player, {MEASURED_TICKS} measured ticks \
         per scenario; budget: mean <= {MEAN_BUDGET:?}, longest tick <= {MAX_BUDGET:?}",
        UNITS_PER_PLAYER
    );
    let mut failures = Vec::new();
    for scenario in [fixture_scenario, shipped_scenario] {
        match scenario() {
            Err(error) => failures.push(error),
            Ok(report) => {
                println!(
                    "{}: {} units spawned; mean tick {:.3} ms, longest tick {:.3} ms; mean over \
                     the {} ticks with at least {BUSY_UNITS} units alive {:.3} ms; units alive \
                     every 500 ticks {:?}; {} units moved, {} shots, {} units destroyed",
                    report.name,
                    report.spawned,
                    report.mean.as_secs_f64() * 1e3,
                    report.max.as_secs_f64() * 1e3,
                    report.busy_ticks,
                    report.busy_mean.as_secs_f64() * 1e3,
                    report.alive,
                    report.moved,
                    report.shots,
                    report.destroyed
                );
                if report.spawned != 2 * UNITS_PER_PLAYER {
                    failures.push(format!(
                        "{}: {} units spawned, not {}",
                        report.name,
                        report.spawned,
                        2 * UNITS_PER_PLAYER
                    ));
                }
                if report.moved != report.spawned || report.shots == 0 || report.destroyed == 0 {
                    failures.push(format!(
                        "{}: the units did not move and fight (moved {}, shots {}, destroyed {})",
                        report.name, report.moved, report.shots, report.destroyed
                    ));
                }
                if report.mean > MEAN_BUDGET || report.busy_mean > MEAN_BUDGET {
                    failures.push(format!(
                        "{}: mean tick {:?} (busy part {:?}) exceeds the budget of \
                         {MEAN_BUDGET:?}",
                        report.name, report.mean, report.busy_mean
                    ));
                }
                if report.busy_ticks == 0 {
                    failures.push(format!(
                        "{}: no tick started with {BUSY_UNITS} units alive",
                        report.name
                    ));
                }
                if report.max > MAX_BUDGET {
                    failures.push(format!(
                        "{}: longest tick {:?} exceeds the budget of {MAX_BUDGET:?}",
                        report.name, report.max
                    ));
                }
            }
        }
    }
    println!(
        "peak heap in use: {:.1} MiB (budget {} MiB)",
        PEAK.load(Ordering::SeqCst) as f64 / f64::from(1u32 << 20),
        MEMORY_BUDGET >> 20
    );
    if failures.is_empty() {
        println!("m7 benchmark: passed");
        ExitCode::SUCCESS
    } else {
        for failure in &failures {
            eprintln!("m7 benchmark failed: {failure}");
        }
        ExitCode::FAILURE
    }
}

// Append to m7_benchmark.rs; these checks preserve both native scenarios and all old budgets.
#[cfg(test)]
mod ra9_boundary_tests {
    use super::*;
    #[test]
    fn ra9_movement_is_monotonic_for_returned_units_and_casualties() {
        let (a, b) = (SimId(1), SimId(2));
        let start = SubPos { x: 0, y: 0 };
        let moved = SubPos { x: 1, y: 0 };
        let mut w = MovementWitness::new(BTreeMap::from([(a, start), (b, start)]));
        w.observe(&BTreeMap::from([(a, moved), (b, start)]));
        assert!(!w.complete());
        w.observe(&BTreeMap::from([(b, moved)]));
        assert!(w.complete());
        w.observe(&BTreeMap::from([(b, start)]));
        assert!(w.complete());
        let mut negative = MovementWitness::new(BTreeMap::from([(a, start), (b, start)]));
        negative.observe(&BTreeMap::from([(b, moved)]));
        assert!(!negative.complete());
        assert_eq!(negative.moved.len(), 1);
    }
    #[test]
    fn ra9_both_real_benchmark_scenarios_witness_all_original_commanded_actors() {
        for scenario in [fixture_scenario, shipped_scenario] {
            let report = scenario().expect("native scenario, no omitted participant");
            assert_eq!(report.spawned, 2 * UNITS_PER_PLAYER);
            assert_eq!(report.moved, report.spawned);
            assert!(report.shots > 0 && report.destroyed > 0);
            assert!(report.busy_ticks > 0);
            assert!(
                report.mean <= MEAN_BUDGET
                    && report.busy_mean <= MEAN_BUDGET
                    && report.max <= MAX_BUDGET
            );
        }
    }
}
