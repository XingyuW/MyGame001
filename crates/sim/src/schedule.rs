use bevy_app::{App, Plugin};
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{
    IntoScheduleConfigs, LogLevel, Schedule, ScheduleBuildSettings, ScheduleLabel, SystemSet,
};
use bevy_ecs::system::ResMut;

use crate::ai::{AiPlayers, run_ai};
use crate::combat::{Projectiles, combat, remove_dead};
use crate::command::{CommandQueue, process_commands};
use crate::definitions::Definitions;
use crate::economy::{Players, advance_production, update_power};
use crate::event::{SimEvent, advance_events};
use crate::harvest::harvest;
use crate::id::SimIdAllocator;
use crate::map::Grid;
use crate::match_setup::{MatchState, check_win_condition};
use crate::movement::move_units;
use crate::pathfinding::find_paths;
use crate::rng::SimRng;
use crate::visibility::update_visibility;

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
    /// The computer players' decisions (M6).
    Ai,
    /// Power, production, and the harvester cycle (M3).
    Economy,
    Pathfinding,
    Movement,
    /// Projectiles, target acquisition, firing, and damage (M4).
    Combat,
    /// Removing destroyed units and buildings (M4).
    Cleanup,
    /// Recomputing every player's visibility grid (M4).
    Visibility,
    /// Defeated players and the end of the match (M6).
    WinCondition,
    /// Bookkeeping that closes the tick; advances [`SimTick`].
    EndOfTick,
}

/// Core work inside a stage. Instrumentation and extensions can run after this set without
/// racing an exclusive specialist system; the outer SimSet still fixes stage order.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimCoreSet(pub SimSet);

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
            (advance_events, process_commands)
                .chain()
                .in_set(SimCoreSet(SimSet::Commands))
                .in_set(SimSet::Commands),
            run_ai.in_set(SimCoreSet(SimSet::Ai)).in_set(SimSet::Ai),
            (
                update_power,
                advance_production,
                crate::economy::repair_buildings,
                harvest,
                crate::special::advance,
            )
                .chain()
                .in_set(SimCoreSet(SimSet::Economy))
                .in_set(SimSet::Economy),
            find_paths
                .in_set(SimCoreSet(SimSet::Pathfinding))
                .in_set(SimSet::Pathfinding),
            move_units
                .in_set(SimCoreSet(SimSet::Movement))
                .in_set(SimSet::Movement),
            (combat, crate::special::resolve)
                .chain()
                .in_set(SimCoreSet(SimSet::Combat))
                .in_set(SimSet::Combat),
            (
                remove_dead,
                crate::props::remove_dead_props,
                crate::combat::update_conditions,
            )
                .chain()
                .in_set(SimCoreSet(SimSet::Cleanup))
                .in_set(SimSet::Cleanup),
            (update_visibility, crate::special::reconnaissance)
                .chain()
                .in_set(SimCoreSet(SimSet::Visibility))
                .in_set(SimSet::Visibility),
            check_win_condition
                .in_set(SimCoreSet(SimSet::WinCondition))
                .in_set(SimSet::WinCondition),
            advance_tick
                .in_set(SimCoreSet(SimSet::EndOfTick))
                .in_set(SimSet::EndOfTick),
        ));

        // The event buffer is advanced by the simulation itself, once per tick (`event.rs`), so
        // it is inserted as a plain resource rather than registered for Bevy's per-frame update.
        app.insert_resource(SimTick::default())
            .insert_resource(CommandQueue::default())
            .insert_resource(SimIdAllocator::default())
            .insert_resource(SimRng::new(self.seed))
            .insert_resource(Grid::default())
            .insert_resource(Definitions::default())
            .insert_resource(Players::default())
            .insert_resource(crate::SpecialState::default())
            .insert_resource(Projectiles::default())
            .insert_resource(crate::Props::default())
            .insert_resource(MatchState::default())
            .insert_resource(AiPlayers::default())
            .insert_resource(Messages::<SimEvent>::default())
            .add_schedule(schedule);
    }
}

/// Invariant I12: the counter wraps at `u64::MAX` in debug and release builds alike (about
/// 2.9 × 10^10 years at 20 Hz; M1 review 2, P2-D).
fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 = tick.0.wrapping_add(1);
}

#[cfg(test)]
mod tests {
    use bevy_app::App;

    use super::*;

    #[test]
    fn tick_counter_wraps_instead_of_overflowing() {
        // Regression (M1 review 2, P2-D): `+= 1` panicked in debug builds and wrapped in
        // release builds.
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 0 });
        app.world_mut().insert_resource(SimTick(u64::MAX - 1));
        app.world_mut().run_schedule(SimSchedule);
        assert_eq!(app.world().resource::<SimTick>().0, u64::MAX);
        app.world_mut().run_schedule(SimSchedule);
        assert_eq!(app.world().resource::<SimTick>().0, 0);
    }
}
