//! Skirmish match setup and the win condition (PLAN.md section 7, M6; docs/DECISIONS.md D40,
//! D41).
//!
//! [`setup_match`] installs a match into a world set up by [`SimPlugin`](crate::SimPlugin) with the
//! map's grid and the definitions installed: it adds the players with the skirmish file's
//! starting credits, places every faction's starting buildings and units at the player's start
//! position, gives every computer player its controller (`crate::ai`), and records the players
//! in [`MatchState`]. Like installing the grid (docs/DECISIONS.md D11), this is setup before the
//! first tick, not a gameplay change, so it does not go through `CommandQueue`.
//!
//! The win condition (the `WinCondition` stage): at the end of every tick, a player of the match
//! who owns no building is defeated ([`SimEvent::PlayerDefeated`]); once at most one player is
//! left, the match ends ([`SimEvent::MatchEnded`]) with that player as the winner, or as a draw if
//! the last players were defeated in the same tick. The outcome is final. A world without a match
//! set up (no players in [`MatchState`], as in the tests of M0 to M5) has no win condition.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_ecs::world::World;

use crate::ai::{AiController, AiPlayers};
use crate::ai_defs::{AiDefinitions, faction_plan_problem, unobtainable_producer};
use crate::building::{Building, SpawnBuildingError, spawn_building};
use crate::command::PlayerId;
use crate::definitions::Definitions;
use crate::economy::{PlayerSetupError, Players};
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::map::{CellPos, Grid};
use crate::pathfinding::destination_cells;
use crate::schedule::SimTick;
use crate::skirmish_defs::{
    MAX_MATCH_PLAYERS, MIN_MATCH_PLAYERS, SkirmishDefinitions, StartProblem, start_problem,
};
use crate::unit::{Owner, SpawnError, spawn_unit};

/// Who gives a player's orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Controller {
    /// A person, through the interface.
    Human,
    /// The computer, with the difficulty of this index in the AI file.
    Ai { difficulty: usize },
}

/// A player of a match to be set up: its id, the index of its faction in the skirmish file, the
/// index of its start position on the map, and who controls it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerSetup {
    pub player: PlayerId,
    pub faction: usize,
    pub start: usize,
    pub controller: Controller,
}

/// What [`setup_match`] placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchStart {
    pub buildings: usize,
    pub units: usize,
}

/// A group of starting units for which the map has too few free cells: fewer than `required`
/// passable, unoccupied cells are reachable from `anchor` (none if it lies outside the map).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartError {
    pub owner: PlayerId,
    pub unit_type: String,
    pub anchor: CellPos,
    pub found: usize,
    pub required: usize,
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "starting units of player {}: {} {:?} units are placed around cell ({}, {}), but \
             only {} free cells are reachable there",
            self.owner.0, self.required, self.unit_type, self.anchor.x, self.anchor.y, self.found
        )?;
        if self.found == 0 {
            write!(f, " (the cell is outside the map or walled in)")?;
        }
        Ok(())
    }
}

impl std::error::Error for StartError {}

