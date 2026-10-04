//! The simulation host: the boundary between the game and the simulation (PLAN.md section 4,
//! invariant I5; section 7, M5: "the interface cannot obtain mutable access to simulation
//! state"; docs/DECISIONS.md D34).
//!
//! The simulation runs in a `World` of its own, held in a private field of [`SimHost`]. Every
//! other module of the game, and so the whole interface, reaches it only through
//! [`SimHost::world`], which returns a shared reference: Bevy gives read-only query items and
//! resources for a `&World`, and the game crate forbids `unsafe` code, so nothing outside this
//! module can obtain mutable access to simulation state. `SimHost` has no public constructor and
//! no public method that takes `&mut self`, so even `ResMut<SimHost>` grants nothing but reads.
//!
//! The only writes into the simulation world are in this module:
//!
//! * match setup before the first tick ([`start_match`]: the map's grid, the definitions, the
//!   players, and the starting buildings and units, D11, D20, D27);
//! * one tick per fixed timestep ([`run_tick`]), into whose `CommandQueue` the commands of the
//!   interface are moved first. The interface pushes commands into [`CommandSink`], which accepts
//!   commands of the human player only, so the interface changes simulation state only through
//!   `CommandQueue` (I5).
//!
//! After every tick, the tick's `SimEvent`s are copied into the game world's message buffer, which
//! Bevy updates once per frame, so the game's readers see every event even when several ticks run
//! in one frame (M2 implementation debt item 2).

use std::fmt;

use bevy::prelude::*;
use sim::{
    Armour, Building, Command, CommandQueue, Definitions, Facing, Grid, Harvester, Health,
    IssuedCommand, Mobile, Orders, Owner, Players, Position, Producer, Projectiles, Sight,
    SimEvent, SimId, SimPlugin, SimSchedule, SimTick, Unit, Weapon,
};

use crate::buildings::{BuildingStatus, setup_economy};
use crate::units::{HUMAN, STARTING_UNITS, UnitStatus, plan_starting_units};

/// Most commands the sink holds between two ticks. The interface issues a few commands per input
/// event, and the sink is emptied at every tick (20 per second), so only a fault could reach it;
/// a command beyond it is refused and logged, never dropped silently.
pub const MAX_PENDING_COMMANDS: usize = 256;

pub struct SimHostPlugin {
    /// Seed of the simulation's random number generator.
    pub seed: u64,
}

impl Plugin for SimHostPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SimHost::new(self.seed))
            .init_resource::<CommandSink>()
            .add_message::<SimEvent>()
            .add_systems(Startup, start_match.after(crate::units::load_match_assets))
            .add_systems(
                FixedUpdate,
                (crate::units::record_previous_positions, run_tick).chain(),
            );
    }
}

/// The simulation: its world, readable by everyone and writable only by this module.
#[derive(Resource)]
pub struct SimHost {
    world: World,
}

impl SimHost {
    /// A simulation with the resources and schedule of `SimPlugin` and no match set up.
    fn new(seed: u64) -> Self {
        let mut app = App::empty();
        app.add_plugins(SimPlugin { seed });
        let mut world = std::mem::take(app.world_mut());
        // Every component of the simulation is registered, so that a read-only query for any of
        // them can be built from a `&World` (`World::try_query`) before the first entity exists.
        world.register_component::<SimId>();
        world.register_component::<Owner>();
        world.register_component::<Unit>();
        world.register_component::<Mobile>();
        world.register_component::<Position>();
        world.register_component::<Facing>();
        world.register_component::<Orders>();
        world.register_component::<Building>();
        world.register_component::<Producer>();
        world.register_component::<Harvester>();
        world.register_component::<Health>();
        world.register_component::<Armour>();
        world.register_component::<Sight>();
        world.register_component::<Weapon>();
        Self { world }
    }

    /// The simulation world, read-only.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Completed ticks.
    pub fn tick(&self) -> u64 {
        self.world
            .get_resource::<SimTick>()
            .map_or(0, |tick| tick.0)
    }

    pub fn players(&self) -> Option<&Players> {
        self.world.get_resource::<Players>()
    }

    pub fn grid(&self) -> Option<&Grid> {
        self.world.get_resource::<Grid>()
    }

    pub fn definitions(&self) -> Option<&Definitions> {
        self.world.get_resource::<Definitions>()
    }

    pub fn projectiles(&self) -> Option<&Projectiles> {
        self.world.get_resource::<Projectiles>()
    }

