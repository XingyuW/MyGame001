//! M0 tests: tick counter, determinism of `state_hash`, and the tick stage order.
//!
//! Every test drives the production path: `SimPlugin` installs the resources and `SimSchedule`,
//! and the schedule is run directly, once per tick.

use bevy_app::App;
use bevy_ecs::prelude::{IntoScheduleConfigs, ResMut, Resource};
use sim::{
    CommandQueue, SimIdAllocator, SimPlugin, SimRng, SimSchedule, SimSet, SimTick, state_hash,
};

fn sim_app(seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    app
}

fn run_ticks(app: &mut App, n: u64) {
    for _ in 0..n {
        app.world_mut().run_schedule(SimSchedule);
    }
}

fn hash(app: &mut App) -> u64 {
    state_hash(app.world_mut())
}

/// Hashes after tick 0 (initial state) and after each of `ticks` runs.
fn hash_trace(app: &mut App, ticks: u64) -> Vec<u64> {
    let mut trace = vec![hash(app)];
    for _ in 0..ticks {
        run_ticks(app, 1);
        trace.push(hash(app));
    }
    trace
}

/// Fixture system standing in for a future consumer of randomness (e.g. AI decisions).
fn consume_randomness(mut rng: ResMut<SimRng>) {
    rng.next_u64();
}

fn sim_app_consuming_randomness(seed: u64) -> App {
    let mut app = sim_app(seed);
    app.add_systems(SimSchedule, consume_randomness.in_set(SimSet::Ai));
    app
}

// --- Tick counter ---------------------------------------------------------------------------

#[test]
fn tick_counter_starts_at_zero() {
    let app = sim_app(0);
    assert_eq!(*app.world().resource::<SimTick>(), SimTick(0));
}

#[test]
fn tick_counter_equals_n_after_n_schedule_runs() {
    for n in [1, 2, 20, 1_000] {
        let mut app = sim_app(0);
        run_ticks(&mut app, n);
        assert_eq!(app.world().resource::<SimTick>().0, n, "after {n} runs");
    }
}

#[test]
fn tick_counter_increments_by_exactly_one_per_run() {
    let mut app = sim_app(0);
    for expected in 1..=100 {
        run_ticks(&mut app, 1);
        assert_eq!(app.world().resource::<SimTick>().0, expected);
    }
}

// --- Determinism (invariant I3) -------------------------------------------------------------

#[test]
fn same_seed_and_empty_command_log_give_equal_hashes_at_every_tick() {
    let mut a = sim_app(42);
    let mut b = sim_app(42);
    assert!(a.world().resource::<CommandQueue>().is_empty());

    let trace_a = hash_trace(&mut a, 500);
    let trace_b = hash_trace(&mut b, 500);

    assert_eq!(trace_a, trace_b);
    assert!(a.world().resource::<CommandQueue>().is_empty());
}

#[test]
fn same_seed_consuming_randomness_gives_equal_hashes_at_every_tick() {
    let trace_a = hash_trace(&mut sim_app_consuming_randomness(7), 500);
    let trace_b = hash_trace(&mut sim_app_consuming_randomness(7), 500);
    assert_eq!(trace_a, trace_b);
}

#[test]
fn different_seeds_consuming_randomness_give_different_hashes() {
    let trace_a = hash_trace(&mut sim_app_consuming_randomness(1), 500);
    let trace_b = hash_trace(&mut sim_app_consuming_randomness(2), 500);
    for (tick, (a, b)) in trace_a.iter().zip(&trace_b).enumerate() {
        assert_ne!(a, b, "hashes collide at tick {tick}");
    }
}

#[test]
fn hash_reflects_consumed_randomness() {
    // Same seed, but only one app draws from the RNG: the hashes agree before the first tick
    // and differ afterwards, so the hash tracks the generator's position, not just its seed.
    let mut consuming = sim_app_consuming_randomness(3);
    let mut idle = sim_app(3);
    assert_eq!(hash(&mut consuming), hash(&mut idle));
    run_ticks(&mut consuming, 1);
    run_ticks(&mut idle, 1);
    assert_ne!(hash(&mut consuming), hash(&mut idle));
}

#[test]
fn hash_changes_every_tick() {
    let trace = hash_trace(&mut sim_app(0), 100);
    for pair in trace.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[test]
fn hash_is_independent_of_entity_spawn_order() {
    // Scenario setup: spawn the same five SimId entities in two different orders.
    fn app_with_ids_spawned_in(order: &[usize]) -> App {
        let mut app = sim_app(5);
        let world = app.world_mut();
        let ids: Vec<_> = (0..order.len())
            .map(|_| world.resource_mut::<SimIdAllocator>().allocate())
            .collect::<Option<_>>()
            .expect("five ids fit in u32");
        for &index in order {
            world.spawn(ids[index]);
        }
        app
    }

    let mut forward = app_with_ids_spawned_in(&[0, 1, 2, 3, 4]);
    let mut shuffled = app_with_ids_spawned_in(&[3, 0, 4, 2, 1]);
    assert_eq!(hash(&mut forward), hash(&mut shuffled));

    run_ticks(&mut forward, 10);
    run_ticks(&mut shuffled, 10);
    assert_eq!(hash(&mut forward), hash(&mut shuffled));
}

#[test]
fn hash_changes_when_an_entity_is_added() {
    let mut app = sim_app(5);
    let before = hash(&mut app);
    let world = app.world_mut();
    let id = world
        .resource_mut::<SimIdAllocator>()
        .allocate()
        .expect("first id");
    world.spawn(id);
    assert_ne!(hash(&mut app), before);
}

// --- Tick stage order (PLAN.md section 4, invariant I4) -------------------------------------

#[derive(Resource, Default)]
struct StageLog(Vec<SimSet>);

#[test]
fn stages_run_once_per_tick_in_plan_order() {
    let mut app = sim_app(0);
    app.init_resource::<StageLog>();
    // Registered in reverse to show that order comes from the set chain, not registration.
    for set in SimSet::ORDER.into_iter().rev() {
        app.add_systems(
            SimSchedule,
            (move |mut log: ResMut<StageLog>| log.0.push(set))
                .in_set(set)
                .after(sim::SimCoreSet(set)),
        );
    }

    run_ticks(&mut app, 2);

    let expected: Vec<SimSet> = SimSet::ORDER
        .iter()
        .chain(&SimSet::ORDER)
        .copied()
        .collect();
    assert_eq!(app.world().resource::<StageLog>().0, expected);
}

#[test]
#[should_panic(
    expected = "systems with conflicting data access have indeterminate execution order"
)]
fn conflicting_unordered_systems_are_a_schedule_build_error() {
    let mut app = sim_app(0);
    app.init_resource::<StageLog>();
    // Two writers of the same resource in the same stage with no order between them.
    app.add_systems(
        SimSchedule,
        (
            |mut log: ResMut<StageLog>| log.0.push(SimSet::Movement),
            |mut log: ResMut<StageLog>| log.0.push(SimSet::Movement),
        )
            .in_set(SimSet::Movement),
    );
    run_ticks(&mut app, 1);
}