/// The cell of every starting unit: for each group `(owner, unit type, anchor, count)` in order,
/// the free cells nearest to its anchor (`destination_cells`), skipping cells occupied in `grid`
/// and cells given to earlier groups. A group that does not get all of its cells is an error, so
/// match setup never spawns fewer units than configured (M2 review 1, P1-2).
pub fn plan_starting_units<'a>(
    grid: &Grid,
    groups: &[(PlayerId, &'a str, (i32, i32), usize)],
) -> Result<Vec<(PlayerId, &'a str, CellPos)>, StartError> {
    let mut planned = Vec::new();
    let mut taken = BTreeSet::new();
    for &(owner, unit_type, (x, y), count) in groups {
        let anchor = CellPos::new(x, y);
        let cells = destination_cells(grid, anchor, count, |cell| {
            grid.occupant(cell).is_some() || taken.contains(&cell)
        });
        if cells.len() < count {
            return Err(StartError {
                owner,
                unit_type: unit_type.to_owned(),
                anchor,
                found: cells.len(),
                required: count,
            });
        }
        for cell in cells {
            taken.insert(cell);
            planned.push((owner, unit_type, cell));
        }
    }
    Ok(planned)
}

/// Why a match could not be set up. After an error the world is partly set up and must be
/// discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchSetupError {
    /// The world has no grid (0 × 0), or no definitions with an economy file.
    NotInstalled,
    /// The map index is not a map of the skirmish file.
    UnknownMap(usize),
    /// Fewer than [`MIN_MATCH_PLAYERS`] players, more than the map has start positions (at most
    /// [`MAX_MATCH_PLAYERS`]), or a player or start position given twice.
    Players(String),
    UnknownFaction {
        player: PlayerId,
        faction: usize,
    },
    UnknownStart {
        player: PlayerId,
        start: usize,
    },
    UnknownDifficulty {
        player: PlayerId,
        difficulty: usize,
    },
    /// Start position `start` of the map is not a cell of the map with passable terrain (M6
    /// review 1 P1-1).
    InvalidStart {
        start: usize,
        cell: CellPos,
        problem: StartProblem,
    },
    /// A computer player's faction does not start with the producer of a building its AI file
    /// has it build, and the build order does not provide it either (M6 review 1 P2-1).
    UnobtainableProducer {
        player: PlayerId,
        /// The field of the AI file that names the building.
        field: String,
        building: String,
        producer: String,
    },
    /// The plan the AI file gives a computer player of its faction is incomplete (M7, docs/
    /// DECISIONS.md D49): the faction does not offer the power building or the harvester, or the
    /// entries it offers break a rule of the AI file.
    AiPlan {
        player: PlayerId,
        faction: String,
        /// The field of the AI file.
        field: String,
        problem: String,
    },
    /// No passable cell free of buildings is reachable from a computer player's start position
    /// over the map's terrain, so it has no gathering point (M6 review 1 P1-1).
    NoGatheringPoint {
        player: PlayerId,
        start: CellPos,
    },
    /// A player could not be added.
    AddPlayer(PlayerSetupError),
    /// A starting building does not fit.
    Building {
        player: PlayerId,
        building: String,
        origin: CellPos,
        error: SpawnBuildingError,
    },
    /// A group of starting units does not fit.
    Units(StartError),
    /// A starting unit could not be spawned.
    Unit {
        player: PlayerId,
        unit: String,
        cell: CellPos,
        error: SpawnError,
    },
}

impl fmt::Display for MatchSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(
                f,
                "the match has no map or no definitions installed before its setup"
            ),
            Self::UnknownMap(map) => write!(f, "map {map} is not a map of the skirmish file"),
            Self::Players(problem) => write!(f, "players: {problem}"),
            Self::UnknownFaction { player, faction } => write!(
                f,
                "player {}: faction {faction} is not a faction of the skirmish file",
                player.0
            ),
            Self::UnknownStart { player, start } => write!(
                f,
                "player {}: start position {start} is not a start position of the map",
                player.0
            ),
            Self::UnknownDifficulty { player, difficulty } => write!(
                f,
                "player {}: difficulty {difficulty} is not a difficulty of the AI file",
                player.0
            ),
            Self::InvalidStart {
                start,
                cell,
                problem,
            } => write!(
                f,
                "start position {start} at ({}, {}) is {problem} of the map, not a cell of the \
                 map with passable terrain",
                cell.x, cell.y
            ),
            Self::UnobtainableProducer {
                player,
                field,
                building,
                producer,
            } => write!(
                f,
                "computer player {}: the AI file's {field} builds {building:?}, whose producer \
                 {producer:?} its faction does not start with",
                player.0
            ),
            Self::AiPlan {
                player,
                faction,
                field,
                problem,
            } => write!(
                f,
                "computer player {} of faction {faction:?}: the AI file's {field}: {problem}",
                player.0
            ),
            Self::NoGatheringPoint { player, start } => write!(
                f,
                "computer player {}: no passable cell free of buildings is reachable from its \
                 start position ({}, {}), so it has no gathering point",
                player.0, start.x, start.y
            ),
            Self::AddPlayer(error) => error.fmt(f),
            Self::Building {
                player,
                building,
                origin,
                error,
            } => write!(
                f,
                "starting building {building:?} of player {} at ({}, {}): {error}",
                player.0, origin.x, origin.y
            ),
            Self::Units(error) => error.fmt(f),
            Self::Unit {
                player,
                unit,
                cell,
                error,
            } => write!(
                f,
                "starting unit {unit:?} of player {} at ({}, {}): {error}",
                player.0, cell.x, cell.y
            ),
        }
    }
}