    /// Installs a match: the map's grid, the definitions, and the starting players, buildings,
    /// and units (match setup before the first tick, D11).
    fn install(&mut self, grid: Grid, definitions: Definitions) -> Result<MatchStart, String> {
        self.world.insert_resource(grid);
        self.world.insert_resource(definitions);
        let buildings = setup_economy(&mut self.world)?;
        let planned = {
            let grid = self
                .world
                .get_resource::<Grid>()
                .ok_or_else(|| "the simulation has no grid".to_string())?;
            plan_starting_units(grid, &STARTING_UNITS).map_err(|error| error.to_string())?
        };
        let mut units = 0;
        for (owner, unit_type, cell) in planned {
            sim::spawn_unit(&mut self.world, owner, unit_type, cell).map_err(|error| {
                format!(
                    "starting unit {unit_type:?} at ({}, {}): {error}",
                    cell.x, cell.y
                )
            })?;
            units += 1;
        }
        Ok(MatchStart { buildings, units })
    }

    /// Runs one tick after moving `commands` (the human player's) into the simulation's command
    /// queue; returns the tick's events.
    fn step(&mut self, commands: Vec<Command>) -> Result<Vec<SimEvent>, String> {
        {
            let mut queue = self
                .world
                .get_resource_mut::<CommandQueue>()
                .ok_or_else(|| "the simulation has no command queue".to_string())?;
            for command in commands {
                queue.push(IssuedCommand {
                    player: HUMAN,
                    command,
                });
            }
        }
        self.world.run_schedule(SimSchedule);
        let events = self
            .world
            .get_resource::<Messages<SimEvent>>()
            .ok_or_else(|| "the simulation has no event buffer".to_string())?;
        Ok(events.iter_current_update_messages().cloned().collect())
    }
}

/// What match setup spawned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchStart {
    pub buildings: usize,
    pub units: usize,
}

/// The grid of the loaded map, waiting for match setup (inserted by `crate::map::load_map`).
#[derive(Resource, Debug)]
pub struct MatchGrid(pub Grid);

/// The loaded definitions, waiting for match setup (inserted by
/// `crate::units::load_match_assets`).
#[derive(Resource, Debug)]
pub struct MatchDefinitions(pub Definitions);

/// Why the sink did not accept a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRefused {
    /// The game is paused; no command is accepted until it resumes.
    Paused,
    /// [`MAX_PENDING_COMMANDS`] commands already wait for the next tick.
    Full,
}

impl fmt::Display for CommandRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paused => write!(f, "the game is paused"),
            Self::Full => write!(
                f,
                "{MAX_PENDING_COMMANDS} commands already wait for the next tick"
            ),
        }
    }
}

/// The human player's commands for the next tick, and whether the game is paused.
///
/// While paused, no tick runs and no command is accepted, so nothing accumulates (D39).
#[derive(Resource, Debug, Default)]
pub struct CommandSink {
    pending: Vec<Command>,
    paused: bool,
    /// Commands accepted so far (for the interface's acknowledgement sound).
    accepted: u64,
    /// Commands refused so far (for the interface's error sound).
    refused: u64,
}

impl CommandSink {
    /// Accepts `command` for the next tick.
    pub fn push(&mut self, command: Command) -> Result<(), CommandRefused> {
        if self.paused {
            self.refused = self.refused.saturating_add(1);
            return Err(CommandRefused::Paused);
        }
        if self.pending.len() >= MAX_PENDING_COMMANDS {
            self.refused = self.refused.saturating_add(1);
            return Err(CommandRefused::Full);
        }
        self.pending.push(command);
        self.accepted = self.accepted.saturating_add(1);
        Ok(())
    }

    /// Pushes every command, logging each one that is refused; returns how many were accepted.
    pub fn issue(&mut self, commands: impl IntoIterator<Item = Command>) -> usize {
        let mut accepted = 0;
        for command in commands {
            match self.push(command.clone()) {
                Ok(()) => accepted += 1,
                Err(reason) => warn!("command not issued: {reason}: {command:?}"),
            }
        }
        accepted
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Pauses or resumes the game; returns whether it is now paused.
    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        self.paused
    }

    /// Commands accepted so far.
    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    /// Commands refused so far.
    pub fn refused(&self) -> u64 {
        self.refused
    }

    fn take(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.pending)
    }
}

