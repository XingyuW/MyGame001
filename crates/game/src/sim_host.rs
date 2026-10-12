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
//! * match setup before the first tick ([`install_match`]: a new simulation world with the map's
//!   grid and the definitions, set up by `sim::setup_match` from the skirmish file with the
//!   players, their starting buildings and units, and the computer player, D11, D40);
//! * one tick per fixed timestep ([`run_tick`]) while a match is running, into whose `CommandQueue`
//!   the commands of the interface are moved first. The interface pushes commands into
//!   [`CommandSink`], which accepts commands of the human player only, so the interface changes
//!   simulation state only through `CommandQueue` (I5). The computer player pushes its own
//!   commands inside the simulation (`sim`'s `Ai` stage, M6).
//!
//! After every tick, the tick's `SimEvent`s are copied into the game world's message buffer, which
//! Bevy updates once per frame, so the game's readers see every event even when several ticks run
//! in one frame (M2 implementation debt item 2).

use std::fmt;

use bevy::prelude::*;
use sim::{
    AiDefinitions, AiPlayers, Armour, Building, Command, CommandQueue, Definitions, Facing, Grid,
    Harvester, Health, IssuedCommand, MatchOutcome, MatchStart, MatchState, Mobile, Orders, Owner,
    PlayerSetup, Players, Position, Producer, Projectiles, Sight, SimEvent, SimId, SimPlugin,
    SimSchedule, SimTick, SkirmishDefinitions, Unit, Weapon,
};

use crate::units::HUMAN;

/// Most commands the sink holds between two ticks. The interface issues a few commands per input
/// event, and the sink is emptied at every tick (20 per second), so only a fault could reach it;
/// a command beyond it is refused and logged, never dropped silently.
pub const MAX_PENDING_COMMANDS: usize = 256;

pub struct SimHostPlugin;

