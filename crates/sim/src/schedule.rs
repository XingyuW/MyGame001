use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{
    IntoScheduleConfigs, LogLevel, Schedule, ScheduleBuildSettings, ScheduleLabel, SystemSet,
};
use bevy_ecs::system::ResMut;

use crate::command::{CommandQueue, process_commands};
use crate::id::SimIdAllocator;
use crate::rng::SimRng;

/// Simulation rate. One run of [`SimSchedule`] advances the simulation by one tick (50 ms).
pub const TICKS_PER_SECOND: u32 = 20;

/// The schedule in which one run equals exactly one simulation tick.
///
/// The game runs it from Bevy's fixed timestep; tests run it directly with
/// `world.run_schedule(SimSchedule)`, so they never depend on wall clock time.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimSchedule;

/// The stages of one tick, executed strictly in declaration order (PLAN.md section 4).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Validate and apply the commands in [`CommandQueue`].
    Commands,
    Ai,
    Economy,
    Pathfinding,
    Movement,
    Combat,
    Cleanup,
    Visibility,
    WinCondition,
    /// Bookkeeping that closes the tick; advances [`SimTick`].
    EndOfTick,
}

impl SimSet {
    /// All stages in execution order.
    pub const ORDER: [SimSet; 10] = [
        SimSet::Commands,
        SimSet::Ai,
        SimSet::Economy,
        SimSet::Pathfinding,
        SimSet::Movement,
        SimSet::Combat,
        SimSet::Cleanup,
        SimSet::Visibility,
        SimSet::WinCondition,
        SimSet::EndOfTick,
    ];
}

/// Number of completed simulation ticks.
///
/// While a tick is running, systems observe the index of the tick in progress (starting at 0);
/// the counter is incremented as the last step of the tick.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimTick(pub u64);

/// Installs the simulation resources and [`SimSchedule`].
#[derive(Clone, Copy, Debug)]
pub struct SimPlugin {
    /// Seed for [`SimRng`], the only source of randomness in the simulation.
    pub seed: u64,
}

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let mut schedule = Schedule::new(SimSchedule);
        // Invariant I4: two systems with conflicting access and no defined order are a build
        // error, so execution order inside a tick can never depend on the executor.
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::new()
        });
        // Must list the same sets as `SimSet::ORDER`; `stages_run_once_per_tick_in_plan_order`
        // checks that they agree.
        schedule.configure_sets(
            (
                SimSet::Commands,
                SimSet::Ai,
                SimSet::Economy,
                SimSet::Pathfinding,
                SimSet::Movement,
                SimSet::Combat,
                SimSet::Cleanup,
                SimSet::Visibility,
                SimSet::WinCondition,
                SimSet::EndOfTick,
            )
                .chain(),
        );
        schedule.add_systems((
            process_commands.in_set(SimSet::Commands),
            advance_tick.in_set(SimSet::EndOfTick),
        ));

        app.insert_resource(SimTick::default())
            .insert_resource(CommandQueue::default())
            .insert_resource(SimIdAllocator::default())
            .insert_resource(SimRng::new(self.seed))
            .add_schedule(schedule);
    }
}

fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}
