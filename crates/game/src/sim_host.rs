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

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

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
            .init_resource::<crate::minimap::PublicBuildingNotices>()
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
    // Client observation state, outside the simulation World/hash. Candidate cells do not
    // grant knowledge: an entry is recorded only when HUMAN actually sees that cell.
    scenery_cells: BTreeSet<sim::CellPos>,
    observed_scenery: BTreeMap<sim::CellPos, ObservedScenery>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ObservedScenery {
    pub(crate) prop: Option<sim::Prop>,
    pub(crate) garrison: Option<(sim::PlayerId, usize)>,
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
        Self {
            world,
            scenery_cells: BTreeSet::new(),
            observed_scenery: BTreeMap::new(),
        }
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

    pub(crate) fn observed_scenery(&self, cell: sim::CellPos) -> Option<&ObservedScenery> {
        self.observed_scenery.get(&cell)
    }

    fn observe_scenery(&mut self) {
        let (Some(props), Some(vision)) = (self.props(), crate::fog::human_vision(self.players()))
        else {
            return;
        };
        let cells: BTreeSet<_> = self
            .scenery_cells
            .iter()
            .copied()
            .chain(props.iter().map(|(cell, _)| cell))
            .collect();
        let special = self.world.get_resource::<sim::SpecialState>();
        let seen: Vec<_> = cells
            .iter()
            .filter(|&&cell| vision.is_visible(cell))
            .map(|&cell| {
                let garrison = props.id_at(cell).and_then(|id| {
                    let state = special?;
                    let garrison = state.garrisons.get(&id)?;
                    (garrison.owner == HUMAN || !state.hidden.contains(&(HUMAN, id)))
                        .then(|| (garrison.owner, state.cargo.get(&id).map_or(0, Vec::len)))
                });
                (
                    cell,
                    ObservedScenery {
                        prop: props.at(cell).copied(),
                        garrison,
                    },
                )
            })
            .collect();
        self.scenery_cells = cells;
        self.observed_scenery.extend(seen);
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
        if self
            .world
            .resource::<Grid>()
            .world_features()
            .next()
            .is_none()
        {
            self.world
                .resource_mut::<Grid>()
                .mark_world_features(&plan.looks);
        }
        let start = sim::setup_match_configured(
            &mut self.world,
            &plan.skirmish,
            &plan.ai,
            plan.map,
            &plan.players,
            &plan.options,
        )
        .map_err(|error| error.to_string())?;
        self.scenery_cells = self.props().map_or_else(BTreeSet::new, |props| {
            props.iter().map(|(cell, _)| cell).collect()
        });
        self.observed_scenery.clear();
        self.observe_scenery();
        Ok(start)
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
        // Observe both sides of each actual tick, including multi-tick client frames. Hidden
        // damage/destruction/occupation cannot replace an earlier visible observation.
        self.observe_scenery();
        self.world.run_schedule(SimSchedule);
        self.observe_scenery();
        let events = self
            .world
            .get_resource::<Messages<SimEvent>>()
            .ok_or_else(|| "the simulation has no event buffer".to_string())?;
        Ok(events.iter_current_update_messages().cloned().collect())
    }
}