impl Plugin for SimHostPlugin {
    fn build(&self, app: &mut App) {
        // No match until one is started from the menu (`crate::skirmish`).
        app.insert_resource(SimHost::new(0))
            .init_resource::<CommandSink>()
            .add_message::<SimEvent>()
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
    /// A simulation with the resources and schedule of `SimPlugin`, seeded with `seed`, and no
    /// match set up.
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

    /// The destructible scenery still standing.
    pub fn props(&self) -> Option<&sim::Props> {
        self.world.get_resource::<sim::Props>()
    }

    pub fn projectiles(&self) -> Option<&Projectiles> {
        self.world.get_resource::<Projectiles>()
    }

    /// The match's state: its players, who has been defeated, and its outcome (M6).
    pub fn match_state(&self) -> Option<&MatchState> {
        self.world.get_resource::<MatchState>()
    }

    /// The outcome of the match, once it has ended.
    pub fn outcome(&self) -> Option<MatchOutcome> {
        self.match_state().and_then(MatchState::outcome)
    }

    /// The computer players.
    pub fn ai(&self) -> Option<&AiPlayers> {
        self.world.get_resource::<AiPlayers>()
    }

    /// Whether the human player has been defeated. With several computer players the
    /// simulation's outcome waits for the last one standing, but for the human the match is over.
    pub fn human_defeated(&self) -> bool {
        self.match_state()
            .is_some_and(|state| state.defeated(HUMAN).is_some())
    }

    /// Whether a match is set up and has not ended for the human player: only then do ticks run.
    pub fn running(&self) -> bool {
        self.match_state().is_some_and(|state| {
            !state.players().is_empty() && state.outcome().is_none() && !self.human_defeated()
        })
    }

    /// Installs a match into this simulation, which has none yet (match setup before the first
    /// tick, D11, D40).
    fn install(&mut self, plan: &MatchPlan) -> Result<MatchStart, String> {
        self.world.insert_resource(plan.grid.clone());
        self.world.insert_resource(plan.definitions.clone());
        // Trees, houses and city blocks take damage like structures.
        if let Some(armour) = plan.definitions.armour_class("structure") {
            self.world
                .insert_resource(sim::Props::from_looks(&plan.grid, &plan.looks, armour));
        }
        let human = |on: bool| if on { vec![HUMAN] } else { Vec::new() };
        sim::setup_match_configured(
            &mut self.world,
            &plan.skirmish,
            &plan.ai,
            plan.map,
            &plan.players,
            &sim::MatchOptions {
                super_players: human(plan.super_me),
                unlock_players: human(plan.unlock_all),
                unit_speed_percent: plan.unit_speed_percent,
                // The computer players fight the human, never each other (the default; the
                // other options are those of Red Alert 2's lobby until the menu offers them).
                free_for_all: false,
                mcv_start: plan.mcv_start,
                ..sim::MatchOptions::default()
            },
        )
        .map_err(|error| error.to_string())
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

/// Everything a match is set up from (`sim::setup_match`). The game makes it from the menu's
/// choices with `crate::skirmish::match_plan`, which enlarges the map for the match's players.
#[derive(Clone, Debug)]
pub struct MatchPlan {
    pub super_me: bool,
    /// Building prerequisites waived for the human player.
    pub unlock_all: bool,
    /// The speed of the human player's units with Super Me, in percent (100: unchanged).
    pub unit_speed_percent: u16,
    /// Whether a country's player starts with the base unit of its side (the lobby's default,
    /// RA1) or with the Construction Yard it deploys into (`sim::MatchOptions::mcv_start`).
    pub mcv_start: bool,
    /// The terrain of the match's map: the map file's, or enlarged for more than two players.
    pub grid: Grid,
    /// The map's cell looks (row-major): where its destructible scenery stands (`sim::Props`).
    pub looks: Vec<u8>,
    pub definitions: Definitions,
    /// The skirmish file; the start positions of map `map` are those on `grid`.
    pub skirmish: SkirmishDefinitions,
    pub ai: AiDefinitions,
    /// The map's index in `skirmish.maps`.
    pub map: usize,
    pub players: Vec<PlayerSetup>,
    /// Seed of the simulation's random number generator.
    pub seed: u64,
}

/// Replaces the simulation with a new one in which `plan`'s match is set up (D34 item 4, D40).
/// On error the previous simulation is kept and nothing in `world` changes.
pub(crate) fn install_match(world: &mut World, plan: &MatchPlan) -> Result<MatchStart, String> {
    let mut host = SimHost::new(plan.seed);
    let start = host.install(plan)?;
    world.insert_resource(host);
    world.resource_mut::<CommandSink>().reset();
    Ok(start)
}

/// Sets up `plan`'s match in a simulation of its own, which is then dropped: whether the match
/// can be set up, without changing anything in the game (the catalogue's check at startup, M6
/// review 1 P1-1).
pub(crate) fn check_match(plan: &MatchPlan) -> Result<MatchStart, String> {
    SimHost::new(plan.seed).install(plan)
}

/// Ends the match: the simulation is replaced by one without a match, so no tick runs. The sink
/// closes, which discards pending commands with a log line (M6 review 1 P2-2), and is unpaused;
/// the next match reopens it.
pub(crate) fn uninstall_match(world: &mut World) {
    world.insert_resource(SimHost::new(0));
    let mut sink = world.resource_mut::<CommandSink>();
    sink.close();
    sink.paused = false;
}

/// Why the sink did not accept a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRefused {
    /// The game is paused; no command is accepted until it resumes.
    Paused,
    /// The match has ended (M6); no command is accepted until the next match.
    MatchOver,
    /// [`MAX_PENDING_COMMANDS`] commands already wait for the next tick.
    Full,
}

impl fmt::Display for CommandRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paused => write!(f, "the game is paused"),
            Self::MatchOver => write!(f, "the match is over"),
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
    /// Set when the match has ended (M6).
    closed: bool,
    /// Commands accepted so far (for the interface's acknowledgement sound).
    accepted: u64,
    /// Commands refused so far (for the interface's error sound).
    refused: u64,
    /// Accepted commands discarded so far without being issued, each with a log line (M6 review
    /// 1 P2-2).
    discarded: u64,
}

impl CommandSink {
    /// Accepts `command` for the next tick.
    pub fn push(&mut self, command: Command) -> Result<(), CommandRefused> {
        if self.closed {
            self.refused = self.refused.saturating_add(1);
            return Err(CommandRefused::MatchOver);
        }
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

    /// Prepares the sink for a new match: no pending command (each one left is discarded with a
    /// log line), not paused. The counters keep counting, so the interface's sounds answer only
    /// commands issued after them.
    fn reset(&mut self) {
        self.discard("a new match starts");
        self.paused = false;
        self.closed = false;
    }

    /// Refuses every command from now on (the match has ended); pending commands are discarded
    /// with a log line naming them.
    fn close(&mut self) {
        self.discard("the match is over");
        self.closed = true;
    }

    /// Discards the pending commands, each with a log line naming it and `why`: the only way
    /// pending commands leave the sink other than [`Self::take`], so none is lost silently.
    fn discard(&mut self, why: &str) {
        for command in self.pending.drain(..) {
            self.discarded = self.discarded.saturating_add(1);
            warn!(
                "command not issued: {why}: {command:?} ({} discarded so far)",
                self.discarded
            );
        }
    }
}

/// Advances the simulation by exactly one tick per fixed timestep (20 Hz) while a match is
/// running (set up, and not ended), unless paused.
fn run_tick(
    mut host: ResMut<SimHost>,
    mut sink: ResMut<CommandSink>,
    mut events: MessageWriter<SimEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    if sink.paused || !host.running() {
        return;
    }
    let commands = sink.take();
    match host.step(commands) {
        Ok(tick_events) => {
            events.write_batch(tick_events);
            if let Some(outcome) = host.outcome() {
                info!("the match has ended: {outcome:?}");
                sink.close();
            } else if host.human_defeated() {
                info!("the human player has been defeated; computer players remain");
                sink.close();
            }
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

    /// A host with the demo match installed, as a match started from the menu with its first
    /// choices installs it (`crate::skirmish::begin_match`, seed 0), except that the players
    /// start with their Construction Yards instead of the base units that deploy into them
    /// (RA1: the lobby's "start with an MCV" off), so that the interface tests that build on it
    /// have a base from the first tick. [`menu_host`] is the match as the menu starts it.
    pub(crate) fn demo_host() -> SimHost {
        demo_match(false)
    }

    /// A host with the demo match installed exactly as a match started from the menu with its
    /// first choices installs it: every player with the base unit of its side.
    pub(crate) fn menu_host() -> SimHost {
        demo_match(true)
    }

    fn demo_match(mcv_start: bool) -> SimHost {
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection::initial(&catalogue);
        let plan = MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start,
            grid: catalogue.maps[selection.map].map.grid.clone(),
            looks: catalogue.maps[selection.map].map.looks.clone(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: crate::skirmish::match_players(&catalogue, selection),
            seed: 0,
        };
        let mut host = SimHost::new(plan.seed);
        let start = host
            .install(&plan)
            .unwrap_or_else(|error| panic!("{error}"));
        let (units, buildings) = crate::skirmish::starting_totals(&catalogue, selection, mcv_start);
        assert_eq!(start, MatchStart { buildings, units });
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
        app.add_plugins((MinimalPlugins, SimHostPlugin));
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
        app.add_plugins((MinimalPlugins, SimHostPlugin));
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
        // M6: matches start from the menu (`crate::skirmish::begin_match`) with the catalogue's
        // data; the demo map holds the demo match.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection::initial(&catalogue);
        let mut world = World::new();
        world.init_resource::<CommandSink>();
        world.insert_resource(SimHost::new(0));
        assert!(
            !world.resource::<SimHost>().running(),
            "no match before setup"
        );
        // As the menu starts it (RA1: every player with the base unit of its side), or with the
        // Construction Yards the base units deploy into.
        let plan = |grid: Grid, mcv_start: bool| MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start,
            grid,
            looks: Vec::new(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: crate::skirmish::match_players(&catalogue, selection),
            seed: 0,
        };
        let demo = catalogue.maps[selection.map].map.grid.clone();
        let start = install_match(&mut world, &plan(demo, true)).expect("the demo match");
        let (units, buildings) = crate::skirmish::starting_totals(&catalogue, selection, true);
        assert_eq!(start, MatchStart { buildings, units });
        assert_eq!(buildings, 0, "the players start with their base units");
        assert!(world.resource::<SimHost>().running());
        // A 16x16 map cannot hold the starting buildings at the demo map's start positions: the
        // setup fails with an error naming the building, and the previous simulation is kept.
        let small = sim::load_map(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../sim/tests/fixtures/maps/open_16x16.tmx"),
        )
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
        let tick_before = world.resource::<SimHost>().tick();
        // The demo map's start positions lie outside the 16x16 map, which match setup reports
        // first (M6 review 1 P1-1); start positions on it, near its edge, leave the yard sticking
        // out (the yard a player starts with when the match does not start with base units).
        for mcv_start in [true, false] {
            let error =
                install_match(&mut world, &plan(small.clone(), mcv_start)).expect_err("too small");
            assert!(error.contains("start position 0 at (15, 37)"), "{error}");
        }
        let mut near_edge = plan(small, false);
        near_edge.skirmish.maps[selection.map].starts =
            vec![CellPos::new(14, 1), CellPos::new(1, 14)];
        let error = install_match(&mut world, &near_edge).expect_err("too small");
        assert!(error.contains("starting building"), "{error}");
        assert!(world.resource::<SimHost>().running());
        assert_eq!(world.resource::<SimHost>().tick(), tick_before);
        // Through the game flow: the error ends the game with exit code 1 and nothing is reported
        // as spawned.
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            SimHostPlugin,
        ))
        .init_asset::<Image>()
        .init_resource::<crate::buildings::BuildingStatus>()
        .init_resource::<crate::units::UnitStatus>()
        .init_resource::<crate::skirmish::Screen>()
        .init_resource::<crate::map::MapStatus>()
        .init_resource::<crate::map::HoveredCell>();
        let mut small_catalogue = catalogue.clone();
        small_catalogue.maps[selection.map].map.grid = sim::load_map(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../sim/tests/fixtures/maps/open_16x16.tmx"),
        )
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
        app.world_mut().insert_resource(small_catalogue);
        crate::skirmish::begin_match(app.world_mut(), selection, 0);
        assert_eq!(app.should_exit(), Some(AppExit::error()));
        assert_eq!(
            app.world().resource::<crate::units::UnitStatus>().spawned,
            0
        );
        assert_eq!(
            app.world()
                .resource::<crate::buildings::BuildingStatus>()
                .spawned,
            0
        );
        assert_eq!(
            *app.world().resource::<crate::skirmish::Screen>(),
            crate::skirmish::Screen::Menu
        );
        assert!(!app.world().resource::<SimHost>().running());
    }

    #[test]
    fn the_sink_refuses_commands_once_the_match_is_over_and_reopens_for_the_next() {
        let mut sink = CommandSink::default();
        let stop = Command::Stop {
            units: vec![SimId(1)],
        };
        assert_eq!(sink.push(stop.clone()), Ok(()));
        sink.close();
        assert!(
            sink.pending.is_empty(),
            "pending commands are discarded, logged"
        );
        assert_eq!(sink.push(stop.clone()), Err(CommandRefused::MatchOver));
        assert_eq!(sink.refused(), 1);
        sink.toggle_pause();
        sink.reset();
        assert!(!sink.paused());
        assert_eq!(sink.push(stop), Ok(()));
        // The counters keep counting across matches (the interface's sounds compare them).
        assert_eq!((sink.accepted(), sink.refused()), (2, 1));
    }

    #[test]
    fn commands_pending_when_a_match_is_replaced_are_discarded_with_a_log_line() {
        // M6 review 1 P2-2: ending a match that is still running (the smoke flow replaces running
        // matches) reset the sink, which cleared its pending commands silently, before closing it,
        // which logs what it discards. Every discarded command is now logged and counted.
        let mut world = World::new();
        world.init_resource::<CommandSink>();
        world.insert_resource(demo_host());
        let stop = Command::Stop {
            units: vec![SimId(1)],
        };
        world
            .resource_mut::<CommandSink>()
            .push(stop.clone())
            .expect("accepted");
        uninstall_match(&mut world);
        let sink = world.resource::<CommandSink>();
        assert!(sink.pending.is_empty());
        assert_eq!(
            sink.discarded, 1,
            "the pending command is discarded with a log line"
        );
        assert!(sink.closed);
        // Installing a match over pending commands discards them with a log line too.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = crate::skirmish::MenuSelection::initial(&catalogue);
        let plan = MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start: true,
            grid: catalogue.maps[selection.map].map.grid.clone(),
            looks: catalogue.maps[selection.map].map.looks.clone(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: crate::skirmish::match_players(&catalogue, selection),
            seed: 0,
        };
        world.resource_mut::<CommandSink>().reset();
        world
            .resource_mut::<CommandSink>()
            .push(stop)
            .expect("accepted");
        install_match(&mut world, &plan).expect("the demo match");
        let sink = world.resource::<CommandSink>();
        assert!(sink.pending.is_empty());
        assert_eq!(sink.discarded, 2);
        assert!(!sink.closed);
    }

    /// What the player asked of every match from the menu: the computer players fight the
    /// human and never each other, Super Me carries its unit speed, and the map's trees and
    /// houses are scenery that can be destroyed.
    #[test]
    fn menu_matches_ally_the_computer_players_and_install_speed_and_scenery() {
        let catalogue = crate::skirmish::tests::production_catalogue();
        let mut with_scenery = 0;
        for map in 0..catalogue.maps.len() {
            let selection = crate::skirmish::MenuSelection {
                map,
                enemies: 1,
                super_me: true,
                unit_speed: 2,
                ..crate::skirmish::MenuSelection::initial(&catalogue)
            };
            // As the menu starts it: on the map enlarged for its three players.
            let id = &catalogue.maps[map].id;
            let (plan, _) = crate::skirmish::match_plan(&catalogue, selection, 0)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            // (A map file so large that the next step would pass the largest size is played
            // as it is.)
            let filed = &catalogue.maps[map].map.grid;
            assert_eq!(
                (plan.grid.width(), plan.grid.height()),
                sim::enlarged_size(filed.width(), filed.height(), 3),
                "{id}"
            );
            assert!(
                plan.grid.width() > filed.width() || filed.width() > 224,
                "{id}"
            );
            let mut host = SimHost::new(plan.seed);
            host.install(&plan)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let players = host.players().expect("players");
            let (first, second) = (PlayerId(1), PlayerId(2));
            assert!(players.hostile(HUMAN, first) && players.hostile(second, HUMAN));
            assert!(
                !players.hostile(first, second),
                "{id}: the enemies are allies"
            );
            assert_eq!(players.unit_speed_percent(HUMAN), 200);
            assert_eq!(players.unit_speed_percent(first), 100);
            let (props, grid) = (host.props().expect("props"), host.grid().expect("grid"));
            for (cell, prop) in props.iter() {
                assert!(
                    !grid.is_passable(cell),
                    "{id}: scenery stands on blocked ground"
                );
                assert_eq!(prop.hit_points, prop.kind.hit_points());
                assert_eq!(props.get(props.id_at(cell).unwrap()).unwrap().0, cell);
            }
            with_scenery += usize::from(!props.is_empty());
        }
        assert!(
            with_scenery >= 9,
            "{with_scenery} maps have destructible scenery"
        );
        // Without Super Me the unit speed option does nothing.
        let plain = crate::skirmish::MenuSelection {
            unit_speed: 4,
            ..crate::skirmish::MenuSelection::initial(&catalogue)
        };
        assert_eq!(plain.unit_speed_percent(), 100);
    }

    #[test]
    fn ticks_run_only_while_a_match_is_running() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimHostPlugin));
        // No match: the tick system runs no tick.
        app.world_mut().run_system_once(run_tick).expect("runs");
        assert_eq!(app.world().resource::<SimHost>().tick(), 0);
        app.world_mut().insert_resource(demo_host());
        app.world_mut().run_system_once(run_tick).expect("runs");
        assert_eq!(app.world().resource::<SimHost>().tick(), 1);
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

    /// The demo match of [`demo_host`] with scenario setup on top (`setup` may spawn what a
    /// test needs, as `sim`'s own scenario tests do).
    pub(crate) fn demo_host_with(setup: impl FnOnce(&mut World)) -> SimHost {
        let mut host = demo_host();
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
