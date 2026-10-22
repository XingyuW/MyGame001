//! Adjustable game speed. The simulation is fixed-step and every gameplay timer (movement,
//! combat cooldowns, projectiles, construction and unit production, harvesting, AI decisions,
//! strategic charges) is counted in simulation ticks, so the speed changes how many ticks run per
//! real second: the `FixedUpdate` timestep becomes `1 / (TICKS_PER_SECOND × factor)`. Nothing in
//! the simulation changes, so determinism, pathfinding budgets per tick and the human-only
//! Super Me progress bonus are unaffected; it multiplies with the speed like any other per-tick
//! rate.
//!
//! The interface (camera panning, menus, sounds) keeps real time. Presentation animations that
//! show gameplay (infantry walk cycles, explosion and smoke particles) follow [`GameClock`], which
//! advances at the game speed and stops while paused. It is a resource of its own so that the
//! animation systems never share state with the sidebar, which changes the speed.
//!
//! | Input | Effect |
//! |-------|--------|
//! | Skirmish menu, "Game speed" | the speed the match starts at |
//! | `]` / sidebar `+` | next faster speed |
//! | `[` / sidebar `−` | next slower speed |

use bevy::prelude::*;
use sim::TICKS_PER_SECOND;

use crate::sim_host::{CommandSink, SimHost};

/// The selectable speeds as fractions of the normal 20 ticks per second.
pub const SPEED_LEVELS: [f32; 6] = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0];

/// Index of 1× in [`SPEED_LEVELS`].
pub const NORMAL_SPEED: usize = 1;

pub struct GameSpeedPlugin;

impl Plugin for GameSpeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameSpeed>()
            .init_resource::<GameClock>()
            .add_systems(PreUpdate, (apply_game_speed, advance_clock).chain());
    }
}

/// The chosen game speed.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct GameSpeed {
    level: usize,
}

impl Default for GameSpeed {
    fn default() -> Self {
        Self {
            level: NORMAL_SPEED,
        }
    }
}

/// The game-time clock of presentation animations.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct GameClock {
    /// Seconds of game time shown so far: real seconds × factor, frozen while paused or between
    /// matches.
    clock: f32,
    /// Real seconds × factor of the last frame (0 while paused).
    delta: f32,
}

impl GameClock {
    #[cfg(test)]
    pub(crate) fn privacy_frame(seconds: f32) -> Self {
        Self {
            clock: seconds,
            delta: seconds,
        }
    }
    /// Game seconds elapsed (for animations that show gameplay).
    pub fn clock(&self) -> f32 {
        self.clock
    }

    /// Game seconds that passed in the last frame (0 while paused).
    pub fn delta(&self) -> f32 {
        self.delta
    }
}

impl GameSpeed {
    /// Speed multiplier, e.g. 1.5.
    pub fn factor(&self) -> f32 {
        SPEED_LEVELS[self.level.min(SPEED_LEVELS.len() - 1)]
    }

    /// Selects `level` (clamped to the available levels); the match setup menu does.
    pub fn set_level(&mut self, level: usize) {
        self.level = level.min(SPEED_LEVELS.len() - 1);
    }

    /// Moves one level faster (`up`) or slower; returns whether the speed changed.
    pub fn step(&mut self, up: bool) -> bool {
        let next = if up {
            (self.level + 1).min(SPEED_LEVELS.len() - 1)
        } else {
            self.level.saturating_sub(1)
        };
        let changed = next != self.level;
        self.level = next;
        changed
    }

    /// Simulation ticks per real second at this speed.
    pub fn ticks_per_second(&self) -> f64 {
        f64::from(TICKS_PER_SECOND) * f64::from(self.factor())
    }

    /// The label shown to the player, e.g. `1.5x`.
    pub fn label(&self) -> String {
        speed_label(self.factor())
    }
}

/// `0.5x`, `1x`, `1.5x`, ... (the interface font has no multiplication sign).
pub fn speed_label(factor: f32) -> String {
    if factor.fract() == 0.0 {
        format!("{factor:.0}x")
    } else {
        format!("{factor}x")
    }
}

/// Sets the fixed timestep whenever the speed changes, so each tick still runs exactly once per
/// timestep and the rendered interpolation (`overstep_fraction`) stays smooth.
pub(crate) fn apply_game_speed(speed: Res<GameSpeed>, mut time: ResMut<Time<Fixed>>) {
    if speed.is_changed() {
        time.set_timestep_hz(speed.ticks_per_second());
    }
}

fn advance_clock(
    speed: Res<GameSpeed>,
    mut clock: ResMut<GameClock>,
    time: Res<Time>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
) {
    let delta = if sink.paused() || !host.running() {
        0.0
    } else {
        time.delta_secs() * speed.factor()
    };
    clock.delta = delta;
    clock.clock += delta;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn levels_labels_and_steps() {
        let mut speed = GameSpeed::default();
        assert_eq!(speed.factor(), 1.0);
        assert_eq!(speed.label(), "1x");
        assert!(!{
            let mut s = speed.clone();
            s.set_level(0);
            s.step(false)
        });
        let labels: Vec<String> = SPEED_LEVELS.iter().map(|&f| speed_label(f)).collect();
        assert_eq!(labels, ["0.5x", "1x", "1.5x", "2x", "3x", "4x"]);
        for expected in &SPEED_LEVELS[2..] {
            assert!(speed.step(true));
            assert_eq!(speed.factor(), *expected);
        }
        assert!(!speed.step(true), "4× is the maximum");
        speed.set_level(99);
        assert_eq!(speed.factor(), 4.0);
        assert_eq!(speed.ticks_per_second(), 80.0);
    }

    #[test]
    fn every_speed_runs_the_simulation_that_many_times_faster_in_real_time() {
        for (level, factor) in SPEED_LEVELS.iter().enumerate() {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                crate::sim_host::SimHostPlugin,
                GameSpeedPlugin,
            ));
            app.insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_secs_f64(1.0 / 60.0),
                ));
            app.world_mut()
                .insert_resource(crate::sim_host::tests::demo_host());
            app.world_mut().resource_mut::<GameSpeed>().set_level(level);
            app.update();
            let start = app.world().resource::<SimHost>().tick();
            let clock = app.world().resource::<GameClock>().clock();
            // Two real seconds of 60 Hz frames.
            for _ in 0..120 {
                app.update();
            }
            let ticks = app.world().resource::<SimHost>().tick() - start;
            let expected = 40.0 * factor;
            assert!(
                (ticks as f32 - expected).abs() <= 1.0,
                "{factor}×: {ticks} ticks in 2 s, expected {expected}"
            );
            let game_seconds = app.world().resource::<GameClock>().clock() - clock;
            assert!((game_seconds - 2.0 * factor).abs() < 0.1, "{game_seconds}");
        }
    }

    #[test]
    fn the_game_clock_stops_while_paused() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            crate::sim_host::SimHostPlugin,
            GameSpeedPlugin,
        ));
        app.world_mut()
            .insert_resource(crate::sim_host::tests::demo_host());
        app.world_mut().resource_mut::<GameSpeed>().set_level(5);
        app.world_mut().resource_mut::<CommandSink>().toggle_pause();
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
            app.world_mut().run_system_once(advance_clock).unwrap();
        }
        assert_eq!(app.world().resource::<GameClock>().clock(), 0.0);
        assert_eq!(app.world().resource::<GameClock>().delta(), 0.0);
    }
}