/// Match setup: installs the loaded map and definitions into the simulation and spawns the
/// starting players, buildings, and units. Does nothing if the map or the data failed to load
/// (they have already ended the game with exit code 1).
pub(crate) fn start_match(world: &mut World) {
    let (Some(MatchGrid(grid)), Some(MatchDefinitions(definitions))) = (
        world.remove_resource::<MatchGrid>(),
        world.remove_resource::<MatchDefinitions>(),
    ) else {
        return;
    };
    let result = world.resource_scope(|_, mut host: Mut<SimHost>| host.install(grid, definitions));
    match result {
        Ok(start) => {
            info!("{} buildings spawned", start.buildings);
            info!("{} units spawned", start.units);
            world.resource_mut::<BuildingStatus>().spawned = start.buildings;
            world.resource_mut::<UnitStatus>().spawned = start.units;
        }
        Err(error) => {
            error!("{error}");
            world.write_message(AppExit::error());
        }
    }
}

/// Advances the simulation by exactly one tick per fixed timestep (20 Hz), unless paused.
fn run_tick(
    mut host: ResMut<SimHost>,
    mut sink: ResMut<CommandSink>,
    mut events: MessageWriter<SimEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    if sink.paused {
        return;
    }
    let commands = sink.take();
    match host.step(commands) {
        Ok(tick_events) => {
            events.write_batch(tick_events);
        }
        Err(error) => {
            error!("{error}");
            exit.write(AppExit::error());
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use sim::{CellPos, PlayerId, ProductionItem};

    use super::*;

    fn assets_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// A host with the demo match installed, as `start_match` installs it.
    pub(crate) fn demo_host() -> SimHost {
        let grid = sim::load_map(assets_root().join(crate::map::MAP_PATH))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        let (loaded, _) =
            crate::units::load_match_data(&assets_root()).unwrap_or_else(|error| panic!("{error}"));
        let mut host = SimHost::new(0);
        let start = host
            .install(grid, loaded.definitions)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            start,
            MatchStart {
                buildings: crate::buildings::STARTING_BUILDINGS.len(),
                units: crate::units::starting_unit_total(),
            }
        );
        host
    }

    #[test]
    fn the_sink_accepts_the_human_players_commands_until_paused_or_full() {
        let mut sink = CommandSink::default();
        let stop = Command::Stop {
            units: vec![SimId(1)],
        };
        assert_eq!(sink.push(stop.clone()), Ok(()));
        assert!(sink.toggle_pause());
        assert_eq!(sink.push(stop.clone()), Err(CommandRefused::Paused));
        assert_eq!(sink.issue([stop.clone(), stop.clone()]), 0);
        assert_eq!(sink.pending, std::slice::from_ref(&stop));
        assert_eq!(sink.refused(), 3);
        assert!(!sink.toggle_pause());
        // Exactly MAX_PENDING_COMMANDS fit; the next is refused, not dropped silently.
        assert_eq!(
            sink.issue(vec![stop.clone(); MAX_PENDING_COMMANDS]),
            MAX_PENDING_COMMANDS - 1
        );
        assert_eq!(sink.pending.len(), MAX_PENDING_COMMANDS);
        assert_eq!(sink.push(stop.clone()), Err(CommandRefused::Full));
        assert_eq!(sink.take().len(), MAX_PENDING_COMMANDS);
        assert_eq!(sink.push(stop), Ok(()));
    }

    #[test]
    fn a_tick_applies_the_sinks_commands_as_the_human_players_and_returns_its_events() {
        let mut host = demo_host();
        let definitions = host.definitions().expect("definitions").clone();
        let power = definitions
            .building_kind("power_plant")
            .expect("power plant");
        // A command for the human player, and one the opponent could not have issued through
        // the sink: the sink has no way to name another player.
        let events = host
            .step(vec![Command::QueueProduction {
                item: ProductionItem::Building(power),
            }])
            .expect("tick");
        assert_eq!(host.tick(), 1);
        assert!(events.is_empty(), "{events:?}");
        let players = host.players().expect("players");
        assert_eq!(
            players.get(HUMAN).map(|state| state.queues().count()),
            Some(1)
        );
        assert_eq!(
            players.get(PlayerId(1)).map(|state| state.queues().count()),
            Some(0)
        );
        // A rejected command comes back as the human player's `CommandRejected`.
        let events = host
            .step(vec![Command::Move {
                units: vec![SimId(1)],
                target: CellPos::new(-1, 0),
            }])
            .expect("tick");
        assert!(matches!(
            events.as_slice(),
            [SimEvent::CommandRejected { player, .. }] if *player == HUMAN
        ));
    }

    #[test]
    fn the_game_world_holds_no_simulation_state() {
        // The boundary of D34: the interface's systems run in the game world, which contains no
        // simulation resource; the simulation's world is reachable only through `SimHost`, whose
        // `world()` is a shared reference.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimHostPlugin { seed: 0 }));
        let world = app.world();
        assert!(world.contains_resource::<SimHost>());
        assert!(!world.contains_resource::<Players>());
        assert!(!world.contains_resource::<Grid>());
        assert!(!world.contains_resource::<Definitions>());
        assert!(!world.contains_resource::<CommandQueue>());
        assert!(!world.contains_resource::<SimTick>());
        assert!(!world.contains_resource::<Projectiles>());
        let host = world.resource::<SimHost>();
        assert!(host.world().contains_resource::<Players>());
        assert!(host.world().contains_resource::<CommandQueue>());
        // A query that names a component mutably still yields read-only items from `&World`
        // (`QueryState::iter` takes `&World` and returns `D::ReadOnly` items).
        let mut query = host.world().try_query::<&mut Health>().expect("registered");
        assert_eq!(query.iter(host.world()).count(), 0);
    }

    #[test]
    fn the_tick_system_does_not_tick_while_paused_and_copies_the_events() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimHostPlugin { seed: 0 }));
        app.world_mut().insert_resource(demo_host());
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::Stop {
                units: vec![SimId(u32::MAX)],
            })
            .expect("accepted");
        app.world_mut().run_system_once(run_tick).expect("runs");
        assert_eq!(app.world().resource::<SimHost>().tick(), 1);
        let messages = app.world().resource::<Messages<SimEvent>>();
        assert_eq!(messages.len(), 1, "the rejection of the stop command");
        app.world_mut().resource_mut::<CommandSink>().toggle_pause();
        for _ in 0..5 {
            app.world_mut().run_system_once(run_tick).expect("runs");
        }
        assert_eq!(app.world().resource::<SimHost>().tick(), 1);
    }

    #[test]
    fn match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error() {
        let setup_app = |grid: Grid| {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, SimHostPlugin { seed: 0 }))
                .init_resource::<BuildingStatus>()
                .init_resource::<UnitStatus>();
            let (loaded, _) = crate::units::load_match_data(&assets_root())
                .unwrap_or_else(|error| panic!("{error}"));
            app.world_mut().insert_resource(MatchGrid(grid));
            app.world_mut()
                .insert_resource(MatchDefinitions(loaded.definitions));
            app.world_mut().run_system_once(start_match).expect("runs");
            app
        };
        // The demo map holds the demo match.
        let demo = sim::load_map(assets_root().join(crate::map::MAP_PATH))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        let app = setup_app(demo);
        assert_eq!(app.should_exit(), None);
        assert_eq!(
            app.world().resource::<UnitStatus>().spawned,
            crate::units::starting_unit_total()
        );
        assert_eq!(
            app.world().resource::<BuildingStatus>().spawned,
            crate::buildings::STARTING_BUILDINGS.len()
        );
        assert!(!app.world().contains_resource::<MatchGrid>());
        // A 16x16 map cannot hold the starting buildings: the game ends with exit code 1 and
        // nothing is reported as spawned.
        let small = sim::load_map(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../sim/tests/fixtures/maps/open_16x16.tmx"),
        )
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
        let app = setup_app(small);
        assert_eq!(app.should_exit(), Some(AppExit::error()));
        assert_eq!(app.world().resource::<UnitStatus>().spawned, 0);
        assert_eq!(app.world().resource::<BuildingStatus>().spawned, 0);
    }

    /// A host on the 32x32 economy fixture map with the combat fixture data, set up by `setup`
    /// (scenario setup for tests of the interface's reading of the simulation).
    pub(crate) fn fixture_host(setup: impl FnOnce(&mut World)) -> SimHost {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim/tests/fixtures");
        let data = root.join("data");
        let mut host = SimHost::new(0);
        host.world.insert_resource(
            sim::load_map(root.join("maps/economy_32x32.tmx"))
                .unwrap_or_else(|error| panic!("{error}"))
                .grid,
        );
        host.world.insert_resource(
            sim::load_match_definitions(
                data.join("units_combat.ron"),
                data.join("economy_combat.ron"),
                data.join("combat.ron"),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        );
        setup(&mut host.world);
        host
    }

    /// Runs one tick with the human player's `commands`.
    pub(crate) fn run(host: &mut SimHost, commands: Vec<Command>) -> Vec<SimEvent> {
        host.step(commands)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    use bevy::ecs::system::RunSystemOnce;
}