impl std::error::Error for MatchSetupError {}

/// How a match ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchOutcome {
    /// `winner` is the only player left with a building, from the tick with index `tick`.
    Won { winner: PlayerId, tick: u64 },
    /// The last players lost their last buildings in the same tick.
    Draw { tick: u64 },
}

/// The players of the match and who has been defeated (docs/DECISIONS.md D41). Without players
/// (a world set up only by `SimPlugin`) there is no win condition.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchState {
    players: Vec<PlayerId>,
    defeated: BTreeMap<PlayerId, u64>,
    outcome: Option<MatchOutcome>,
}

impl MatchState {
    /// The players of the match in ascending id order.
    pub fn players(&self) -> &[PlayerId] {
        &self.players
    }

    /// Whether `player` has been defeated, and the index of the tick in which it was.
    pub fn defeated(&self, player: PlayerId) -> Option<u64> {
        self.defeated.get(&player).copied()
    }

    /// The outcome, once the match has ended.
    pub fn outcome(&self) -> Option<MatchOutcome> {
        self.outcome
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.players.len() as u64);
        for player in &self.players {
            hasher.write(&[player.0]);
        }
        hasher.write_u64(self.defeated.len() as u64);
        for (player, tick) in &self.defeated {
            hasher.write(&[player.0]);
            hasher.write_u64(*tick);
        }
        match self.outcome {
            None => hasher.write(&[0]),
            Some(MatchOutcome::Won { winner, tick }) => {
                hasher.write(&[1, winner.0]);
                hasher.write_u64(tick);
            }
            Some(MatchOutcome::Draw { tick }) => {
                hasher.write(&[2]);
                hasher.write_u64(tick);
            }
        }
    }
}

/// Sets up a skirmish match on `map` (an index into `skirmish.maps`) for `players` (docs/DECISIONS.md
/// D40). The world must have been set up by [`SimPlugin`](crate::SimPlugin), with the map's grid and
/// the match definitions (units, economy, and combat files) installed, and no players yet.
///
/// The configuration is checked before anything is changed: from [`MIN_MATCH_PLAYERS`] players to
/// as many as the map has start positions (at most [`MAX_MATCH_PLAYERS`]; a match with more
/// players than the map file lists is set up on the map enlarged for them, `crate::map_scale`,
/// whose grid and start positions the caller installs), each with a
/// faction, a start position, and (for a computer player) a difficulty that exist; every start
/// position of the map is a cell of the map with passable terrain; and every computer player's
/// faction starts with the producers its AI file needs (`ai_defs::unobtainable_producer`) and
/// offers a complete plan (`ai_defs::faction_plan_problem`, M7).
///
/// Every player gets the roster of its faction (M7): it may queue only the items its faction
/// offers.
///
/// Every player starts with `skirmish.starting_credits`; its faction's buildings are placed with
/// their footprint origin at its start position plus their offset (the footprint rules apply, the
/// distance rule does not, as for `spawn_building`), then its faction's units on the free cells
/// nearest to the start position plus their offsets. Computer players get a controller with their
/// difficulty, or the setup fails if one has no gathering point. Returns what was placed; after an
/// error found once players were added, the world is partly set up and must be discarded.
pub fn setup_match(
    world: &mut World,
    skirmish: &SkirmishDefinitions,
    ai: &AiDefinitions,
    map: usize,
    players: &[PlayerSetup],
) -> Result<MatchStart, MatchSetupError> {
    setup_match_with_super_me(world, skirmish, ai, map, players, &[])
}

/// Match-local bonuses; only explicitly requested human controllers qualify. AI is never modified.
pub fn setup_match_with_super_me(
    world: &mut World,
    skirmish: &SkirmishDefinitions,
    ai: &AiDefinitions,
    map: usize,
    players: &[PlayerSetup],
    super_players: &[PlayerId],
) -> Result<MatchStart, MatchSetupError> {
    setup_match_with_options(world, skirmish, ai, map, players, super_players, &[])
}