/// Everything a match is set up from (`sim::setup_match_configured`). The game makes it from the
/// menu's choices with `crate::skirmish::match_plan`, which with MAP GROWTH on enlarges the map
/// for the match's players.
#[derive(Clone, Debug)]
pub struct MatchPlan {
    /// The options of the match (`sim::MatchOptions`): the lobby's of Red Alert 2 (starting
    /// credits, unit count, MCV start, short game, superweapons, crates, MCV repacks, teams, the
    /// players with a random country) and this game's extensions for the human player (Super
    /// Me with its unit speed, "unlock all"). `crate::skirmish::MenuSelection::options` makes
    /// them from the menu's rows.
    pub options: sim::MatchOptions,
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
    crate::minimap::clear_public_building_notices(world);
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
    crate::minimap::clear_public_building_notices(world);
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
    mut notices: Option<ResMut<crate::minimap::PublicBuildingNotices>>,
    mut exit: MessageWriter<AppExit>,
) {
    if sink.paused || !host.running() {
        return;
    }
    let commands = sink.take();
    match host.step(commands) {
        Ok(tick_events) => {
            if let Some(notices) = notices.as_deref_mut() {
                notices.receive(&tick_events);
            }
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

    /// A host with the match of `selection` installed as the menu starts it
    /// (`crate::skirmish::match_plan`, with the seed `seed`), and what its setup placed.
    pub(crate) fn match_host(
        catalogue: &crate::skirmish::Catalogue,
        selection: crate::skirmish::MenuSelection,
        seed: u64,
    ) -> (SimHost, MatchStart) {
        let (plan, _) = crate::skirmish::match_plan(catalogue, selection, seed)
            .unwrap_or_else(|error| panic!("{selection:?}: {error}"));
        let mut host = SimHost::new(plan.seed);
        let start = host
            .install(&plan)
            .unwrap_or_else(|error| panic!("{selection:?}: {error}"));
        (host, start)
    }

    fn demo_match(mcv_start: bool) -> SimHost {
        let catalogue = crate::skirmish::tests::production_catalogue();
        // The menu's first choices with a fixed country for the computer player (RA1-C: the
        // menu itself opens with a RANDOM one), so that the tests know what the match holds.
        let selection = crate::skirmish::MenuSelection {
            mcv_start,
            ..crate::skirmish::MenuSelection::versus(&catalogue)
        };
        let plan = MatchPlan {
            options: selection.options(),
            grid: catalogue.maps[selection.map].map.grid.clone(),
            looks: catalogue.maps[selection.map].map.looks.clone(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: selection.players(),
            seed: 0,
        };
        let mut host = SimHost::new(plan.seed);
        let start = host
            .install(&plan)
            .unwrap_or_else(|error| panic!("{error}"));
        let (units, buildings) =
            crate::skirmish::selection_totals(&catalogue, selection).expect("fixed countries");
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
        // Nothing but the crate the first tick of a match puts down (RA5).
        assert!(
            matches!(events.as_slice(), [SimEvent::CrateAppeared { .. }]),
            "{events:?}"
        );
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
        assert_eq!(
            messages.len(),
            2,
            "the rejection of the stop command, and the match's first crate (RA5)"
        );
        assert_eq!(
            messages
                .iter_current_update_messages()
                .filter(|event| matches!(event, SimEvent::CommandRejected { .. }))
                .count(),
            1
        );
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
        let selection = crate::skirmish::MenuSelection::versus(&catalogue);
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
            options: crate::skirmish::MenuSelection {
                mcv_start,
                ..selection
            }
            .options(),
            grid,
            looks: Vec::new(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: selection.players(),
            seed: 0,
        };
        let demo = catalogue.maps[selection.map].map.grid.clone();
        let start = install_match(&mut world, &plan(demo, true)).expect("the demo match");
        let (units, buildings) =
            crate::skirmish::selection_totals(&catalogue, selection).expect("fixed countries");
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
        let selection = crate::skirmish::MenuSelection::versus(&catalogue);
        let plan = MatchPlan {
            options: selection.options(),
            grid: catalogue.maps[selection.map].map.grid.clone(),
            looks: catalogue.maps[selection.map].map.looks.clone(),
            definitions: catalogue.definitions.clone(),
            skirmish: catalogue.skirmish.clone(),
            ai: catalogue.ai.clone(),
            map: selection.map,
            players: selection.players(),
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
            // (Since RA1 the map grows with the players only with the menu's MAP GROWTH row
            // on, docs/DECISIONS.md R4; the computer players' countries are RANDOM, as the
            // menu has them at first.)
            let selection = crate::skirmish::MenuSelection {
                map,
                enemies: 1,
                map_growth: true,
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
                assert_eq!(
                    grid.is_passable(cell),
                    prop.kind == sim::PropKind::Bridge,
                    "{id}: intact decks are passable; other scenery stands on blocked ground"
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

    /// Runs `setup` on the simulation world of `host` (scenario setup in tests only).
    pub(crate) fn with_world(host: &mut SimHost, setup: impl FnOnce(&mut World)) {
        setup(&mut host.world);
    }

    /// Takes the actual client input commands for exact mapping assertions in game tests.
    pub(crate) fn take_commands(app: &mut App) -> Vec<Command> {
        app.world_mut().resource_mut::<CommandSink>().take()
    }

    /// Test-only public issued-command bridge; identical queue/schedule/event route to step.
    /// No AI intent witness, synthetic receipt, viewer API, or mutable-world access escapes.
    pub(crate) fn run_issued(host: &mut SimHost, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
        {
            let mut queue = host.world.resource_mut::<CommandQueue>();
            for command in commands {
                queue.push(command);
            }
        }
        host.world.run_schedule(SimSchedule);
        host.world
            .resource::<bevy::ecs::message::Messages<SimEvent>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    /// Scenario setup only. Every subsequent privacy-control change uses issued commands and
    /// the real schedule; no mutable simulation access leaves this test helper.
    pub(crate) fn privacy_host(plan: &MatchPlan, setup: impl FnOnce(&mut World)) -> SimHost {
        let mut host = SimHost::new(plan.seed);
        host.install(plan).expect("privacy scenario match");
        setup(&mut host.world);
        host
    }

    pub(crate) fn privacy_plan(fog: bool) -> MatchPlan {
        ra9_notice_plan(fog, "germany")
    }

    pub(crate) fn privacy_step(host: &mut SimHost, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
        for command in commands {
            host.world.resource_mut::<CommandQueue>().push(command);
        }
        host.step(Vec::new()).expect("actual host step")
    }

    /// Test-only snapshots and actual game tick bridge, without exposing mutable Sim state.
    /// Bounded native input test assertion; no mutable host/viewer API is exposed.
    pub(crate) fn assert_native_cells_for_test(host: &mut SimHost) {
        assert!(sim::cell_invariant_violations(&mut host.world).is_empty());
        assert!(sim::stray_claims(&mut host.world).is_empty());
        assert!(
            host.world
                .resource::<sim::Players>()
                .iter()
                .all(|(_, p)| p.credits() >= 0)
        );
    }
    pub(crate) fn pending_commands_for_test(sink: &CommandSink) -> Vec<Command> {
        sink.pending.clone()
    }
    pub(crate) fn run_sink_tick_for_test(world: &mut World) {
        use bevy::ecs::system::RunSystemOnce;
        world
            .run_system_once(run_tick)
            .expect("actual run_tick system");
    }

    /// Runs one tick with the human player's `commands`.
    pub(crate) fn run(host: &mut SimHost, commands: Vec<Command>) -> Vec<SimEvent> {
        host.step(commands)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    use bevy::ecs::system::RunSystemOnce;
    // Historical preparation note: UNCOMPILED / UNRUN. Append shared helpers inside sim_host.rs::tests.
    // No simulation mutation after setup except via CommandSink -> run_tick -> SimSchedule.
    const RA9_PUBLIC_COLOUR: [u8; 4] = [255, 232, 120, 255];
    const RA9_PUBLIC_CELL: CellPos = CellPos::new(24, 12);

    fn ra9_notice_plan(fog: bool, human_country: &str) -> MatchPlan {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
        let definitions = sim::load_ruleset(&data).expect("shipped definitions");
        let mut skirmish =
            sim::load_skirmish(data.join("skirmish.ron"), &definitions).expect("shipped skirmish");
        let ai = sim::load_ai(data.join("ai.ron"), &definitions).expect("shipped ai");
        skirmish.maps[0].starts = vec![
            CellPos::new(8, 8),
            CellPos::new(38, 38),
            CellPos::new(38, 8),
        ];
        skirmish.starting_credits = 10_000;
        let players = [human_country, "great_britain", "russia"]
            .into_iter()
            .enumerate()
            .map(|(i, country)| PlayerSetup {
                player: PlayerId(i as u8),
                faction: skirmish
                    .factions
                    .iter()
                    .position(|f| f.id == country)
                    .expect("country"),
                start: i,
                controller: sim::Controller::Human,
            })
            .collect();
        MatchPlan {
            options: sim::MatchOptions {
                mcv_start: false,
                unit_count: Some(0),
                short_game: false,
                crates: false,
                fog_of_war: fog,
                free_for_all: true,
                ..default()
            },
            grid: Grid::from_cells(
                48,
                48,
                vec![
                    sim::Cell {
                        passable: true,
                        water: false,
                        buildable: true,
                        ore: 0,
                    };
                    48 * 48
                ],
            )
            .expect("grid"),
            looks: vec![0; 48 * 48],
            definitions,
            skirmish,
            ai,
            map: 0,
            players,
            seed: 17,
        }
    }

    fn ra9_notice_app(fog: bool, radar: bool, superweapon: &str) -> (App, SimId, SimId, SimId) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimHostPlugin));
        app.init_resource::<Assets<Image>>()
            .insert_resource(crate::minimap::MinimapState::default())
            .insert_resource(crate::skirmish::Screen::Match);
        let plan = ra9_notice_plan(fog, "germany");
        install_match(app.world_mut(), &plan).expect("actual install path");
        app.world_mut()
            .insert_resource(crate::map::MapTerrain(plan.grid.clone()));
        crate::minimap::size_minimap(app.world_mut(), 48, 48);
        let (power, target, spy) = {
            let mut host = app.world_mut().resource_mut::<SimHost>();
            let world = &mut host.world;
            let power = sim::spawn_building(world, HUMAN, "nuclear_reactor", CellPos::new(2, 2))
                .expect("own generator setup");
            if radar {
                sim::spawn_building(
                    world,
                    HUMAN,
                    "american_airforce_command",
                    CellPos::new(2, 6),
                )
                .expect("own radar setup");
            }
            sim::spawn_building(world, PlayerId(2), "nuclear_reactor", CellPos::new(3, 40))
                .expect("enemy gap power setup");
            sim::spawn_building(world, PlayerId(2), "gap_generator", CellPos::new(24, 16))
                .expect("enemy gap setup");
            let target = sim::spawn_building(world, PlayerId(2), superweapon, RA9_PUBLIC_CELL)
                .expect("hidden flagged building setup");
            sim::spawn_building(world, PlayerId(2), "power_plant", CellPos::new(30, 2))
                .expect("unrelated hidden building setup");
            sim::spawn_unit(world, PlayerId(2), "conscript", CellPos::new(13, 8))
                .expect("ordinary honestly seen enemy setup");
            let spy = sim::spawn_unit(world, PlayerId(2), "spy", CellPos::new(25, 15))
                .expect("unrelated concealed Spy setup");
            (power, target, spy)
        };
        (app, power, target, spy)
    }

    fn ra9_notice_tick(app: &mut App) {
        // Rotate the client buffer as a UI frame does, while individual tests may withhold drawing.
        // SimHost::step independently rotates the simulation buffer every native SimSchedule tick.
        app.world_mut()
            .resource_mut::<Messages<SimEvent>>()
            .update();
        app.world_mut()
            .run_system_once(run_tick)
            .expect("actual scheduled event bridge");
    }

    fn ra9_notice_draw(app: &mut App) -> (Vec<u8>, crate::minimap::MinimapLayout) {
        app.world_mut()
            .run_system_once(crate::minimap::draw_minimap)
            .expect("actual image system");
        let world = app.world();
        let minimap = world.resource::<crate::minimap::Minimap>();
        let pixels = world
            .resource::<Assets<Image>>()
            .get(&minimap.image)
            .and_then(|image| image.data.clone())
            .expect("actual image bytes");
        (pixels, minimap.layout)
    }

    fn ra9_cell_public_pixels(
        pixels: &[u8],
        layout: crate::minimap::MinimapLayout,
        cell: CellPos,
    ) -> usize {
        // Independent bounded neighbourhood of the projection; does not call the notice renderer.
        let point = layout.normalised(cell.x as f32 + 0.5, cell.y as f32 + 0.5);
        let (width, height) = layout.size();
        let (cx, cy) = (
            (point.x * width as f32) as i32,
            (point.y * (width as f32 / 2.0)) as i32,
        );
        let mut count = 0;
        for y in (cy - 3).max(0)..=(cy + 3).min(height as i32 - 1) {
            for x in (cx - 3).max(0)..=(cx + 3).min(width as i32 - 1) {
                let offset = (y as usize * width as usize + x as usize) * 4;
                count += usize::from(pixels[offset..offset + 4] == RA9_PUBLIC_COLOUR);
            }
        }
        count
    }

    fn ra9_notice_age(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs_f32(seconds));
        // Prospective call supplied by the client patch; do not include this helper BEFORE it.
        app.world_mut()
            .run_system_once(crate::minimap::age_public_building_notices)
            .expect("real-time client expiry system");
    }

    // Historical preparation note: UNCOMPILED / UNRUN AFTER tests. Append with common helpers inside sim_host::tests.
    // Requires K integrated event/schema + this client production patch. All actors/scenarios
    // are setup with real shipped definitions; dynamic changes below use actual commands.
    #[test]
    fn ra9_actual_tick_events_draw_only_the_public_coordinate_even_under_gap() {
        for fog in [false, true] {
            for kind in [
                "chronosphere",
                "weather_control",
                "iron_curtain",
                "nuclear_silo",
            ] {
                let (mut app, _, target, spy) = ra9_notice_app(fog, true, kind);
                ra9_notice_tick(&mut app);
                let host = app.world().resource::<SimHost>();
                assert!(crate::minimap::radar_online(host));
                let events = app.world().resource::<Messages<SimEvent>>();
                assert_eq!(events.iter_current_update_messages().filter(|event|
                matches!(event, SimEvent::BuildingRevealed { building, player, origin, .. }
                    if *building == target && *player == PlayerId(2) && *origin == RA9_PUBLIC_CELL)
            ).count(), 1, "real Visibility receipt is a premise");
                let vision = host
                    .players()
                    .expect("players")
                    .get(HUMAN)
                    .expect("human")
                    .vision();
                assert!(!vision.is_visible(RA9_PUBLIC_CELL));
                assert!(
                    !vision.is_explored(RA9_PUBLIC_CELL),
                    "powered hostile Gap erases terrain"
                );
                assert!(
                    host.world()
                        .resource::<sim::SpecialState>()
                        .hidden
                        .contains(&(HUMAN, spy))
                );
                let contacts = crate::minimap::owned_markers(host);
                assert!(contacts.iter().any(|(_, marker, owner)| *owner == HUMAN
                    && *marker == crate::minimap::Marker::OwnBuilding));
                assert!(!contacts.iter().any(|(cell, _, owner)| {
                    *owner == PlayerId(2)
                        && [RA9_PUBLIC_CELL, CellPos::new(30, 2), CellPos::new(25, 15)]
                            .contains(cell)
                }));
                let (pixels, layout) = ra9_notice_draw(&mut app);
                assert_eq!(
                    ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL),
                    12,
                    "native hollow diamond, not a state counter"
                );
                assert!(
                    contacts
                        .iter()
                        .any(|(cell, marker, owner)| *cell == CellPos::new(13, 8)
                            && *owner == PlayerId(2)
                            && *marker == crate::minimap::Marker::EnemyUnit),
                    "normal known contact premise"
                );
                for &(cell, _, owner) in &contacts {
                    let [r, g, b] = crate::presentation::TeamColors(0).rgb_of(owner);
                    let point = layout.normalised(cell.x as f32 + 0.5, cell.y as f32 + 0.5);
                    let (width, _) = layout.size();
                    let x = (point.x * width as f32) as usize;
                    let y = (point.y * (width as f32 / 2.0)) as usize;
                    let offset = (y * width as usize + x) * 4;
                    assert_eq!(
                        pixels[offset..offset + 4],
                        [r, g, b, 255],
                        "ordinary known/owned contact retained"
                    );
                }
                assert_eq!(
                    ra9_cell_public_pixels(&pixels, layout, CellPos::new(30, 2)),
                    0,
                    "no notice for unrelated hidden building"
                );
                // The nearby Spy remains unmarked. Its centre is outside the public diamond;
                // a broad neighbourhood would deliberately overlap the legitimate public notice.
                let point = layout.normalised(25.5, 15.5);
                let (width, _) = layout.size();
                let x = (point.x * width as f32) as usize;
                let y = (point.y * (width as f32 / 2.0)) as usize;
                let offset = (y * width as usize + x) * 4;
                assert_eq!(
                    pixels[offset..offset + 4],
                    [0, 0, 0, 255],
                    "Spy cell stays shroud"
                );
                // Two more native ticks before drawing again cannot erase or re-date first receipt.
                ra9_notice_age(&mut app, 8.0);
                ra9_notice_tick(&mut app);
                ra9_notice_tick(&mut app);
                assert!(!app.world().resource::<Messages<SimEvent>>().iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::BuildingRevealed { building, .. } if *building == target)));
                let (pixels, layout) = ra9_notice_draw(&mut app);
                assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 12);
                ra9_notice_age(&mut app, 1.0); // Exact pre-existing NOTICE_SECONDS=9 boundary.
                let (pixels, layout) = ra9_notice_draw(&mut app);
                assert_eq!(
                    ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL),
                    0,
                    "ordinary later ticks do not indefinitely renew a public receipt"
                );
            }
        }
    }

    #[test]
    fn ra9_public_notice_expires_while_paused_and_redraws_at_the_unchanged_tick() {
        let (mut app, _, _, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        let (before, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&before, layout, RA9_PUBLIC_CELL), 12);
        assert!(app.world_mut().resource_mut::<CommandSink>().toggle_pause());
        let (tick, hash, vision) = {
            let mut host = app.world_mut().resource_mut::<SimHost>();
            let tick = host.tick();
            let vision = host
                .players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .vision()
                .clone();
            (tick, sim::state_hash(&mut host.world), vision)
        };
        ra9_notice_age(&mut app, 8.99);
        ra9_notice_tick(&mut app); // Native paused bridge must leave the simulation untouched.
        let (almost, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&almost, layout, RA9_PUBLIC_CELL), 12);
        ra9_notice_age(&mut app, 0.02);
        let (expired, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&expired, layout, RA9_PUBLIC_CELL), 0);
        let mut host = app.world_mut().resource_mut::<SimHost>();
        assert_eq!(host.tick(), tick);
        assert_eq!(sim::state_hash(&mut host.world), hash);
        assert_eq!(
            host.players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .vision(),
            &vision
        );
    }

    #[test]
    fn ra9_receipt_never_powers_radar_and_power_recovery_does_not_replay_expired_notice() {
        let (mut offline, _, _, _) = ra9_notice_app(true, false, "chronosphere");
        ra9_notice_tick(&mut offline);
        assert!(!crate::minimap::radar_online(
            offline.world().resource::<SimHost>()
        ));
        assert!(
            offline
                .world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::BuildingRevealed { .. }))
        );
        let (pixels, layout) = ra9_notice_draw(&mut offline);
        assert_eq!(pixels, crate::minimap::offline_pixels(&layout));

        let (mut app, power, _, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 12);
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::SellBuilding { building: power })
            .expect("human sell accepted by sink");
        ra9_notice_tick(&mut app);
        let host = app.world().resource::<SimHost>();
        assert!(
            host.players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .low_power()
        );
        assert!(!crate::minimap::radar_online(host));
        assert!(
            !app.world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
        );
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(pixels, crate::minimap::offline_pixels(&layout));
        ra9_notice_age(&mut app, 9.0);
        let kind = app
            .world()
            .resource::<SimHost>()
            .definitions()
            .expect("definitions")
            .building_kind("power_plant")
            .expect("power kind");
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::QueueProduction {
                item: ProductionItem::Building(kind),
            })
            .expect("queue");
        let mut completed = false;
        for _ in 0..2400 {
            ra9_notice_tick(&mut app);
            assert!(
                !app.world()
                    .resource::<Messages<SimEvent>>()
                    .iter_current_update_messages()
                    .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
            );
            if app
                .world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| {
                    matches!(event, SimEvent::ProductionComplete { item, .. }
                if *item == ProductionItem::Building(kind))
                })
            {
                completed = true;
                break;
            }
        }
        assert!(completed, "actual low-power paid production must complete");
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::PlaceBuilding {
                building: kind,
                origin: CellPos::new(8, 11),
            })
            .expect("place");
        ra9_notice_tick(&mut app);
        assert!(
            !app.world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
        );
        assert!(crate::minimap::radar_online(
            app.world().resource::<SimHost>()
        ));
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_ne!(pixels, crate::minimap::offline_pixels(&layout));
        assert_eq!(
            ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL),
            0,
            "power restored through real paid construction cannot resurrect old notices"
        );
    }

    #[test]
    fn ra9_menu_install_country_switch_and_old_buffered_messages_reset_public_notices() {
        let (mut app, _, target, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 12);
        // The exact old public event is still in the game buffer across the real menu transition.
        assert!(app.world().resource::<Messages<SimEvent>>().iter_current_update_messages()
        .any(|event| matches!(event, SimEvent::BuildingRevealed { building, .. } if *building == target)));
        crate::skirmish::end_match(app.world_mut());
        assert_eq!(
            *app.world().resource::<crate::skirmish::Screen>(),
            crate::skirmish::Screen::Menu
        );
        let plan = ra9_notice_plan(true, "russia"); // Actual player choice change: P0's country.
        install_match(app.world_mut(), &plan).expect("actual next-match install");
        app.world_mut()
            .insert_resource(crate::map::MapTerrain(plan.grid.clone()));
        crate::minimap::size_minimap(app.world_mut(), 48, 48);
        {
            let mut host = app.world_mut().resource_mut::<SimHost>();
            sim::spawn_building(
                &mut host.world,
                HUMAN,
                "nuclear_reactor",
                CellPos::new(2, 2),
            )
            .expect("new-country power setup before first tick");
            sim::spawn_building(&mut host.world, HUMAN, "radar_tower", CellPos::new(2, 6))
                .expect("new-country radar setup before first tick");
        }
        // No tick means old game messages remain, yet client records must already be gone.
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 0);
        ra9_notice_tick(&mut app);
        assert!(crate::minimap::radar_online(
            app.world().resource::<SimHost>()
        ));
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 0);
        // Direct successful installation (without the menu) must also clear the old records.
        let (mut app, _, _, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        install_match(app.world_mut(), &ra9_notice_plan(true, "germany"))
            .expect("direct replacement");
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 0);
        // Isolate uninstall from install and menu terrain teardown: the lower-level boundary
        // replaces the host while this deliberately leaves its old client terrain/image installed.
        let (mut app, _, _, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 12);
        uninstall_match(app.world_mut());
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(
            ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL),
            0,
            "uninstall itself clears the cache before either install or map teardown"
        );
    }

    #[test]
    fn ra9_ticks_across_undrawn_client_frames_keep_every_actual_public_receipt() {
        let (mut app, _, first, _) = ra9_notice_app(true, true, "chronosphere");
        let second_cell = CellPos::new(32, 16);
        let second = {
            let mut host = app.world_mut().resource_mut::<SimHost>();
            sim::spawn_building(&mut host.world, PlayerId(2), "weather_control", second_cell)
                .expect("second hidden flagged building scenario setup before first tick")
        };
        ra9_notice_tick(&mut app);
        for id in [first, second] {
            assert!(app.world().resource::<Messages<SimEvent>>().iter_current_update_messages()
            .any(|event| matches!(event, SimEvent::BuildingRevealed { building, .. } if *building == id)));
        }
        ra9_notice_tick(&mut app);
        ra9_notice_tick(&mut app);
        assert!(
            !app.world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::BuildingRevealed { .. }))
        );
        let host = app.world().resource::<SimHost>();
        let vision = host
            .players()
            .expect("players")
            .get(HUMAN)
            .expect("human")
            .vision();
        assert!(!vision.is_visible(RA9_PUBLIC_CELL));
        assert!(!vision.is_visible(second_cell));
        let (pixels, layout) = ra9_notice_draw(&mut app);
        for cell in [RA9_PUBLIC_CELL, second_cell] {
            assert_eq!(ra9_cell_public_pixels(&pixels, layout, cell), 12);
        }
    }

    #[test]
    fn ra9_power_return_before_expiry_shows_the_remaining_public_notice() {
        let (mut app, power, _, _) = ra9_notice_app(true, true, "chronosphere");
        ra9_notice_tick(&mut app);
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::SellBuilding { building: power })
            .expect("sell");
        ra9_notice_tick(&mut app);
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(pixels, crate::minimap::offline_pixels(&layout));
        ra9_notice_age(&mut app, 1.0);
        let kind = app
            .world()
            .resource::<SimHost>()
            .definitions()
            .expect("definitions")
            .building_kind("power_plant")
            .expect("kind");
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::QueueProduction {
                item: ProductionItem::Building(kind),
            })
            .expect("queue");
        let mut completed = false;
        for _ in 0..2400 {
            ra9_notice_tick(&mut app);
            assert!(
                !app.world()
                    .resource::<Messages<SimEvent>>()
                    .iter_current_update_messages()
                    .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
            );
            if app
                .world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| {
                    matches!(event, SimEvent::ProductionComplete { item, .. }
                if *item == ProductionItem::Building(kind))
                })
            {
                completed = true;
                break;
            }
        }
        assert!(completed, "actual low-power paid production");
        app.world_mut()
            .resource_mut::<CommandSink>()
            .push(Command::PlaceBuilding {
                building: kind,
                origin: CellPos::new(8, 11),
            })
            .expect("place");
        ra9_notice_tick(&mut app);
        assert!(
            !app.world()
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
        );
        assert!(crate::minimap::radar_online(
            app.world().resource::<SimHost>()
        ));
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL), 12);
        ra9_notice_age(&mut app, 8.0);
        let (pixels, layout) = ra9_notice_draw(&mut app);
        assert_eq!(
            ra9_cell_public_pixels(&pixels, layout, RA9_PUBLIC_CELL),
            0,
            "eight remaining real seconds, with no pause or tick-count lifetime substitution"
        );
    }

    #[test]
    fn ra9_public_coordinate_notice_at_the_map_edge_never_colours_off_map_pixels() {
        let (mut app, _, _, _) = ra9_notice_app(true, true, "chronosphere");
        let edge = CellPos::new(0, 45);
        {
            let mut host = app.world_mut().resource_mut::<SimHost>();
            sim::spawn_building(
                &mut host.world,
                PlayerId(2),
                "gap_generator",
                CellPos::new(0, 40),
            )
            .expect("edge Gap scenario setup");
            sim::spawn_building(&mut host.world, PlayerId(2), "weather_control", edge)
                .expect("edge public building scenario setup");
        }
        ra9_notice_tick(&mut app);
        let host = app.world().resource::<SimHost>();
        assert!(
            !host
                .players()
                .expect("players")
                .get(HUMAN)
                .expect("human")
                .vision()
                .is_explored(edge),
            "edge public event remains distinct from terrain"
        );
        assert!(app.world().resource::<Messages<SimEvent>>().iter_current_update_messages()
        .any(|event| matches!(event, SimEvent::BuildingRevealed { origin, .. } if *origin == edge)));
        let (pixels, layout) = ra9_notice_draw(&mut app);
        let count = ra9_cell_public_pixels(&pixels, layout, edge);
        assert!(
            count > 0 && count < 12,
            "native marker is clipped by map diamond: {count}"
        );
        let (width, height) = layout.size();
        for y in 0..height {
            for x in 0..width {
                if layout.pixel_cell(x, y).is_none() {
                    let offset = (y as usize * width as usize + x as usize) * 4;
                    assert_eq!(pixels[offset..offset + 4], [0, 0, 0, 0]);
                }
            }
        }
    }
}
