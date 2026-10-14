//! Skirmish match setup and the win condition (PLAN.md section 7, M6; docs/DECISIONS.md D40,
//! D41; RA1: docs/RA2_PARITY_PLAN.md sections 3.5 and 4.9, docs/DECISIONS.md R13).
//!
//! [`setup_match`] installs a match into a world set up by [`SimPlugin`](crate::SimPlugin) with the
//! map's grid and the definitions installed: it adds the players with their starting credits,
//! places what every player starts with at its start position, gives every computer player its
//! controller (`crate::ai`), and records the players and the rules of the match in
//! [`MatchState`]. Like installing the grid (docs/DECISIONS.md D11), this is setup before the
//! first tick, not a gameplay change, so it does not go through `CommandQueue`.
//!
//! What a player starts with depends on its faction (`crate::skirmish_defs`): a faction without
//! a side gives its starting buildings and groups of units, as since M6; a country gives the
//! base unit of its side at the start position (an MCV, or with [`MatchOptions::mcv_start`] off
//! the Construction Yard it deploys into) and the starting units of [`starting_units`].
//!
//! The win condition (the `WinCondition` stage): at the end of every tick, a player of the match
//! who has lost is defeated ([`SimEvent::PlayerDefeated`]); once at most one player is left, or
//! only allies, the match ends ([`SimEvent::MatchEnded`]) with that player as the winner, or as a
//! draw if the last players were defeated in the same tick. The outcome is final. A player has
//! lost when it owns no building and, in a world with a rules file (RA1), in a short game no
//! base unit either, in a long game no unit at all; units carried in a transport count for its
//! owner. A world without a rules file keeps "no building" (D41). A world without a match set up
//! (no players in [`MatchState`], as in the tests of M0 to M5) has no win condition.

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
use crate::definitions::{Definitions, UnitKind};
use crate::economy::{PlayerSetupError, Players};
use crate::economy_defs::ProductionItem;
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::map::{CellPos, Grid};
use crate::pathfinding::destination_cells;
use crate::rng::SimRng;
use crate::rules::Side;
use crate::schedule::SimTick;
use crate::skirmish_defs::{
    MAX_GROUP_UNITS, MAX_MATCH_PLAYERS, MIN_MATCH_PLAYERS, SkirmishDefinitions, StartProblem,
    start_problem,
};
use crate::unit::{Owner, SpawnError, Unit, spawn_unit};

/// Who gives a player's orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Controller {
    /// A person, through the interface.
    Human,
    /// The computer, with the difficulty of this index in the AI file.
    Ai { difficulty: usize },
}

/// A player of a match to be set up: its id, the index of its faction in the skirmish file (not
/// read for a player listed in [`MatchOptions::random_countries`]), the index of its start
/// position on the map, and who controls it.
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
    /// An option of the match is outside what the rules file allows (RA1): the starting credits
    /// or the unit count outside their range, or a random country without any country.
    Options(String),
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
            Self::Options(problem) => write!(f, "options: {problem}"),
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

/// The rules of a match that its options set (RA1, docs/RA2_PARITY_PLAN.md section 3.5). The
/// default is that of Red Alert 2's lobby, which is also what a world without a match has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchRules {
    /// A player is defeated without buildings and base unit; off: without buildings and units.
    /// Read only in a world with a rules file.
    pub short_game: bool,
    /// Superweapon buildings may be queued.
    pub superweapons: bool,
    /// `Command::Undeploy` is accepted.
    pub mcv_repacks: bool,
    /// Crates appear (RA5, `crate::crates`): in a match with players, in a world with a rules
    /// file.
    pub crates: bool,
    /// Ore grows and spreads (RA5, `crate::ore`; the lobby's `TiberiumGrows`). Read only in a
    /// world with a rules file.
    pub ore_grows: bool,
    pub bridge_destruction: bool,
}

impl Default for MatchRules {
    fn default() -> Self {
        Self {
            short_game: true,
            superweapons: true,
            mcv_repacks: true,
            crates: true,
            ore_grows: true,
            bridge_destruction: true,
        }
    }
}