/// [`setup_match_with_super_me`] with the "unlock all" option for the human controllers listed in
/// `unlock_players`: their building prerequisites are waived (producers, costs, power and
/// placement still apply). Computer players are never modified.
pub fn setup_match_with_options(
    world: &mut World,
    skirmish: &SkirmishDefinitions,
    ai: &AiDefinitions,
    map: usize,
    players: &[PlayerSetup],
    super_players: &[PlayerId],
    unlock_players: &[PlayerId],
) -> Result<MatchStart, MatchSetupError> {
    setup_match_configured(
        world,
        skirmish,
        ai,
        map,
        players,
        &MatchOptions {
            super_players: super_players.to_vec(),
            unlock_players: unlock_players.to_vec(),
            ..MatchOptions::default()
        },
    )
}

/// The team of the computer players of a match with [`MatchOptions::allied_ai`].
pub const AI_TEAM: u8 = 1;

/// Largest [`MatchOptions::unit_speed_percent`]: four times a unit type's speed.
pub const MAX_UNIT_SPEED_PERCENT: u16 = 400;

/// The options of a match beyond its players.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchOptions {
    /// Human controllers with Super Me.
    pub super_players: Vec<PlayerId>,
    /// Human controllers whose building prerequisites are waived.
    pub unlock_players: Vec<PlayerId>,
    /// The speed of the units of the Super Me players in percent of their type's speed, from
    /// 100 to [`MAX_UNIT_SPEED_PERCENT`] (values outside are clamped; 0 means 100). Players
    /// without Super Me are never changed.
    pub unit_speed_percent: u16,
    /// In a match with a human player, the computer players form one team: they do not fire at
    /// each other, do not march on each other's bases, and the match ends once only they are
    /// left. Without a human player nothing changes.
    pub allied_ai: bool,
}

/// [`setup_match`] with every match option.
pub fn setup_match_configured(
    world: &mut World,
    skirmish: &SkirmishDefinitions,
    ai: &AiDefinitions,
    map: usize,
    players: &[PlayerSetup],
    options: &MatchOptions,
) -> Result<MatchStart, MatchSetupError> {
    let super_players = options.super_players.as_slice();
    let unlock_players = options.unlock_players.as_slice();
    let allied_ai = options.allied_ai
        && players
            .iter()
            .any(|setup| setup.controller == Controller::Human);
    let speed_bonus = options
        .unit_speed_percent
        .clamp(100, MAX_UNIT_SPEED_PERCENT)
        - 100;
    let map_def = skirmish
        .maps
        .get(map)
        .ok_or(MatchSetupError::UnknownMap(map))?;
    {
        let grid = world.resource::<Grid>();
        let definitions = world.resource::<Definitions>();
        if grid.width() == 0 || grid.height() == 0 || definitions.buildings().is_empty() {
            return Err(MatchSetupError::NotInstalled);
        }
    }
    // Check the whole configuration before anything is changed.
    let most = map_def.starts.len().min(MAX_MATCH_PLAYERS);
    if !(MIN_MATCH_PLAYERS..=most).contains(&players.len()) {
        return Err(MatchSetupError::Players(format!(
            "{} players, a skirmish match on this map has {MIN_MATCH_PLAYERS} to {most}",
            players.len()
        )));
    }
    let mut ids = BTreeSet::new();
    let mut starts = BTreeSet::new();
    for setup in players {
        if !ids.insert(setup.player) {
            return Err(MatchSetupError::Players(format!(
                "player {} is given twice",
                setup.player.0
            )));
        }
        if setup.faction >= skirmish.factions.len() {
            return Err(MatchSetupError::UnknownFaction {
                player: setup.player,
                faction: setup.faction,
            });
        }
        if setup.start >= map_def.starts.len() {
            return Err(MatchSetupError::UnknownStart {
                player: setup.player,
                start: setup.start,
            });
        }
        if !starts.insert(setup.start) {
            return Err(MatchSetupError::Players(format!(
                "start position {} is given twice",
                setup.start
            )));
        }
        if let Controller::Ai { difficulty } = setup.controller {
            if difficulty >= ai.difficulties.len() {
                return Err(MatchSetupError::UnknownDifficulty {
                    player: setup.player,
                    difficulty,
                });
            }
            let definitions = world.resource::<Definitions>();
            if let Some((field, building, producer)) =
                unobtainable_producer(ai, definitions, &skirmish.factions[setup.faction])
            {
                let name = |kind| {
                    definitions
                        .building(kind)
                        .map_or_else(String::new, |def| def.id.clone())
                };
                return Err(MatchSetupError::UnobtainableProducer {
                    player: setup.player,
                    field,
                    building: name(building),
                    producer: name(producer),
                });
            }
            let faction = &skirmish.factions[setup.faction];
            if let Err((field, problem)) = faction_plan_problem(ai, definitions, faction) {
                return Err(MatchSetupError::AiPlan {
                    player: setup.player,
                    faction: faction.id.clone(),
                    field,
                    problem: problem.to_string(),
                });
            }
        }
    }
    // Every start position of the map is a cell of the map with passable terrain, so that the
    // buildings, units, gathering point, and waypoints derived from it lie on the map.
    {
        let grid = world.resource::<Grid>();
        for (start, &cell) in map_def.starts.iter().enumerate() {
            if let Some(problem) = start_problem(grid, cell) {
                return Err(MatchSetupError::InvalidStart {
                    start,
                    cell,
                    problem,
                });
            }
        }
    }

    let definitions = world.resource::<Definitions>().clone();
    let mut sorted: Vec<PlayerSetup> = players.to_vec();
    sorted.sort_unstable_by_key(|setup| setup.player);
    {
        let mut state = world.resource_mut::<Players>();
        for setup in &sorted {
            state
                .add_with_roster(
                    setup.player,
                    skirmish.starting_credits,
                    // A human with the unified arsenal may produce all of it and every faction's
                    // original roster (M7 units such as the Meridian's grenadier, raider, heavy
                    // tank and rocket launcher), which the arsenal-only roster had hidden.
                    if setup.controller == Controller::Human && !definitions.arsenal().0.is_empty()
                    {
                        definitions
                            .arsenal()
                            .0
                            .keys()
                            .copied()
                            .chain(
                                skirmish
                                    .factions
                                    .iter()
                                    .flat_map(|faction| faction.roster.iter().copied()),
                            )
                            .collect()
                    } else {
                        skirmish.factions[setup.faction].roster.clone()
                    },
                )
                .map_err(MatchSetupError::AddPlayer)?;
            if setup.controller == Controller::Human
                && super_players.contains(&setup.player)
                && let Some(player) = state.get_mut(setup.player)
            {
                player.super_me = true;
                player.unit_speed_bonus = speed_bonus;
            }
            if allied_ai
                && matches!(setup.controller, Controller::Ai { .. })
                && let Some(player) = state.get_mut(setup.player)
            {
                player.team = AI_TEAM;
            }
            if setup.controller == Controller::Human
                && unlock_players.contains(&setup.player)
                && let Some(player) = state.get_mut(setup.player)
            {
                player.unlock_all = true;
            }
        }
    }
    let definitions = world.resource::<Definitions>().clone();
    let offset = |start: CellPos, (dx, dy): (i32, i32)| {
        CellPos::new(start.x.saturating_add(dx), start.y.saturating_add(dy))
    };
    let mut buildings = 0;
    for setup in &sorted {
        let start = map_def.starts[setup.start];
        for building in &skirmish.factions[setup.faction].buildings {
            let name = definitions
                .building(building.kind)
                .map_or_else(String::new, |def| def.id.clone());
            let origin = offset(start, building.offset);
            spawn_building(world, setup.player, &name, origin).map_err(|error| {
                MatchSetupError::Building {
                    player: setup.player,
                    building: name.clone(),
                    origin,
                    error,
                }
            })?;
            buildings += 1;
        }
    }
    let names: Vec<(PlayerId, String, (i32, i32), usize)> = sorted
        .iter()
        .flat_map(|setup| {
            let start = map_def.starts[setup.start];
            skirmish.factions[setup.faction]
                .units
                .iter()
                .map(move |group| (setup, start, group))
        })
        .map(|(setup, start, group)| {
            let anchor = offset(start, group.offset);
            (
                setup.player,
                definitions
                    .unit(group.kind)
                    .map_or_else(String::new, |def| def.id.clone()),
                (anchor.x, anchor.y),
                usize::try_from(group.count).unwrap_or(usize::MAX),
            )
        })
        .collect();
    let groups: Vec<(PlayerId, &str, (i32, i32), usize)> = names
        .iter()
        .map(|(player, name, anchor, count)| (*player, name.as_str(), *anchor, *count))
        .collect();
    let planned =
        plan_starting_units(world.resource::<Grid>(), &groups).map_err(MatchSetupError::Units)?;
    let mut units = 0;
    for (player, unit, cell) in planned {
        spawn_unit(world, player, unit, cell).map_err(|error| MatchSetupError::Unit {
            player,
            unit: unit.to_owned(),
            cell,
            error,
        })?;
        units += 1;
    }

    let mut controllers = BTreeMap::new();
    for setup in &sorted {
        if let Controller::Ai { difficulty } = setup.controller {
            let enemies: Vec<CellPos> = sorted
                .iter()
                .filter(|other| {
                    world
                        .resource::<Players>()
                        .hostile(setup.player, other.player)
                })
                .map(|other| map_def.starts[other.start])
                .collect();
            let start = map_def.starts[setup.start];
            let controller = AiController::new(
                world.resource::<Grid>(),
                &definitions,
                ai.plan_for(&skirmish.factions[setup.faction]),
                ai.difficulties[difficulty].clone(),
                start,
                enemies,
            )
            .ok_or(MatchSetupError::NoGatheringPoint {
                player: setup.player,
                start,
            })?;
            controllers.insert(setup.player, controller);
        }
    }
    world.insert_resource(AiPlayers::new(ai.clone(), controllers));
    world.insert_resource(MatchState {
        players: sorted.iter().map(|setup| setup.player).collect(),
        defeated: BTreeMap::new(),
        outcome: None,
    });
    Ok(MatchStart { buildings, units })
}