/// The players of the match, its rules, and who has been defeated (docs/DECISIONS.md D41).
/// Without players (a world set up only by `SimPlugin`) there is no win condition.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchState {
    players: Vec<PlayerId>,
    rules: MatchRules,
    defeated: BTreeMap<PlayerId, u64>,
    outcome: Option<MatchOutcome>,
}

impl MatchState {
    /// The rules the options of the match set.
    pub fn rules(&self) -> MatchRules {
        self.rules
    }

    /// Sets the rules of the match. Scenario and match setup only, like [`Players::add`].
    pub fn set_rules(&mut self, rules: MatchRules) {
        self.rules = rules;
    }

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
        hasher.write(&[
            u8::from(self.rules.short_game),
            u8::from(self.rules.superweapons),
            u8::from(self.rules.mcv_repacks),
            u8::from(self.rules.crates),
        ]);
        // RA5: written only when it differs from the default, so that a match with the
        // default hashes as before.
        if !self.rules.bridge_destruction {
            hasher.write(b"bridges indestructible");
        }
        if !self.rules.ore_grows {
            hasher.write(b"ore stands still");
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
            // As before the options existed: no teams.
            free_for_all: true,
            ..MatchOptions::default()
        },
    )
}

/// The team of the computer players of a match without [`MatchOptions::free_for_all`].
pub const AI_TEAM: u8 = 1;

/// Fewest cells (Chebyshev distance) between a country's start position and its starting units
/// (docs/RA2_PARITY_PLAN.md section 4.9): the base unit deploys into a Construction Yard whose
/// footprint begins at the start, and the units stand clear of it.
pub const STARTING_UNIT_DISTANCE: u32 = 3;

/// Largest [`MatchOptions::unit_speed_percent`]: four times a unit type's speed.
pub const MAX_UNIT_SPEED_PERCENT: u16 = 400;

/// The options of a match beyond its players (docs/RA2_PARITY_PLAN.md section 3.5). The default
/// is that of Red Alert 2's lobby; the extensions of this game default to off.
///
/// Whether the map grows with the number of players is not an option of the simulation: the map
/// is enlarged before the setup (`crate::map_scale::enlarged_map`), by whoever installs the grid
/// and the start positions (docs/DECISIONS.md R4, R13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOptions {
    /// Human controllers with Super Me.
    pub super_players: Vec<PlayerId>,
    /// Human controllers whose building prerequisites are waived.
    pub unlock_players: Vec<PlayerId>,
    /// The speed of the units of the Super Me players in percent of their type's speed, from
    /// 100 to [`MAX_UNIT_SPEED_PERCENT`] (values outside are clamped; 0 means 100). Players
    /// without Super Me are never changed.
    pub unit_speed_percent: u16,
    /// Off (the default, fixed decision 3): in a match with a human player, the computer players
    /// form one team: they do not fire at each other, do not march on each other's bases, and
    /// the match ends once only they are left; without a human player nothing changes. On:
    /// every player for itself.
    pub free_for_all: bool,
    /// Players whose faction is drawn from the countries of the skirmish file with `SimRng`
    /// (the lobby's "random"), in ascending order of their ids; their `PlayerSetup::faction` is
    /// not read.
    pub random_countries: Vec<PlayerId>,
    /// The starting credits of every player; `None` for those of the skirmish file. With a
    /// rules file they must lie in its range and on its step (`MinMoney`, `MaxMoney`,
    /// `MoneyIncrement`).
    pub starting_credits: Option<i64>,
    /// The lobby's unit count, from which the starting units of a country's players follow
    /// ([`starting_units`]); `None` for the default of the rules file. It must lie in the
    /// range of the rules file.
    pub unit_count: Option<u32>,
    /// A country's player starts with the base unit of its side; off: with the Construction
    /// Yard it deploys into, standing at the start position.
    pub mcv_start: bool,
    /// See [`MatchRules`].
    pub short_game: bool,
    pub superweapons: bool,
    pub crates: bool,
    pub mcv_repacks: bool,
    pub ore_grows: bool,
    pub bridge_destruction: bool,
}

impl Default for MatchOptions {
    fn default() -> Self {
        let rules = MatchRules::default();
        Self {
            super_players: Vec::new(),
            unlock_players: Vec::new(),
            unit_speed_percent: 0,
            free_for_all: false,
            random_countries: Vec::new(),
            starting_credits: None,
            unit_count: None,
            mcv_start: true,
            short_game: rules.short_game,
            superweapons: rules.superweapons,
            crates: rules.crates,
            mcv_repacks: rules.mcv_repacks,
            ore_grows: rules.ore_grows,
            bridge_destruction: rules.bridge_destruction,
        }
    }
}

/// The starting units of a player of `side` in a match with the lobby's unit count `unit_count`
/// (docs/RA2_PARITY_PLAN.md section 4.9; single source, appendix D topic 15):
///
/// ```text
/// average = mean cost of the starting unit types of all sides, rounded down
/// budget  = (unit_count - 1) x average          (0 for a unit count of 0)
/// while spent < budget: a vehicle of the side while spent < budget x 2 div 3 (if it has one),
///                       else an infantry of the side;  spent += its cost
/// ```
///
/// A type is drawn from its list with `rng` when the list has more than one entry (vanilla Red
/// Alert 2 has one vehicle and one infantry per side, so nothing is drawn). At most
/// `MAX_GROUP_UNITS` units; none for a side the skirmish file does not list.
pub fn starting_units(
    skirmish: &SkirmishDefinitions,
    definitions: &Definitions,
    side: Side,
    unit_count: u32,
    rng: &mut SimRng,
) -> Vec<UnitKind> {
    let mut units = Vec::new();
    let Some(start) = skirmish.side(side) else {
        return units;
    };
    let budget = i64::from(unit_count.saturating_sub(1))
        .saturating_mul(skirmish.average_starting_cost(definitions));
    let mut pick = |list: &[UnitKind]| -> Option<UnitKind> {
        match list.len() {
            0 => None,
            1 => list.first().copied(),
            // Bounded by `MAX_STARTING_TYPES`, so the remainder is an index.
            length => list
                .get(usize::try_from(rng.next_u64() % (length as u64)).unwrap_or(0))
                .copied(),
        }
    };
    let mut spent = 0_i64;
    while spent < budget && units.len() < MAX_GROUP_UNITS as usize {
        let vehicle = (spent < budget.saturating_mul(2) / 3)
            .then(|| pick(&start.vehicles))
            .flatten();
        let Some(kind) = vehicle.or_else(|| pick(&start.infantry)) else {
            break;
        };
        // Every starting unit type has a cost above 0 (`skirmish_defs`).
        let cost = definitions
            .production(ProductionItem::Unit(kind))
            .map_or(0, |terms| terms.cost);
        if cost <= 0 {
            break;
        }
        spent = spent.saturating_add(cost);
        units.push(kind);
    }
    units
}