/// The win condition (the `WinCondition` stage, docs/DECISIONS.md D41): players of the match who
/// own no building are defeated, and the match ends when at most one player is left. Buildings
/// removed in this tick (destroyed or sold) no longer count: their removal was applied at the sync
/// points before this stage.
pub(crate) fn check_win_condition(
    tick: Res<SimTick>,
    mut state: ResMut<MatchState>,
    mut events: ResMut<Messages<SimEvent>>,
    teams: Res<Players>,
    buildings: Query<&Owner, bevy_ecs::query::With<Building>>,
) {
    if state.players.is_empty() || state.outcome.is_some() {
        return;
    }
    let owners: BTreeSet<PlayerId> = buildings.iter().map(|owner| owner.0).collect();
    let players = state.players.clone();
    for player in players {
        if state.defeated.contains_key(&player) || owners.contains(&player) {
            continue;
        }
        state.defeated.insert(player, tick.0);
        events.write(SimEvent::PlayerDefeated { player });
    }
    let left: Vec<PlayerId> = state
        .players
        .iter()
        .copied()
        .filter(|player| !state.defeated.contains_key(player))
        .collect();
    let outcome = match left.as_slice() {
        [] => MatchOutcome::Draw { tick: tick.0 },
        [winner] => MatchOutcome::Won {
            winner: *winner,
            tick: tick.0,
        },
        // Only allies are left: nobody is going to fight any more. The first of them is
        // reported as the winner.
        [winner, rest @ ..] if rest.iter().all(|&other| !teams.hostile(*winner, other)) => {
            MatchOutcome::Won {
                winner: *winner,
                tick: tick.0,
            }
        }
        _ => return,
    };
    state.outcome = Some(outcome);
    events.write(SimEvent::MatchEnded {
        winner: match outcome {
            MatchOutcome::Won { winner, .. } => Some(winner),
            MatchOutcome::Draw { .. } => None,
        },
    });
}