/// [`setup_match`] with every match option (RA1). Beyond the checks of [`setup_match`], the
/// options are checked against the rules file before anything is changed; then the factions of
/// the players with a random country are drawn (the only use of `SimRng` here besides
/// [`starting_units`]).
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
    let allied_ai = !options.free_for_all
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
    let definitions = world.resource::<Definitions>().clone();
    let dialog = definitions.rules().map(|rules| rules.dialog);
    let starting_credits = options
        .starting_credits
        .unwrap_or(skirmish.starting_credits);
    if let (Some(credits), Some(dialog)) = (options.starting_credits, dialog) {
        let (low, high, step) = (
            i64::from(dialog.min_credits),
            i64::from(dialog.max_credits),
            i64::from(dialog.credits_step.max(1)),
        );
        if !(low..=high).contains(&credits) || (credits - low) % step != 0 {
            return Err(MatchSetupError::Options(format!(
                "{credits} starting credits, the rules allow {low} to {high} in steps of {step}"
            )));
        }
    }
    let unit_count = match (options.unit_count, dialog) {
        (Some(count), Some(dialog)) => {
            let (low, high) = (dialog.min_unit_count, dialog.max_unit_count);
            if !(i64::from(low)..=i64::from(high)).contains(&i64::from(count)) {
                return Err(MatchSetupError::Options(format!(
                    "a unit count of {count}, the rules allow {low} to {high}"
                )));
            }
            count
        }
        (Some(count), None) => count,
        (None, Some(dialog)) => u32::try_from(dialog.unit_count).unwrap_or(0),
        (None, None) => 0,
    };
    // The checks of a player's faction: it exists, and a computer player can play it.
    let check_faction = |setup: &PlayerSetup| -> Result<(), MatchSetupError> {
        let Some(faction) = skirmish.factions.get(setup.faction) else {
            return Err(MatchSetupError::UnknownFaction {
                player: setup.player,
                faction: setup.faction,
            });
        };
        if matches!(setup.controller, Controller::Ai { .. }) {
            if let Some((field, building, producer)) =
                unobtainable_producer(ai, &definitions, faction)
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
            if let Err((field, problem)) = faction_plan_problem(ai, &definitions, faction) {
                return Err(MatchSetupError::AiPlan {
                    player: setup.player,
                    faction: faction.id.clone(),
                    field,
                    problem: problem.to_string(),
                });
            }
        }
        Ok(())
    };
    let random = |player: PlayerId| options.random_countries.contains(&player);
    let mut ids = BTreeSet::new();
    let mut starts = BTreeSet::new();
    for setup in players {
        if setup.player == crate::NEUTRAL {
            return Err(MatchSetupError::Players(
                "neutral owner cannot compete".into(),
            ));
        }
        if !ids.insert(setup.player) {
            return Err(MatchSetupError::Players(format!(
                "player {} is given twice",
                setup.player.0
            )));
        }
        if !random(setup.player) && setup.faction >= skirmish.factions.len() {
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
        if let Controller::Ai { difficulty } = setup.controller
            && difficulty >= ai.difficulties.len()
        {
            return Err(MatchSetupError::UnknownDifficulty {
                player: setup.player,
                difficulty,
            });
        }
        if !random(setup.player) {
            check_faction(setup)?;
        }
    }
    let countries = skirmish.countries();
    if let Some(player) = options
        .random_countries
        .iter()
        .find(|player| !ids.contains(player) || countries.is_empty())
    {
        return Err(MatchSetupError::Options(format!(
            "a random country for player {}, who is not a player of the match or has no \
             country of the skirmish file to draw from",
            player.0
        )));
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

    let mut sorted: Vec<PlayerSetup> = players.to_vec();
    sorted.sort_unstable_by_key(|setup| setup.player);
    // The random countries, in ascending order of the players' ids.
    for setup in &mut sorted {
        if random(setup.player) {
            let draw = world.resource_mut::<SimRng>().next_u64();
            // `countries` is not empty (checked above) and far smaller than `u64::MAX`.
            let index = usize::try_from(draw % (countries.len().max(1) as u64)).unwrap_or(0);
            setup.faction = countries.get(index).copied().unwrap_or(0);
        }
    }
    for setup in sorted.iter().filter(|setup| random(setup.player)) {
        check_faction(setup)?;
    }

    // RA5 (docs/RA2_PARITY_PLAN.md section 4.10): with a rules file the ore amounts of the map
    // file become bails, at most the densest ore of the rules.
    if let Some(economy) = definitions.rules().map(|rules| rules.economy) {
        let value = |credits: i32| u32::try_from(credits.max(1)).unwrap_or(1);
        world.resource_mut::<Grid>().ore_to_bails(
            value(economy.ore_value),
            value(economy.gem_value),
            value(economy.ore_max_bails),
        );
    }

    {
        let mut state = world.resource_mut::<Players>();
        for setup in &sorted {
            let faction = &skirmish.factions[setup.faction];
            match faction.side {
                // A country's player has no roster: the buildability rule decides (RA1).
                Some(_) => state.add(setup.player, starting_credits),
                None => state.add_with_roster(
                    setup.player,
                    starting_credits,
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
                                    .filter(|faction| faction.side.is_none())
                                    .flat_map(|faction| faction.roster.iter().copied()),
                            )
                            .collect()
                    } else {
                        faction.roster.clone()
                    },
                ),
            }
            .map_err(MatchSetupError::AddPlayer)?;
            state.set_country(setup.player, faction.country);
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
    let offset = |start: CellPos, (dx, dy): (i32, i32)| {
        CellPos::new(start.x.saturating_add(dx), start.y.saturating_add(dy))
    };
    let building_name = |kind| {
        definitions
            .building(kind)
            .map_or_else(String::new, |def| def.id.clone())
    };
    let unit_name = |kind| {
        definitions
            .unit(kind)
            .map_or_else(String::new, |def| def.id.clone())
    };
    let mut buildings = 0;
    let mut units = 0;
    for setup in &sorted {
        let start = map_def.starts[setup.start];
        let faction = &skirmish.factions[setup.faction];
        for building in &faction.buildings {
            let name = building_name(building.kind);
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
        // A country: the base unit of its side at the start position, or the Construction Yard
        // it deploys into with its footprint's origin there, which is where deploying the base
        // unit in place puts it.
        let Some(side) = faction
            .side
            .and_then(|side| definitions.rules().and_then(|rules| rules.side(side)))
        else {
            continue;
        };
        if options.mcv_start {
            let name = unit_name(side.base_unit);
            spawn_unit(world, setup.player, &name, start).map_err(|error| {
                MatchSetupError::Unit {
                    player: setup.player,
                    unit: name.clone(),
                    cell: start,
                    error,
                }
            })?;
            units += 1;
        } else {
            let name = building_name(side.construction_yard);
            spawn_building(world, setup.player, &name, start).map_err(|error| {
                MatchSetupError::Building {
                    player: setup.player,
                    building: name.clone(),
                    origin: start,
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
                unit_name(group.kind),
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
    for (player, unit, cell) in planned {
        spawn_unit(world, player, unit, cell).map_err(|error| MatchSetupError::Unit {
            player,
            unit: unit.to_owned(),
            cell,
            error,
        })?;
        units += 1;
    }
    // The starting units of the countries' players: on the free cells nearest to the start
    // position, at least `STARTING_UNIT_DISTANCE` cells from it.
    for setup in &sorted {
        let Some(side) = skirmish.factions[setup.faction].side else {
            continue;
        };
        let start = map_def.starts[setup.start];
        let kinds = starting_units(
            skirmish,
            &definitions,
            side,
            unit_count,
            &mut world.resource_mut::<SimRng>(),
        );
        let cells = {
            let grid = world.resource::<Grid>();
            destination_cells(grid, start, kinds.len(), |cell| {
                grid.occupant(cell).is_some()
                    || cell.x.abs_diff(start.x).max(cell.y.abs_diff(start.y))
                        < STARTING_UNIT_DISTANCE
            })
        };
        if cells.len() < kinds.len() {
            return Err(MatchSetupError::Units(StartError {
                owner: setup.player,
                unit_type: kinds
                    .first()
                    .map_or_else(String::new, |&kind| unit_name(kind)),
                anchor: start,
                found: cells.len(),
                required: kinds.len(),
            }));
        }
        for (kind, cell) in kinds.into_iter().zip(cells) {
            let name = unit_name(kind);
            spawn_unit(world, setup.player, &name, cell).map_err(|error| {
                MatchSetupError::Unit {
                    player: setup.player,
                    unit: name.clone(),
                    cell,
                    error,
                }
            })?;
            units += 1;
        }
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
            let faction = &skirmish.factions[setup.faction];
            // `faction_plan_problem` found the plan of the faction's side above.
            let plan = ai
                .plan_for(faction)
                .ok_or_else(|| MatchSetupError::AiPlan {
                    player: setup.player,
                    faction: faction.id.clone(),
                    field: "plans".into(),
                    problem: "no plan for the side of the faction".into(),
                })?;
            // The yard a base unit deploys into, where it stands: the start position.
            let yard = faction
                .side
                .filter(|_| options.mcv_start)
                .and_then(|side| definitions.rules()?.side(side))
                .and_then(|side| definitions.building(side.construction_yard))
                .map(|def| crate::building::Footprint {
                    origin: start,
                    width: def.width,
                    height: def.height,
                });
            let controller = AiController::new(
                world.resource::<Grid>(),
                &definitions,
                plan,
                ai.difficulties[difficulty].clone(),
                start,
                enemies,
                yard,
            )
            .ok_or(MatchSetupError::NoGatheringPoint {
                player: setup.player,
                start,
            })?;
            controllers.insert(setup.player, controller);
        }
    }
    crate::world_objects::install(world, options.bridge_destruction);
    world.insert_resource(AiPlayers::new(ai.clone(), controllers));
    world.insert_resource(MatchState {
        players: sorted.iter().map(|setup| setup.player).collect(),
        rules: MatchRules {
            short_game: options.short_game,
            superweapons: options.superweapons,
            mcv_repacks: options.mcv_repacks,
            crates: options.crates,
            ore_grows: options.ore_grows,
            bridge_destruction: options.bridge_destruction,
        },
        defeated: BTreeMap::new(),
        outcome: None,
    });
    Ok(MatchStart { buildings, units })
}

/// The win condition (the `WinCondition` stage, docs/DECISIONS.md D41, R13): players of the match
/// who have lost are defeated, and the match ends when at most one player, or only allies, are
/// left. Without a rules file a player has lost when it owns no building; with one (RA1), when
/// it owns no building and, in a short game, no base unit of any side, or in a long game no unit
/// at all (a unit carried by one of its transports counts). Buildings and units removed in this
/// tick (destroyed or sold) no longer count: their removal was applied at the sync points before
/// this stage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_win_condition(
    tick: Res<SimTick>,
    mut state: ResMut<MatchState>,
    mut events: ResMut<Messages<SimEvent>>,
    teams: Res<Players>,
    definitions: Res<Definitions>,
    special: Res<crate::SpecialState>,
    buildings: Query<(&crate::SimId, &Owner, &Building)>,
    units: Query<(&crate::SimId, &Owner, &Unit)>,
) {
    if state.players.is_empty() || state.outcome.is_some() {
        return;
    }
    let mut alive: BTreeSet<PlayerId> = buildings
        .iter()
        .filter(|(_, owner, building)| {
            owner.0 != crate::NEUTRAL
                && !definitions
                    .metadata(ProductionItem::Building(building.kind))
                    .is_some_and(|entry| entry.insignificant)
        })
        .map(|(_, owner, _)| owner.0)
        .collect();
    if let Some(rules) = definitions.rules() {
        let short_game = state.rules.short_game;
        let counts = |kind: UnitKind| !short_game || rules.is_base_unit(kind);
        if !short_game {
            for (id, owner, _) in &buildings {
                if owner.0 != crate::NEUTRAL
                    && special.cargo.get(id).is_some_and(|crew| !crew.is_empty())
                {
                    alive.insert(owner.0);
                }
            }
            for (id, g) in &special.garrisons {
                if special.cargo.get(id).is_some_and(|crew| !crew.is_empty()) {
                    alive.insert(g.owner);
                }
            }
        }
        for (id, owner, unit) in &units {
            if counts(unit.kind)
                || special
                    .cargo
                    .get(id)
                    .is_some_and(|cargo| cargo.iter().any(|passenger| counts(passenger.kind)))
            {
                alive.insert(owner.0);
            }
        }
    }
    let players = state.players.clone();
    for player in players {
        if state.defeated.contains_key(&player) || alive.contains(&player) {
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
